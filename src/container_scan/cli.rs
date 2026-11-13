//! `oneform container-entries <FILE>` and `oneform container-diff <A> <B>`: the command line side of the container tools.
//!
//! Both commands only read their arguments. They never look at `.oneform`, history, diagnostics, `EPF_OOF_PATH`,
//! `ONEFORM_CARRIERS` or `.oneform.json`, and they write nothing but stdout / stderr.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use super::owners::{parse_expected, OwnerIndex};
use super::{compatibility_version, open, process_entries, warnings_of, Compression, EntryFacts, ScanWarning, ScannedContainer};
use crate::canonical::{dumps_with, JsonOpts};
use crate::cli_epf::{EXIT_DIFFERS, EXIT_FAILED, EXIT_OK, EXIT_USAGE};
use crate::errors::{ExtractionError, Result, CONTAINER_ENTRY_INVALID, IO_ERROR};

const PRETTY_SORTED: JsonOpts = JsonOpts { indent: Some(2), sort_keys: true, ensure_ascii: false, compact: false };
/// Read buffer for the whole-file SHA-256.
const HASH_BUF: usize = 1 << 20;

/// Options of `container-diff`.
pub struct DiffOptions<'a> {
    pub a: &'a str,
    pub b: &'a str,
    /// `--by inflated` (default: stored bytes).
    pub inflated: bool,
    pub owners: &'a [String],
    pub expect_owners: Option<&'a str>,
    pub json: bool,
    pub workers: usize,
}

fn usage_error(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_USAGE
}

fn print_json(v: &Value) {
    println!("{}", dumps_with(v, &PRETTY_SORTED).unwrap_or_default());
}

/// A failure of the scan: exit 1, an error object on stdout with `--json`, a line on stderr otherwise.
fn failure(e: &ExtractionError, json_output: bool) -> i32 {
    if json_output {
        let mut doc = json!({"status": "error", "exitCode": EXIT_FAILED, "code": e.code, "message": e.message});
        if !e.params.is_empty() {
            doc["details"] = Value::Object(e.params.clone());
        }
        print_json(&doc);
    } else {
        eprintln!("oneform: error: {}: {}", e.code, e.message);
    }
    EXIT_FAILED
}

fn io_error(path: &Path, e: &std::io::Error) -> ExtractionError {
    ExtractionError::new(IO_ERROR, format!("{}: {e}", path.display())).with("path", path.display().to_string())
}

