//! Builders of synthetic 1C containers for the container reader tests (`tests/container_scan.rs`).
//!
//! A container is built from a list of records in the 32-bit layout or in the 64-bit layout (with or without the
//! real 32-bit stub). `Opts` selects the page sizes, the placement of the blocks and the damage to inject.
//! The python oracle (`scripts/container_oracle.py`) reads what these builders write.
#![allow(dead_code)]

use std::io::Write;
use std::path::PathBuf;

use flate2::write::DeflateEncoder;
use flate2::Compression;

/// Size of the 32-bit stub at the start of `cf-64bit-stub.cf`.
pub const STUB_SIZE: usize = 4953;

/// One top-level record: name, stored bytes (already compressed, or not), the dates, bytes after the name's NUL.
#[derive(Clone, Debug)]
pub struct Rec {
    pub name: String,
    pub stored: Vec<u8>,
    pub created: u64,
    pub modified: u64,
    pub garbage: Vec<u8>,
}

impl Rec {
    /// A record with the given stored bytes and fixed default dates.
    pub fn new(name: &str, stored: Vec<u8>) -> Rec {
        Rec { name: name.to_string(), stored, created: 636782437890000, modified: 638780677540000, garbage: Vec::new() }
    }

    /// Raw-DEFLATE compressed content.
    pub fn deflated(name: &str, plain: &[u8]) -> Rec {
        Rec::new(name, deflate_raw(plain))
    }

    pub fn dates(mut self, created: u64, modified: u64) -> Rec {
        self.created = created;
        self.modified = modified;
        self
    }

