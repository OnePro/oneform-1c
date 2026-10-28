//! Automatic history of `pack`: before a `pack` publishes a change to a form, the previous binary (`Form.bin` of a
//! configuration form or the `.epf`), the previous and the edited projection files and a manifest are saved in
//! `<object directory>/.oneform/history/<YYYYMMDD-HHMMSS>-<form>/`. `oneform history` lists the snapshots,
//! `oneform history restore <id>` puts a saved binary back (after saving the current state as a new snapshot).
//! Nothing here ever writes into `src/`; a snapshot failure never fails the pack that asked for it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::canonical::sha256_hex;
use crate::cli_epf::{clean_path, EXIT_FAILED, EXIT_OK, EXIT_USAGE};
use crate::cli_oneform::{discover_targets, path_of, run_target, Kind, Options};
use crate::formbin::{form_bin_rel, walk_dirs, ConfigObject, ConfigPackPlan, Sources};
use crate::manifest::MANIFEST_PATH;
use crate::pack::{atomic_write, PackPlan};
use crate::transaction::{check_ownership, read_tree, Snapshot, Tree};

pub const KEEP_ENV: &str = "ONEFORM_HISTORY_KEEP";
pub const DEFAULT_KEEP: usize = 20;
/// A previous binary above this size is not copied (the snapshot is skipped with a warning).
pub const MAX_BINARY_BYTES: usize = 200 << 20;
const MANIFEST_NAME: &str = "manifest.json";
const SKIP_DIRS: [&str; 5] = [".git", ".venv", ".extract-work", ".oneform", "src"];

/// `<object directory>/.oneform/history`.
pub fn history_dir(object_dir: &Path) -> PathBuf {
    object_dir.join(".oneform").join("history")
}

/// How many snapshots to keep per object directory: `ONEFORM_HISTORY_KEEP` (0 = history off), default 20.
pub fn keep_limit() -> usize {
    std::env::var(KEEP_ENV).ok().and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(DEFAULT_KEEP)
}

/// What a snapshot stores.
#[derive(Debug, Clone)]
pub struct Entry {
    /// `pack` | `restore`
    pub reason: &'static str,
    pub form: String,
    /// `config-form` | `epf`
    pub kind: &'static str,
    /// Path of the saved binary relative to the object directory (`Forms/<Form>/Ext/Form.bin`, `<name>.epf`).
    pub binary: String,
    /// `src`-relative directory of the projection of the form (config forms), if known.
    pub projection: Option<String>,
    /// Previous binary; saved as `before/<file name of binary>`.
    pub before: Vec<u8>,
    pub after_sha256: String,
    pub before_files: Vec<(String, Vec<u8>)>,
    pub after_files: Vec<(String, Vec<u8>)>,
    pub summary: Vec<String>,
    pub warnings: Vec<String>,
}

// --------------------------------------------------------------------------- time

fn civil(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(windows)]
fn local_offset_secs(unix: i64) -> i64 {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        weekday: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }
    extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut st = SystemTime::default();
    // SAFETY: `GetLocalTime` only fills the caller-provided structure of the documented layout.
    unsafe { GetLocalTime(&mut st) };
    let (y, m, d) = (i64::from(st.year), i64::from(st.month), i64::from(st.day));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    let local = days * 86400 + i64::from(st.hour) * 3600 + i64::from(st.minute) * 60 + i64::from(st.second);
    // the two clocks were read a moment apart: round to a quarter of an hour
    ((local - unix) as f64 / 900.0).round() as i64 * 900
}

#[cfg(not(windows))]
fn local_offset_secs(_unix: i64) -> i64 {
    0
}

/// Seconds since the epoch of the local wall clock (UTC where the local zone is not available).
pub(crate) fn local_secs() -> i64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    unix + local_offset_secs(unix)
}

pub(crate) fn stamp(secs: i64) -> String {
    let (y, m, d, h, mi, s) = civil(secs);
    format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}")
}

