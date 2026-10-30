//! Per-processor lock, ownership checks and journaled `src/` publication (port of `transaction.py`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::canonical::{dumps_with, sha256_hex, ASCII_SORTED_PRETTY};
use crate::config::load_json_bytes;
use crate::discovery::{is_link, scan_dir, DirEnt};
use crate::fsretry;
use crate::errors::{
    ExtractionError, Result, GENERATED_FILE_MODIFIED, IO_ERROR, OUTPUT_NOT_OWNED, OUTPUT_SCHEMA_UNKNOWN,
    PROCESSOR_LOCKED, RECOVERY_REQUIRED, SOURCE_CHANGED,
};
use crate::manifest::MANIFEST_PATH;
use crate::naming::safe_join;

pub const WORK_NAME: &str = ".extract-work";
pub const JOURNAL_NAME: &str = "journal.json";
pub const PREPARED: &str = "prepared";
pub const SRC_MOVED: &str = "src-moved";
pub const IN_PLACE: &str = "in-place";
pub const COMMITTED: &str = "committed";

pub type Tree = BTreeMap<String, Vec<u8>>;

type RenameHook = Box<dyn FnMut(&Path, &Path) -> std::io::Result<()>>;
type VoidHook = Box<dyn FnMut()>;

thread_local! {
    static RENAME_HOOK: RefCell<Option<RenameHook>> = const { RefCell::new(None) };
    static BEFORE_PUBLISH: RefCell<Option<VoidHook>> = const { RefCell::new(None) };
}

/// Test hook: replaces `rename` for the current thread (`None` restores the real one).
pub fn set_rename_hook(hook: Option<RenameHook>) {
    RENAME_HOOK.with(|h| *h.borrow_mut() = hook);
}

/// Test hook: runs right before the final source re-check of `publish`.
pub fn set_before_publish_hook(hook: Option<VoidHook>) {
    BEFORE_PUBLISH.with(|h| *h.borrow_mut() = hook);
}

pub fn work_dir(root: &Path) -> PathBuf {
    root.join(WORK_NAME)
}

fn rename(src: &Path, dst: &Path) -> std::io::Result<()> {
    let hooked = RENAME_HOOK.with(|h| h.borrow_mut().as_mut().map(|f| f(src, dst)));
    match hooked {
        Some(r) => r,
        None => fsretry::rename(src, dst),
    }
}

fn before_publish() {
    BEFORE_PUBLISH.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f();
        }
    });
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

// --------------------------------------------------------------------------- lock

/// Exclusive non-blocking OS lock; released by the OS even after a crash.
pub struct ProcessorLock {
    file: File,
}

impl Drop for ProcessorLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

pub fn processor_lock(root: &Path, proc_rel: &str) -> Result<ProcessorLock> {
    let locks = work_dir(root).join("locks");
    fsretry::create_dir_all(&locks)?;
    let digest = sha256_hex(proc_rel.as_bytes());
    let mut file = OpenOptions::new().read(true).append(true).create(true).open(locks.join(format!("{}.lock", &digest[..16])))?;
    if file.metadata()?.len() == 0 {
        file.write_all(&[0])?;
        file.flush()?;
    }
    match file.try_lock() {
        Ok(()) => Ok(ProcessorLock { file }),
        Err(_) => Err(ExtractionError::new(PROCESSOR_LOCKED, "another run holds the processor lock").with("processor", proc_rel)),
    }
}

// ------------------------------------------------------------------------ source

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub data: Vec<u8>,
    pub size: usize,
    pub sha256: String,
}

pub fn snapshot_epf(path: &Path) -> Result<Snapshot> {
    let data = fs::read(path)?;
    let sha256 = sha256_hex(&data);
    Ok(Snapshot { size: data.len(), data, sha256 })
}

pub fn verify_source_unchanged(path: &Path, snap: &Snapshot) -> Result<()> {
    let now = fs::read(path)
        .map_err(|e| ExtractionError::new(SOURCE_CHANGED, "EPF can no longer be read").with("error", e.to_string()))?;
    // Byte comparison instead of re-hashing: the snapshot keeps the bytes whose digest is `snap.sha256`.
    let same = now.len() == snap.size
        && if snap.data.len() == snap.size { now == snap.data } else { sha256_hex(&now) == snap.sha256 };
    if !same {
        return Err(ExtractionError::new(SOURCE_CHANGED, "EPF changed during the run; run again").with("epf", name_of(path)));
    }
    Ok(())
}

// --------------------------------------------------------------------- ownership

