//! Phase 2: stream the records, hash stored and inflated bytes, classify the compression.

use std::fs::File;
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::{Decompress, FlushDecompress};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use super::{
    corrupt, limit_err, looks_like_nested, ticks_text, EntryMeta, ScanWarning, ScannedContainer, Src, MAX_INFLATED_PER_RECORD,
    MAX_INFLATED_TOTAL, WARN_ENTRY_INVALID_COMPRESSION,
};
use crate::errors::{ExtractionError, Result, IO_ERROR};

/// Bytes read from the file per step, and size of the inflate output buffer.
const BUF: usize = 64 * 1024;
/// Bytes of the (inflated or stored) record kept for the `nested` test: signature + file header + block header.
const HEAD: usize = 47;
/// Largest `version` record that is inflated for the compatibility version.
const VERSION_CAP: u64 = 1 << 20;

/// How the stored bytes of a record are packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// One raw DEFLATE stream that ends exactly at the last stored byte.
    Deflate,
    /// Empty, or does not inflate and is itself a well-formed container.
    None,
    /// Anything else.
    Invalid,
}

impl Compression {
    pub fn as_str(self) -> &'static str {
        match self {
            Compression::Deflate => "deflate",
            Compression::None => "none",
            Compression::Invalid => "invalid",
        }
    }
}

/// Facts about one record, from phase 2. The inflated fields are `Some` only for `Compression::Deflate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFacts {
    pub name: String,
    pub stored_size: u64,
    pub stored_sha256: String,
    pub compression: Compression,
    pub inflated_size: Option<u64>,
    pub inflated_sha256: Option<String>,
    pub nested: bool,
    pub created: u64,
    pub modified: u64,
    pub created_text: Option<String>,
    pub modified_text: Option<String>,
}

struct Bufs {
    input: Vec<u8>,
    output: Vec<u8>,
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Streaming raw-DEFLATE state of one record.
struct Inflater<'a> {
    dec: Decompress,
    hasher: Sha256,
    size: u64,
    head: Vec<u8>,
    /// Decoder reached the end of the stream.
    ended: bool,
    /// The decoder reported an error.
    failed: bool,
    /// Stored bytes after the end of the stream.
    extra: u64,
    total: &'a AtomicU64,
    name: &'a str,
}

impl<'a> Inflater<'a> {
    fn new(total: &'a AtomicU64, name: &'a str) -> Inflater<'a> {
        Inflater {
            dec: Decompress::new(false),
            hasher: Sha256::new(),
            size: 0,
            head: Vec::with_capacity(HEAD),
            ended: false,
            failed: false,
            extra: 0,
            total,
            name,
        }
    }

    fn feed(&mut self, input: &[u8], out: &mut [u8]) -> Result<()> {
        if self.failed {
            return Ok(());
        }
        if self.ended {
            self.extra += input.len() as u64;
            return Ok(());
        }
        let mut pos = 0usize;
        loop {
            let (in0, out0) = (self.dec.total_in(), self.dec.total_out());
            let r = self.dec.decompress(&input[pos..], out, FlushDecompress::None);
            let used = (self.dec.total_in() - in0) as usize;
            let produced = (self.dec.total_out() - out0) as usize;
            pos += used;
            if produced > 0 {
                self.size += produced as u64;
                if self.size > MAX_INFLATED_PER_RECORD {
                    return Err(limit_err("maxInflatedPerRecord", "inflated record exceeds the limit").with("name", self.name));
                }
                if self.total.fetch_add(produced as u64, Ordering::Relaxed) + produced as u64 > MAX_INFLATED_TOTAL {
                    return Err(limit_err("maxInflatedTotal", "inflated bytes of all records exceed the limit"));
                }
                self.hasher.update(&out[..produced]);
                if self.head.len() < HEAD {
                    let take = (HEAD - self.head.len()).min(produced);
                    self.head.extend_from_slice(&out[..take]);
                }
            }
            match r {
                Err(_) => {
                    self.failed = true;
                    return Ok(());
                }
                Ok(flate2::Status::StreamEnd) => {
                    self.ended = true;
                    self.extra += (input.len() - pos) as u64;
                    return Ok(());
                }
                Ok(_) => {}
            }
            if (pos == input.len() && produced < out.len()) || (used == 0 && produced == 0) {
                return Ok(());
            }
        }
    }
}

