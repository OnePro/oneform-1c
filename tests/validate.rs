//! `oneform validate`: the scope of safe editing per ordinary form, reason codes, JSON, exit codes, no writes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

use oneform::container::{parse_container, serialize_container, EntryOut, Limits};
use serde_json::Value;

const DOC_FORM: &str = "Documents/ДокументТест/Forms/ФормаДокумента/Ext/Form.bin";
const DOC_LABEL: &str = "Documents/ДокументТест/ФормаДокумента";
const COMMON_LABEL: &str = "CommonForms/ОбщаяФорма/ОбщаяФорма";
const LABEL_GUID: &str = "0fc7e20d-f241-460c-bdf4-5ad88e5474a5";
const ACTIVEX_GUID: &str = "621e95f1-064f-11d4-9400-008048da11f9";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

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

/// A configuration dump with a common form and a document form.
fn config_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&manifest_dir().join("tests/golden/config-extract/input"), dir.path());
    dir
}

fn run(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oneform")).args(args).current_dir(cwd).output().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn json(root: &Path) -> (i32, Vec<Value>) {
    let o = run(&["validate", ".", "--json"], root);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", out(&o)));
    (code(&o), v.as_array().unwrap().clone())
}

fn find<'a>(items: &'a [Value], label: &str) -> &'a Value {
    items
        .iter()
        .find(|v| format!("{}/{}", v["object"].as_str().unwrap(), v["form"].as_str().unwrap()) == label)
        .unwrap_or_else(|| panic!("no {label} in {items:?}"))
}

fn codes(item: &Value) -> Vec<&str> {
    item["reasons"].as_array().unwrap().iter().map(|r| r["code"].as_str().unwrap()).collect()
}

/// Replace the `form` entry of a `Form.bin` by `edit(stream)`; every other entry and header stays as it was.
fn edit_stream(root: &Path, rel: &str, edit: impl FnOnce(String) -> String) {
    let path = root.join(rel);
    let raw = fs::read(&path).unwrap();
    let limits = Limits::default();
    let c = parse_container(&raw, &limits, 0, None).unwrap();
    let new_form = edit(String::from_utf8(c.get("form").unwrap().raw.clone()).unwrap()).into_bytes();
    let entries: Vec<EntryOut<'_>> = c
        .entries
        .iter()
        .map(|e| (e.name.as_str(), if e.name == "form" { new_form.as_slice() } else { e.raw.as_slice() }, Some(e.header.as_slice())))
        .collect();
    fs::write(path, serialize_container(&entries, 0x200).unwrap()).unwrap();
}

fn replace_first(s: &str, from: &str, to: &str) -> String {
    assert!(s.contains(from), "{from} is not in the stream");
    s.replacen(from, to, 1)
}

