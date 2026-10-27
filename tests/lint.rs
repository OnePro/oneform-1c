//! `oneform lint`: layout findings of synthetic extracted forms, JSON output, exit codes and the text format.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// One control as the extractor writes it: opening tag on its own line, geometry and anchors below.
struct Ctl {
    tag: &'static str,
    name: String,
    id: u32,
    rect: [i32; 4],
    tab: u32,
    anchors: String,
    caption: Option<&'static str>,
}

impl Ctl {
    fn new(tag: &'static str, name: &str, id: u32, rect: [i32; 4]) -> Ctl {
        Ctl { tag, name: name.to_string(), id, rect, tab: id, anchors: String::new(), caption: None }
    }
    fn tab(mut self, tab: u32) -> Ctl {
        self.tab = tab;
        self
    }
    fn anchors(mut self, a: &str) -> Ctl {
        self.anchors = a.to_string();
        self
    }
    fn caption(mut self, c: &'static str) -> Ctl {
        self.caption = Some(c);
        self
    }
    fn xml(&self) -> String {
        let [l, t, w, h] = self.rect;
        let mut s = format!("    <{} name=\"{}\" id=\"{}\">\n", self.tag, self.name, self.id);
        if let Some(c) = self.caption {
            s += &format!("      <Caption><Item language=\"ru\">{c}</Item></Caption>\n");
        }
        s += &format!("      <Left>{l}</Left>\n      <Top>{t}</Top>\n      <Width>{w}</Width>\n      <Height>{h}</Height>\n");
        s += &format!("      <ZOrder>{}</ZOrder>\n      <TabOrder>{}</TabOrder>\n", self.id, self.tab);
        if !self.anchors.is_empty() {
            s += &format!("      <Anchors {}/>\n", self.anchors);
        }
        s + &format!("    </{}>\n", self.tag)
    }
}

fn top_bar(id: u32, w: i32) -> Ctl {
    Ctl::new("CommandBar", "ДействияФормы", id, [0, 0, w, 25])
        .tab(1)
        .anchors(&format!("bottom=\"Top\" bottomTarget=\"{id}\" bottomOffset=\"25\" left=\"Left\" right=\"Right\""))
}

fn bottom_bar(id: u32, w: i32, h: i32) -> Ctl {
    Ctl::new("CommandBar", "ОсновныеДействияФормы", id, [0, h - 25, w, 25])
        .tab(99)
        .anchors(&format!("top=\"Bottom\" topTarget=\"{id}\" topOffset=\"-25\" bottom=\"Bottom\" left=\"Left\" right=\"Right\""))
}

fn label(id: u32, name: &str, rect: [i32; 4]) -> Ctl {
    Ctl::new("LabelDecoration", name, id, rect).caption("Подпись:")
}

fn field(id: u32, name: &str, rect: [i32; 4]) -> Ctl {
    Ctl::new("InputField", name, id, rect)
}

fn stretch(offset: i32) -> String {
    format!("right=\"Right\" rightOffset=\"{offset}\"")
}

fn form_xml(w: i32, h: i32, ctls: &[Ctl]) -> String {
    let body: String = ctls.iter().map(Ctl::xml).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Form id=\"11111111-2222-3333-4444-555555555555\" name=\"ФормаЭлемента\" ordinaryFormVersion=\"2.2\" reconstructionComplete=\"false\">\n  <Width>{w}</Width>\n  <Height>{h}</Height>\n  <MainAttribute attributeId=\"0\"/>\n  <Attributes>\n    <Attribute id=\"0\" name=\"СправочникОбъект\">\n      <Main>true</Main>\n    </Attribute>\n  </Attributes>\n  <ChildItems>\n{body}  </ChildItems>\n</Form>\n"
    )
}

/// A workspace `<family>/Тест/src/Forms/<form>/Form.xml`; returns (guard, root).
fn workspace(family: &str, form: &str, xml: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(family).join("Тест").join("src").join("Forms").join(form);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("Form.xml"), xml).unwrap();
    let root = tmp.path().to_path_buf();
    (tmp, root)
}

