//! Read-only streaming reader of 1C containers (`.cf`, `.cfe`, `.epf`, `.erf`) in every known layout.
//!
//! Independent of `crate::container` (the in-memory 32-bit reader used by the forms code); only the name rules are shared.
//!
//! Two phases:
//! 1. [`open`] (serial, small): detects the layout, reads the table of contents and the record headers, and keeps
//!    for every record only the list of its data chunks `(file offset, length)`. Record data is never read.
//! 2. [`process_entries`] (parallel): streams every record block by block, hashes the stored bytes, inflates them
//!    as raw DEFLATE, hashes the inflated bytes and classifies the record. Memory per worker is a fixed buffer.
//!
//! Layouts: *32-bit* (file header 16 bytes, block header 31 bytes with three 8-digit hexadecimal fields, table of
//! contents record 12 bytes), *64-bit* (file header 20, block header 55 with 16-digit fields, record 24; every
//! address counts from the start of the 64-bit container). A 64-bit container may follow a small 32-bit stub
//! (`64bit-stub`, a `.cf` of compatibility mode 8.3.16+). Both are driven by one [`Layout`] value.
//! A block occupies its header plus its page size; its data is `min(page size, rest of the document)` bytes.
//!
//! Fixed limits (not configurable): input 16 GiB, 2,000,000 records, 4 GiB inflated per record, 64 GiB inflated in total.

mod process;

use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::container::validate_entry_name;
use crate::errors::{
    ExtractionError, Result, CONTAINER_CORRUPT, CONTAINER_CYCLE, CONTAINER_DUPLICATE_ENTRY, CONTAINER_LAYOUT_UNSUPPORTED,
    CONTAINER_LIMIT_EXCEEDED, CONTAINER_TRAILING_DATA,
};

pub use process::{compatibility_version, process_entries, warnings_of, Compression, EntryFacts};

/// Largest input file.
pub const MAX_INPUT_BYTES: u64 = 16 << 30;
/// Largest number of records.
pub const MAX_RECORDS: u64 = 2_000_000;
/// Largest inflated size of one record.
pub const MAX_INFLATED_PER_RECORD: u64 = 4 << 30;
/// Largest sum of inflated sizes of all records.
pub const MAX_INFLATED_TOTAL: u64 = 64 << 30;
/// Largest record header document (two dates, 4 bytes, a name) that is read into memory.
const MAX_HEADER_DOC: u64 = 1 << 20;

pub const WARN_HEADER_ENTRY_COUNT_MISMATCH: &str = "HEADER_ENTRY_COUNT_MISMATCH";
pub const WARN_LAYOUT_UNVERIFIED: &str = "LAYOUT_UNVERIFIED";
pub const WARN_ENTRY_INVALID_COMPRESSION: &str = "ENTRY_INVALID_COMPRESSION";

/// Layout of the container found in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    /// Plain 32-bit container.
    Bits32,
    /// 32-bit stub followed by the 64-bit container (`container_offset` = end of the stub).
    Bits64Stub,
    /// 64-bit container at offset 0 (never seen on a real file, `layout_verified` is false).
    Bits64,
}

impl LayoutKind {
    pub fn as_str(self) -> &'static str {
        match self {
            LayoutKind::Bits32 => "32bit",
            LayoutKind::Bits64Stub => "64bit-stub",
            LayoutKind::Bits64 => "64bit",
        }
    }
}

/// A fact about the file that does not stop the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanWarning {
    pub code: String,
    pub message: String,
    /// Name of the record, when the warning is about one.
    pub entry: Option<String>,
}

/// Where a record lives, from phase 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeta {
    pub name: String,
    /// Raw dates as stored (0.1 ms since 0001-01-01).
    pub created: u64,
    pub modified: u64,
    pub stored_size: u64,
    /// `(absolute file offset, length)` of the data bytes of each block of the record, in order.
    pub chunks: Vec<(u64, u64)>,
}

/// Result of phase 1.
#[derive(Debug, Clone)]
pub struct ScannedContainer {
    pub path: PathBuf,
    pub file_size: u64,
    pub layout: LayoutKind,
    /// False only for `Bits64` (not checked on a real file).
    pub layout_verified: bool,
    /// Start of the container proper: 0, or the end of the stub for `Bits64Stub`.
    pub container_offset: u64,
    /// The entry count field of the file header (only cross-checked).
    pub header_entry_count: u32,
    /// Sorted by name in byte order.
    pub entries: Vec<EntryMeta>,
    /// `HEADER_ENTRY_COUNT_MISMATCH`, `LAYOUT_UNVERIFIED`.
    pub warnings: Vec<ScanWarning>,
}

