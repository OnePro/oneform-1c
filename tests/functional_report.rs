//! A real, fully functional external report made in the Configurator (settings panel with one page, object
//! module, spreadsheet template, ordinary form with the report form extension `343b4e00` and the main attribute
//! `ОтчетОбъект`). `tests/fixtures/functional-report.erf` is the file; `tests/fixtures/functional-report/` is its
//! `erf-dump` (the form stream `Ext/Form.bin` is the key). The edited reports were built back with `erf-build` and
//! opened in 1C: `docs/evidence/functional-report.md`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OBJECT: &str = "ЗадолженностьПоСрокамВозникновения";
const TABS_OFF: &str = "<ShowTabs type=\"ShowTabs\" member=\"None\" code=\"0\"/>";
const TABS_TOP: &str = "<ShowTabs type=\"ShowTabs\" member=\"Top\" code=\"1\"/>";
const PANEL_END: &str = "      </ChildItems>\n    </Panel>\n    <CommandBar name=\"ДействияНастройка\"";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn original_bin() -> Vec<u8> {
    fs::read(fixture("functional-report").join(OBJECT).join("Forms/ФормаОтчета/Ext/Form.bin")).unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oneform")).args(args).arg(dir).arg("--no-progress").output().unwrap()
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

struct Report {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Report {
    /// A copy of the dump with the form extracted.
    fn new() -> Report {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        copy_dir(&fixture("functional-report"), &root);
        let out = run(&root, &["extract"]);
        assert!(out.status.success(), "{}", text(&out));
        Report { _tmp: tmp, root }
    }

    fn object(&self) -> PathBuf {
        self.root.join(OBJECT)
    }

    fn form_xml(&self) -> PathBuf {
        self.object().join("src/Forms/ФормаОтчета/Form.xml")
    }

    fn module(&self) -> PathBuf {
        self.object().join("src/Forms/ФормаОтчета/FormModule.bsl")
    }

    fn bin(&self) -> Vec<u8> {
        fs::read(self.object().join("Forms/ФормаОтчета/Ext/Form.bin")).unwrap()
    }

    fn xml(&self) -> String {
        fs::read_to_string(self.form_xml()).unwrap().replace("\r\n", "\n")
    }

    fn edit(&self, f: impl FnOnce(String) -> String) {
        fs::write(self.form_xml(), f(self.xml())).unwrap();
    }

    fn pack(&self) -> String {
        let out = run(&self.root, &["pack"]);
        assert!(out.status.success(), "{}", text(&out));
        text(&out)
    }

    fn refused(&self) -> String {
        let out = run(&self.root, &["pack"]);
        assert!(!out.status.success(), "must be refused: {}", text(&out));
        text(&out)
    }

    /// The Form.xml a fresh `extract` of the packed `Form.bin` gives equals the packed one (nothing is lost).
    fn assert_reextract_equals(&self) {
        let tmp = tempfile::tempdir().unwrap();
        let object = tmp.path().join(OBJECT);
        copy_dir(&self.object().join("Forms"), &object.join("Forms"));
        copy_dir(&self.object().join("Templates"), &object.join("Templates"));
        fs::create_dir_all(object.join("Ext")).unwrap();
        fs::copy(self.object().join("Ext/ObjectModule.bsl"), object.join("Ext/ObjectModule.bsl")).unwrap();
        fs::copy(self.root.join(format!("{OBJECT}.xml")), tmp.path().join(format!("{OBJECT}.xml"))).unwrap();
        let out = run(tmp.path(), &["extract"]);
        assert!(out.status.success(), "{}", text(&out));
        let fresh = fs::read_to_string(object.join("src/Forms/ФормаОтчета/Form.xml")).unwrap();
        assert_eq!(fresh.replace("\r\n", "\n"), self.xml(), "re-extracted Form.xml differs from the packed one");
    }

    fn assert_outside_the_form_untouched(&self) {
        for rel in ["Ext/ObjectModule.bsl", "Templates/Макет/Ext/Template.xml", "Templates/Макет.xml", "Forms/ФормаОтчета.xml"] {
            let now = fs::read(self.object().join(rel)).unwrap();
            let was = fs::read(fixture("functional-report").join(OBJECT).join(rel)).unwrap();
            assert_eq!(now, was, "{rel} must stay byte-identical");
        }
        let root = format!("{OBJECT}.xml");
        assert_eq!(fs::read(self.root.join(&root)).unwrap(), fs::read(fixture("functional-report").join(&root)).unwrap());
    }
}

fn add_page(xml: String, name: &str) -> String {
    let page = format!("        <Page name=\"{name}\">\n          <Title>\n            <Item language=\"ru\">{name}</Item>\n          </Title>\n        </Page>\n");
    assert_eq!(xml.matches(PANEL_END).count(), 1);
    xml.replacen(PANEL_END, &format!("{page}{PANEL_END}"), 1)
}

/// `(left, top, width, height)` of the page `name` in the extracted Form.xml.
fn page_area(xml: &str, name: &str) -> [i64; 4] {
    let at = xml.find(&format!("<Page name=\"{name}\">")).unwrap_or_else(|| panic!("page {name}"));
    let block = &xml[at..at + xml[at..].find("</Page>").unwrap()];
    ["Left", "Top", "Width", "Height"].map(|tag| {
        let open = format!("<{tag}>");
        let from = block.find(&open).unwrap_or_else(|| panic!("{tag} of page {name}")) + open.len();
        block[from..from + block[from..].find('<').unwrap()].parse().unwrap()
    })
}

#[test]
fn the_erf_itself_is_skipped_with_the_hint() {
    let tmp = tempfile::tempdir().unwrap();
    fs::copy(fixture("functional-report.erf"), tmp.path().join("r.erf")).unwrap();
    let out = run(tmp.path(), &["extract"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("внешний отчёт: выгрузите в XML (erf-dump)"), "{}", text(&out));
}

#[test]
fn the_real_report_is_in_scope_and_a_no_op_round_trip_keeps_the_stream_byte_exact() {
    let r = Report::new();
    assert_eq!(r.bin(), original_bin());
    let validate = run(&r.root, &["validate"]);
    assert!(text(&validate).contains("ФормаОтчета: IN_SCOPE") && validate.status.success(), "{}", text(&validate));
    let xml = r.xml();
    // the panel of the report: one page without tabs, 300x386
    assert!(xml.contains("<Panel name=\"ПанельНастроек\" id=\"29\">"), "{xml}");
    assert!(xml.contains(TABS_OFF));
    assert_eq!(page_area(&xml, "Страница1"), [0, 0, 300, 386]);
    assert!(xml.contains("<MainAttribute attributeId=\"1\"/>") && xml.contains("name=\"ОтчетОбъект\""));
    let dry = run(&r.root, &["pack", "--dry-run"]);
    assert!(text(&dry).contains("NO CHANGES"), "{}", text(&dry));
    let out = r.pack();
    assert!(out.contains("UNCHANGED") || out.contains("NO CHANGES"), "{out}");
    assert_eq!(r.bin(), original_bin(), "Form.bin must be byte-exact after a no-op pack");
    r.assert_outside_the_form_untouched();
}

#[test]
fn pages_of_the_real_panel_are_renamed_added_reordered_and_deleted() {
    let r = Report::new();
    // rename
    r.edit(|x| x.replace("<Page name=\"Страница1\">", "<Page name=\"Настройка\">").replace("<Item language=\"ru\">Страница1</Item>", "<Item language=\"ru\">Настройка</Item>"));
    r.pack();
    r.assert_reextract_equals();
    assert_eq!(page_area(&r.xml(), "Настройка"), [0, 0, 300, 386]);
    // add two pages (an empty page takes the area of the others)
    r.edit(|x| add_page(add_page(x, "Вторая"), "Третья"));
    r.pack();
    r.assert_reextract_equals();
    let xml = r.xml();
    assert_eq!(page_area(&xml, "Третья"), [0, 0, 300, 386]);
    assert!(xml.find("name=\"Настройка\"").unwrap() < xml.find("name=\"Вторая\"").unwrap());
    // reorder: the page with the controls goes last, its controls stay on it
    r.edit(|x| {
        let first = x.find("        <Page name=\"Настройка\">").unwrap();
        let second = x.find("        <Page name=\"Вторая\">").unwrap();
        let end = x.find(PANEL_END).unwrap();
        format!("{}{}{}{}", &x[..first], &x[second..end], &x[first..second], &x[end..])
    });
    r.pack();
    r.assert_reextract_equals();
    let xml = r.xml();
    let (second, third, first) = (xml.find("<Page name=\"Вторая\">").unwrap(), xml.find("<Page name=\"Третья\">").unwrap(), xml.find("<Page name=\"Настройка\">").unwrap());
    assert!(second < third && third < first, "page order");
    let table = xml.find("<Table name=\"Счета\"").unwrap();
    assert!(table > first && table < xml.find(PANEL_END).unwrap(), "the table stays on its page");
    // delete the two empty pages
    r.edit(|x| {
        let from = x.find("        <Page name=\"Вторая\">").unwrap();
        let to = x.find("        <Page name=\"Настройка\">").unwrap();
        format!("{}{}", &x[..from], &x[to..])
    });
    r.pack();
    r.assert_reextract_equals();
    assert_eq!(r.xml().matches("<Page name=").count(), 1);
    r.assert_outside_the_form_untouched();
}

#[test]
fn show_tabs_of_the_real_panel_moves_the_page_area_both_ways() {
    let r = Report::new();
    let original_xml = r.xml();
    r.edit(|x| x.replace(TABS_OFF, TABS_TOP));
    r.pack();
    r.assert_reextract_equals();
    // the tab strip takes 2 px of width and 20 px of height: the area of a panel with tabs
    assert_eq!(page_area(&r.xml(), "Страница1"), [1, 19, 298, 366]);
    r.edit(|x| x.replace(TABS_TOP, TABS_OFF));
    r.pack();
    r.assert_reextract_equals();
    assert_eq!(page_area(&r.xml(), "Страница1"), [0, 0, 300, 386]);
    // there and back again: the form reads as the original one (the stream is re-encoded, so not byte for byte)
    assert_eq!(r.xml(), original_xml);
    r.assert_outside_the_form_untouched();
}

#[test]
fn show_tabs_with_a_rename_in_one_pack_a_new_page_in_the_next_and_the_refusals() {
    let r = Report::new();
    r.edit(|x| {
        x.replace(TABS_OFF, TABS_TOP)
            .replace("<Page name=\"Страница1\">", "<Page name=\"Настройка\">")
            .replace("<Item language=\"ru\">Страница1</Item>", "<Item language=\"ru\">Настройка</Item>")
    });
    r.pack();
    r.assert_reextract_equals();
    assert_eq!(page_area(&r.xml(), "Настройка"), [1, 19, 298, 366]);

    // tabs and a new page in one pack: refused (change the tabs first, then the pages)
    let r = Report::new();
    r.edit(|x| add_page(x.replace(TABS_OFF, TABS_TOP), "Вторая"));
    let msg = r.refused();
    assert!(msg.contains("change the tabs first, then the pages"), "{msg}");
    assert_eq!(r.bin(), original_bin());
    // ... and then the page, in the next pack, takes the area of the page with tabs
    let r = Report::new();
    r.edit(|x| x.replace(TABS_OFF, TABS_TOP));
    r.pack();
    r.edit(|x| add_page(x, "Вторая"));
    r.pack();
    r.assert_reextract_equals();
    assert_eq!(page_area(&r.xml(), "Вторая"), [1, 19, 298, 366]);

    // another tab code: the page area of side tabs is unknown, pack refuses and writes nothing
    let r = Report::new();
    r.edit(|x| x.replace(TABS_OFF, "<ShowTabs type=\"ShowTabs\" member=\"Unknown\" code=\"2\"/>"));
    let msg = r.refused();
    assert!(msg.contains("only 0 (None) <-> 1 (Top)"), "{msg}");
    assert_eq!(r.bin(), original_bin());
}

#[test]
fn edits_of_the_report_form_keep_the_object_module_and_the_template() {
    let r = Report::new();
    let added = "    <LabelDecoration name=\"НадписьПримечание\" id=\"60\">\n      <Caption>\n        <Item language=\"ru\">Примечание:</Item>\n      </Caption>\n      <Left>8</Left>\n      <Top>104</Top>\n      <Width>70</Width>\n      <Height>19</Height>\n      <ZOrder>12</ZOrder>\n      <TabOrder>13</TabOrder>\n    </LabelDecoration>\n    <InputField name=\"Примечание\" id=\"61\">\n      <DataPath attributeId=\"14\">Примечание</DataPath>\n      <Left>84</Left>\n      <Top>104</Top>\n      <Width>440</Width>\n      <Height>19</Height>\n      <ZOrder>13</ZOrder>\n      <TabOrder>14</TabOrder>\n    </InputField>\n    <Button name=\"КнопкаОчистить\" id=\"62\">\n      <Caption>\n        <Item language=\"ru\">Очистить</Item>\n      </Caption>\n      <Left>532</Left>\n      <Top>104</Top>\n      <Width>160</Width>\n      <Height>19</Height>\n      <ZOrder>14</ZOrder>\n      <TabOrder>15</TabOrder>\n      <Events>\n        <Click id=\"0\">КнопкаОчиститьНажатие</Click>\n      </Events>\n    </Button>\n";
    r.edit(|x| {
        let x = x.replace("<Item language=\"ru\">Период:</Item>", "<Item language=\"ru\">На дату:</Item>");
        let x = x.replacen("<Width>196</Width>", "<Width>180</Width>", 1);
        let x = x.replacen(
            "    <Attribute id=\"13\" name=\"ОтображатьНастройку\">",
            "    <Attribute id=\"14\" name=\"Примечание\">\n      <ValueType kind=\"String\" composite=\"false\" length=\"100\"/>\n    </Attribute>\n    <Attribute id=\"13\" name=\"ОтображатьНастройку\">",
            1,
        );
        // written ABOVE the existing controls: the stream keeps them last
        let x = x.replacen("  <ChildItems>\n", &format!("  <ChildItems>\n{added}"), 1);
        x.replacen("<Height>498</Height>", "<Height>526</Height>", 1)
    });
    let module = fs::read_to_string(r.module()).unwrap();
    fs::write(r.module(), format!("{module}\nПроцедура КнопкаОчиститьНажатие(Кнопка)\n\tПримечание = \"\";\nКонецПроцедуры\n")).unwrap();
    let out = r.pack();
    assert!(!out.contains("КнопкаОчиститьНажатие("), "the handler is declared in FormModule.bsl, no warning: {out}");
    r.assert_reextract_equals();
    let xml = r.xml();
    // the added controls are last; the field takes the type of its attribute and has no drop-down / spin buttons
    assert!(xml.find("name=\"НадписьПримечание\"").unwrap() > xml.find("name=\"ДействияНастройка\"").unwrap());
    let field = &xml[xml.find("<InputField name=\"Примечание\"").unwrap()..];
    let field = &field[..field.find("</InputField>").unwrap()];
    assert!(field.contains("<ValueType kind=\"String\" composite=\"false\" length=\"100\"/>"), "{field}");
    assert!(field.contains("<DropListButton>false</DropListButton>") && field.contains("<SpinButton>false</SpinButton>"), "{field}");
    assert!(xml.contains("<Click id=\"0\">КнопкаОчиститьНажатие</Click>"));
    assert!(xml.contains("<Height>526</Height>"));
    r.assert_outside_the_form_untouched();
}
