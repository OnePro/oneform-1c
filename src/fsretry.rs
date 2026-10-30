//! File-system calls that survive the short or permanent holds of other processes on Windows.
//!
//! An antivirus scan, an indexer or a file watcher (`ReadDirectoryChangesW` on `src/` or `.oneform/`) makes renames and
//! removals fail with `ERROR_ACCESS_DENIED` (5), `ERROR_SHARING_VIOLATION` (32), `ERROR_LOCK_VIOLATION` (33) or
//! `ERROR_DIR_NOT_EMPTY` (145). The wrappers here retry such errors with a growing delay (`ONEFORM_FS_RETRY_MS` = total
//! delay budget in milliseconds, default 1500, `0` = no retry). Other errors and other platforms: plain pass-through.
//!
//! [`sync_dir`] is the fallback for a directory that cannot be renamed at all: it updates the files inside it.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const RETRY_ENV: &str = "ONEFORM_FS_RETRY_MS";
const DEFAULT_BUDGET_MS: u64 = 1500;
const FIRST_DELAY_MS: u64 = 15;
const MAX_DELAY_MS: u64 = 250;
/// Suffix of the temporary sibling used to replace a file inside a directory that is updated in place.
pub const TMP_SUFFIX: &str = ".oneform-tmp";

/// A Windows error that usually goes away by itself (or is an exclusive hold that is worth a fallback).
pub fn is_transient(e: &io::Error) -> bool {
    cfg!(windows) && (matches!(e.raw_os_error(), Some(5 | 32 | 33 | 145)) || e.kind() == io::ErrorKind::PermissionDenied)
}

fn parse_budget(raw: Option<&str>) -> u64 {
    raw.and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(DEFAULT_BUDGET_MS)
}

/// Total delay budget in milliseconds.
pub fn budget_ms() -> u64 {
    parse_budget(std::env::var(RETRY_ENV).ok().as_deref())
}

/// Runs `op`; after a failure that `transient` accepts it sleeps (15 ms, doubling up to 250 ms) and runs it again until
/// the delays sum to `budget_ms`. The last error is returned when the budget is spent.
pub fn retry_with<T>(
    budget_ms: u64,
    mut sleep: impl FnMut(Duration),
    transient: impl Fn(&io::Error) -> bool,
    mut op: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    let (mut slept, mut delay) = (0u64, FIRST_DELAY_MS);
    loop {
        match op() {
            Err(e) if transient(&e) && slept < budget_ms => {
                let step = delay.min(budget_ms - slept);
                sleep(Duration::from_millis(step));
                slept += step;
                delay = (delay * 2).min(MAX_DELAY_MS);
            }
            other => return other,
        }
    }
}

/// [`retry_with`] with the real clock, the environment budget and [`is_transient`].
pub fn retry<T>(op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    retry_with(budget_ms(), std::thread::sleep, is_transient, op)
}

/// A removal that finds the target gone after an earlier failed attempt has succeeded in effect.
fn retry_removal(mut op: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut attempts = 0u32;
    retry(|| {
        attempts += 1;
        match op() {
            Err(e) if attempts > 1 && e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    })
}

pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    retry(|| fs::rename(from, to))
}

pub fn remove_file(path: &Path) -> io::Result<()> {
    retry_removal(|| fs::remove_file(path))
}

pub fn remove_dir(path: &Path) -> io::Result<()> {
    retry_removal(|| fs::remove_dir(path))
}

pub fn remove_dir_all(path: &Path) -> io::Result<()> {
    retry_removal(|| fs::remove_dir_all(path))
}

pub fn create_dir_all(path: &Path) -> io::Result<()> {
    retry(|| fs::create_dir_all(path))
}

pub fn create_dir(path: &Path) -> io::Result<()> {
    retry(|| fs::create_dir(path))
}

pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
    retry(|| fs::write(path, data))
}

fn clear_readonly(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        let mut perm = meta.permissions();
        if perm.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = fs::set_permissions(path, perm);
        }
    }
}

fn target_path(dir: &Path, rel: &str) -> PathBuf {
    let mut p = dir.to_path_buf();
    for part in rel.split('/') {
        p.push(part);
    }
    p
}

/// Replace one file inside a directory: write a temporary sibling, rename it over the target (both retried).
fn replace_file(target: &Path, data: &[u8]) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(target) {
        if meta.is_dir() {
            remove_dir_all(target)?;
        } else if fs::read(target).is_ok_and(|cur| cur == data) {
            return Ok(());
        }
    }
    if let Some(parent) = target.parent() {
        create_dir_all(parent)?;
    }
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = target.with_file_name(format!("{name}{TMP_SUFFIX}"));
    write(&tmp, data)?;
    clear_readonly(target);
    rename(&tmp, target).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

fn prune(dir: &Path, prefix: &str, files: &BTreeMap<String, Vec<u8>>, keep: &dyn Fn(&str) -> bool) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if keep(&rel) {
            continue;
        }
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_symlink() {
            remove_file(&path).or_else(|_| remove_dir(&path))?;
        } else if kind.is_dir() {
            let below = format!("{rel}/");
            if files.keys().any(|k| k.starts_with(&below)) {
                prune(&path, &rel, files, keep)?;
            } else {
                remove_dir_all(&path)?;
            }
        } else if !files.contains_key(&rel) {
            remove_file(&path)?;
        }
    }
    Ok(())
}

