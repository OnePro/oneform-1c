//! `oneform export-forms`, the writer: safety of the output location, the mirror of a plan into a folder, `--check`.
//!
//! Ownership: only files named `Form.xml` / `FormModule.bsl` at the two layout depths (`<Type>/<Object>/Forms/<Form>/<file>`
//! and `CommonForms/<Name>/<file>`) may be removed, and the marker is the only other file written. Links and junctions are
//! never followed or removed. The order of `apply_export` is: remove stale files, prune the directories that became
//! empty, write changed files, write the marker last.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

use super::{ExportPlan, FORM_MODULE_FILE, FORM_XML_FILE};
use crate::canonical::{dumps_with, JsonOpts};
use crate::fsretry;
use crate::oof::ORDINARY_FORM_VERSION;

/// Name of the marker file in the root of the output folder.
pub const MARKER_FILE: &str = ".oneform-export.json";

/// The deepest level at which a `Form.bin` is looked for when deciding that a folder is a dump.
const DUMP_PROBE_DEPTH: usize = 7;

/// The output location is unsafe (the CLI maps it to exit code 2). Nothing was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRefusal {
    pub message: String,
}

impl fmt::Display for ExportRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExportRefusal {}

/// An I/O error while writing the output (the CLI maps it to exit code 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError {
    pub message: String,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExportError {}

fn io_err(what: &str, path: &Path, e: &io::Error) -> ExportError {
    ExportError { message: format!("{what} {}: {e}", path.display()) }
}

/// What `apply_export` did. Paths are relative to the output folder, `/` separated, sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Files created or rewritten (the marker included).
    pub written: Vec<String>,
    /// Stale owned files removed.
    pub removed: Vec<String>,
    /// Files already equal to the plan (the marker included).
    pub unchanged: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DifferenceKind {
    Added,
    Changed,
    Removed,
}

/// A path where the output folder differs from what `apply_export` would leave.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Difference {
    pub path: String,
    pub kind: DifferenceKind,
}

// ---------------------------------------------------------------------------------------------------------------------
// Links and path comparison
// ---------------------------------------------------------------------------------------------------------------------

/// A symbolic link, or on Windows any reparse point (a junction included).
fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// `canonicalize` without the Windows verbatim prefix (`\\?\C:\x` -> `C:\x`, `\\?\UNC\s\x` -> `\\s\x`).
fn canonical(path: &Path) -> io::Result<PathBuf> {
    let c = fs::canonicalize(path)?;
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return Ok(PathBuf::from(format!(r"\\{rest}")));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            let b = rest.as_bytes();
            if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
                return Ok(PathBuf::from(rest));
            }
        }
    }
    Ok(c)
}

/// An absolute path in which every existing part is resolved (links included) and the rest is appended lexically, so
/// that a folder that does not exist yet can be compared.
fn resolve(path: &Path) -> io::Result<PathBuf> {
    let abs = std::path::absolute(path)?;
    let mut done = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => done.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                done.pop();
            }
            Component::Normal(n) => {
                done.push(n);
                if fs::symlink_metadata(&done).is_ok() {
                    done = canonical(&done)?;
                }
            }
        }
    }
    Ok(done)
}

/// The key under which two paths are compared: Windows (and macOS) file systems are case-insensitive by default.
fn fold(p: &Path) -> PathBuf {
    if cfg!(any(windows, target_os = "macos")) {
        PathBuf::from(p.to_string_lossy().to_lowercase())
    } else {
        p.to_path_buf()
    }
}