fn walk_tree(base: &Path, dir: &Path, tree: &mut Tree) -> Result<()> {
    let (dirs, files): (Vec<DirEnt>, Vec<DirEnt>) = scan_dir(dir)?.into_iter().partition(|e| e.is_dir);
    for d in &dirs {
        if d.link {
            return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src contains a link").with("path", d.name.clone()));
        }
    }
    for f in &files {
        if f.link || !f.is_file {
            return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src contains a non-regular file").with("path", f.name.clone()));
        }
        let rel = f.path.strip_prefix(base).expect("under base").to_string_lossy().replace('\\', "/");
        tree.insert(rel, fs::read(&f.path)?);
    }
    for d in &dirs {
        walk_tree(base, &d.path, tree)?;
    }
    Ok(())
}

/// All files of `src` (relative POSIX path -> bytes); a missing `src` is empty.
pub fn read_tree(src: &Path) -> Result<Tree> {
    if fs::symlink_metadata(src).is_err() {
        return Ok(Tree::new());
    }
    if is_link(src) || !src.is_dir() {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src is not a plain directory"));
    }
    let mut tree = Tree::new();
    walk_tree(src, src, &mut tree)?;
    Ok(tree)
}

/// Owned path -> sha256 from the old manifest (without the manifest itself).
pub fn manifest_hashes(existing: &Tree) -> Result<BTreeMap<String, String>> {
    let Some(raw) = existing.get(MANIFEST_PATH) else {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src is not empty and has no ExtractionManifest.json"));
    };
    let manifest = load_json_bytes(raw, MANIFEST_PATH)
        .map_err(|_| ExtractionError::new(OUTPUT_NOT_OWNED, "ExtractionManifest.json is not valid JSON"))?;
    let Value::Object(m) = &manifest else {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "ExtractionManifest.json is not an object"));
    };
    match m.get("schemaVersion") {
        Some(v) if v.is_i64() && v.as_i64() == Some(1) => {}
        _ => return Err(ExtractionError::new(OUTPUT_SCHEMA_UNKNOWN, "unknown manifest schemaVersion; stopping")),
    }
    let Some(Value::Array(entries)) = m.get("files") else {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "manifest has no files list"));
    };
    let mut owned = BTreeMap::new();
    for entry in entries {
        let path = entry.get("path").and_then(Value::as_str);
        let digest = entry.get("sha256").and_then(Value::as_str);
        let (Some(path), Some(digest)) = (path, digest) else {
            return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "manifest file entry is malformed"));
        };
        let parts: Vec<&str> = path.split('/').collect();
        if safe_join(&parts)? != path {
            return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "manifest path is not canonical").with("path", path));
        }
        owned.insert(path.to_string(), digest.to_string());
    }
    Ok(owned)
}

pub fn check_ownership(existing: &Tree, replace_generated: bool) -> Result<()> {
    if existing.is_empty() {
        return Ok(());
    }
    let owned = manifest_hashes(existing)?;
    let unknown: Vec<Value> = existing
        .keys()
        .filter(|p| !owned.contains_key(*p) && p.as_str() != MANIFEST_PATH)
        .map(|p| Value::from(p.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src contains files not listed in the manifest")
            .with("paths", Value::Array(unknown)));
    }
    let modified: Vec<Value> = owned
        .iter()
        .filter(|(p, digest)| existing.get(*p).map(|d| sha256_hex(d) != **digest).unwrap_or(true))
        .map(|(p, _)| Value::from(p.as_str()))
        .collect();
    if !modified.is_empty() && !replace_generated {
        return Err(ExtractionError::new(GENERATED_FILE_MODIFIED, "generated files were modified or removed; use --replace-generated")
            .with("paths", Value::Array(modified)));
    }
    Ok(())
}

/// (added, updated, deleted) relative paths.
pub fn plan(existing: &Tree, new: &Tree) -> (Vec<String>, Vec<String>, Vec<String>) {
    let added = new.keys().filter(|p| !existing.contains_key(*p)).cloned().collect();
    let deleted = existing.keys().filter(|p| !new.contains_key(*p)).cloned().collect();
    let updated = new.iter().filter(|(p, d)| existing.get(*p).is_some_and(|o| o != *d)).map(|(p, _)| p.clone()).collect();
    (added, updated, deleted)
}

// ----------------------------------------------------------------------- journal

#[cfg(not(windows))]
fn fsync_dir(path: &Path) {
    if let Ok(f) = File::open(path) {
        let _ = f.sync_all();
    }
}

