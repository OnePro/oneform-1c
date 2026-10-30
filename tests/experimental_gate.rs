//! The experimental gate: `pack` writes only module text unless `--experimental` is given; `new-form` and `clone-form`
//! need it; the mode is recorded in the history manifest, the diagnostics log, `diagnose` and `feedback add`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const OBJECT: &str = "Documents/ДокументТест";
const FORM: &str = "ФормаДокумента";
const DRY_LINE: &str = "EXPERIMENTAL: this plan changes the form stream; a real pack needs --experimental";
const DONE_LINE: &str = "EXPERIMENTAL: the form stream was rewritten from Form.xml; open the form in 1C and check it";

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

/// Every file below `dir` (relative path -> bytes).
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

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn object(&self) -> PathBuf {
        self.root().join(OBJECT)
    }

    fn form_dir(&self) -> PathBuf {
        self.object().join("src/Forms").join(FORM)
    }

    fn xml(&self) -> PathBuf {
        self.form_dir().join("Form.xml")
    }

    fn module(&self) -> PathBuf {
        self.form_dir().join("FormModule.bsl")
    }

    fn diag(&self) -> PathBuf {
        self.object().join(".oneform/diagnostics.jsonl")
    }

    fn history(&self) -> PathBuf {
        self.object().join(".oneform/history")
    }

    fn form_bin(&self) -> String {
        format!("{OBJECT}/Forms/{FORM}/Ext/Form.bin")
    }

    fn edit_caption(&self) {
        let old = fs::read_to_string(self.xml()).unwrap();
        let xml = old
            .replace("<Item language=\"ru\">Выполнить</Item>", "<Item language=\"ru\">Запустить</Item>")
            .replace("<Item language=\"ro\">Выполнить</Item>", "<Item language=\"ro\">Запустить</Item>");
        assert_ne!(xml, old);
        fs::write(self.xml(), xml).unwrap();
    }

    fn edit_module(&self) {
        let mut m = fs::read_to_string(self.module()).unwrap();
        m.push_str("\r\n// проверка шлюза\r\n");
        fs::write(self.module(), m).unwrap();
    }

    fn lines(&self) -> Vec<Value> {
        fs::read_to_string(self.diag()).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    /// The manifests of the history snapshots of the object.
    fn manifests(&self) -> Vec<Value> {
        let Ok(rd) = fs::read_dir(self.history()) else { return Vec::new() };
        rd.map(|e| serde_json::from_slice(&fs::read(e.unwrap().path().join("manifest.json")).unwrap()).unwrap()).collect()
    }
}

#[test]
fn a_module_only_pack_works_without_the_flag() {
    let w = Work::config();
    let before = tree(w.root());
    w.edit_module();
    let out = ok(&run(w.root(), &["pack", "."]));
    assert!(out.contains("PACKED") && out.contains("pack Forms/ФормаДокумента/FormModule.bsl"), "{out}");
    assert!(!out.contains("EXPERIMENTAL"), "{out}");
    assert_ne!(tree(w.root())[&w.form_bin()], before[&w.form_bin()], "the module text is written into Form.bin");
    let manifests = w.manifests();
    assert_eq!(manifests.len(), 1);
    assert!(manifests[0]["experimental"].as_bool() != Some(true), "{}", manifests[0]);
    // the flag does not mark a module-only pack either: nothing experimental happened
    let mut m = fs::read_to_string(w.module()).unwrap();
    m.push_str("// ещё\r\n");
    fs::write(w.module(), m).unwrap();
    let out = ok(&run(w.root(), &["pack", ".", "--experimental"]));
    assert!(out.contains("PACKED") && !out.contains("EXPERIMENTAL"), "{out}");
    assert!(w.manifests().iter().all(|m| m["experimental"].as_bool() != Some(true)));
}

#[test]
fn a_form_xml_edit_without_the_flag_is_refused_and_nothing_is_written() {
    let w = Work::config();
    w.edit_caption();
    let before = tree(w.root());
    let out = run(w.root(), &["pack", ".", "--backup"]);
    let t = text(&out);
    assert_eq!(out.status.code(), Some(2), "{t}");
    assert!(t.contains("EXPERIMENTAL_REQUIRED") && t.contains("--experimental"), "{t}");
    assert!(t.contains("pack Forms/ФормаДокумента/Form.xml") && t.contains("CommandBar 'ОсновныеДействияФормы'"), "the plan is listed: {t}");
    assert!(t.contains("explicit confirmation"), "{t}");
    let after = tree(w.root());
    // the only new file is the diagnostics line
    let added: Vec<&String> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    assert_eq!(added, [&format!("{OBJECT}/.oneform/diagnostics.jsonl")], "{added:?}");
    assert!(!t.contains("Viewer:"), "{t}");
    for (k, v) in &before {
        assert_eq!(after.get(k), Some(v), "{k} changed");
    }
    assert!(!w.history().exists(), "no history snapshot");
    assert!(after.keys().all(|k| !k.ends_with(".bak")), "a .bak was written");
}

