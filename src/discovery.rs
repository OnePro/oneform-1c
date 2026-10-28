//! Processor discovery (`--all`) and selector validation (`--processor`), port of `discovery.py`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::{ExtractionError, Result, CONFIG_INVALID, IO_ERROR};
use crate::naming::is_alias;

pub const SKIP_DIRS: [&str; 5] = [".git", ".venv", ".extract-work", ".oneform", "src"];
pub const CONFIG_NAME: &str = "extraction.json";

fn bad(message: &str) -> ExtractionError {
    ExtractionError::new(CONFIG_INVALID, message)
}

fn io_err(e: std::io::Error, path: &Path) -> ExtractionError {
    ExtractionError::new(IO_ERROR, e.to_string()).with("path", path.to_string_lossy().into_owned())
}

/// Symlink everywhere; on Windows also a junction / any reparse point.
pub fn is_link(path: &Path) -> bool {
    let Ok(md) = fs::symlink_metadata(path) else {
        return false;
    };
    if md.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if md.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub fn root_parts(processor_root: &str) -> Result<Vec<String>> {
    let norm = processor_root.replace('\\', "/");
    let parts: Vec<String> = norm.trim_matches('/').split('/').map(str::to_string).collect();
    if processor_root.starts_with(['/', '\\']) || !parts.iter().all(|p| is_alias(p)) {
        return Err(bad("processorRoot segments must match [A-Za-z][A-Za-z0-9_-]{0,63} and be relative")
            .with("processorRoot", processor_root));
    }
    Ok(parts)
}

/// One directory entry as `os.scandir` sees it.
#[derive(Debug, Clone)]
pub struct DirEnt {
    pub name: String,
    pub path: PathBuf,
    /// `DirEntry.is_file(follow_symlinks=False)`.
    pub is_file: bool,
    /// `DirEntry.is_dir()` (follows links, `False` on error): what `os.walk` puts into `dirnames`.
    pub is_dir: bool,
    /// [`is_link`] of the entry.
    pub link: bool,
}

/// `(link, is_dir, is_file)` without following links, from the directory listing itself (no extra syscall on
/// Windows, where `DirEntry::metadata` comes from the `FindNextFile` data).
#[cfg(windows)]
fn entry_kind(e: &fs::DirEntry) -> std::io::Result<(bool, bool, bool)> {
    use std::os::windows::fs::MetadataExt;
    let md = e.metadata()?;
    let ft = md.file_type();
    Ok((ft.is_symlink() || md.file_attributes() & 0x400 != 0, ft.is_dir(), ft.is_file()))
}

#[cfg(not(windows))]
fn entry_kind(e: &fs::DirEntry) -> std::io::Result<(bool, bool, bool)> {
    let ft = e.file_type()?;
    Ok((ft.is_symlink(), ft.is_dir(), ft.is_file()))
}

/// Directory entries sorted by name (explicitly; never in OS order). Only links cost an extra `stat`.
pub fn scan_dir(directory: &Path) -> std::io::Result<Vec<DirEnt>> {
    let mut out = Vec::new();
    for e in fs::read_dir(directory)? {
        let e = e?;
        let path = e.path();
        let (link, plain_dir, is_file) = entry_kind(&e)?;
        let is_dir = if link { fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) } else { plain_dir };
        out.push(DirEnt { name: e.file_name().to_string_lossy().into_owned(), path, is_file, is_dir, link });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn scan(directory: &Path) -> Result<Vec<DirEnt>> {
    scan_dir(directory).map_err(|e| io_err(e, directory))
}

/// `find_epfs` candidate: `*.epf` (any case), a regular file and not a link.
pub fn is_epf(e: &DirEnt) -> bool {
    e.name.to_lowercase().ends_with(".epf") && e.is_file && !e.link
}

/// A binary external report `*.erf` (any case), a regular file and not a link. `oneform` does not read it: the container
/// is the same as of `.epf`, but the root metadata (class ids, header, attribute / form groups) is the one of a report.
pub fn is_erf(e: &DirEnt) -> bool {
    e.name.to_lowercase().ends_with(".erf") && e.is_file && !e.link
}

/// What `oneform` says about a binary `.erf` (it is skipped, never processed).
pub const ERF_HINT: &str = "внешний отчёт: выгрузите в XML (erf-dump), oneform работает с XML-выгрузкой";

/// Names of `*.epf` files (case-insensitive suffix; `.epf.bak` is not a candidate).
pub fn find_epfs(directory: &Path) -> Result<Vec<String>> {
    Ok(scan(directory)?.into_iter().filter(is_epf).map(|e| e.name).collect())
}

fn check_nesting(found: &[String]) -> Result<()> {
    for a in found {
        for b in found {
            if a != b && b.starts_with(&format!("{a}/")) {
                return Err(bad("nested processors are not allowed").with("parent", a.as_str()).with("child", b.as_str()));
            }
        }
    }
    Ok(())
}

/// Relative POSIX paths (from `root`) of all processor directories, sorted.
pub fn discover_all(root: &Path, processor_root: &str) -> Result<Vec<String>> {
    let parts = root_parts(processor_root)?;
    let mut base = root.to_path_buf();
    base.extend(&parts);
    let mut found = Vec::new();

    fn walk(directory: &Path, rel: &str, found: &mut Vec<String>) -> Result<()> {
        let entries = scan(directory)?;
        let epfs = entries.iter().filter(|e| is_epf(e)).count();
        if epfs > 0 {
            if epfs > 1 {
                return Err(bad("more than one EPF in a processor directory").with("processor", rel));
            }
            found.push(rel.to_string());
        }
        for e in &entries {
            if SKIP_DIRS.contains(&e.name.as_str()) || !e.is_dir || e.link {
                continue;
            }
            walk(&directory.join(&e.name), &format!("{rel}/{}", e.name), found)?;
        }
        Ok(())
    }

    if base.is_dir() && !is_link(&base) {
        walk(&base, &parts.join("/"), &mut found)?;
    }
    found.sort();
    check_nesting(&found)?;
    if found.is_empty() {
        return Err(bad("no processors found").with("processorRoot", processor_root));
    }
    Ok(found)
}

/// Validate `--processor` paths (relative to `root`); return sorted unique POSIX paths.
pub fn resolve_selectors<S: AsRef<str>>(root: &Path, processor_root: &str, selectors: &[S]) -> Result<Vec<String>> {
    let rparts = root_parts(processor_root)?;
    let mut result = std::collections::BTreeSet::new();
    for raw in selectors {
        let raw = raw.as_ref();
        let text = raw.replace('\\', "/");
        if text.starts_with('/') || (text.chars().count() > 1 && text.chars().nth(1) == Some(':')) {
            return Err(bad("processor path must be relative to the repository root").with("path", raw));
        }
        let parts: Vec<&str> = text.trim_matches('/').split('/').filter(|p| !p.is_empty() && *p != ".").collect();
        if parts.is_empty() || !parts.iter().all(|p| is_alias(p)) {
            return Err(bad("processor path segments must match [A-Za-z][A-Za-z0-9_-]{0,63}").with("path", raw));
        }
        if parts.len() <= rparts.len() || parts[..rparts.len()].iter().zip(&rparts).any(|(a, b)| a != b) {
            return Err(bad("processor is not inside processorRoot")
                .with("path", raw)
                .with("processorRoot", processor_root));
        }
        let mut cur = root.to_path_buf();
        for p in &parts {
            cur.push(p);
            if is_link(&cur) {
                return Err(bad("symlinks are not allowed in processor paths").with("path", raw));
            }
        }
        if !cur.is_dir() {
            return Err(bad("processor directory does not exist").with("path", raw));
        }
        if !cur.join(CONFIG_NAME).is_file() {
            return Err(bad("processor extraction.json is required").with("path", raw));
        }
        result.insert(parts.join("/"));
    }
    let ordered: Vec<String> = result.into_iter().collect();
    check_nesting(&ordered)?;
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const ROOT: &str = "ExternalDataProcessors";

    fn touch(base: &Path, rel: &str, data: &[u8]) {
        let p: PathBuf = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    fn make(tmp: &Path, name: &str) {
        touch(tmp, &format!("{ROOT}/{name}/a.epf"), b"x");
        touch(tmp, &format!("{ROOT}/{name}/extraction.json"), b"{}");
    }

    #[test]
    fn discover_sorted_and_filtered() {
        // port: tests/test_discovery.py::test_discover_sorted_and_filtered
        let t = tempfile::tempdir().unwrap();
        let t = t.path();
        for rel in [
            "B/b.epf", "A/a.EPF", "C/c.epf.bak", "A2/src/old.epf", ".git/x.epf", ".extract-work/x.epf",
            ".venv/x.epf", "Group/D/d.epf",
        ] {
            touch(t, &format!("{ROOT}/{rel}"), b"x");
        }
        assert_eq!(
            discover_all(t, ROOT).unwrap(),
            vec![format!("{ROOT}/A"), format!("{ROOT}/B"), format!("{ROOT}/Group/D")]
        );
    }

    #[test]
    fn discover_skips_symlinks() {
        // port: tests/test_discovery.py::test_discover_skips_symlinks
        let t = tempfile::tempdir().unwrap();
        let t = t.path();
        touch(t, &format!("{ROOT}/A/a.epf"), b"x");
        touch(t, "elsewhere/e.epf", b"x");
        #[cfg(windows)]
        let r = std::os::windows::fs::symlink_dir(t.join("elsewhere"), t.join(ROOT).join("Link"));
        #[cfg(unix)]
        let r = std::os::unix::fs::symlink(t.join("elsewhere"), t.join(ROOT).join("Link"));
        if r.is_err() {
            return; // symlinks unavailable (pytest.skip)
        }
        assert_eq!(discover_all(t, ROOT).unwrap(), vec![format!("{ROOT}/A")]);
    }

    #[test]
    fn discover_nested_is_error() {
        // port: tests/test_discovery.py::test_discover_nested_is_error
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), &format!("{ROOT}/A/a.epf"), b"x");
        touch(t.path(), &format!("{ROOT}/A/Inner/i.epf"), b"x");
        assert_eq!(discover_all(t.path(), ROOT).unwrap_err().code, CONFIG_INVALID);
    }

    #[test]
    fn discover_two_epfs_is_error() {
        // port: tests/test_discovery.py::test_discover_two_epfs_is_error
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), &format!("{ROOT}/A/a.epf"), b"x");
        touch(t.path(), &format!("{ROOT}/A/b.epf"), b"x");
        assert!(discover_all(t.path(), ROOT).is_err());
    }

    #[test]
    fn erf_is_recognised_but_never_an_epf() {
        let t = tempfile::tempdir().unwrap();
        for n in ["a.erf", "B.ERF", "c.erf.bak", "d.epf"] {
            touch(t.path(), n, b"x");
        }
        let ents = scan(t.path()).unwrap();
        let erfs: Vec<&str> = ents.iter().filter(|e| is_erf(e)).map(|e| e.name.as_str()).collect();
        assert_eq!(erfs, ["B.ERF", "a.erf"]);
        assert_eq!(find_epfs(t.path()).unwrap(), ["d.epf"]);
    }

    #[test]
    fn discover_zero_is_error() {
        // port: tests/test_discovery.py::test_discover_zero_is_error
        let t = tempfile::tempdir().unwrap();
        fs::create_dir(t.path().join(ROOT)).unwrap();
        assert!(discover_all(t.path(), ROOT).is_err());
        assert!(discover_all(&t.path().join("nowhere"), ROOT).is_err());
    }

    #[test]
    fn selectors_ok() {
        // port: tests/test_discovery.py::test_selectors_ok
        let t = tempfile::tempdir().unwrap();
        make(t.path(), "A");
        make(t.path(), "B");
        let sel = [format!("{ROOT}/B/"), format!("{ROOT}\\A"), format!("{ROOT}/A")];
        assert_eq!(resolve_selectors(t.path(), ROOT, &sel).unwrap(), vec![format!("{ROOT}/A"), format!("{ROOT}/B")]);
    }

    #[test]
    fn selectors_rejected() {
        // port: tests/test_discovery.py::test_selectors_rejected
        let t = tempfile::tempdir().unwrap();
        make(t.path(), "A");
        for path in [
            "/abs/A".to_string(),
            "C:/x/A".to_string(),
            format!("{ROOT}/../A"),
            format!("{ROOT}/1bad"),
            format!("{ROOT}/A b"),
            format!("{ROOT}/Обработка"),
            "Other/A".to_string(),
            ROOT.to_string(),
            format!("{ROOT}/Missing"),
        ] {
            let e = resolve_selectors(t.path(), ROOT, &[&path]).unwrap_err();
            assert_eq!(e.code, CONFIG_INVALID, "{path}");
        }
    }

    #[test]
    fn selector_requires_local_config() {
        // port: tests/test_discovery.py::test_selector_requires_local_config
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), &format!("{ROOT}/A/a.epf"), b"x");
        assert!(resolve_selectors(t.path(), ROOT, &[format!("{ROOT}/A")]).is_err());
    }

    #[test]
    fn selectors_nested_rejected() {
        // port: tests/test_discovery.py::test_selectors_nested_rejected
        let t = tempfile::tempdir().unwrap();
        make(t.path(), "A");
        make(t.path(), "A/Inner");
        assert!(resolve_selectors(t.path(), ROOT, &[format!("{ROOT}/A"), format!("{ROOT}/A/Inner")]).is_err());
    }
}