/// `dump` is a configuration dump that is only read; `out` is where the forms go. Refuse when:
/// `out` is the dump, inside it or contains it (compared after resolving links, case-insensitively on Windows);
/// `out` is a file; `out` is a non-empty folder without the marker; `out` looks like a dump (`Configuration.xml` in its
/// root or a `Form.bin` below it). A folder that does not exist yet and an empty one are fine.
pub fn check_out_location(dump: &Path, out: &Path) -> Result<(), ExportRefusal> {
    let refuse = |m: String| Err(ExportRefusal { message: m });
    let (d, o) = match (resolve(dump), resolve(out)) {
        (Ok(d), Ok(o)) => (d, o),
        (Err(e), _) => return refuse(format!("cannot resolve the dump path {}: {e}", dump.display())),
        (_, Err(e)) => return refuse(format!("cannot resolve the output path {}: {e}", out.display())),
    };
    let (df, of) = (fold(&d), fold(&o));
    if df == of {
        return refuse(format!("the output folder {} is the dump", out.display()));
    }
    if of.starts_with(&df) {
        return refuse(format!("the output folder {} is inside the dump {}", out.display(), dump.display()));
    }
    if df.starts_with(&of) {
        return refuse(format!("the output folder {} contains the dump {}", out.display(), dump.display()));
    }
    let meta = match fs::metadata(&o) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return refuse(format!("cannot read the output folder {}: {e}", out.display())),
    };
    if !meta.is_dir() {
        return refuse(format!("the output path {} is a file, not a folder", out.display()));
    }
    if o.join("Configuration.xml").exists() {
        return refuse(format!("the output folder {} contains Configuration.xml: it looks like a dump", out.display()));
    }
    match contains_form_bin(&o, 0) {
        Ok(false) => {}
        Ok(true) => return refuse(format!("the output folder {} contains a Form.bin: it looks like a dump", out.display())),
        Err(e) => return refuse(format!("cannot read the output folder {}: {e}", out.display())),
    }
    let marker = fs::symlink_metadata(o.join(MARKER_FILE)).is_ok_and(|m| m.is_file() && !is_link(&m));
    if marker {
        return Ok(());
    }
    match fs::read_dir(&o) {
        Ok(mut it) => {
            if it.next().is_some() {
                return refuse(format!("the output folder {} is not empty and has no {MARKER_FILE}", out.display()));
            }
            Ok(())
        }
        Err(e) => refuse(format!("cannot read the output folder {}: {e}", out.display())),
    }
}

/// Is there a file named `Form.bin` in `dir` or in a real subfolder, at most `DUMP_PROBE_DEPTH` levels deep.
fn contains_form_bin(dir: &Path, depth: usize) -> io::Result<bool> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if depth < DUMP_PROBE_DEPTH && contains_form_bin(&entry.path(), depth + 1)? {
                return Ok(true);
            }
        } else if entry.file_name() == "Form.bin" {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------------------------------------------------
// Marker
// ---------------------------------------------------------------------------------------------------------------------

fn marker_bytes(plan: &ExportPlan, version: &str) -> Vec<u8> {
    let mut without: Vec<(&str, &str)> =
        plan.report.without_projection.iter().map(|w| (w.path.as_str(), w.reason.as_str())).collect();
    without.sort();
    let without: Vec<Value> = without.into_iter().map(|(path, reason)| json!({"path": path, "reason": reason})).collect();
    let marker = json!({
        "schemaVersion": 1,
        "tool": "oneform",
        "oneformVersion": version,
        "ordinaryFormVersion": ORDINARY_FORM_VERSION,
        "forms": plan.form_count,
        "withoutProjection": without,
    });
    const OPTS: JsonOpts = JsonOpts { indent: Some(2), sort_keys: true, ensure_ascii: false, compact: false };
    let mut text = dumps_with(&marker, &OPTS).expect("marker has no floats");
    text.push('\n');
    text.into_bytes()
}

// ---------------------------------------------------------------------------------------------------------------------
// Scan and delta
// ---------------------------------------------------------------------------------------------------------------------

/// The owned files below `out` (relative `/` paths, the spelling on disk), found without following links.
fn scan_owned(out: &Path) -> io::Result<BTreeSet<String>> {
    fn walk(dir: &Path, parts: &mut Vec<String>, found: &mut BTreeSet<String>) -> io::Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(it) => it,
            Err(e) if e.kind() == io::ErrorKind::NotFound && parts.is_empty() => return Ok(()),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let meta = fs::symlink_metadata(entry.path())?;
            if is_link(&meta) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let depth = parts.len() + 1;
            if meta.is_dir() {
                // Folder levels: 1 `<Type>`, 2 `<Object>` / `<Name>`, 3 `Forms`, 4 `<Form>`.
                if depth <= 2 || (depth == 3 && name == "Forms") || depth == 4 {
                    parts.push(name);
                    walk(&entry.path(), parts, found)?;
                    parts.pop();
                }
            } else if meta.is_file() && (name == FORM_XML_FILE || name == FORM_MODULE_FILE) {
                let common = parts.len() == 2 && parts[0] == "CommonForms";
                let object = parts.len() == 4 && parts[2] == "Forms";
                if common || object {
                    found.insert(format!("{}/{name}", parts.join("/")));
                }
            }
        }
        Ok(())
    }
    let mut found = BTreeSet::new();
    walk(out, &mut Vec::new(), &mut found)?;
    Ok(found)
}

