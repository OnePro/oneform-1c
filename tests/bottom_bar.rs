//! An added bottom command bar is written like the Designer's: border `ЧертаСверху`, the Designer defaults for what the
//! XML omits, a note naming them and a warning for a placement that differs from the Designer's.

mod common;

use std::path::{Path, PathBuf};

use oneform::form_patch::{enrich_ext_properties, parse_form_xml};
use oneform::oof::{decode_oof_stream, render_form_xml};
use oneform::pack::{plan_layout_patch, LayoutPatch};
use oneform::transaction::Tree;

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

/// A new bar `НижняяПанель` (request id 900) in the page `Страница1` of the form: geometry `geo`, then `tags`.
fn with_bar(geo: &str, tags: &str) -> String {
    let block = format!(
        "<CommandBar name=\"НижняяПанель\" id=\"900\">\n          {geo}\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>{tags}\n          <Buttons><Button name=\"Закрыть\" kind=\"Action\" action=\"Standard:142\"/></Buttons>\n        </CommandBar>\n        "
    );
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    baseline().replacen(anchor, &(block + anchor), 1)
}

fn geometry(left: i64, top: i64, width: i64, height: i64) -> String {
    format!("<Left>{left}</Left><Top>{top}</Top><Width>{width}</Width><Height>{height}</Height>")
}

fn plan(edited: &str) -> LayoutPatch {
    let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
    plan_layout_patch(&layout, &tree(edited, &stream()), &tree(&baseline(), &stream()))
        .unwrap_or_else(|e| panic!("{e} {:?}", e.params))
        .expect("a layout edit")
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

/// Packs, runs the staged verification and returns the plan and the extract of the result.
fn pack(edited: &str) -> (LayoutPatch, String) {
    let lp = plan(edited);
    let xml = decoded_xml(&lp.patched.bytes);
    lp.verify_staged(&tree(&xml, &lp.patched.bytes)).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
    (lp, xml)
}

fn count(bytes: &[u8], needle: &str) -> usize {
    String::from_utf8_lossy(bytes).matches(needle).count()
}

const LINE_ABOVE: &str = "{3,0,{0},7,1,0,";
const UNDERLINE: &str = "{3,0,{0},4,1,0,";

fn lines(lp: &LayoutPatch, kind: &str) -> Vec<String> {
    lp.summary().into_iter().filter(|l| l.starts_with(&format!("  {kind} "))).collect()
}

fn bar_xml(xml: &str) -> String {
    let at = xml.find("<CommandBar name=\"НижняяПанель\"").expect("the bar");
    xml[at..xml[at..].find("</CommandBar>").unwrap() + at].to_string()
}

#[test]
fn a_minimal_bottom_bar_gets_the_designer_defaults_and_the_border() {
    let base = stream();
    let (lp, xml) = pack(&with_bar(&geometry(0, 648, 935, 25), ""));
    let id = lp.patched.allocated[0].1;
    let bar = bar_xml(&xml);
    assert!(bar.contains("<HorizontalAlign>Right</HorizontalAlign>"), "{bar}");
    assert!(bar.contains("<AutoFill>false</AutoFill>"), "{bar}");
    assert!(bar.contains(&format!("<Anchors top=\"Bottom\" topTarget=\"{id}\" topOffset=\"-25\" bottom=\"Bottom\" left=\"Left\" right=\"Right\"/>")), "{bar}");
    assert_eq!(count(&lp.patched.bytes, LINE_ABOVE), count(&base, LINE_ABOVE) + 1);
    assert_eq!(count(&lp.patched.bytes, UNDERLINE), count(&base, UNDERLINE));
    let notes = lines(&lp, "note");
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("note Form.xml: bottom command bar 'НижняяПанель' gets the Designer defaults: HorizontalAlign Right, AutoFill false, anchors <Anchors top=\"Bottom\" topTarget=\"900\" topOffset=\"-25\""), "{notes:?}");
    assert!(notes[0].ends_with("border ЧертаСверху"), "{notes:?}");
    assert!(lines(&lp, "warning").is_empty());
}

#[test]
fn explicit_values_win_and_the_border_is_still_the_line_above() {
    let base = stream();
    let tags = "<Anchors top=\"Bottom\" topTarget=\"900\" topOffset=\"-30\" bottom=\"Bottom\" left=\"Left\" right=\"Right\"/><HorizontalAlign>Left</HorizontalAlign>";
    let (lp, xml) = pack(&with_bar(&geometry(0, 648, 935, 25), tags));
    let bar = bar_xml(&xml);
    assert!(!bar.contains("<HorizontalAlign>Right"), "{bar}");
    assert!(bar.contains("topOffset=\"-30\""), "{bar}");
    assert!(bar.contains("<AutoFill>false</AutoFill>"), "{bar}");
    assert_eq!(count(&lp.patched.bytes, LINE_ABOVE), count(&base, LINE_ABOVE) + 1);
    let notes = lines(&lp, "note");
    assert!(notes[0].contains("AutoFill false") && !notes[0].contains("HorizontalAlign") && !notes[0].contains("anchors"), "{notes:?}");
    let w = lines(&lp, "warning");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("anchors -> <Anchors") && w[0].contains("HorizontalAlign Left -> Right"), "{w:?}");
}

