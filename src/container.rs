//! Bounded, pure in-memory reader/writer for 1C 8.x 32-bit containers (port of `adapters/container.py`).

use std::collections::{HashMap, HashSet};

use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use serde_json::Value;

use crate::errors::{
    ExtractionError, Result, CONTAINER_BAD_ENTRY_NAME, CONTAINER_CORRUPT, CONTAINER_CYCLE,
    CONTAINER_DUPLICATE_ENTRY, CONTAINER_LAYOUT_UNSUPPORTED, CONTAINER_LIMIT_EXCEEDED,
};

pub const END_MARKER: i32 = 0x7FFF_FFFF;
pub const FILE_HEADER_SIZE: usize = 16;
pub const BLOCK_HEADER_SIZE: usize = 31;
pub const TOC_RECORD_SIZE: usize = 12;
pub const ENTRY_HEADER_FIXED: usize = 20;
pub const CONTAINER_MAGIC: [u8; 4] = [0xff, 0xff, 0xff, 0x7f];
const MIB: f64 = 1024.0 * 1024.0;

/// Resource limits; defaults match the root `extraction.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    pub max_input_mib: f64,
    pub max_expanded_mib: f64,
    pub max_entry_mib: f64,
    pub max_entries: i64,
    pub max_depth: i64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_input_mib: 256.0, max_expanded_mib: 1024.0, max_entry_mib: 64.0, max_entries: 100_000, max_depth: 128 }
    }
}

impl Limits {
    /// `Limits.from_json`: missing keys fall back to the defaults.
    pub fn from_json(d: &Value) -> Limits {
        let def = Limits::default();
        let f = |k: &str, dv: f64| d.get(k).and_then(Value::as_f64).unwrap_or(dv);
        let i = |k: &str, dv: i64| d.get(k).and_then(Value::as_i64).unwrap_or(dv);
        Limits {
            max_input_mib: f("maxInputMiB", def.max_input_mib),
            max_expanded_mib: f("maxExpandedMiB", def.max_expanded_mib),
            max_entry_mib: f("maxEntryMiB", def.max_entry_mib),
            max_entries: i("maxEntries", def.max_entries),
            max_depth: i("maxDepth", def.max_depth),
        }
    }

    pub fn max_input(&self) -> i64 {
        (self.max_input_mib * MIB) as i64
    }

    pub fn max_expanded(&self) -> i64 {
        (self.max_expanded_mib * MIB) as i64
    }

    pub fn max_entry(&self) -> i64 {
        (self.max_entry_mib * MIB) as i64
    }
}

fn limit_err(which: &str, message: &str) -> ExtractionError {
    ExtractionError::new(CONTAINER_LIMIT_EXCEEDED, message).with("limit", which)
}

fn corrupt(message: &str) -> ExtractionError {
    ExtractionError::new(CONTAINER_CORRUPT, message)
}

/// Expansion/entry budget shared across nested parses and inflations.
#[derive(Debug, Clone)]
pub struct Budget {
    pub limits: Limits,
    pub expanded: i64,
    pub entries: i64,
}

impl Budget {
    pub fn new(limits: &Limits) -> Budget {
        Budget { limits: limits.clone(), expanded: 0, entries: 0 }
    }

    pub fn remaining(&self) -> i64 {
        self.limits.max_expanded() - self.expanded
    }

    pub fn charge(&mut self, nbytes: i64) -> Result<()> {
        if nbytes > self.remaining() {
            return Err(limit_err("maxExpandedMiB", "expanded data exceeds limit"));
        }
        self.expanded += nbytes;
        Ok(())
    }

