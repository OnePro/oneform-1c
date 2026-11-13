//! `container-entries` and `container-diff`: the command line (fixtures against `expected.json`, synthetic containers
//! from `tests/common/containers.rs`, owners from synthetic XML dumps, usage errors, read-only and environment independence).

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use common::containers::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FIXTURES: [&str; 6] = ["cf-32bit.cf", "cf-64bit-stub.cf", "cfe-32bit.cfe", "epf-803.epf", "epf-802.epf", "erf-803.erf"];

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not one JSON object ({e}): {}\nstderr: {}", self.stdout, self.stderr))
    }
}

fn run_with(cwd: Option<&Path>, envs: &[(&str, &str)], args: &[&str]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oneform"));
    cmd.args(args);
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn run(args: &[&str]) -> Run {
    run_with(None, &[], args)
}

fn fx(name: &str) -> String {
    fixture_path(name).to_string_lossy().into_owned()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn put(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let p = dir.join(name);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, bytes).unwrap();
    p.to_string_lossy().into_owned()
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn expected() -> Value {
    serde_json::from_slice(&fs::read(fixture_path("expected.json")).unwrap()).unwrap()
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap().to_string()).collect()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn sorted(mut v: Vec<&str>) -> Vec<String> {
    v.sort();
    v.into_iter().map(String::from).collect()
}

/// Raw DEFLATE made of stored blocks: the same content as `deflate_raw`, other bytes.
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::none());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

fn text(n: usize) -> Vec<u8> {
    format!("record text {n} ").repeat(80).into_bytes()
}

// ---------------------------------------------------------------- container-entries

#[test]
fn entries_on_every_fixture_match_expected_json() {
    let exp = expected();
    for name in FIXTURES {
        let path = fx(name);
        let r = run(&["container-entries", &path, "--json"]);
        assert_eq!(r.code, 0, "{name}: {}", r.stderr);
        assert!(r.stderr.is_empty(), "{name}: {}", r.stderr);
        // exactly one JSON object and one final line feed
        assert!(r.stdout.starts_with("{\n") && r.stdout.ends_with("}\n") && !r.stdout.ends_with("\n\n"), "{name}");
        let v = r.json();
        assert_eq!(
            keys(&v),
            sorted(vec![
                "status", "exitCode", "file", "fileSize", "fileSha256", "layout", "layoutVerified", "containerOffset", "entryCount",
                "headerEntryCount", "compatibilityVersion", "entries", "warnings"
            ]),
            "{name}"
        );
        assert_eq!((v["status"].as_str(), v["exitCode"].as_i64()), (Some("ok"), Some(0)));
        assert_eq!(v["file"], path.as_str());
        let bytes = fs::read(fixture_path(name)).unwrap();
        assert_eq!((v["fileSize"].as_u64(), v["fileSha256"].as_str()), (Some(bytes.len() as u64), Some(sha(&bytes).as_str())), "{name}");
        for k in ["layout", "containerOffset", "entryCount", "headerEntryCount", "compatibilityVersion", "entries"] {
            assert_eq!(v[k], exp[name][k], "{name}: {k}");
        }
        assert_eq!(v["layoutVerified"], true);
        assert_eq!(v["warnings"], json!([]));
        for e in v["entries"].as_array().unwrap() {
            assert_eq!(
                keys(e),
                sorted(vec![
                    "name", "storedSize", "storedSha256", "compression", "inflatedSize", "inflatedSha256", "nested", "created",
                    "modified", "createdText", "modifiedText"
                ])
            );
        }
    }
}

#[test]
fn entries_layouts_and_counts() {
    let cases = [
        ("cf-32bit.cf", "32bit", 212, 0),
        ("cf-64bit-stub.cf", "64bit-stub", 63, 4953),
        ("cfe-32bit.cfe", "32bit", 9, 0),
        ("epf-803.epf", "32bit", 14, 0),
        ("epf-802.epf", "32bit", 17, 0),
        ("erf-803.erf", "32bit", 7, 0),
    ];
    for (name, layout, count, offset) in cases {
        let v = run(&["container-entries", &fx(name), "--json"]).json();
        assert_eq!(
            (v["layout"].as_str(), v["entryCount"].as_u64(), v["containerOffset"].as_u64()),
            (Some(layout), Some(count), Some(offset)),
            "{name}"
        );
        assert_eq!(v["entries"].as_array().unwrap().len() as u64, count);
    }
}

#[test]
fn entries_text_mode_has_the_header_and_one_line_per_record() {
    let r = run(&["container-entries", &fx("cf-64bit-stub.cf")]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("layout: 64bit-stub, container offset 4953"), "{}", r.stdout);
    assert!(r.stdout.contains("records: 63 (file header says 63)"));
    assert!(r.stdout.contains("compatibility version: 80319"));
    assert!(r.stdout.contains("size: 403373 bytes, sha256 250cf7c5623fba453fed6b0b3dc11bc21d17982eceecc38127a7d7ae7d5c477b"));
    assert!(r.stdout.contains("warnings: 0"));
    let v = run(&["container-entries", &fx("cf-64bit-stub.cf"), "--json"]).json();
    let lines: Vec<&str> = r.stdout.lines().filter(|l| l.contains("  deflate  ")).collect();
    assert_eq!(lines.len(), 63);
    let first = &v["entries"][0];
    let expect = format!(
        "{}  {}  {}  deflate  {}",
        first["name"].as_str().unwrap(),
        first["storedSize"],
        first["storedSha256"].as_str().unwrap(),
        first["inflatedSize"]
    );
    assert_eq!(lines[0], expect);
    assert!(lines.iter().any(|l| l.ends_with("  nested")));
}

#[test]
fn entries_warnings_and_strict() {
    let dir = tempfile::tempdir().unwrap();
    // header count mismatch + an invalid record
    let recs = [Rec::deflated("ok", &text(1)), Rec::new("junk", random_bytes(300, 99))];
    let bad = put(dir.path(), "bad.cf", &build32(&recs, &Opts::default().header_count(7)));
    let v = run(&["container-entries", &bad, "--json"]).json();
    assert_eq!(v["exitCode"], 0);
    let codes: Vec<&str> = v["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap()).collect();
    assert_eq!(codes, ["HEADER_ENTRY_COUNT_MISMATCH", "ENTRY_INVALID_COMPRESSION"]);
    assert_eq!(v["warnings"][1]["entry"], "junk");
    let junk = v["entries"].as_array().unwrap().iter().find(|e| e["name"] == "junk").unwrap();
    assert_eq!(
        (junk["compression"].as_str(), junk["inflatedSize"].clone(), junk["inflatedSha256"].clone()),
        (Some("invalid"), Value::Null, Value::Null)
    );
    assert_eq!(junk["storedSize"], 300);
    let r = run(&["container-entries", &bad]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("ENTRY_INVALID_COMPRESSION") && r.stdout.contains("\"junk\""), "{}", r.stdout);
    // --strict
    let r = run(&["container-entries", &bad, "--strict", "--json"]);
    assert_eq!(r.code, 3);
    assert_eq!(r.json()["exitCode"], 3);
    assert_eq!(run(&["container-entries", &bad, "--strict"]).code, 3);
    // --strict without warnings
    assert_eq!(run(&["container-entries", &fx("cf-32bit.cf"), "--strict"]).code, 0);
    // unverified 64-bit layout without a stub is a warning
    let p = put(dir.path(), "plain64.cf", &build64(&[Rec::deflated("a", &text(2))], &Opts::default()));
    let v = run(&["container-entries", &p, "--json"]).json();
    assert_eq!((v["layout"].as_str(), v["layoutVerified"].as_bool()), (Some("64bit"), Some(false)));
    assert_eq!(v["warnings"][0]["code"], "LAYOUT_UNVERIFIED");
    assert_eq!(run(&["container-entries", &p, "--strict"]).code, 3);
}

#[test]
fn entries_nested_and_uncompressed_nested() {
    let dir = tempfile::tempdir().unwrap();
    let inner = build32(&[Rec::deflated("x", b"inner")], &Opts::default());
    let recs = [Rec::deflated("packed", &inner), Rec::new("raw", inner.clone()), Rec::deflated("plain", b"text")];
    let p = put(dir.path(), "n.cf", &build32(&recs, &Opts::default()));
    let v = run(&["container-entries", &p, "--json"]).json();
    let get = |n: &str| v["entries"].as_array().unwrap().iter().find(|e| e["name"] == n).unwrap().clone();
    assert_eq!((get("packed")["compression"].as_str(), get("packed")["nested"].as_bool()), (Some("deflate"), Some(true)));
    assert_eq!((get("raw")["compression"].as_str(), get("raw")["nested"].as_bool()), (Some("none"), Some(true)));
    assert_eq!(get("plain")["nested"], false);
    assert_eq!(v["warnings"], json!([]));
    assert!(run(&["container-entries", &p]).stdout.lines().any(|l| l.starts_with("packed  ") && l.ends_with("  nested")));
}

#[test]
fn damaged_files_fail_with_the_right_code() {
    let dir = tempfile::tempdir().unwrap();
    let recs = [Rec::deflated("a", &text(1)), Rec::new("b", random_bytes(500, 5)), Rec::deflated("c", &text(2))];
    let dup = [Rec::deflated("same", b"1"), Rec::deflated("same", b"2")];
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("truncated.cf", build32(&recs, &Opts::default().page(64).truncate(10)), "CONTAINER_CORRUPT"),
        ("truncated64.cf", build64_stub(&recs, &Opts::default().page(64).truncate(10)), "CONTAINER_CORRUPT"),
        ("cycle.cf", build32(&recs, &Opts::default().page(64).cycle_record(1)), "CONTAINER_CYCLE"),
        ("dup.cf", build32(&dup, &Opts::default()), "CONTAINER_DUPLICATE_ENTRY"),
        ("trailing.cf", build32(&recs, &Opts::default().trailing(b"junk")), "CONTAINER_TRAILING_DATA"),
        ("random.cf", random_bytes(2000, 3), "CONTAINER_CORRUPT"),
        ("text.cf", b"this is not a container, only text".repeat(40), "CONTAINER_CORRUPT"),
        ("short.cf", b"short".to_vec(), "CONTAINER_CORRUPT"),
    ];
    let good = put(dir.path(), "good.cf", &build32(&recs, &Opts::default()));
    for (name, bytes, code) in cases {
        let p = put(dir.path(), name, &bytes);
        let r = run(&["container-entries", &p, "--json"]);
        assert_eq!(r.code, 1, "{name}: {}", r.stdout);
        assert!(r.stderr.is_empty(), "{name}: {}", r.stderr);
        let v = r.json();
        assert_eq!((v["status"].as_str(), v["code"].as_str(), v["exitCode"].as_i64()), (Some("error"), Some(code), Some(1)), "{name}: {v}");
        assert!(v.get("entries").is_none() && v.get("counts").is_none(), "no partial result");
        if code == "CONTAINER_CORRUPT" && name != "random.cf" {
            assert!(v["message"].as_str().unwrap().contains("offset"), "{name}: {v}");
        }
        // without --json: nothing on stdout, one line on stderr
        let r = run(&["container-entries", &p]);
        assert_eq!((r.code, r.stdout.as_str()), (1, ""), "{name}");
        assert!(r.stderr.starts_with(&format!("oneform: error: {code}: ")), "{name}: {}", r.stderr);
        // the same through container-diff, on either side
        for (x, y) in [(&p, &good), (&good, &p)] {
            let r = run(&["container-diff", x, y, "--json"]);
            assert_eq!(r.code, 1, "{name}");
            let v = r.json();
            assert_eq!((v["status"].as_str(), v["code"].as_str()), (Some("error"), Some(code)), "{name}");
        }
    }
}

// ---------------------------------------------------------------- container-diff

#[test]
fn diff_fixture_pair_matches_expected_json() {
    let exp = expected();
    let pair = &exp["pair:cf-32bit.cf|cf-64bit-stub.cf"];
    let (a, b) = (fx("cf-32bit.cf"), fx("cf-64bit-stub.cf"));
    let r = run(&["container-diff", &a, &b, "--json"]);
    assert_eq!(r.code, 3, "{}", r.stderr);
    assert!(r.stderr.is_empty());
    let v = r.json();
    assert_eq!(
        keys(&v),
        sorted(vec!["status", "exitCode", "by", "a", "b", "counts", "differ", "onlyA", "onlyB", "warnings"]),
        "no byOwner / unexpectedOwners without the options"
    );
    assert_eq!((v["status"].as_str(), v["exitCode"].as_i64(), v["by"].as_str()), (Some("ok"), Some(3), Some("stored")));
    assert_eq!(v["counts"], json!({"a": 212, "b": 63, "same": 57, "differ": 6, "onlyA": 149, "onlyB": 0}));
    assert_eq!(v["counts"], pair["counts"]);
    assert_eq!(names(&v["differ"]), strings(&pair["differ"]));
    assert_eq!(names(&v["onlyA"]), strings(&pair["onlyA"]));
    assert_eq!(v["onlyB"], json!([]));
    for d in v["differ"].as_array().unwrap() {
        assert_eq!(keys(d), ["contentSame", "name"]);
        assert!(d["contentSame"].is_boolean());
    }
    for d in v["onlyA"].as_array().unwrap() {
        assert_eq!(keys(d), ["name"]);
    }
    assert_eq!(
        (v["a"]["layout"].as_str(), v["a"]["entryCount"].as_u64(), v["a"]["compatibilityVersion"].as_u64()),
        (Some("32bit"), Some(212), Some(80315))
    );
    assert_eq!(
        (v["b"]["layout"].as_str(), v["b"]["containerOffset"].as_u64(), v["b"]["layoutVerified"].as_bool()),
        (Some("64bit-stub"), Some(4953), Some(true))
    );
    assert_eq!(keys(&v["a"]), sorted(vec!["file", "layout", "layoutVerified", "containerOffset", "entryCount", "compatibilityVersion"]));
    // swapped: onlyA and onlyB trade places
    let w = run(&["container-diff", &b, &a, "--json"]);
    assert_eq!(w.code, 3);
    let w = w.json();
    assert_eq!(w["counts"], json!({"a": 63, "b": 212, "same": 57, "differ": 6, "onlyA": 0, "onlyB": 149}));
    assert_eq!(names(&w["onlyB"]), names(&v["onlyA"]));
    assert_eq!(names(&w["differ"]), names(&v["differ"]));
    // text mode
    let t = run(&["container-diff", &a, &b]);
    assert_eq!(t.code, 3);
    assert!(t.stdout.contains("counts: a=212 b=63 same=57 differ=6 onlyA=149 onlyB=0"), "{}", t.stdout);
    assert_eq!(t.stdout.lines().filter(|l| l.starts_with("differ  ")).count(), 6);
    assert_eq!(t.stdout.lines().filter(|l| l.starts_with("onlyA  ")).count(), 149);
    assert_eq!(t.stdout.lines().filter(|l| l.starts_with("onlyB  ")).count(), 0);
}

#[test]
fn diff_of_a_file_with_itself_is_clean() {
    for name in ["cf-32bit.cf", "cf-64bit-stub.cf"] {
        let f = fx(name);
        let r = run(&["container-diff", &f, &f, "--json"]);
        assert_eq!(r.code, 0, "{name}");
        let v = r.json();
        assert_eq!((v["differ"].clone(), v["onlyA"].clone(), v["onlyB"].clone()), (json!([]), json!([]), json!([])));
        assert_eq!(v["counts"]["same"], v["counts"]["a"]);
        assert_eq!(run(&["container-diff", &f, &f]).code, 0);
    }
}

#[test]
fn diff_repack_only_record() {
    let dir = tempfile::tempdir().unwrap();
    let plain = text(7);
    let (packed, stored) = (deflate_raw(&plain), deflate_stored(&plain));
    assert_ne!(packed, stored);
    let a = put(dir.path(), "a.cf", &build32(&[Rec::new("r", packed), Rec::deflated("s", b"same")], &Opts::default()));
    let b = put(dir.path(), "b.cf", &build32(&[Rec::new("r", stored), Rec::deflated("s", b"same")], &Opts::default()));
    let r = run(&["container-diff", &a, &b, "--json"]);
    assert_eq!(r.code, 3);
    let v = r.json();
    assert_eq!(v["counts"], json!({"a": 2, "b": 2, "same": 1, "differ": 1, "onlyA": 0, "onlyB": 0}));
    assert_eq!(v["differ"], json!([{"name": "r", "contentSame": true}]));
    assert!(run(&["container-diff", &a, &b]).stdout.contains("differ  r  content=same"));
    // by inflated the record is the same
    let r = run(&["container-diff", &a, &b, "--by", "inflated", "--json"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let v = r.json();
    assert_eq!((v["by"].as_str(), v["counts"]["same"].as_u64(), v["differ"].clone()), (Some("inflated"), Some(2), json!([])));
    // another content: contentSame false in both modes
    let c = put(dir.path(), "c.cf", &build32(&[Rec::deflated("r", &text(8)), Rec::deflated("s", b"same")], &Opts::default()));
    let v = run(&["container-diff", &a, &c, "--by", "inflated", "--json"]).json();
    assert_eq!(v["differ"], json!([{"name": "r", "contentSame": false}]));
    let v = run(&["container-diff", &a, &c, "--json"]).json();
    assert_eq!(v["differ"], json!([{"name": "r", "contentSame": false}]));
    assert!(run(&["container-diff", &a, &c]).stdout.contains("differ  r  content=differs"));
}

#[test]
fn diff_invalid_records() {
    let dir = tempfile::tempdir().unwrap();
    let junk1 = random_bytes(300, 99);
    let junk2 = random_bytes(300, 98);
    let ok = || Rec::deflated("ok", b"fine");
    let a = put(dir.path(), "a.cf", &build32(&[ok(), Rec::new("junk", junk1.clone()), Rec::new("only-a", random_bytes(200, 97))], &Opts::default()));
    let b = put(dir.path(), "b.cf", &build32(&[ok(), Rec::new("junk", junk2)], &Opts::default()));
    let same = put(dir.path(), "same.cf", &build32(&[ok(), Rec::new("junk", junk1), Rec::new("only-a", random_bytes(200, 97))], &Opts::default()));
    // by stored: listed, contentSame unknown, warnings of both sides are prefixed and carry the side
    let r = run(&["container-diff", &a, &b, "--json"]);
    assert_eq!(r.code, 3);
    let v = r.json();
    assert_eq!(v["differ"], json!([{"name": "junk", "contentSame": null}]));
    assert_eq!(names(&v["onlyA"]), ["only-a"]);
    let w = v["warnings"].as_array().unwrap();
    assert_eq!(w.len(), 3, "{v}");
    let side = |f: &str| w.iter().filter(|x| x["file"] == f).count();
    assert_eq!((side("a"), side("b")), (2, 1));
    assert!(w.iter().all(|x| x["code"] == "ENTRY_INVALID_COMPRESSION"));
    assert!(w.iter().filter(|x| x["file"] == "a").all(|x| x["message"].as_str().unwrap().starts_with("A: ")));
    assert!(w.iter().filter(|x| x["file"] == "b").all(|x| x["message"].as_str().unwrap().starts_with("B: ")));
    // by stored, equal invalid bytes: same
    let v = run(&["container-diff", &a, &same, "--json"]).json();
    assert_eq!(v["counts"]["same"], 3);
    // by inflated: an invalid record present in both files fails
    let r = run(&["container-diff", &a, &b, "--by", "inflated", "--json"]);
    assert_eq!(r.code, 1);
    let v = r.json();
    assert_eq!((v["status"].as_str(), v["code"].as_str()), (Some("error"), Some("CONTAINER_ENTRY_INVALID")));
    let m = v["message"].as_str().unwrap();
    assert!(m.contains("\"junk\"") && m.contains("A (") && m.contains("B ("), "{m}");
    assert!(!m.contains("only-a"));
    let r = run(&["container-diff", &a, &b, "--by", "inflated"]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.is_empty() && r.stderr.starts_with("oneform: error: CONTAINER_ENTRY_INVALID: "), "{}", r.stderr);
    // invalid on one side only
    let c = put(dir.path(), "c.cf", &build32(&[ok(), Rec::deflated("junk", b"x")], &Opts::default()));
    let v = run(&["container-diff", &c, &a, "--by", "inflated", "--json"]).json();
    assert_eq!(v["code"], "CONTAINER_ENTRY_INVALID");
    assert!(v["message"].as_str().unwrap().contains("B ("));
    // records only in one file are not compared: no error
    let d = put(dir.path(), "d.cf", &build32(&[ok()], &Opts::default()));
    let r = run(&["container-diff", &a, &d, "--by", "inflated", "--json"]);
    assert_eq!(r.code, 3);
    assert_eq!(names(&r.json()["onlyA"]), ["junk", "only-a"]);
}

#[test]
fn diff_mixed_layouts_with_equal_content() {
    let dir = tempfile::tempdir().unwrap();
    let recs: Vec<Rec> = (0..12).map(|i| Rec::deflated(&format!("rec{i:02}"), &text(i))).collect();
    let a = put(dir.path(), "a32.cf", &build32(&recs, &Opts::default()));
    let b = put(dir.path(), "b64stub.cf", &build64_stub(&recs, &Opts::default().page(128)));
    let c = put(dir.path(), "c64.cf", &build64(&recs, &Opts::default()));
    let r = run(&["container-diff", &a, &b, "--json"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let v = r.json();
    assert_eq!(v["counts"], json!({"a": 12, "b": 12, "same": 12, "differ": 0, "onlyA": 0, "onlyB": 0}));
    assert_eq!((v["a"]["layout"].as_str(), v["b"]["layout"].as_str()), (Some("32bit"), Some("64bit-stub")));
    assert_eq!(v["warnings"], json!([]));
    assert_eq!(run(&["container-diff", &b, &a]).code, 0);
    // the 64-bit layout without a stub is unverified: equal content, a warning of side B
    let r = run(&["container-diff", &a, &c, "--json"]);
    assert_eq!(r.code, 0);
    let v = r.json();
    assert_eq!(v["b"]["layoutVerified"], false);
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1);
    assert_eq!((v["warnings"][0]["code"].as_str(), v["warnings"][0]["file"].as_str()), (Some("LAYOUT_UNVERIFIED"), Some("b")));
    assert!(v["warnings"][0]["message"].as_str().unwrap().starts_with("B: "));
}

#[test]
fn many_blocks_and_scattered_chains() {
    // several blocks per document, a table of contents of several blocks, other placement: same facts
    let dir = tempfile::tempdir().unwrap();
    let recs: Vec<Rec> = (0..30).map(|i| Rec::deflated(&format!("r{i:02}"), &text(i))).collect();
    let a = put(dir.path(), "a.cf", &build32(&recs, &Opts::default()));
    let b = put(dir.path(), "b.cf", &build64_stub(&recs, &Opts::default().page(48).toc_page(96).header_page(40).scatter()));
    let r = run(&["container-diff", &a, &b, "--json"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert_eq!(r.json()["counts"]["same"], 30);
}

// ---------------------------------------------------------------- owners

/// A UUID with hexadecimal letters in it (the dump holds it in capitals in one place).
fn uu(n: u32) -> String {
    format!("{n:08x}-aaaa-4bbb-8ccc-{n:012x}")
}

fn xml(root_child: &str, uuid: &str, more: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- generated -->\r\n<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"2.17\">\r\n\t<{root_child} uuid=\"{uuid}\">\r\n\t\t<Properties><Name>x</Name></Properties>{more}\r\n\t</{root_child}>\r\n</MetaDataObject>"
    )
}

const U_CONFIG: u32 = 1;
const U_DOC: u32 = 2;
const U_FORM: u32 = 3;
const U_CAT: u32 = 4;
const U_LANG: u32 = 5;
const U_SUB: u32 = 6;
const U_SUB_CHILD: u32 = 7;
const U_EXT: u32 = 8;
const U_INFO: u32 = 9;
const U_ENUM: u32 = 10;
const U_REPORT_FORM: u32 = 11;
const U_NOWHERE: u32 = 12;
const U_LATER: u32 = 13;

/// Two dumps in `root/one` and `root/two`; returns their paths.
fn make_dumps(root: &Path) -> (String, String) {
    let one = root.join("one");
    let two = root.join("two");
    let later = format!("<Item uuid=\"{}\"/>", uu(U_LATER));
    let w = |base: &Path, rel: &str, content: &str| {
        put(base, rel, content.as_bytes());
    };
    w(&one, "Configuration.xml", &xml("Configuration", &uu(U_CONFIG), ""));
    w(&one, "Documents/Расходная.xml", &xml("Document", &uu(U_DOC).to_uppercase(), &later));
    w(&one, "Documents/Расходная/Forms/ФормаДокумента.xml", &xml("Form", &uu(U_FORM), &later));
    // ignored: below Ext, and the dump info file
    w(&one, "Documents/Расходная/Ext/Predefined.xml", &xml("Item", &uu(U_EXT), ""));
    w(&one, "ConfigDumpInfo.xml", &xml("ConfigDumpInfo", &uu(U_INFO), ""));
    // a catalog with a BOM; the same uuid in a second file (duplicate)
    let mut cat = b"\xef\xbb\xbf".to_vec();
    cat.extend_from_slice(xml("Catalog", &uu(U_CAT), "").as_bytes());
    put(&one, "Catalogs/Товары.xml", &cat);
    w(&one, "Catalogs/Товары2.xml", &xml("Catalog", &uu(U_CAT), ""));
    w(&one, "Languages/Русский.xml", &xml("Language", &uu(U_LANG), ""));
    w(&one, "Subsystems/Родитель.xml", &xml("Subsystem", &uu(U_SUB), ""));
    w(&one, "Subsystems/Родитель/Subsystems/Дочерняя.xml", &xml("Subsystem", &uu(U_SUB_CHILD), ""));
    // not an owner: a stray file at the root, a file that is not xml
    w(&one, "Other.xml", &xml("Other", &uu(99), ""));
    w(&one, "Catalogs/Notes.txt", &xml("Catalog", &uu(98), ""));
    // second dump: the catalog under another name (the first dump wins), an enum, a form without the object file
    w(&two, "Catalogs/ДругойТовары.xml", &xml("Catalog", &uu(U_CAT), ""));
    w(&two, "Enums/Перечисление.xml", &xml("Enum", &uu(U_ENUM), ""));
    w(&two, "Reports/Отчёт/Forms/Форма.xml", &xml("Form", &uu(U_REPORT_FORM), ""));
    (one.to_string_lossy().into_owned(), two.to_string_lossy().into_owned())
}

/// A: U_DOC (differs), U_FORM.0, root (differs), version, U_NOWHERE, U_CONFIG, U_SUB, U_LANG, U_ENUM.1c (same)
/// B: U_DOC, root, U_CAT, U_EXT, U_INFO, U_SUB_CHILD, U_REPORT_FORM, U_ENUM.1c
fn owner_pair(dir: &Path) -> (String, String) {
    let rec = |n: &str, c: &str| Rec::deflated(n, c.as_bytes());
    let a = vec![
        rec(&uu(U_DOC), "a"),
        rec(&format!("{}.0", uu(U_FORM)), "a"),
        rec("root", "ra"),
        rec("version", "v"),
        rec(&uu(U_NOWHERE), "x"),
        rec(&uu(U_CONFIG), "c"),
        rec(&uu(U_SUB), "s"),
        rec(&uu(U_LANG), "l"),
        rec(&format!("{}.1c", uu(U_ENUM)), "e"),
    ];
    let b = vec![
        rec(&uu(U_DOC), "b"),
        rec("root", "rb"),
        rec(&uu(U_CAT), "n"),
        rec(&uu(U_EXT), "n"),
        rec(&uu(U_INFO), "n"),
        rec(&uu(U_SUB_CHILD), "n"),
        rec(&uu(U_REPORT_FORM), "n"),
        rec(&format!("{}.1c", uu(U_ENUM)), "e"),
    ];
    (put(dir, "a.cf", &build32(&a, &Opts::default())), put(dir, "b.cf", &build64_stub(&b, &Opts::default())))
}

fn owner_map(v: &Value) -> BTreeMap<String, String> {
    ["differ", "onlyA", "onlyB"]
        .iter()
        .flat_map(|k| v[*k].as_array().unwrap())
        .map(|x| (x["name"].as_str().unwrap().to_string(), x["owner"].as_str().unwrap().to_string()))
        .collect()
}

const ALL_OWNERS: [&str; 8] = [
    "Document.Расходная",
    "service",
    "unmapped",
    "Configuration",
    "Subsystem.Родитель",
    "Language.Русский",
    "Catalog.Товары",
    "Reports.Отчёт",
];

#[test]
fn owners_of_records() {
    let dir = tempfile::tempdir().unwrap();
    let (one, two) = make_dumps(&dir.path().join("dumps"));
    let (a, b) = owner_pair(dir.path());
    let r = run(&["container-diff", &a, &b, "--owners", &one, "--owners", &two, "--json"]);
    assert_eq!(r.code, 3, "{}", r.stdout);
    let v = r.json();
    let owners = owner_map(&v);
    let expect = [
        (uu(U_DOC), "Document.Расходная"),
        (format!("{}.0", uu(U_FORM)), "Document.Расходная"),
        ("root".to_string(), "service"),
        ("version".to_string(), "service"),
        (uu(U_NOWHERE), "unmapped"),
        (uu(U_CONFIG), "Configuration"),
        (uu(U_SUB), "Subsystem.Родитель"),
        (uu(U_LANG), "Language.Русский"),
        (uu(U_CAT), "Catalog.Товары"),
        (uu(U_EXT), "unmapped"),
        (uu(U_INFO), "unmapped"),
        (uu(U_SUB_CHILD), "Subsystem.Родитель"),
        (uu(U_REPORT_FORM), "Reports.Отчёт"),
    ];
    let expect: BTreeMap<String, String> = expect.into_iter().map(|(k, v)| (k, v.to_string())).collect();
    assert_eq!(owners, expect);
    assert_eq!(v["counts"]["same"], 1);
    // grouped counts
    let group = |differ: u64, only_a: u64, only_b: u64| json!({"differ": differ, "onlyA": only_a, "onlyB": only_b});
    assert_eq!(
        v["byOwner"],
        json!({
            "Document.Расходная": group(1, 1, 0),
            "service": group(1, 1, 0),
            "unmapped": group(0, 1, 2),
            "Configuration": group(0, 1, 0),
            "Subsystem.Родитель": group(0, 1, 1),
            "Language.Русский": group(0, 1, 0),
            "Catalog.Товары": group(0, 0, 1),
            "Reports.Отчёт": group(0, 0, 1),
        })
    );
    assert!(v.get("unexpectedOwners").is_none());
    // the duplicate uuid of the first dump is a warning that names both files
    let dup: Vec<&Value> = v["warnings"].as_array().unwrap().iter().filter(|w| w["code"] == "DUPLICATE_UUID").collect();
    assert_eq!(dup.len(), 1, "{}", v["warnings"]);
    let m = dup[0]["message"].as_str().unwrap();
    assert!(m.contains(&uu(U_CAT)) && m.contains("Catalogs/Товары.xml") && m.contains("Catalogs/Товары2.xml"), "{m}");
    assert!(dup[0].get("file").is_none());
    // text mode
    let t = run(&["container-diff", &a, &b, "--owners", &one, "--owners", &two]);
    assert_eq!(t.code, 3);
    assert!(t.stdout.contains(&format!("differ  {}  content=differs  owner=Document.Расходная", uu(U_DOC))), "{}", t.stdout);
    assert!(t.stdout.contains(&format!("onlyB  {}  owner=Catalog.Товары", uu(U_CAT))));
    assert!(t.stdout.contains("by owner:\n  Catalog.Товары  differ=0 onlyA=0 onlyB=1"), "{}", t.stdout);
    assert!(t.stdout.contains("DUPLICATE_UUID"));
}

#[test]
fn owners_precedence_follows_the_order_of_the_dumps() {
    let dir = tempfile::tempdir().unwrap();
    let (one, two) = make_dumps(&dir.path().join("dumps"));
    let (a, b) = owner_pair(dir.path());
    let v = run(&["container-diff", &a, &b, "--owners", &two, "--owners", &one, "--json"]).json();
    assert_eq!(owner_map(&v)[&uu(U_CAT)], "Catalog.ДругойТовары");
    // only the second dump is given: the documents of the first are unmapped, the enum is known
    let v = run(&["container-diff", &a, &b, "--owners", &two, "--json"]).json();
    let owners = owner_map(&v);
    assert_eq!(owners[&uu(U_DOC)], "unmapped");
    assert_eq!(owners[&uu(U_CAT)], "Catalog.ДругойТовары");
    assert_eq!(owners[&uu(U_REPORT_FORM)], "Reports.Отчёт");
    assert!(v["warnings"].as_array().unwrap().iter().all(|w| w["code"] != "DUPLICATE_UUID"));
}

#[test]
fn expect_owners_decide_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let (one, two) = make_dumps(&dir.path().join("dumps"));
    let (a, b) = owner_pair(dir.path());
    let with = |extra: &[&str]| -> Vec<String> {
        let mut args: Vec<String> = ["container-diff", a.as_str(), b.as_str(), "--owners", one.as_str(), "--owners", two.as_str()]
            .iter()
            .map(|x| x.to_string())
            .collect();
        args.extend(extra.iter().map(|x| x.to_string()));
        args
    };
    let call = |extra: &[&str]| {
        let args = with(extra);
        run(&args.iter().map(String::as_str).collect::<Vec<_>>())
    };
    // BOM, CRLF, comments, blanks, spaces
    let mut file = b"\xef\xbb\xbf# expected owners\r\n\r\n".to_vec();
    for o in ALL_OWNERS {
        file.extend_from_slice(format!("  {o}  \r\n").as_bytes());
    }
    file.extend_from_slice("   \r\n# Catalog.Лишний\r\n".as_bytes());
    let exp = put(dir.path(), "expected.txt", &file);
    let r = call(&["--expect-owners", &exp, "--json"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let v = r.json();
    assert_eq!(v["unexpectedOwners"], json!([]));
    assert_eq!(v["exitCode"], 0);
    assert_eq!(v["counts"]["differ"], 2, "differences are present");
    assert_eq!(call(&["--expect-owners", &exp]).code, 0);
    // one owner missing from the file
    let without: Vec<&str> = ALL_OWNERS.iter().copied().filter(|o| *o != "Catalog.Товары").collect();
    let exp2 = put(dir.path(), "expected2.txt", without.join("\n").as_bytes());
    let r = call(&["--expect-owners", &exp2, "--json"]);
    assert_eq!(r.code, 3);
    let v = r.json();
    assert_eq!((v["unexpectedOwners"].clone(), v["exitCode"].clone()), (json!(["Catalog.Товары"]), json!(3)));
    let t = call(&["--expect-owners", &exp2]);
    assert_eq!(t.code, 3);
    assert!(t.stdout.contains("unexpected owners: 1\n  Catalog.Товары"), "{}", t.stdout);
    // service and unmapped are ordinary names
    let no_special: Vec<&str> = ALL_OWNERS.iter().copied().filter(|o| !matches!(*o, "service" | "unmapped")).collect();
    let exp3 = put(dir.path(), "expected3.txt", no_special.join("\n").as_bytes());
    let v = call(&["--expect-owners", &exp3, "--json"]).json();
    assert_eq!(v["unexpectedOwners"], json!(["service", "unmapped"]));
    assert_eq!(v["exitCode"], 3);
    // an empty file expects nothing
    let empty = put(dir.path(), "empty.txt", b"");
    let v = call(&["--expect-owners", &empty, "--json"]).json();
    assert_eq!(v["unexpectedOwners"].as_array().unwrap().len(), ALL_OWNERS.len());
    // no differences: exit 0 whatever the file says
    let r = run(&["container-diff", &a, &a, "--owners", &one, "--expect-owners", &empty, "--json"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.json()["unexpectedOwners"], json!([]));
    assert_eq!(run(&["container-diff", &a, &a, "--owners", &one]).code, 0);
    // without --expect-owners any difference is 3
    assert_eq!(call(&[]).code, 3);
}

#[test]
fn owners_from_an_empty_dump() {
    let dir = tempfile::tempdir().unwrap();
    let empty_dump = dir.path().join("empty");
    fs::create_dir_all(&empty_dump).unwrap();
    let (a, b) = owner_pair(dir.path());
    let v = run(&["container-diff", &a, &b, "--owners", s(&empty_dump), "--json"]).json();
    let owners = owner_map(&v);
    assert!(owners.values().all(|o| o == "service" || o == "unmapped"));
    assert_eq!(v["byOwner"].as_object().unwrap().len(), 2);
}

#[cfg(unix)]
#[test]
fn owners_do_not_follow_links() {
    let dir = tempfile::tempdir().unwrap();
    let (one, _) = make_dumps(&dir.path().join("dumps"));
    let outside = dir.path().join("outside");
    put(&outside, "Linked.xml", xml("Catalog", &uu(U_NOWHERE), "").as_bytes());
    std::os::unix::fs::symlink(&outside, Path::new(&one).join("Catalogs/LinkedDir")).unwrap();
    std::os::unix::fs::symlink(outside.join("Linked.xml"), Path::new(&one).join("Catalogs/Linked.xml")).unwrap();
    let (a, b) = owner_pair(dir.path());
    let v = run(&["container-diff", &a, &b, "--owners", &one, "--json"]).json();
    assert_eq!(owner_map(&v)[&uu(U_NOWHERE)], "unmapped");
}

// ---------------------------------------------------------------- usage errors

#[test]
fn usage_errors_exit_2_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let (cf, other) = (fx("cf-32bit.cf"), fx("cf-64bit-stub.cf"));
    let plain_file = put(dir.path(), "plain.txt", b"x");
    let missing = s(&dir.path().join("missing.cf")).to_string();
    let folder = s(dir.path()).to_string();
    let no_such_expect = s(&dir.path().join("no-such.txt")).to_string();
    let (e, d) = ("container-entries", "container-diff");
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec![e], "needs a container file"),
        (vec![e, &cf, &other], "unexpected argument"),
        (vec![e, &folder], "is not a file"),
        (vec![e, &missing], "is not a file"),
        (vec![e, &cf, "--by", "stored"], "--by is only valid for container-diff"),
        (vec![e, &cf, "--owners", &folder], "--owners is only valid for container-diff"),
        (vec![e, &cf, "--expect-owners", &plain_file], "--expect-owners is only valid for container-diff"),
        (vec![e, &cf, "--dry-run"], "container-entries takes only"),
        (vec![e, &cf, "--experimental"], "--experimental is only valid"),
        (vec![e, &cf, "--oof-path", "x"], "--oof-path is not valid"),
        (vec![e, &cf, "--out", &folder], "--out and --check are only valid for export-forms"),
        (vec![e, &cf, "--check"], "--out and --check are only valid for export-forms"),
        (vec![e, &cf, "-j", "0"], "--workers must be at least 1"),
        (vec![e, &cf, "--replace-generated"], "takes only"),
        (vec![e, &cf, "--carriers"], "takes only"),
        (vec![d], "needs two container files"),
        (vec![d, &cf], "needs two container files"),
        (vec![d, &cf, &other, &cf], "unexpected argument"),
        (vec![d, &cf, &other, "--by", "bytes"], "--by must be stored or inflated"),
        (vec![d, &cf, &other, "--by", "Stored"], "--by must be stored or inflated"),
        (vec![d, &cf, &folder], "is not a file"),
        (vec![d, &folder, &cf], "is not a file"),
        (vec![d, &cf, &missing], "is not a file"),
        (vec![d, &cf, &other, "--owners", &plain_file], "is not a directory"),
        (vec![d, &cf, &other, "--owners", &missing], "is not a directory"),
        (vec![d, &cf, &other, "--owners", &folder, "--expect-owners", &no_such_expect], "--expect-owners"),
        (vec![d, &cf, &other, "--expect-owners", &plain_file], "needs at least one --owners"),
        (vec![d, &cf, &other, "--strict"], "--strict is only valid for lint and container-entries"),
        (vec![d, &cf, &other, "--dry-run"], "container-diff takes only"),
        (vec![d, &cf, &other, "--experimental"], "--experimental is only valid"),
        (vec![d, &cf, &other, "--oof-path", "x"], "--oof-path is not valid"),
        (vec![d, &cf, &other, "--out", &folder], "--out and --check are only valid for export-forms"),
        (vec![d, &cf, &other, "--check"], "--out and --check are only valid for export-forms"),
        (vec![d, &cf, &other, "-j", "0"], "--workers must be at least 1"),
        (vec!["extract", &folder, "--by", "stored"], "--by is only valid for container-diff"),
        (vec!["lint", &folder, "--owners", &folder], "--owners is only valid for container-diff"),
        (vec!["check", &folder, "--expect-owners", &plain_file], "--expect-owners is only valid for container-diff"),
        (vec!["extract", &folder, "--strict"], "only valid for lint and container-entries"),
    ];
    for (args, message) in cases {
        for json_flag in [false, true] {
            let mut full = args.clone();
            if json_flag {
                full.push("--json");
            }
            let r = run_with(Some(cwd.path()), &[], &full);
            assert_eq!(r.code, 2, "{full:?}: {}{}", r.stdout, r.stderr);
            assert!(r.stdout.is_empty(), "{full:?}: {}", r.stdout);
            assert!(r.stderr.starts_with("oneform: error: ") && r.stderr.contains(message), "{full:?}: {}", r.stderr);
        }
    }
    assert_eq!(fs::read_dir(cwd.path()).unwrap().count(), 0, "nothing is written to the working directory");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "nothing is written next to the arguments");
}

// ---------------------------------------------------------------- read-only, independence, workers

type Snapshot = BTreeMap<PathBuf, (bool, Vec<u8>, Option<SystemTime>)>;

fn snapshot(root: &Path) -> Snapshot {
    fn walk(dir: &Path, root: &Path, out: &mut Snapshot) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let meta = fs::symlink_metadata(&p).unwrap();
            let rel = p.strip_prefix(root).unwrap().to_path_buf();
            if meta.is_dir() {
                out.insert(rel, (true, Vec::new(), meta.modified().ok()));
                walk(&p, root, out);
            } else {
                out.insert(rel, (false, fs::read(&p).unwrap(), meta.modified().ok()));
            }
        }
    }
    let mut out = Snapshot::new();
    walk(root, root, &mut out);
    out
}

fn copy_fixtures(to: &Path) {
    for n in ["cf-32bit.cf", "cf-64bit-stub.cf", "cfe-32bit.cfe"] {
        fs::copy(fixture_path(n), to.join(n)).unwrap();
    }
}

#[test]
fn commands_only_read() {
    let work = tempfile::tempdir().unwrap();
    let inputs = work.path().join("inputs");
    let dumps = work.path().join("dumps");
    let cwd = work.path().join("cwd");
    fs::create_dir_all(&inputs).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    copy_fixtures(&inputs);
    let (a, b) = owner_pair(&inputs);
    let (one, two) = make_dumps(&dumps);
    fs::write(inputs.join("expected.txt"), "service\nunmapped\n").unwrap();
    let expect = s(&inputs.join("expected.txt")).to_string();
    let missing = s(&inputs.join("missing.cf")).to_string();
    let (c32, c64) = (s(&inputs.join("cf-32bit.cf")).to_string(), s(&inputs.join("cf-64bit-stub.cf")).to_string());
    let before = (snapshot(&inputs), snapshot(&dumps), snapshot(&cwd));
    std::thread::sleep(std::time::Duration::from_millis(30));
    let runs: Vec<Vec<&str>> = vec![
        vec!["container-entries", &c32],
        vec!["container-entries", &c64, "--json"],
        vec!["container-entries", &c64, "--strict"],
        vec!["container-diff", &c32, &c64],
        vec!["container-diff", &c32, &c64, "--json", "--by", "inflated"],
        vec!["container-diff", &a, &b, "--owners", &one, "--owners", &two, "--json"],
        vec!["container-diff", &a, &b, "--owners", &one, "--expect-owners", &expect],
        vec!["container-entries", &missing],
    ];
    for args in &runs {
        let r = run_with(Some(&cwd), &[], args);
        assert!(matches!(r.code, 0 | 2 | 3), "{args:?}: {}", r.stderr);
    }
    let after = (snapshot(&inputs), snapshot(&dumps), snapshot(&cwd));
    assert!(before == after, "files or modification times changed");
    assert!(after.2.is_empty());
    for path in after.0.keys().chain(after.1.keys()) {
        assert!(!path.file_name().unwrap().to_string_lossy().starts_with(".oneform"), "{path:?}");
    }
}

#[test]
fn output_does_not_depend_on_the_environment() {
    let work = tempfile::tempdir().unwrap();
    let inputs = work.path().join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    copy_fixtures(&inputs);
    let (c32, c64) = (s(&inputs.join("cf-32bit.cf")).to_string(), s(&inputs.join("cf-64bit-stub.cf")).to_string());
    let cwd_plain = tempfile::tempdir().unwrap();
    let cwd_env = tempfile::tempdir().unwrap();
    let (a, b) = owner_pair(&inputs);
    let (one, _) = make_dumps(&work.path().join("dumps"));
    let calls: Vec<Vec<&str>> = vec![
        vec!["container-entries", &c32, "--json"],
        vec!["container-entries", &c64],
        vec!["container-diff", &c32, &c64, "--json"],
        vec!["container-diff", &c32, &c64],
        vec!["container-diff", &a, &b, "--owners", &one, "--json"],
    ];
    let plain: Vec<Run> = calls.iter().map(|c| run_with(Some(cwd_plain.path()), &[], c)).collect();
    // a project config with tiny limits next to the inputs and in the working directory
    let cfg = r#"{"limits": {"maxEntries": 1, "maxEntryBytes": 1, "maxExpandedBytes": 1, "maxInputBytes": 1}, "oofPath": "bogus", "carriers": true}"#;
    fs::write(inputs.join(".oneform.json"), cfg).unwrap();
    fs::write(cwd_env.path().join(".oneform.json"), cfg).unwrap();
    let envs = [("EPF_OOF_PATH", "bogus"), ("ONEFORM_CARRIERS", "1"), ("ONEFORM_DIAG_CAP", "0"), ("ONEFORM_HISTORY_KEEP", "0")];
    let snap = snapshot(&inputs);
    for (c, p) in calls.iter().zip(&plain) {
        assert!(p.code == 0 || p.code == 3, "{c:?}: {}", p.stderr);
        let r = run_with(Some(cwd_env.path()), &envs, c);
        assert_eq!((r.code, &r.stdout, &r.stderr), (p.code, &p.stdout, &p.stderr), "{c:?}");
    }
    assert!(snapshot(&inputs) == snap);
    assert_eq!(snapshot(cwd_env.path()).len(), 1, "only the config that the test wrote");
}

#[test]
fn workers_do_not_change_the_output() {
    let (a, b) = (fx("cf-32bit.cf"), fx("cf-64bit-stub.cf"));
    let dir = tempfile::tempdir().unwrap();
    let (one, two) = make_dumps(&dir.path().join("dumps"));
    let (oa, ob) = owner_pair(dir.path());
    let calls: Vec<Vec<&str>> = vec![
        vec!["container-entries", &a],
        vec!["container-entries", &b, "--json"],
        vec!["container-diff", &a, &b],
        vec!["container-diff", &a, &b, "--json"],
        vec!["container-diff", &a, &b, "--json", "--by", "inflated"],
        vec!["container-diff", &oa, &ob, "--owners", &one, "--owners", &two, "--json"],
    ];
    for c in calls {
        let with = |extra: [&str; 2]| {
            let mut args = c.clone();
            args.extend(extra);
            run(&args)
        };
        let (x, y) = (with(["-j", "1"]), with(["-j", "8"]));
        assert_eq!((x.code, &x.stdout), (y.code, &y.stdout), "{c:?}");
        assert!(!x.stdout.is_empty());
        assert_eq!(with(["--workers", "2"]).stdout, x.stdout);
    }
}

// ---------------------------------------------------------------- help

#[test]
fn help_lists_both_commands_in_the_stable_part() {
    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    let t = &r.stdout;
    let stable = t.find("STABLE (no flag needed):").expect("stable heading");
    let experimental = t.find("EXPERIMENTAL (needs --experimental): writing").expect("experimental heading");
    for needle in [
        "  container-entries FILE [--json] [--strict]",
        "  container-diff A B [--by stored|inflated] [--owners DUMP]... [--expect-owners FILE] [--json]",
    ] {
        let at = t.find(needle).unwrap_or_else(|| panic!("{needle} missing in\n{t}"));
        assert!(stable < at && at < experimental, "{needle}");
    }
    for needle in ["--by <stored|inflated>", "--owners <DUMP>", "--expect-owners <FILE>", "oneform container-entries FILE", "oneform container-diff A B"] {
        assert!(t.contains(needle), "{needle} missing in\n{t}");
    }
    // the unknown-command message names them
    let r = run(&["bogus", "."]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("export-forms, container-entries, container-diff"), "{}", r.stderr);
}

/// Real files, driven by the environment (not part of the normal run):
/// `ONEFORM_REAL_CF_A`, `ONEFORM_REAL_CF_B`, and optionally `ONEFORM_REAL_OWNERS` (XML dump folders separated by the
/// platform path separator). Compares `container-diff` with the Python oracle (an independent parser), checks that every
/// listed record has an owner when owners are given, and prints the time.
#[test]
#[ignore = "needs ONEFORM_REAL_CF_A and ONEFORM_REAL_CF_B"]
fn real_pair_matches_the_oracle() {
    let (Ok(a), Ok(b)) = (std::env::var("ONEFORM_REAL_CF_A"), std::env::var("ONEFORM_REAL_CF_B")) else {
        panic!("set ONEFORM_REAL_CF_A and ONEFORM_REAL_CF_B");
    };
    let owners: Vec<String> = std::env::var("ONEFORM_REAL_OWNERS")
        .map(|v| std::env::split_paths(&v).map(|p| p.to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    let python = if cfg!(windows) { "python" } else { "python3" };
    let oracle = Command::new(python)
        .args(["scripts/container_oracle.py", "diff", &a, &b])
        .output()
        .expect("python is needed for the oracle");
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    let oracle: Value = serde_json::from_slice(&oracle.stdout).unwrap();

    let mut args: Vec<&str> = vec!["container-diff", &a, &b, "--json"];
    for o in &owners {
        args.push("--owners");
        args.push(o);
    }
    let started = std::time::Instant::now();
    let r = run(&args);
    let seconds = started.elapsed().as_secs_f64();
    eprintln!("container-diff of the real pair took {seconds:.1} s ({} build)", if cfg!(debug_assertions) { "debug" } else { "release" });
    let v = r.json();
    assert!(r.code == 0 || r.code == 3, "exit {}: {}", r.code, r.stderr);
    assert_eq!(v["counts"], oracle["counts"], "counts differ from the oracle");
    for list in ["differ", "onlyA", "onlyB"] {
        assert_eq!(names(&v[list]), strings(&oracle[list]), "{list} differs from the oracle");
        if !owners.is_empty() {
            for item in v[list].as_array().unwrap() {
                assert!(item["owner"].as_str().is_some_and(|o| !o.is_empty()), "no owner for {item}");
            }
        }
    }
    if let Some(by) = v.get("byOwner") {
        eprintln!("byOwner: {}", serde_json::to_string(by).unwrap());
    }
}
