//! `oneform new-form`: blank ordinary forms of every kind.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const NS: &str = r#"xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20""#;

#[derive(Clone, Copy)]
struct Case {
    kind: &'static str,
    /// directory of the object class under the workspace ("" for the dump of an external object)
    family: &'static str,
    root: &'static str,
    /// the name of the main attribute of the form template
    main: &'static str,
    /// the property that `--default` sets and its value prefix
    default_property: &'static str,
    /// the generated type whose TypeId the form must carry
    type_name: &'static str,
    /// which of the two types of the synthetic object (1 = object, 2 = list)
    type_n: u8,
}

const CASES: [Case; 5] = [
    Case { kind: "catalog-item", family: "Catalogs", root: "Catalog", main: "СправочникОбъект", default_property: "DefaultObjectForm", type_name: "CatalogObject", type_n: 1 },
    Case { kind: "catalog-list", family: "Catalogs", root: "Catalog", main: "СправочникСписок", default_property: "DefaultListForm", type_name: "CatalogList", type_n: 2 },
    Case { kind: "document", family: "Documents", root: "Document", main: "ДокументОбъект", default_property: "DefaultObjectForm", type_name: "DocumentObject", type_n: 1 },
    Case { kind: "processor", family: "", root: "ExternalDataProcessor", main: "ОбработкаОбъект", default_property: "DefaultForm", type_name: "ExternalDataProcessorObject", type_n: 1 },
    Case { kind: "report", family: "", root: "ExternalReport", main: "ОтчетОбъект", default_property: "DefaultForm", type_name: "ExternalReportObject", type_n: 1 },
];

fn type_id(n: u8) -> String {
    format!("aaaaaaaa-0000-4000-8000-00000000000{n}")
}

/// The metadata file of a synthetic object: generated types, one attribute, one tabular section.
fn object_xml(c: &Case, name: &str, with_types: bool, default_form: &str) -> String {
    let types: &[(&str, &str)] = match c.root {
        "Catalog" => &[("CatalogObject", "Object"), ("CatalogList", "List")],
        "Document" => &[("DocumentObject", "Object"), ("DocumentList", "List")],
        "ExternalDataProcessor" => &[("ExternalDataProcessorObject", "Object")],
        _ => &[("ExternalReportObject", "Object")],
    };
    let info: String = if with_types {
        types
            .iter()
            .enumerate()
            .map(|(i, (t, cat))| {
                let n = i + 1;
                format!("<xr:GeneratedType name=\"{t}.{name}\" category=\"{cat}\"><xr:TypeId>{}</xr:TypeId><xr:ValueId>bbbbbbbb-0000-4000-8000-00000000000{n}</xr:ValueId></xr:GeneratedType>", type_id(n as u8))
            })
            .collect()
    } else {
        String::new()
    };
    let props = match c.root {
        "Catalog" | "Document" => format!("<DefaultObjectForm>{default_form}</DefaultObjectForm><DefaultListForm/>"),
        _ => format!("<DefaultForm>{default_form}</DefaultForm><AuxiliaryForm/>"),
    };
    let string_type = |len: u32| format!("<Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>{len}</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject {NS}><{root} uuid="cccccccc-0000-4000-8000-000000000001"><InternalInfo>{info}</InternalInfo><Properties><Name>{name}</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>{name}</v8:content></v8:item></Synonym>{props}</Properties><ChildObjects><Attribute uuid="11111111-0000-4000-8000-000000000001"><Properties><Name>ИНН</Name>{inn}</Properties></Attribute><TabularSection uuid="22222222-0000-4000-8000-000000000001"><InternalInfo><xr:GeneratedType name="{root}TabularSection.{name}.Контакты" category="TabularSection"><xr:TypeId>44444444-0000-4000-8000-000000000001</xr:TypeId><xr:ValueId>44444444-0000-4000-8000-000000000002</xr:ValueId></xr:GeneratedType></InternalInfo><Properties><Name>Контакты</Name></Properties><ChildObjects><Attribute uuid="33333333-0000-4000-8000-000000000001"><Properties><Name>Значение</Name>{value}</Properties></Attribute></ChildObjects></TabularSection></ChildObjects></{root}></MetaDataObject>"#,
        root = c.root,
        inn = string_type(12),
        value = string_type(20),
    )
}