/// Every file with its bytes and modification time, hidden directories included.
fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, (Vec<u8>, SystemTime)>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.insert(format!("{}/", p.strip_prefix(base).unwrap().display()), (Vec::new(), SystemTime::UNIX_EPOCH));
                walk(base, &p, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                out.insert(rel, (fs::read(&p).unwrap(), fs::metadata(&p).unwrap().modified().unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn inside_the_object_directory_the_form_is_named_with_the_object_path() {
    let w = config_copy();
    let object = w.path().join("Documents/ДокументТест");
    let o = run(&["validate", "."], &object);
    let text = out(&o);
    assert!(text.contains(&format!("{DOC_LABEL}: IN_SCOPE")), "{text}");
    assert!(!text.contains("./ФормаДокумента"), "{text}");
}

#[test]
fn ordinary_forms_are_in_scope() {
    let w = config_copy();
    let o = run(&["validate", "."], w.path());
    assert_eq!(code(&o), 0, "{}", out(&o));
    let text = out(&o);
    assert!(text.contains(&format!("{DOC_LABEL}: IN_SCOPE")), "{text}");
    assert!(text.contains(&format!("{COMMON_LABEL}: IN_SCOPE")), "{text}");
    assert!(text.contains("2 forms, 2 in scope (100.0 %)"), "{text}");
}

#[test]
fn donor_fixture_forms_are_in_scope() {
    for name in ["DonorForm", "DonorFormAlt"] {
        let w = config_copy();
        let stream = fs::read_to_string(manifest_dir().join("tests/fixtures/forms").join(name).join("FormStream.bin")).unwrap();
        edit_stream(w.path(), DOC_FORM, |_| stream);
        let (c, items) = json(w.path());
        assert_eq!(c, 0, "{name}: {items:?}");
        assert!(find(&items, DOC_LABEL)["in_scope"].as_bool().unwrap(), "{name}");
    }
}

#[test]
fn epf_form_is_in_scope() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(manifest_dir().join("tests/fixtures/donor.epf"), dir.path().join("donor.epf")).unwrap();
    let (c, items) = json(dir.path());
    assert_eq!(c, 0, "{items:?}");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["object"], "donor.epf");
}

#[test]
fn duplicate_control_names_are_out_of_scope() {
    let w = config_copy();
    edit_stream(w.path(), DOC_FORM, |s| replace_first(&s, "\"ПолеВвода2\"", "\"ПолеВвода3\""));
    let (c, items) = json(w.path());
    assert_eq!(c, 1);
    let item = find(&items, DOC_LABEL);
    assert_eq!(item["in_scope"], false);
    assert_eq!(codes(item), ["duplicate-name"]);
    assert!(item["reasons"][0]["hint"].as_str().unwrap().contains("ПолеВвода3"));
    assert!(find(&items, COMMON_LABEL)["in_scope"].as_bool().unwrap());
    let text = out(&run(&["validate", "."], w.path()));
    assert!(text.contains(&format!("{DOC_LABEL}: OUT_OF_SCOPE duplicate-name \u{2014} ")), "{text}");
    assert!(text.contains("2 forms, 1 in scope (50.0 %)"), "{text}");
    assert!(text.contains("  duplicate-name: 1"), "{text}");
}

#[test]
fn format_80_is_out_of_scope() {
    let w = config_copy();
    edit_stream(w.path(), DOC_FORM, |s| replace_first(&s, "{27,", "{20,"));
    let (c, items) = json(w.path());
    assert_eq!(c, 1);
    let item = find(&items, DOC_LABEL);
    assert_eq!(codes(item), ["format-8.0"]);
    assert!(item["reasons"][0]["hint"].as_str().unwrap().contains("Configurator"));
}

#[test]
fn activex_and_unknown_control_kinds_are_out_of_scope() {
    for (guid, shown) in [(ACTIVEX_GUID, "ActiveXControl"), ("aaaaaaaa-1111-2222-3333-444444444444", "unknown kind")] {
        let w = config_copy();
        edit_stream(w.path(), DOC_FORM, |s| replace_first(&s, LABEL_GUID, guid));
        let (c, items) = json(w.path());
        assert_eq!(c, 1, "{shown}");
        let item = find(&items, DOC_LABEL);
        assert!(codes(item).contains(&"unsupported-control"), "{shown}: {item}");
        let hint = item["reasons"].as_array().unwrap().iter().find(|r| r["code"] == "unsupported-control").unwrap()["hint"].as_str().unwrap();
        assert!(hint.contains(shown), "{hint}");
    }
}

#[test]
fn a_stream_that_does_not_round_trip_is_out_of_scope() {
    // an unnamed control: the projection is written, but pack refuses to match the control (empty `name`)
    let w = config_copy();
    edit_stream(w.path(), DOC_FORM, |s| replace_first(&s, "\"ПолеВвода2\"", "\"\""));
    let (c, items) = json(w.path());
    assert_eq!(c, 1);
    let item = find(&items, DOC_LABEL);
    assert_eq!(codes(item), ["round-trip-differs"], "{item}");
    assert!(item["reasons"][0]["hint"].as_str().unwrap().contains("empty"), "{item}");
}

#[test]
fn stale_projection_is_reported_only_when_src_exists() {
    let w = config_copy();
    let o = run(&["extract", "."], w.path());
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    // a fresh projection is current; pending edits of Form.xml are not staleness
    let (c, items) = json(w.path());
    assert_eq!(c, 0, "{items:?}");
    let xml = w.path().join("Documents/ДокументТест/src/Forms/ФормаДокумента/Form.xml");
    let text = fs::read_to_string(&xml).unwrap();
    fs::write(&xml, format!("{text}<!-- pending edit -->\n")).unwrap();
    assert_eq!(json(w.path()).0, 0);
    // an older Form.xml version is stale
    assert!(text.contains("ordinaryFormVersion=\"2.2\""));
    fs::write(&xml, text.replace("ordinaryFormVersion=\"2.2\"", "ordinaryFormVersion=\"2.1\"")).unwrap();
    let (c, items) = json(w.path());
    // a stale projection is reported, but it is not a scope problem: `extract` refreshes it
    assert_eq!(c, 0);
    assert_eq!(codes(find(&items, DOC_LABEL)), ["stale-projection"]);
    assert!(find(&items, DOC_LABEL)["in_scope"].as_bool().unwrap());
    assert!(find(&items, COMMON_LABEL)["in_scope"].as_bool().unwrap());
    // Form.bin changed after the extraction: stale as well
    fs::write(&xml, &text).unwrap();
    edit_stream(w.path(), DOC_FORM, |s| s.replacen("\"Надпись1\"", "\"Надпись9\"", 1));
    let (c, items) = json(w.path());
    // a stale projection is reported, but it is not a scope problem: `extract` refreshes it
    assert_eq!(c, 0);
    assert_eq!(codes(find(&items, DOC_LABEL)), ["stale-projection"]);
    assert!(find(&items, DOC_LABEL)["in_scope"].as_bool().unwrap());
}

#[test]
fn json_shape() {
    let w = config_copy();
    edit_stream(w.path(), DOC_FORM, |s| replace_first(&s, "\"ПолеВвода2\"", "\"ПолеВвода3\""));
    let (_, items) = json(w.path());
    assert_eq!(items.len(), 2);
    for v in &items {
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["object", "form", "in_scope", "reasons"]);
        for r in v["reasons"].as_array().unwrap() {
            let keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
            assert_eq!(keys, ["code", "hint"]);
        }
    }
    assert_eq!(find(&items, COMMON_LABEL)["reasons"], Value::Array(vec![]));
}