#[cfg(windows)]
fn fsync_dir(_path: &Path) {}

fn write_journal(run_dir: &Path, journal: &Value) -> Result<()> {
    let tmp = run_dir.join(format!("{JOURNAL_NAME}.tmp"));
    let text = dumps_with(journal, &ASCII_SORTED_PRETTY)?;
    fsretry::write(&tmp, text.as_bytes())?;
    fsretry::rename(&tmp, &run_dir.join(JOURNAL_NAME))?;
    fsync_dir(run_dir);
    Ok(())
}

fn clear_readonly(path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(path) {
        let mut perm = meta.permissions();
        if perm.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = fs::set_permissions(path, perm);
        }
        if meta.is_dir() {
            if let Ok(rd) = fs::read_dir(path) {
                for e in rd.flatten() {
                    clear_readonly(&e.path());
                }
            }
        }
    }
}

/// Remove a workspace run directory (never `src`).
fn rmtree(path: &Path) -> Result<()> {
    if fs::remove_dir_all(path).is_ok() {
        return Ok(());
    }
    clear_readonly(path);
    fsretry::remove_dir_all(path)?;
    Ok(())
}

fn manifest_sha(files: &Tree) -> Option<String> {
    files.get(MANIFEST_PATH).map(|d| sha256_hex(d))
}

fn is_valid_new(src: &Path, expected_manifest_sha: &str) -> bool {
    let Ok(existing) = read_tree(src) else { return false };
    manifest_sha(&existing).as_deref() == Some(expected_manifest_sha) && check_ownership(&existing, false).is_ok()
}

/// Resolve leftovers of crashed runs for one processor (call under its lock).
pub fn recover(root: &Path, proc_rel: &str) -> Result<Vec<String>> {
    let work = work_dir(root);
    let mut actions = Vec::new();
    if !work.is_dir() {
        return Ok(actions);
    }
    let mut src = root.to_path_buf();
    for part in proc_rel.split('/') {
        src.push(part);
    }
    src.push("src");
    let mut runs: Vec<PathBuf> = fs::read_dir(&work)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join(JOURNAL_NAME).is_file())
        .collect();
    runs.sort();
    for run_dir in runs {
        let run_name = name_of(&run_dir);
        let unreadable = || ExtractionError::new(RECOVERY_REQUIRED, "unreadable journal; resolve manually").with("run", run_name.clone());
        let mut journal: Value = fs::read(run_dir.join(JOURNAL_NAME))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or_else(unreadable)?;
        let (Some(state_v), Some(processor)) = (journal.get("state").cloned(), journal.get("processor").cloned()) else {
            return Err(unreadable());
        };
        if processor != proc_rel {
            continue;
        }
        let backup = run_dir.join("Backup");
        let stop = |why: &str| {
            ExtractionError::new(RECOVERY_REQUIRED, format!("{why}; nothing was deleted"))
                .with("run", run_name.clone())
                .with("state", state_v.clone())
        };
        match state_v.as_str() {
            Some(COMMITTED) => {}
            Some(PREPARED) => {
                if backup.exists() {
                    if fs::symlink_metadata(&src).is_ok() {
                        return Err(stop("both src and Backup exist"));
                    }
                    rename(&backup, &src)?;
                    actions.push(format!("restored src from {run_name}"));
                }
            }
            Some(SRC_MOVED) => {
                if fs::symlink_metadata(&src).is_err() {
                    if !backup.exists() {
                        return Err(stop("src and Backup are both missing"));
                    }
                    rename(&backup, &src)?;
                    actions.push(format!("restored src from {run_name}"));
                } else if journal.get("newManifestSha256").and_then(Value::as_str).is_some_and(|s| is_valid_new(&src, s)) {
                    journal["state"] = Value::from(COMMITTED);
                    write_journal(&run_dir, &journal)?;
                    actions.push(format!("completed commit of {run_name}"));
                } else {
                    return Err(stop("src is neither absent nor the expected new tree"));
                }
            }
            Some(IN_PLACE) => {
                if !backup.is_dir() {
                    return Err(stop("src was being updated in place and Backup is missing"));
                }
                if journal.get("newManifestSha256").and_then(Value::as_str).is_some_and(|s| is_valid_new(&src, s)) {
                    journal["state"] = Value::from(COMMITTED);
                    write_journal(&run_dir, &journal)?;
                    actions.push(format!("completed commit of {run_name}"));
                } else {
                    apply_in_place(&src, &read_tree(&backup)?).map_err(|e| stop(&format!("in-place restore failed: {}", e.message)))?;
                    actions.push(format!("restored src from {run_name} (in place)"));
                }
            }
            _ => return Err(stop("unknown journal state")),
        }
        rmtree(&run_dir)?;
    }
    Ok(actions)
}

