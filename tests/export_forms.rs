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
