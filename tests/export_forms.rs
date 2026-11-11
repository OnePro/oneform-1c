//! `export-forms`, stage A (the plan) and stage B (the writer, `--check`, the safety of the output folder).
//! Stage A: the plan of `Form.xml` / `FormModule.bsl` of every ordinary form of a configuration dump
//! (layout, equality with `extract`, per-form failure isolation, forms without a projection, determinism).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

use oneform::container::{parse_container, serialize_container, EntryOut, Limits};
use oneform::export_forms::{
    apply_export, check_export, check_out_location, discover_forms, plan_export, DifferenceKind, ExportPlan, MARKER_FILE,
};
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

// ---------------------------------------------------------------------------------------------------------------------
// Stage B: the writer
// ---------------------------------------------------------------------------------------------------------------------

const VERSION: &str = "0.0.1-test";

type Snapshot = BTreeMap<String, (Vec<u8>, SystemTime)>;

/// Every file below `root` (relative `/` path -> bytes and modification time), empty directories as `path/`.
fn snapshot(root: &Path) -> Snapshot {
    fn walk(dir: &Path, prefix: &str, out: &mut Snapshot) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let rel = format!("{prefix}{}", e.file_name().to_string_lossy());
            if e.file_type().unwrap().is_dir() {
                out.insert(format!("{rel}/"), (Vec::new(), SystemTime::UNIX_EPOCH));
                walk(&e.path(), &format!("{rel}/"), out);
            } else {
                out.insert(rel, (fs::read(e.path()).unwrap(), e.metadata().unwrap().modified().unwrap()));
            }
        }
    }
    let mut out = Snapshot::new();
    walk(root, "", &mut out);
    out
}

fn bytes_only(s: &Snapshot) -> BTreeMap<&str, &[u8]> {
    s.iter().map(|(k, (b, _))| (k.as_str(), b.as_slice())).collect()
}

fn out_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn export(dump: &Path, out: &Path) -> oneform::export_forms::ApplyReport {
    apply_export(out, &plan_export(dump, 2), VERSION).unwrap()
}

fn set_entry(root: &Path, rel: &str, entry: &str, data: &[u8]) {
    let path = root.join(rel);
    let raw = fs::read(&path).unwrap();
    let c = parse_container(&raw, &Limits::default(), 0, None).unwrap();
    let entries: Vec<EntryOut<'_>> = c
        .entries
        .iter()
        .map(|e| (e.name.as_str(), if e.name == entry { data } else { e.raw.as_slice() }, Some(e.header.as_slice())))
        .collect();
    fs::write(path, serialize_container(&entries, 0x200).unwrap()).unwrap();
}

fn exists(out: &Path, rel: &str) -> bool {
    out.join(rel).exists()
}

#[test]
fn first_export_writes_the_tree_and_the_marker() {
    let out = out_dir();
    let plan = plan_export(&fixture(), 1);
    let report = apply_export(out.path(), &plan, VERSION).unwrap();
    assert_eq!(report.written.len(), 5);
    assert!(report.written.windows(2).all(|w| w[0] < w[1]) && report.written.contains(&MARKER_FILE.to_string()));
    assert!(report.removed.is_empty() && report.unchanged == 0);
    for (path, bytes) in &plan.tree {
        assert_eq!(&fs::read(out.path().join(path)).unwrap(), bytes, "{path}");
    }
    let files: Vec<String> = snapshot(out.path()).keys().filter(|k| !k.ends_with('/')).cloned().collect();
    assert_eq!(files.len(), 5, "{files:?}");
    assert!(!files.iter().any(|f| f.ends_with(oneform::fsretry::TMP_SUFFIX)));
}

#[test]
fn second_export_changes_nothing_and_no_mtime() {
    let out = out_dir();
    export(&fixture(), out.path());
    let before = snapshot(out.path());
    std::thread::sleep(std::time::Duration::from_millis(30));
    let report = export(&fixture(), out.path());
    assert!(report.written.is_empty() && report.removed.is_empty(), "{report:?}");
    assert_eq!(report.unchanged, 5);
    assert_eq!(before, snapshot(out.path()), "bytes and modification times");
}

#[test]
fn a_form_removed_from_the_dump_is_removed_and_empty_folders_pruned() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    fs::remove_dir_all(dump.path().join("Documents/ДокументТест/Forms/ФормаДокумента")).unwrap();
    fs::remove_file(dump.path().join("Documents/ДокументТест/Forms/ФормаДокумента.xml")).unwrap();
    let report = export(dump.path(), out.path());
    assert_eq!(report.removed, [format!("{DOC_DIR}/Form.xml"), format!("{DOC_DIR}/FormModule.bsl")]);
    assert_eq!(report.written, [MARKER_FILE.to_string()], "only the form count of the marker changed");
    assert!(!exists(out.path(), "Documents"), "emptied folders are gone");
    assert!(exists(out.path(), &format!("{COMMON_DIR}/Form.xml")));
}