/// SHA-256 of the whole file, streamed.
fn file_sha256(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| io_error(path, &e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; HASH_BUF];
    loop {
        let n = f.read(&mut buf).map_err(|e| io_error(path, &e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// A scanned file with its record facts.
struct Loaded {
    container: ScannedContainer,
    facts: Vec<EntryFacts>,
    version: Option<u32>,
    /// Header and compression warnings.
    warnings: Vec<ScanWarning>,
}

fn load(path: &str, workers: usize) -> Result<Loaded> {
    let container = open(Path::new(path))?;
    let facts = process_entries(&container, workers)?;
    let version = compatibility_version(&container)?;
    let mut warnings = container.warnings.clone();
    warnings.extend(warnings_of(&facts));
    Ok(Loaded { container, facts, version, warnings })
}

fn opt<T: Into<Value>>(v: Option<T>) -> Value {
    v.map(Into::into).unwrap_or(Value::Null)
}

fn warning_json(w: &ScanWarning, file: Option<&str>) -> Value {
    let mut v = json!({"code": w.code, "message": w.message});
    if let Some(e) = &w.entry {
        v["entry"] = json!(e);
    }
    if let Some(f) = file {
        v["file"] = json!(f);
    }
    v
}

fn is_file(path: &str) -> bool {
    Path::new(path).is_file()
}

// ------------------------------------------------------------------------------------------------ container-entries

/// `container-entries <FILE> [--json] [--strict] [-j N]`; the result is the process exit code:
/// 0 listed, 1 damaged container, 2 usage, 3 `--strict` and a warning.
pub fn run_entries(file: &str, json_output: bool, strict: bool, workers: usize) -> i32 {
    if !is_file(file) {
        return usage_error(&format!("{file} is not a file"));
    }
    let (sha, l) = match load(file, workers).and_then(|l| Ok((file_sha256(Path::new(file))?, l))) {
        Ok(v) => v,
        Err(e) => return failure(&e, json_output),
    };
    let c = &l.container;
    let code = if strict && !l.warnings.is_empty() { EXIT_DIFFERS } else { EXIT_OK };
    if json_output {
        let entries: Vec<Value> = l
            .facts
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
        print_json(&json!({
            "status": "ok",
            "exitCode": code,
            "file": file,
            "fileSize": c.file_size,
            "fileSha256": sha,
            "layout": c.layout.as_str(),
            "layoutVerified": c.layout_verified,
            "containerOffset": c.container_offset,
            "entryCount": c.entries.len(),
            "headerEntryCount": c.header_entry_count,
            "compatibilityVersion": opt(l.version),
            "entries": entries,
            "warnings": l.warnings.iter().map(|w| warning_json(w, None)).collect::<Vec<_>>(),
        }));
        return code;
    }
    println!("file: {file}");
    println!("size: {} bytes, sha256 {sha}", c.file_size);
    println!(
        "layout: {}{}, container offset {}",
        c.layout.as_str(),
        if c.layout_verified { "" } else { " (not verified)" },
        c.container_offset
    );
    println!("records: {} (file header says {})", c.entries.len(), c.header_entry_count);
    println!("compatibility version: {}", l.version.map_or("-".to_string(), |v| v.to_string()));
    println!("warnings: {}", l.warnings.len());
    for w in &l.warnings {
        println!("  {}: {}", w.code, w.message);
    }
    println!();
    println!("name  storedSize  storedSha256  compression  inflatedSize");
    for f in &l.facts {
        println!(
            "{}  {}  {}  {}  {}{}",
            f.name,
            f.stored_size,
            f.stored_sha256,
            f.compression.as_str(),
            f.inflated_size.map_or("-".to_string(), |n| n.to_string()),
            if f.nested { "  nested" } else { "" }
        );
    }
    code
}

// ------------------------------------------------------------------------------------------------ container-diff

/// The hash of the content of a record: inflated for `deflate`, stored for `none`, none for `invalid`.
fn content_hash(f: &EntryFacts) -> Option<&str> {
    match f.compression {
        Compression::Deflate => f.inflated_sha256.as_deref(),
        Compression::None => Some(&f.stored_sha256),
        Compression::Invalid => None,
    }
}

struct Item {
    name: String,
    content_same: Option<bool>,
    owner: Option<String>,
}

#[derive(Default)]
struct Counts {
    differ: usize,
    only_a: usize,
    only_b: usize,
}

struct Diff {
    a: Loaded,
    b: Loaded,
    same: usize,
    differ: Vec<Item>,
    only_a: Vec<Item>,
    only_b: Vec<Item>,
    /// `Some` only with `--owners`.
    by_owner: Option<BTreeMap<String, Counts>>,
    /// `Some` only with `--expect-owners`.
    unexpected: Option<Vec<String>>,
    /// `(file, warning)` of A, B, then the owner index.
    warnings: Vec<(Option<&'static str>, ScanWarning)>,
    code: i32,
}

fn invalid_error(name: &str, a: &Loaded, b: &Loaded, in_a: bool, in_b: bool) -> ExtractionError {
    let mut files = Vec::new();
    if in_a {
        files.push(format!("A ({})", a.container.path.display()));
    }
    if in_b {
        files.push(format!("B ({})", b.container.path.display()));
    }
    ExtractionError::new(
        CONTAINER_ENTRY_INVALID,
        format!(
            "record {name:?} in {} is neither a raw DEFLATE stream nor a container: --by inflated cannot compare it",
            files.join(" and ")
        ),
    )
    .with("name", name)
}

fn compute_diff(o: &DiffOptions, expected: Option<BTreeSet<String>>) -> Result<Diff> {
    let a = load(o.a, o.workers)?;
    let b = load(o.b, o.workers)?;
    let index = (!o.owners.is_empty()).then(|| {
        let dirs: Vec<PathBuf> = o.owners.iter().map(PathBuf::from).collect();
        OwnerIndex::build(&dirs, o.workers)
    });
    let owner = |name: &str| index.as_ref().map(|i| i.owner_of(name));
    let (mut same, mut differ, mut only_a, mut only_b) = (0usize, Vec::new(), Vec::new(), Vec::new());
    let (mut i, mut j) = (0usize, 0usize);
    // Both lists are sorted by name in byte order.
    while i < a.facts.len() || j < b.facts.len() {
        let order = match (a.facts.get(i), b.facts.get(j)) {
            (Some(x), Some(y)) => x.name.as_bytes().cmp(y.name.as_bytes()),
            (Some(_), None) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Greater,
        };
        match order {
            std::cmp::Ordering::Less => {
                let name = a.facts[i].name.clone();
                only_a.push(Item { owner: owner(&name), name, content_same: None });
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                let name = b.facts[j].name.clone();
                only_b.push(Item { owner: owner(&name), name, content_same: None });
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                let (x, y) = (&a.facts[i], &b.facts[j]);
                let (hx, hy) = (content_hash(x), content_hash(y));
                let is_same = if o.inflated {
                    let (bad_a, bad_b) = (hx.is_none(), hy.is_none());
                    if bad_a || bad_b {
                        return Err(invalid_error(&x.name, &a, &b, bad_a, bad_b));
                    }
                    hx == hy
                } else {
                    x.stored_sha256 == y.stored_sha256
                };
                if is_same {
                    same += 1;
                } else {
                    let content_same = match (hx, hy) {
                        (Some(p), Some(q)) => Some(p == q),
                        _ => None,
                    };
                    differ.push(Item { owner: owner(&x.name), name: x.name.clone(), content_same });
                }
                i += 1;
                j += 1;
            }
        }
    }
    let by_owner = index.as_ref().map(|_| {
        let mut m: BTreeMap<String, Counts> = BTreeMap::new();
        for (items, which) in [(&differ, 0), (&only_a, 1), (&only_b, 2)] {
            for it in items.iter() {
                let c = m.entry(it.owner.clone().unwrap_or_default()).or_default();
                match which {
                    0 => c.differ += 1,
                    1 => c.only_a += 1,
                    _ => c.only_b += 1,
                }
            }
        }
        m
    });
    let unexpected = expected.map(|set| {
        let seen: BTreeSet<&String> = differ.iter().chain(&only_a).chain(&only_b).filter_map(|it| it.owner.as_ref()).collect();
        seen.into_iter().filter(|o| !set.contains(*o)).cloned().collect::<Vec<String>>()
    });
    let any = !(differ.is_empty() && only_a.is_empty() && only_b.is_empty());
    let code = match &unexpected {
        _ if !any => EXIT_OK,
        Some(u) if u.is_empty() => EXIT_OK,
        _ => EXIT_DIFFERS,
    };
    let mut warnings: Vec<(Option<&'static str>, ScanWarning)> = Vec::new();
    for (side, l) in [("a", &a), ("b", &b)] {
        let label = side.to_ascii_uppercase();
        for w in &l.warnings {
            let mut w = w.clone();
            w.message = format!("{label}: {}", w.message);
            warnings.push((Some(if side == "a" { "a" } else { "b" }), w));
        }
    }
    if let Some(i) = &index {
        warnings.extend(i.warnings.iter().map(|w| (None, w.clone())));
    }
    Ok(Diff { a, b, same, differ, only_a, only_b, by_owner, unexpected, warnings, code })
}

fn side_json(l: &Loaded) -> Value {
    let c = &l.container;
    json!({
        "file": c.path.to_string_lossy(),
        "layout": c.layout.as_str(),
        "layoutVerified": c.layout_verified,
        "containerOffset": c.container_offset,
        "entryCount": c.entries.len(),
        "compatibilityVersion": opt(l.version),
    })
}

fn item_json(it: &Item, with_content: bool) -> Value {
    let mut v = Map::new();
    v.insert("name".into(), json!(it.name));
    if with_content {
        v.insert("contentSame".into(), opt(it.content_same));
    }
    if let Some(o) = &it.owner {
        v.insert("owner".into(), json!(o));
    }
    Value::Object(v)
}

fn diff_json(d: &Diff, inflated: bool) -> Value {
    let mut doc = json!({
        "status": "ok",
        "exitCode": d.code,
        "by": if inflated { "inflated" } else { "stored" },
        "a": side_json(&d.a),
        "b": side_json(&d.b),
        "counts": {
            "a": d.a.facts.len(), "b": d.b.facts.len(), "same": d.same,
            "differ": d.differ.len(), "onlyA": d.only_a.len(), "onlyB": d.only_b.len(),
        },
        "differ": d.differ.iter().map(|i| item_json(i, true)).collect::<Vec<_>>(),
        "onlyA": d.only_a.iter().map(|i| item_json(i, false)).collect::<Vec<_>>(),
        "onlyB": d.only_b.iter().map(|i| item_json(i, false)).collect::<Vec<_>>(),
        "warnings": d.warnings.iter().map(|(f, w)| warning_json(w, *f)).collect::<Vec<_>>(),
    });
    if let Some(m) = &d.by_owner {
        let obj: Map<String, Value> = m
            .iter()
            .map(|(k, c)| (k.clone(), json!({"differ": c.differ, "onlyA": c.only_a, "onlyB": c.only_b})))
            .collect();
        doc["byOwner"] = Value::Object(obj);
    }
    if let Some(u) = &d.unexpected {
        doc["unexpectedOwners"] = json!(u);
    }
    doc
}

fn side_text(label: &str, l: &Loaded) -> String {
    let c = &l.container;
    format!(
        "{label}: {} ({}{}, offset {}, {} records, compatibility version {})",
        c.path.display(),
        c.layout.as_str(),
        if c.layout_verified { "" } else { ", not verified" },
        c.container_offset,
        c.entries.len(),
        l.version.map_or("-".to_string(), |v| v.to_string())
    )
}

fn print_diff_text(d: &Diff, inflated: bool) {
    println!("{}", side_text("A", &d.a));
    println!("{}", side_text("B", &d.b));
    println!("by: {}", if inflated { "inflated" } else { "stored" });
    println!(
        "counts: a={} b={} same={} differ={} onlyA={} onlyB={}",
        d.a.facts.len(),
        d.b.facts.len(),
        d.same,
        d.differ.len(),
        d.only_a.len(),
        d.only_b.len()
    );
    let owner = |it: &Item| it.owner.as_ref().map(|o| format!("  owner={o}")).unwrap_or_default();
    for it in &d.differ {
        let content = match it.content_same {
            Some(true) => "same",
            Some(false) => "differs",
            None => "unknown",
        };
        println!("differ  {}  content={content}{}", it.name, owner(it));
    }
    for it in &d.only_a {
        println!("onlyA  {}{}", it.name, owner(it));
    }
    for it in &d.only_b {
        println!("onlyB  {}{}", it.name, owner(it));
    }
    if let Some(m) = &d.by_owner {
        println!("by owner:");
        for (k, c) in m {
            println!("  {k}  differ={} onlyA={} onlyB={}", c.differ, c.only_a, c.only_b);
        }
    }
    if let Some(u) = &d.unexpected {
        println!("unexpected owners: {}", u.len());
        for o in u {
            println!("  {o}");
        }
    }
    println!("warnings: {}", d.warnings.len());
    for (_, w) in &d.warnings {
        println!("  {}: {}", w.code, w.message);
    }
}

/// `container-diff <A> <B> ...`; the result is the process exit code: 0 identical (or only expected owners differ),
/// 1 damaged container or `CONTAINER_ENTRY_INVALID`, 2 usage, 3 differences.
pub fn run_diff(o: &DiffOptions) -> i32 {
    for f in [o.a, o.b] {
        if !is_file(f) {
            return usage_error(&format!("{f} is not a file"));
        }
    }
    for d in o.owners {
        if !Path::new(d).is_dir() {
            return usage_error(&format!("--owners {d} is not a directory"));
        }
    }
    let expected = match o.expect_owners {
        None => None,
        Some(_) if o.owners.is_empty() => return usage_error("--expect-owners needs at least one --owners <dump directory>"),
        Some(f) => match std::fs::read(f).map_err(|e| e.to_string()).and_then(|b| parse_expected(&b)) {
            Ok(set) => Some(set),
            Err(e) => return usage_error(&format!("--expect-owners {f}: {e}")),
        },
    };
    match compute_diff(o, expected) {
        Ok(d) => {
            if o.json {
                print_json(&diff_json(&d, o.inflated));
            } else {
                print_diff_text(&d, o.inflated);
            }
            d.code
        }
        Err(e) => failure(&e, o.json),
    }
}