/// Geometry of one layout; `base` is the offset of the container in the file.
#[derive(Debug, Clone, Copy)]
struct Layout {
    hexw: usize,
    fhdr: u64,
    bhdr: u64,
    rec: usize,
    end: u64,
    base: u64,
}

impl Layout {
    fn bits32() -> Layout {
        Layout { hexw: 8, fhdr: 16, bhdr: 31, rec: 12, end: 0x7fff_ffff, base: 0 }
    }

    fn bits64(base: u64) -> Layout {
        Layout { hexw: 16, fhdr: 20, bhdr: 55, rec: 24, end: u64::MAX, base }
    }
}

/// Positioned reads of the input file (no shared cursor, so workers can share one handle).
pub(crate) struct Src<'a> {
    pub(crate) file: &'a File,
    pub(crate) size: u64,
}

#[cfg(unix)]
fn read_at(f: &File, buf: &mut [u8], off: u64) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(f, buf, off)
}

#[cfg(windows)]
fn read_at(f: &File, mut buf: &mut [u8], mut off: u64) -> std::io::Result<()> {
    while !buf.is_empty() {
        let n = std::os::windows::fs::FileExt::seek_read(f, buf, off)?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
        off += n as u64;
    }
    Ok(())
}

impl Src<'_> {
    pub(crate) fn read(&self, off: u64, buf: &mut [u8]) -> Result<()> {
        read_at(self.file, buf, off).map_err(|e| ExtractionError::new(crate::errors::IO_ERROR, e.to_string()).with("offset", off))
    }
}

pub(crate) fn corrupt(message: &str, offset: u64) -> ExtractionError {
    ExtractionError::new(CONTAINER_CORRUPT, format!("{message} at offset {offset}")).with("offset", offset)
}

pub(crate) fn limit_err(which: &str, message: &str) -> ExtractionError {
    ExtractionError::new(CONTAINER_LIMIT_EXCEEDED, message).with("limit", which)
}

fn trailing(offset: u64, why: &str) -> ExtractionError {
    ExtractionError::new(CONTAINER_TRAILING_DATA, format!("{why} at offset {offset}")).with("offset", offset)
}

/// Fields of a block header: `(document size, page size, next block)`.
fn parse_block_header(h: &[u8], l: &Layout) -> Option<(u64, u64, u64)> {
    let w = l.hexw;
    if h.len() as u64 != l.bhdr || &h[..2] != b"\r\n" || &h[h.len() - 2..] != b"\r\n" {
        return None;
    }
    let mut vals = [0u64; 3];
    for (i, v) in vals.iter_mut().enumerate() {
        let start = 2 + i * (w + 1);
        let field = &h[start..start + w];
        if h[start + w] != b' ' || !field.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        *v = u64::from_str_radix(std::str::from_utf8(field).ok()?, 16).ok()?;
    }
    Some((vals[0], vals[1], vals[2]))
}

/// Signature `ff ff ff 7f` plus a well-formed 32-bit first block header at offset 16 (needs the first 47 bytes).
pub(crate) fn looks_like_nested(head: &[u8]) -> bool {
    let l = Layout::bits32();
    let end = (l.fhdr + l.bhdr) as usize;
    head.len() >= end && head[..4] == [0xff, 0xff, 0xff, 0x7f] && parse_block_header(&head[l.fhdr as usize..end], &l).is_some()
}

/// A document: the chain of its blocks.
struct Chain {
    size: u64,
    chunks: Vec<(u64, u64)>,
    /// Absolute end (offset + header + page size) of the last block in the chain, maximum over the chain.
    end: u64,
}

/// Follow the blocks of the document whose first block is at `rel` (relative to the container).
fn walk(src: &Src, l: &Layout, mut rel: u64) -> Result<Chain> {
    let mut visited: HashSet<u64> = HashSet::new();
    let mut chunks = Vec::new();
    let mut size = 0u64;
    let mut left = 0u64;
    let mut end = 0u64;
    let mut first = true;
    let mut hb = [0u8; 55];
    loop {
        let abs = l.base.saturating_add(rel);
        if rel < l.fhdr || abs.saturating_add(l.bhdr) > src.size {
            return Err(corrupt("block offset out of range", abs));
        }
        if !visited.insert(abs) {
            return Err(ExtractionError::new(CONTAINER_CYCLE, format!("block chain revisits a block at offset {abs}")).with("offset", abs));
        }
        let h = &mut hb[..l.bhdr as usize];
        src.read(abs, h)?;
        let Some((doc, page, next)) = parse_block_header(h, l) else {
            return Err(corrupt("bad block header", abs));
        };
        if first {
            if doc > src.size {
                return Err(corrupt("document larger than the file", abs));
            }
            size = doc;
            left = doc;
            first = false;
        }
        let block_end = abs.saturating_add(l.bhdr).saturating_add(page);
        if block_end > src.size {
            return Err(corrupt("truncated block", abs));
        }
        end = end.max(block_end);
        let take = page.min(left);
        if left > 0 && take == 0 {
            return Err(corrupt("empty block in an unfinished document", abs));
        }
        if take > 0 {
            chunks.push((abs + l.bhdr, take));
        }
        left -= take;
        if left == 0 {
            return Ok(Chain { size, chunks, end });
        }
        if next == l.end {
            return Err(corrupt("document shorter than its declared size", abs));
        }
        rel = next;
    }
}

