//! Advisory warnings of a planned `Form.xml` edit: references in the modules to removed or renamed controls, an added
//! field that keeps the donor value type, an added or moved control beyond its container. None of them blocks the pack.

use std::path::{Path, PathBuf};

use oneform::object_schema::{with_object_schema, ObjectSchema};
use oneform::pack::{plan_layout_patch, LayoutPatch};
use oneform::transaction::Tree;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm")
}

fn baseline() -> String {
    std::fs::read_to_string(dir().join("Form.xml")).unwrap().replace("\r\n", "\n")
}

fn cut_block(xml: &str, open: &str, close: &str) -> String {
    let at = xml.find(open).unwrap();
    let line = xml[..at].rfind('\n').map_or(0, |i| i + 1);
    let end = at + xml[at..].find(close).unwrap() + close.len();
    format!("{}{}", &xml[..line], xml[end..].trim_start_matches(['\n', ' ']))
}

fn schema() -> ObjectSchema {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.17">
  <Catalog uuid="aaaaaaaa-0000-0000-0000-000000000001">
    <Properties><Name>Объект</Name></Properties>
    <ChildObjects>
      <Attribute uuid="11111111-0000-0000-0000-000000000002"><Properties><Name>КПП</Name></Properties></Attribute>
    </ChildObjects>
  </Catalog>
</MetaDataObject>"#;
    ObjectSchema::from_object_xml(xml).unwrap()
}

fn tree(xml: &str, module: Option<&str>) -> Tree {
    let mut t: Tree = [
        ("Form.xml".to_string(), xml.as_bytes().to_vec()),
        ("FormStream.bin".to_string(), std::fs::read(dir().join("FormStream.bin")).unwrap()),
    ]
    .into_iter()
    .collect();
    if let Some(m) = module {
        t.insert("FormModule.bsl".to_string(), m.as_bytes().to_vec());
    }
    t
}

fn plan(edited: &str, module: Option<&str>) -> LayoutPatch {
    let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
    with_object_schema(Some(schema()), || plan_layout_patch(&layout, &tree(edited, module), &tree(&baseline(), None)))
        .unwrap_or_else(|e| panic!("{e} {:?}", e.params))
        .expect("a layout edit")
}

fn warnings(edited: &str, module: Option<&str>) -> Vec<String> {
    plan(edited, module).summary().into_iter().filter(|l| l.contains("warning")).collect()
}

fn without_input() -> String {
    cut_block(&baseline(), "<InputField name=\"ПолеВвода1\"", "</InputField>")
}

fn without_button() -> String {
    cut_block(&baseline(), "<Button name=\"Кнопка1\" id=\"4\"", "</Button>")
}

#[test]
fn a_removed_control_still_used_in_the_module_is_reported_with_its_line() {
    let module = "Процедура Открыть()\n\tЭлементыФормы.ПолеВвода1.Значение = 1;\n\tx = ЭлементыФормы.Флажок1;\nКонецПроцедуры\n";
    let w = warnings(&without_input(), Some(module));
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("removed control 'ПолеВвода1' is still referenced at FormModule.bsl:2: ЭлементыФормы.ПолеВвода1.Значение = 1;"),
        "{w:?}"
    );
    assert!(w[0].starts_with("  warning Form.xml: "), "{w:?}");
}

#[test]
fn a_renamed_control_still_used_in_the_module_is_reported() {
    let edited = baseline().replacen(r#"<LabelDecoration name="Надпись1" id="5">"#, r#"<LabelDecoration name="НадписьНовая" id="5">"#, 1);
    let module = "Процедура П()\n\tЭлементы[\"Надпись1\"].Видимость = Ложь;\n\tЭлементыФормы.Надпись1.Заголовок = \"\";\nКонецПроцедуры\nПроцедура Надпись1Нажатие()\nКонецПроцедуры\n";
    let w = warnings(&edited, Some(module));
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w[0].contains("renamed control 'Надпись1' (now 'НадписьНовая') is still referenced at FormModule.bsl:2"), "{w:?}");
    assert!(w[1].contains("FormModule.bsl:3: ЭлементыФормы.Надпись1.Заголовок"), "{w:?}");
    // the handler stays linked by name: not an orphan
    assert!(!w.iter().any(|l| l.contains("orphan")), "{w:?}");
}