#[derive(Default)]
struct Delta {
    added: Vec<String>,
    changed: Vec<String>,
    removed: Vec<String>,
    unchanged: usize,
}

fn same_bytes(path: &Path, want: &[u8]) -> bool {
    match fs::metadata(path) {
        Ok(m) if m.len() == want.len() as u64 => fs::read(path).is_ok_and(|cur| cur == want),
        _ => false,
    }
}

/// Folder paths (lower case) of the forms that failed: their stale-looking files stay.
fn kept_dirs(plan: &ExportPlan) -> BTreeSet<String> {
    plan.report.failures.iter().map(|f| f.path.to_lowercase()).collect()
}

fn delta(out: &Path, plan: &ExportPlan, marker: &[u8]) -> io::Result<Delta> {
    let existing = scan_owned(out)?;
    let kept = kept_dirs(plan);
    let mut d = Delta::default();
    for (path, bytes) in &plan.tree {
        if !existing.contains(path) {
            d.added.push(path.clone());
        } else if same_bytes(&out.join(path), bytes) {
            d.unchanged += 1;
        } else {
            d.changed.push(path.clone());
        }
    }
    for path in &existing {
        if plan.tree.contains_key(path) {
            continue;
        }
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir).to_lowercase();
        if !kept.contains(&dir) {
            d.removed.push(path.clone());
        }
    }
    let marker_path = out.join(MARKER_FILE);
    match fs::symlink_metadata(&marker_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => d.added.push(MARKER_FILE.to_string()),
        Err(e) => return Err(e),
        Ok(m) if m.is_file() && !is_link(&m) && same_bytes(&marker_path, marker) => d.unchanged += 1,
        Ok(_) => d.changed.push(MARKER_FILE.to_string()),
    }
    Ok(d)
}

/// What `apply_export` would add, change and remove (the marker included); nothing is written. Sorted by path.
/// An output folder that cannot be read is reported as one `Changed` difference at `.`.
pub fn check_export(out: &Path, plan: &ExportPlan, version: &str) -> Vec<Difference> {
    let marker = marker_bytes(plan, version);
    let d = match delta(out, plan, &marker) {
        Ok(d) => d,
        Err(_) => return vec![Difference { path: ".".into(), kind: DifferenceKind::Changed }],
    };
    let mut all: Vec<Difference> = Vec::new();
    all.extend(d.added.into_iter().map(|path| Difference { path, kind: DifferenceKind::Added }));
    all.extend(d.changed.into_iter().map(|path| Difference { path, kind: DifferenceKind::Changed }));
    all.extend(d.removed.into_iter().map(|path| Difference { path, kind: DifferenceKind::Removed }));
    all.sort();
    all
}

// ---------------------------------------------------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------------------------------------------------

