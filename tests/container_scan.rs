//! Streaming container reader (`oneform::container_scan`): fixtures against the python oracle's `expected.json`,
//! synthetic containers from `tests/common/containers.rs` (both layouts, damage), the oracle on the builders.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::containers::*;
use oneform::container_scan::{
    compatibility_version, open, process_entries, ticks_text, warnings_of, Compression, EntryFacts, LayoutKind, ScannedContainer,
};
use oneform::errors::ExtractionError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FIXTURES: [&str; 6] = ["cf-32bit.cf", "cf-64bit-stub.cf", "cfe-32bit.cfe", "epf-803.epf", "epf-802.epf", "erf-803.erf"];

fn sha(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn tmp(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.cf");
    std::fs::write(&p, bytes).unwrap();
    (dir, p)
}

fn scan(bytes: &[u8]) -> Result<ScannedContainer, ExtractionError> {
    let (_d, p) = tmp(bytes);
    open(&p)
}

/// A scanned container that keeps its temporary file alive.
struct Held {
    c: ScannedContainer,
    _dir: tempfile::TempDir,
}

impl std::ops::Deref for Held {
    type Target = ScannedContainer;
    fn deref(&self) -> &ScannedContainer {
        &self.c
    }
}

fn ok(bytes: &[u8]) -> Held {
    let (dir, p) = tmp(bytes);
    let c = open(&p).unwrap_or_else(|e| panic!("open failed: {e}"));
    Held { c, _dir: dir }
}

fn err(bytes: &[u8]) -> ExtractionError {
    match scan(bytes) {
        Ok(_) => panic!("open unexpectedly succeeded"),
        Err(e) => e,
    }
}

/// Open + process with a given worker count; returns the container and the facts.
fn facts_of(path: &Path, workers: usize) -> (ScannedContainer, Vec<EntryFacts>) {
    let c = open(path).unwrap();
    let f = process_entries(&c, workers).unwrap();
    (c, f)
}

fn facts_of_bytes(bytes: &[u8]) -> (ScannedContainer, Vec<EntryFacts>) {
    let (_d, p) = tmp(bytes);
    facts_of(&p, 3)
}

fn opt<T: Into<Value>>(v: Option<T>) -> Value {
    v.map(Into::into).unwrap_or(Value::Null)
}

/// The same JSON shape as `container_oracle.py entries`.
fn entries_value(path: &Path, workers: usize) -> Value {
    let (c, facts) = facts_of(path, workers);
    let entries: Vec<Value> = facts
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "storedSize": f.stored_size,
                "storedSha256": f.stored_sha256,
                "compression": f.compression.as_str(),
                "inflatedSize": opt(f.inflated_size),
                "inflatedSha256": opt(f.inflated_sha256.clone()),
                "nested": f.nested,
                "created": f.created,
                "modified": f.modified,
                "createdText": opt(f.created_text.clone()),
                "modifiedText": opt(f.modified_text.clone()),
            })
        })
        .collect();
    json!({
        "layout": c.layout.as_str(),
        "containerOffset": c.container_offset,
        "headerEntryCount": c.header_entry_count,
        "entryCount": c.entries.len(),
        "compatibilityVersion": opt(compatibility_version(&c).unwrap()),
        "entries": entries,
    })
}

fn expected() -> Value {
    serde_json::from_slice(&std::fs::read(fixture_path("expected.json")).unwrap()).unwrap()
}

/// `container_oracle.py diff` shape, by the SHA-256 of the stored bytes.
fn diff_value(a: &[EntryFacts], b: &[EntryFacts]) -> Value {
    use std::collections::BTreeMap;
    let ma: BTreeMap<&str, &str> = a.iter().map(|f| (f.name.as_str(), f.stored_sha256.as_str())).collect();
    let mb: BTreeMap<&str, &str> = b.iter().map(|f| (f.name.as_str(), f.stored_sha256.as_str())).collect();
    let differ: Vec<&str> = ma.iter().filter(|(n, h)| mb.get(*n).is_some_and(|x| x != *h)).map(|(n, _)| *n).collect();
    let only_a: Vec<&str> = ma.keys().filter(|n| !mb.contains_key(*n)).copied().collect();
    let only_b: Vec<&str> = mb.keys().filter(|n| !ma.contains_key(*n)).copied().collect();
    let common = ma.keys().filter(|n| mb.contains_key(*n)).count();
    json!({
        "counts": {"a": ma.len(), "b": mb.len(), "same": common - differ.len(), "differ": differ.len(), "onlyA": only_a.len(), "onlyB": only_b.len()},
        "differ": differ, "onlyA": only_a, "onlyB": only_b,
    })
}