#[test]
fn a_dry_run_works_without_the_flag_and_marks_the_plan() {
    let w = Work::config();
    w.edit_caption();
    let before = tree(w.root());
    let out = ok(&run(w.root(), &["pack", ".", "--dry-run"]));
    assert!(out.contains("WOULD PACK") && out.contains(DRY_LINE), "{out}");
    assert_eq!(tree(w.root()), before, "a dry run writes nothing");
    // a module-only dry run has no mark
    let m = Work::config();
    m.edit_module();
    let out = ok(&run(m.root(), &["pack", ".", "--dry-run"]));
    assert!(out.contains("WOULD PACK") && !out.contains("EXPERIMENTAL"), "{out}");
}

#[test]
fn with_the_flag_the_pack_is_marked_everywhere() {
    let w = Work::config();
    w.edit_caption();
    let out = ok(&run(w.root(), &["pack", ".", "--experimental"]));
    assert!(out.contains("PACKED") && out.contains(DONE_LINE), "{out}");
    let manifests = w.manifests();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0]["experimental"], true);
    let id = manifests[0]["id"].as_str().unwrap().to_string();

    // diagnose and history show it
    let diag = ok(&run(w.root(), &["diagnose", "."]));
    assert!(diag.contains(&id) && diag.contains("experimental"), "{diag}");
    let json: Value = serde_json::from_str(&ok(&run(w.root(), &["diagnose", ".", "--json"]))).unwrap();
    assert_eq!(json["last_snapshot"]["experimental"], true, "{json}");
    let hist = ok(&run(w.root(), &["history", "."]));
    assert!(hist.contains("[experimental]"), "{hist}");
    let hist: Value = serde_json::from_str(&ok(&run(w.root(), &["history", ".", "--json"]))).unwrap();
    assert_eq!(hist[0]["experimental"], true);

    // feedback add reports the mode of the snapshot
    let fb = ok(&run(w.root(), &["feedback", "add", ".", "--no-files"]));
    let dir = fb
        .lines()
        .find_map(|l| l.strip_prefix("feedback "))
        .and_then(|r| r.split_once(": "))
        .map(|(_, d)| PathBuf::from(d.trim()))
        .unwrap();
    let report: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["experimental"], true);
    assert!(fs::read_to_string(dir.join("report.md")).unwrap().contains("experimental: true"));
}

#[test]
fn an_old_manifest_without_the_field_is_readable() {
    let w = Work::config();
    w.edit_caption();
    ok(&run(w.root(), &["pack", ".", "--experimental"]));
    let snap = fs::read_dir(w.history()).unwrap().next().unwrap().unwrap().path();
    let mut m: Value = serde_json::from_slice(&fs::read(snap.join("manifest.json")).unwrap()).unwrap();
    m.as_object_mut().unwrap().remove("experimental");
    fs::write(snap.join("manifest.json"), serde_json::to_vec_pretty(&m).unwrap()).unwrap();
    let json: Value = serde_json::from_str(&ok(&run(w.root(), &["diagnose", ".", "--json"]))).unwrap();
    assert_eq!(json["last_snapshot"]["experimental"], false);
    ok(&run(w.root(), &["history", "."]));
}

#[test]
fn a_mixed_plan_is_refused_as_a_whole() {
    let w = Work::config();
    w.edit_caption();
    w.edit_module();
    let before = tree(w.root());
    let out = run(w.root(), &["pack", "."]);
    let t = text(&out);
    assert_eq!(out.status.code(), Some(2), "{t}");
    assert!(t.contains("EXPERIMENTAL_REQUIRED") && t.contains("FormModule.bsl") && t.contains("Form.xml"), "{t}");
    assert_eq!(tree(w.root())[&w.form_bin()], before[&w.form_bin()], "the module text is not written either");
    assert!(!w.history().exists());
    // with the flag the same plan is written as a whole
    let out = ok(&run(w.root(), &["pack", ".", "--experimental"]));
    assert!(out.contains(DONE_LINE) && out.contains("FormModule.bsl"), "{out}");
    assert_ne!(tree(w.root())[&w.form_bin()], before[&w.form_bin()]);
}