fn target(out: &Path, rel: &str) -> PathBuf {
    let mut p = out.to_path_buf();
    p.extend(rel.split('/'));
    p
}

/// Create the folders above `rel` inside `out`, refusing to go through a link or a file.
fn ensure_parent(out: &Path, rel: &str) -> Result<(), ExportError> {
    let mut cur = out.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        cur.push(part);
        match fs::symlink_metadata(&cur) {
            Ok(m) if is_link(&m) => {
                return Err(ExportError { message: format!("{} is a link; the export does not write through links", cur.display()) })
            }
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(ExportError { message: format!("{} is a file, a folder is needed", cur.display()) }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fsretry::create_dir(&cur).map_err(|e| io_err("cannot create", &cur, &e))?;
            }
            Err(e) => return Err(io_err("cannot read", &cur, &e)),
        }
    }
    Ok(())
}

/// A temporary sibling, then a rename over the target.
fn write_file(out: &Path, rel: &str, data: &[u8]) -> Result<(), ExportError> {
    ensure_parent(out, rel)?;
    let path = target(out, rel);
    if fs::symlink_metadata(&path).is_ok_and(|m| is_link(&m) || m.is_dir()) {
        return Err(ExportError { message: format!("{} is a link or a folder, a file is needed", path.display()) });
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}{}", fsretry::TMP_SUFFIX));
    fsretry::write(&tmp, data).map_err(|e| io_err("cannot write", &tmp, &e))?;
    fsretry::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_err("cannot replace", &path, &e)
    })
}

/// Remove the folders that the removal of `files` left empty, deepest first, never `out` and never a link.
fn prune_empty(out: &Path, files: &[String]) -> Result<(), ExportError> {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for f in files {
        let mut cur = f.as_str();
        while let Some((parent, _)) = cur.rsplit_once('/') {
            dirs.insert(parent.to_string());
            cur = parent;
        }
    }
    for dir in dirs.iter().rev() {
        let path = target(out, dir);
        let real = fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && !is_link(&m));
        let empty = real && fs::read_dir(&path).is_ok_and(|mut it| it.next().is_none());
        if empty {
            fsretry::remove_dir(&path).map_err(|e| io_err("cannot remove", &path, &e))?;
        }
    }
    Ok(())
}

/// Make `out` hold the plan: stale owned files (not those of a failed form) are removed first, so that a case-only rename
/// leaves the new spelling; emptied folders are pruned; new and changed files are written through a temporary sibling and
/// a rename, files that are already equal are not touched; the marker is written last. `version` is the `oneform` version
/// recorded in the marker. The result does not depend on the number of workers that built the plan.
pub fn apply_export(out: &Path, plan: &ExportPlan, version: &str) -> Result<ApplyReport, ExportError> {
    let marker = marker_bytes(plan, version);
    let d = delta(out, plan, &marker).map_err(|e| io_err("cannot read", out, &e))?;
    for rel in &d.removed {
        let path = target(out, rel);
        fsretry::remove_file(&path).map_err(|e| io_err("cannot remove", &path, &e))?;
    }
    prune_empty(out, &d.removed)?;
    fsretry::create_dir_all(out).map_err(|e| io_err("cannot create", out, &e))?;
    let mut written: Vec<String> = d.added.iter().chain(&d.changed).filter(|p| p.as_str() != MARKER_FILE).cloned().collect();
    written.sort();
    let tree: &BTreeMap<String, Vec<u8>> = &plan.tree;
    for rel in &written {
        write_file(out, rel, &tree[rel])?;
    }
    if d.added.iter().chain(&d.changed).any(|p| p == MARKER_FILE) {
        write_file(out, MARKER_FILE, &marker)?;
        written.push(MARKER_FILE.to_string());
        written.sort();
    }
    let mut removed = d.removed;
    removed.sort();
    Ok(ApplyReport { written, removed, unchanged: d.unchanged })
}
