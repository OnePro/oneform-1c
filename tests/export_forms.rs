//! `export-forms`, stage A: the plan of `Form.xml` / `FormModule.bsl` of every ordinary form of a configuration dump
//! (layout, equality with `extract`, per-form failure isolation, forms without a projection, determinism).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use oneform::container::{parse_container, serialize_container, EntryOut, Limits};
use oneform::export_forms::{discover_forms, plan_export, ExportPlan};
use serde_json::Value;

const DOC_DIR: &str = "Documents/ДокументТест/Forms/ФормаДокумента";
const DOC_FORM: &str = "Documents/ДокументТест/Forms/ФормаДокумента/Ext/Form.bin";
const COMMON_DIR: &str = "CommonForms/ОбщаяФорма";
const COMMON_FORM: &str = "CommonForms/ОбщаяФорма/Ext/Form.bin";

/// Tests that touch the process environment or run `extract` (which reads `EPF_OOF_PATH`) take turns.
static ENV: Mutex<()> = Mutex::new(());

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/config-extract/input")
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

fn config_copy() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), dir.path());
    dir
}

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

fn paths(plan: &ExportPlan) -> Vec<&str> {
    plan.tree.keys().map(String::as_str).collect()
}

#[test]
fn layout_follows_the_dump_directories() {
    let forms = discover_forms(&fixture());
    let dirs: Vec<&str> = forms.iter().map(|f| f.dir.as_str()).collect();
    assert_eq!(dirs, [COMMON_DIR, DOC_DIR], "sorted, dump names");
    let plan = plan_export(&fixture(), 1);
    assert_eq!(
        paths(&plan),
        [
            format!("{COMMON_DIR}/Form.xml"),
            format!("{COMMON_DIR}/FormModule.bsl"),
            format!("{DOC_DIR}/Form.xml"),
            format!("{DOC_DIR}/FormModule.bsl"),
        ]
    );
    assert!(plan.tree.keys().all(|p| !p.split('/').any(|s| s == "src")));
    assert_eq!(plan.form_count, 2);
    assert!(plan.report.failures.is_empty() && plan.report.without_projection.is_empty());
    assert!(!plan.tree[&format!("{DOC_DIR}/Form.xml")].is_empty());
}

#[test]
fn bytes_equal_the_extraction() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let plan = plan_export(&fixture(), 1);
    let w = config_copy();
    let o = Command::new(env!("CARGO_BIN_EXE_oneform")).args(["extract", ".", "-j", "1"]).current_dir(w.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let mut compared = 0;
    for f in discover_forms(&fixture()) {
        let obj_rel = f.object.as_str();
        let bin = if obj_rel.starts_with("CommonForms/") { "Ext/Form.bin".to_string() } else { format!("Forms/{}/Ext/Form.bin", f.name) };
        let manifest: Value = serde_json::from_slice(&fs::read(w.path().join(obj_rel).join("src/ExtractionManifest.json")).unwrap()).unwrap();
        let alias = manifest["source"]["forms"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, v)| v["binPath"] == bin.as_str())
            .map(|(k, _)| k.clone())
            .unwrap();
        for file in ["Form.xml", "FormModule.bsl"] {
            let extracted = fs::read(w.path().join(obj_rel).join("src/Forms").join(&alias).join(file)).unwrap();
            assert_eq!(plan.tree[&format!("{}/{file}", f.dir)], extracted, "{}/{file}", f.dir);
            compared += 1;
        }
    }
    assert_eq!(compared, 4);
}

#[test]
fn a_damaged_form_is_a_failure_and_the_others_go_on() {
    let w = config_copy();
    fs::write(w.path().join(DOC_FORM), b"this is not a container").unwrap();
    let plan = plan_export(w.path(), 2);
    assert_eq!(plan.report.failures.len(), 1);
    let failure = &plan.report.failures[0];
    assert_eq!(failure.path, DOC_DIR);
    assert!(!failure.code.is_empty() && !failure.message.is_empty());
    assert_eq!(paths(&plan), [format!("{COMMON_DIR}/Form.xml"), format!("{COMMON_DIR}/FormModule.bsl")]);
    assert_eq!(plan.form_count, 2);
    assert!(plan.report.without_projection.is_empty());
}

#[test]
fn a_missing_form_bin_is_a_failure() {
    let w = config_copy();
    // The container has no `form` entry: it is read, but is not a form.
    let raw = fs::read(w.path().join(DOC_FORM)).unwrap();
    let c = parse_container(&raw, &Limits::default(), 0, None).unwrap();
    let entries: Vec<EntryOut<'_>> =
        c.entries.iter().filter(|e| e.name != "form").map(|e| (e.name.as_str(), e.raw.as_slice(), Some(e.header.as_slice()))).collect();
    fs::write(w.path().join(DOC_FORM), serialize_container(&entries, 0x200).unwrap()).unwrap();
    let plan = plan_export(w.path(), 1);
    assert_eq!(plan.report.failures.len(), 1);
    assert_eq!(plan.report.failures[0].code, "PAYLOAD_MISSING");
    assert_eq!(plan.tree.len(), 2);
}