// ---------------------------------------------------------------- fixtures

#[test]
fn fixtures_match_expected_json() {
    let exp = expected();
    for name in FIXTURES {
        let got = entries_value(&fixture_path(name), 4);
        assert_eq!(got, exp[name], "fixture {name}");
    }
}

#[test]
fn fixture_layouts_and_counts() {
    let (c, f) = facts_of(&fixture_path("cf-32bit.cf"), 2);
    assert_eq!((c.layout, c.container_offset, c.entries.len(), f.len()), (LayoutKind::Bits32, 0, 212, 212));
    assert!(c.layout_verified && c.warnings.is_empty());
    let (c, _) = facts_of(&fixture_path("cf-64bit-stub.cf"), 2);
    assert_eq!((c.layout, c.container_offset, c.entries.len()), (LayoutKind::Bits64Stub, 4953, 63));
    assert!(c.layout_verified && c.warnings.is_empty());
    for n in ["cfe-32bit.cfe", "epf-803.epf", "epf-802.epf", "erf-803.erf"] {
        assert_eq!(open(&fixture_path(n)).unwrap().layout, LayoutKind::Bits32, "{n}");
    }
}

#[test]
fn fixture_pair_matches_expected_json() {
    let (_, a) = facts_of(&fixture_path("cf-32bit.cf"), 2);
    let (_, b) = facts_of(&fixture_path("cf-64bit-stub.cf"), 2);
    let got = diff_value(&a, &b);
    assert_eq!(got["counts"], json!({"a": 212, "b": 63, "same": 57, "differ": 6, "onlyA": 149, "onlyB": 0}));
    assert_eq!(got, expected()["pair:cf-32bit.cf|cf-64bit-stub.cf"]);
}

#[test]
fn workers_do_not_change_the_result() {
    for n in ["cf-32bit.cf", "cf-64bit-stub.cf"] {
        let (c, one) = facts_of(&fixture_path(n), 1);
        let eight = process_entries(&c, 8).unwrap();
        assert_eq!(one, eight, "{n}");
    }
    let recs: Vec<Rec> = (0..40).map(|i| Rec::deflated(&format!("r{i:02}"), format!("text {i} ").repeat(300 + i).as_bytes())).collect();
    let c = ok(&build32(&recs, &Opts::default().page(64)));
    assert_eq!(process_entries(&c, 1).unwrap(), process_entries(&c, 8).unwrap());
}

// ---------------------------------------------------------------- dates, version

#[test]
fn ticks_text_examples() {
    assert_eq!(ticks_text(636782437890000).as_deref(), Some("2018-11-19T17:03:09.000"));
    assert_eq!(ticks_text(638780677540000).as_deref(), Some("2025-03-20T11:42:34.000"));
    assert_eq!(ticks_text(0).as_deref(), Some("0001-01-01T00:00:00.000"));
    // 0.1 ms units: 12 345 ticks = 1234.5 ms; the sub-millisecond digit is dropped.
    assert_eq!(ticks_text(12_345).as_deref(), Some("0001-01-01T00:00:01.234"));
    assert_eq!(ticks_text(630_874_655_999_990).as_deref(), Some("2000-02-29T23:59:59.999"));
    // 3652059 days = 10000-01-01
    assert_eq!(ticks_text(3_652_059u64 * 864_000_000), None);
    assert_eq!(ticks_text(3_652_059u64 * 864_000_000 - 1).as_deref(), Some("9999-12-31T23:59:59.999"));
    assert_eq!(ticks_text(u64::MAX), None);
}

fn version_container(text: Option<&[u8]>) -> Held {
    let mut recs = vec![Rec::deflated("root", b"{2,\"x\"}")];
    if let Some(t) = text {
        recs.push(Rec::deflated("version", t));
    }
    ok(&build32(&recs, &Opts::default()))
}

