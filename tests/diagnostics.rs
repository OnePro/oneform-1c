//! Diagnostics log (`.oneform/diagnostics.jsonl`), `oneform history diff`, `oneform diagnose` and `oneform feedback`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

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
    c.args(args).args(experimental(args)).arg("-j").arg("1").arg("--no-progress").current_dir(root);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// `pack`, `new-form` and `clone-form` may write the form layout: they need `--experimental` (the gate itself is tested in
/// experimental_gate.rs).
fn experimental(args: &[&str]) -> Option<&'static str> {
    matches!(args.first(), Some(&("pack" | "new-form" | "clone-form"))).then_some("--experimental")
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

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn object(&self) -> PathBuf {
        self.root().join(OBJECT)
    }

    fn diag(&self) -> PathBuf {
        self.object().join(".oneform/diagnostics.jsonl")
    }

    fn xml(&self) -> PathBuf {
        self.object().join("src/Forms").join(FORM).join("Form.xml")
    }

    fn lines(&self) -> Vec<Value> {
        fs::read_to_string(self.diag()).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    /// An edit that `pack` refuses.
    fn break_form(&self) {
        let xml = fs::read_to_string(self.xml()).unwrap();
        fs::write(self.xml(), xml.replace("representation=\"Text\" actionKind=\"Handler\"", "representation=\"Text\" actionKind=\"Bogus\"")).unwrap();
    }

    fn edit_caption(&self) {
        let xml = fs::read_to_string(self.xml()).unwrap();
        let xml = xml
            .replace("<Item language=\"ru\">Выполнить</Item>", "<Item language=\"ru\">Запустить</Item>")
            .replace("<Item language=\"ro\">Выполнить</Item>", "<Item language=\"ro\">Запустить</Item>");
        fs::write(self.xml(), xml).unwrap();
    }

    /// Change the caption and pack; the history id.
    fn pack_caption(&self) -> String {
        self.edit_caption();
        history_id(&ok(&run(self.root(), &["pack", "."]))).expect("history id")
    }
}

#[test]
fn a_refusal_is_logged_a_clean_pack_is_not_and_no_contents_leak() {
    let w = Work::config();
    let id = w.pack_caption();
    assert!(!w.diag().exists(), "a clean pack writes no diagnostics");
    assert!(!id.is_empty());

    w.break_form();
    let out = run(w.root(), &["pack", "."]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    let lines = w.lines();
    assert_eq!(lines.len(), 1);
    let e = &lines[0];
    assert_eq!(e["command"], "pack");
    assert_eq!(e["exit_code"], 1);
    assert_eq!(e["code"], "METADATA_LAYOUT_UNSUPPORTED");
    assert_eq!(e["kind"], "verification-failed");
    assert_eq!(e["form"], FORM);
    assert_eq!(e["target"], OBJECT);
    assert_eq!(e["dry_run"], false);
    assert_eq!(e["oneform_version"], env!("CARGO_PKG_VERSION"));
    assert!(e["time"].as_str().is_some_and(|t| t.len() == 19));
    assert_eq!(e["args"][0], "pack");
    let raw = fs::read_to_string(w.diag()).unwrap();
    for secret in ["Запустить", "Bogus", "Процедура", "КнопкаВыполнитьНажатие"] {
        assert!(!raw.contains(secret), "the log holds file contents: {secret}\n{raw}");
    }
    assert!(!raw.contains(&w.root().to_string_lossy().to_string()), "an absolute path in the log: {raw}");

    // a dry run records the refusal with dry_run: true
    run(w.root(), &["pack", ".", "--dry-run"]);
    let lines = w.lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["dry_run"], true);

    // a successful pack afterwards adds nothing
    fs::write(w.xml(), fs::read_to_string(w.xml()).unwrap().replace("actionKind=\"Bogus\"", "actionKind=\"Handler\"")).unwrap();
    ok(&run(w.root(), &["pack", "."]));
    assert_eq!(w.lines().len(), 2);
}

#[test]
fn the_log_is_capped_and_diagnostics_are_ignored_by_discovery() {
    let w = Work::config();
    w.break_form();
    for _ in 0..5 {
        run_env(w.root(), &["pack", "."], &[("ONEFORM_DIAG_CAP", "3")]);
    }
    assert_eq!(w.lines().len(), 3);

    // forms and logs inside .oneform are not forms of the project
    let fb = w.object().join(".oneform/feedback/20260101-000000-bug/files/before");
    fs::create_dir_all(&fb).unwrap();
    fs::copy(w.xml(), fb.join("Form.xml")).unwrap();
    let validate: Value = serde_json::from_slice(&run(w.root(), &["validate", ".", "--json"]).stdout).unwrap();
    assert_eq!(validate.as_array().unwrap().len(), 2, "{validate}");
    let lint = run(w.root(), &["lint", ".", "--json"]);
    assert!(!text(&lint).contains(".oneform"), "{}", text(&lint));
    let preview = ok(&run(w.root(), &["preview", "."]));
    assert!(!preview.contains(".oneform/feedback"), "{preview}");
    // extract and the viewer rebuild keep both
    ok(&run(w.root(), &["extract", ".", "--replace-generated"]));
    assert!(w.diag().is_file());
    assert!(fb.join("Form.xml").is_file());
    let lines_before = w.lines().len();
    ok(&run(w.root(), &["extract", "."]));
    assert_eq!(w.lines().len(), lines_before);
}