/// A workspace with the object `Тест`; returns (guard, workspace root, object directory).
fn workspace(c: &Case) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let base = if c.family.is_empty() { root.clone() } else { root.join(c.family) };
    let object = base.join("Тест");
    fs::create_dir_all(&object).unwrap();
    fs::write(base.join("Тест.xml"), object_xml(c, "Тест", true, "")).unwrap();
    (tmp, root, object)
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_oneform")).args(args).output().unwrap()
}

fn text(out: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn new_form(object: &Path, kind: &str, name: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["new-form", object.to_str().unwrap(), "--kind", kind, "--name", name];
    args.extend_from_slice(extra);
    run(&args)
}

fn object_file(object: &Path) -> PathBuf {
    object.parent().unwrap().join("Тест.xml")
}

fn between<'a>(s: &'a str, a: &str, b: &str) -> &'a str {
    let start = s.find(a).unwrap() + a.len();
    &s[start..start + s[start..].find(b).unwrap()]
}

#[test]
fn every_kind_creates_registers_defaults_and_extracts() {
    for c in CASES {
        let (_tmp, root, object) = workspace(&c);
        let out = new_form(&object, c.kind, "Нов", &["--synonym", "Новая & форма", "--default"]);
        assert!(out.status.success(), "{}: {}", c.kind, text(&out));

        let meta = fs::read_to_string(object.join("Forms/Нов.xml")).unwrap();
        assert!(meta.starts_with('\u{feff}') && meta.contains("<Name>Нов</Name>"), "{}: {meta}", c.kind);
        assert!(meta.contains("<v8:content>Новая &amp; форма</v8:content>") && meta.contains("<FormType>Ordinary</FormType>"), "{meta}");
        assert_eq!(meta.contains("<ExtendedPresentation/>"), c.family.is_empty(), "{}", c.kind);
        assert!(meta.contains("version=\"2.20\""), "the version of the object is used: {meta}");
        let uuid = between(&meta, "<Form uuid=\"", "\"");
        assert_eq!(uuid.len(), 36);

        let bin = fs::read(object.join("Forms/Нов/Ext/Form.bin")).unwrap();
        let has = |id: String| bin.windows(36).any(|w| w == id.as_bytes());
        assert!(has(type_id(c.type_n)), "{}: the main attribute holds the TypeId of {}", c.kind, c.type_name);
        assert!(!has(type_id(3 - c.type_n)), "{}: the other generated type of the object is not used", c.kind);
        assert!(!bin.windows(36).any(|w| w.starts_with(b"7f5a1c00-")), "{}: the placeholder is replaced", c.kind);

        let obj = fs::read_to_string(object_file(&object)).unwrap();
        assert_eq!(obj.matches("<Form>Нов</Form>").count(), 1, "{obj}");
        let wanted = format!("<{0}>{1}.Тест.Form.Нов</{0}>", c.default_property, c.root);
        assert!(obj.contains(&wanted), "{}: {obj}", c.kind);
        if c.root == "Catalog" || c.root == "Document" {
            let other = if c.default_property == "DefaultObjectForm" { "<DefaultListForm/>" } else { "<DefaultObjectForm></DefaultObjectForm>" };
            assert!(obj.contains(other), "the other default form stays: {obj}");
        }

        let form_xml = fs::read_to_string(object.join("src/Forms/Нов/Form.xml")).unwrap();
        assert!(form_xml.contains(&format!("id=\"{uuid}\"")) && form_xml.contains(&format!("name=\"{}\"", c.main)), "{}: {form_xml}", c.kind);
        assert!(form_xml.contains("<Main>true</Main>") && !form_xml.contains("<Events>"), "{}", c.kind);
        if c.kind == "catalog-list" {
            assert!(form_xml.contains("<AutoFill>true</AutoFill>") && form_xml.contains("<Table name=\"СправочникСписок\""), "the list and its panel");
        } else {
            assert!(form_xml.contains("Standard:142"), "{}: the standard panel", c.kind);
        }
        assert_eq!(fs::read(object.join("src/Forms/Нов/FormModule.bsl")).unwrap().iter().filter(|b| !b.is_ascii_whitespace() && **b != 0xEF && **b != 0xBB && **b != 0xBF).count(), 0);

        let dry = run(&["pack", root.to_str().unwrap(), "--dry-run", "--no-progress"]);
        assert!(text(&dry).contains("NO CHANGES"), "{}: {}", c.kind, text(&dry));
        let check = run(&["check", root.to_str().unwrap(), "--no-progress"]);
        assert!(check.status.success(), "{}: {}", c.kind, text(&check));

        // a second form of the same object, without --default
        let again = new_form(&object, c.kind, "Второй", &[]);
        assert!(again.status.success(), "{}: {}", c.kind, text(&again));
        let obj = fs::read_to_string(object_file(&object)).unwrap();
        assert!(obj.contains(&wanted) && obj.contains("<Form>Второй</Form>"), "the default form stays the first: {obj}");
        assert!(object.join("src/Forms/Второй/Form.xml").is_file() && object.join("src/Forms/Нов/Form.xml").is_file());
        let dry = run(&["pack", root.to_str().unwrap(), "--dry-run", "--no-progress"]);
        assert!(text(&dry).contains("NO CHANGES"), "{}: {}", c.kind, text(&dry));
    }
}