#[test]
fn compatibility_version_cases() {
    // The BOM of the real record is part of the text.
    let c = version_container(Some(b"\xef\xbb\xbf{216,0,{80309,0}}"));
    assert_eq!(compatibility_version(&c).unwrap(), Some(80309));
    assert_eq!(compatibility_version(&version_container(Some(b"{217,0,{80315,1,{\"x\",1}}}"))).unwrap(), Some(80315));
    assert_eq!(compatibility_version(&version_container(Some(b"{216,0}"))).unwrap(), None);
    assert_eq!(compatibility_version(&version_container(Some(b"{216,0,{7031,0}}"))).unwrap(), None);
    assert_eq!(compatibility_version(&version_container(None)).unwrap(), None);
    // The first match wins.
    assert_eq!(compatibility_version(&version_container(Some(b"{1,{80316,2},{80319,3}}"))).unwrap(), Some(80316));
    // A `version` record that is not a DEFLATE stream.
    let bytes = build32(&[Rec::new("version", random_bytes(40, 7))], &Opts::default());
    assert_eq!(compatibility_version(&ok(&bytes)).unwrap(), None);
    // Fixtures.
    assert_eq!(compatibility_version(&open(&fixture_path("cfe-32bit.cfe")).unwrap()).unwrap(), None);
    assert_eq!(compatibility_version(&open(&fixture_path("cf-64bit-stub.cf")).unwrap()).unwrap(), Some(80319));
}

// ---------------------------------------------------------------- synthetic containers

fn text(i: usize, n: usize) -> Vec<u8> {
    (0..n).map(|k| b"abcdefghij"[(k * 7 + i) % 10] ^ (k as u8 & 3)).collect()
}

fn sample() -> Vec<Rec> {
    vec![
        Rec::deflated("root", b"{2,\"4f2c7a2e-0000-0000-0000-000000000001\"}"),
        Rec::deflated("version", b"{216,0,{80309,0}}"),
        Rec::deflated("z-last", &random_bytes(3000, 11)),
        Rec::deflated("a-first", &text(1, 700)),
        Rec::deflated("\u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}", &text(2, 100)),
    ]
}

fn check_sample(c: &ScannedContainer, f: &[EntryFacts], recs: &[Rec]) {
    assert_eq!(c.entries.len(), recs.len());
    assert_eq!(f.len(), recs.len());
    let names: Vec<&str> = f.iter().map(|x| x.name.as_str()).collect();
    let mut sorted: Vec<&str> = recs.iter().map(|r| r.name.as_str()).collect();
    sorted.sort_by_key(|n| n.as_bytes().to_vec());
    assert_eq!(names, sorted);
    for x in f {
        let r = recs.iter().find(|r| r.name == x.name).unwrap();
        assert_eq!(x.stored_size, r.stored.len() as u64, "{}", x.name);
        assert_eq!(x.stored_sha256, sha(&r.stored), "{}", x.name);
        assert_eq!((x.created, x.modified), (r.created, r.modified));
    }
}

#[test]
fn multi_block_documents_and_table_in_every_layout() {
    let recs = sample();
    let plain = random_bytes(3000, 11);
    for scatter in [false, true] {
        for (label, bytes, layout, off) in [
            ("32", build32(&recs, &opts(scatter)), LayoutKind::Bits32, 0),
            ("64", build64(&recs, &opts(scatter)), LayoutKind::Bits64, 0),
            ("stub", build64_stub(&recs, &opts(scatter)), LayoutKind::Bits64Stub, STUB_SIZE as u64),
        ] {
            let c = ok(&bytes);
            let f = process_entries(&c, 3).unwrap();
            assert_eq!((c.layout, c.container_offset), (layout, off), "{label}");
            assert!(c.entries.iter().any(|e| e.chunks.len() > 5), "{label}: multi-block record expected");
            check_sample(&c, &f, &recs);
            let z = f.iter().find(|x| x.name == "z-last").unwrap();
            assert_eq!((z.compression, z.inflated_size, z.inflated_sha256.clone()), (Compression::Deflate, Some(3000), Some(sha(&plain))));
            assert_eq!(compatibility_version(&c).unwrap(), Some(80309));
            // Header entry count matches: no warning besides "unverified" for the layout without a stub.
            assert!(c.warnings.iter().all(|w| w.code == "LAYOUT_UNVERIFIED"), "{label}");
        }
    }
}

