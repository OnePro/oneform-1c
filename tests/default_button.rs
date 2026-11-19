//! The default button of a command bar: `extract` shows it, `pack` writes it where the platform keeps it (the form
//! header `root[1][1][2]` names the control, `info[1][11]` of the bar names the button) and clears it again.

mod common;

use std::path::{Path, PathBuf};

use oneform::brace::parse_spanned;
use oneform::form_patch::{enrich_ext_properties, parse_form_xml, ControlIndex};
use oneform::oof::{decode_oof_stream, render_form_xml};
use oneform::pack::{plan_layout_patch, LayoutPatch};
use oneform::transaction::Tree;

const FLAG: &str = "<DefaultButton>true</DefaultButton>";
const BAR: &str = "<CommandBar name=\"ОсновныеДействияФормы\" id=\"3\">";
/// The flagged button of the Designer bar, with the line of the flag.
const FLAGGED: &str = "\n              <DefaultButton>true</DefaultButton>";

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm")
}

fn baseline() -> String {
    std::fs::read_to_string(dir().join("Form.xml")).unwrap().replace("\r\n", "\n")
}

fn stream() -> Vec<u8> {
    std::fs::read(dir().join("FormStream.bin")).unwrap()
}

fn tree(xml: &str, stream: &[u8]) -> Tree {
    [("Form.xml".to_string(), xml.as_bytes().to_vec()), ("FormStream.bin".to_string(), stream.to_vec())].into_iter().collect()
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

/// Packs, runs the staged verification of `oneform pack` and returns the plan and the extract of the result.
fn pack(edited: &str) -> (LayoutPatch, String) {
    let lp = plan(edited).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
    let xml = decoded_xml(&lp.patched.bytes);
    lp.verify_staged(&tree(&xml, &lp.patched.bytes)).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
    (lp, xml)
}

/// `(header default control, x[11] of the bar `bar`)` of a stream.
fn slots(bytes: &[u8], bar: i64) -> (i64, i64) {
    let s = parse_spanned(bytes).unwrap();
    let header = s.root.node(1).unwrap().node(1).unwrap().atom(2).unwrap().value.parse().unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let slot = idx.get(bar).unwrap().info().unwrap().node.node(1).unwrap().atom(11).unwrap().value.parse().unwrap();
    (header, slot)
}

/// The names of the buttons of `xml` that hold the flag.
fn flagged(xml: &str) -> Vec<String> {
    let model = parse_form_xml(xml.as_bytes(), "x").unwrap();
    let mut out = Vec::new();
    fn walk(list: &[oneform::form_patch::CommandBarButton], out: &mut Vec<String>) {
        for b in list {
            if b.default_button == Some(true) {
                out.push(b.name.clone());
            }
            walk(&b.buttons, out);
        }
    }
    for c in model.controls.iter().filter(|c| c.type_name == "CommandBar") {
        walk(&c.values.buttons, &mut out);
    }
    out
}

fn without_flag(xml: &str) -> String {
    assert_eq!(xml.matches(FLAGGED).count(), 1);
    xml.replacen(FLAGGED, "", 1)
}

/// `xml` without the bar 3.
fn without_bar(xml: &str) -> String {
    let at = xml.find(BAR).unwrap();
    let line = xml[..at].rfind('\n').unwrap() + 1;
    let end = at + xml[at..].find("</CommandBar>\n").unwrap() + "</CommandBar>\n".len();
    format!("{}{}", &xml[..line], &xml[end..])
}

fn handler(name: &str, flag: bool) -> String {
    let flag = if flag { FLAG } else { "" };
    format!("<Button name=\"{name}\" kind=\"Action\" action=\"{name}Нажатие\"><Caption><Item language=\"ru\">{name}</Item></Caption>{flag}</Button>")
}

/// A new bar `НижняяПанель` (request id 900) with `buttons`, before the table.
fn with_bar(xml: &str, buttons: &str) -> String {
    let block = format!(
        "<CommandBar name=\"НижняяПанель\" id=\"900\">\n          <Left>0</Left><Top>560</Top><Width>935</Width><Height>25</Height>\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>\n          <AutoFill>false</AutoFill>\n          <Buttons>{buttons}</Buttons>\n        </CommandBar>\n        "
    );
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    assert!(xml.contains(anchor));
    xml.replacen(anchor, &(block + anchor), 1)
}

/// `xml` with `<DefaultButton>` after the `<TabOrder>` of the control `name`.
fn flag_control(xml: &str, name: &str) -> String {
    let at = xml.find(&format!("name=\"{name}\" id=")).unwrap();
    let tab = at + xml[at..].find("</TabOrder>").unwrap() + "</TabOrder>".len();
    format!("{}\n          {FLAG}{}", &xml[..tab], &xml[tab..])
}

fn refused_kind(edited: &str) -> String {
    let e = plan(edited).expect_err("must be refused");
    e.params["kind"].as_str().unwrap_or_default().to_string()
}

#[test]
fn extract_shows_the_default_button_of_the_designer_bar_only() {
    let xml = decoded_xml(&stream());
    assert_eq!(xml.matches(FLAG).count(), 1);
    assert_eq!(flagged(&xml), ["ОсновныеДействияФормыВыполнить"]);
    let at = xml.find("name=\"ОсновныеДействияФормыВыполнить\"").unwrap();
    let flag = xml.find(FLAG).unwrap();
    assert!(at < flag && flag < xml.find("name=\"Разделитель\"").unwrap(), "the flag sits in the Выполнить button");
    assert_eq!(xml, baseline(), "the fixture is the extract");
}

#[test]
fn a_new_bottom_bar_gets_the_header_and_its_default_button() {
    let edited = with_bar(&without_bar(&baseline()), &format!("{}{}", handler("Первая", true), handler("Вторая", false)));
    let (lp, xml) = pack(&edited);
    let bar = lp.patched.allocated.iter().find(|(x, _)| *x == 900).unwrap().1;
    let list = &lp.patched.button_lists.iter().find(|(id, _)| *id == bar).unwrap().1;
    let first = list[0].id.unwrap();
    assert_eq!(slots(&lp.patched.bytes, bar), (bar, first));
    assert_eq!(flagged(&xml), ["Первая"]);
}

#[test]
fn a_new_button_with_the_flag_joins_an_existing_bar() {
    let bar = baseline().find(BAR).unwrap();
    let close = bar + baseline()[bar..].find("</Buttons>").unwrap();
    let edited = format!("{}  {}\n          {}", &baseline()[..close - 10], handler("Новая", true), &baseline()[close..]);
    assert!(edited.contains("name=\"Новая\""));
    let (lp, xml) = pack(&without_flag(&edited));
    assert_eq!(slots(&lp.patched.bytes, 3), (3, 4), "the new button takes the id 4");
    assert_eq!(flagged(&xml), ["Новая"]);
}

#[test]
fn the_flag_moves_between_the_buttons_of_a_bar() {
    let edited = without_flag(&baseline()).replacen(
        "<Item language=\"ru\">Закрыть</Item>\n                <Item language=\"ro\">Закрыть</Item>\n              </Caption>",
        &format!("<Item language=\"ru\">Закрыть</Item>\n                <Item language=\"ro\">Закрыть</Item>\n              </Caption>\n              {FLAG}"),
        1,
    );
    let (lp, xml) = pack(&edited);
    assert_eq!(slots(&lp.patched.bytes, 3), (3, 3), "only the bar slot changes, the header stays");
    assert_eq!(flagged(&xml), ["ОсновныеДействияФормыЗакрыть"]);
    let before = stream();
    let changed = lp.patched.replacements.len();
    assert_eq!(changed, 1, "one replacement, x[11]: {:?}", lp.patched.replacements.iter().map(|r| r.range).collect::<Vec<_>>());
    assert_eq!(slots(&before, 3), (3, 1));
}

#[test]
fn a_bar_button_that_loses_the_flag_clears_the_header_and_the_bar() {
    let (lp, xml) = pack(&without_flag(&baseline()));
    assert_eq!(slots(&lp.patched.bytes, 3), (0, 0));
    assert!(flagged(&xml).is_empty());
    // the same with an explicit `false`
    let explicit = baseline().replacen(FLAG, "<DefaultButton>false</DefaultButton>", 1);
    let (lp, xml) = pack(&explicit);
    assert_eq!(slots(&lp.patched.bytes, 3), (0, 0));
    assert!(flagged(&xml).is_empty());
}

// A `Button` keeps its default flag in `info[1][5]`, the slot that `MenuMode` reads too, so the staged verification of
// `pack` cannot tell them apart: the two tests below check the patched stream, not the staged extract.
#[test]
fn the_flag_moves_from_a_bar_button_to_a_button_of_the_form() {
    let lp = plan(&flag_control(&without_flag(&baseline()), "Кнопка1")).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
    assert_eq!(slots(&lp.patched.bytes, 3), (4, 0));
    assert!(flagged(&decoded_xml(&lp.patched.bytes)).is_empty(), "no bar button holds it any more");
}

#[test]
fn the_flag_moves_from_a_button_to_a_bar_button() {
    // the packed form of the previous test is the new baseline
    let packed = plan(&flag_control(&without_flag(&baseline()), "Кнопка1")).unwrap().patched.bytes;
    let base = decoded_xml(&packed);
    let tooltip_end = "<Item language=\"ro\">Кнопка выполнить нажатие</Item>\n              </Tooltip>";
    let edited = base.replacen(tooltip_end, &format!("{tooltip_end}\n              {FLAG}"), 1);
    let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
    let lp = plan_layout_patch(&layout, &tree(&edited, &packed), &tree(&base, &packed)).unwrap().expect("a layout edit");
    assert_eq!(slots(&lp.patched.bytes, 3), (3, 1));
}

#[test]
fn removing_the_default_bar_clears_the_header() {
    let (lp, _) = pack(&without_bar(&baseline()));
    let s = parse_spanned(&lp.patched.bytes).unwrap();
    assert_eq!(s.root.node(1).unwrap().node(1).unwrap().atom(2).unwrap().value, "0");
}

#[test]
fn two_defaults_in_a_changed_request_are_refused() {
    // two bar buttons
    let two = baseline().replacen(
        "<Item language=\"ru\">Закрыть</Item>\n                <Item language=\"ro\">Закрыть</Item>\n              </Caption>",
        &format!("<Item language=\"ru\">Закрыть</Item>\n                <Item language=\"ro\">Закрыть</Item>\n              </Caption>\n              {FLAG}"),
        1,
    );
    assert_eq!(refused_kind(&two), "duplicate-default-button");
    // a bar button and a Button
    assert_eq!(refused_kind(&flag_control(&baseline(), "Кнопка1")), "duplicate-default-button");
    // a new bar button and the old one
    let new = with_bar(&baseline(), &handler("Первая", true));
    assert_eq!(refused_kind(&new), "duplicate-default-button");
}

#[test]
fn only_an_action_button_can_be_the_default_button() {
    let edited = without_flag(&baseline()).replacen(
        "<Button name=\"Разделитель\" id=\"2\" kind=\"Separator\" checked=\"false\" representation=\"Text\"/>",
        &format!("<Button name=\"Разделитель\" id=\"2\" kind=\"Separator\" checked=\"false\" representation=\"Text\">{FLAG}</Button>"),
        1,
    );
    let e = plan(&edited).expect_err("must be refused");
    assert_eq!(e.params["kind"].as_str(), Some("unsupported-button"));
    assert!(e.to_string().contains("action button"), "{e}");
}