#[test]
fn removal_prunes_up_to_but_not_the_output_root() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    fs::remove_dir_all(dump.path().join("Documents")).unwrap();
    fs::remove_dir_all(dump.path().join("CommonForms")).unwrap();
    let report = export(dump.path(), out.path());
    assert_eq!(report.removed.len(), 4);
    assert!(out.path().is_dir());
    let left: Vec<String> = snapshot(out.path()).into_keys().collect();
    assert_eq!(left, [MARKER_FILE.to_string()], "no folder is left");
    let marker: Value = serde_json::from_slice(&fs::read(out.path().join(MARKER_FILE)).unwrap()).unwrap();
    assert_eq!(marker["forms"], 0);
}

#[test]
fn foreign_files_keep_their_folders() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    fs::write(out.path().join(DOC_DIR).join("README.md"), "mine").unwrap();
    fs::write(out.path().join("Documents/ДокументТест/notes.txt"), "mine").unwrap();
    fs::remove_dir_all(dump.path().join("Documents/ДокументТест/Forms/ФормаДокумента")).unwrap();
    fs::remove_file(dump.path().join("Documents/ДокументТест/Forms/ФормаДокумента.xml")).unwrap();
    let report = export(dump.path(), out.path());
    assert_eq!(report.removed.len(), 2);
    assert!(!exists(out.path(), &format!("{DOC_DIR}/Form.xml")) && !exists(out.path(), &format!("{DOC_DIR}/FormModule.bsl")));
    assert_eq!(fs::read(out.path().join(DOC_DIR).join("README.md")).unwrap(), b"mine");
    assert_eq!(fs::read(out.path().join("Documents/ДокументТест/notes.txt")).unwrap(), b"mine");
}

#[test]
fn files_of_other_names_or_depths_are_never_removed() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    for rel in ["Form.xml", "Documents/Form.xml", "Documents/ДокументТест/Form.xml", "Documents/ДокументТест/Forms/FormModule.bsl", "Documents/ДокументТест/Forms/Х/Ext/Form.xml", "CommonForms/Form.xml", "CommonForms/Х/Ext/Form.xml"] {
        let path = out.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "mine").unwrap();
    }
    let before = snapshot(out.path());
    let report = export(dump.path(), out.path());
    assert!(report.removed.is_empty() && report.written.is_empty(), "{report:?}");
    assert_eq!(before, snapshot(out.path()));
}

#[test]
fn a_failed_form_keeps_its_old_files_and_the_others_are_updated() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    let old_xml = fs::read(out.path().join(DOC_DIR).join("Form.xml")).unwrap();
    let old_module = fs::read(out.path().join(DOC_DIR).join("FormModule.bsl")).unwrap();
    fs::write(dump.path().join(DOC_FORM), b"damaged").unwrap();
    set_entry(dump.path(), COMMON_FORM, "module", b"// changed module\n");
    let plan = plan_export(dump.path(), 2);
    assert_eq!(plan.report.failures.len(), 1);
    assert!(check_export(out.path(), &plan, VERSION).iter().all(|d| d.kind != DifferenceKind::Removed));
    let report = apply_export(out.path(), &plan, VERSION).unwrap();
    assert!(report.removed.is_empty(), "{report:?}");
    assert_eq!(fs::read(out.path().join(DOC_DIR).join("Form.xml")).unwrap(), old_xml);
    assert_eq!(fs::read(out.path().join(DOC_DIR).join("FormModule.bsl")).unwrap(), old_module);
    assert_eq!(fs::read(out.path().join(COMMON_DIR).join("FormModule.bsl")).unwrap(), b"// changed module\n");
    assert_eq!(report.written, [format!("{COMMON_DIR}/FormModule.bsl")]);
}

#[test]
fn a_case_only_rename_leaves_the_new_spelling() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    let forms = dump.path().join("Documents/ДокументТест/Forms");
    let upper = "ФормаДокумента".to_uppercase();
    assert_ne!(upper, "ФормаДокумента");
    fs::rename(forms.join("ФормаДокумента"), forms.join(&upper)).unwrap();
    fs::rename(forms.join("ФормаДокумента.xml"), forms.join(format!("{upper}.xml"))).unwrap();
    let plan = plan_export(dump.path(), 1);
    assert!(plan.report.failures.is_empty(), "{:?}", plan.report.failures);
    let report = apply_export(out.path(), &plan, VERSION).unwrap();
    assert_eq!(report.removed.len(), 2, "{report:?}");
    let names: Vec<String> = fs::read_dir(out.path().join("Documents/ДокументТест/Forms"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, std::slice::from_ref(&upper), "only the new spelling is on the disk");
    assert!(out.path().join(format!("Documents/ДокументТест/Forms/{upper}/Form.xml")).is_file());
    assert!(check_export(out.path(), &plan, VERSION).is_empty());
}