#[test]
fn a_comment_or_a_string_that_mentions_the_removed_control_is_not_a_reference() {
    let module = "// ЭлементыФормы.ПолеВвода1 убрали\nПроцедура П()\n\tСообщить(\"ЭлементыФормы.ПолеВвода1\"); // ЭлементыФормы.ПолеВвода1\n\tТекст = \"первая\n\t|ЭлементыФормы.ПолеВвода1\";\nКонецПроцедуры\n";
    assert!(warnings(&without_input(), Some(module)).is_empty());
}

#[test]
fn the_handler_of_a_removed_control_is_an_orphan() {
    let module = "Процедура Кнопка1Нажатие(Кнопка)\nКонецПроцедуры\n\nПроцедура Другая()\nКонецПроцедуры\n\nПроцедура Кнопка1ПриИзменении(Элемент)\nКонецПроцедуры\n";
    let w = warnings(&without_button(), Some(module));
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w[0].contains("procedure Кнопка1Нажатие belongs to removed control 'Кнопка1'"), "{w:?}");
    assert!(w[1].contains("procedure Кнопка1ПриИзменении belongs to removed control 'Кнопка1'"), "{w:?}");
}

#[test]
fn at_most_twenty_references_are_listed_per_control() {
    let module: String = (0..25).map(|i| format!("ЭлементыФормы.ПолеВвода1.Значение = {i};\n")).collect();
    let w = warnings(&without_input(), Some(&module));
    assert_eq!(w.len(), 21, "{w:?}");
    assert!(w[20].contains("+5 more"), "{w:?}");
}

#[test]
fn the_object_module_is_scanned_too() {
    let mut lp = plan(&without_input(), None);
    assert!(lp.summary().iter().all(|l| !l.contains("warning")));
    lp.add_module_warnings("ObjectModule.bsl", "Функция Ф(Ф)\n\tВозврат Ф.ЭлементыФормы.ПолеВвода1;\nКонецФункции\n".as_bytes());
    let w: Vec<String> = lp.summary().into_iter().filter(|l| l.contains("warning")).collect();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("removed control 'ПолеВвода1' is still referenced at ObjectModule.bsl:2"), "{w:?}");
}

fn with_field(name: &str, extra: &str, left: i64) -> String {
    let block = format!(
        "<InputField name=\"{name}\" id=\"999\">\n          <DataPath attributeId=\"1\">ОбработкаОбъект.КПП</DataPath>{extra}\n          <Left>{left}</Left><Top>500</Top><Width>120</Width><Height>20</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n        </InputField>\n        ",
    );
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    baseline().replacen(anchor, &(block + anchor), 1)
}

#[test]
fn a_field_that_keeps_the_template_type_is_reported_unless_the_type_is_given() {
    let w = warnings(&with_field("НовоеПоле", "", 520), None);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("field 'НовоеПоле' is bound to 'ОбработкаОбъект.КПП' but its value type could not be resolved; it keeps the template type (ValueList)"),
        "{w:?}"
    );
    let given = with_field("НовоеПоле", "<ValueType kind=\"String\" length=\"10\"/>", 520);
    assert!(warnings(&given, None).is_empty());
}

#[test]
fn an_added_control_beyond_its_container_is_reported_with_the_fix() {
    let w = warnings(&with_field("Далеко", "<ValueType kind=\"String\" length=\"10\"/>", 900), None);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("control 'Далеко' extends beyond its container (right edge 1020 is beyond the width"), "{w:?}");
    assert!(w[0].contains("move the control to Left") && w[0].contains("or enlarge the container to Width 1020"), "{w:?}");
    // the controls the edit does not touch are not reported, even if they stick out
    assert!(w.iter().all(|l| l.contains("'Далеко'")));
}

#[test]
fn a_moved_control_beyond_its_container_is_reported_and_one_inside_is_not() {
    // the field ПолеВвода1 (278 wide) moved to Left 900
    let doc = baseline();
    let at = doc.find("<InputField name=\"ПолеВвода1\"").unwrap();
    let left = at + doc[at..].find("<Left>").unwrap();
    let end = left + doc[left..].find("</Left>").unwrap();
    let edited = format!("{}<Left>900{}", &doc[..left], &doc[end..]);
    let w = warnings(&edited, None);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("control 'ПолеВвода1' extends beyond its container"), "{w:?}");
    let inside = format!("{}<Left>20{}", &doc[..left], &doc[end..]);
    assert!(warnings(&inside, None).is_empty());
}