#[test]
fn format_80_gets_an_empty_form_xml_and_keeps_the_module() {
    let w = config_copy();
    edit_stream(w.path(), DOC_FORM, |s| {
        assert!(s.contains("{27,"));
        s.replacen("{27,", "{20,", 1)
    });
    let plan = plan_export(w.path(), 1);
    assert!(plan.report.failures.is_empty());
    assert_eq!(plan.report.without_projection.len(), 1);
    assert_eq!((plan.report.without_projection[0].path.as_str(), plan.report.without_projection[0].reason.as_str()), (DOC_DIR, "format-8.0"));
    assert!(plan.tree[&format!("{DOC_DIR}/Form.xml")].is_empty());
    let original = plan_export(&fixture(), 1);
    assert_eq!(plan.tree[&format!("{DOC_DIR}/FormModule.bsl")], original.tree[&format!("{DOC_DIR}/FormModule.bsl")]);
    assert_eq!(plan.tree[&format!("{COMMON_DIR}/Form.xml")], original.tree[&format!("{COMMON_DIR}/Form.xml")]);
}

#[test]
fn an_unknown_stream_version_is_metadata_layout() {
    let w = config_copy();
    edit_stream(w.path(), COMMON_FORM, |s| s.replacen("{27,", "{99,", 1));
    let plan = plan_export(w.path(), 1);
    let item = &plan.report.without_projection;
    assert_eq!(item.len(), 1);
    assert_eq!((item[0].path.as_str(), item[0].reason.as_str()), (COMMON_DIR, "metadata-layout"));
    assert!(plan.tree[&format!("{COMMON_DIR}/Form.xml")].is_empty());
}

#[test]
fn workers_do_not_change_the_plan() {
    assert_eq!(plan_export(&fixture(), 1), plan_export(&fixture(), 8));
    let w = config_copy();
    fs::write(w.path().join(DOC_FORM), b"garbage").unwrap();
    assert_eq!(plan_export(w.path(), 1), plan_export(w.path(), 8));
}

#[test]
fn the_environment_and_the_project_config_are_not_consulted() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let plain = plan_export(&fixture(), 1);
    let w = config_copy();
    let tiny = r#"{"schemaVersion": 1, "defaults": {"limits": {"maxEntries": 1, "maxInputMiB": 0.0001}}}"#;
    fs::write(w.path().join("extraction.json"), tiny).unwrap();
    fs::write(w.path().join(".oneform.json"), tiny).unwrap();
    std::env::set_var("EPF_OOF_PATH", "definitely-not-a-program");
    let with_env = plan_export(w.path(), 1);
    std::env::remove_var("EPF_OOF_PATH");
    assert_eq!(plain, with_env);
}

/// Task 1.4 on the owner's real dump (read-only): `cargo test --test export_forms real_dump -- --ignored --nocapture`.
/// `ONEFORM_REAL_DUMP` is the dump, `ONEFORM_REAL_FORMS` the existing `…/src/Forms/…` mirror to compare with.
#[test]
#[ignore]
fn real_dump_matches_the_old_mirror() {
    let dump = PathBuf::from(std::env::var("ONEFORM_REAL_DUMP").expect("ONEFORM_REAL_DUMP"));
    let mirror = PathBuf::from(std::env::var("ONEFORM_REAL_FORMS").expect("ONEFORM_REAL_FORMS"));
    let workers: usize = std::env::var("ONEFORM_WORKERS").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
    let started = std::time::Instant::now();
    let plan = plan_export(&dump, workers);
    let planned_in = started.elapsed();
    let mut same = 0usize;
    let mut diffs: Vec<String> = Vec::new();
    for f in discover_forms(&dump) {
        // `<Type>/<Obj>/Forms/<Form>` -> `<Type>/<Obj>/src/Forms/<Form>`; `CommonForms/<N>` -> `CommonForms/<N>/src/Forms/<N>`.
        let old = match f.dir.split_once("/Forms/") {
            Some((obj, form)) => format!("{obj}/src/Forms/{form}"),
            None => format!("{0}/src/Forms/{1}", f.dir, f.name),
        };
        let mut form_same = true;
        for file in ["Form.xml", "FormModule.bsl"] {
            let new = plan.tree.get(&format!("{}/{file}", f.dir));
            let was = fs::read(mirror.join(&old).join(file)).ok();
            let why = match (new, was) {
                (None, _) => Some("not planned (failure)".to_string()),
                (Some(_), None) => Some("missing in the old mirror".to_string()),
                (Some(n), Some(o)) if *n == o => None,
                (Some(n), Some(o)) => {
                    let line = n.split(|b| *b == b'\n').zip(o.split(|b| *b == b'\n')).position(|(a, b)| a != b);
                    Some(format!("differs (new {} bytes, old {} bytes, first differing line {:?})", n.len(), o.len(), line.map(|l| l + 1)))
                }
            };
            if let Some(why) = why {
                form_same = false;
                if diffs.len() < 10 {
                    diffs.push(format!("{}/{file}: {why}", f.dir));
                }
            }
        }
        same += usize::from(form_same);
    }
    println!(
        "forms {}, identical {same}, planned files {}, without projection {}, failures {}, planning time {:.2?} (-j {workers})",
        plan.form_count,
        plan.tree.len(),
        plan.report.without_projection.len(),
        plan.report.failures.len(),
        planned_in
    );
    for w in &plan.report.without_projection {
        println!("no Form.xml: {} ({})", w.path, w.reason);
    }
    for f in &plan.report.failures {
        println!("failure: {} {} {}", f.path, f.code, f.message);
    }
    for d in &diffs {
        println!("diff: {d}");
    }
}