#[test]
fn a_form_without_a_projection_gets_a_zero_byte_form_xml_and_is_in_the_marker() {
    let dump = config_copy();
    edit_stream(dump.path(), DOC_FORM, |s| s.replacen("{27,", "{20,", 1));
    let out = out_dir();
    export(dump.path(), out.path());
    let xml = out.path().join(DOC_DIR).join("Form.xml");
    assert!(xml.is_file());
    assert_eq!(fs::metadata(xml).unwrap().len(), 0);
    assert!(fs::metadata(out.path().join(DOC_DIR).join("FormModule.bsl")).unwrap().len() > 0);
    let marker: Value = serde_json::from_slice(&fs::read(out.path().join(MARKER_FILE)).unwrap()).unwrap();
    assert_eq!(marker["withoutProjection"], serde_json::json!([{"path": DOC_DIR, "reason": "format-8.0"}]));
    // The second run leaves the empty file alone.
    let report = export(dump.path(), out.path());
    assert!(report.written.is_empty() && report.removed.is_empty());
}

#[test]
fn an_empty_module_is_an_empty_file() {
    let dump = config_copy();
    set_entry(dump.path(), DOC_FORM, "module", b"");
    let out = out_dir();
    export(dump.path(), out.path());
    let module = out.path().join(DOC_DIR).join("FormModule.bsl");
    assert!(module.is_file());
    assert_eq!(fs::metadata(module).unwrap().len(), 0);
}

#[test]
fn the_marker_is_sorted_has_no_time_and_follows_the_version() {
    let dump = config_copy();
    edit_stream(dump.path(), DOC_FORM, |s| s.replacen("{27,", "{20,", 1));
    edit_stream(dump.path(), COMMON_FORM, |s| s.replacen("{27,", "{99,", 1));
    let out = out_dir();
    export(dump.path(), out.path());
    let text = fs::read_to_string(out.path().join(MARKER_FILE)).unwrap();
    let want = format!(
        "{{\n  \"forms\": 2,\n  \"oneformVersion\": \"{VERSION}\",\n  \"ordinaryFormVersion\": \"2.2\",\n  \"schemaVersion\": 1,\n  \"tool\": \"oneform\",\n  \"withoutProjection\": [\n    {{\n      \"path\": \"{COMMON_DIR}\",\n      \"reason\": \"metadata-layout\"\n    }},\n    {{\n      \"path\": \"{DOC_DIR}\",\n      \"reason\": \"format-8.0\"\n    }}\n  ]\n}}\n"
    );
    assert_eq!(text, want);
    assert!(!text.contains('\r'));
    // Same version: untouched. Another version: only the marker is rewritten.
    let plan = plan_export(dump.path(), 1);
    let again = apply_export(out.path(), &plan, VERSION).unwrap();
    assert!(again.written.is_empty());
    let next = apply_export(out.path(), &plan, "0.0.2-test").unwrap();
    assert_eq!(next.written, [MARKER_FILE.to_string()]);
    assert!(fs::read_to_string(out.path().join(MARKER_FILE)).unwrap().contains("0.0.2-test"));
    assert_eq!(check_export(out.path(), &plan, VERSION).len(), 1);
}

#[test]
fn workers_do_not_change_the_written_tree() {
    let (a, b) = (out_dir(), out_dir());
    apply_export(a.path(), &plan_export(&fixture(), 1), VERSION).unwrap();
    apply_export(b.path(), &plan_export(&fixture(), 8), VERSION).unwrap();
    let (sa, sb) = (snapshot(a.path()), snapshot(b.path()));
    assert_eq!(bytes_only(&sa), bytes_only(&sb));
    assert!(sa.contains_key(MARKER_FILE));
}

fn refusal(dump: &Path, out: &Path) -> String {
    check_out_location(dump, out).expect_err("must be refused").message
}