/// Make the contents of `dir` equal to `files` (relative `/` path -> bytes) without renaming `dir` itself: changed or new
/// files go through a temporary sibling and a rename, files and directories that are not in `files` are removed, except the
/// entries for which `keep` (relative path) is true. The file `last` is written after everything else (and so a reader that
/// finds it in its new form knows the rest is already in place).
pub fn sync_dir(
    dir: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    keep: &dyn Fn(&str) -> bool,
    last: Option<&str>,
) -> io::Result<()> {
    create_dir_all(dir)?;
    for (rel, data) in files.iter().filter(|(rel, _)| Some(rel.as_str()) != last) {
        replace_file(&target_path(dir, rel), data)?;
    }
    prune(dir, "", files, keep)?;
    if let Some(rel) = last {
        if let Some(data) = files.get(rel) {
            replace_file(&target_path(dir, rel), data)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn raw5() -> io::Error {
        io::Error::from_raw_os_error(5)
    }

    fn always(_: &io::Error) -> bool {
        true
    }

    fn never(_: &io::Error) -> bool {
        false
    }

    #[test]
    fn succeeds_after_transient_failures() {
        let calls = Cell::new(0);
        let slept = Cell::new(0u128);
        let r = retry_with(
            1500,
            |d| slept.set(slept.get() + d.as_millis()),
            always,
            || {
                calls.set(calls.get() + 1);
                if calls.get() <= 3 {
                    Err(raw5())
                } else {
                    Ok(7)
                }
            },
        );
        assert_eq!(r.unwrap(), 7);
        assert_eq!(calls.get(), 4);
        assert_eq!(slept.get(), 15 + 30 + 60);
    }

    #[test]
    fn gives_up_after_the_budget() {
        let calls = Cell::new(0);
        let slept = Cell::new(0u128);
        let r: io::Result<()> = retry_with(
            100,
            |d| slept.set(slept.get() + d.as_millis()),
            always,
            || {
                calls.set(calls.get() + 1);
                Err(raw5())
            },
        );
        assert_eq!(r.unwrap_err().raw_os_error(), Some(5));
        assert_eq!(slept.get(), 100);
        assert_eq!(calls.get(), 4); // sleeps of 15, 30 and 55 ms (the last one cut to the budget)
    }

    #[test]
    fn non_transient_errors_are_not_retried() {
        let calls = Cell::new(0);
        let r: io::Result<()> = retry_with(
            1500,
            |_| panic!("must not sleep"),
            never,
            || {
                calls.set(calls.get() + 1);
                Err(io::Error::other("disk says no"))
            },
        );
        assert!(r.is_err());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn zero_budget_means_one_attempt() {
        let calls = Cell::new(0);
        let r: io::Result<()> = retry_with(
            0,
            |_| panic!("must not sleep"),
            always,
            || {
                calls.set(calls.get() + 1);
                Err(raw5())
            },
        );
        assert!(r.is_err());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn delays_double_up_to_the_cap() {
        let steps = std::cell::RefCell::new(Vec::new());
        let _: io::Result<()> = retry_with(1500, |d| steps.borrow_mut().push(d.as_millis()), always, || Err(raw5()));
        let steps = steps.into_inner();
        assert_eq!(&steps[..6], &[15, 30, 60, 120, 240, 250]);
        assert_eq!(steps.iter().sum::<u128>(), 1500);
    }

    #[test]
    fn budget_parsing() {
        assert_eq!(parse_budget(None), 1500);
        assert_eq!(parse_budget(Some("0")), 0);
        assert_eq!(parse_budget(Some(" 40 ")), 40);
        assert_eq!(parse_budget(Some("x")), 1500);
    }

    #[test]
    fn transient_classification_follows_the_platform() {
        for code in [5, 32, 33, 145] {
            assert_eq!(is_transient(&io::Error::from_raw_os_error(code)), cfg!(windows), "os error {code}");
        }
        assert!(!is_transient(&io::Error::other("x")));
        assert!(!is_transient(&io::Error::from(io::ErrorKind::NotFound)));
        assert_eq!(is_transient(&io::Error::from(io::ErrorKind::PermissionDenied)), cfg!(windows));
    }

    #[test]
    fn removal_of_a_missing_target_on_the_first_attempt_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(remove_file(&t.path().join("missing")).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(remove_dir_all(&t.path().join("missing")).unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn sync_dir_updates_in_place() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("d");
        fs::create_dir_all(d.join("sub/deep")).unwrap();
        fs::create_dir_all(d.join("gone/x")).unwrap();
        fs::create_dir_all(d.join("keepme")).unwrap();
        fs::write(d.join("a.txt"), "old").unwrap();
        fs::write(d.join("same.txt"), "same").unwrap();
        fs::write(d.join("sub/stale.txt"), "x").unwrap();
        fs::write(d.join("sub/deep/stale.txt"), "x").unwrap();
        fs::write(d.join("gone/x/f.txt"), "x").unwrap();
        fs::write(d.join("keepme/f.txt"), "k").unwrap();
        fs::write(d.join(format!("leftover.txt{TMP_SUFFIX}")), "x").unwrap();
        let mut files = BTreeMap::new();
        files.insert("a.txt".to_string(), b"new".to_vec());
        files.insert("same.txt".to_string(), b"same".to_vec());
        files.insert("sub/b.txt".to_string(), b"b".to_vec());
        files.insert("c/d/e.txt".to_string(), b"e".to_vec());
        files.insert("last.json".to_string(), b"{}".to_vec());
        sync_dir(&d, &files, &|rel| rel == "keepme", Some("last.json")).unwrap();
        let mut found = BTreeMap::new();
        fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for e in fs::read_dir(dir).unwrap().flatten() {
                if e.path().is_dir() {
                    walk(base, &e.path(), out);
                } else {
                    let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                    out.insert(rel, fs::read(e.path()).unwrap());
                }
            }
        }
        walk(&d, &d, &mut found);
        files.insert("keepme/f.txt".to_string(), b"k".to_vec());
        assert_eq!(found, files);
        assert!(!d.join("gone").exists());
    }
}