pub(crate) fn time_text(secs: i64) -> String {
    let (y, m, d, h, mi, s) = civil(secs);
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

// --------------------------------------------------------------------------- snapshots

fn file_component(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let s = s.trim_matches(|c| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        "form".to_string()
    } else {
        s
    }
}

/// `YYYYMMDD-HHMMSS-<name>`: the only names this module creates (and prunes).
fn is_snapshot_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 16
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'-'
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_snapshot(partial: &Path, id: &str, secs: i64, e: &Entry) -> Result<(), String> {
    let before_name = Path::new(&e.binary).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    write_file(&partial.join("before").join(&before_name), &e.before)?;
    for (name, data) in &e.before_files {
        write_file(&partial.join("before").join(name), data)?;
    }
    for (name, data) in &e.after_files {
        write_file(&partial.join("after").join(name), data)?;
    }
    let manifest = json!({
        "schemaVersion": 1,
        "id": id,
        "time": time_text(secs),
        "unixTime": secs,
        "reason": e.reason,
        "form": e.form,
        "kind": e.kind,
        "binary": e.binary,
        "projection": e.projection,
        "beforeBinary": before_name,
        "beforeSha256": sha256_hex(&e.before),
        "afterSha256": e.after_sha256,
        "summary": e.summary,
        "warnings": e.warnings,
        "oneformVersion": env!("CARGO_PKG_VERSION"),
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?;
    write_file(&partial.join(MANIFEST_NAME), text.as_bytes())
}

/// Save one snapshot under `object_dir`, then drop the oldest ones beyond `keep`; returns the snapshot id.
pub fn save(object_dir: &Path, e: &Entry, keep: usize) -> Result<String, String> {
    if e.before.len() > MAX_BINARY_BYTES {
        return Err(format!(
            "the previous {} is {} MB, over the {} MB limit",
            e.binary,
            e.before.len() >> 20,
            MAX_BINARY_BYTES >> 20
        ));
    }
    let dir = history_dir(object_dir);
    fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut secs = local_secs();
    let form = file_component(&e.form);
    let id = loop {
        let id = format!("{}-{form}", stamp(secs));
        if !dir.join(&id).exists() {
            break id;
        }
        secs += 1;
    };
    let partial = dir.join(format!(".{id}.partial"));
    let _ = fs::remove_dir_all(&partial);
    let written = write_snapshot(&partial, &id, secs, e)
        .and_then(|()| fs::rename(&partial, dir.join(&id)).map_err(|err| format!("{}: {err}", partial.display())));
    if let Err(err) = written {
        let _ = fs::remove_dir_all(&partial);
        return Err(err);
    }
    prune(&dir, keep);
    Ok(id)
}

/// Snapshot for `pack`: `Ok(None)` when history is off (`ONEFORM_HISTORY_KEEP=0`).
fn save_for_pack(object_dir: &Path, e: &Entry) -> Result<Option<String>, String> {
    match keep_limit() {
        0 => Ok(None),
        keep => save(object_dir, e, keep).map(Some),
    }
}

/// Output lines of a snapshot attempt: `history <id>`, or a warning (a pack is never failed by its history).
fn note(result: Result<Option<String>, String>) -> (Option<String>, Vec<String>) {
    match result {
        Ok(Some(id)) => (Some(id.clone()), vec![format!("  history {id}")]),
        Ok(None) => (None, Vec::new()),
        Err(m) => (None, vec![format!("  warning: history snapshot not saved: {m}")]),
    }
}

/// Remove a snapshot written for a pack whose publication then failed.
pub fn discard(object_dir: &Path, id: &str) {
    if is_snapshot_name(id) {
        let _ = fs::remove_dir_all(history_dir(object_dir).join(id));
    }
}

/// A listed snapshot.
#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub path: PathBuf,
    pub manifest: Value,
}

/// Snapshots of one history directory, oldest first (only directories of this module's naming that hold a manifest).
pub fn list(dir: &Path) -> Vec<Item> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut items: Vec<Item> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if !is_snapshot_name(&id) || !path.is_dir() {
                return None;
            }
            let manifest = serde_json::from_slice::<Value>(&fs::read(path.join(MANIFEST_NAME)).ok()?).ok()?;
            Some(Item { id, path, manifest })
        })
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items
}

/// Keep the newest `keep` snapshots; only directories created by this module are removed.
pub fn prune(dir: &Path, keep: usize) {
    let items = list(dir);
    let excess = items.len().saturating_sub(keep);
    for item in items.into_iter().take(excess) {
        let _ = fs::remove_dir_all(&item.path);
    }
}