/// Read the bytes of a small document.
fn read_chain(src: &Src, c: &Chain) -> Result<Vec<u8>> {
    let mut out = vec![0u8; c.size as usize];
    let mut at = 0usize;
    for &(off, len) in &c.chunks {
        src.read(off, &mut out[at..at + len as usize])?;
        at += len as usize;
    }
    Ok(out)
}

fn parse_name(header: &[u8], at: u64) -> Result<String> {
    if header.len() < 22 {
        return Err(corrupt("record header too short", at));
    }
    let rest = &header[20..];
    let rest = &rest[..rest.len() / 2 * 2];
    let Some(end) = rest.chunks_exact(2).position(|c| c[0] == 0 && c[1] == 0).map(|p| p * 2) else {
        return Err(corrupt("record name is not terminated", at));
    };
    let units: Vec<u16> = rest[..end].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let name = String::from_utf16(&units).map_err(|_| {
        ExtractionError::new(crate::errors::CONTAINER_BAD_ENTRY_NAME, format!("record name is not valid UTF-16 at offset {at}"))
            .with("offset", at)
    })?;
    validate_entry_name(&name).map_err(|e| e.with("offset", at))?;
    Ok(name)
}

fn word(b: &[u8], at: usize, l: &Layout) -> u64 {
    if l.hexw == 8 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as u64
    } else {
        u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
    }
}

struct Parsed {
    header_count: u32,
    entries: Vec<EntryMeta>,
    /// Absolute end of the last block of any document.
    end: u64,
}