#[test]
fn an_unwritable_oneform_never_changes_the_result() {
    let w = Work::config();
    fs::write(w.object().join(".oneform"), "a file, not a directory").unwrap();
    w.break_form();
    let out = run(w.root(), &["pack", "."]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("METADATA_LAYOUT_UNSUPPORTED"), "{}", text(&out));
    assert!(w.object().join(".oneform").is_file());
    let dry = run(w.root(), &["pack", ".", "--dry-run"]);
    assert_eq!(dry.status.code(), Some(1));
    w.edit_caption();
    fs::write(w.xml(), fs::read_to_string(w.xml()).unwrap().replace("actionKind=\"Bogus\"", "actionKind=\"Handler\"")).unwrap();
    let packed = run(w.root(), &["pack", "."]);
    assert!(packed.status.success(), "{}", text(&packed));
}

#[test]
fn history_diff_shows_the_changed_caption() {
    let w = Work::config();
    let none = run(w.root(), &["history", "diff", "."]);
    assert_eq!(none.status.code(), Some(1));
    assert!(text(&none).contains("no history"), "{}", text(&none));

    let id = w.pack_caption();
    let out = ok(&run(w.root(), &["history", "diff"]));
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with(&format!("snapshot {id} ({FORM}, {OBJECT}, ")), "{}", lines[0]);
    assert_eq!(lines[1], "--- before/Form.xml");
    assert_eq!(lines[2], "+++ after/Form.xml");
    assert_eq!(lines[3], "@@ -121,8 +121,8 @@");
    assert_eq!(lines[7], "-                <Item language=\"ru\">Выполнить</Item>");
    assert_eq!(lines[8], "-                <Item language=\"ro\">Выполнить</Item>");
    assert_eq!(lines[9], "+                <Item language=\"ru\">Запустить</Item>");
    assert_eq!(lines[10], "+                <Item language=\"ro\">Запустить</Item>");
    assert_eq!(lines.len(), 14, "{out}");
    // the same through the object path, an exact id and a unique prefix
    let body = |t: &str| t.lines().skip(1).collect::<Vec<_>>().join("
");
    assert_eq!(body(&ok(&run(w.root(), &["history", "diff", OBJECT, &id]))), body(&out));
    assert_eq!(body(&ok(&run(w.root(), &["history", "diff", ".", &id[..10]]))), body(&out));
    let missing = run(w.root(), &["history", "diff", ".", "2000"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(text(&missing).contains("no history snapshot '2000'"), "{}", text(&missing));
    let bad = run(w.root(), &["history", "diff", ".", "--json"]);
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn diagnose_summarizes_in_text_and_json() {
    let w = Work::config();
    let quiet = ok(&run(w.root(), &["diagnose", "."]));
    assert!(quiet.contains("Последние записи диагностики (0)") && quiet.contains("Последний снимок истории: нет"), "{quiet}");

    let id = w.pack_caption();
    w.break_form();
    run(w.root(), &["pack", "."]);
    let human = ok(&run(w.root(), &["diagnose", "."]));
    assert!(human.contains("pack exit 1") && human.contains("METADATA_LAYOUT_UNSUPPORTED/verification-failed"), "{human}");
    assert!(human.contains(&format!("Последний снимок истории: {id}")), "{human}");
    assert!(human.contains("Documents/ДокументТест/ФормаДокумента: IN_SCOPE") || human.contains("OUT_OF_SCOPE"), "{human}");
    assert!(human.contains("docs/agent-guide.md") && human.contains("раздел «Область применимости: `oneform validate`»"), "{human}");
    assert!(human.contains("Форма не открылась в 1С:"), "{human}");
    assert!(human.contains(&format!("oneform history diff {id}")), "{human}");
    assert!(human.contains(&format!("oneform history restore {id}")), "{human}");
    assert!(human.contains(&format!("oneform feedback add --history {id} --1c-error \"<текст ошибки 1С>\"")), "{human}");

    let json: Value = serde_json::from_slice(&run(w.root(), &["diagnose", ".", "--json"]).stdout).unwrap();
    assert_eq!(json["diagnostics"][0]["code"], "METADATA_LAYOUT_UNSUPPORTED");
    assert_eq!(json["last_snapshot"]["id"], id.as_str());
    assert_eq!(json["last_snapshot"]["form"], FORM);
    assert_eq!(json["hints"][0]["doc"], "docs/agent-guide.md");
    assert_eq!(json["checklist"]["commands"][1], format!("oneform history restore {id}"));
    assert!(json["validate"].as_array().is_some_and(|v| v.len() == 2), "{json}");
    // read-only
    let before = fs::read_to_string(w.diag()).unwrap();
    run(w.root(), &["diagnose", "."]);
    assert_eq!(fs::read_to_string(w.diag()).unwrap(), before);
    assert_eq!(run(w.root(), &["diagnose", ".", "--dry-run"]).status.code(), Some(2));
}

#[test]
fn feedback_add_list_show_and_mirror() {
    let w = Work::config();
    let id = w.pack_caption();
    let mirror = tempfile::tempdir().unwrap();
    let mirror_path = mirror.path().to_string_lossy().to_string();
    let out = ok(&run_env(
        w.root(),
        &["feedback", "add", ".", "--history", &id, "--1c-error", "Ошибка формата потока: строка 7", "--note", "добавил кнопку"],
        &[("ONEFORM_FEEDBACK_DIR", &mirror_path)],
    ));
    assert!(out.contains("tokens or passwords"), "{out}");
    let fb_dir = w.object().join(".oneform/feedback");
    let reports: Vec<String> = fs::read_dir(&fb_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(reports.len(), 1);
    let rid = &reports[0];
    assert!(rid.ends_with("-bug") && rid.len() == "20260101-000000-bug".len(), "{rid}");
    let dir = fb_dir.join(rid);
    let md = fs::read_to_string(dir.join("report.md")).unwrap();
    for part in ["## Ошибка 1С", "Ошибка формата потока: строка 7", "добавил кнопку", "## Что изменил pack", "+                <Item language=\"ru\">Запустить</Item>", "## Окружение"] {
        assert!(md.contains(part), "{part}\n{md}");
    }
    let json: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(json["1c_error"], "Ошибка формата потока: строка 7");
    assert_eq!(json["note"], "добавил кнопку");
    assert_eq!(json["kind"], "bug");
    assert_eq!(json["history"], id.as_str());
    assert_eq!(json["snapshot"]["form"], FORM);
    assert_eq!(json["versions"]["oneform"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["environment"]["oneform_config_root_set"], false);
    assert!(json["validate"].as_array().is_some_and(|v| v.len() == 1));
    assert!(json["lint"].is_array() && json["diagnostics"].is_array());
    for f in ["files/before/Form.xml", "files/after/Form.xml", "files/before/FormModule.bsl", "files/after/FormModule.bsl"] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    assert!(!dir.join("files/before/Form.bin").exists(), "binaries only with --include-binaries");
    let mirrored = mirror.path().join("ДокументТест").join(rid);
    assert!(mirrored.join("report.md").is_file(), "{out}");
    assert!(out.contains(&mirrored.to_string_lossy().to_string()), "{out}\n{}", mirrored.display());

    // --include-binaries and --no-files, --kind wish
    std::thread::sleep(std::time::Duration::from_millis(1100));
    ok(&run(w.root(), &["feedback", "add", OBJECT, "--include-binaries", "--kind", "wish"]));
    let wish = fs::read_dir(&fb_dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).find(|n| n.ends_with("-wish")).unwrap();
    assert!(fb_dir.join(&wish).join("files/before/Form.bin").is_file());
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let none_text = ok(&run(w.root(), &["feedback", "add", OBJECT, "--no-files"]));
    assert!(!none_text.contains("tokens or passwords"), "{none_text}");
    assert_eq!(fs::read_dir(&fb_dir).unwrap().flatten().filter(|e| e.path().join("report.md").is_file()).count(), 3);

    // list and show
    let list = ok(&run(w.root(), &["feedback", "list", "."]));
    assert!(list.contains(rid.as_str()) && list.contains("Ошибка формата потока: строка 7") && list.contains(&id), "{list}");
    let list_json: Value = serde_json::from_slice(&run(w.root(), &["feedback", "list", ".", "--json"]).stdout).unwrap();
    assert_eq!(list_json.as_array().unwrap().len(), 3);
    let shown = ok(&run(w.root(), &["feedback", "show", rid]));
    assert_eq!(shown, md);
    let shown_json: Value = serde_json::from_slice(&run(w.root(), &["feedback", "show", ".", &rid[..15], "--json"]).stdout).unwrap();
    assert_eq!(shown_json["id"], rid.as_str());
    assert_eq!(run(w.root(), &["feedback", "show"]).status.code(), Some(2));
    assert_eq!(run(w.root(), &["feedback", "show", ".", "nope"]).status.code(), Some(1));
}

#[test]
fn feedback_usage_errors() {
    let w = Work::config();
    for args in [
        vec!["feedback"],
        vec!["feedback", "bogus"],
        vec!["feedback", "add", ".", "--kind", "idea"],
        vec!["feedback", "list", ".", "--note", "x"],
        vec!["pack", ".", "--note", "x"],
        vec!["feedback", "add", ".", "--json"],
    ] {
        let o = run(w.root(), &args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", text(&o));
    }
    let o = run(w.root(), &["feedback", "add", ".", "--history", "2000"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o).contains("no history snapshot '2000'"), "{}", text(&o));
    // without any snapshot a report is still possible
    let o = ok(&run(w.root(), &["feedback", "add", OBJECT, "--1c-error", "x"]));
    assert!(o.contains("no history snapshot"), "{o}");
}