// --------------------------------------------------------------------------- pack hooks

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn dirname(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

type Files = Vec<(String, Vec<u8>)>;

/// The projection files to keep for the `packed` paths: each packed file and the `Form.xml` / `FormModule.bsl` next to it,
/// stored flat by file name (`<dir>__<name>` when two directories give the same name).
fn projection_files(packed: &[String], before: &Tree, after: &Tree) -> (Files, Files) {
    let mut paths: BTreeSet<String> = BTreeSet::new();
    for p in packed {
        paths.insert(p.clone());
        let dir = dirname(p);
        for name in ["Form.xml", "FormModule.bsl"] {
            let sibling = if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") };
            if before.contains_key(&sibling) || after.contains_key(&sibling) {
                paths.insert(sibling);
            }
        }
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let flat = paths.iter().any(|p| !seen.insert(basename(p)));
    let name_of = |p: &str| if flat { p.replace('/', "__") } else { basename(p).to_string() };
    let pick = |tree: &Tree| -> Files { paths.iter().filter_map(|p| tree.get(p).map(|d| (name_of(p), d.clone()))).collect() };
    (pick(before), pick(after))
}

fn split_notes(lines: impl Iterator<Item = String>) -> (Vec<String>, Vec<String>) {
    let (mut summary, mut warnings) = (Vec::new(), Vec::new());
    for l in lines {
        let t = l.trim().to_string();
        if t.starts_with("warning") {
            warnings.push(t);
        } else {
            summary.push(t);
        }
    }
    (summary, warnings)
}

/// Snapshot of an `.epf` pack that is about to be published; the id (for [`discard`]) and the lines to print.
pub fn record_epf(epf_path: &Path, plan: &PackPlan, snap: &Snapshot) -> (Option<String>, Vec<String>) {
    let Some(dir) = epf_path.parent() else { return (None, Vec::new()) };
    let name = epf_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name.strip_suffix(".epf").unwrap_or(&name).to_string();
    let (before_files, after_files) = projection_files(&plan.packed_paths, &plan.baseline, &plan.existing);
    let (summary, warnings) =
        split_notes(plan.packed_paths.iter().map(|p| format!("pack {p}")).chain(plan.edit_notes.iter().cloned()));
    let entry = Entry {
        reason: "pack",
        form: stem,
        kind: "epf",
        binary: name,
        projection: None,
        before: snap.data.clone(),
        after_sha256: sha256_hex(&plan.new_epf),
        before_files,
        after_files,
        summary,
        warnings,
    };
    note(save_for_pack(dir, &entry))
}

/// Snapshots (one per changed form) of a configuration pack that is about to be published: ids and the lines to print.
pub fn record_config(object_dir: &Path, obj: &ConfigObject, plan: &ConfigPackPlan, before: &Sources) -> (Vec<String>, Vec<String>) {
    let manifest: Value = plan.result.files.get(MANIFEST_PATH).and_then(|b| serde_json::from_slice(b).ok()).unwrap_or(Value::Null);
    let (mut ids, mut lines) = (Vec::new(), Vec::new());
    for (form, new_bin) in &plan.new_sources {
        let Some(old_bin) = before.get(form) else { continue };
        let bin = form_bin_rel(obj, form);
        let rec = manifest["forms"].as_array().and_then(|forms| {
            forms.iter().find(|r| {
                let alias = r["alias"].as_str().unwrap_or("");
                manifest["source"]["forms"][alias]["binPath"].as_str() == Some(bin.as_str())
            })
        });
        let xml = rec.and_then(|r| r["layout"]["xml"].as_str()).unwrap_or("");
        let module = rec.and_then(|r| r["module"]["path"].as_str()).unwrap_or("");
        let projection = (!xml.is_empty()).then(|| dirname(xml).to_string());
        let own: Vec<String> = [xml, module].iter().filter(|p| !p.is_empty()).map(|p| (*p).to_string()).collect();
        let (before_files, after_files) = projection_files(&own, &plan.baseline, &plan.existing);
        let prefix = projection.as_ref().filter(|d| !d.is_empty()).map(|d| format!("{d}/"));
        let mine = |l: &String| prefix.as_ref().is_none_or(|p| l.contains(p.as_str()));
        let (summary, warnings) = split_notes(
            plan.packed_paths.iter().map(|p| format!("pack {p}")).chain(plan.edit_notes.iter().cloned()).filter(mine),
        );
        let entry = Entry {
            reason: "pack",
            form: form.clone(),
            kind: "config-form",
            binary: bin,
            projection,
            before: old_bin.clone(),
            after_sha256: sha256_hex(new_bin),
            before_files,
            after_files,
            summary,
            warnings,
        };
        let (id, l) = note(save_for_pack(object_dir, &entry));
        ids.extend(id);
        lines.extend(l);
    }
    (ids, lines)
}

// --------------------------------------------------------------------------- oneform history

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

fn summary_text(m: &Value) -> String {
    let all: Vec<&str> = m["summary"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    // the edit lines say more than the list of packed files
    let edits: Vec<&str> = all.iter().copied().filter(|l| !l.starts_with("pack ")).collect();
    let lines = if edits.is_empty() { all } else { edits };
    let mut text = lines.join("; ");
    if text.chars().count() > 110 {
        text = text.chars().take(107).collect::<String>() + "...";
    }
    text
}

pub(crate) fn rel_of(root: &Path, dir: &Path) -> String {
    let rel = dir
        .strip_prefix(root)
        .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
        .unwrap_or_default();
    if rel.is_empty() {
        ".".to_string()
    } else {
        rel
    }
}

/// Every directory below (and including) `root` that has `.oneform/history`, with its snapshots.
pub(crate) fn find_histories(root: &Path) -> Vec<(PathBuf, Vec<Item>)> {
    let mut found = Vec::new();
    let skip = |d: &str| SKIP_DIRS.contains(&d);
    walk_dirs(root, &skip, &mut |dir, _| {
        let items = list(&history_dir(dir));
        if !items.is_empty() {
            found.push((dir.to_path_buf(), items));
        }
    });
    found
}

/// The newest snapshot under `root` (by id, which starts with the time) and the object directory that holds it.
pub(crate) fn latest_under(root: &Path) -> Option<(PathBuf, Item)> {
    find_histories(root)
        .into_iter()
        .flat_map(|(dir, items)| items.into_iter().map(move |i| (dir.clone(), i)))
        .max_by(|a, b| a.1.id.cmp(&b.1.id))
}

/// The snapshot `id` (exact or a unique prefix) under `root`, or the newest one without an id.
pub(crate) fn resolve_snapshot(root: &Path, id: Option<&str>) -> Result<(PathBuf, Item), String> {
    let all: Vec<(PathBuf, Item)> = find_histories(root)
        .into_iter()
        .flat_map(|(dir, items)| items.into_iter().map(move |i| (dir.clone(), i)))
        .collect();
    let Some(id) = id else {
        return all.into_iter().max_by(|a, b| a.1.id.cmp(&b.1.id)).ok_or_else(|| {
            format!("no history under {} (pack writes .oneform/history next to the object)", root.display())
        });
    };
    let exact: Vec<&(PathBuf, Item)> = all.iter().filter(|(_, i)| i.id == id).collect();
    let hits: Vec<&(PathBuf, Item)> =
        if exact.is_empty() { all.iter().filter(|(_, i)| i.id.starts_with(id)).collect() } else { exact };
    match hits.as_slice() {
        [one] => Ok((one.0.clone(), one.1.clone())),
        [] => {
            let ids: Vec<&str> = all.iter().map(|(_, i)| i.id.as_str()).collect();
            let known = if ids.is_empty() { "no snapshots under this path".to_string() } else { format!("available: {}", ids.join(", ")) };
            Err(format!("no history snapshot '{id}' under {}; {known}", root.display()))
        }
        many => {
            let dirs: Vec<String> = many.iter().map(|(d, i)| format!("{} ({})", i.id, rel_of(root, d))).collect();
            Err(format!("snapshot id '{id}' is ambiguous: {}; point PATH at the object directory", dirs.join(", ")))
        }
    }
}

fn is_text_name(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".xml") || l.ends_with(".bsl")
}

fn read_text(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).trim_start_matches('\u{feff}').to_string())
}