fn process_one(src: &Src, e: &EntryMeta, bufs: &mut Bufs, total: &AtomicU64) -> Result<EntryFacts> {
    let mut stored = Sha256::new();
    let mut stored_head: Vec<u8> = Vec::with_capacity(HEAD);
    let mut inf = Inflater::new(total, &e.name);
    for &(off, len) in &e.chunks {
        let mut done = 0u64;
        while done < len {
            let n = (len - done).min(BUF as u64) as usize;
            src.read(off + done, &mut bufs.input[..n])?;
            let piece = &bufs.input[..n];
            stored.update(piece);
            if stored_head.len() < HEAD {
                let take = (HEAD - stored_head.len()).min(n);
                stored_head.extend_from_slice(&piece[..take]);
            }
            inf.feed(piece, &mut bufs.output)?;
            done += n as u64;
        }
    }
    let deflate = inf.ended && !inf.failed && inf.extra == 0;
    let (compression, probe) = if deflate {
        (Compression::Deflate, &inf.head)
    } else if e.stored_size == 0 || looks_like_nested(&stored_head) {
        (Compression::None, &stored_head)
    } else {
        (Compression::Invalid, &stored_head)
    };
    let nested = compression != Compression::Invalid && looks_like_nested(probe);
    Ok(EntryFacts {
        name: e.name.clone(),
        stored_size: e.stored_size,
        stored_sha256: hex(&stored.finalize()),
        compression,
        inflated_size: deflate.then_some(inf.size),
        inflated_sha256: deflate.then(|| hex(&inf.hasher.clone().finalize())),
        nested,
        created: e.created,
        modified: e.modified,
        created_text: ticks_text(e.created),
        modified_text: ticks_text(e.modified),
    })
}

fn open_checked(c: &ScannedContainer) -> Result<File> {
    let file = File::open(&c.path).map_err(|e| ExtractionError::new(IO_ERROR, e.to_string()).with("path", c.path.display().to_string()))?;
    if file.metadata()?.len() != c.file_size {
        return Err(corrupt("file changed while it was being read", 0));
    }
    Ok(file)
}

/// Phase 2: facts about every record, sorted by name, identical for every `workers` value.
///
/// Records are streamed block by block by a pool of `workers` threads sharing one file handle with positioned
/// reads; each worker holds two 64 KiB buffers, whatever the size of the record.
/// A record that inflates beyond 4 GiB (or all records beyond 64 GiB) fails with `CONTAINER_LIMIT_EXCEEDED`.
pub fn process_entries(c: &ScannedContainer, workers: usize) -> Result<Vec<EntryFacts>> {
    let file = open_checked(c)?;
    let src = Src { file: &file, size: c.file_size };
    let total = AtomicU64::new(0);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.max(1))
        .build()
        .map_err(|e| ExtractionError::new(IO_ERROR, e.to_string()))?;
    pool.install(|| {
        c.entries
            .par_iter()
            .map_init(
                || Bufs { input: vec![0u8; BUF], output: vec![0u8; BUF] },
                |bufs, e| process_one(&src, e, bufs, &total),
            )
            .collect::<Result<Vec<_>>>()
    })
}

/// `ENTRY_INVALID_COMPRESSION` for every record whose compression is `invalid`.
pub fn warnings_of(facts: &[EntryFacts]) -> Vec<ScanWarning> {
    facts
        .iter()
        .filter(|f| f.compression == Compression::Invalid)
        .map(|f| ScanWarning {
            code: WARN_ENTRY_INVALID_COMPRESSION.into(),
            message: format!("record {:?} is neither a raw DEFLATE stream nor a container", f.name),
            entry: Some(f.name.clone()),
        })
        .collect()
}

/// First `{8NNNN,` in the text of the `version` record: the compatibility version (e.g. 80315). `None` when there is
/// no `version` record, it is not a DEFLATE stream (or larger than 1 MiB inflated), or it holds no such number.
pub fn compatibility_version(c: &ScannedContainer) -> Result<Option<u32>> {
    let Some(e) = c.entries.iter().find(|e| e.name == "version") else { return Ok(None) };
    if e.stored_size > VERSION_CAP {
        return Ok(None);
    }
    let file = open_checked(c)?;
    let src = Src { file: &file, size: c.file_size };
    let mut stored = vec![0u8; e.stored_size as usize];
    let mut at = 0usize;
    for &(off, len) in &e.chunks {
        src.read(off, &mut stored[at..at + len as usize])?;
        at += len as usize;
    }
    let mut dec = Decompress::new(false);
    let mut out: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; BUF];
    let mut pos = 0usize;
    loop {
        let (in0, out0) = (dec.total_in(), dec.total_out());
        let r = dec.decompress(&stored[pos..], &mut buf, FlushDecompress::None);
        let used = (dec.total_in() - in0) as usize;
        let produced = (dec.total_out() - out0) as usize;
        pos += used;
        out.extend_from_slice(&buf[..produced]);
        if out.len() as u64 > VERSION_CAP {
            return Ok(None);
        }
        match r {
            Err(_) => return Ok(None),
            Ok(flate2::Status::StreamEnd) => break,
            Ok(_) => {}
        }
        if used == 0 && produced == 0 {
            return Ok(None);
        }
    }
    if pos != stored.len() {
        return Ok(None);
    }
    Ok(find_version(&out))
}

fn find_version(text: &[u8]) -> Option<u32> {
    text.windows(7).find_map(|w| {
        (w[0] == b'{' && w[1] == b'8' && w[2..6].iter().all(u8::is_ascii_digit) && w[6] == b',')
            .then(|| std::str::from_utf8(&w[1..6]).ok()?.parse().ok())
            .flatten()
    })
}