/// Parse the file header, the table of contents and the record headers of the container at `l.base`.
fn parse(src: &Src, l: &Layout) -> Result<Parsed> {
    if l.base + l.fhdr > src.size {
        return Err(corrupt("file shorter than the file header", l.base));
    }
    let mut fh = [0u8; 20];
    src.read(l.base, &mut fh[..l.fhdr as usize])?;
    if l.hexw == 8 {
        let first_free = i32::from_le_bytes(fh[0..4].try_into().unwrap());
        let page = i32::from_le_bytes(fh[4..8].try_into().unwrap());
        if first_free != l.end as i32 && !(0 <= first_free && (first_free as u64) < src.size) {
            return Err(corrupt("bad first free block offset", l.base));
        }
        if page <= 0 {
            return Err(corrupt("bad default block size", l.base + 4));
        }
    } else {
        if fh[0..8] != [0xff; 8] {
            return Err(corrupt("bad first free block marker", l.base));
        }
        if u32::from_le_bytes(fh[8..12].try_into().unwrap()) == 0 {
            return Err(corrupt("bad default block size", l.base + 8));
        }
    }
    let header_count = u32::from_le_bytes(fh[l.fhdr as usize - 8..l.fhdr as usize - 4].try_into().unwrap());
    let toc_chain = walk(src, l, l.fhdr)?;
    let mut end = toc_chain.end;
    if toc_chain.size % l.rec as u64 != 0 {
        return Err(corrupt(&format!("table of contents size is not a multiple of {}", l.rec), l.base + l.fhdr));
    }
    if toc_chain.size / l.rec as u64 > MAX_RECORDS {
        return Err(limit_err("maxEntries", "too many records"));
    }
    let toc = read_chain(src, &toc_chain)?;
    let count = toc.len() / l.rec;
    let w = l.rec / 3;
    let mut entries = Vec::with_capacity(count);
    let mut seen: HashSet<String> = HashSet::with_capacity(count);
    for i in 0..count {
        let header_at = word(&toc, i * l.rec, l);
        let data_at = word(&toc, i * l.rec + w, l);
        if word(&toc, i * l.rec + 2 * w, l) != l.end {
            return Err(corrupt(&format!("bad table of contents record {i}"), l.base + l.fhdr));
        }
        let hc = walk(src, l, header_at)?;
        end = end.max(hc.end);
        if hc.size > MAX_HEADER_DOC {
            return Err(limit_err("maxHeaderBytes", "record header too large"));
        }
        let header = read_chain(src, &hc)?;
        let at = l.base.saturating_add(header_at);
        let name = parse_name(&header, at)?;
        if !seen.insert(name.clone()) {
            return Err(ExtractionError::new(CONTAINER_DUPLICATE_ENTRY, format!("duplicate record name {name:?}"))
                .with("name", name));
        }
        let dc = walk(src, l, data_at)?;
        end = end.max(dc.end);
        entries.push(EntryMeta {
            name,
            created: u64::from_le_bytes(header[0..8].try_into().unwrap()),
            modified: u64::from_le_bytes(header[8..16].try_into().unwrap()),
            stored_size: dc.size,
            chunks: dc.chunks,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Parsed { header_count, entries, end })
}

/// Is there a 64-bit file header and a well-formed first block header at `at`?
fn has_container64(src: &Src, at: u64) -> Result<bool> {
    let l = Layout::bits64(at);
    if at + l.fhdr + l.bhdr > src.size {
        return Ok(false);
    }
    let mut buf = [0u8; 75];
    src.read(at, &mut buf)?;
    Ok(buf[..8] == [0xff; 8] && parse_block_header(&buf[20..75], &l).is_some())
}

/// Phase 1: detect the layout, read the table of contents and the record headers.
///
/// Errors: `CONTAINER_CORRUPT`, `CONTAINER_CYCLE`, `CONTAINER_DUPLICATE_ENTRY`, `CONTAINER_BAD_ENTRY_NAME`,
/// `CONTAINER_LIMIT_EXCEEDED`, `CONTAINER_TRAILING_DATA`, `CONTAINER_LAYOUT_UNSUPPORTED` (offset in the message).
pub fn open(path: &Path) -> Result<ScannedContainer> {
    let file = File::open(path).map_err(|e| ExtractionError::new(crate::errors::IO_ERROR, e.to_string()).with("path", path.display().to_string()))?;
    let size = file.metadata()?.len();
    if size > MAX_INPUT_BYTES {
        return Err(limit_err("maxInputBytes", "input file is too large"));
    }
    let src = Src { file: &file, size };
    if size < Layout::bits32().fhdr + Layout::bits32().bhdr {
        return Err(corrupt("file is shorter than a container header", 0));
    }
    let mut first = [0u8; 8];
    src.read(0, &mut first)?;
    let (layout, offset, parsed) = if first == [0xff; 8] {
        if !has_container64(&src, 0)? {
            return Err(ExtractionError::new(CONTAINER_LAYOUT_UNSUPPORTED, "64-bit container at offset 0 has no well-formed first block header")
                .with("offset", 0u64));
        }
        let p = parse(&src, &Layout::bits64(0))?;
        if p.end != size {
            return Err(trailing(p.end, "data after the end of the 64-bit container"));
        }
        (LayoutKind::Bits64, 0, p)
    } else {
        let p = parse(&src, &Layout::bits32())?;
        if p.end == size {
            (LayoutKind::Bits32, 0, p)
        } else if p.end > size {
            return Err(corrupt("container is cut off: its last block ends beyond the end of the file", size));
        } else {
            let at = p.end;
            if !has_container64(&src, at)? {
                return Err(trailing(at, "data after the end of the 32-bit container is not a valid 64-bit container"));
            }
            let p64 = parse(&src, &Layout::bits64(at))?;
            if p64.end != size {
                return Err(trailing(p64.end, "data after the end of the 64-bit container"));
            }
            (LayoutKind::Bits64Stub, at, p64)
        }
    };
    let mut warnings = Vec::new();
    let layout_verified = layout != LayoutKind::Bits64;
    if !layout_verified {
        warnings.push(ScanWarning {
            code: WARN_LAYOUT_UNVERIFIED.into(),
            message: "the 64-bit layout without a 32-bit stub was not checked on a real file".into(),
            entry: None,
        });
    }
    if parsed.header_count as usize != parsed.entries.len() {
        warnings.push(ScanWarning {
            code: WARN_HEADER_ENTRY_COUNT_MISMATCH.into(),
            message: format!("file header says {} records, the table of contents has {}", parsed.header_count, parsed.entries.len()),
            entry: None,
        });
    }
    Ok(ScannedContainer {
        path: path.to_path_buf(),
        file_size: size,
        layout,
        layout_verified,
        container_offset: offset,
        header_entry_count: parsed.header_count,
        entries: parsed.entries,
        warnings,
    })
}

/// Stored date (0.1 ms since 0001-01-01) as `YYYY-MM-DDTHH:MM:SS.mmm`, no zone; `None` when the year would exceed 9999.
pub fn ticks_text(ticks: u64) -> Option<String> {
    let ms = ticks / 10;
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // days from 0001-01-01 -> days from 1970-01-01, then the civil date (Hinnant's civil_from_days).
    let z = days - 719_162 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    if y > 9999 {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}", rem / 3600, rem % 3600 / 60, rem % 60, ms % 1000))
}