    /// Bytes stored after the terminating NUL of the name in the record header.
    pub fn garbage(mut self, bytes: &[u8]) -> Rec {
        self.garbage = bytes.to_vec();
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bits {
    B32,
    B64,
}

/// Build options. `Opts::default()` is a plain well-formed container.
#[derive(Clone, Debug)]
pub struct Opts {
    /// Page size of the blocks of record data documents (a larger document gets several blocks).
    pub page: usize,
    /// Page size of the blocks of the table of contents (a small one gives a multi-block table).
    pub toc_page: usize,
    /// Page size of the blocks of record header documents; `None` = the header size (one block).
    pub header_page: Option<usize>,
    /// Place all blocks except those of the table of contents in reverse order (chains jump backwards).
    pub scatter: bool,
    /// Value of the entry count field of the file header; `None` = the number of records.
    pub header_count: Option<u32>,
    /// The data document of this record gets a block chain that revisits its first block.
    pub cycle_record: Option<usize>,
    /// Bytes appended after the container (after the truncation, if any).
    pub trailing: Vec<u8>,
    /// Number of bytes cut off the end of the container.
    pub truncate: usize,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts {
            page: 0x200,
            toc_page: 0x200,
            header_page: None,
            scatter: false,
            header_count: None,
            cycle_record: None,
            trailing: Vec::new(),
            truncate: 0,
        }
    }
}

impl Opts {
    pub fn page(mut self, page: usize) -> Opts {
        self.page = page;
        self
    }
    pub fn toc_page(mut self, page: usize) -> Opts {
        self.toc_page = page;
        self
    }
    pub fn header_page(mut self, page: usize) -> Opts {
        self.header_page = Some(page);
        self
    }
    pub fn scatter(mut self) -> Opts {
        self.scatter = true;
        self
    }
    pub fn header_count(mut self, n: u32) -> Opts {
        self.header_count = Some(n);
        self
    }
    pub fn cycle_record(mut self, i: usize) -> Opts {
        self.cycle_record = Some(i);
        self
    }
    pub fn trailing(mut self, bytes: &[u8]) -> Opts {
        self.trailing = bytes.to_vec();
        self
    }
    pub fn truncate(mut self, n: usize) -> Opts {
        self.truncate = n;
        self
    }
}

/// Raw DEFLATE (no zlib header), as stored in the records of a `.cf`.
pub fn deflate_raw(data: &[u8]) -> Vec<u8> {
    let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// Deterministic pseudo-random bytes (xorshift); with the seeds used in the tests they do not inflate.
pub fn random_bytes(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.max(1);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/containers").join(name)
}

/// The 32-bit stub of the 64-bit sample: the first 4953 bytes of `cf-64bit-stub.cf`.
pub fn stub_bytes() -> Vec<u8> {
    let mut v = std::fs::read(fixture_path("cf-64bit-stub.cf")).unwrap();
    v.truncate(STUB_SIZE);
    v
}

/// 32-bit container (+ `opts.trailing`, - `opts.truncate`).
pub fn build32(recs: &[Rec], o: &Opts) -> Vec<u8> {
    finish(build_body(Bits::B32, recs, o), o)
}

/// 64-bit container without a stub.
pub fn build64(recs: &[Rec], o: &Opts) -> Vec<u8> {
    finish(build_body(Bits::B64, recs, o), o)
}

/// Real 32-bit stub followed by a 64-bit container (addresses of the body count from its own start).
pub fn build64_stub(recs: &[Rec], o: &Opts) -> Vec<u8> {
    let mut v = stub_bytes();
    v.extend_from_slice(&build_body(Bits::B64, recs, o));
    finish(v, o)
}

fn finish(mut v: Vec<u8>, o: &Opts) -> Vec<u8> {
    v.truncate(v.len() - o.truncate);
    v.extend_from_slice(&o.trailing);
    v
}

struct Geo {
    hexw: usize,
    fhdr: usize,
    bhdr: usize,
    rec: usize,
    end: u64,
}

fn geo(bits: Bits) -> Geo {
    match bits {
        Bits::B32 => Geo { hexw: 8, fhdr: 16, bhdr: 31, rec: 12, end: 0x7fff_ffff },
        Bits::B64 => Geo { hexw: 16, fhdr: 20, bhdr: 55, rec: 24, end: u64::MAX },
    }
}

struct Blk {
    doc: usize,
    /// Offset of the first data byte of this block inside the document.
    start: usize,
    len: usize,
    off: usize,
}

fn header_doc(r: &Rec) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(&r.created.to_le_bytes());
    h.extend_from_slice(&r.modified.to_le_bytes());
    h.extend_from_slice(&[0, 0, 0, 0]);
    for u in r.name.encode_utf16() {
        h.extend_from_slice(&u.to_le_bytes());
    }
    h.extend_from_slice(&[0, 0]);
    h.extend_from_slice(&r.garbage);
    h
}

fn word(g: &Geo, v: u64, out: &mut Vec<u8>) {
    if g.hexw == 8 {
        out.extend_from_slice(&(v as u32).to_le_bytes());
    } else {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn hex(g: &Geo, v: u64) -> String {
    if g.hexw == 8 {
        format!("{v:08x}")
    } else {
        format!("{v:016X}")
    }
}

/// The container proper; addresses count from its first byte.
fn build_body(bits: Bits, recs: &[Rec], o: &Opts) -> Vec<u8> {
    let g = geo(bits);
    assert!(o.page > 0 && o.toc_page > 0);
    // Documents: 0 = table of contents, then (header, data) of every record.
    let mut sizes: Vec<(usize, usize)> = vec![(recs.len() * g.rec, o.toc_page)];
    let mut contents: Vec<Vec<u8>> = vec![Vec::new()];
    for r in recs {
        let h = header_doc(r);
        sizes.push((h.len(), o.header_page.unwrap_or(h.len().max(1))));
        contents.push(h);
        sizes.push((r.stored.len(), o.page));
        contents.push(r.stored.clone());
    }
    // Blocks of every document.
    let mut blks: Vec<Blk> = Vec::new();
    let mut chains: Vec<Vec<usize>> = Vec::new();
    for (d, &(size, page)) in sizes.iter().enumerate() {
        let mut chain = Vec::new();
        let mut start = 0;
        loop {
            let len = page.min(size - start);
            chain.push(blks.len());
            blks.push(Blk { doc: d, start, len, off: 0 });
            start += len;
            if start >= size {
                break;
            }
        }
        chains.push(chain);
    }
    // Placement: the table of contents first (the reader looks for it at the file header size).
    let toc_blocks = chains[0].len();
    let mut order: Vec<usize> = (0..blks.len()).collect();
    if o.scatter {
        order[toc_blocks..].reverse();
    }
    let mut pos = g.fhdr;
    for &b in &order {
        blks[b].off = pos;
        pos += g.bhdr + sizes[blks[b].doc].1;
    }
    let total = pos;
    // Table of contents.
    let mut toc = Vec::new();
    for i in 0..recs.len() {
        word(&g, blks[chains[1 + 2 * i][0]].off as u64, &mut toc);
        word(&g, blks[chains[2 + 2 * i][0]].off as u64, &mut toc);
        word(&g, g.end, &mut toc);
    }
    contents[0] = toc;
    // Output.
    let mut out = vec![0u8; total];
    if bits == Bits::B32 {
        out[0..4].copy_from_slice(&0x7fff_ffffu32.to_le_bytes());
        out[4..8].copy_from_slice(&(o.page as u32).to_le_bytes());
        out[8..12].copy_from_slice(&o.header_count.unwrap_or(recs.len() as u32).to_le_bytes());
    } else {
        out[0..8].copy_from_slice(&[0xff; 8]);
        out[8..12].copy_from_slice(&512u32.to_le_bytes());
        out[12..16].copy_from_slice(&o.header_count.unwrap_or(recs.len() as u32).to_le_bytes());
    }
    let cycle_doc = o.cycle_record.map(|i| 2 + 2 * i);
    for (d, chain) in chains.iter().enumerate() {
        let page = sizes[d].1;
        for (k, &b) in chain.iter().enumerate() {
            let blk = &blks[b];
            let cyc = cycle_doc == Some(d);
            let doc_size = if cyc { (chain.len() + 1) * page } else { sizes[d].0 };
            let next = if cyc && k + 1 == chain.len() {
                blks[chain[0]].off as u64
            } else if k + 1 < chain.len() {
                blks[chain[k + 1]].off as u64
            } else {
                g.end
            };
            let hdr = format!("\r\n{} {} {} \r\n", hex(&g, doc_size as u64), hex(&g, page as u64), hex(&g, next));
            assert_eq!(hdr.len(), g.bhdr);
            out[blk.off..blk.off + g.bhdr].copy_from_slice(hdr.as_bytes());
            let at = blk.off + g.bhdr;
            out[at..at + blk.len].copy_from_slice(&contents[d][blk.start..blk.start + blk.len]);
        }
    }
    out
}