// ---------------------------------------------------------------------- publish

#[derive(Debug, Clone)]
pub struct PublishResult {
    /// "UPDATED" | "UNCHANGED"
    pub status: &'static str,
    /// Set when the workspace was kept.
    pub run_dir: Option<PathBuf>,
}

pub fn write_tree(base: &Path, files: &Tree) -> Result<()> {
    fsretry::create_dir(base).or_else(|e| if base.is_dir() { Err(e) } else { fsretry::create_dir_all(base) })?;
    for (path, data) in files {
        let parts: Vec<&str> = path.split('/').collect();
        let joined = safe_join(&parts)?;
        let mut target = base.to_path_buf();
        for part in joined.split('/') {
            target.push(part);
        }
        if let Some(parent) = target.parent() {
            fsretry::create_dir_all(parent)?;
        }
        fsretry::write(&target, data)?;
    }
    Ok(())
}

fn token_hex8() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

fn io_failure(e: &std::io::Error) -> ExtractionError {
    ExtractionError::new(IO_ERROR, "could not move the new tree into place").with("error", e.to_string())
}

/// Update the files inside `src` when `src` itself cannot be renamed. The old tree is copied to `Backup` first (the journal
/// state `in-place` says that `Backup` is complete), then the files of `new_files` replace the files of `src` one by one,
/// the manifest last; the result is read back and compared. On any failure the old files are put back from the copy.
fn publish_in_place(
    run_dir: &Path,
    src: &Path,
    existing: &Tree,
    new_files: &Tree,
    journal: &mut Value,
    retain: &mut bool,
) -> Result<()> {
    let backup = run_dir.join("Backup");
    let partial = run_dir.join("Backup.partial");
    write_tree(&partial, existing)?;
    if &read_tree(&partial)? != existing {
        return Err(ExtractionError::new(IO_ERROR, "the backup copy of src differs from src"));
    }
    fsretry::rename(&partial, &backup)?;
    journal["state"] = Value::from(IN_PLACE);
    write_journal(run_dir, journal)?;
    *retain = true;
    let applied = apply_in_place(src, new_files);
    let Err(e) = applied else { return Ok(()) };
    if apply_in_place(src, existing).is_err() {
        return Err(ExtractionError::new(RECOVERY_REQUIRED, "publish and rollback failed; old tree is in the workspace")
            .with("run", name_of(run_dir))
            .with("error", e.message.clone()));
    }
    *retain = false;
    Err(e)
}

/// Make `src` hold exactly `tree` by replacing files inside it, and read it back to check.
fn apply_in_place(src: &Path, tree: &Tree) -> Result<()> {
    fsretry::sync_dir(src, tree, &|_| false, Some(MANIFEST_PATH))?;
    if &read_tree(src)? != tree {
        return Err(ExtractionError::new(IO_ERROR, "src does not hold the expected files after the in-place update"));
    }
    Ok(())
}