fn lint(root: &Path, extra: &[&str]) -> (i32, String, String) {
    let mut args = vec!["lint", root.to_str().unwrap()];
    args.extend_from_slice(extra);
    let out = Command::new(env!("CARGO_BIN_EXE_oneform")).args(&args).output().unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn line_of(xml: &str, name: &str) -> usize {
    xml.lines().position(|l| l.contains(&format!("name=\"{name}\""))).unwrap() + 1
}

/// A well-formed catalog item form 400 x 200: bars, one labelled field that stretches.
fn clean_item() -> Vec<Ctl> {
    vec![
        top_bar(1, 400),
        bottom_bar(2, 400, 200),
        label(3, "НадписьНаименование", [8, 33, 84, 19]),
        field(4, "Наименование", [92, 33, 300, 19]).anchors(&stretch(-8)),
    ]
}

fn findings(root: &Path, extra: &[&str]) -> Vec<Value> {
    let mut args = vec!["--json"];
    args.extend_from_slice(extra);
    let (_, out, err) = lint(root, &args);
    serde_json::from_str::<Value>(&out).unwrap_or_else(|e| panic!("not JSON: {e}\n{out}\n{err}")).as_array().unwrap().clone()
}

fn rules(f: &[Value]) -> Vec<String> {
    f.iter().map(|x| format!("{}:{}", x["severity"].as_str().unwrap(), x["rule"].as_str().unwrap())).collect()
}

#[test]
fn clean_form_has_no_findings() {
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &clean_item()));
    let (code, out, _) = lint(&root, &[]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), "Lint: 1 forms, 0 with findings; 0 errors, 0 warnings, 0 info; exit 0");
    assert_eq!(lint(&root, &["--strict"]).0, 0);
}

#[test]
fn field_beyond_the_form_width_is_an_error_with_its_line() {
    let mut c = clean_item();
    c[3] = field(4, "Наименование", [92, 33, 340, 19]).anchors(&stretch(-8));
    let xml = form_xml(400, 200, &c);
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &xml);
    let (code, out, _) = lint(&root, &[]);
    assert_eq!(code, 1, "{out}");
    let want = format!(
        "Catalogs/Тест/ФормаЭлемента: error outside-container line {}: Наименование: right edge 432 is beyond the width 400 of the form",
        line_of(&xml, "Наименование")
    );
    assert!(out.contains(&want), "{out}");
    assert!(out.contains("(fix: reduce Width to 308 or Left to 60"), "{out}");
    assert!(out.contains("1 errors"), "{out}");
}

#[test]
fn negative_coordinates_duplicate_name_and_id_are_errors() {
    let mut c = clean_item();
    c.push(field(5, "Наименование", [-4, 60, 100, 19]));
    c.push(field(4, "Другое", [8, 90, 100, 19]));
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    let r = rules(&f);
    assert!(r.contains(&"error:outside-container".to_string()), "{r:?}");
    assert!(r.contains(&"error:duplicate-name".to_string()), "{r:?}");
    assert!(r.contains(&"error:duplicate-id".to_string()), "{r:?}");
    assert_eq!(lint(&root, &[]).0, 1);
}

#[test]
fn overlapping_fields_are_a_warning_and_strict_exits_3() {
    let mut c = clean_item();
    c.push(field(5, "Второе", [92, 40, 200, 19]));
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    assert!(rules(&f).contains(&"warning:overlap".to_string()), "{f:?}");
    assert!(f
        .iter()
        .any(|x| x["rule"] == "overlap" && x["control"] == "Второе" && x["message"].as_str().unwrap().contains("overlaps 'Наименование'")));
    assert_eq!(lint(&root, &[]).0, 0);
    assert_eq!(lint(&root, &["--strict"]).0, 3);
}

#[test]
fn touching_and_stacked_controls_do_not_overlap() {
    let mut c = clean_item();
    // flush neighbours, and an alternative laid exactly over a field
    c.push(field(5, "Рядом", [8, 60, 100, 19]));
    c.push(field(6, "Следом", [108, 60, 100, 19]));
    c.push(field(7, "Альтернатива", [92, 33, 300, 19]).anchors(&stretch(-8)));
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    assert!(!rules(&f).iter().any(|r| r.ends_with(":overlap")), "{f:?}");
}

