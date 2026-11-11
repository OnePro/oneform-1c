//! `pack` on a configuration form whose controls share a name: a module-only edit is accepted and leaves the form stream
//! byte for byte as it was; an edit of `Form.xml` is refused before any write and names the repeated control name.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use oneform::container::{parse_container, serialize_container, EntryOut, Limits};

const DOC_FORM: &str = "Documents/ДокументТест/Forms/ФормаДокумента/Ext/Form.bin";

type Tree = BTreeMap<String, Vec<u8>>;

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

/// `experimental` is added to `pack` only when asked: a module-only pack is stable and needs no gate.
fn run_with(root: &Path, args: &[&str], experimental: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oneform"));
    cmd.args(args).args(["-j", "1"]).current_dir(root);
    if experimental {
        cmd.arg("--experimental");
    }
    cmd.output().unwrap()
}

fn run(root: &Path, args: &[&str]) -> Output {
    run_with(root, args, false)
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn snapshot(root: &Path) -> Tree {
    fn walk(base: &Path, dir: &Path, out: &mut Tree) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
            if rel == ".extract-work" || rel == ".oneform" || rel.ends_with(".oneform/diagnostics.jsonl") {
                continue;
            }
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = Tree::new();
    walk(root, root, &mut out);
    out
}

/// The `form` entry (the form stream) of a `Form.bin`.
fn form_stream(path: &Path) -> Vec<u8> {
    let raw = fs::read(path).unwrap();
    parse_container(&raw, &Limits::default(), 0, None).unwrap().get("form").unwrap().raw.clone()
}

/// A configuration dump whose document form has two controls named `ПолеВвода3`, extracted.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/config-extract/input"), dir.path());
    let path = dir.path().join(DOC_FORM);
    let raw = fs::read(&path).unwrap();
    let c = parse_container(&raw, &Limits::default(), 0, None).unwrap();
    let stream = String::from_utf8(c.get("form").unwrap().raw.clone()).unwrap();
    assert!(stream.contains("\"ПолеВвода2\""));
    let new_form = stream.replacen("\"ПолеВвода2\"", "\"ПолеВвода3\"", 1).into_bytes();
    let entries: Vec<EntryOut<'_>> = c
        .entries
        .iter()
        .map(|e| (e.name.as_str(), if e.name == "form" { new_form.as_slice() } else { e.raw.as_slice() }, Some(e.header.as_slice())))
        .collect();
    fs::write(&path, serialize_container(&entries, 0x200).unwrap()).unwrap();
    let o = run(dir.path(), &["extract", "."]);
    assert!(o.status.success(), "extract failed: {}", text(&o));
    dir
}

fn edit_bsl(root: &Path) {
    let p = find_src(root, "FormModule.bsl");
    let mut bsl = fs::read(&p).unwrap();
    bsl.extend_from_slice("\n// edited\n".as_bytes());
    fs::write(&p, bsl).unwrap();
}

fn find_src(root: &Path, name: &str) -> PathBuf {
    let tree = snapshot(root);
    let hit = tree.keys().find(|k| k.starts_with("Documents/") && k.contains("/src/") && k.ends_with(name)).expect("source file");
    root.join(hit)
}

fn changed(before: &Tree, after: &Tree) -> Vec<String> {
    before.keys().chain(after.keys()).filter(|k| before.get(*k) != after.get(*k)).cloned().collect()
}

#[test]
fn module_only_pack_keeps_the_form_stream_of_a_form_with_repeated_names() {
    let w = workspace();
    let stream_before = form_stream(&w.path().join(DOC_FORM));
    edit_bsl(w.path());
    let before = snapshot(w.path());
    let o = run(w.path(), &["pack", "."]);
    assert!(o.status.success(), "{}", text(&o));
    let after = snapshot(w.path());
    assert!(changed(&before, &after).iter().any(|k| k == DOC_FORM), "Form.bin was not rewritten");
    assert_eq!(stream_before, form_stream(&w.path().join(DOC_FORM)), "the form stream changed");
}

#[test]
fn dry_run_of_a_module_edit_writes_nothing() {
    let w = workspace();
    edit_bsl(w.path());
    let before = snapshot(w.path());
    let o = run(w.path(), &["pack", ".", "--dry-run"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(changed(&before, &snapshot(w.path())), Vec::<String>::new());
}

#[test]
fn layout_edit_of_a_form_with_repeated_names_is_refused_before_any_write() {
    let w = workspace();
    let p = find_src(w.path(), "Form.xml");
    let xml = fs::read_to_string(&p).unwrap();
    let at = xml.find("<Width>").expect("Width") + "<Width>".len();
    let end = at + xml[at..].find('<').unwrap();
    fs::write(&p, format!("{}-1{}", &xml[..at], &xml[end..])).unwrap();
    let before = snapshot(w.path());
    let o = run_with(w.path(), &["pack", "."], true);
    let msg = text(&o);
    assert!(!o.status.success(), "{msg}");
    assert!(msg.contains("ПолеВвода3"), "{msg}");
    assert_eq!(changed(&before, &snapshot(w.path())), Vec::<String>::new());
}