fn opts(scatter: bool) -> Opts {
    let o = Opts::default().page(64).toc_page(48).header_page(40);
    if scatter {
        o.scatter()
    } else {
        o
    }
}

#[test]
fn table_of_contents_over_many_blocks() {
    let recs: Vec<Rec> = (0..30).map(|i| Rec::deflated(&format!("{i:08}-0000-0000-0000-000000000000.0"), &text(i, 50 + i))).collect();
    for bytes in [build32(&recs, &Opts::default().toc_page(24)), build64(&recs, &Opts::default().toc_page(48))] {
        let (c, f) = facts_of_bytes(&bytes);
        check_sample(&c, &f, &recs);
    }
}

#[test]
fn name_ends_at_the_first_nul() {
    let recs = vec![Rec::deflated("rec", b"x").garbage(&[0x41, 0, 0x42, 0, 0x99]), Rec::deflated("other", b"y").garbage(&[0xff, 0xff])];
    for bytes in [build32(&recs, &Opts::default()), build64_stub(&recs, &Opts::default())] {
        let (c, f) = facts_of_bytes(&bytes);
        let names: Vec<&str> = f.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["other", "rec"]);
        assert_eq!(c.entries.len(), 2);
    }
}

#[test]
fn empty_record_is_none_and_not_nested() {
    let (_, f) = facts_of_bytes(&build32(&[Rec::new("empty", vec![]), Rec::deflated("full", b"data")], &Opts::default()));
    let e = &f[0];
    assert_eq!(e.name, "empty");
    assert_eq!((e.stored_size, e.compression, e.inflated_size, e.nested), (0, Compression::None, None, false));
    assert_eq!(e.stored_sha256, sha(b""));
    assert!(warnings_of(&f).is_empty());
    // Same in the 64-bit layout.
    let (_, f) = facts_of_bytes(&build64_stub(&[Rec::new("empty", vec![])], &Opts::default().page(32)));
    assert_eq!(f[0].compression, Compression::None);
}

#[test]
fn random_bytes_are_invalid_with_a_warning() {
    let r = random_bytes(300, 99);
    let (_, f) = facts_of_bytes(&build32(&[Rec::new("junk", r.clone()), Rec::deflated("ok", b"fine")], &Opts::default()));
    let junk = f.iter().find(|x| x.name == "junk").unwrap();
    assert_eq!(junk.compression, Compression::Invalid);
    assert_eq!((junk.inflated_size, junk.inflated_sha256.clone(), junk.nested), (None, None, false));
    assert_eq!((junk.stored_size, junk.stored_sha256.clone()), (300, sha(&r)));
    let w = warnings_of(&f);
    assert_eq!(w.len(), 1);
    assert_eq!((w[0].code.as_str(), w[0].entry.as_deref()), ("ENTRY_INVALID_COMPRESSION", Some("junk")));
}

#[test]
fn deflate_with_bytes_after_the_stream_or_cut_is_invalid() {
    let good = deflate_raw(&text(5, 2000));
    let mut extra = good.clone();
    extra.push(0);
    let cut = good[..good.len() - 3].to_vec();
    let (_, f) = facts_of_bytes(&build32(
        &[Rec::new("extra", extra), Rec::new("cut", cut), Rec::new("good", good)],
        &Opts::default().page(32),
    ));
    let by = |n: &str| f.iter().find(|x| x.name == n).unwrap().compression;
    assert_eq!((by("extra"), by("cut"), by("good")), (Compression::Invalid, Compression::Invalid, Compression::Deflate));
}

fn inner_container() -> Vec<u8> {
    build32(&[Rec::deflated("a", b"inner"), Rec::deflated("b", b"inner two")], &Opts::default())
}

