//! A `CommandBar` added together with its `<Buttons>` in one pack: the donor panel is created empty and its button table
//! is built on the patched stream (handlers, separators, `Standard:N`); the extract of the result equals the request.

mod common;

use std::path::{Path, PathBuf};

use oneform::form_patch::{enrich_ext_properties, parse_form_xml, CommandBarButton};
use oneform::oof::{decode_oof_stream, render_form_xml};
use oneform::pack::{plan_layout_patch, LayoutPatch};
use oneform::transaction::Tree;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm")
}

fn baseline() -> String {
    std::fs::read_to_string(dir().join("Form.xml")).unwrap().replace("\r\n", "\n")
}

fn tree(xml: &str, stream: &[u8]) -> Tree {
    [("Form.xml".to_string(), xml.as_bytes().to_vec()), ("FormStream.bin".to_string(), stream.to_vec())].into_iter().collect()
}

fn stream() -> Vec<u8> {
    std::fs::read(dir().join("FormStream.bin")).unwrap()
}

/// `Form.xml` with a new `CommandBar` (request id 900) holding `inner` (the children of the panel after `<AutoFill>`).
fn with_bar(anchors: &str, inner: &str) -> String {
    let block = format!(
        "<CommandBar name=\"НоваяПанель\" id=\"900\">\n          <Left>0</Left><Top>560</Top><Width>935</Width><Height>25</Height>\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>{anchors}\n          <AutoFill>false</AutoFill>{inner}\n        </CommandBar>\n        "
    );
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    assert!(baseline().contains(anchor));
    baseline().replacen(anchor, &(block + anchor), 1)
}

fn buttons(items: &str) -> String {
    format!("\n          <Buttons>{items}</Buttons>")
}

fn handler(name: &str, proc: &str, caption: &str) -> String {
    format!("<Button name=\"{name}\" kind=\"Action\" action=\"{proc}\"><Caption><Item language=\"ru\">{caption}</Item></Caption></Button>")
}

fn separator(name: &str) -> String {
    format!("<Button name=\"{name}\" kind=\"Separator\"/>")
}

fn standard(name: &str, code: u32) -> String {
    format!("<Button name=\"{name}\" kind=\"Action\" action=\"Standard:{code}\"/>")
}

fn plan(edited: &str) -> Result<LayoutPatch, oneform::errors::ExtractionError> {
    let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
    plan_layout_patch(&layout, &tree(edited, &stream()), &tree(&baseline(), &stream())).map(|p| p.expect("a layout edit"))
}

fn decoded_xml(bytes: &[u8]) -> String {
    let base = baseline();
    let doc = roxmltree::Document::parse(&base).unwrap();
    let (id, name) = (doc.root_element().attribute("id").unwrap(), doc.root_element().attribute("name").unwrap());
    let lookup = common::read_meta_lookup(&dir().join("Document.xml"));
    let model = decode_oof_stream(bytes, id, Some(&lookup)).unwrap();
    let xml = render_form_xml(&common::form_info(id, name, bytes), &model).unwrap();
    String::from_utf8(enrich_ext_properties(bytes, &xml).unwrap()).unwrap().replace("\r\n", "\n")
}

/// Packs `edited`, runs the staged check of `oneform pack` and returns the buttons of the new panel as extracted.
fn pack(edited: &str) -> (LayoutPatch, String, Vec<CommandBarButton>) {
    let lp = plan(edited).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
    let xml = decoded_xml(&lp.patched.bytes);
    lp.verify_staged(&tree(&xml, &lp.patched.bytes)).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
    let model = parse_form_xml(xml.as_bytes(), "decoded").unwrap();
    let panel = model.controls.iter().find(|c| c.name == "НоваяПанель").expect("the new panel");
    let got = panel.values.buttons.clone();
    (lp, xml, got)
}

fn shape(list: &[CommandBarButton]) -> Vec<(String, String, String, String)> {
    list.iter().map(|b| (b.name.clone(), b.kind.clone(), b.action_kind.clone(), b.action.clone())).collect()
}

#[test]
fn a_new_panel_gets_its_handler_separator_and_standard_buttons_in_one_pack() {
    let items = [handler("Выполнить", "ВыполнитьНажатие", "Выполнить"), separator("Разделитель"), standard("Закрыть", 142)].concat();
    let (lp, xml, got) = pack(&with_bar("", &buttons(&items)));
    assert_eq!(
        shape(&got),
        vec![
            ("Выполнить".into(), "Action".into(), "Handler".into(), "ВыполнитьНажатие".into()),
            ("Разделитель".into(), "Separator".into(), String::new(), String::new()),
            ("Закрыть".into(), "Action".into(), "Standard".into(), "Standard:142".into()),
        ]
    );
    assert_eq!(got[0].caption, vec![("ru".to_string(), "Выполнить".to_string())]);
    assert_eq!(got[2].caption, vec![("ru".to_string(), "Закрыть".to_string())], "the caption of the catalogue");
    assert_eq!(got.iter().map(|b| b.id.unwrap()).collect::<Vec<_>>(), vec![31, 32, 33], "the ids follow the counter of the donor panel");
    // the extract of the result packed again is no edit
    let again = plan_layout_patch(
        &serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}}),
        &tree(&xml, &lp.patched.bytes),
        &tree(&xml, &lp.patched.bytes),
    )
    .unwrap();
    assert!(again.is_none_or(|p| p.patched.replacements.is_empty()));
    // the other panels of the form are unchanged
    assert_eq!(lp.patched.allocated.len(), 1);
}