#[test]
fn default_replaces_an_existing_default_and_is_optional() {
    let c = CASES[3];
    let (_tmp, _root, object) = workspace(&c);
    fs::write(object_file(&object), object_xml(&c, "Тест", true, "ExternalDataProcessor.Тест.Form.Старая")).unwrap();
    let out = new_form(&object, c.kind, "Нов", &[]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(fs::read_to_string(object_file(&object)).unwrap().contains("<DefaultForm>ExternalDataProcessor.Тест.Form.Старая</DefaultForm>"));
    let out = new_form(&object, c.kind, "Нов2", &["--default"]);
    assert!(out.status.success(), "{}", text(&out));
    let obj = fs::read_to_string(object_file(&object)).unwrap();
    assert!(obj.contains("<DefaultForm>ExternalDataProcessor.Тест.Form.Нов2</DefaultForm>") && !obj.contains("Старая"), "{obj}");
}

#[test]
fn the_new_form_takes_bound_fields_tables_and_buttons() {
    for c in CASES {
        let (_tmp, root, object) = workspace(&c);
        assert!(new_form(&object, c.kind, "Нов", &[]).status.success());
        let form = object.join("src/Forms/Нов/Form.xml");
        let xml = fs::read_to_string(&form).unwrap();
        let main_id = between(&xml, "<MainAttribute attributeId=\"", "\"");
        let m = c.main;
        let added = format!(
            r#"    <InputField name="ИНН" id="50">
      <DataPath attributeId="{main_id}">{m}.ИНН</DataPath>
      <Left>8</Left><Top>33</Top><Width>200</Width><Height>20</Height><ZOrder>10</ZOrder><TabOrder>10</TabOrder>
    </InputField>
    <Table name="Контакты" id="51">
      <DataPath attributeId="{main_id}">{m}.Контакты</DataPath>
      <Left>8</Left><Top>60</Top><Width>380</Width><Height>100</Height><ZOrder>11</ZOrder><TabOrder>11</TabOrder>
      <Columns>
        <Column name="Значение" id="1">
          <DataPath attributeId="{main_id}">{m}.Контакты.Значение</DataPath>
          <Header><Item language="ru">Значение</Item></Header>
          <Width>200</Width>
          <Editor>InputField</Editor>
        </Column>
      </Columns>
    </Table>
"#
        );
        let mut edited = xml.replace("  </ChildItems>", &format!("{added}  </ChildItems>"));
        if c.kind != "catalog-list" {
            // the panel «Закрыть» of the forms of objects; the command bar of a report form has no such panel
            let anchor = if c.kind == "report" { "<Button name=\"Действие5\" id=\"31\"" } else { "<Button name=\"ОсновныеДействияФормыЗакрыть\"" };
            let at = edited.find(anchor).expect("the panel has a button to put the new one before");
            edited.insert_str(at, "<Button name=\"Обновить\" kind=\"Action\" action=\"Standard:90\"><Caption><Item language=\"ru\">Обновить</Item></Caption></Button>\n        ");
        }
        fs::write(&form, edited).unwrap();
        let packed = run(&["pack", root.to_str().unwrap(), "--no-progress"]);
        assert!(packed.status.success(), "{}: {}", c.kind, text(&packed));
        let again = fs::read_to_string(&form).unwrap();
        for needle in [
            format!("{m}.ИНН"),
            format!("{m}.Контакты</DataPath>"),
            format!("{m}.Контакты.Значение"),
            "<Table name=\"Контакты\"".to_string(),
            "<InputField name=\"ИНН\"".to_string(),
        ] {
            assert!(again.contains(&needle), "{}: {needle} is missing after pack:\n{again}", c.kind);
        }
        assert!(again.contains("steps=\"11111111-0000-4000-8000-000000000001\""), "{}: bound to the attribute of the object", c.kind);
        if c.kind != "catalog-list" {
            assert!(again.contains("<Button name=\"Обновить\""), "{}: the new button", c.kind);
        }
        let dry = run(&["pack", root.to_str().unwrap(), "--dry-run", "--no-progress"]);
        assert!(text(&dry).contains("NO CHANGES"), "{}: {}", c.kind, text(&dry));
    }
}

#[test]
fn the_new_form_takes_form_attributes_and_a_value_table_with_columns() {
    for c in CASES {
        let (_tmp, root, object) = workspace(&c);
        assert!(new_form(&object, c.kind, "Нов", &[]).status.success());
        let form = object.join("src/Forms/Нов/Form.xml");
        let xml = fs::read_to_string(&form).unwrap();
        let next_id = xml.match_indices("<Attribute id=\"").map(|(at, m)| xml[at + m.len()..].split('"').next().unwrap().parse::<i64>().unwrap()).max().unwrap() + 1;
        let (counter, table) = (next_id, next_id + 1);
        let attributes = format!(
            "    <Attribute id=\"{counter}\" name=\"Счётчик\">
      <ValueType kind=\"Number\" digits=\"10\" fractionDigits=\"0\" composite=\"false\"/>
    </Attribute>
    <Attribute id=\"{table}\" name=\"Таблица\">
      <ValueType kind=\"ValueTable\" composite=\"false\"/>
    </Attribute>
"
        );
        // an attribute without any binding
        let alone = xml.replace("  </Attributes>", &format!("{}  </Attributes>", attributes.lines().take(3).map(|l| format!("{l}
")).collect::<String>()));
        fs::write(&form, &alone).unwrap();
        let out = run(&["pack", root.to_str().unwrap(), "--no-progress"]);
        assert!(out.status.success(), "{}: {}", c.kind, text(&out));
        let shown = fs::read_to_string(&form).unwrap();
        assert!(shown.contains("name=\"Счётчик\"") && !shown.contains("name=\"Таблица\""), "{}: {shown}", c.kind);

        // a value table attribute with a table bound to it, columns of four types
        let column = |name: &str, id: i64, ty: &str, extra: &str| {
            format!("        <Column name=\"{name}\" id=\"{id}\"><Header><Item language=\"ru\">{name}</Item></Header><Width>100</Width>{extra}<ValueType {ty} composite=\"false\"/></Column>
")
        };
        let columns = [
            column("Наименование", 0, "kind=\"String\" length=\"50\"", ""),
            column("Сумма", 1, "kind=\"Number\" digits=\"12\" fractionDigits=\"2\"", ""),
            column("Пометка", 2, "kind=\"Boolean\"", ""),
            column("Дата", 3, "kind=\"Date\"", "<ReadOnly>true</ReadOnly>"),
        ]
        .concat();
        let added = format!(
            "    <Table name=\"ТаблицаДанных\" id=\"60\">
      <DataPath attributeId=\"{table}\">Таблица</DataPath>
      <Left>8</Left><Top>8</Top><Width>400</Width><Height>120</Height><ZOrder>20</ZOrder><TabOrder>20</TabOrder>
      <Columns>
{columns}      </Columns>
    </Table>
"
        );
        let edited = shown
            .replace("  </Attributes>", &format!("{}  </Attributes>", attributes.lines().skip(3).map(|l| format!("{l}
")).collect::<String>()))
            .replace("  </ChildItems>", &format!("{added}  </ChildItems>"));
        fs::write(&form, edited).unwrap();
        let out = run(&["pack", root.to_str().unwrap(), "--no-progress"]);
        assert!(out.status.success(), "{}: {}", c.kind, text(&out));
        let again = fs::read_to_string(&form).unwrap();
        for needle in [
            format!("<DataPath attributeId=\"{table}\">Таблица</DataPath>"),
            format!("<DataPath attributeId=\"{table}\">Таблица.Сумма</DataPath>"),
            "<ValueType kind=\"String\" composite=\"false\" length=\"50\"/>".to_string(),
            "<ValueType kind=\"Number\" composite=\"false\" digits=\"12\" fractionDigits=\"2\"/>".to_string(),
            "<ValueType kind=\"Boolean\" composite=\"false\"/>".to_string(),
            "<ValueType kind=\"Date\" composite=\"false\"/>".to_string(),
            "<Column name=\"Пометка\" id=\"2\">".to_string(),
        ] {
            assert!(again.contains(&needle), "{}: {needle} is missing after pack:
{again}", c.kind);
        }
        let dry = run(&["pack", root.to_str().unwrap(), "--dry-run", "--no-progress"]);
        assert!(text(&dry).contains("NO CHANGES"), "{}: {}", c.kind, text(&dry));
        let check = run(&["check", root.to_str().unwrap(), "--no-progress"]);
        assert!(check.status.success(), "{}: {}", c.kind, text(&check));

        // the table still uses its attribute: the attribute stays; the unused one goes
        let block = |name: &str| {
            let at = again.find(&format!("<Attribute id=\"{}\" name=\"{name}\">", if name == "Счётчик" { counter } else { table })).unwrap();
            again[at..at + again[at..].find("</Attribute>").unwrap() + "</Attribute>
".len()].to_string()
        };
        fs::write(&form, again.replacen(&block("Таблица"), "", 1)).unwrap();
        let out = run(&["pack", root.to_str().unwrap(), "--no-progress"]);
        assert!(!out.status.success() && text(&out).contains("cannot be removed") && text(&out).contains("ТаблицаДанных"), "{}: {}", c.kind, text(&out));
        fs::write(&form, again.replacen(&block("Счётчик"), "", 1)).unwrap();
        let out = run(&["pack", root.to_str().unwrap(), "--no-progress"]);
        assert!(out.status.success(), "{}: {}", c.kind, text(&out));
        let last = fs::read_to_string(&form).unwrap();
        assert!(!last.contains("Счётчик") && last.contains("name=\"Таблица\""), "{}: {last}", c.kind);
        let dry = run(&["pack", root.to_str().unwrap(), "--dry-run", "--no-progress"]);
        assert!(text(&dry).contains("NO CHANGES"), "{}: {}", c.kind, text(&dry));
    }
}

#[test]
fn the_report_form_is_the_form_of_the_configurator_without_user_controls() {
    let report = CASES.iter().find(|c| c.kind == "report").unwrap();
    let (_tmp, root, object) = workspace(report);
    assert!(new_form(&object, "report", "ФормаОтчета", &["--default"]).status.success());
    let xml = fs::read_to_string(object.join("src/Forms/ФормаОтчета/Form.xml")).unwrap();
    // size and caption as in the Configurator form (two spaces after «Отчет»)
    assert!(xml.contains("<Item language=\"ru\">Отчет  Тест</Item>") && xml.contains("<Width>520</Width>") && xml.contains("<Height>300</Height>"), "{xml}");
    // main attribute id 3 `ОтчетОбъект` and the report extension, no other attribute
    assert!(xml.contains("<MainAttribute attributeId=\"3\"/>") && xml.contains("<FormExtension guid=\"343b4e00-29f0-11d6-a3c7-0050bae0a776\"/>"), "{xml}");
    assert_eq!(xml.matches("<Attribute id=").count(), 1, "{xml}");
    assert!(xml.contains("<Attribute id=\"3\" name=\"ОтчетОбъект\">"), "{xml}");
    // the controls of the Configurator form: result, command bar, settings table (60 columns bound to the settings composer)
    for control in ["<SpreadsheetDocumentField name=\"Результат\" id=\"1\">", "<CommandBar name=\"ДействияФормы\" id=\"2\">", "<Table name=\"Отбор\" id=\"7\">"] {
        assert!(xml.contains(control), "{control} is missing:
{xml}");
    }
    assert!(xml.contains("steps=\"b9754f01-29e9-11d6-a3c7-0050bae0a776 0 1\">ОтчетОбъект.[b9754f01-29e9-11d6-a3c7-0050bae0a776].[0].[1]</DataPath>"), "{xml}");
    assert_eq!(xml.matches("<Column name=").count(), 60);
    // nothing of the user's controls of the sample is left
    for gone in ["ПолеВвода1", "Надпись1", "Кнопка1", "<Events>", "ОсновныеДействияФормы", "ТаблицаНоменклатуры"] {
        assert!(!xml.contains(gone), "{gone} is left:
{xml}");
    }
    let module = fs::read(object.join("src/Forms/ФормаОтчета/FormModule.bsl")).unwrap();
    assert!(module.iter().all(|b| b.is_ascii_whitespace() || [0xEF, 0xBB, 0xBF].contains(b)), "the module is empty");
    // the object keeps its own metadata: the form is the default one
    assert!(fs::read_to_string(object_file(&object)).unwrap().contains("<DefaultForm>ExternalReport.Тест.Form.ФормаОтчета</DefaultForm>"));
    let dry = run(&["pack", root.to_str().unwrap(), "--dry-run", "--no-progress"]);
    assert!(text(&dry).contains("NO CHANGES"), "{}", text(&dry));
    let check = run(&["check", root.to_str().unwrap(), "--no-progress"]);
    assert!(check.status.success(), "{}", text(&check));
}

#[test]
fn the_report_caption_follows_the_object_name() {
    let report = CASES.iter().find(|c| c.kind == "report").unwrap();
    let (_tmp, _root, object) = workspace(report);
    assert!(new_form(&object, "report", "Ф", &[]).status.success());
    let xml = fs::read_to_string(object.join("src/Forms/Ф/Form.xml")).unwrap();
    // the name is put into the stream (which is longer than the marker of the template): the stream still reads back
    assert!(xml.contains("Отчет  Тест</Item>") && !xml.contains("%ИмяОбъекта%"), "{xml}");
}

#[test]
fn the_processor_form_has_the_main_attribute_only() {
    let processor = CASES.iter().find(|c| c.kind == "processor").unwrap();
    let (_tmp, _root, object) = workspace(processor);
    assert!(new_form(&object, "processor", "Ф", &[]).status.success());
    let xml = fs::read_to_string(object.join("src/Forms/Ф/Form.xml")).unwrap();
    assert!(xml.contains("<MainAttribute attributeId=\"0\"/>") && xml.contains("name=\"ОбработкаОбъект\""), "{xml}");
    assert_eq!(xml.matches("<Attribute id=").count(), 1, "the unused value table of the old template is gone:
{xml}");
    assert!(!xml.contains("ТаблицаНоменклатуры") && !xml.contains("ValueTable"), "{xml}");
    assert!(xml.contains("Standard:142"), "the panel with «Закрыть» stays");
}

/// Everything under `dir` with the content, for "nothing changed" checks (the diagnostics log of a refusal is not a change
/// of the project: tests/diagnostics.rs covers it).
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(snapshot(&p));
        } else if p.file_name().is_some_and(|n| n != "diagnostics.jsonl") {
            out.push((p.clone(), fs::read(&p).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn refused_requests_change_nothing() {
    for c in CASES {
        let (tmp, _root, object) = workspace(&c);
        assert!(new_form(&object, c.kind, "Нов", &[]).status.success());
        let before = snapshot(tmp.path());
        let wrong_kind = if c.root == "Catalog" { "document" } else { "catalog-item" };
        let o = object.to_str().unwrap();
        let bad: Vec<(Vec<&str>, &str)> = vec![
            (vec!["new-form", o, "--kind", c.kind, "--name", "Нов"], "already exists"),
            (vec!["new-form", o, "--kind", c.kind, "--name", "1плохо"], "not a valid form name"),
            (vec!["new-form", o, "--kind", c.kind, "--name", "с пробелом"], "not a valid form name"),
            (vec!["new-form", o, "--kind", wrong_kind, "--name", "Другая"], "needs an object of type"),
            (vec!["new-form", o, "--kind", "dialog", "--name", "Другая"], "unknown form kind"),
            (vec!["new-form", o, "--kind", c.kind], "needs --kind"),
            (vec!["new-form", o, "--name", "Другая"], "needs --kind"),
        ];
        for (args, hint) in bad {
            let out = run(&args);
            assert!(!out.status.success(), "{}: {args:?}", c.kind);
            assert!(text(&out).contains(hint), "{}: {args:?}: {}", c.kind, text(&out));
            assert_eq!(snapshot(tmp.path()), before, "{}: {args:?} changed something", c.kind);
        }
    }
}

#[test]
fn objects_that_cannot_hold_the_form_are_refused() {
    let c = CASES[0];
    // no generated types in the metadata file
    let (tmp, _root, object) = workspace(&c);
    fs::write(object_file(&object), object_xml(&c, "Тест", false, "")).unwrap();
    let before = snapshot(tmp.path());
    let out = new_form(&object, c.kind, "Нов", &[]);
    assert!(!out.status.success() && text(&out).contains("GeneratedType"), "{}", text(&out));
    assert_eq!(snapshot(tmp.path()), before);

    // a binary external processor / report, a missing directory, a directory without a metadata file
    let tmp = tempfile::tempdir().unwrap();
    for kind in ["processor", "report"] {
        let epf = tmp.path().join("Обработка.epf");
        fs::write(&epf, b"not a dump").unwrap();
        let out = new_form(&epf, kind, "Нов", &[]);
        assert!(!out.status.success() && text(&out).contains("epf-dump"), "{}", text(&out));
    }
    let out = new_form(&tmp.path().join("Нет"), "processor", "Нов", &[]);
    assert!(!out.status.success() && text(&out).contains("not a directory"), "{}", text(&out));
    fs::create_dir(tmp.path().join("Пусто")).unwrap();
    let out = new_form(&tmp.path().join("Пусто"), "processor", "Нов", &[]);
    assert!(!out.status.success() && text(&out).contains("not an object directory"), "{}", text(&out));
    assert_eq!(fs::read_dir(tmp.path().join("Пусто")).unwrap().count(), 0);

    // --kind / --default belong to new-form only
    let out = run(&["extract", tmp.path().to_str().unwrap(), "--default"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
}

#[test]
fn a_failing_extract_leaves_nothing_behind() {
    let c = CASES[0];
    let (tmp, _root, object) = workspace(&c);
    assert!(new_form(&object, c.kind, "Нов", &[]).status.success());
    // a hand-edited generated file makes the next extract refuse: the second form must not stay
    let form = object.join("src/Forms/Нов/Form.xml");
    let edited = fs::read_to_string(&form).unwrap().replace("<Width>400</Width>", "<Width>500</Width>");
    fs::write(&form, edited).unwrap();
    let before = snapshot(tmp.path());
    let out = new_form(&object, c.kind, "Второй", &["--default"]);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("nothing was changed"), "{}", text(&out));
    assert_eq!(snapshot(tmp.path()), before);
    assert!(!object.join("Forms/Второй.xml").exists() && !object.join("Forms/Второй").exists());
}

#[test]
fn an_object_that_already_lists_the_form_is_refused_before_writing() {
    let c = CASES[2];
    let (tmp, _root, object) = workspace(&c);
    assert!(!object.join("Forms").exists());
    // the object lists the form in its metadata but has no files for it
    fs::write(object_file(&object), object_xml(&c, "Тест", true, "").replace("</ChildObjects>", "<Form>Нов</Form></ChildObjects>")).unwrap();
    let before = snapshot(tmp.path());
    let out = new_form(&object, c.kind, "Нов", &[]);
    assert!(!out.status.success() && text(&out).contains("already lists"), "{}", text(&out));
    assert_eq!(snapshot(tmp.path()), before);
    assert!(!object.join("Forms").exists());
}