/// Unified diff of the text files (`Form.xml`, `FormModule.bsl`, ...) of a snapshot: `before/` against `after/`.
pub(crate) fn snapshot_diff(item: &Item) -> Vec<String> {
    let names = |sub: &str| -> BTreeSet<String> {
        fs::read_dir(item.path.join(sub))
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| is_text_name(n)).collect())
            .unwrap_or_default()
    };
    let mut all: Vec<String> = names("before").union(&names("after")).cloned().collect();
    let rank = |n: &str| match n {
        "Form.xml" => 0,
        "FormModule.bsl" => 1,
        _ => 2,
    };
    all.sort_by(|a, b| (rank(a), a).cmp(&(rank(b), b)));
    let mut out = Vec::new();
    for name in all {
        let old = read_text(&item.path.join("before").join(&name));
        let new = read_text(&item.path.join("after").join(&name));
        out.extend(crate::textdiff::file_diff(&name, old.as_deref(), new.as_deref()));
    }
    out
}

fn diff_cmd(path: &Path, id: Option<&str>) -> i32 {
    let root = clean_path(path);
    if !root.is_dir() {
        return usage(&format!("{} is not a directory", root.display()));
    }
    let (dir, item) = match resolve_snapshot(&root, id) {
        Ok(v) => v,
        Err(m) => return failure(&m),
    };
    let m = &item.manifest;
    println!(
        "snapshot {} ({}, {}, {}): {}",
        item.id,
        m["form"].as_str().unwrap_or(""),
        rel_of(&root, &dir),
        m["time"].as_str().unwrap_or(""),
        m["reason"].as_str().unwrap_or("")
    );
    let lines = snapshot_diff(&item);
    if lines.is_empty() {
        println!("no differences in Form.xml / FormModule.bsl (the projection files before and after are the same)");
    }
    for l in lines {
        println!("{l}");
    }
    EXIT_OK
}