#[test]
fn the_order_of_the_buttons_is_the_order_of_the_request() {
    let items = [standard("Закрыть", 142), handler("Б", "ПроцБ", "Б"), separator("Р"), handler("А", "ПроцА", "А")].concat();
    let (_, _, got) = pack(&with_bar("", &buttons(&items)));
    assert_eq!(got.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["Закрыть", "Б", "Р", "А"]);
    assert_eq!(got.iter().map(|b| b.id.unwrap()).collect::<Vec<_>>(), vec![31, 32, 33, 34]);
}

#[test]
fn a_new_panel_without_buttons_is_as_before() {
    let (_, _, got) = pack(&with_bar("", ""));
    assert!(got.is_empty());
}

#[test]
fn the_anchor_of_the_new_panel_may_name_its_own_request_id() {
    let anchors = "\n          <Anchors top=\"Bottom\" topTarget=\"900\" topOffset=\"-113\" bottom=\"Bottom\" left=\"Left\" right=\"Right\"/>";
    let items = [standard("Закрыть", 142)].concat();
    let (lp, xml, got) = pack(&with_bar(anchors, &buttons(&items)));
    assert_eq!(got.len(), 1);
    let new_id = lp.patched.allocated[0].1;
    assert!(xml.contains(&format!("topTarget=\"{new_id}\"")), "{xml}");
}

#[test]
fn a_user_sub_menu_of_a_new_panel_is_built_with_its_buttons() {
    let sub = "<Button name=\"Действия\" kind=\"Submenu\"><Caption><Item language=\"ru\">Действия</Item></Caption><Buttons>".to_string()
        + &handler("Один", "ОдинНажатие", "Один")
        + &separator("Р2")
        + &standard("Закрыть2", 142)
        + "</Buttons></Button>";
    let items = handler("Первая", "ПерваяНажатие", "Первая") + &sub;
    let (_, _, got) = pack(&with_bar("", &buttons(&items)));
    assert_eq!(got.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["Первая", "Действия"]);
    assert_eq!(got[1].kind, "Submenu");
    assert_eq!(got[1].buttons.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["Один", "Р2", "Закрыть2"]);
}

#[test]
fn a_missing_handler_procedure_of_a_new_panel_is_a_warning() {
    let items = handler("Выполнить", "ВыполнитьНажатие", "Выполнить") + &handler("Есть", "ЕстьНажатие", "Есть");
    let edited = with_bar("", &buttons(&items));
    let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
    let mut existing = tree(&edited, &stream());
    existing.insert("FormModule.bsl".to_string(), "Процедура ЕстьНажатие(Кнопка)
КонецПроцедуры
".as_bytes().to_vec());
    let mut current = tree(&baseline(), &stream());
    current.insert("FormModule.bsl".to_string(), "Процедура ЕстьНажатие(Кнопка)
КонецПроцедуры
".as_bytes().to_vec());
    let lp = plan_layout_patch(&layout, &existing, &current).unwrap().unwrap();
    let w: Vec<String> = lp.summary().into_iter().filter(|l| l.contains("warning")).collect();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("button 'Выполнить' of 'НоваяПанель' calls the procedure 'ВыполнитьНажатие'"), "{w:?}");
}

#[test]
fn an_unsupported_button_of_a_new_panel_is_refused_with_its_line() {
    let items = standard("Х", 99999);
    let e = plan(&with_bar("", &buttons(&items))).unwrap_err();
    assert!(e.message.contains("no template for Standard:99999"), "{e}");
    let items = "<Button name=\"Пусто\" kind=\"Action\"/>";
    let e = plan(&with_bar("", &buttons(items))).unwrap_err();
    assert!(e.message.contains("needs action"), "{e}");
}

#[test]
fn another_new_control_cannot_anchor_to_the_new_panel() {
    let label = "<LabelDecoration name=\"Подпись\" id=\"901\">\n          <Caption><Item language=\"ru\">П</Item></Caption>\n          <Left>10</Left><Top>590</Top><Width>80</Width><Height>20</Height>\n          <ZOrder>51</ZOrder><TabOrder>51</TabOrder>\n          <Anchors top=\"Bottom\" topTarget=\"900\" topOffset=\"5\"/>\n        </LabelDecoration>\n        ";
    let edited = with_bar("", "").replacen("        <Table name=\"ТабличноеПоле1\"", &format!("{label}        <Table name=\"ТабличноеПоле1\""), 1);
    let e = plan(&edited).unwrap_err();
    assert!(e.message.contains("is another new control of the same edit"), "{e}");
}