#[test]
fn field_at_the_right_margin_that_does_not_stretch() {
    // a report form: 94 % of the fields that reach the right margin stretch, and the neighbouring field does
    let mut c = clean_item();
    c.push(label(5, "НадписьВторое", [8, 60, 84, 19]));
    c.push(field(6, "Второе", [92, 60, 300, 19]));
    let (_g, root) = workspace("Reports", "ФормаОтчета", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    let hit = f.iter().find(|x| x["rule"] == "field-no-stretch").unwrap_or_else(|| panic!("{f:?}"));
    assert_eq!(hit["severity"], "warning");
    assert_eq!(hit["control"], "Второе");
    assert_eq!(hit["kind"], "report");
    assert!(hit["fix"].as_str().unwrap().contains("rightOffset=\"-8\""));
    // a catalog item: only 66 % stretch, the finding is info and carries its share
    let (_g2, root2) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f2 = findings(&root2, &[]);
    let hit2 = f2.iter().find(|x| x["rule"] == "field-no-stretch").unwrap();
    assert_eq!(hit2["severity"], "info");
    assert_eq!(hit2["share"], 66);
    assert!(hit2["message"].as_str().unwrap().contains("share=66 %"), "{hit2}");
    // a fixed dialog where nothing stretches is not reported
    let mut fixed = clean_item();
    fixed[3] = field(4, "Наименование", [92, 33, 300, 19]);
    let (_g3, root3) = workspace("Reports", "ФормаОтчета", &form_xml(400, 200, &fixed));
    assert!(findings(&root3, &[]).iter().all(|x| x["rule"] != "field-no-stretch"));
}

#[test]
fn bottom_panel_not_at_the_bottom() {
    let mut c = clean_item();
    c[1] = Ctl::new("CommandBar", "ОсновныеДействияФормы", 2, [0, 150, 360, 25])
        .tab(99)
        .anchors("bottom=\"Top\" bottomTarget=\"2\" bottomOffset=\"25\" left=\"Left\" right=\"Right\"");
    let xml = form_xml(400, 200, &c);
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &xml);
    let f = findings(&root, &[]);
    let r = rules(&f);
    for want in ["warning:bottom-bar-position", "warning:bottom-bar-width", "warning:bottom-bar-anchor"] {
        assert!(r.contains(&want.to_string()), "{want} in {r:?}");
    }
    let pos = f.iter().find(|x| x["rule"] == "bottom-bar-position").unwrap();
    assert_eq!(pos["line"].as_u64().unwrap() as usize, line_of(&xml, "ОсновныеДействияФормы"));
    assert!(pos["fix"].as_str().unwrap().contains("set Top to 175"));
}

#[test]
fn table_that_does_not_stretch_down_to_the_bottom_panel() {
    let mut c = clean_item();
    c.push(Ctl::new("Table", "Таблица", 5, [8, 60, 384, 107]).anchors("right=\"Right\" rightOffset=\"-8\""));
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    let hit = f.iter().find(|x| x["rule"] == "table-no-stretch-bottom").unwrap_or_else(|| panic!("{f:?}"));
    assert_eq!(hit["severity"], "warning");
    assert!(hit["message"].as_str().unwrap().contains("above the bottom panel"));
    assert!(f.iter().all(|x| x["rule"] != "table-no-stretch"), "{f:?}");
}

#[test]
fn tab_order_against_reading_order() {
    // five rows, TabOrder runs bottom to top
    let mut c = vec![top_bar(1, 400), bottom_bar(2, 400, 300)];
    for i in 0..5u32 {
        c.push(field(10 + i, &format!("Поле{i}"), [92, 33 + 27 * i as i32, 300, 19]).anchors(&stretch(-8)).tab(20 - i));
    }
    let (_g, root) = workspace("Reports", "ФормаОтчета", &form_xml(400, 300, &c));
    let f = findings(&root, &[]);
    let hit = f.iter().find(|x| x["rule"] == "tab-order").unwrap_or_else(|| panic!("{f:?}"));
    assert_eq!(hit["severity"], "info");
    assert!(hit["message"].as_str().unwrap().contains("share="), "{hit}");
    // the same fields in reading order: no finding
    let mut ok = vec![top_bar(1, 400), bottom_bar(2, 400, 300)];
    for i in 0..5u32 {
        ok.push(field(10 + i, &format!("Поле{i}"), [92, 33 + 27 * i as i32, 300, 19]).anchors(&stretch(-8)).tab(10 + i));
    }
    let (_g2, root2) = workspace("Reports", "ФормаОтчета", &form_xml(400, 300, &ok));
    assert!(findings(&root2, &[]).iter().all(|x| x["rule"] != "tab-order"));
}