#[test]
fn nested_containers() {
    let inner = inner_container();
    let mut bad_header = vec![0xff, 0xff, 0xff, 0x7f];
    bad_header.extend_from_slice(&[b'x'; 60]);
    let recs = vec![
        Rec::new("plain-nested", inner.clone()),
        Rec::deflated("deflated-nested", &inner),
        Rec::deflated("sig-only", &[0xff, 0xff, 0xff, 0x7f]),
        Rec::deflated("sig-bad-header", &bad_header),
        Rec::new("stored-sig-bad-header", bad_header.clone()),
        Rec::deflated("not-nested", b"just text"),
    ];
    for bytes in [build32(&recs, &Opts::default()), build64_stub(&recs, &Opts::default())] {
        let (_, f) = facts_of_bytes(&bytes);
        let g = |n: &str| f.iter().find(|x| x.name == n).unwrap();
        let p = g("plain-nested");
        assert_eq!((p.compression, p.nested, p.inflated_size), (Compression::None, true, None));
        let d = g("deflated-nested");
        assert_eq!((d.compression, d.nested, d.inflated_size), (Compression::Deflate, true, Some(inner.len() as u64)));
        for n in ["sig-only", "sig-bad-header", "not-nested"] {
            assert_eq!((g(n).compression, g(n).nested), (Compression::Deflate, false), "{n}");
        }
        assert_eq!((g("stored-sig-bad-header").compression, g("stored-sig-bad-header").nested), (Compression::Invalid, false));
    }
}

#[test]
fn lowercase_hex_in_the_64_bit_layout_is_accepted() {
    let mut bytes = build64(&[Rec::deflated("a", b"x")], &Opts::default());
    // Lowercase every hexadecimal digit of the block headers (they are the only uppercase A-F in this body).
    for b in bytes.iter_mut() {
        if (b'A'..=b'F').contains(b) {
            *b = b.to_ascii_lowercase();
        }
    }
    let c = ok(&bytes);
    assert_eq!(c.entries.len(), 1);
}

// ---------------------------------------------------------------- errors

fn code(e: &ExtractionError) -> &'static str {
    e.code
}

#[test]
fn duplicate_and_bad_names() {
    let dup = [Rec::deflated("same", b"1"), Rec::deflated("same", b"2")];
    for bytes in [build32(&dup, &Opts::default()), build64_stub(&dup, &Opts::default())] {
        let e = err(&bytes);
        assert_eq!(code(&e), "CONTAINER_DUPLICATE_ENTRY");
        assert!(e.message.contains("same"));
    }
    for bad in ["a/b", "a\\b", "..", ".", "", "a:b", "tab\there", "del\u{7f}"] {
        let e = err(&build32(&[Rec::deflated(bad, b"1")], &Opts::default()));
        assert_eq!(code(&e), "CONTAINER_BAD_ENTRY_NAME", "{bad:?}");
    }
}

#[test]
fn block_chain_cycle() {
    let recs = [Rec::deflated("a", b"x"), Rec::new("b", random_bytes(500, 5)), Rec::deflated("c", b"y")];
    for bytes in [
        build32(&recs, &Opts::default().page(64).cycle_record(1)),
        build64_stub(&recs, &Opts::default().page(64).cycle_record(1)),
        build32(&recs, &Opts::default().page(1000).cycle_record(0)),
    ] {
        let e = err(&bytes);
        assert_eq!(code(&e), "CONTAINER_CYCLE");
        assert!(e.message.contains("offset"), "{}", e.message);
    }
}

#[test]
fn truncated_last_block() {
    let recs = sample();
    for cut in [1usize, 10, 100] {
        for bytes in [
            build32(&recs, &Opts::default().page(64).truncate(cut)),
            build64(&recs, &Opts::default().page(64).truncate(cut)),
            build64_stub(&recs, &Opts::default().page(64).truncate(cut)),
        ] {
            let e = err(&bytes);
            assert_eq!(code(&e), "CONTAINER_CORRUPT", "cut {cut}");
            assert!(e.message.contains("offset") && e.params.contains_key("offset"), "{}", e.message);
        }
    }
}

#[test]
fn trailing_bytes_after_a_32_bit_container() {
    let recs = sample();
    for t in [&b"x"[..], &[0u8; 100][..], &[0xff; 7][..]] {
        let bytes = build32(&recs, &Opts::default().trailing(t));
        let e = err(&bytes);
        assert_eq!(code(&e), "CONTAINER_TRAILING_DATA");
        let end = build32(&recs, &Opts::default()).len();
        assert!(e.message.contains(&format!("offset {end}")), "{}", e.message);
        assert_eq!(e.params["offset"], json!(end));
    }
}

