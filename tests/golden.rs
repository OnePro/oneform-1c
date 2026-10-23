//! Golden harness: runs the `oneform` binary on the corpus produced by `scripts/gen_golden.py`
//! (the Python Reference) and compares exit code, stdout, stderr and the resulting tree byte for byte.
//!
//! Corpus contract: see the module docstring of `scripts/gen_golden.py`.
//! Env: `GOLDEN_FORCE=1` runs cases listed in `rust/golden-pending.txt` too; `GOLDEN_FILTER=substr` selects cases.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use oneform::container::{inflate, looks_like_container, parse_container, Limits};
use regex::Regex;
use serde_json::Value;

type Tree = BTreeMap<String, Vec<u8>>;

const ROOT_TOKEN: &str = "<ROOT>";

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn pending_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden-pending.txt")
}

fn read_tree(base: &Path) -> Tree {
    let mut out = Tree::new();
    fn walk(base: &Path, dir: &Path, out: &mut Tree) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if p.is_file() {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    walk(base, base, &mut out);
    out
}

fn write_tree(base: &Path, tree: &Tree) {
    for (rel, data) in tree {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }
}

struct Case {
    dir: PathBuf,
    cmd: Value,
}

impl Case {
    fn load(name: &str) -> Case {
        let dir = golden_dir().join(name);
        let cmd: Value = serde_json::from_slice(&fs::read(dir.join("cmd.json")).unwrap()).unwrap();
        Case { dir, cmd }
    }

    fn strings(&self, key: &str) -> Vec<String> {
        self.cmd
            .get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    }

    fn ignores(&self, what: &str) -> bool {
        self.strings("ignore").iter().any(|s| s == what)
    }

    fn base(&self) -> Option<String> {
        self.cmd.get("base").and_then(Value::as_str).map(str::to_string)
    }

    /// Input tree = (result tree of `base` minus `input_deleted`) overlaid with `input/`.
    fn input_tree(&self) -> Tree {
        let mut t = match self.base() {
            Some(b) => result_tree(&Case::load(&b)),
            None => Tree::new(),
        };
        for p in self.strings("input_deleted") {
            t.remove(&p);
        }
        t.extend(read_tree(&self.dir.join("input")));
        t
    }
}

/// Result tree of a case = input tree minus `deleted` overlaid with `expected/` (used as base of other cases).
fn result_tree(c: &Case) -> Tree {
    let mut t = c.input_tree();
    for p in c.strings("deleted") {
        t.remove(&p);
    }
    t.extend(read_tree(&c.dir.join("expected")));
    t
}

fn root_variants(root: &str) -> Vec<String> {
    let mut v = vec![root.to_string(), root.replace('\\', "/"), root.replace('\\', "\\\\")];
    v.sort_by_key(|s| std::cmp::Reverse(s.len()));
    v.dedup();
    v
}

fn normalize(text: &str, root: &str) -> String {
    let mut text = text.to_string();
    for v in root_variants(root) {
        text = text.replace(&v, ROOT_TOKEN);
    }
    let tail = Regex::new(&format!("{}[^\\s\"'),;]*", regex::escape(ROOT_TOKEN))).unwrap();
    tail.replace_all(&text, |m: &regex::Captures| m[0].replace('\\', "/")).into_owned()
}

fn mask_durations(tree: &mut Tree) {
    let re = regex::bytes::Regex::new(r#"("durationSeconds":\s*)[-+0-9.eE]+"#).unwrap();
    for (rel, data) in tree.iter_mut() {
        if rel.rsplit('/').next() == Some("report.json") {
            *data = re.replace_all(data, &b"${1}0"[..]).into_owned();
        }
    }
}

fn describe_diff(what: &str, want: &[u8], got: &[u8]) -> String {
    if let (Ok(w), Ok(g)) = (std::str::from_utf8(want), std::str::from_utf8(got)) {
        let diff = similar::TextDiff::from_lines(w, g);
        let text = diff.unified_diff().context_radius(2).header("expected", "actual").to_string();
        let mut lines: Vec<&str> = text.lines().take(40).collect();
        if text.lines().count() > 40 {
            lines.push("... (diff truncated)");
        }
        format!("{what}: text differs\n{}", lines.join("\n"))
    } else {
        let at = want.iter().zip(got).position(|(a, b)| a != b).unwrap_or(want.len().min(got.len()));
        format!("{what}: binary differs (expected {} bytes, actual {} bytes, first difference at {at})", want.len(), got.len())
    }
}

type EntryTuple = (String, Vec<u8>, Vec<u8>);

fn container_tree(raw: &[u8]) -> Option<Vec<EntryTuple>> {
    if !looks_like_container(raw) {
        return None;
    }
    let l = Limits::default();
    let c = parse_container(raw, &l, 0, None).ok()?;
    let mut out = Vec::new();
    for e in c.entries {
        let data = inflate(&e.raw, &l, None).unwrap_or_else(|_| e.raw.clone());
        let data = if let Some(sub) = container_tree(&data) {
            serde_json::to_vec(&sub).unwrap()
        } else {
            data
        };
        out.push((e.name, e.header, data));
    }
    Some(out)
}

fn mask_pack_meta(rel: &str, bytes: &[u8]) -> Vec<u8> {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if name == "ExtractionManifest.json" {
        if let Ok(mut v) = serde_json::from_slice::<Value>(bytes) {
            if let Some(s) = v.get_mut("source").and_then(Value::as_object_mut) {
                s.insert("size".into(), 0.into());
                s.insert("sha256".into(), "".into());
            }
            if let Some(files) = v.get_mut("files").and_then(Value::as_array_mut) {
                for f in files {
                    if f.get("role").and_then(Value::as_str) == Some("report") {
                        f["sha256"] = "".into();
                    }
                }
            }
            return serde_json::to_vec(&v).unwrap();
        }
    } else if name == "ExtractionReport.md" {
        let re = regex::bytes::Regex::new(r"\(sha256 `[0-9a-f]{64}`\)").unwrap();
        return re.replace_all(bytes, &b"(sha256 `<MASK>`)"[..]).into_owned();
    }
    bytes.to_vec()
}

/// Container files (*.epf, *.bin, *.bak) are compared by inflated entry content in `compare: pack` cases.
fn same_file(pack: bool, rel: &str, want: &[u8], got: &[u8]) -> bool {
    if want == got {
        return true;
    }
    if !pack {
        return false;
    }
    if [".epf", ".bin", ".bak"].iter().any(|s| rel.ends_with(s)) {
        if let (Some(a), Some(b)) = (container_tree(want), container_tree(got)) {
            return a == b;
        }
    }
    mask_pack_meta(rel, want) == mask_pack_meta(rel, got)
}

fn run_case(c: &Case) -> Result<(), String> {
    let work = tempfile::tempdir().unwrap();
    let work_path = work.path().canonicalize().unwrap();
    let root = work_path.to_string_lossy().trim_start_matches(r"\\?\").to_string();
    let input = c.input_tree();
    write_tree(Path::new(&root), &input);

    let argv = c.strings("argv");
    let out = Command::new(env!("CARGO_BIN_EXE_oneform"))
        .args(&argv)
        .current_dir(&root)
        .output()
        .map_err(|e| format!("cannot run binary: {e}"))?;

    let mut problems: Vec<String> = Vec::new();
    let want_code: i32 = fs::read_to_string(c.dir.join("exit_code")).unwrap().trim().parse().unwrap();
    let code = out.status.code().unwrap_or(-1);
    if code != want_code {
        problems.push(format!("exit code: expected {want_code}, actual {code}"));
    }
    let stdout = normalize(&String::from_utf8_lossy(&out.stdout), &root);
    let want_stdout = fs::read(c.dir.join("stdout.txt")).unwrap();
    if stdout.as_bytes() != want_stdout.as_slice() {
        problems.push(describe_diff("stdout", &want_stdout, stdout.as_bytes()));
    }
    if !c.ignores("stderr") {
        let stderr = normalize(&String::from_utf8_lossy(&out.stderr), &root);
        let want_stderr = fs::read(c.dir.join("stderr.txt")).unwrap();
        if stderr.as_bytes() != want_stderr.as_slice() {
            problems.push(describe_diff("stderr", &want_stderr, stderr.as_bytes()));
        }
    }

    let mut actual = read_tree(Path::new(&root));
    if c.ignores("durationSeconds") {
        mask_durations(&mut actual);
    }
    let only = c.strings("only");
    let pack = c.cmd.get("compare").and_then(Value::as_str) == Some("pack");
    let want_changed = read_tree(&c.dir.join("expected"));
    let got_changed: Tree = actual
        .iter()
        .filter(|(p, d)| input.get(*p) != Some(*d))
        .filter(|(p, _)| only.is_empty() || only.iter().any(|s| p.ends_with(s.as_str())))
        .map(|(p, d)| (p.clone(), d.clone()))
        .collect();
    let all: BTreeSet<&String> = want_changed.keys().chain(got_changed.keys()).collect();
    for p in all {
        match (want_changed.get(p), got_changed.get(p)) {
            (Some(w), Some(g)) => {
                if !same_file(pack, p, w, g) {
                    problems.push(describe_diff(&format!("file {p}"), w, g));
                }
            }
            (Some(_), None) => problems.push(format!("file {p}: expected but missing/unchanged")),
            (None, Some(_)) => problems.push(format!("file {p}: unexpected new/changed file")),
            (None, None) => {}
        }
    }
    let mut deleted: Vec<String> = input.keys().filter(|p| !actual.contains_key(*p)).cloned().collect();
    deleted.sort();
    let mut want_deleted = c.strings("deleted");
    want_deleted.sort();
    if deleted != want_deleted {
        problems.push(format!("deleted files: expected {want_deleted:?}, actual {deleted:?}"));
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn pending() -> BTreeSet<String> {
    fs::read_to_string(pending_file())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
fn golden_corpus() {
    let force = std::env::var("GOLDEN_FORCE").is_ok_and(|v| v == "1");
    let filter = std::env::var("GOLDEN_FILTER").unwrap_or_default();
    let pending = pending();
    let mut names: Vec<String> = fs::read_dir(golden_dir())
        .unwrap()
        .flatten()
        .filter(|e| e.path().join("cmd.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let (mut ran, mut skipped, mut failures) = (0, 0, Vec::new());
    for name in names.iter().filter(|n| n.contains(&filter)) {
        if pending.contains(name) && !force {
            skipped += 1;
            continue;
        }
        ran += 1;
        if let Err(e) = run_case(&Case::load(name)) {
            failures.push(format!("=== {name}\n{e}"));
        }
    }
    println!("golden: {ran} ran, {skipped} pending (skipped), {} failed", failures.len());
    if !failures.is_empty() {
        let shown: Vec<&String> = failures.iter().take(10).collect();
        panic!(
            "{} golden case(s) failed:\n{}",
            failures.len(),
            shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
        );
    }
}

#[test]
fn pending_cases_exist() {
    let dir = golden_dir();
    for name in pending() {
        assert!(dir.join(&name).join("cmd.json").is_file(), "golden-pending.txt lists unknown case {name}");
    }
}
