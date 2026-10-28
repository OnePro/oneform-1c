//! Automatic history of `pack` (`.oneform/history`) and `oneform history [restore]`: snapshots of configuration forms and
//! `.epf` files, retention, restore round trips, and that the snapshots are never taken for forms or EPF files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OBJECT: &str = "Documents/ДокументТест";
const FORM: &str = "ФормаДокумента";

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

fn run_env(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_oneform"));
    c.args(args).arg("-j").arg("1").arg("--no-progress").current_dir(root);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn run(root: &Path, args: &[&str]) -> Output {
    run_env(root, args, &[])
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

fn ok(o: &Output) -> String {
    assert!(o.status.success(), "{}", text(o));
    text(o)
}

fn history_id(out: &str) -> Option<String> {
    out.lines().find_map(|l| l.trim().strip_prefix("history ").map(|r| r.split_whitespace().next().unwrap_or("").to_string()))
}

struct Work {
    dir: tempfile::TempDir,
}

impl Work {
    fn config() -> Work {
        let dir = tempfile::tempdir().unwrap();
        copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/config-extract/input"), dir.path());
        ok(&run(dir.path(), &["extract", "."]));
        Work { dir }
    }

    fn epf() -> Work {
        let dir = tempfile::tempdir().unwrap();
        fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/donor.epf"), dir.path().join("donor.epf")).unwrap();
        ok(&run(dir.path(), &["extract", "."]));
        Work { dir }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn object(&self) -> PathBuf {
        self.root().join(OBJECT)
    }

    fn bin(&self) -> PathBuf {
        self.object().join("Forms").join(FORM).join("Ext/Form.bin")
    }

    fn src_form(&self, name: &str) -> PathBuf {
        self.object().join("src/Forms").join(FORM).join(name)
    }

    fn history(&self) -> PathBuf {
        self.object().join(".oneform/history")
    }

    fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = fs::read_dir(self.history())
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// Append a line to the form module of the object and pack; returns the printed history id.
    fn edit_and_pack(&self, line: &str) -> Option<String> {
        let p = self.src_form("FormModule.bsl");
        let mut bsl = fs::read(&p).unwrap();
        bsl.extend_from_slice(format!("\n// {line}\n").as_bytes());
        fs::write(&p, bsl).unwrap();
        history_id(&ok(&run(self.root(), &["pack", "."])))
    }

    /// Rename the button caption of the document form in Form.xml.
    fn edit_caption(&self) {
        let p = self.src_form("Form.xml");
        let xml = fs::read_to_string(&p).unwrap();
        let xml = xml
            .replace("<Item language=\"ru\">Выполнить</Item>", "<Item language=\"ru\">Запустить</Item>")
            .replace("<Item language=\"ro\">Выполнить</Item>", "<Item language=\"ro\">Запустить</Item>");
        fs::write(&p, xml).unwrap();
    }
}

#[test]
fn pack_saves_before_and_after_with_a_manifest() {
    let w = Work::config();
    let bin_before = fs::read(w.bin()).unwrap();
    let xml_before = fs::read(w.src_form("Form.xml")).unwrap();
    let bsl_before = fs::read(w.src_form("FormModule.bsl")).unwrap();
    w.edit_caption();
    let xml_edited = fs::read(w.src_form("Form.xml")).unwrap();
    let id = w.edit_and_pack("правка модуля").expect("pack prints the history id");
    let bsl_edited = {
        let mut b = bsl_before.clone();
        b.extend_from_slice("\n// правка модуля\n".as_bytes());
        b
    };
    let snap = w.history().join(&id);
    assert_eq!(w.ids(), std::slice::from_ref(&id));
    assert!(id.ends_with(&format!("-{FORM}")), "{id}");
    assert_eq!(fs::read(snap.join("before/Form.bin")).unwrap(), bin_before);
    assert_eq!(fs::read(snap.join("before/Form.xml")).unwrap(), xml_before);
    assert_eq!(fs::read(snap.join("before/FormModule.bsl")).unwrap(), bsl_before);
    assert_eq!(fs::read(snap.join("after/Form.xml")).unwrap(), xml_edited);
    assert_eq!(fs::read(snap.join("after/FormModule.bsl")).unwrap(), bsl_edited);
    assert_ne!(fs::read(w.bin()).unwrap(), bin_before, "the pack changed Form.bin");

    let m: serde_json::Value = serde_json::from_slice(&fs::read(snap.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["id"], id.as_str());
    assert_eq!(m["form"], FORM);
    assert_eq!(m["kind"], "config-form");
    assert_eq!(m["reason"], "pack");
    assert_eq!(m["binary"], format!("Forms/{FORM}/Ext/Form.bin"));
    assert_eq!(m["beforeSha256"], oneform::canonical::sha256_hex(&bin_before));
    assert_eq!(m["afterSha256"], oneform::canonical::sha256_hex(&fs::read(w.bin()).unwrap()));
    assert_eq!(m["oneformVersion"], env!("CARGO_PKG_VERSION"));
    assert!(m["time"].as_str().is_some_and(|t| t.len() == 19), "{m}");
    let summary = m["summary"].to_string();
    assert!(summary.contains("CommandBar") && summary.contains("FormModule.bsl"), "{summary}");

    // nothing of the history lands in src/, and the other object has none
    assert!(fs::read_dir(w.object().join("src")).unwrap().flatten().all(|e| e.file_name() != ".oneform"));
    assert!(!w.root().join("CommonForms/ОбщаяФорма/.oneform").exists());
    // the list shows it, in text and JSON
    let list = ok(&run(w.root(), &["history", "."]));
    assert!(list.contains(&id) && list.contains(OBJECT), "{list}");
    let json: serde_json::Value = serde_json::from_slice(&run(w.root(), &["history", ".", "--json"]).stdout).unwrap();
    assert_eq!(json[0]["id"], id.as_str());
    assert_eq!(json[0]["objectDir"], OBJECT);
}

#[test]
fn dry_run_and_noop_pack_keep_no_history() {
    let w = Work::config();
    w.edit_caption();
    let out = ok(&run(w.root(), &["pack", ".", "--dry-run"]));
    assert!(out.contains("WOULD PACK") && !out.contains("history"), "{out}");
    assert!(!w.history().exists());
    let noop = Work::config();
    let out = ok(&run(noop.root(), &["pack", "."]));
    assert!(out.contains("UNCHANGED") && !out.contains("history"), "{out}");
    assert!(!noop.history().exists());
}

#[test]
fn retention_keeps_the_last_twenty_and_only_its_own_directories() {
    let w = Work::config();
    let mut ids = Vec::new();
    fs::create_dir_all(w.history().join("notes")).unwrap();
    fs::create_dir_all(w.history().join("20200101-000000-nomanifest")).unwrap();
    for i in 0..25 {
        ids.push(w.edit_and_pack(&format!("edit {i}")).unwrap());
    }
    let left = w.ids();
    let own: Vec<&String> = left.iter().filter(|n| ids.contains(*n)).collect();
    assert_eq!(own.len(), 20, "{left:?}");
    assert_eq!(own.into_iter().cloned().collect::<Vec<_>>(), ids[5..].to_vec(), "the oldest five are gone");
    assert!(left.contains(&"notes".to_string()) && left.contains(&"20200101-000000-nomanifest".to_string()), "{left:?}");
}

#[test]
fn history_keep_variable_limits_or_disables() {
    let w = Work::config();
    for i in 0..4 {
        let p = w.src_form("FormModule.bsl");
        let mut bsl = fs::read(&p).unwrap();
        bsl.extend_from_slice(format!("\n// e{i}\n").as_bytes());
        fs::write(&p, bsl).unwrap();
        ok(&run_env(w.root(), &["pack", "."], &[("ONEFORM_HISTORY_KEEP", "2")]));
    }
    assert_eq!(w.ids().len(), 2);

    let off = Work::config();
    let p = off.src_form("FormModule.bsl");
    let mut bsl = fs::read(&p).unwrap();
    bsl.extend_from_slice(b"\n// off\n");
    fs::write(&p, bsl).unwrap();
    let out = ok(&run_env(off.root(), &["pack", "."], &[("ONEFORM_HISTORY_KEEP", "0")]));
    assert!(out.contains("PACKED") && !out.contains("history"), "{out}");
    assert!(!off.history().exists());
}

#[test]
fn restore_round_trip_returns_the_saved_bytes_and_is_itself_reversible() {
    let w = Work::config();
    let original = fs::read(w.bin()).unwrap();
    let id = w.edit_and_pack("правка").unwrap();
    let packed = fs::read(w.bin()).unwrap();
    assert_ne!(packed, original);

    let dry = ok(&run(w.root(), &["history", "restore", ".", &id, "--dry-run"]));
    assert!(dry.contains("WOULD RESTORE"), "{dry}");
    assert_eq!(fs::read(w.bin()).unwrap(), packed);
    assert_eq!(w.ids().len(), 1, "a dry run writes nothing");

    let out = ok(&run(w.root(), &["history", "restore", &id]));
    assert!(out.contains("RESTORED") && out.contains("src/ refreshed"), "{out}");
    assert_eq!(fs::read(w.bin()).unwrap(), original, "Form.bin is byte-equal to the original");
    let module = fs::read_to_string(w.src_form("FormModule.bsl")).unwrap();
    assert!(!module.contains("// правка"), "src/ was refreshed from the restored form");
    let ids = w.ids();
    assert_eq!(ids.len(), 2, "{ids:?}");
    let restore_id = ids.iter().find(|i| **i != id).unwrap().clone();
    let m: serde_json::Value = serde_json::from_slice(&fs::read(w.history().join(&restore_id).join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["reason"], "restore");
    assert_eq!(fs::read(w.history().join(&restore_id).join("before/Form.bin")).unwrap(), packed);

    // restoring the saved current state goes back to the packed bytes
    ok(&run(w.root(), &["history", "restore", w.object().to_str().unwrap(), &restore_id]));
    assert_eq!(fs::read(w.bin()).unwrap(), packed);
    assert!(fs::read_to_string(w.src_form("FormModule.bsl")).unwrap().contains("// правка"));
    // the refreshed src/ matches the restored form
    let check = run(w.root(), &["check", "."]);
    assert!(check.status.success(), "{}", text(&check));
}

#[test]
fn restore_refuses_unpacked_edits_and_unknown_ids() {
    let w = Work::config();
    let id = w.edit_and_pack("правка").unwrap();
    let packed = fs::read(w.bin()).unwrap();
    let p = w.src_form("FormModule.bsl");
    let mut bsl = fs::read(&p).unwrap();
    bsl.extend_from_slice(b"\n// not packed yet\n");
    fs::write(&p, &bsl).unwrap();

    let out = run(w.root(), &["history", "restore", ".", &id]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("unpacked edits") && text(&out).contains("--replace-generated"), "{}", text(&out));
    assert_eq!(fs::read(w.bin()).unwrap(), packed, "nothing was restored");
    assert_eq!(fs::read(&p).unwrap(), bsl, "the unpacked edit is untouched");
    assert_eq!(w.ids().len(), 1);

    let bad = run(w.root(), &["history", "restore", ".", "20990101-000000-Нет"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(text(&bad).contains("available:") && text(&bad).contains(&id), "{}", text(&bad));

    ok(&run(w.root(), &["history", "restore", ".", &id, "--replace-generated"]));
    assert_ne!(fs::read(w.bin()).unwrap(), packed);
    assert!(!fs::read_to_string(&p).unwrap().contains("not packed yet"));
}

#[test]
fn epf_pack_saves_the_previous_epf_and_restores_it() {
    let w = Work::epf();
    let original = fs::read(w.root().join("donor.epf")).unwrap();
    let module = fs::read_dir(w.root().join("src/Forms")).unwrap().flatten().next().unwrap().path().join("FormModule.bsl");
    let mut bsl = fs::read(&module).unwrap();
    bsl.extend_from_slice(b"\n// epf edit\n");
    fs::write(&module, bsl).unwrap();
    let out = ok(&run(w.root(), &["pack", ".", "--backup"]));
    let id = history_id(&out).expect(&out);
    assert!(w.root().join("donor.epf.bak").is_file(), "--backup still writes the .bak");
    let snap = w.root().join(".oneform/history").join(&id);
    assert_eq!(fs::read(snap.join("before/donor.epf")).unwrap(), original);
    assert!(snap.join("before/FormModule.bsl").is_file() && snap.join("after/FormModule.bsl").is_file());
    let m: serde_json::Value = serde_json::from_slice(&fs::read(snap.join("manifest.json")).unwrap()).unwrap();
    assert_eq!((m["kind"].as_str(), m["form"].as_str(), m["binary"].as_str()), (Some("epf"), Some("donor"), Some("donor.epf")));
    assert_ne!(fs::read(w.root().join("donor.epf")).unwrap(), original);

    // the snapshot's .epf is not an extraction target and does not turn the layout into a multi-EPF one
    let again = ok(&run(w.root(), &["extract", "."]));
    assert!(again.contains("Batch: 1 targets"), "{again}");

    ok(&run(w.root(), &["history", "restore", ".", &id]));
    assert_eq!(fs::read(w.root().join("donor.epf")).unwrap(), original);
    assert!(!fs::read_to_string(&module).unwrap().contains("// epf edit"));
}

#[test]
fn snapshots_are_not_forms_for_discovery_validate_lint_or_extract() {
    let w = Work::config();
    w.edit_caption();
    ok(&run(w.root(), &["pack", "."]));
    assert_eq!(w.ids().len(), 1);
    let validate = text(&run(w.root(), &["validate", ".", "--json"]));
    assert!(!validate.contains(".oneform"), "{validate}");
    let lint = text(&run(w.root(), &["lint", ".", "--json"]));
    assert!(!lint.contains(".oneform"), "{lint}");
    let extract = ok(&run(w.root(), &["extract", "."]));
    assert!(extract.contains("Batch: 2 targets") && !extract.contains(".oneform/history"), "{extract}");
    let check = ok(&run(w.root(), &["check", "."]));
    assert!(check.contains("Batch: 2 targets"), "{check}");
    let preview = ok(&run(w.root(), &["preview", "."]));
    assert!(!preview.contains("history"), "{preview}");
    // clone-form works on an object that has a history
    let cloned = Command::new(env!("CARGO_BIN_EXE_oneform"))
        .args(["clone-form", w.object().to_str().unwrap(), "--from", FORM, "--name", "Копия"])
        .output()
        .unwrap();
    ok(&cloned);
    assert_eq!(w.ids().len(), 1);
    let again = ok(&run(w.root(), &["extract", "."]));
    assert!(again.contains("Batch: 2 targets"), "{again}");
}

#[test]
fn history_survives_the_viewer_rebuild_when_the_object_is_the_root() {
    let w = Work::config();
    let object = w.object();
    ok(&run(&object, &["extract", "."]));
    let p = object.join("src/Forms").join(FORM).join("FormModule.bsl");
    let mut bsl = fs::read(&p).unwrap();
    bsl.extend_from_slice(b"\n// root is the object\n");
    fs::write(&p, bsl).unwrap();
    ok(&run(&object, &["pack", "."]));
    assert!(object.join(".oneform/index.html").is_file(), "the viewer is next to the history");
    assert_eq!(fs::read_dir(object.join(".oneform/history")).unwrap().count(), 1);
    // viewer rebuilds keep the snapshots
    ok(&run(&object, &["extract", "."]));
    ok(&run(&object, &["preview", "."]));
    assert_eq!(fs::read_dir(object.join(".oneform/history")).unwrap().count(), 1);
    // a .oneform that holds only history is not a foreign directory
    for e in fs::read_dir(object.join(".oneform")).unwrap().flatten() {
        if e.file_name() != "history" {
            let p = e.path();
            if p.is_dir() {
                fs::remove_dir_all(p).unwrap();
            } else {
                fs::remove_file(p).unwrap();
            }
        }
    }
    ok(&run(&object, &["extract", "."]));
    assert!(object.join(".oneform/index.html").is_file());
    assert_eq!(fs::read_dir(object.join(".oneform/history")).unwrap().count(), 1);
}

#[test]
fn a_snapshot_failure_does_not_fail_or_hide_the_pack() {
    let w = Work::config();
    // `.oneform` is a file next to the object: the history directory cannot be created
    fs::write(w.object().join(".oneform"), b"in the way").unwrap();
    w.edit_caption();
    let before = fs::read(w.bin()).unwrap();
    let out = ok(&run(w.root(), &["pack", "."]));
    assert!(out.contains("PACKED") && out.contains("warning: history snapshot not saved"), "{out}");
    assert!(history_id(&out).is_none(), "{out}");
    assert_ne!(fs::read(w.bin()).unwrap(), before, "the pack itself succeeded");
}

#[test]
fn history_usage_errors() {
    let w = Work::config();
    let out = run(w.root(), &["history", "restore"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let out = run(w.root(), &["history", ".", "--dry-run"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let out = run(w.root(), &["pack", ".", "extra"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let out = ok(&run(w.root(), &["history", "."]));
    assert!(out.contains("no history"), "{out}");
}