#[test]
fn label_on_another_row_than_its_field() {
    let mut c = clean_item();
    c[2] = label(3, "НадписьНаименование", [8, 44, 84, 19]);
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    let hit = f.iter().find(|x| x["rule"] == "label-row").unwrap_or_else(|| panic!("{f:?}"));
    assert_eq!(hit["severity"], "warning");
    assert_eq!(hit["control"], "НадписьНаименование");
    assert!(hit["fix"].as_str().unwrap().contains("set Top to 33"));
}

#[test]
fn deviation_from_the_usual_value_is_info_with_share_and_kind() {
    let mut c = clean_item();
    c[2] = label(3, "НадписьНаименование", [14, 33, 84, 19]);
    c[3] = field(4, "Наименование", [98, 33, 294, 22]).anchors(&stretch(-8));
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let f = findings(&root, &[]);
    let left = f.iter().find(|x| x["rule"] == "margin-left").unwrap_or_else(|| panic!("{f:?}"));
    assert_eq!(left["severity"], "info");
    assert_eq!(left["kind"], "catalog_item");
    assert!(left["message"].as_str().unwrap().contains("left margin is 14 px, usual 8 [share=98 %, kind=catalog_item]"), "{left}");
    assert!(f.iter().any(|x| x["rule"] == "field-height" && x["severity"] == "info"));
    assert_eq!(lint(&root, &["--strict"]).0, 0);
}

#[test]
fn json_output_and_usage_errors() {
    let mut c = clean_item();
    c[3] = field(4, "Наименование", [92, 33, 340, 19]);
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let (code, out, _) = lint(&root, &["--json"]);
    assert_eq!(code, 1);
    let v: Value = serde_json::from_str(&out).unwrap();
    let first = &v.as_array().unwrap()[0];
    for key in ["form", "file", "kind", "severity", "rule", "line", "control", "message", "fix", "share"] {
        assert!(first.get(key).is_some(), "{key} in {first}");
    }
    assert_eq!(first["form"], "Catalogs/Тест/ФормаЭлемента");
    assert_eq!(first["file"], "Catalogs/Тест/src/Forms/ФормаЭлемента/Form.xml");
    // a clean tree prints an empty array
    let (_g2, root2) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &clean_item()));
    assert_eq!(lint(&root2, &["--json"]).1.trim(), "[]");
    // usage errors: nothing to lint, flags of lint on other commands
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(lint(empty.path(), &[]).0, 2);
    let out = Command::new(env!("CARGO_BIN_EXE_oneform")).args(["check", root.to_str().unwrap(), "--strict"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("only valid for lint"));
}

#[test]
fn lint_does_not_touch_the_files() {
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &clean_item()));
    fn walk(base: &Path, d: &Path, v: &mut Vec<(String, Vec<u8>)>) {
        for e in fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, v);
            } else {
                v.push((p.strip_prefix(base).unwrap().to_string_lossy().into_owned(), fs::read(&p).unwrap()));
            }
        }
    }
    let snapshot = |dir: &Path| {
        let mut v = Vec::new();
        walk(dir, dir, &mut v);
        v.sort();
        v
    };
    let before = snapshot(&root);
    lint(&root, &[]);
    lint(&root, &["--json", "--strict"]);
    assert_eq!(before, snapshot(&root));
}

/// The text format is a golden file: `tests/golden-lint/text.txt` (`GOLDEN_BLESS=1` rewrites it).
#[test]
fn text_format_golden() {
    let mut c = clean_item();
    c[3] = field(4, "Наименование", [92, 33, 340, 19]).anchors(&stretch(-8));
    c.push(field(5, "Второе", [92, 40, 200, 19]));
    c.push(label(6, "НадписьТретье", [8, 90, 84, 19]));
    c.push(field(7, "Третье", [92, 97, 200, 19]));
    let (_g, root) = workspace("Catalogs", "ФормаЭлемента", &form_xml(400, 200, &c));
    let (code, out, _) = lint(&root, &["--strict"]);
    assert_eq!(code, 1);
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden-lint/text.txt");
    if std::env::var("GOLDEN_BLESS").is_ok_and(|v| v == "1") {
        fs::write(&golden, &out).unwrap();
    }
    assert_eq!(out, fs::read_to_string(&golden).unwrap().replace("\r\n", "\n"));
}
