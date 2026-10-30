//! Windows: a file watcher (or an antivirus) holds a handle on `src/` and `.oneform/` that forbids renaming or deleting the
//! directories themselves, while the files inside stay writable (`ReadDirectoryChangesW` handles do exactly that). `extract`
//! and `pack` must still work: renames are retried, then the files inside the directories are updated in place.
#![cfg(windows)]

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OBJECT: &str = "Documents/ДокументТест";
const FORM: &str = "ФормаДокумента";
const SRC_WARNING: &str = "src updated in place (the directory is held by another process)";
const VIEWER_WARNING: &str = "viewer updated in place (the directory is held by another process)";

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oneform")).args(args).arg("-j").arg("1").arg("--no-progress").current_dir(root).output().unwrap()
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn ok(o: &Output) -> String {
    assert!(o.status.success(), "{}", text(o));
    text(o)
}

fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// What a watcher holds: read access, sharing READ and WRITE but not DELETE, opened with backup semantics (a directory).
fn hold(dir: &Path) -> File {
    OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(0x0200_0000)
        .open(dir)
        .unwrap_or_else(|e| panic!("cannot hold {}: {e}", dir.display()))
}

struct Work {
    dir: tempfile::TempDir,
}

impl Work {
    fn new() -> Work {
        let dir = tempfile::tempdir().unwrap();
        copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/config-extract/input"), dir.path());
        ok(&run(dir.path(), &["extract", "."]));
        Work { dir }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn src(&self) -> PathBuf {
        self.root().join(OBJECT).join("src")
    }

    fn viewer(&self) -> PathBuf {
        self.root().join(".oneform")
    }

    fn form_dir(&self) -> PathBuf {
        self.src().join("Forms").join(FORM)
    }

    fn module(&self) -> PathBuf {
        self.form_dir().join("FormModule.bsl")
    }

    fn form_bin(&self) -> PathBuf {
        self.root().join(OBJECT).join("Forms").join(FORM).join("Ext/Form.bin")
    }
}

#[test]
fn extract_replace_generated_updates_src_and_viewer_in_place() {
    let w = Work::new();
    let src_before = tree(&w.src());
    let viewer_before = tree(&w.viewer());
    assert!(!viewer_before.is_empty());

    // a generated file edited by hand and another one deleted: `--replace-generated` has to bring both back
    let xml = w.form_dir().join("Form.xml");
    fs::write(&xml, b"<edited/>").unwrap();
    fs::remove_file(w.form_dir().join("FormModule.bsl")).unwrap();
    // a stale viewer file must disappear, the history must stay
    fs::write(w.viewer().join("stale.txt"), b"x").unwrap();
    fs::write(w.viewer().join("forms").join("stale.data.js"), b"x").unwrap();

    let (_src_handle, _viewer_handle) = (hold(&w.src()), hold(&w.viewer()));
    // the premise of the test: the held directories cannot be renamed, the files inside can be written
    assert!(fs::rename(w.src(), w.src().with_file_name("src-moved")).is_err());
    assert!(fs::rename(w.viewer(), w.viewer().with_file_name(".oneform-moved")).is_err());

    let out = ok(&run(w.root(), &["extract", ".", "--replace-generated"]));
    assert!(out.contains(SRC_WARNING), "{out}");
    assert!(out.contains(VIEWER_WARNING), "{out}");
    assert_eq!(tree(&w.src()), src_before);
    let viewer_after = tree(&w.viewer());
    assert!(!viewer_after.contains_key("stale.txt") && !viewer_after.contains_key("forms/stale.data.js"));
    assert_eq!(viewer_after.keys().collect::<Vec<_>>(), viewer_before.keys().collect::<Vec<_>>());
    assert!(!w.root().join(OBJECT).join("src-moved").exists());
    let leftovers: Vec<String> = fs::read_dir(w.root())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".oneform.") || n.contains(".oneform-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // the work directory of the run is gone (the backup copy was only needed while the update ran)
    let runs: Vec<_> = fs::read_dir(w.root().join(".extract-work")).map(|rd| rd.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    assert!(runs.iter().all(|n: &std::ffi::OsString| n == "locks"), "{runs:?}");

    drop((_src_handle, _viewer_handle));
    // the result is a normal extraction: nothing left to do
    let again = ok(&run(w.root(), &["extract", "."]));
    assert!(again.contains("UNCHANGED"), "{again}");
}

#[test]
fn module_only_pack_updates_src_and_viewer_in_place() {
    let w = Work::new();
    let bin_before = fs::read(w.form_bin()).unwrap();
    let mut module = fs::read_to_string(w.module()).unwrap();
    module.push_str("\r\n// held directories\r\n");
    fs::write(w.module(), &module).unwrap();

    let (_src_handle, _viewer_handle) = (hold(&w.src()), hold(&w.viewer()));
    let out = ok(&run(w.root(), &["pack", "."]));
    assert!(out.contains(VIEWER_WARNING), "{out}");
    assert_ne!(fs::read(w.form_bin()).unwrap(), bin_before, "{out}");
    assert!(out.contains(SRC_WARNING), "{out}");
    // the pack normalizes the module text of `src` to LF
    let lf = module.replace("\r\n", "\n");
    assert_eq!(fs::read_to_string(w.module()).unwrap(), lf);
    drop((_src_handle, _viewer_handle));

    // the packed form reads back as the edited module, and a second pack has nothing to do
    let again = ok(&run(w.root(), &["pack", "."]));
    assert!(!again.contains("UPDATED") && !again.contains("PACKED"), "{again}");
    let check = ok(&run(w.root(), &["extract", "."]));
    assert!(check.contains("UNCHANGED"), "{check}");
}