#[test]
fn a_32_bit_container_followed_by_a_corrupted_64_bit_header() {
    let recs = sample();
    let good64 = build64(&recs, &Opts::default());
    // Control: the unmodified 64-bit container after a 32-bit one is read as the stub layout.
    let first = build32(&[Rec::deflated("root", b"x")], &Opts::default());
    let mut ok_bytes = first.clone();
    ok_bytes.extend_from_slice(&good64);
    let c = ok(&ok_bytes);
    assert_eq!((c.layout, c.container_offset, c.entries.len()), (LayoutKind::Bits64Stub, first.len() as u64, recs.len()));
    // Damage: the marker, the block header CRLF, a hex digit, a space.
    for (at, v) in [(0usize, 0xfeu8), (20, b'x'), (22, b'g'), (30, b'#'), (20 + 2 + 16, b'_')] {
        let mut bytes = first.clone();
        let mut b64 = good64.clone();
        b64[at] = v;
        bytes.extend_from_slice(&b64);
        let e = err(&bytes);
        assert_eq!(code(&e), "CONTAINER_TRAILING_DATA", "damage at {at}");
        assert_eq!(e.params["offset"], json!(first.len()));
    }
    // Only eight 0xff bytes, nothing more.
    let mut bytes = first.clone();
    bytes.extend_from_slice(&[0xff; 8]);
    assert_eq!(code(&err(&bytes)), "CONTAINER_TRAILING_DATA");
}

#[test]
fn stub_whose_64_bit_body_does_not_end_at_the_end_of_the_file() {
    let recs = sample();
    let e = err(&build64_stub(&recs, &Opts::default().trailing(b"junk")));
    assert_eq!(code(&e), "CONTAINER_TRAILING_DATA");
    assert!(e.message.contains("offset"));
    let e = err(&build64(&recs, &Opts::default().trailing(b"junk")));
    assert_eq!(code(&e), "CONTAINER_TRAILING_DATA");
}

#[test]
fn bad_table_of_contents_and_not_containers() {
    // Not a container at all, and too short.
    assert_eq!(code(&err(b"")), "CONTAINER_CORRUPT");
    assert_eq!(code(&err(b"short")), "CONTAINER_CORRUPT");
    assert_eq!(code(&err(&random_bytes(2000, 3))), "CONTAINER_CORRUPT");
    assert_eq!(code(&err(&[b' '; 500])), "CONTAINER_CORRUPT");
    // Starts like a 64-bit container but the first block header is not well-formed.
    let mut junk = vec![0xff; 8];
    junk.extend_from_slice(&[0x41; 200]);
    assert_eq!(code(&err(&junk)), "CONTAINER_LAYOUT_UNSUPPORTED");
    // A broken table-of-contents marker.
    let mut bytes = build32(&sample(), &Opts::default());
    let toc_marker_at = 16 + 31 + 8;
    bytes[toc_marker_at] ^= 1;
    assert_eq!(code(&err(&bytes)), "CONTAINER_CORRUPT");
}

#[test]
fn layout_without_a_stub_is_unverified() {
    let recs = sample();
    let c = ok(&build64(&recs, &Opts::default().page(128)));
    assert_eq!((c.layout, c.layout_verified, c.container_offset), (LayoutKind::Bits64, false, 0));
    assert_eq!(c.warnings.len(), 1);
    assert_eq!(c.warnings[0].code, "LAYOUT_UNVERIFIED");
    let c = ok(&build64_stub(&recs, &Opts::default()));
    assert!(c.layout_verified);
}

#[test]
fn header_count_mismatch_is_a_warning() {
    let recs = sample();
    for bytes in [build32(&recs, &Opts::default().header_count(9)), build64_stub(&recs, &Opts::default().header_count(0))] {
        let c = ok(&bytes);
        assert_eq!(c.entries.len(), 5);
        let w: Vec<&str> = c.warnings.iter().map(|w| w.code.as_str()).collect();
        assert_eq!(w, ["HEADER_ENTRY_COUNT_MISMATCH"]);
        assert!(c.warnings[0].message.contains('9') || c.warnings[0].message.contains('0'));
        assert_ne!(c.header_entry_count as usize, c.entries.len());
    }
    assert!(ok(&build32(&recs, &Opts::default())).warnings.is_empty());
}

#[test]
fn zero_records() {
    let c = ok(&build32(&[], &Opts::default()));
    assert_eq!((c.entries.len(), c.layout), (0, LayoutKind::Bits32));
    assert!(process_entries(&c, 2).unwrap().is_empty());
}

// ---------------------------------------------------------------- the oracle on the builders and on real files