#[test]
fn new_form_and_clone_form_need_the_flag() {
    let w = Work::config();
    let o = w.object();
    let obj = o.to_str().unwrap();
    let before = tree(w.root());
    for args in [
        vec!["clone-form", obj, "--from", FORM, "--name", "Копия"],
        vec!["new-form", obj, "--kind", "document", "--name", "Нов"],
    ] {
        let out = run(w.root(), &args);
        let t = text(&out);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {t}");
        assert!(t.contains("EXPERIMENTAL_REQUIRED") && t.contains("--experimental"), "{t}");
    }
    let after = tree(w.root());
    for (k, v) in &before {
        assert_eq!(after.get(k), Some(v), "{k} changed");
    }
    assert!(!o.join("Forms/Копия.xml").exists() && !o.join("Forms/Нов.xml").exists());
    let lines = w.lines();
    assert!(
        lines.iter().any(|l| l["command"] == "clone-form" && l["code"] == "EXPERIMENTAL_REQUIRED" && l["exit_code"] == 2),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l["command"] == "new-form" && l["code"] == "EXPERIMENTAL_REQUIRED"), "{lines:?}");

    ok(&run(w.root(), &["clone-form", obj, "--from", FORM, "--name", "Копия", "--experimental"]));
    assert!(o.join("Forms/Копия.xml").is_file());
    // new-form with the flag gets past the gate (its success is covered by tests/new_form.rs); this fixture has no object types
    let out = run(w.root(), &["new-form", obj, "--kind", "document", "--name", "Нов", "--experimental"]);
    assert!(!text(&out).contains("EXPERIMENTAL_REQUIRED"), "{}", text(&out));
}

#[test]
fn the_flag_is_a_usage_error_for_other_commands() {
    let w = Work::config();
    for command in ["extract", "check", "preview", "lint", "validate", "history", "diagnose", "feedback"] {
        let out = run(w.root(), &[command, ".", "--experimental"]);
        assert_eq!(out.status.code(), Some(2), "{command}: {}", text(&out));
        assert!(text(&out).contains("--experimental"), "{command}: {}", text(&out));
    }
    let out = run(w.root(), &["history", "restore", ".", "20000101-000000-x", "--experimental"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
}

#[test]
fn a_refusal_is_recorded_in_diagnostics_with_the_code() {
    let w = Work::config();
    w.edit_caption();
    let out = run(w.root(), &["pack", "."]);
    assert_eq!(out.status.code(), Some(2));
    let lines = w.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let e = &lines[0];
    assert_eq!(e["command"], "pack");
    assert_eq!(e["code"], "EXPERIMENTAL_REQUIRED");
    assert_eq!(e["exit_code"], 2);
    assert_eq!(e["experimental"], false);
    assert_eq!(e["target"], OBJECT);
    assert!(e["message"].as_str().unwrap().contains("--experimental"));
    let diag = ok(&run(w.root(), &["diagnose", "."]));
    assert!(diag.contains("EXPERIMENTAL_REQUIRED"), "{diag}");
    // the same pack with the flag: a success writes no diagnostics line, the snapshot says experimental
    ok(&run(w.root(), &["pack", ".", "--experimental"]));
    assert_eq!(w.lines().len(), 1);
    assert_eq!(w.manifests()[0]["experimental"], true);
}

#[test]
fn a_failed_experimental_pack_is_logged_as_experimental() {
    let w = Work::config();
    let xml = fs::read_to_string(w.xml()).unwrap();
    let broken = xml.replace("representation=\"Text\" actionKind=\"Handler\"", "representation=\"Text\" actionKind=\"Bogus\"");
    assert_ne!(xml, broken);
    fs::write(w.xml(), broken).unwrap();
    let out = run(w.root(), &["pack", ".", "--experimental"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    let lines = w.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["experimental"], true);
}

#[test]
fn help_groups_stable_and_experimental() {
    let out = Command::new(env!("CARGO_BIN_EXE_oneform")).arg("--help").output().unwrap();
    let t = text(&out);
    assert!(out.status.success());
    assert!(t.contains("STABLE (no flag needed)") && t.contains("EXPERIMENTAL (needs --experimental)"), "{t}");
    assert!(t.contains("EXPERIMENTAL_REQUIRED") && t.contains("--experimental"), "{t}");
}