#[test]
fn a_bar_at_the_top_or_in_a_panel_page_is_written_as_before() {
    let base = stream();
    let (lp, xml) = pack(&with_bar(&geometry(0, 0, 935, 25), ""));
    assert!(!bar_xml(&xml).contains("<HorizontalAlign>Right"));
    assert_eq!(count(&lp.patched.bytes, LINE_ABOVE), count(&base, LINE_ABOVE));
    assert_eq!(count(&lp.patched.bytes, UNDERLINE), count(&base, UNDERLINE) + 1);
    assert!(lines(&lp, "note").is_empty() && lines(&lp, "warning").is_empty());

    // a bar in the page of a Panel, its bottom edge on the bottom of the page
    let page = "              <Height>102</Height>\n            </Page>";
    let block = "              <Height>102</Height>\n              <ChildItems>\n                <CommandBar name=\"НижняяПанель\" id=\"900\"><Left>0</Left><Top>77</Top><Width>147</Width><Height>25</Height><ZOrder>50</ZOrder><TabOrder>50</TabOrder></CommandBar>\n              </ChildItems>\n            </Page>";
    assert!(baseline().contains(page));
    let edited = baseline().replacen(page, block, 1);
    let lp = plan(&edited);
    assert_eq!(count(&lp.patched.bytes, LINE_ABOVE), count(&base, LINE_ABOVE));
    assert_eq!(count(&lp.patched.bytes, UNDERLINE), count(&base, UNDERLINE) + 1);
    assert!(lines(&lp, "note").is_empty() && lines(&lp, "warning").is_empty());
}

#[test]
fn a_bar_placed_like_scany_is_reported_and_kept() {
    // the page `Страница1` is 935 x 673: a bar 6 px above the bottom, at Left 116, 480 wide
    let tags = "<Anchors top=\"Bottom\" topTarget=\"900\" topOffset=\"-31\" bottom=\"Bottom\" bottomOffset=\"-6\" left=\"Right\" leftOffset=\"-819\" right=\"Right\"/>";
    let tags = &format!("{tags}<HorizontalAlign>Left</HorizontalAlign>");
    let lp = plan(&with_bar(&geometry(116, 642, 480, 25), tags));
    let w = lines(&lp, "warning");
    assert_eq!(w.len(), 1, "{w:?}");
    for part in ["Left 116 -> 0", "Width 480 -> 935", "Top 642 -> 648", "anchors -> <Anchors top=\"Bottom\" topTarget=\"900\" topOffset=\"-25\"", "HorizontalAlign Left -> Right"] {
        assert!(w[0].contains(part), "{part}: {w:?}");
    }
    let xml = decoded_xml(&lp.patched.bytes);
    let bar = bar_xml(&xml);
    assert!(bar.contains("<Left>116</Left>") && bar.contains("<Width>480</Width>"), "{bar}");
    assert!(parse_form_xml(xml.as_bytes(), "decoded").is_ok());
}

#[test]
fn a_bottom_bar_is_not_secondary_and_its_buttons_show_their_text() {
    let (lp, xml) = pack(&with_bar(&geometry(0, 648, 935, 25), ""));
    let bar = bar_xml(&xml);
    assert!(bar.contains("<Secondary>false</Secondary>"), "{bar}");
    assert!(bar.contains("representation=\"Text\""), "{bar}");
    let notes = lines(&lp, "note");
    assert!(notes[0].contains("Secondary false") && notes[0].contains("buttons representation Text"), "{notes:?}");

    // explicit values win
    let edited = with_bar(&geometry(0, 648, 935, 25), "\n          <Secondary>true</Secondary>")
        .replace("<Button name=\"Закрыть\" kind=\"Action\" action=\"Standard:142\"/>", "<Button name=\"Закрыть\" kind=\"Action\" representation=\"Picture\" action=\"Standard:142\"/>");
    let (lp, xml) = pack(&edited);
    let bar = bar_xml(&xml);
    assert!(!bar.contains("<Secondary>false</Secondary>"), "{bar}");
    assert!(bar.contains("representation=\"Picture\""), "{bar}");
    let notes = lines(&lp, "note");
    assert!(!notes[0].contains("Secondary") && !notes[0].contains("representation"), "{notes:?}");
}