/// Replace `<processor>/src` with `new_files`; call under the processor lock.
pub fn publish(
    root: &Path,
    proc_rel: &str,
    new_files: &Tree,
    epf_path: &Path,
    snap: &Snapshot,
    keep_work: bool,
    replace_generated: bool,
) -> Result<PublishResult> {
    let mut src = root.to_path_buf();
    for part in proc_rel.split('/') {
        src.push(part);
    }
    src.push("src");
    let existing = read_tree(&src)?;
    if &existing == new_files {
        return Ok(PublishResult { status: "UNCHANGED", run_dir: None });
    }
    check_ownership(&existing, replace_generated)?;

    let run_dir = work_dir(root).join(token_hex8());
    let new_src = run_dir.join("NewSrc");
    let backup = run_dir.join("Backup");
    let mut retain = false; // true while the run dir may hold the only copy of the old tree

    let outcome = (|| -> Result<()> {
        fsretry::create_dir_all(&run_dir)?;
        write_tree(&new_src, new_files)?;
        before_publish();
        verify_source_unchanged(epf_path, snap)?;

        let mut journal = json!({
            "schemaVersion": 1,
            "processor": proc_rel,
            "state": PREPARED,
            "oldManifestSha256": manifest_sha(&existing),
            "newManifestSha256": manifest_sha(new_files),
        });
        write_journal(&run_dir, &journal)?;

        let had_src = fs::symlink_metadata(&src).is_ok();
        if had_src {
            retain = true;
            match rename(&src, &backup) {
                Ok(()) => {
                    journal["state"] = Value::from(SRC_MOVED);
                    write_journal(&run_dir, &journal)?;
                }
                // another process holds `src` itself (a file watcher): the files inside can still be replaced
                Err(e) if fsretry::is_transient(&e) => {
                    publish_in_place(&run_dir, &src, &existing, new_files, &mut journal, &mut retain).map_err(|err| {
                        if err.code == RECOVERY_REQUIRED { err } else { io_failure(&e).with("inPlaceError", err.message.clone()) }
                    })?;
                    journal["state"] = Value::from(COMMITTED);
                    write_journal(&run_dir, &journal)?;
                    retain = false;
                    eprintln!("oneform: warning: {proc_rel}: src updated in place (the directory is held by another process)");
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        }
        if let Err(e) = rename(&new_src, &src) {
            if had_src {
                if rename(&backup, &src).is_err() {
                    return Err(ExtractionError::new(RECOVERY_REQUIRED, "publish and rollback failed; old tree is in the workspace")
                        .with("run", name_of(&run_dir)));
                }
                retain = false;
            }
            return Err(io_failure(&e));
        }
        journal["state"] = Value::from(COMMITTED);
        write_journal(&run_dir, &journal)?;
        retain = false;
        Ok(())
    })();

    if run_dir.exists() && !retain && !keep_work {
        let _ = rmtree(&run_dir);
    }
    outcome?;
    Ok(PublishResult { status: "UPDATED", run_dir: if keep_work { Some(run_dir) } else { None } })
}

/// Read a whole file (helper for callers that want the same error mapping as the Reference).
pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    File::open(path)?.read_to_end(&mut v)?;
    Ok(v)
}

/// Paths (relative) of all files below `dir`, sorted; used by tests and the CLI plan output.
pub fn list_files(dir: &Path) -> Result<BTreeSet<String>> {
    Ok(read_tree(dir)?.into_keys().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(tag: &str) -> Tree {
        let mut t = Tree::new();
        t.insert("ObjectModule.bsl".into(), format!("module {tag}").into_bytes());
        t.insert("Forms/X/FormModule.bsl".into(), format!("form {tag}").into_bytes());
        let entries: Vec<Value> = t.iter().map(|(p, d)| json!({"path": p, "sha256": sha256_hex(d)})).collect();
        let manifest = json!({"schemaVersion": 1, "files": entries});
        t.insert(MANIFEST_PATH.into(), crate::canonical::dumps_json(&manifest).unwrap());
        t
    }

    struct Env {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        epf: PathBuf,
        src: PathBuf,
        snap: Snapshot,
    }

    const PROC: &str = "ExternalDataProcessors/S";

    fn env() -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let pdir = root.join("ExternalDataProcessors").join("S");
        fs::create_dir_all(&pdir).unwrap();
        let epf = pdir.join("Proc.epf");
        fs::write(&epf, b"epf-v1").unwrap();
        let snap = snapshot_epf(&epf).unwrap();
        Env { src: pdir.join("src"), _tmp: tmp, root, epf, snap }
    }

    impl Env {
        fn publish(&self, f: &Tree, replace: bool) -> Result<PublishResult> {
            publish(&self.root, PROC, f, &self.epf, &self.snap, false, replace)
        }

        fn runs(&self) -> Vec<PathBuf> {
            let w = work_dir(&self.root);
            if !w.exists() {
                return vec![];
            }
            fs::read_dir(w).unwrap().flatten().map(|e| e.path()).filter(|p| name_of(p) != "locks").collect()
        }

        fn crash_run(&self, state: &str, new: &Tree, name: &str, proc_rel: &str) -> PathBuf {
            let run = work_dir(&self.root).join(name);
            fs::create_dir_all(&run).unwrap();
            write_tree(&run.join("NewSrc"), new).unwrap();
            let j = json!({"processor": proc_rel, "state": state, "newManifestSha256": manifest_sha(new)});
            write_journal(&run, &j).unwrap();
            run
        }
    }

    #[test]
    fn publish_first_then_unchanged_then_update() {
        // port: tests/test_transaction.py::test_publish_first_then_unchanged_then_update
        let e = env();
        let (v1, v2) = (files("A"), files("B"));
        assert_eq!(e.publish(&v1, false).unwrap().status, "UPDATED");
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
        assert_eq!(e.publish(&v1, false).unwrap().status, "UNCHANGED");
        assert_eq!(e.publish(&v2, false).unwrap().status, "UPDATED");
        assert_eq!(read_tree(&e.src).unwrap(), v2);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn exact_bytes_written() {
        // port: tests/test_transaction.py::test_exact_bytes_written
        let e = env();
        let mut f = files("A");
        f.insert("Forms/X/blob.bin".into(), b"\r\n\x00\xff\r".to_vec());
        e.publish(&f, false).unwrap();
        assert_eq!(fs::read(e.src.join("Forms/X/blob.bin")).unwrap(), b"\r\n\x00\xff\r");
    }

    #[test]
    fn ownership_errors() {
        // port: tests/test_transaction.py::test_ownership_errors
        let e = env();
        e.publish(&files("A"), false).unwrap();
        let new = files("B");
        fs::write(e.src.join("extra.txt"), "x").unwrap();
        let err = e.publish(&new, true).unwrap_err();
        assert_eq!(err.code, OUTPUT_NOT_OWNED);
        assert_eq!(err.params["paths"], json!(["extra.txt"]));
        fs::remove_file(e.src.join("extra.txt")).unwrap();
        fs::write(e.src.join("ObjectModule.bsl"), b"edited").unwrap();
        assert_eq!(e.publish(&new, false).unwrap_err().code, GENERATED_FILE_MODIFIED);
        assert_eq!(read_tree(&e.src).unwrap()["ObjectModule.bsl"], b"edited");
        e.publish(&new, true).unwrap();
        assert_eq!(read_tree(&e.src).unwrap(), new);
    }

    #[test]
    fn unknown_schema_and_forged_paths() {
        // port: tests/test_transaction.py::test_unknown_schema_and_forged_paths
        let e = env();
        e.publish(&files("A"), false).unwrap();
        let new = files("B");
        let mp = e.src.join(MANIFEST_PATH);
        let mut m: Value = serde_json::from_slice(&fs::read(&mp).unwrap()).unwrap();
        m["schemaVersion"] = json!(2);
        fs::write(&mp, serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(e.publish(&new, true).unwrap_err().code, OUTPUT_SCHEMA_UNKNOWN);
        m["schemaVersion"] = json!(1);
        m["files"].as_array_mut().unwrap().push(json!({"path": "../outside.txt", "sha256": "0".repeat(64)}));
        fs::write(&mp, serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(e.publish(&new, true).unwrap_err().code, crate::errors::PATH_UNSAFE);
    }

    #[test]
    fn source_changed_before_publish() {
        // port: tests/test_transaction.py::test_source_changed_before_publish
        let e = env();
        let v1 = files("A");
        e.publish(&v1, false).unwrap();
        let epf = e.epf.clone();
        set_before_publish_hook(Some(Box::new(move || fs::write(&epf, b"epf-v2-saved-in-designer").unwrap())));
        let err = e.publish(&files("B"), false).unwrap_err();
        set_before_publish_hook(None);
        assert_eq!(err.code, SOURCE_CHANGED);
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn second_rename_failure_restores_backup() {
        // port: tests/test_transaction.py::test_second_rename_failure_restores_backup
        let e = env();
        let v1 = files("A");
        e.publish(&v1, false).unwrap();
        set_rename_hook(Some(Box::new(|a, b| {
            if name_of(a) == "NewSrc" {
                Err(std::io::Error::other("disk says no"))
            } else {
                fs::rename(a, b)
            }
        })));
        let err = e.publish(&files("B"), false).unwrap_err();
        set_rename_hook(None);
        assert_eq!(err.code, IO_ERROR);
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn rollback_failure_keeps_workspace() {
        // port: tests/test_transaction.py::test_rollback_failure_keeps_workspace
        let e = env();
        let v1 = files("A");
        e.publish(&v1, false).unwrap();
        set_rename_hook(Some(Box::new(|a, b| {
            if matches!(name_of(a).as_str(), "NewSrc" | "Backup") {
                Err(std::io::Error::other("nope"))
            } else {
                fs::rename(a, b)
            }
        })));
        let err = e.publish(&files("B"), false).unwrap_err();
        assert_eq!(err.code, RECOVERY_REQUIRED);
        let runs = e.runs();
        assert_eq!(runs.len(), 1);
        assert!(runs[0].join("Backup").join(MANIFEST_PATH).is_file());
        set_rename_hook(None);
        assert!(!recover(&e.root, PROC).unwrap().is_empty());
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    /// `rename(src, Backup)` fails like a directory that another process holds.
    #[cfg(windows)]
    fn hold_src_dir() {
        set_rename_hook(Some(Box::new(|a, b| {
            if name_of(a) == "src" {
                Err(std::io::Error::from_raw_os_error(5))
            } else {
                fs::rename(a, b)
            }
        })));
    }

    #[cfg(windows)]
    #[test]
    fn held_src_is_updated_in_place() {
        let e = env();
        let (v1, v2) = (files("A"), files("B"));
        e.publish(&v1, false).unwrap();
        hold_src_dir();
        let res = e.publish(&v2, false);
        set_rename_hook(None);
        assert_eq!(res.unwrap().status, "UPDATED");
        assert_eq!(read_tree(&e.src).unwrap(), v2);
        assert!(e.runs().is_empty());
        // files that the new tree lacks are gone, new ones are there, no temporary file is left behind
        let mut v3 = files("C");
        v3.remove("Forms/X/FormModule.bsl");
        v3.insert("New/Deep/F.bsl".into(), b"deep".to_vec());
        let entries: Vec<Value> = v3.iter().filter(|(p, _)| *p != MANIFEST_PATH).map(|(p, d)| json!({"path": p, "sha256": sha256_hex(d)})).collect();
        v3.insert(MANIFEST_PATH.into(), crate::canonical::dumps_json(&json!({"schemaVersion": 1, "files": entries})).unwrap());
        hold_src_dir();
        let res = e.publish(&v3, false);
        set_rename_hook(None);
        res.unwrap();
        assert_eq!(read_tree(&e.src).unwrap(), v3);
        assert!(!e.src.join("Forms").exists());
    }

    #[cfg(windows)]
    #[test]
    fn failed_in_place_update_restores_the_old_files() {
        let e = env();
        // `Zed` is a file in the old tree and a directory in the new one: the update fails after `ObjectModule.bsl` was replaced
        let mut v1 = files("A");
        v1.insert("Zed".into(), b"file".to_vec());
        let entries: Vec<Value> = v1.iter().filter(|(p, _)| *p != MANIFEST_PATH).map(|(p, d)| json!({"path": p, "sha256": sha256_hex(d)})).collect();
        v1.insert(MANIFEST_PATH.into(), crate::canonical::dumps_json(&json!({"schemaVersion": 1, "files": entries})).unwrap());
        write_tree(&e.src, &v1).unwrap();
        let mut v2 = files("B");
        v2.insert("Zed/sub/f.txt".into(), b"x".to_vec());
        hold_src_dir();
        let err = e.publish(&v2, false).unwrap_err();
        set_rename_hook(None);
        assert_eq!(err.code, IO_ERROR);
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn recover_in_place_rolls_back_a_half_updated_src() {
        let e = env();
        let (v1, v2) = (files("A"), files("B"));
        e.publish(&v1, false).unwrap();
        let run = e.crash_run("in-place", &v2, "abc123", PROC);
        write_tree(&run.join("Backup"), &v1).unwrap();
        // half way: one file of the new tree is in place, the manifest is still the old one
        fs::write(e.src.join("ObjectModule.bsl"), &v2["ObjectModule.bsl"]).unwrap();
        fs::write(e.src.join(format!("Forms/X/FormModule.bsl{}", crate::fsretry::TMP_SUFFIX)), b"half").unwrap();
        let actions = recover(&e.root, PROC).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn recover_in_place_completes_a_fully_updated_src() {
        let e = env();
        let (v1, v2) = (files("A"), files("B"));
        e.publish(&v1, false).unwrap();
        let run = e.crash_run("in-place", &v2, "abc123", PROC);
        write_tree(&run.join("Backup"), &v1).unwrap();
        crate::fsretry::sync_dir(&e.src, &v2, &|_| false, Some(MANIFEST_PATH)).unwrap();
        recover(&e.root, PROC).unwrap();
        assert_eq!(read_tree(&e.src).unwrap(), v2);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn recover_in_place_without_backup_stops() {
        let e = env();
        e.publish(&files("A"), false).unwrap();
        let run = e.crash_run("in-place", &files("B"), "abc123", PROC);
        assert_eq!(recover(&e.root, PROC).unwrap_err().code, RECOVERY_REQUIRED);
        assert!(run.exists());
    }

    #[test]
    fn lock_is_exclusive_and_released() {
        // port: tests/test_transaction.py::test_lock_is_exclusive_and_released
        let e = env();
        {
            let _a = processor_lock(&e.root, PROC).unwrap();
            assert_eq!(processor_lock(&e.root, PROC).err().unwrap().code, PROCESSOR_LOCKED);
            let _b = processor_lock(&e.root, "ExternalDataProcessors/Other").unwrap();
        }
        let _c = processor_lock(&e.root, PROC).unwrap();
    }

    #[test]
    fn recover_prepared_deletes_run_only() {
        // port: tests/test_transaction.py::test_recover_prepared_deletes_run_only
        let e = env();
        let v1 = files("A");
        e.publish(&v1, false).unwrap();
        let run = e.crash_run(PREPARED, &files("B"), "abc123", PROC);
        assert!(run.join("NewSrc").is_dir());
        recover(&e.root, PROC).unwrap();
        assert!(!run.exists());
        assert_eq!(read_tree(&e.src).unwrap(), v1);
    }

    #[test]
    fn recover_src_moved_restores_backup() {
        // port: tests/test_transaction.py::test_recover_src_moved_restores_backup
        let e = env();
        let v1 = files("A");
        e.publish(&v1, false).unwrap();
        let run = e.crash_run(SRC_MOVED, &files("B"), "abc123", PROC);
        fs::rename(&e.src, run.join("Backup")).unwrap();
        recover(&e.root, PROC).unwrap();
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn recover_prepared_with_backup_and_missing_src() {
        // port: tests/test_transaction.py::test_recover_prepared_with_backup_and_missing_src
        let e = env();
        let v1 = files("A");
        e.publish(&v1, false).unwrap();
        let run = e.crash_run(PREPARED, &files("B"), "abc123", PROC);
        fs::rename(&e.src, run.join("Backup")).unwrap();
        recover(&e.root, PROC).unwrap();
        assert_eq!(read_tree(&e.src).unwrap(), v1);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn recover_src_moved_with_valid_new_src_commits() {
        // port: tests/test_transaction.py::test_recover_src_moved_with_valid_new_src_commits
        let e = env();
        let (v1, v2) = (files("A"), files("B"));
        e.publish(&v1, false).unwrap();
        let run = e.crash_run(SRC_MOVED, &v2, "abc123", PROC);
        fs::rename(&e.src, run.join("Backup")).unwrap();
        fs::rename(run.join("NewSrc"), &e.src).unwrap();
        recover(&e.root, PROC).unwrap();
        assert_eq!(read_tree(&e.src).unwrap(), v2);
        assert!(e.runs().is_empty());
    }

    #[test]
    fn recover_ambiguous_deletes_nothing() {
        // port: tests/test_transaction.py::test_recover_ambiguous_deletes_nothing
        let e = env();
        e.publish(&files("A"), false).unwrap();
        let run = e.crash_run(SRC_MOVED, &files("B"), "abc123", PROC);
        let backup = run.join("Backup");
        fs::create_dir(&backup).unwrap();
        fs::write(e.src.join("ObjectModule.bsl"), b"garbage").unwrap();
        assert_eq!(recover(&e.root, PROC).unwrap_err().code, RECOVERY_REQUIRED);
        assert!(run.exists() && backup.exists());
        assert_eq!(read_tree(&e.src).unwrap()["ObjectModule.bsl"], b"garbage");
    }

    #[test]
    fn recover_corrupt_journal_and_foreign_journal() {
        // port: tests/test_transaction.py::test_recover_corrupt_journal_and_foreign_journal
        let e = env();
        let other = e.crash_run(SRC_MOVED, &files("A"), "other", "ExternalDataProcessors/Other");
        assert!(recover(&e.root, PROC).unwrap().is_empty());
        assert!(other.exists());
        let bad = work_dir(&e.root).join("bad");
        fs::create_dir(&bad).unwrap();
        fs::write(bad.join("journal.json"), "{not json").unwrap();
        assert_eq!(recover(&e.root, PROC).unwrap_err().code, RECOVERY_REQUIRED);
        assert!(bad.exists());
    }

    #[test]
    fn run_without_journal_is_ignored() {
        // port: tests/test_transaction.py::test_run_without_journal_is_ignored
        let e = env();
        let qual = work_dir(&e.root).join("qual");
        fs::create_dir_all(&qual).unwrap();
        fs::write(qual.join("keep.txt"), "x").unwrap();
        assert!(recover(&e.root, PROC).unwrap().is_empty());
        e.publish(&files("A"), false).unwrap();
        assert!(qual.join("keep.txt").exists());
    }
}