/// Run the python oracle; `None` when python is not installed.
fn oracle(args: &[&str]) -> Option<Value> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/container_oracle.py");
    for py in ["python", "python3"] {
        if let Ok(out) = Command::new(py).arg(&script).args(args).output() {
            assert!(out.status.success(), "oracle failed: {}", String::from_utf8_lossy(&out.stderr));
            return Some(serde_json::from_slice(&out.stdout).unwrap());
        }
    }
    None
}

#[test]
fn builders_are_read_the_same_by_the_oracle() {
    let mut recs = sample();
    recs.push(Rec::new("empty", vec![]));
    recs.push(Rec::new("junk", random_bytes(200, 99)));
    recs.push(Rec::new("plain-nested", inner_container()));
    recs.push(Rec::deflated("deflated-nested", &inner_container()));
    recs.push(Rec::deflated("sig-only", &[0xff, 0xff, 0xff, 0x7f]));
    recs.push(Rec::deflated("tail", b"x").garbage(&[1, 2, 3, 4, 5]).dates(1, u64::MAX));
    let cases = [
        ("b32", build32(&recs, &Opts::default())),
        ("b32-small", build32(&recs, &opts(false))),
        ("b32-scatter", build32(&recs, &opts(true))),
        ("b64", build64(&recs, &opts(false))),
        ("b64-scatter", build64(&recs, &opts(true))),
        ("stub", build64_stub(&recs, &Opts::default())),
        ("stub-small", build64_stub(&recs, &opts(true))),
        ("mismatch", build32(&recs, &Opts::default().header_count(77))),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (label, bytes) in cases {
        let p = dir.path().join(format!("{label}.cf"));
        std::fs::write(&p, &bytes).unwrap();
        let Some(want) = oracle(&["entries", p.to_str().unwrap()]) else { return };
        assert_eq!(entries_value(&p, 4), want, "{label}");
    }
}

#[test]
fn oracle_rejects_what_the_reader_rejects() {
    let recs = sample();
    let dir = tempfile::tempdir().unwrap();
    let bad = [
        build32(&recs, &Opts::default().trailing(b"x")),
        build32(&recs, &Opts::default().truncate(5)),
        build32(&recs, &Opts::default().page(64).cycle_record(1)),
        build64_stub(&recs, &Opts::default().trailing(b"x")),
    ];
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/container_oracle.py");
    for (i, bytes) in bad.iter().enumerate() {
        let p = dir.path().join(format!("bad{i}.cf"));
        std::fs::write(&p, bytes).unwrap();
        assert!(open(&p).is_err(), "{i}");
        for py in ["python", "python3"] {
            if let Ok(out) = Command::new(py).arg(&script).arg("entries").arg(&p).output() {
                assert_eq!(out.status.code(), Some(1), "oracle must refuse case {i}");
                break;
            }
        }
    }
}

#[test]
#[ignore = "needs ONEFORM_REAL_CF_A (and optionally ONEFORM_REAL_CF_B)"]
fn real_files_match_oracle() {
    let mut paths = Vec::new();
    for var in ["ONEFORM_REAL_CF_A", "ONEFORM_REAL_CF_B"] {
        if let Ok(p) = std::env::var(var) {
            paths.push(PathBuf::from(p));
        }
    }
    assert!(!paths.is_empty(), "set ONEFORM_REAL_CF_A");
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    for p in &paths {
        let t = std::time::Instant::now();
        let c = open(p).unwrap();
        eprintln!("{}: open (phase 1) {:?}", p.display(), t.elapsed());
        drop(c);
        let t = std::time::Instant::now();
        let got = entries_value(p, workers);
        let took = t.elapsed();
        let Some(want) = oracle(&["entries", p.to_str().unwrap()]) else { return };
        assert_eq!(got["layout"], want["layout"]);
        assert_eq!(got["entryCount"], want["entryCount"]);
        assert_eq!(got, want, "{}", p.display());
        eprintln!("{}: {} records, {:?}", p.display(), got["entryCount"], took);
    }
    if let [a, b] = paths.as_slice() {
        let (_, fa) = facts_of(a, 4);
        let (_, fb) = facts_of(b, 4);
        let want = oracle(&["diff", a.to_str().unwrap(), b.to_str().unwrap()]).unwrap();
        assert_eq!(diff_value(&fa, &fb), want);
    }
}
