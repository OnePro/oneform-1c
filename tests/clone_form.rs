//! `oneform clone-form`: a new form of a configuration object from an existing one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            fs::copy(e.path(), target).unwrap();
        }
    }
}

fn workspace() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/config-extract/input/Documents");
    let docs = tmp.path().join("Documents");
    copy_dir(&input, &docs);
    let object = docs.join("ДокументТест");
    (tmp, object)
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_oneform")).args(args).output().unwrap()
}

#[test]
fn clone_form_registers_a_new_form_that_extracts() {
    let (tmp, object) = workspace();
    let o = object.to_str().unwrap();
    let out = run(&["clone-form", o, "--from", "ФормаДокумента", "--name", "НоваяФорма", "--synonym", "Новая форма"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let meta = fs::read_to_string(object.join("Forms/НоваяФорма.xml")).unwrap();
    assert!(meta.contains("<Name>НоваяФорма</Name>") && meta.contains("<v8:content>Новая форма</v8:content>"), "{meta}");
    let source = fs::read_to_string(object.join("Forms/ФормаДокумента.xml")).unwrap();
    assert!(!meta.contains(source.split("uuid=\"").nth(1).unwrap().split('"').next().unwrap()), "the uuid must be new");
    assert_eq!(
        fs::read(object.join("Forms/НоваяФорма/Ext/Form.bin")).unwrap(),
        fs::read(object.join("Forms/ФормаДокумента/Ext/Form.bin")).unwrap()
    );
    let registered = fs::read_to_string(object.parent().unwrap().join("ДокументТест.xml")).unwrap();
    assert!(registered.contains("<ChildObjects><Form>НоваяФорма</Form></ChildObjects>"), "{registered}");
    // the next extract sees both forms
    let ex = run(&["extract", tmp.path().to_str().unwrap(), "--no-progress"]);
    assert!(ex.status.success(), "{}", String::from_utf8_lossy(&ex.stdout));
    assert!(object.join("src/Forms/НоваяФорма/Form.xml").is_file() && object.join("src/Forms/ФормаДокумента/Form.xml").is_file());
}

#[test]
fn clone_form_refuses_bad_requests_and_changes_nothing() {
    let (_tmp, object) = workspace();
    let o = object.to_str().unwrap();
    let before = fs::read_to_string(object.parent().unwrap().join("ДокументТест.xml")).unwrap();
    for args in [
        vec!["clone-form", o, "--from", "ФормаДокумента", "--name", "1плохо"],
        vec!["clone-form", o, "--from", "Нет", "--name", "Новая"],
        vec!["clone-form", o, "--from", "ФормаДокумента", "--name", "ФормаДокумента"],
        vec!["clone-form", o, "--from", "ФормаДокумента"],
    ] {
        let out = run(&args);
        assert!(!out.status.success(), "{args:?}");
    }
    assert_eq!(fs::read_to_string(object.parent().unwrap().join("ДокументТест.xml")).unwrap(), before);
    assert!(!object.join("Forms/Новая.xml").exists());
}

#[test]
fn clone_form_accepts_the_current_directory_as_the_object() {
    let (_tmp, object) = workspace();
    let out = Command::new(env!("CARGO_BIN_EXE_oneform"))
        .current_dir(&object)
        .args(["clone-form", ".", "--from", "ФормаДокумента", "--name", "ИзТекущей"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(object.join("Forms/ИзТекущей.xml").is_file());
    let registered = fs::read_to_string(object.parent().unwrap().join("ДокументТест.xml")).unwrap();
    assert!(registered.contains("<Form>ИзТекущей</Form>"), "{registered}");
}

#[test]
fn clone_form_names_the_dump_when_given_an_epf() {
    let tmp = tempfile::tempdir().unwrap();
    let epf = tmp.path().join("Обработка.epf");
    fs::write(&epf, b"x").unwrap();
    let out = run(&["clone-form", epf.to_str().unwrap(), "--from", "Форма", "--name", "Новая"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("object directory of an XML dump") && err.contains("epf-dump"), "{err}");
    // a directory that holds no such form says the same
    let dir = tmp.path().join("epf");
    fs::create_dir_all(&dir).unwrap();
    let out = run(&["clone-form", dir.to_str().unwrap(), "--from", "Форма", "--name", "Новая"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not found") && err.contains("epf-dump"), "{err}");
}

#[test]
fn clone_form_refuses_while_src_holds_unpacked_edits() {
    let (tmp, object) = workspace();
    let ex = run(&["extract", tmp.path().to_str().unwrap(), "--no-progress"]);
    assert!(ex.status.success(), "{}", String::from_utf8_lossy(&ex.stdout));
    let module = object.join("src/Forms/ФормаДокумента/FormModule.bsl");
    let original = fs::read(&module).unwrap();
    let mut edited = original.clone();
    edited.extend_from_slice("
// правка
".as_bytes());
    fs::write(&module, &edited).unwrap();
    let before = fs::read_to_string(object.parent().unwrap().join("ДокументТест.xml")).unwrap();
    let out = run(&["clone-form", object.to_str().unwrap(), "--from", "ФормаДокумента", "--name", "Копия"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not packed yet") && err.contains("FormModule.bsl") && err.contains("oneform pack"), "{err}");
    assert!(!object.join("Forms/Копия.xml").exists());
    assert_eq!(fs::read_to_string(object.parent().unwrap().join("ДокументТест.xml")).unwrap(), before);
    // once the edit is packed the clone goes through
    let packed = run(&["pack", object.to_str().unwrap()]);
    assert!(packed.status.success(), "{}{}", String::from_utf8_lossy(&packed.stdout), String::from_utf8_lossy(&packed.stderr));
    let out = run(&["clone-form", object.to_str().unwrap(), "--from", "ФормаДокумента", "--name", "Копия"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