fn usage(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_USAGE
}

fn failure(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_FAILED
}

/// `oneform history [PATH]` and `oneform history restore [PATH] <id>`; `args` are the positionals after `history`.
pub fn run_cli(args: &[String], json: bool, dry_run: bool, replace_generated: bool) -> i32 {
    if args.first().map(String::as_str) == Some("diff") {
        if json || dry_run || replace_generated {
            return usage("oneform history diff takes no --json, --dry-run or --replace-generated");
        }
        return match &args[1..] {
            [] => diff_cmd(Path::new("."), None),
            [one] if Path::new(one).is_dir() => diff_cmd(Path::new(one), None),
            [one] => diff_cmd(Path::new("."), Some(one)),
            [path, id] => diff_cmd(Path::new(path), Some(id)),
            _ => usage("usage: oneform history diff [PATH] [id]"),
        };
    }
    if args.first().map(String::as_str) == Some("restore") {
        if json {
            return usage("--json is only valid for oneform history (the list)");
        }
        return match args.len() {
            2 => restore(Path::new("."), &args[1], dry_run, replace_generated),
            3 => restore(Path::new(&args[1]), &args[2], dry_run, replace_generated),
            _ => usage("usage: oneform history restore [PATH] <id> [--dry-run] [--replace-generated]"),
        };
    }
    if args.len() > 1 {
        return usage("usage: oneform history [PATH] [--json] | oneform history diff [PATH] [id] | oneform history restore [PATH] <id>");
    }
    if dry_run || replace_generated {
        return usage("--dry-run and --replace-generated are only valid for oneform history restore");
    }
    let root = clean_path(Path::new(args.first().map_or(".", String::as_str)));
    if !root.is_dir() {
        return usage(&format!("{} is not a directory", root.display()));
    }
    list_cmd(&root, json)
}

fn list_cmd(root: &Path, json: bool) -> i32 {
    let found = find_histories(root);
    if json {
        let mut out = Vec::new();
        for (dir, items) in &found {
            for item in items.iter().rev() {
                let m = &item.manifest;
                out.push(json!({
                    "id": item.id,
                    "objectDir": rel_of(root, dir),
                    "time": m["time"],
                    "reason": m["reason"],
                    "form": m["form"],
                    "kind": m["kind"],
                    "summary": m["summary"],
                    "warnings": m["warnings"],
                    "beforeSha256": m["beforeSha256"],
                    "afterSha256": m["afterSha256"],
                    "path": item.path.to_string_lossy(),
                }));
            }
        }
        println!("{}", serde_json::to_string_pretty(&Value::Array(out)).unwrap_or_default());
        return EXIT_OK;
    }
    if found.is_empty() {
        println!("{}: no history (pack writes .oneform/history next to the object)", root.display());
        return EXIT_OK;
    }
    for (dir, items) in &found {
        println!("{}: {} snapshot(s)", rel_of(root, dir), items.len());
        for item in items.iter().rev() {
            let m = &item.manifest;
            println!(
                "  {}  {}  {}  {}",
                item.id,
                m["time"].as_str().unwrap_or(""),
                m["reason"].as_str().unwrap_or(""),
                summary_text(m)
            );
        }
    }
    EXIT_OK
}