#[test]
fn the_output_may_not_be_the_dump_inside_it_or_around_it() {
    let dump = config_copy();
    let d = dump.path();
    assert!(refusal(d, d).contains("is the dump"));
    assert!(refusal(d, &d.join(".")).contains("is the dump"));
    assert!(refusal(d, &d.join("forms")).contains("inside the dump"));
    assert!(refusal(d, &d.join("Documents")).contains("inside the dump"));
    assert!(refusal(d, &d.join("a").join("..").join("b")).contains("inside the dump"));
    assert!(refusal(d, d.parent().unwrap()).contains("contains the dump"));
    if cfg!(windows) {
        let upper = PathBuf::from(d.to_string_lossy().to_uppercase());
        assert!(refusal(d, &upper).contains("is the dump"));
        assert!(refusal(&upper, &d.join("forms")).contains("inside the dump"));
    }
}

#[test]
fn the_output_may_not_be_reached_through_a_link_into_the_dump() {
    let dump = config_copy();
    let holder = out_dir();
    let link = holder.path().join("link");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(dump.path(), &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(dump.path(), &link).is_ok();
    #[cfg(not(any(unix, windows)))]
    let made = false;
    if !made {
        eprintln!("skipped: cannot create a symbolic link here");
        return;
    }
    assert!(refusal(dump.path(), &link).contains("is the dump"));
    assert!(refusal(dump.path(), &link.join("forms")).contains("inside the dump"));
}

#[test]
fn a_foreign_or_dump_like_output_is_refused() {
    let dump = config_copy();
    let base = out_dir();
    let p = base.path();
    // a file
    fs::write(p.join("file"), "x").unwrap();
    assert!(refusal(dump.path(), &p.join("file")).contains("is a file"));
    // non-empty without the marker
    fs::create_dir(p.join("foreign")).unwrap();
    fs::write(p.join("foreign/notes.txt"), "x").unwrap();
    assert!(refusal(dump.path(), &p.join("foreign")).contains(MARKER_FILE));
    // Configuration.xml, with and without the marker
    fs::create_dir(p.join("dumplike")).unwrap();
    fs::write(p.join("dumplike/Configuration.xml"), "x").unwrap();
    assert!(refusal(dump.path(), &p.join("dumplike")).contains("Configuration.xml"));
    fs::write(p.join("dumplike").join(MARKER_FILE), "{}").unwrap();
    assert!(refusal(dump.path(), &p.join("dumplike")).contains("Configuration.xml"));
    // a Form.bin somewhere below
    fs::create_dir_all(p.join("bins/Catalogs/А/Forms/Б/Ext")).unwrap();
    fs::write(p.join("bins/Catalogs/А/Forms/Б/Ext/Form.bin"), "x").unwrap();
    assert!(refusal(dump.path(), &p.join("bins")).contains("Form.bin"));
}

#[test]
fn a_new_empty_or_marked_output_is_accepted() {
    let dump = config_copy();
    let base = out_dir();
    let p = base.path();
    check_out_location(dump.path(), &p.join("new/deeper/still")).unwrap();
    assert!(!p.join("new").exists(), "nothing is created by the check");
    fs::create_dir(p.join("empty")).unwrap();
    check_out_location(dump.path(), &p.join("empty")).unwrap();
    fs::create_dir(p.join("marked")).unwrap();
    fs::write(p.join("marked").join(MARKER_FILE), "{}").unwrap();
    fs::write(p.join("marked/README.md"), "x").unwrap();
    check_out_location(dump.path(), &p.join("marked")).unwrap();
    // A real export is accepted again.
    export(dump.path(), &p.join("real"));
    check_out_location(dump.path(), &p.join("real")).unwrap();
}

#[test]
fn the_dump_is_untouched_by_everything() {
    let dump = config_copy();
    let before = snapshot(dump.path());
    let base = out_dir();
    let out = base.path().join("out");
    check_out_location(dump.path(), &out).unwrap();
    let plan = plan_export(dump.path(), 4);
    assert!(!check_export(&out, &plan, VERSION).is_empty());
    apply_export(&out, &plan, VERSION).unwrap();
    assert!(check_export(&out, &plan, VERSION).is_empty());
    let _ = check_out_location(dump.path(), &dump.path().join("forms"));
    apply_export(&out, &plan_export(dump.path(), 1), VERSION).unwrap();
    assert_eq!(before, snapshot(dump.path()), "bytes and modification times of the dump");
}

#[test]
fn check_export_sees_up_to_date_changed_removed_and_added() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    let plan = plan_export(dump.path(), 1);
    assert!(check_export(out.path(), &plan, VERSION).is_empty(), "up to date");

    // A dump change: Changed, and nothing is written.
    set_entry(dump.path(), DOC_FORM, "module", b"// new text\n");
    let changed = plan_export(dump.path(), 1);
    let before = snapshot(out.path());
    let diffs = check_export(out.path(), &changed, VERSION);
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    assert_eq!((diffs[0].path.as_str(), diffs[0].kind), ((format!("{DOC_DIR}/FormModule.bsl")).as_str(), DifferenceKind::Changed));
    assert_eq!(before, snapshot(out.path()), "check writes nothing");

    // A stale owned file: Removed (and still on the disk afterwards).
    let stale = "Documents/ДокументТест/Forms/Старая/Form.xml";
    fs::create_dir_all(out.path().join(stale).parent().unwrap()).unwrap();
    fs::write(out.path().join(stale), "old").unwrap();
    let diffs = check_export(out.path(), &plan, VERSION);
    assert_eq!(diffs.len(), 1);
    assert_eq!((diffs[0].path.as_str(), diffs[0].kind), (stale, DifferenceKind::Removed));
    assert!(exists(out.path(), stale));
    fs::remove_dir_all(out.path().join("Documents/ДокументТест/Forms/Старая")).unwrap();

    // Missing files: Added; sorted.
    fs::remove_file(out.path().join(COMMON_DIR).join("FormModule.bsl")).unwrap();
    fs::remove_file(out.path().join(MARKER_FILE)).unwrap();
    let diffs = check_export(out.path(), &plan, VERSION);
    let got: Vec<(&str, DifferenceKind)> = diffs.iter().map(|d| (d.path.as_str(), d.kind)).collect();
    assert_eq!(got, [(MARKER_FILE, DifferenceKind::Added), (format!("{COMMON_DIR}/FormModule.bsl").as_str(), DifferenceKind::Added)]);

    // A missing output folder: everything is Added.
    let none = out.path().join("nothing-here");
    assert_eq!(check_export(&none, &plan, VERSION).len(), 5);
    assert!(!none.exists());
}