    pub fn charge_entry(&mut self) -> Result<()> {
        self.entries += 1;
        if self.entries > self.limits.max_entries {
            return Err(limit_err("maxEntries", "too many entries"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub header_offset: usize,
    pub data_offset: usize,
    pub raw: Vec<u8>,
    pub header: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Container {
    pub entries: Vec<Entry>,
    pub depth: i64,
    pub budget: Budget,
    by_name: HashMap<String, usize>,
}

impl Container {
    pub fn new(entries: Vec<Entry>, depth: i64, budget: Budget) -> Container {
        let by_name = entries.iter().enumerate().map(|(i, e)| (e.name.clone(), i)).collect();
        Container { entries, depth, budget, by_name }
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.by_name.get(name).map(|&i| &self.entries[i])
    }

    pub fn contains(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }
}

pub fn looks_like_container(data: &[u8]) -> bool {
    data.len() >= 4 && data[..4] == CONTAINER_MAGIC
}

fn hex8(f: &[u8]) -> Option<i64> {
    if f.len() != 8 || !f.iter().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    i64::from_str_radix(std::str::from_utf8(f).ok()?, 16).ok()
}

/// Read a document (chain of blocks) starting at `offset`.
fn read_document(data: &[u8], mut offset: i64, limits: &Limits, budget: &mut Budget) -> Result<Vec<u8>> {
    let total = data.len() as i64;
    let mut visited: HashSet<i64> = HashSet::new();
    let mut out: Vec<u8> = Vec::new();
    let mut left: Option<i64> = None;
    loop {
        if offset < FILE_HEADER_SIZE as i64 || offset + BLOCK_HEADER_SIZE as i64 > total {
            return Err(corrupt("block offset out of range").with("offset", offset));
        }
        if !visited.insert(offset) {
            return Err(ExtractionError::new(CONTAINER_CYCLE, "block chain revisits a block").with("offset", offset));
        }
        let o = offset as usize;
        let h = &data[o..o + BLOCK_HEADER_SIZE];
        if &h[0..2] != b"\r\n" || h[10] != b' ' || h[19] != b' ' || h[28] != b' ' || &h[29..31] != b"\r\n" {
            return Err(corrupt("bad block header").with("offset", offset));
        }
        let (Some(doc_size), Some(block_size), Some(nxt)) = (hex8(&h[2..10]), hex8(&h[11..19]), hex8(&h[20..28])) else {
            return Err(corrupt("bad hex in block header").with("offset", offset));
        };
        let remaining = match left {
            None => {
                if doc_size > limits.max_entry() {
                    return Err(limit_err("maxEntryMiB", "document exceeds entry limit"));
                }
                budget.charge(doc_size)?;
                doc_size
            }
            Some(l) => l,
        };
        let take = block_size.min(remaining);
        let start = o + BLOCK_HEADER_SIZE;
        if start as i64 + take > total {
            return Err(corrupt("truncated block data").with("offset", offset));
        }
        if remaining > 0 && take == 0 {
            return Err(corrupt("empty block in unfinished document").with("offset", offset));
        }
        out.extend_from_slice(&data[start..start + take as usize]);
        let remaining = remaining - take;
        left = Some(remaining);
        if remaining == 0 {
            return Ok(out);
        }
        if nxt == END_MARKER as i64 {
            return Err(corrupt("document shorter than declared size").with("offset", offset));
        }
        offset = nxt;
    }
}

pub fn validate_entry_name(name: &str) -> Result<()> {
    let bad = |c: char| matches!(c, '/' | '\\' | ':' | '\u{7f}') || (c as u32) < 0x20;
    if name.is_empty() || name == "." || name == ".." || name.chars().any(bad) {
        return Err(ExtractionError::new(CONTAINER_BAD_ENTRY_NAME, "invalid entry name").with("name", name));
    }
    Ok(())
}

fn parse_entry_name(header: &[u8], header_offset: usize) -> Result<String> {
    if header.len() < ENTRY_HEADER_FIXED + 2 {
        return Err(corrupt("entry header too short").with("offset", header_offset as u64));
    }
    let rest = &header[ENTRY_HEADER_FIXED..];
    let rest = &rest[..rest.len() / 2 * 2];
    let Some(end) = rest.chunks_exact(2).position(|c| c[0] == 0 && c[1] == 0).map(|p| p * 2) else {
        return Err(corrupt("entry name is not NUL-terminated").with("offset", header_offset as u64));
    };
    let units: Vec<u16> = rest[..end].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let name = String::from_utf16(&units).map_err(|_| {
        ExtractionError::new(CONTAINER_BAD_ENTRY_NAME, "entry name is not valid UTF-16").with("offset", header_offset as u64)
    })?;
    validate_entry_name(&name)?;
    Ok(name)
}

fn le_i32(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Parse the table of contents of a 32-bit container; entry data stays raw.
pub fn parse_container(data: &[u8], limits: &Limits, depth: i64, budget: Option<Budget>) -> Result<Container> {
    let mut budget = budget.unwrap_or_else(|| Budget::new(limits));
    if depth > limits.max_depth {
        return Err(limit_err("maxDepth", "container nesting too deep"));
    }
    if data.len() as i64 > limits.max_input() {
        return Err(limit_err("maxInputMiB", "container exceeds input limit"));
    }
    if data.len() < FILE_HEADER_SIZE {
        return Err(corrupt("container shorter than file header"));
    }
    if data[..8] == [0xff; 8] {
        return Err(ExtractionError::new(CONTAINER_LAYOUT_UNSUPPORTED, "64-bit container layout is not supported"));
    }
    let first_empty = le_i32(data, 0);
    let block_size = le_i32(data, 4);
    if first_empty != END_MARKER && !(0 <= first_empty && (first_empty as usize) < data.len()) {
        return Err(corrupt("bad first free block offset").with("value", first_empty));
    }
    if block_size <= 0 {
        return Err(corrupt("bad default block size").with("value", block_size));
    }

    let toc = read_document(data, FILE_HEADER_SIZE as i64, limits, &mut budget)?;
    if toc.len() % TOC_RECORD_SIZE != 0 {
        return Err(corrupt("table of contents size is not a multiple of 12"));
    }
    let count = toc.len() / TOC_RECORD_SIZE;
    if count as i64 > limits.max_entries {
        return Err(limit_err("maxEntries", "too many entries"));
    }

    let mut entries: Vec<Entry> = Vec::with_capacity(count);
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..count {
        let hdr_off = le_i32(&toc, i * TOC_RECORD_SIZE);
        let data_off = le_i32(&toc, i * TOC_RECORD_SIZE + 4);
        let marker = le_i32(&toc, i * TOC_RECORD_SIZE + 8);
        if marker != END_MARKER {
            return Err(corrupt("bad table of contents record").with("index", i as u64));
        }
        budget.charge_entry()?;
        let header = read_document(data, hdr_off as i64, limits, &mut budget)?;
        let name = parse_entry_name(&header, hdr_off as usize)?;
        if !seen.insert(name.clone()) {
            return Err(ExtractionError::new(CONTAINER_DUPLICATE_ENTRY, "duplicate entry name").with("name", name));
        }
        let raw = read_document(data, data_off as i64, limits, &mut budget)?;
        entries.push(Entry { name, header_offset: hdr_off as usize, data_offset: data_off as usize, raw, header });
    }
    Ok(Container::new(entries, depth, budget))
}

/// Raw-DEFLATE decompress; limits are enforced before output is kept.
pub fn inflate(raw: &[u8], limits: &Limits, budget: Option<&mut Budget>) -> Result<Vec<u8>> {
    let mut own;
    let budget = match budget {
        Some(b) => b,
        None => {
            own = Budget::new(limits);
            &mut own
        }
    };
    let mut d = Decompress::new(false);
    let mut out: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let entry_left = limits.max_entry() - out.len() as i64;
        let allowed = entry_left.min(budget.remaining());
        let want = ((allowed + 1).max(0) as usize).min(chunk.len());
        let in_before = d.total_in();
        let out_before = d.total_out();
        let status = d
            .decompress(&raw[in_before as usize..], &mut chunk[..want], FlushDecompress::None)
            .map_err(|e| corrupt("invalid deflate stream").with("detail", e.to_string()))?;
        let produced = (d.total_out() - out_before) as usize;
        if produced as i64 > allowed {
            let which = if entry_left <= budget.remaining() { "maxEntryMiB" } else { "maxExpandedMiB" };
            return Err(limit_err(which, "inflated data exceeds limit"));
        }
        budget.charge(produced as i64)?;
        out.extend_from_slice(&chunk[..produced]);
        match status {
            Status::StreamEnd => break,
            _ => {
                if produced == 0 && d.total_in() == in_before {
                    return Err(corrupt("truncated deflate stream"));
                }
            }
        }
    }
    let used = d.total_in() as usize;
    if used < raw.len() {
        return Err(corrupt("trailing data after deflate stream").with("extra", (raw.len() - used) as u64));
    }
    Ok(out)
}

/// Raw-DEFLATE compress (the entry payload format of EPF containers).
pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut c = Compress::new(Compression::default(), false);
    let mut out: Vec<u8> = Vec::with_capacity(data.len() / 2 + 64);
    loop {
        let in_before = c.total_in() as usize;
        if out.capacity() - out.len() < 1024 {
            out.reserve(data.len() / 4 + 1024);
        }
        let status = c.compress_vec(&data[in_before..], &mut out, FlushCompress::Finish).expect("deflate");
        if status == Status::StreamEnd {
            break;
        }
    }
    out
}

fn block(doc_size: usize, block_size: usize, data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(BLOCK_HEADER_SIZE + block_size.max(data.len()));
    v.extend_from_slice(b"\r\n");
    v.extend_from_slice(format!("{doc_size:08x} {block_size:08x} {:08x} ", END_MARKER).as_bytes());
    v.extend_from_slice(b"\r\n");
    v.extend_from_slice(data);
    if data.len() < block_size {
        v.resize(BLOCK_HEADER_SIZE + block_size, 0);
    }
    v
}

/// One entry to serialize: `(name, raw_data, raw_header_or_None)`.
pub type EntryOut<'a> = (&'a str, &'a [u8], Option<&'a [u8]>);

/// Build a 32-bit container from `(name, raw_data, raw_header_or_None)` tuples.
pub fn serialize_container(entries: &[EntryOut<'_>], default_block_size: i32) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    for v in [END_MARKER, default_block_size, entries.len() as i32, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let toc_size = TOC_RECORD_SIZE * entries.len();
    let toc_at = out.len();
    out.extend_from_slice(&block(toc_size, (default_block_size.max(0) as usize).max(toc_size), &vec![0u8; toc_size]));
    let mut toc: Vec<u8> = Vec::with_capacity(toc_size);
    for (name, raw_data, raw_header) in entries {
        validate_entry_name(name)?;
        let hdr: Vec<u8> = match raw_header {
            Some(h) if !h.is_empty() => h.to_vec(),
            _ => {
                let mut h = vec![0u8; 20];
                for u in name.encode_utf16() {
                    h.extend_from_slice(&u.to_le_bytes());
                }
                h.extend_from_slice(&[0, 0, 0, 0]);
                h
            }
        };
        let h_off = out.len() as i32;
        out.extend_from_slice(&block(hdr.len(), hdr.len(), &hdr));
        let d_off = out.len() as i32;
        let size = if default_block_size > 0 { (default_block_size as usize).max(raw_data.len()) } else { raw_data.len().max(1) };
        out.extend_from_slice(&block(raw_data.len(), size, raw_data));
        for v in [h_off, d_off, END_MARKER] {
            toc.extend_from_slice(&v.to_le_bytes());
        }
    }
    let start = toc_at + BLOCK_HEADER_SIZE;
    out[start..start + toc_size].copy_from_slice(&toc);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        serialize_container(&[("a", b"hello", None), ("b.c", b"", None)], 0x200).unwrap()
    }

    #[test]
    fn roundtrip() {
        let data = sample();
        assert!(looks_like_container(&data));
        let c = parse_container(&data, &Limits::default(), 0, None).unwrap();
        assert_eq!(c.names(), vec!["a", "b.c"]);
        assert_eq!(c.get("a").unwrap().raw, b"hello");
        let again: Vec<EntryOut> = c.entries.iter().map(|e| (e.name.as_str(), e.raw.as_slice(), Some(e.header.as_slice()))).collect();
        assert_eq!(serialize_container(&again, 0x200).unwrap(), data);
    }

    #[test]
    fn deflate_inflate() {
        let src: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let z = deflate(&src);
        assert_eq!(inflate(&z, &Limits::default(), None).unwrap(), src);
        let small = Limits { max_entry_mib: 0.01, ..Limits::default() };
        assert_eq!(inflate(&z, &small, None).unwrap_err().code, CONTAINER_LIMIT_EXCEEDED);
        let mut bad = z.clone();
        bad.push(0);
        assert_eq!(inflate(&bad, &Limits::default(), None).unwrap_err().code, CONTAINER_CORRUPT);
        assert_eq!(inflate(&z[..z.len() / 2], &Limits::default(), None).unwrap_err().code, CONTAINER_CORRUPT);
    }

    #[test]
    fn corrupt_inputs() {
        let l = Limits::default();
        assert_eq!(parse_container(b"short", &l, 0, None).unwrap_err().code, CONTAINER_CORRUPT);
        assert_eq!(parse_container(&[0xff; 32], &l, 0, None).unwrap_err().code, CONTAINER_LAYOUT_UNSUPPORTED);
        let mut data = sample();
        data[FILE_HEADER_SIZE] = b'X';
        assert_eq!(parse_container(&data, &l, 0, None).unwrap_err().code, CONTAINER_CORRUPT);
        let dup = serialize_container(&[("a", b"1", None), ("a", b"2", None)], 0x200).unwrap();
        assert_eq!(parse_container(&dup, &l, 0, None).unwrap_err().code, CONTAINER_DUPLICATE_ENTRY);
        assert!(validate_entry_name("a/b").is_err());
        assert!(validate_entry_name("..").is_err());
    }
}