fn restore(path: &Path, id: &str, dry_run: bool, replace_generated: bool) -> i32 {
    let root = clean_path(path);
    if !root.is_dir() {
        return usage(&format!("{} is not a directory", root.display()));
    }
    let found = find_histories(&root);
    let mut hits: Vec<(&PathBuf, &Item)> = Vec::new();
    for (dir, items) in &found {
        let exact: Vec<&Item> = items.iter().filter(|i| i.id == id).collect();
        let chosen = if exact.is_empty() { items.iter().filter(|i| i.id.starts_with(id)).collect() } else { exact };
        hits.extend(chosen.into_iter().map(|i| (dir, i)));
    }
    let (obj_dir, item) = match hits.as_slice() {
        [one] => (one.0.clone(), one.1.clone()),
        [] => {
            let ids: Vec<String> = found.iter().flat_map(|(_, items)| items.iter().map(|i| i.id.clone())).collect();
            let known = if ids.is_empty() { "no snapshots under this path".to_string() } else { format!("available: {}", ids.join(", ")) };
            return failure(&format!("no history snapshot '{id}' under {}; {known}", root.display()));
        }
        many => {
            let dirs: Vec<String> = many.iter().map(|(d, i)| format!("{} ({})", i.id, rel_of(&root, d))).collect();
            return failure(&format!("snapshot id '{id}' is ambiguous: {}; point PATH at the object directory", dirs.join(", ")));
        }
    };
    match restore_item(&root, &obj_dir, &item, dry_run, replace_generated) {
        Ok(lines) => {
            for l in lines {
                println!("{l}");
            }
            EXIT_OK
        }
        Err(m) => {
            let record = crate::diagnostics::from_error("history restore", dry_run, EXIT_FAILED, None, None, &m);
            crate::diagnostics::append(&obj_dir, &record);
            failure(&m)
        }
    }
}