#[test]
fn validate_writes_nothing() {
    let w = config_copy();
    edit_stream(w.path(), DOC_FORM, |s| replace_first(&s, "\"ПолеВвода2\"", "\"ПолеВвода3\""));
    let before = snapshot(w.path());
    assert_eq!(code(&run(&["validate", "."], w.path())), 1);
    assert_eq!(code(&run(&["validate", ".", "--json"], w.path())), 1);
    assert_eq!(snapshot(w.path()), before, "validate changed the tree");
    // after an extract as well (src/, .oneform and .extract-work exist now)
    assert_eq!(code(&run(&["extract", "."], w.path())), 0);
    let before = snapshot(w.path());
    run(&["validate", "."], w.path());
    assert_eq!(snapshot(w.path()), before, "validate changed the extracted tree");
}

#[test]
fn usage_errors_exit_2() {
    let w = config_copy();
    assert_eq!(code(&run(&["validate", "no-such-dir"], w.path())), 2);
    assert_eq!(code(&run(&["validate", ".", "--dry-run"], w.path())), 2);
    assert_eq!(code(&run(&["validate", ".", "--strict"], w.path())), 2);
    assert_eq!(code(&run(&["validate", ".", "--replace-generated"], w.path())), 2);
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(code(&run(&["validate", "."], empty.path())), 2);
    // --json stays an error for the commands that do not print JSON
    assert_eq!(code(&run(&["check", ".", "--json"], w.path())), 2);
}

#[test]
fn unreadable_object_is_reported_not_fatal() {
    let w = config_copy();
    fs::write(w.path().join(DOC_FORM), b"not a container").unwrap();
    let (c, items) = json(w.path());
    assert_eq!(c, 1);
    assert_eq!(codes(find(&items, "Documents/ДокументТест/*")), ["unreadable"]);
    assert!(find(&items, COMMON_LABEL)["in_scope"].as_bool().unwrap());
}