#[test]
fn removal_does_not_follow_links() {
    let dump = config_copy();
    let out = out_dir();
    export(dump.path(), out.path());
    let outside = out_dir();
    fs::write(outside.path().join("Form.xml"), "outside").unwrap();
    fs::write(outside.path().join("FormModule.bsl"), "outside").unwrap();
    let link = out.path().join("Documents/ДокументТест/Forms/Ссылка");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(outside.path(), &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(outside.path(), &link).is_ok();
    #[cfg(not(any(unix, windows)))]
    let made = false;
    if !made {
        eprintln!("skipped: cannot create a symbolic link here");
        return;
    }
    // Also a link at the common-forms level.
    let link2 = out.path().join("CommonForms/Ссылка");
    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink(outside.path(), &link2);
    #[cfg(windows)]
    let _ = std::os::windows::fs::symlink_dir(outside.path(), &link2);
    let plan = plan_export(dump.path(), 1);
    assert!(check_export(out.path(), &plan, VERSION).is_empty(), "files behind a link are not owned");
    let report = apply_export(out.path(), &plan, VERSION).unwrap();
    assert!(report.removed.is_empty(), "{report:?}");
    assert_eq!(fs::read(outside.path().join("Form.xml")).unwrap(), b"outside");
    assert_eq!(fs::read(outside.path().join("FormModule.bsl")).unwrap(), b"outside");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
}

// ---------------------------------------------------------------------------------------------------------------------
// Stage C: the command line (`oneform export-forms`)
// ---------------------------------------------------------------------------------------------------------------------

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn oneform(args: &[&str], envs: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oneform"));
    cmd.args(args).env_remove("EPF_OOF_PATH").env_remove("ONEFORM_CARRIERS");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let o = cmd.output().unwrap();
    Run {
        code: o.status.code().unwrap(),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// A temporary folder with the dump in `dump/`; the output goes to `out/` next to it.
struct Work {
    base: tempfile::TempDir,
}

impl Work {
    fn new() -> Work {
        let base = tempfile::tempdir().unwrap();
        copy_dir(&fixture(), &base.path().join("dump"));
        Work { base }
    }
    fn dump(&self) -> PathBuf {
        self.base.path().join("dump")
    }
    fn out(&self) -> PathBuf {
        self.base.path().join("out")
    }
    fn export(&self, extra: &[&str], envs: &[(&str, &str)]) -> Run {
        let (d, o) = (self.dump(), self.out());
        let mut args = vec!["export-forms", s(&d), "--out", s(&o)];
        args.extend_from_slice(extra);
        oneform(&args, envs)
    }
}

fn no_workspace_files(snap: &Snapshot) {
    for key in snap.keys() {
        let key = key.trim_end_matches('/');
        assert!(
            !key.split('/').any(|c| c == ".oneform" || c == "diagnostics.jsonl" || c == "src" || c == ".extract-work"),
            "unexpected {key}"
        );
    }
}

fn under(snap: &Snapshot, prefix: &str) -> Snapshot {
    snap.iter().filter(|(k, _)| k.starts_with(prefix)).map(|(k, v)| (k.clone(), v.clone())).collect()
}

#[test]
fn cli_missing_out_is_a_usage_error_and_writes_nothing() {
    let w = Work::new();
    let before = snapshot(w.base.path());
    let d = w.dump();
    let r = oneform(&["export-forms", s(&d)], &[]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("--out"), "{}", r.stderr);
    assert_eq!(before, snapshot(w.base.path()));
}

#[test]
fn cli_oof_path_and_foreign_options_are_usage_errors() {
    let w = Work::new();
    let before = snapshot(w.base.path());
    let r = w.export(&["--oof-path", "x"], &[]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("built-in decoder"), "{}", r.stderr);
    for flag in [&["--dry-run"][..], &["--replace-generated"], &["--carriers"], &["--experimental"], &["--strict"], &["--name", "x"]] {
        let r = w.export(flag, &[]);
        assert_eq!(r.code, 2, "{flag:?}: {}{}", r.stdout, r.stderr);
    }
    assert_eq!(w.export(&["-j", "0"], &[]).code, 2);
    // --out and --check belong to export-forms only
    let (d, o) = (w.dump(), w.out());
    assert_eq!(oneform(&["lint", s(&d), "--out", s(&o)], &[]).code, 2);
    assert_eq!(oneform(&["validate", s(&d), "--check"], &[]).code, 2);
    assert_eq!(oneform(&["extract", s(&d), "--check"], &[]).code, 2);
    assert_eq!(before, snapshot(w.base.path()));
}

#[test]
fn cli_unsafe_output_is_exit_2_and_the_dump_is_unchanged() {
    let w = Work::new();
    let before = snapshot(w.base.path());
    let d = w.dump();
    let inside = d.join("forms");
    let r = oneform(&["export-forms", s(&d), "--out", s(&inside)], &[]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.starts_with("oneform: error: "), "{}", r.stderr);
    let r = oneform(&["export-forms", s(&d), "--out", s(&d)], &[]);
    assert_eq!(r.code, 2);
    assert_eq!(before, snapshot(w.base.path()));
    // a dump that is not a folder
    let missing = w.base.path().join("nope");
    assert_eq!(oneform(&["export-forms", s(&missing), "--out", s(&w.out())], &[]).code, 2);
    assert!(!w.out().exists());
}

#[test]
fn cli_a_dump_without_ordinary_forms_is_exit_2() {
    let base = tempfile::tempdir().unwrap();
    let dump = base.path().join("dump");
    fs::create_dir_all(dump.join("Catalogs")).unwrap();
    fs::write(dump.join("Configuration.xml"), "<x/>").unwrap();
    let out = base.path().join("out");
    let r = oneform(&["export-forms", s(&dump), "--out", s(&out)], &[]);
    assert_eq!(r.code, 2, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("no ordinary forms under"), "{}", r.stderr);
    assert!(!out.exists());
}

#[test]
fn cli_run_writes_the_forms_and_nothing_else_anywhere() {
    let w = Work::new();
    let before = snapshot(w.base.path());
    let r = w.export(&[], &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(r.stdout.trim_end(), "export-forms: 2 forms, 5 written, 0 removed, 0 unchanged, 0 without Form.xml, 0 failed");
    assert!(r.stderr.is_empty(), "{}", r.stderr);
    let out = w.out();
    for dir in [DOC_DIR, COMMON_DIR] {
        assert!(!fs::read(out.join(dir).join("Form.xml")).unwrap().is_empty());
        assert!(out.join(dir).join("FormModule.bsl").is_file());
    }
    assert!(out.join(MARKER_FILE).is_file());
    // the dump is byte- and mtime-identical, the base folder holds only `dump` and `out`
    let after = snapshot(w.base.path());
    assert_eq!(under(&before, "dump/"), under(&after, "dump/"));
    assert!(after.keys().all(|k| k.starts_with("dump") || k.starts_with("out")), "{:?}", after.keys().collect::<Vec<_>>());
    no_workspace_files(&after);
    assert_eq!(fs::read_dir(w.base.path()).unwrap().count(), 2);
    // the output equals the plan of the library
    let lib = out_dir();
    export(&w.dump(), lib.path());
    let (a, b) = (snapshot(&out), snapshot(lib.path()));
    let (mut a, mut b) = (bytes_only(&a), bytes_only(&b));
    a.remove(MARKER_FILE);
    b.remove(MARKER_FILE);
    assert_eq!(a, b);
}

#[test]
fn cli_second_run_writes_nothing_and_keeps_mtimes() {
    let w = Work::new();
    assert_eq!(w.export(&[], &[]).code, 0);
    let before = snapshot(&w.out());
    std::thread::sleep(std::time::Duration::from_millis(30));
    let r = w.export(&[], &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(r.stdout.trim_end(), "export-forms: 2 forms, 0 written, 0 removed, 5 unchanged, 0 without Form.xml, 0 failed");
    assert_eq!(before, snapshot(&w.out()));
}

#[test]
fn cli_json_has_the_documented_keys_and_a_clean_stdout() {
    let w = Work::new();
    let r = w.export(&["--json"], &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["check", "dump", "exitCode", "failures", "forms", "out", "removed", "unchanged", "withoutProjection", "written"]);
    assert_eq!(v["forms"], 2);
    assert_eq!(v["check"], false);
    assert_eq!(v["exitCode"], 0);
    assert_eq!(v["unchanged"], 0);
    assert_eq!(v["dump"], s(&w.dump()));
    assert_eq!(v["out"], s(&w.out()));
    let written: Vec<&str> = v["written"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert_eq!(written.len(), 5);
    assert!(written.windows(2).all(|p| p[0] <= p[1]));
    assert!(written.contains(&MARKER_FILE));
    // --check adds `differences`
    let c = w.export(&["--check", "--json"], &[]);
    assert_eq!(c.code, 0);
    let v: Value = serde_json::from_str(&c.stdout).unwrap();
    assert_eq!(v["check"], true);
    assert_eq!(v["differences"], serde_json::json!([]));
}

#[test]
fn cli_check_up_to_date_stale_and_edited() {
    let w = Work::new();
    assert_eq!(w.export(&["--check"], &[]).code, 3, "nothing exported yet: everything is added");
    assert!(!w.out().exists(), "--check writes nothing");
    assert_eq!(w.export(&[], &[]).code, 0);
    let clean = w.export(&["--check"], &[]);
    assert_eq!(clean.code, 0, "{}{}", clean.stdout, clean.stderr);
    assert_eq!(clean.stdout.trim_end(), "export-forms --check: 2 forms, 0 differences, 0 without Form.xml, 0 failed");
    // the dump changed
    set_entry(&w.dump(), COMMON_FORM, "module", b"// changed module\n");
    let before = snapshot(&w.out());
    let stale = w.export(&["--check"], &[]);
    assert_eq!(stale.code, 3, "{}{}", stale.stdout, stale.stderr);
    assert!(stale.stdout.contains(&format!("CHANGED {COMMON_DIR}/FormModule.bsl")), "{}", stale.stdout);
    assert!(stale.stdout.contains("1 differences"), "{}", stale.stdout);
    let json: Value = serde_json::from_str(&w.export(&["--check", "--json"], &[]).stdout).unwrap();
    assert_eq!(json["exitCode"], 3);
    assert_eq!(json["differences"], serde_json::json!([{"path": format!("{COMMON_DIR}/FormModule.bsl"), "kind": "changed"}]));
    assert_eq!(before, snapshot(&w.out()), "--check writes nothing");
    // an edited output file, and a form that is no longer in the dump
    assert_eq!(w.export(&[], &[]).code, 0);
    fs::write(w.out().join(DOC_DIR).join("Form.xml"), "edited").unwrap();
    let edited = w.export(&["--check"], &[]);
    assert_eq!(edited.code, 3);
    assert!(edited.stdout.contains(&format!("CHANGED {DOC_DIR}/Form.xml")), "{}", edited.stdout);
    assert_eq!(fs::read(w.out().join(DOC_DIR).join("Form.xml")).unwrap(), b"edited");
    fs::remove_dir_all(w.dump().join("CommonForms")).unwrap();
    let gone = w.export(&["--check"], &[]);
    assert!(gone.stdout.contains(&format!("REMOVED {COMMON_DIR}/Form.xml")), "{}", gone.stdout);
}

#[test]
fn cli_a_damaged_form_is_exit_1_and_the_others_are_exported() {
    let w = Work::new();
    assert_eq!(w.export(&[], &[]).code, 0);
    let old_xml = fs::read(w.out().join(DOC_DIR).join("Form.xml")).unwrap();
    fs::write(w.dump().join(DOC_FORM), b"damaged").unwrap();
    set_entry(&w.dump(), COMMON_FORM, "module", b"// changed module\n");
    let r = w.export(&[], &[]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.starts_with(&format!("oneform: failed: {DOC_DIR}: ")), "{}", r.stderr);
    assert!(r.stdout.contains("2 forms, 1 written, 0 removed, 2 unchanged, 0 without Form.xml, 1 failed"), "{}", r.stdout);
    assert_eq!(fs::read(w.out().join(DOC_DIR).join("Form.xml")).unwrap(), old_xml);
    assert_eq!(fs::read(w.out().join(COMMON_DIR).join("FormModule.bsl")).unwrap(), b"// changed module\n");
    // failures win over differences in --check, and --json lists them
    set_entry(&w.dump(), COMMON_FORM, "module", b"// again\n");
    let c = w.export(&["--check", "--json"], &[]);
    assert_eq!(c.code, 1);
    let v: Value = serde_json::from_str(&c.stdout).unwrap();
    assert_eq!(v["exitCode"], 1);
    assert_eq!(v["failures"][0]["path"], DOC_DIR);
    assert!(v["failures"][0]["code"].is_string() && v["failures"][0]["message"].is_string());
    assert_eq!(v["differences"].as_array().unwrap().len(), 1);
}

#[test]
fn cli_a_form_without_a_projection_is_exit_0_with_an_empty_form_xml() {
    let w = Work::new();
    edit_stream(&w.dump(), DOC_FORM, |s| s.replacen("{27,", "{20,", 1));
    let r = w.export(&[], &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains(&format!("without Form.xml: {DOC_DIR} (format-8.0)")), "{}", r.stdout);
    assert!(r.stdout.trim_end().ends_with("1 without Form.xml, 0 failed"), "{}", r.stdout);
    assert!(fs::read(w.out().join(DOC_DIR).join("Form.xml")).unwrap().is_empty());
    let marker = fs::read_to_string(w.out().join(MARKER_FILE)).unwrap();
    assert!(marker.contains("format-8.0") && marker.contains(DOC_DIR), "{marker}");
    let v: Value = serde_json::from_str(&w.export(&["--json"], &[]).stdout).unwrap();
    assert_eq!(v["withoutProjection"], serde_json::json!([{"path": DOC_DIR, "reason": "format-8.0"}]));
}

#[test]
fn cli_the_environment_and_the_project_config_do_not_change_the_output() {
    let plain = Work::new();
    assert_eq!(plain.export(&[], &[]).code, 0);
    let expected = snapshot(&plain.out());

    let w = Work::new();
    let tiny = r#"{"schemaVersion": 1, "defaults": {"limits": {"maxEntries": 1, "maxInputMiB": 0.0001}}}"#;
    fs::write(w.dump().join(".oneform.json"), tiny).unwrap();
    fs::write(w.dump().join("extraction.json"), tiny).unwrap();
    let r = w.export(&[], &[("EPF_OOF_PATH", "definitely-not-a-program"), ("ONEFORM_CARRIERS", "1")]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(bytes_only(&expected), bytes_only(&snapshot(&w.out())), "tiny limits, bogus decoder, carriers");
    // the dump got no workspace files either
    no_workspace_files(&snapshot(w.base.path()));
}

#[test]
fn cli_workers_do_not_change_the_tree() {
    let (a, b) = (Work::new(), Work::new());
    assert_eq!(a.export(&["-j", "1"], &[]).code, 0);
    assert_eq!(b.export(&["-j", "8"], &[]).code, 0);
    assert_eq!(bytes_only(&snapshot(&a.out())), bytes_only(&snapshot(&b.out())));
}

#[test]
fn cli_extract_is_unchanged_and_writes_no_marker() {
    let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let w = Work::new();
    assert_eq!(w.export(&[], &[]).code, 0);
    let plain = config_copy();
    let r = oneform(&["extract", s(plain.path()), "-j", "1", "--no-progress"], &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let after = snapshot(plain.path());
    assert!(after.keys().all(|k| !k.contains(MARKER_FILE)), "extract must not write the marker");
    // extract's Form.xml / FormModule.bsl are the exported ones
    for (src, dir) in [
        ("Documents/ДокументТест/src/Forms/ФормаДокумента", DOC_DIR),
        ("CommonForms/ОбщаяФорма/src/Forms/ОбщаяФорма", COMMON_DIR),
    ] {
        for file in ["Form.xml", "FormModule.bsl"] {
            assert_eq!(after[&format!("{src}/{file}")].0, fs::read(w.out().join(dir).join(file)).unwrap(), "{dir}/{file}");
        }
    }
}

#[test]
fn cli_help_lists_export_forms_in_the_stable_group() {
    let r = oneform(&["--help"], &[]);
    assert_eq!(r.code, 0);
    let stable = r.stdout.find("STABLE (no flag needed)").unwrap();
    let experimental = r.stdout.rfind("EXPERIMENTAL (needs --experimental):").unwrap();
    let at = r.stdout.find("  export-forms DUMP --out DIR").unwrap();
    assert!(stable < at && at < experimental);
    assert!(r.stdout.contains("--out <DIR>") && r.stdout.contains("--check"), "{}", r.stdout);
}