fn restore_item(root: &Path, obj_dir: &Path, item: &Item, dry_run: bool, replace_generated: bool) -> Result<Vec<String>, String> {
    let m = &item.manifest;
    let text = |k: &str| m[k].as_str().unwrap_or("").to_string();
    let (kind, form, binary) = (text("kind"), text("form"), text("binary"));
    let saved_path = item.path.join("before").join(text("beforeBinary"));
    let saved =
        fs::read(&saved_path).map_err(|e| format!("snapshot {} has no saved binary ({}): {e}", item.id, saved_path.display()))?;
    if sha256_hex(&saved) != text("beforeSha256") {
        return Err(format!("snapshot {}: the saved binary does not match its manifest (sha256)", item.id));
    }
    let mut bin_path = obj_dir.to_path_buf();
    bin_path.extend(binary.split('/'));
    let current = fs::read(&bin_path).map_err(|e| format!("{}: {e}", bin_path.display()))?;
    let label = rel_of(root, obj_dir);
    let (current_sha, saved_sha) = (sha256_hex(&current), sha256_hex(&saved));
    if current_sha == saved_sha {
        return Ok(vec![format!("{label}: UNCHANGED, {binary} already equals snapshot {}", item.id)]);
    }
    let target = discover_targets(root, "extract").into_iter().find(|t| {
        path_of(root, &t.rel) == obj_dir
            && match t.kind {
                Kind::Config => t.config.as_ref().is_some_and(|c| c.forms.contains(&form)),
                Kind::Epf => t.epf == binary,
                Kind::Xml => false,
            }
    });
    let Some(target) = target else {
        return Err(format!("{label}: no form '{form}' to restore (is the object still here?)"));
    };
    // the projection of the object (src next to the binary) is refreshed only when it exists
    let src_dir = path_of(root, if target.kind == Kind::Epf { &target.proc_rel } else { &target.rel }).join("src");
    let existing = if src_dir.join(MANIFEST_PATH).is_file() { Some(read_tree(&src_dir).map_err(|e| e.message)?) } else { None };
    if let Some(existing) = &existing {
        check_ownership(existing, replace_generated).map_err(|e| {
            let paths = e
                .params
                .get("paths")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
                .unwrap_or_default();
            format!("{label}: src/ has unpacked edits ({paths}); run oneform pack first, or restore with --replace-generated to drop them")
        })?;
    }
    let mut lines = Vec::new();
    if dry_run {
        lines.push(format!("{label}: WOULD RESTORE snapshot {} ({form}), {binary} {} -> {}", item.id, short(&current_sha), short(&saved_sha)));
        lines.push("  the current state would be saved as a new history snapshot first".to_string());
        if existing.is_some() {
            lines.push("  src/ would be refreshed with extract".to_string());
        }
        return Ok(lines);
    }
    let projection = m["projection"].as_str().filter(|_| existing.is_some()).map(str::to_string);
    let before_files: Files = match (&projection, &existing) {
        (Some(dir), Some(tree)) => ["Form.xml", "FormModule.bsl"]
            .iter()
            .filter_map(|n| tree.get(&format!("{dir}/{n}")).map(|d| ((*n).to_string(), d.clone())))
            .collect(),
        _ => Vec::new(),
    };
    let entry = Entry {
        reason: "restore",
        form: form.clone(),
        kind: if kind == "epf" { "epf" } else { "config-form" },
        binary: binary.clone(),
        projection,
        before: current,
        after_sha256: saved_sha.clone(),
        before_files,
        after_files: Vec::new(),
        summary: vec![format!("restore of {}", item.id)],
        warnings: Vec::new(),
    };
    // a restore stays reversible even when automatic history is off: the current state is always saved first
    let new_id = save(obj_dir, &entry, keep_limit().max(1))
        .map_err(|e| format!("cannot save the current state first, nothing restored: {e}"))?;
    if let Err(e) = atomic_write(&bin_path, &saved) {
        discard(obj_dir, &new_id);
        return Err(format!("{}: {}", bin_path.display(), e.message));
    }
    lines.push(format!("{label}: RESTORED snapshot {} ({form}), {binary} {} -> {}", item.id, short(&current_sha), short(&saved_sha)));
    lines.push(format!("  history {new_id} (the state before the restore)"));
    if existing.is_some() {
        let opts = Options { command: "extract".to_string(), dry_run: false, backup: false, replace_generated, oof_path: None };
        let out = run_target(&(opts, root.to_path_buf(), target));
        if out.status == "FAILED" {
            return Err(format!(
                "{binary} was restored, but refreshing src/ failed: {} {}; run oneform extract {}",
                out.code.unwrap_or(""),
                out.message,
                root.display()
            ));
        }
        lines.push(format!("  src/ refreshed ({})", out.status));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_793_179_935), "20261028-093215");
        assert_eq!(time_text(951_782_400 + 3661), "2000-02-29 01:01:01");
    }

    #[test]
    fn snapshot_names_and_pruning_only_touch_own_directories() {
        assert!(is_snapshot_name("20260101-120000-Форма"));
        assert!(!is_snapshot_name("20260101-120000"));
        assert!(!is_snapshot_name(".20260101-120000-Форма.partial"));
        assert!(!is_snapshot_name("notes-20260101-120000"));
        let t = tempfile::tempdir().unwrap();
        for i in 0..5 {
            let d = t.path().join(format!("2026010{i}-120000-F"));
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(MANIFEST_NAME), "{}").unwrap();
        }
        fs::create_dir_all(t.path().join("20250101-000000-nomanifest")).unwrap();
        fs::create_dir_all(t.path().join("mine")).unwrap();
        prune(t.path(), 2);
        let left: Vec<String> = list(t.path()).into_iter().map(|i| i.id).collect();
        assert_eq!(left, ["20260103-120000-F", "20260104-120000-F"]);
        assert!(t.path().join("20250101-000000-nomanifest").is_dir() && t.path().join("mine").is_dir());
    }

    #[test]
    fn file_components_are_safe() {
        assert_eq!(file_component("a/b:c"), "a_b_c");
        assert_eq!(file_component(".."), "form");
    }
}
