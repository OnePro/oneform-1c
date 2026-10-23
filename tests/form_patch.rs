//! Tasks 3.1-3.3 and 4.1-4.6: semantic `Form.xml` models, control identity index, the conservative diff and the
//! lossless property patchers (`oneform::form_patch`). Uses the real ordinary-form fixtures; patched streams are
//! decoded by the normal OOF decoder and compared with the requested XML.

mod common;

use std::path::{Path, PathBuf};

use oneform::brace::{parse_spanned, Replacement, SpannedMetadata};
use oneform::errors::{ExtractionError, GENERATED_FILE_MODIFIED, METADATA_LAYOUT_UNSUPPORTED};
use oneform::form_patch::{
    diff_forms, parse_form_xml, patch_form, plan_form_patch, ControlIndex, FormPatchPlan, NodeRef, PatchedForm, Property,
};
use oneform::oof::{decode_oof_stream, render_form_xml};

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms").join(name)
}

fn stream(name: &str) -> SpannedMetadata {
    parse_spanned(&std::fs::read(dir(name).join("FormStream.bin")).unwrap()).unwrap()
}

fn baseline(name: &str) -> String {
    std::fs::read_to_string(dir(name).join("Form.xml")).unwrap().replace("\r\n", "\n")
}

fn replace_once(xml: &str, from: &str, to: &str) -> String {
    assert_eq!(xml.matches(from).count(), 1, "fixture anchor must be unique: {from}");
    xml.replacen(from, to, 1)
}

fn plan<'a>(s: &'a SpannedMetadata, edited: &str) -> Result<FormPatchPlan<'a>, ExtractionError> {
    plan_form_patch(s, baseline("DonorForm").as_bytes(), edited.as_bytes())
}

/// The rejection error: `kind` and the full text (message + problems) for assertions.
fn rejected(edited: &str) -> (String, String) {
    let s = stream("DonorForm");
    let e = plan(&s, edited).expect_err("must be rejected");
    assert_eq!(e.code, GENERATED_FILE_MODIFIED, "{e}");
    let kind = e.params["kind"].as_str().unwrap().to_string();
    (kind, format!("{e} {}", e.params.get("problems").map(|p| p.to_string()).unwrap_or_default()))
}

/// Replace the first `from` after `marker` (unique), for anchors that repeat across controls.
fn mutate_in(marker: &str, from: &str, to: &str) -> String {
    let xml = baseline("DonorForm");
    let at = replace_once(&xml, marker, marker).find(marker).unwrap();
    let off = at + xml[at..].find(from).unwrap();
    format!("{}{}{}", &xml[..off], to, &xml[off + from.len()..])
}

fn mutate(from: &str, to: &str) -> String {
    replace_once(&baseline("DonorForm"), from, to)
}

/// Cut the first `open ... close` block (starting at the line indentation of `open`) out of `xml`.
fn cut_block(xml: &str, open: &str, close: &str) -> (String, String) {
    let tag_at = xml.find(open).unwrap();
    let start = xml[..tag_at].rfind('\n').unwrap() + 1;
    let end = tag_at + xml[tag_at..].find(close).unwrap() + close.len() + 1;
    (format!("{}{}", &xml[..start], &xml[end..]), xml[start..end].to_string())
}

// ---------------------------------------------------------------- 3.1

#[test]
fn identical_and_reformatted_xml_produce_no_edits() {
    let s = stream("DonorForm");
    assert!(plan(&s, &baseline("DonorForm")).unwrap().is_empty());

    // CRLF, tab indentation, blank lines, reordered attributes, expanded empty element, comment, BOM.
    let mut xml = mutate(r#"<InputField name="ПолеВвода1" id="7">"#, r#"<InputField id="7" name="ПолеВвода1">"#);
    xml = xml
        .lines()
        .map(|l| format!("{}{}", "\t".repeat((l.len() - l.trim_start().len()) / 2), l.trim_start()))
        .collect::<Vec<_>>()
        .join("\n");
    xml = xml.replace(
        "<MainAttribute attributeId=\"1\"/>",
        "<MainAttribute\n\t\tattributeId=\"1\"></MainAttribute>\n\n   <!-- comment -->",
    );
    xml = xml.replace("<Form id=", "<Form\n  id=").replace('\n', "\r\n\r\n");
    xml = format!("\u{feff}{xml}");
    assert!(plan(&s, &xml).unwrap().is_empty());
}

#[test]
fn malformed_xml_is_actionable() {
    let (kind, text) = rejected("<Form name=\"x\"><Caption></Form>");
    assert_eq!(kind, "malformed-xml");
    assert!(text.contains("edited Form.xml is not valid"), "{text}");
    let (kind, _) = rejected("");
    assert_eq!(kind, "malformed-xml");
    let (_, text) = rejected("<Other/>");
    assert!(text.contains("root element must be <Form>"), "{text}");
    let (kind, text) = rejected(&mutate("<Left>79</Left>", "<Left>abc</Left>"));
    assert_eq!(kind, "invalid-xml");
    assert!(text.contains("Надпись1") && text.contains("<Left>") && text.contains("line "), "{text}");
    let (_, text) = rejected(&mutate("<Left>79</Left>", ""));
    assert!(text.contains("required <Left>"), "{text}");
    let (_, text) = rejected(&mutate("<Left>79</Left>", "<Left>79</Left><Left>80</Left>"));
    assert!(text.contains("duplicate <Left>"), "{text}");
    let (_, text) = rejected(&mutate(r#"<LabelDecoration name="Надпись1" id="5">"#, r#"<LabelDecoration name="Надпись1" id="x">"#));
    assert!(text.contains("non-integer 'id'"), "{text}");
}

#[test]
fn malformed_baseline_is_reported_as_baseline() {
    let s = stream("DonorForm");
    let e = plan_form_patch(&s, b"<Form", baseline("DonorForm").as_bytes()).unwrap_err();
    assert!(e.message.starts_with("baseline Form.xml"), "{e}");
}

// ---------------------------------------------------------------- 3.2

#[test]
fn index_matches_every_control_of_both_fixtures() {
    for name in ["DonorForm", "DonorFormAlt"] {
        let s = stream(name);
        let index = ControlIndex::build(&s).unwrap();
        let xml = parse_form_xml(baseline(name).as_bytes(), "baseline").unwrap();
        assert!(!xml.controls.is_empty());
        assert_eq!(index.entries.len(), xml.controls.len(), "{name}");
        index.validate(&xml).unwrap();
        for e in &index.entries {
            let c = xml.control(e.id).unwrap();
            assert_eq!((c.name.as_str(), c.type_name.as_str(), c.parent), (e.name.as_str(), e.type_name, e.parent));
            // Node + SpanNode correspond: the span covers `{`..`}` of the control record.
            let bytes = s.slice(e.control.span.span).unwrap();
            assert_eq!((bytes[0], bytes[bytes.len() - 1]), (b'{', b'}'));
            assert_eq!(e.control.span.children.len(), e.control.node.children.len());
            let geom = e.geom().unwrap();
            assert_eq!(geom.span.children.len(), geom.node.children.len());
            assert!(e.meta().is_some() && e.info().is_some() && e.children_list().is_some());
        }
    }
}

#[test]
fn identity_errors_are_actionable() {
    // Deleted (whole control removed).
    let (deleted, _) = cut_block(&baseline("DonorForm"), "<LabelDecoration name=\"Надпись1\"", "</LabelDecoration>");
    let (kind, text) = rejected(&deleted);
    assert_eq!(kind, "deleted-control");
    assert!(text.contains("Надпись1") && text.contains("id 5"), "{text}");

    // Added (new id) and duplicate (existing id reused).
    let new_ctl = |id: u32| {
        format!(
            "<LabelDecoration name=\"Новая\" id=\"{id}\"><Left>0</Left><Top>0</Top><Width>1</Width><Height>1</Height><ZOrder>0</ZOrder></LabelDecoration>\n        <Table name=\"ТабличноеПоле1\""
        )
    };
    let (kind, text) = rejected(&mutate("<Table name=\"ТабличноеПоле1\"", &new_ctl(999)));
    assert_eq!(kind, "added-control");
    assert!(text.contains("Новая") && text.contains("999"), "{text}");
    let (kind, text) = rejected(&mutate("<Table name=\"ТабличноеПоле1\"", &new_ctl(5)));
    assert_eq!(kind, "duplicate-control");
    assert!(text.contains("id 5"), "{text}");

    // Renamed and retyped.
    let (kind, text) = rejected(&mutate(r#"name="Надпись1" id="5""#, r#"name="Другое" id="5""#));
    assert_eq!(kind, "renamed-control");
    assert!(text.contains("Надпись1") && text.contains("Другое"), "{text}");
    let retyped = mutate(r#"<LabelDecoration name="Надпись1" id="5">"#, r#"<InputField name="Надпись1" id="5">"#)
        .replace("</LabelDecoration>", "</InputField>");
    let (kind, text) = rejected(&retyped);
    assert_eq!(kind, "retyped-control");
    assert!(text.contains("LabelDecoration") && text.contains("InputField"), "{text}");
}

#[test]
fn moved_control_between_parents_is_rejected() {
    // Move "Надпись1" (id 5) from the first page's ChildItems into a Panel1 page.
    let (without, block) = cut_block(&baseline("DonorForm"), "<LabelDecoration name=\"Надпись1\"", "</LabelDecoration>");
    let anchor = "<ChildItems>\n                <Table name=\"ТабличноеПоле2\"";
    let moved = replace_once(&without, anchor, &format!("<ChildItems>\n{block}                <Table name=\"ТабличноеПоле2\""));
    let (kind, text) = rejected(&moved);
    assert_eq!(kind, "moved-control", "{text}");
    assert!(text.contains("Надпись1"), "{text}");
}

#[test]
fn index_rejects_unmappable_layout() {
    let s = parse_spanned(b"{27,{18}}").unwrap();
    let e = ControlIndex::build(&s).unwrap_err();
    assert_eq!(e.code, METADATA_LAYOUT_UNSUPPORTED);
}

// ---------------------------------------------------------------- 3.3

#[test]
fn supported_edits_are_classified_with_old_and_new_values() {
    let s = stream("DonorForm");
    let mut xml = baseline("DonorForm");
    xml = replace_once(&xml, "<Item language=\"ru\">Надпись1</Item>", "<Item language=\"ru\">Новая</Item><Item language=\"en\">New</Item>");
    xml = replace_once(
        &xml,
        "<Left>79</Left>\n          <Top>42</Top>\n          <Width>53</Width>\n          <Height>17</Height>\n          <ZOrder>3</ZOrder>\n          <TabOrder>3</TabOrder>",
        "<Left>80</Left>\n          <Top>43</Top>\n          <Width>60</Width>\n          <Height>18</Height>\n          <ZOrder>9</ZOrder>\n          <Visible>false</Visible>",
    );
    // A second control changes TabOrder only.
    xml = replace_once(&xml, "<TabOrder>4</TabOrder>\n          <ValueType kind=\"ValueList\"", "<TabOrder>40</TabOrder>\n          <ValueType kind=\"ValueList\"");
    let p = plan(&s, &xml).unwrap();
    assert!(!p.is_empty());
    assert_eq!(p.diff.edits.len(), 2);
    let label = &p.diff.edits[0];
    assert_eq!((label.id, label.name.as_str(), label.type_name.as_str()), (5, "Надпись1", "LabelDecoration"));
    assert_eq!(
        label.changed,
        vec![Property::Caption, Property::Left, Property::Top, Property::Width, Property::Height, Property::Visible, Property::ZOrder, Property::TabOrder]
    );
    assert_eq!(label.old.caption, vec![("ru".to_string(), "Надпись1".to_string())]);
    assert_eq!(label.new.caption.len(), 2);
    assert_eq!((label.old.left, label.new.left, label.old.visible, label.new.visible), (79, 80, true, false));
    assert_eq!((label.old.tab_order, label.new.tab_order), (3, 0));
    let input = &p.diff.edits[1];
    assert_eq!((input.id, input.changed.clone()), (7, vec![Property::TabOrder]));
    // Patchers get the matching binary node + span.
    for (edit, entry) in p.targets() {
        assert_eq!((entry.id, entry.name.as_str()), (edit.id, edit.name.as_str()));
        assert!(entry.geom().is_some());
    }
}

#[test]
fn visible_default_and_property_order_are_semantic_not_textual() {
    let s = stream("DonorForm");
    let xml = mutate("<Left>79</Left>", "<Visible>true</Visible><Left>79</Left>");
    assert!(plan(&s, &xml).unwrap().is_empty());
    let xml = mutate("<Left>79</Left>\n          <Top>42</Top>", "<Top>42</Top>\n          <Left>79</Left>");
    assert!(plan(&s, &xml).unwrap().is_empty());
}

fn assert_unsupported(edited: String, kind: &str, needles: &[&str]) {
    let (k, text) = rejected(&edited);
    assert_eq!(k, kind, "{text}");
    for n in needles {
        assert!(text.contains(n), "missing {n:?} in {text}");
    }
}

#[test]
fn form_level_differences_are_rejected() {
    assert_unsupported(mutate("<Width>937</Width>", "<Width>938</Width>"), "unsupported-form-value", &["form 'Форма'", "Width"]);
    assert_unsupported(
        mutate("<Item language=\"ru\">Обработка  ВнешняяОбработка1</Item>", "<Item language=\"ru\">X</Item>"),
        "unsupported-form-value",
        &["Caption"],
    );
    assert_unsupported(mutate("<MainAttribute attributeId=\"1\"/>", "<MainAttribute attributeId=\"2\"/>"), "unsupported-form-value", &["MainAttribute"]);
    assert_unsupported(mutate("name=\"ОбработкаОбъект\"", "name=\"Другое\""), "unsupported-form-value", &["Attributes"]);
    assert_unsupported(mutate("ordinaryFormVersion=\"2.1\"", "ordinaryFormVersion=\"2.2\""), "unsupported-form-attribute", &["ordinaryFormVersion"]);
    assert_unsupported(mutate("reconstructionComplete=\"false\"", "reconstructionComplete=\"true\""), "unsupported-form-attribute", &["reconstructionComplete"]);
}

#[test]
fn event_differences_are_rejected() {
    assert_unsupported(mutate(">ПередОткрытием<", ">Другой<"), "unsupported-event", &["form 'Форма'", "BeforeOpen"]);
    assert_unsupported(mutate("<OnOpen id=\"70001\">", "<OnOpen id=\"70099\">"), "unsupported-event", &["OnOpen"]);
    assert_unsupported(mutate(">Надпись1Нажатие<", ">Иначе<"), "unsupported-event", &["Надпись1", "Click"]);
    assert_unsupported(
        mutate("          <Events>\n            <Click id=\"0\">Надпись1Нажатие</Click>\n          </Events>\n", ""),
        "unsupported-event",
        &["Events"],
    );
}

#[test]
fn panel_table_and_command_bar_differences_are_rejected() {
    assert_unsupported(mutate("member=\"Unknown\" code=\"2\"", "member=\"Top\" code=\"1\""), "unsupported-panel", &["Панель1", "ShowTabs"]);
    assert_unsupported(
        mutate(
            "<Page name=\"Страница2\">\n              <Title>\n                <Item language=\"ru\">Страница2</Item>",
            "<Page name=\"Страница2\">\n              <Title>\n                <Item language=\"ru\">Стр</Item>",
        ),
        "unsupported-panel",
        &["Панель1", "Page[name=Страница2]"],
    );
    assert_unsupported(
        mutate(
            "<Width>276</Width>\n              <Height>134</Height>\n              <Visible>false</Visible>",
            "<Width>277</Width>\n              <Height>134</Height>\n              <Visible>false</Visible>",
        ),
        "unsupported-panel",
        &["Панель1"],
    );
    assert_unsupported(mutate("              <Visible>false</Visible>\n", ""), "unsupported-panel", &["Visible"]);
    assert_unsupported(mutate_in("<Table name=\"ТабличноеПоле1\"", "<Footer>false</Footer>", "<Footer>true</Footer>"), "unsupported-property", &["Table", "Footer"]);
    assert_unsupported(mutate_in("<Table name=\"ТабличноеПоле1\"", "<Editor>InputField</Editor>", "<Editor>Other</Editor>"), "unsupported-table", &["ТабличноеПоле1", "Columns"]);
    assert_unsupported(
        mutate("<Item language=\"ru\">Колонка1</Item>\n              </Header>", "<Item language=\"ru\">К</Item>\n              </Header>"),
        "unsupported-table",
        &["Columns"],
    );
    assert_unsupported(
        mutate(
            "<Item language=\"ru\">Выполнить</Item>\n                <Item language=\"ro\">Выполнить</Item>",
            "<Item language=\"ru\">Run</Item>\n                <Item language=\"ro\">Выполнить</Item>",
        ),
        "unsupported-command-bar",
        &["ОсновныеДействияФормы", "Buttons"],
    );
    assert_unsupported(mutate("<HorizontalAlign>Right</HorizontalAlign>", "<HorizontalAlign>Left</HorizontalAlign>"), "unsupported-command-bar", &["HorizontalAlign"]);
}

#[test]
fn property_attribute_and_unknown_element_differences_are_rejected() {
    assert_unsupported(
        mutate("<DataPath attributeId=\"3\">ПолеВвода1</DataPath>", "<DataPath attributeId=\"4\">ПолеВвода1</DataPath>"),
        "unsupported-property",
        &["ПолеВвода1", "DataPath"],
    );
    assert_unsupported(mutate_in(r#"<InputField name="ПолеВвода1" id="7">"#, "<ChoiceButton>true</ChoiceButton>", "<ChoiceButton>false</ChoiceButton>"), "unsupported-property", &["ChoiceButton"]);
    assert_unsupported(
        mutate_in(
            r#"<InputField name="ПолеВвода1" id="7">"#,
            "<ValueType kind=\"ValueList\" composite=\"false\"/>",
            "<ValueType kind=\"String\" composite=\"false\"/>",
        ),
        "unsupported-property",
        &["ValueType"],
    );
    assert_unsupported(
        mutate_in(r#"<InputField name="ПолеВвода1" id="7">"#, "<ChoiceButton>true</ChoiceButton>", "<ChoiceButton>true</ChoiceButton><Surprise>1</Surprise>"),
        "unsupported-unknown-element",
        &["ПолеВвода1", "Surprise"],
    );
    assert_unsupported(mutate("<DataProcessorFormExtension/>", "<DataProcessorFormExtension/><Surprise/>"), "unsupported-unknown-element", &["Surprise"]);
    assert_unsupported(
        mutate("<DataProcessorFormExtension/>", "<DataProcessorFormExtension><Inner/></DataProcessorFormExtension>"),
        "unsupported-form-value",
        &["DataProcessorFormExtension"],
    );
    assert_unsupported(
        mutate(r#"<InputField name="ПолеВвода1" id="7">"#, r#"<InputField name="ПолеВвода1" id="7" extra="1">"#),
        "unsupported-control-attribute",
        &["extra", "ПолеВвода1"],
    );
    assert_unsupported(
        mutate(r#"<InputField name="ПолеВвода1" id="7">"#, r#"<InputField name="ПолеВвода1" id="7">stray"#),
        "unsupported-control-attribute",
        &["text changed"],
    );
    // Supported + unsupported combined: still rejected.
    let combined = replace_once(&mutate_in(r#"<InputField name="ПолеВвода1" id="7">"#, "<ChoiceButton>true</ChoiceButton>", "<ChoiceButton>false</ChoiceButton>"), "<Left>79</Left>", "<Left>80</Left>");
    assert_unsupported(combined, "unsupported-property", &["ChoiceButton"]);
}

#[test]
fn sibling_reordering_and_caption_structure_are_rejected() {
    // Swap two adjacent sibling controls (document order is part of the projection).
    let xml = baseline("DonorForm");
    let (rest, label) = cut_block(&xml, "<LabelDecoration name=\"Надпись1\"", "</LabelDecoration>");
    let table_at = rest.find("        <Table name=\"ТабличноеПоле1\"").unwrap();
    let table_end = table_at + rest[table_at..].find("        </Table>\n").unwrap() + "        </Table>\n".len();
    let swapped = format!("{}{}{}", &rest[..table_end], label, &rest[table_end..]);
    assert_unsupported(swapped, "unsupported-panel", &["control#"]);
    assert_unsupported(
        mutate("<Item language=\"ru\">Надпись1</Item>", "<Item language=\"ru\">A</Item><Item language=\"ru\">B</Item>"),
        "invalid-xml",
        &["duplicate Caption language"],
    );
}

// ---------------------------------------------------------------- 4.x helpers

/// Edit the element block of control `name`: every `(from, to)` must match exactly once inside it.
fn edit_ctl(xml: &str, name: &str, subs: &[(&str, &str)]) -> String {
    let at = xml.find(&format!("name=\"{name}\" id=")).unwrap_or_else(|| panic!("control {name} not found"));
    let line = xml[..at].rfind('\n').unwrap() + 1;
    let indent = &xml[line..line + xml[line..].find('<').unwrap()];
    let end = at + xml[at..].find(&format!("\n{indent}</")).unwrap();
    let mut block = xml[at..end].to_string();
    for (from, to) in subs {
        assert_eq!(block.matches(from).count(), 1, "anchor {from:?} in control {name}");
        block = block.replacen(from, to, 1);
    }
    format!("{}{}{}", &xml[..at], block, &xml[end..])
}

fn patch(name: &str, edited: &str) -> Result<PatchedForm, ExtractionError> {
    patch_form(&stream(name), baseline(name).as_bytes(), edited.as_bytes())
}

/// Error `kind` (and full text) of a rejected patch; the stream is never produced.
fn patch_rejected(name: &str, edited: &str) -> (String, String) {
    let e = patch(name, edited).expect_err("must be rejected");
    (e.params["kind"].as_str().unwrap().to_string(), e.to_string())
}

/// The normal decoder + XML writer applied to a (patched) stream.
fn decode_xml(name: &str, raw: &[u8]) -> String {
    let base = baseline(name);
    let doc = roxmltree::Document::parse(&base).unwrap();
    let (id, fname) = (doc.root_element().attribute("id").unwrap(), doc.root_element().attribute("name").unwrap());
    let lookup = common::read_meta_lookup(&dir(name).join("Document.xml"));
    let model = decode_oof_stream(raw, id, Some(&lookup)).unwrap();
    assert_eq!(model["warnings"], serde_json::json!([]), "{name}");
    String::from_utf8(render_form_xml(&common::form_info(id, fname, raw), &model).unwrap()).unwrap().replace("\r\n", "\n")
}

/// Independent byte-range check: the patched stream is the original with exactly `replacements` spliced in.
fn assert_only_authorized_changes(original: &[u8], pf: &PatchedForm) {
    let mut expected = Vec::new();
    let mut at = 0;
    let mut last_end = 0;
    for r in &pf.replacements {
        assert!(r.range.start >= last_end, "replacements must be sorted and disjoint");
        expected.extend_from_slice(&original[at..r.range.start]);
        expected.extend_from_slice(&r.bytes);
        at = r.range.end;
        last_end = r.range.end;
    }
    expected.extend_from_slice(&original[at..]);
    assert!(expected == pf.bytes, "bytes differ outside the authorized ranges");
    assert_eq!(pf.authorized_ranges().len(), pf.replacements.len());
    pf.verify_preserved(original).unwrap();
}

/// Patch, then decode the patched stream with the normal decoder and require exactly the requested XML
/// semantics (every supported value equal and no other projection difference). Returns the patched form.
fn round_trip(name: &str, edited: &str) -> PatchedForm {
    let s = stream(name);
    let pf = patch_form(&s, baseline(name).as_bytes(), edited.as_bytes()).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    assert!(!pf.replacements.is_empty());
    let decoded = decode_xml(name, &pf.bytes);
    let requested = parse_form_xml(edited.as_bytes(), "requested").unwrap().in_decoder_order().with_derived_pages(&pf.derived_pages);
    let got = parse_form_xml(decoded.as_bytes(), "decoded").unwrap();
    assert_eq!(requested.controls.len(), got.controls.len());
    for c in &requested.controls {
        let d = got.control(c.id).unwrap();
        assert_eq!(c.values, d.values, "control {} after decode", c.describe());
    }
    assert!(diff_forms(&requested, &got).unwrap().is_empty(), "decoded XML differs from the requested XML");
    // Controls the user did not touch are untouched in the decoded XML as well.
    let base = parse_form_xml(baseline(name).as_bytes(), "baseline").unwrap();
    let edited_ids: Vec<i64> = plan_form_patch(&s, baseline(name).as_bytes(), edited.as_bytes()).unwrap().diff.edits.iter().map(|e| e.id).collect();
    for c in &base.controls {
        if !edited_ids.contains(&c.id) {
            assert_eq!(c.values, got.control(c.id).unwrap().values);
        }
    }
    pf
}

/// A copy of the stream where the source text of `range_of(index)` is replaced.
fn tweak_stream(name: &str, rewrite: impl Fn(&SpannedMetadata, &ControlIndex) -> Replacement) -> SpannedMetadata {
    let s = stream(name);
    let index = ControlIndex::build(&s).unwrap();
    let r = rewrite(&s, &index);
    parse_spanned(&s.apply(&[r]).unwrap()).unwrap()
}

fn rejected_on(s: &SpannedMetadata, edited: &str) -> (String, String) {
    let e = plan_form_patch(s, baseline("DonorForm").as_bytes(), edited.as_bytes())
        .and_then(|p| p.apply())
        .expect_err("must be rejected");
    (e.params["kind"].as_str().unwrap().to_string(), e.to_string())
}

fn text_of(s: &SpannedMetadata, span: oneform::brace::Span) -> String {
    String::from_utf8(s.slice(span).unwrap().to_vec()).unwrap()
}

fn replace_in(s: &SpannedMetadata, span: oneform::brace::Span, f: impl Fn(&str) -> String) -> Replacement {
    Replacement { range: span, bytes: f(&text_of(s, span)).into_bytes() }
}

// ---------------------------------------------------------------- 4.1 caption

const LABEL_CAPTION: &str = "<Item language=\"ru\">Надпись1</Item>";

#[test]
fn caption_text_change_replaces_only_the_text_atom() {
    let new = "<Item language=\"ru\">Он сказал &quot;привет&quot; &amp; &lt;ушёл&gt;</Item>";
    let xml = edit_ctl(&baseline("DonorForm"), "Надпись1", &[(LABEL_CAPTION, new)]);
    let pf = round_trip("DonorForm", &xml);
    assert_eq!(pf.replacements.len(), 1);
    let r = &pf.replacements[0];
    assert_eq!((r.control_id, r.properties.clone()), (5, vec![Property::Caption]));
    assert_eq!(String::from_utf8(r.bytes.clone()).unwrap(), "\"Он сказал \"\"привет\"\" & <ушёл>\"");
    // The replaced range is exactly the old quoted text atom.
    assert_eq!(text_of(&stream("DonorForm"), r.range), "\"Надпись1\"");
}

#[test]
fn caption_languages_can_be_added_removed_and_reordered() {
    for name in ["DonorForm", "DonorFormAlt"] {
        let base = baseline(name);
        // Add a language: node rewritten, existing item kept verbatim, CRLF style preserved.
        let two = format!("{LABEL_CAPTION}<Item language=\"en\">Label \"1\"</Item>");
        let added = edit_ctl(&base, "Надпись1", &[(LABEL_CAPTION, &two)]);
        let pf = round_trip(name, &added);
        assert_eq!(pf.replacements.len(), 1);
        assert_eq!(
            String::from_utf8(pf.replacements[0].bytes.clone()).unwrap(),
            "{1,2,\r\n{\"ru\",\"Надпись1\"},\r\n{\"en\",\"Label \"\"1\"\"\"}\r\n}"
        );
        // Replace the language set and reorder at once.
        let mixed = edit_ctl(&base, "Надпись1", &[(LABEL_CAPTION, "<Item language=\"en\">One</Item><Item language=\"ru\">Один</Item>")]);
        round_trip(name, &mixed);
        // Remove every caption: the localized node becomes `{1,0}`.
        let none = "<Caption>\n            <Item language=\"ru\">Надпись1</Item>\n          </Caption>\n          ";
        let cleared = edit_ctl(&base, "Надпись1", &[(none, "")]);
        let pf = round_trip(name, &cleared);
        assert_eq!(String::from_utf8(pf.replacements[0].bytes.clone()).unwrap(), "{1,0}");
        // Restoring the caption from the patched stream gives back the original bytes.
        let patched = parse_spanned(&pf.bytes).unwrap();
        let back = patch_form(&patched, decode_xml(name, &pf.bytes).as_bytes(), base.as_bytes()).unwrap();
        assert!(back.bytes == stream(name).source(), "{name}: removing then restoring a caption must be lossless");
    }
}

#[test]
fn caption_edits_cover_checkbox_radiobutton_and_fallback_button() {
    let base = baseline("DonorForm");
    let flag = "<Item language=\"ru\">Флажок1</Item>";
    let mut xml = edit_ctl(&base, "Флажок1", &[(flag, &format!("{flag}<Item language=\"en\">Flag</Item>"))]);
    xml = edit_ctl(&xml, "Переключатель1", &[("<Item language=\"ru\">Переключатель1</Item>", "<Item language=\"ru\">Радио</Item>")]);
    // Button has no known caption path; its single localized property is the caption the decoder reads.
    let button = ("<Item language=\"ru\">Кнопка1</Item>", "<Item language=\"ru\">Нажми</Item><Item language=\"ro\">Apasa</Item>");
    xml = edit_ctl(&xml, "Кнопка1", &[button]);
    let pf = round_trip("DonorForm", &xml);
    let ids: Vec<i64> = pf.replacements.iter().map(|r| r.control_id).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&4) && ids.contains(&8) && ids.contains(&9));
}

#[test]
fn caption_patch_requires_a_confident_mapping() {
    let base = baseline("DonorForm");
    // A control type without any caption node cannot receive a caption.
    let add = ("<Left>137</Left>", "<Caption><Item language=\"ru\">Подпись</Item></Caption><Left>137</Left>");
    let (kind, text) = patch_rejected("DonorForm", &edit_ctl(&base, "ПолеВвода1", &[add]));
    assert_eq!(kind, "caption-unmappable", "{text}");
    assert!(text.contains("ПолеВвода1") && text.contains("Caption"), "{text}");

    // Fallback type with two non-empty localized properties: ambiguous, nothing is guessed.
    let s = tweak_stream("DonorForm", |s, idx| {
        let info = idx.get(4).unwrap().info().unwrap();
        // Put a second non-empty localized string in front of the caption property.
        replace_in(s, info.span.span, |t| t.replacen("14,\r\n{1,1,", "{1,1,{\"ru\",\"Другое\"}},\r\n{1,1,", 1))
    });
    let to_x = (LABEL_CAPTION, "<Item language=\"ru\">X</Item>");
    let button = ("<Item language=\"ru\">Кнопка1</Item>", "<Item language=\"ru\">Нажми</Item>");
    let (kind, text) = rejected_on(&s, &edit_ctl(&base, "Кнопка1", &[button]));
    assert_eq!(kind, "caption-unmappable", "{text}");
    assert!(text.contains("could be the caption"), "{text}");

    // Known-path type whose caption node is not a plain localized string.
    let s = tweak_stream("DonorForm", |s, idx| {
        let info = idx.get(5).unwrap().info().unwrap();
        replace_in(s, info.span.span, |t| t.replacen("{1,1,\r\n{\"ru\",\"Надпись1\"}\r\n}", "{2}", 1))
    });
    let label_x = edit_ctl(&base, "Надпись1", &[to_x]);
    let (kind, text) = rejected_on(&s, &label_x);
    assert_eq!(kind, "caption-unmappable", "{text}");

    // The stream does not hold the caption the baseline XML claims: stale baseline.
    let s = tweak_stream("DonorForm", |s, idx| {
        let info = idx.get(5).unwrap().info().unwrap();
        replace_in(s, info.span.span, |t| t.replacen("Надпись1\"}", "Другая\"}", 1))
    });
    let (kind, text) = rejected_on(&s, &label_x);
    assert_eq!(kind, "baseline-mismatch", "{text}");

    // An empty language is not representable.
    let empty_lang = edit_ctl(&base, "Надпись1", &[(LABEL_CAPTION, "<Item language=\"\">X</Item>")]);
    assert_eq!(patch_rejected("DonorForm", &empty_lang).0, "invalid-caption");
}

// ---------------------------------------------------------------- 4.2 geometry

fn geom5<'a>(idx: &ControlIndex<'a>) -> NodeRef<'a> {
    idx.get(5).unwrap().geom().unwrap()
}

fn label_geometry(l: &str, t: &str, w: &str, h: &str) -> String {
    edit_ctl(
        &baseline("DonorForm"),
        "Надпись1",
        &[("<Left>79</Left>", l), ("<Top>42</Top>", t), ("<Width>53</Width>", w), ("<Height>17</Height>", h)],
    )
}

fn texts(pf: &PatchedForm) -> Vec<String> {
    pf.replacements.iter().map(|r| String::from_utf8(r.bytes.clone()).unwrap()).collect()
}

#[test]
fn each_geometry_component_updates_consistently() {
    let (l, t, w, h) = ("<Left>79</Left>", "<Top>42</Top>", "<Width>53</Width>", "<Height>17</Height>");
    for name in ["DonorForm", "DonorFormAlt"] {
        let with = |subs: &[(&str, &str)]| edit_ctl(&baseline(name), "Надпись1", subs);
        // Left moves the control: left and right (= left + width) change, top/bottom stay.
        let pf = round_trip(name, &with(&[(l, "<Left>100</Left>")]));
        assert_eq!(texts(&pf), ["100", "153"]);
        assert_eq!(pf.replacements[0].properties, [Property::Left]);
        assert_eq!(pf.replacements[1].properties, [Property::Left]);
        // Top moves it vertically.
        let pf = round_trip(name, &with(&[(t, "<Top>10</Top>")]));
        assert_eq!(texts(&pf), ["10", "27"]);
        // Width / Height change only right / bottom.
        let pf = round_trip(name, &with(&[(w, "<Width>60</Width>")]));
        assert_eq!(texts(&pf), ["139"]);
        assert_eq!(pf.replacements[0].properties, [Property::Width]);
        let pf = round_trip(name, &with(&[(h, "<Height>30</Height>")]));
        assert_eq!(texts(&pf), ["72"]);
        // Move and resize together; a zero-size control and a negative position are fine.
        let pf = round_trip(name, &with(&[(l, "<Left>-5</Left>"), (t, "<Top>0</Top>"), (w, "<Width>0</Width>"), (h, "<Height>200</Height>")]));
        assert_eq!(texts(&pf), ["-5", "0", "-5", "200"]);
    }
}

#[test]
fn geometry_on_other_control_types_and_nested_controls() {
    let base = baseline("DonorForm");
    // Table inside a page of the panel (page slot 1), a CommandBar, and a top-level Panel move (not resize).
    let mut xml = edit_ctl(&base, "ТабличноеПоле2", &[("<Left>6</Left>", "<Left>8</Left>"), ("<Height>90</Height>", "<Height>95</Height>")]);
    xml = edit_ctl(&xml, "КоманднаяПанель2", &[("<Width>193</Width>", "<Width>200</Width>")]);
    xml = edit_ctl(&xml, "Панель1", &[("<Left>420</Left>", "<Left>430</Left>"), ("<Top>223</Top>", "<Top>224</Top>")]);
    round_trip("DonorForm", &xml);
}

#[test]
fn invalid_geometry_is_rejected() {
    let (l, w, h) = ("<Left>79</Left>", "<Width>53</Width>", "<Height>17</Height>");
    let reject = |xml: String, kind: &str, needle: &str| {
        let (k, text) = patch_rejected("DonorForm", &xml);
        assert_eq!(k, kind, "{text}");
        assert!(text.contains(needle), "missing {needle:?} in {text}");
    };
    reject(label_geometry(l, "<Top>42</Top>", "<Width>-1</Width>", h), "invalid-geometry", "must not be negative");
    reject(label_geometry(l, "<Top>42</Top>", w, "<Height>-4</Height>"), "invalid-geometry", "Height must not be negative");
    reject(label_geometry("<Left>2147483648</Left>", "<Top>42</Top>", w, h), "invalid-geometry", "32-bit");
    reject(label_geometry("<Left>2147483640</Left>", "<Top>42</Top>", "<Width>100</Width>", h), "invalid-geometry", "32-bit");
    reject(label_geometry("<Left>9223372036854775807</Left>", "<Top>42</Top>", "<Width>1</Width>", h), "invalid-geometry", "overflows");
    reject(label_geometry(l, "<Top>-2147483649</Top>", w, h), "invalid-geometry", "32-bit");
    // Not a number at all is rejected while parsing the XML.
    reject(label_geometry("<Left>1.5</Left>", "<Top>42</Top>", w, h), "invalid-xml", "not an integer");
    // The edit is reported with the control and property.
    let (_, text) = patch_rejected("DonorForm", &label_geometry(l, "<Top>42</Top>", "<Width>-1</Width>", h));
    assert!(text.contains("Надпись1") && text.contains("id 5") && text.contains("Width"), "{text}");
}

#[test]
fn malformed_geometry_shapes_are_rejected() {
    let moved = label_geometry("<Left>90</Left>", "<Top>42</Top>", "<Width>53</Width>", "<Height>17</Height>");
    // A coordinate that is not a bare integer.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(1).unwrap().1, |_| "\"79\"".to_string()));
    let (kind, text) = rejected_on(&s, &moved);
    assert_eq!(kind, "unexpected-shape", "{text}");
    assert!(text.contains("left") && text.contains("Надпись1"), "{text}");
    // Wrong counted-list size: lists run past the end / the tail has the wrong length.
    for size in ["9", "x", "-1"] {
        let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(12).unwrap().1, |_| size.to_string()));
        assert_eq!(rejected_on(&s, &moved).0, "unexpected-shape", "list size {size}");
    }
    // One item too few after the incoming lists.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).span.span, |t| format!("{}}}", t.strip_suffix(",0}").unwrap())));
    let (kind, text) = rejected_on(&s, &moved);
    assert_eq!(kind, "unexpected-shape", "{text}");
    // One item too many.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).span.span, |t| format!("{},0}}", t.strip_suffix('}').unwrap())));
    assert_eq!(rejected_on(&s, &moved).0, "unexpected-shape");
    // A geometry node that is far too short.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).span.span, |_| "{8,79,42,132,59,1}".to_string()));
    assert_eq!(rejected_on(&s, &moved).0, "unexpected-shape");
    // Stream geometry that disagrees with the baseline XML (stale baseline).
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(1).unwrap().1, |_| "80".to_string()));
    let (kind, text) = rejected_on(&s, &moved);
    assert_eq!(kind, "baseline-mismatch", "{text}");
    assert!(text.contains("Left") && text.contains("79") && text.contains("80"), "{text}");
}

// ---------------------------------------------------------------- 4.3 visible

const HIDE: (&str, &str) = ("</ZOrder>", "</ZOrder><Visible>false</Visible>");

#[test]
fn visible_true_false_round_trip_for_representative_controls() {
    let controls = [
        "Надпись1",
        "Кнопка1",
        "ПолеВвода1",
        "Флажок1",
        "ТабличноеПоле1",
        "КоманднаяПанель2",
        "Панель2",
        "ПолеТабличногоДокумента1",
        "Разделитель1",
    ];
    for name in ["DonorForm", "DonorFormAlt"] {
        let s = stream(name);
        for control in controls {
            let hidden = edit_ctl(&baseline(name), control, &[HIDE]);
            let pf = round_trip(name, &hidden);
            // Exactly the one visibility atom `1` -> `0`; neighbours and unknown data are untouched.
            assert_eq!(pf.replacements.len(), 1, "{control}");
            let r = &pf.replacements[0];
            assert_eq!(r.properties, [Property::Visible]);
            assert_eq!((text_of(&s, r.range), String::from_utf8(r.bytes.clone()).unwrap()), ("1".to_string(), "0".to_string()), "{control}");
            // ... and back: dropping `<Visible>false</Visible>` restores the original stream byte for byte.
            let patched = parse_spanned(&pf.bytes).unwrap();
            let shown = patch_form(&patched, decode_xml(name, &pf.bytes).as_bytes(), baseline(name).as_bytes()).unwrap();
            assert!(shown.bytes == s.source(), "{name}/{control}: hide then show must restore the stream");
            assert_eq!(texts(&shown), ["1"]);
        }
    }
}

#[test]
fn visible_is_patched_independently_of_other_properties_and_controls() {
    let mut xml = edit_ctl(&baseline("DonorForm"), "Надпись1", &[HIDE]);
    xml = edit_ctl(&xml, "ПолеВвода1", &[HIDE]);
    xml = edit_ctl(&xml, "Флажок1", &[("<Left>420</Left>", "<Left>421</Left>")]);
    let pf = round_trip("DonorForm", &xml);
    let visible: Vec<i64> = pf.replacements.iter().filter(|r| r.properties == [Property::Visible]).map(|r| r.control_id).collect();
    assert_eq!(visible, [5, 7]);
    // Sorted by offset, disjoint.
    assert!(pf.replacements.windows(2).all(|w| w[0].range.end <= w[1].range.start));
}

#[test]
fn visible_with_unexpected_slot_values_is_handled_strictly() {
    let hidden = edit_ctl(&baseline("DonorForm"), "Надпись1", &[HIDE]);
    // Any non-zero integer means visible; hiding writes a clean 0.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(5).unwrap().1, |_| "2".to_string()));
    let pf = plan_form_patch(&s, baseline("DonorForm").as_bytes(), hidden.as_bytes()).unwrap().apply().unwrap();
    assert_eq!(texts(&pf), ["0"]);
    // A quoted slot is not a visibility flag.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(5).unwrap().1, |_| "\"1\"".to_string()));
    let (kind, text) = rejected_on(&s, &hidden);
    assert_eq!(kind, "unexpected-shape", "{text}");
    // The stream says hidden while the baseline XML says visible: stale baseline.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(5).unwrap().1, |_| "0".to_string()));
    assert_eq!(rejected_on(&s, &hidden).0, "baseline-mismatch");
}

// ---------------------------------------------------------------- 4.4 ZOrder / TabOrder

fn page_of(bytes: &[u8], id: i64) -> i64 {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    idx.get(id).unwrap().page
}

#[test]
fn zorder_and_taborder_round_trip_without_touching_the_page() {
    let (z, t) = ("<ZOrder>3</ZOrder>", "<TabOrder>3</TabOrder>");
    for name in ["DonorForm", "DonorFormAlt"] {
        let with = |subs: &[(&str, &str)]| edit_ctl(&baseline(name), "Надпись1", subs);
        let pf = round_trip(name, &with(&[(z, "<ZOrder>30</ZOrder>")]));
        assert_eq!((texts(&pf), pf.replacements[0].properties.clone()), (vec!["30".to_string()], vec![Property::ZOrder]));
        let pf = round_trip(name, &with(&[(t, "<TabOrder>33</TabOrder>")]));
        assert_eq!((texts(&pf), pf.replacements[0].properties.clone()), (vec!["33".to_string()], vec![Property::TabOrder]));
        let pf = round_trip(name, &with(&[(z, "<ZOrder>31</ZOrder>"), (t, "<TabOrder>34</TabOrder>")]));
        assert_eq!(texts(&pf), ["31", "34"]);
        // TabOrder 0 is projected as an absent element.
        let pf = round_trip(name, &with(&[(&format!("{t}\n          "), "")]));
        assert_eq!(texts(&pf), ["0"]);
        assert_eq!(page_of(&pf.bytes, 5), page_of(stream(name).source(), 5));
    }
}

#[test]
fn page_membership_is_preserved_for_controls_on_a_panel_page() {
    // ТабличноеПоле2 (id 34) lives on page slot 1 of Панель1; its page must stay 1.
    let base = baseline("DonorForm");
    let xml = edit_ctl(&base, "ТабличноеПоле2", &[("<ZOrder>0</ZOrder>", "<ZOrder>7</ZOrder>"), ("<TabOrder>2</TabOrder>", "<TabOrder>9</TabOrder>")]);
    let pf = round_trip("DonorForm", &xml);
    assert_eq!(texts(&pf), ["7", "9"]);
    assert_eq!(page_of(stream("DonorForm").source(), 34), 1);
    for id in [34, 35, 22, 5] {
        assert_eq!(page_of(&pf.bytes, id), page_of(stream("DonorForm").source(), id), "page of {id}");
    }
}

#[test]
fn swapping_zorder_between_siblings_is_consistent() {
    // Кнопка1 (z 2) and Надпись1 (z 3) exchange their order values.
    let base = baseline("DonorForm");
    let xml = edit_ctl(&base, "Кнопка1", &[("<ZOrder>2</ZOrder>", "<ZOrder>3</ZOrder>")]);
    let xml = edit_ctl(&xml, "Надпись1", &[("<ZOrder>3</ZOrder>", "<ZOrder>2</ZOrder>")]);
    let pf = round_trip("DonorForm", &xml);
    assert_eq!(texts(&pf), ["3", "2"]);
}

#[test]
fn inconsistent_or_invalid_ordering_is_rejected() {
    let base = baseline("DonorForm");
    let reject = |xml: String, kind: &str, needle: &str| {
        let (k, text) = patch_rejected("DonorForm", &xml);
        assert_eq!(k, kind, "{text}");
        assert!(text.contains(needle), "missing {needle:?} in {text}");
    };
    // Duplicate ZOrder among siblings (Кнопка1 already uses 2).
    reject(edit_ctl(&base, "Надпись1", &[("<ZOrder>3</ZOrder>", "<ZOrder>2</ZOrder>")]), "invalid-order", "collides");
    // Out-of-range values.
    reject(edit_ctl(&base, "Надпись1", &[("<ZOrder>3</ZOrder>", "<ZOrder>-1</ZOrder>")]), "invalid-order", "ZOrder must be between 0");
    reject(edit_ctl(&base, "Надпись1", &[("<ZOrder>3</ZOrder>", "<ZOrder>2147483648</ZOrder>")]), "invalid-order", "ZOrder");
    reject(edit_ctl(&base, "Надпись1", &[("<TabOrder>3</TabOrder>", "<TabOrder>-4</TabOrder>")]), "invalid-order", "TabOrder must be between 0");
    reject(edit_ctl(&base, "Надпись1", &[("<TabOrder>3</TabOrder>", "<TabOrder>2147483648</TabOrder>")]), "invalid-order", "TabOrder");
    reject(edit_ctl(&base, "Надпись1", &[("<TabOrder>3</TabOrder>", "<TabOrder>x</TabOrder>")]), "invalid-xml", "not an integer");
    // Errors name the control and property.
    let (_, text) = patch_rejected("DonorForm", &edit_ctl(&base, "Надпись1", &[("<ZOrder>3</ZOrder>", "<ZOrder>2</ZOrder>")]));
    assert!(text.contains("Надпись1") || text.contains("Кнопка1"), "{text}");
}

#[test]
fn unexpected_geometry_tails_are_rejected_for_ordering() {
    let moved = edit_ctl(&baseline("DonorForm"), "Надпись1", &[("<ZOrder>3</ZOrder>", "<ZOrder>30</ZOrder>")]);
    let len = |idx: &ControlIndex| geom5(idx).node.len();
    // A tail item is not an integer.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(len(idx) - 4).unwrap().1, |_| "\"3\"".to_string()));
    assert_eq!(rejected_on(&s, &moved).0, "unexpected-shape");
    // The first incoming list claims more items than exist.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(12).unwrap().1, |_| "4".to_string()));
    assert_eq!(rejected_on(&s, &moved).0, "unexpected-shape");
    // Stale ZOrder/TabOrder in the baseline.
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(len(idx) - 4).unwrap().1, |_| "8".to_string()));
    let (kind, text) = rejected_on(&s, &moved);
    assert_eq!(kind, "baseline-mismatch", "{text}");
    assert!(text.contains("ZOrder"), "{text}");
    let tab = edit_ctl(&baseline("DonorForm"), "Надпись1", &[("<TabOrder>3</TabOrder>", "<TabOrder>5</TabOrder>")]);
    let s = tweak_stream("DonorForm", |s, idx| replace_in(s, geom5(idx).atom(len(idx) - 3).unwrap().1, |_| "8".to_string()));
    assert_eq!(rejected_on(&s, &tab).0, "baseline-mismatch");
}

// ---------------------------------------------------------------- 4.5 combined and multi-control edits

/// Replace the first `from` inside the block of the control `name`/`id` (for blocks that repeat the tag, e.g. pages).
fn edit_first(xml: &str, name: &str, id: i64, from: &str, to: &str) -> String {
    let at = xml.find(&format!("name=\"{name}\" id=\"{id}\"")).unwrap_or_else(|| panic!("control {name} not found"));
    let line = xml[..at].rfind('\n').unwrap() + 1;
    let indent = &xml[line..line + xml[line..].find('<').unwrap()];
    let end = at + xml[at..].find(&format!("\n{indent}</")).unwrap();
    let rel = xml[at..end].find(from).unwrap_or_else(|| panic!("{from:?} not in control {name}"));
    format!("{}{}{}", &xml[..at + rel], to, &xml[at + rel + from.len()..])
}

#[test]
fn combined_properties_across_several_controls_round_trip() {
    for name in ["DonorForm", "DonorFormAlt"] {
        let base = baseline(name);
        let s = stream(name);
        // Label: caption (+language), geometry, hidden and tab-order together.
        let label = [
            (LABEL_CAPTION, "<Item language=\"ru\">Метка</Item><Item language=\"en\">Label</Item>"),
            ("<Left>79</Left>", "<Left>90</Left>"),
            ("<Width>53</Width>", "<Width>70</Width>"),
            ("<ZOrder>3</ZOrder>", "<ZOrder>3</ZOrder><Visible>false</Visible>"),
            ("<TabOrder>3</TabOrder>", "<TabOrder>33</TabOrder>"),
        ];
        let mut xml = edit_ctl(&base, "Надпись1", &label);
        // Button (fallback caption) + top; CheckBox caption language + height; Radio geometry + tab.
        xml = edit_ctl(&xml, "Кнопка1", &[("<Item language=\"ru\">Кнопка1</Item>", "<Item language=\"ru\">Старт \"1\"</Item>"), ("<Top>42</Top>", "<Top>44</Top>")]);
        xml = edit_ctl(&xml, "Флажок1", &[("<Item language=\"ru\">Флажок1</Item>", "<Item language=\"ru\">Флажок1</Item><Item language=\"ro\">Bifa</Item>"), ("<Height>19</Height>", "<Height>25</Height>")]);
        xml = edit_ctl(&xml, "Переключатель1", &[("<Left>222</Left>", "<Left>225</Left>"), ("<Width>193</Width>", "<Width>190</Width>"), ("<TabOrder>10</TabOrder>", "<TabOrder>12</TabOrder>")]);
        // Page-bound table + a field that only changes visibility and tab order.
        xml = edit_ctl(&xml, "ТабличноеПоле2", &[("<Top>38</Top>", "<Top>40</Top>"), ("<TabOrder>2</TabOrder>", "<TabOrder>8</TabOrder>")]);
        xml = edit_ctl(&xml, "ПолеВвода7", &[("</ZOrder>", "</ZOrder><Visible>false</Visible>"), ("<TabOrder>26</TabOrder>", "<TabOrder>2</TabOrder>")]);
        let pf = round_trip(name, &xml);
        let mut ids: Vec<i64> = pf.replacements.iter().map(|r| r.control_id).collect();
        ids.dedup();
        ids.sort_unstable();
        assert_eq!(ids, [4, 5, 8, 9, 33, 34], "{name}");
        // Unrelated controls' source text is byte-identical: only the six controls' own ranges differ.
        let outside: usize = pf.replacements.iter().map(|r| r.range.end - r.range.start).sum();
        assert!(outside < s.source().len() / 100, "{name}: replaced {outside} bytes");
        // Reverting every edit from the patched stream restores the original bytes exactly.
        let patched = parse_spanned(&pf.bytes).unwrap();
        let back = patch_form(&patched, decode_xml(name, &pf.bytes).as_bytes(), base.as_bytes()).unwrap();
        assert!(back.bytes == s.source(), "{name}: reverting the edits must restore the stream");
    }
}

#[test]
fn every_control_can_be_moved_hidden_and_reordered_at_once() {
    for name in ["DonorForm", "DonorFormAlt"] {
        let base = baseline(name);
        let parsed = parse_form_xml(base.as_bytes(), "baseline").unwrap();
        let mut xml = base.clone();
        for c in &parsed.controls {
            let v = &c.values;
            xml = edit_first(&xml, &c.name, c.id, &format!("<Left>{}</Left>", v.left), &format!("<Left>{}</Left>", v.left + 1));
            xml = edit_first(&xml, &c.name, c.id, &format!("<Top>{}</Top>", v.top), &format!("<Top>{}</Top>", v.top + 2));
            xml = edit_first(&xml, &c.name, c.id, &format!("<ZOrder>{}</ZOrder>", v.z_order), &format!("<ZOrder>{}</ZOrder><Visible>false</Visible>", v.z_order + 100));
        }
        let pf = round_trip(name, &xml);
        assert_eq!(pf.replacements.iter().map(|r| r.control_id).collect::<std::collections::BTreeSet<_>>().len(), parsed.controls.len(), "{name}");
        // Every edit is attributed to supported properties only.
        for r in &pf.replacements {
            assert!(!r.properties.is_empty() && !r.properties.contains(&Property::Caption), "{name}");
        }
    }
}

#[test]
fn empty_plan_returns_the_exact_original_bytes() {
    for name in ["DonorForm", "DonorFormAlt"] {
        let s = stream(name);
        let base = baseline(name);
        let pf = patch_form(&s, base.as_bytes(), base.as_bytes()).unwrap();
        assert!(pf.bytes == s.source() && pf.replacements.is_empty() && pf.authorized_ranges().is_empty(), "{name}");
        pf.verify_preserved(s.source()).unwrap();
        // Formatting-only changes (CRLF, comment, attribute order) are no-ops too.
        let crlf = base.replacen("?>\n", "?>\n<!-- c -->\n", 1).replace('\n', "\r\n");
        let noisy = format!("\u{feff}{}", crlf.replace(r#"<InputField name="ПолеВвода1" id="7">"#, r#"<InputField id="7" name="ПолеВвода1">"#));
        let pf = patch_form(&s, base.as_bytes(), noisy.as_bytes()).unwrap();
        assert!(pf.bytes == s.source() && pf.replacements.is_empty(), "{name}");
        let plan = plan_form_patch(&s, base.as_bytes(), base.as_bytes()).unwrap();
        assert!(plan.apply().unwrap().bytes == s.source());
    }
}

#[test]
fn unsupported_or_partially_applicable_edits_produce_no_stream() {
    // A supported edit next to an unsupported one: nothing is produced.
    let base = baseline("DonorForm");
    let xml = edit_ctl(&base, "Надпись1", &[("<Left>79</Left>", "<Left>80</Left>")]);
    let xml = xml.replace("<ChoiceButton>true</ChoiceButton>", "<ChoiceButton>false</ChoiceButton>");
    let e = patch("DonorForm", &xml).unwrap_err();
    assert_eq!(e.code, GENERATED_FILE_MODIFIED);
    // A good edit on one control and an impossible one on another: no partial result.
    let xml = edit_ctl(&base, "Надпись1", &[("<Left>79</Left>", "<Left>80</Left>")]);
    let xml = edit_ctl(&xml, "Надпись1", &[("<Width>53</Width>", "<Width>-3</Width>")]);
    assert_eq!(patch_rejected("DonorForm", &xml).0, "invalid-geometry");
    let xml = edit_ctl(&base, "Надпись1", &[("<Left>79</Left>", "<Left>80</Left>")]);
    let xml = edit_ctl(&xml, "ПолеВвода1", &[("<Left>137</Left>", "<Caption><Item language=\"ru\">X</Item></Caption><Left>137</Left>")]);
    assert_eq!(patch_rejected("DonorForm", &xml).0, "caption-unmappable");
}

// ---------------------------------------------------------------- 4.6 panel resize

const PANEL1: i64 = 13; // 278x154 at (420,223), 2 pages, ShowTabs code 2; page 1 hidden
const PANEL2: i64 = 36; // 185x104 at (728,441), 1 page, ShowTabs code 3

fn panel_name(id: i64) -> &'static str {
    if id == PANEL1 {
        "Панель1"
    } else {
        "Панель2"
    }
}

fn panel_old(id: i64) -> [i64; 4] {
    if id == PANEL1 {
        [420, 223, 278, 154]
    } else {
        [728, 441, 185, 104]
    }
}

type Geom = [Option<i64>; 4];

/// `Панель1`/`Панель2` with new left/top/width/height (`None` keeps the baseline value).
fn resized_panel(base: &str, id: i64, geom: Geom) -> String {
    let (old, tags) = (panel_old(id), ["Left", "Top", "Width", "Height"]);
    let mut subs = Vec::new();
    for i in 0..4 {
        if let Some(v) = geom[i] {
            subs.push((format!("<{0}>{1}</{0}>", tags[i], old[i]), format!("<{0}>{1}</{0}>", tags[i], v)));
        }
    }
    let refs: Vec<(&str, &str)> = subs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    edit_ctl(base, panel_name(id), &refs)
}

fn find_control(controls: &serde_json::Value, id: i64) -> Option<serde_json::Value> {
    for c in controls.as_array()? {
        if c["id"].as_i64() == Some(id) {
            return Some(c.clone());
        }
        if let Some(found) = find_control(&c["children"], id) {
            return Some(found);
        }
    }
    None
}

/// The normal decoder's model of a panel (incl. `pages`) in a (patched) stream.
fn decoded_panel(name: &str, raw: &[u8], id: i64) -> serde_json::Value {
    let base = baseline(name);
    let doc = roxmltree::Document::parse(&base).unwrap();
    let lookup = common::read_meta_lookup(&dir(name).join("Document.xml"));
    let model = decode_oof_stream(raw, doc.root_element().attribute("id").unwrap(), Some(&lookup)).unwrap();
    find_control(&model["controls"], id).expect("panel in decoded model")
}

fn page_geometry(page: &serde_json::Value) -> [i64; 4] {
    ["left", "top", "width", "height"].map(|k| page[k].as_i64().unwrap())
}

/// Independent oracle: every page keeps Left/Top, title, visibility and enabled state; Width/Height move by the
/// panel's size change; the panel itself has the requested size and nothing else about it changed.
fn assert_panel_resized(name: &str, original: &[u8], patched: &[u8], id: i64, size: (i64, i64)) {
    let (before, after) = (decoded_panel(name, original, id), decoded_panel(name, patched, id));
    let (dw, dh) = (size.0 - before["width"].as_i64().unwrap(), size.1 - before["height"].as_i64().unwrap());
    assert_eq!((after["width"].as_i64(), after["height"].as_i64()), (Some(size.0), Some(size.1)));
    assert_eq!(before["showTabs"], after["showTabs"]);
    let (pb, pa) = (before["pages"].as_array().unwrap(), after["pages"].as_array().unwrap());
    assert_eq!(pb.len(), pa.len());
    assert!(!pb.is_empty());
    for (b, a) in pb.iter().zip(pa) {
        for key in ["name", "title", "visible", "enabled", "left", "top"] {
            assert_eq!(b[key], a[key], "{name} panel {id} page {} {key}", b["name"]);
        }
        assert_eq!(a["width"].as_i64().unwrap(), b["width"].as_i64().unwrap() + dw, "{name} page width");
        assert_eq!(a["height"].as_i64().unwrap(), b["height"].as_i64().unwrap() + dh, "{name} page height");
    }
    for key in ["id", "name", "type", "caption", "visible", "page", "ordinal", "tabOrder", "events", "props", "children"] {
        assert_eq!(before[key], after[key], "{name} panel {id} {key}");
    }
}

fn panel_rec<'a>(idx: &ControlIndex<'a>, id: i64) -> NodeRef<'a> {
    idx.get(id).unwrap().info().unwrap().child(1).unwrap()
}

/// Index of the panel record item after the six counted lists (`oof::panel_pages` `base`).
fn panel_base(p: NodeRef) -> usize {
    let mut idx = 2;
    for _ in 0..6 {
        idx += 1 + p.atom(idx).unwrap().0.value.parse::<usize>().unwrap();
    }
    idx
}

/// A copy of the stream where `f(stream, panel record, base)` edits the panel record of `id`.
fn tweak_panel(name: &str, id: i64, f: impl Fn(&SpannedMetadata, NodeRef, usize) -> Vec<Replacement>) -> SpannedMetadata {
    let s = stream(name);
    let index = ControlIndex::build(&s).unwrap();
    let p = panel_rec(&index, id);
    let reps = f(&s, p, panel_base(p));
    parse_spanned(&s.apply(&reps).unwrap()).unwrap()
}

fn set(span: oneform::brace::Span, text: &str) -> Replacement {
    Replacement { range: span, bytes: text.as_bytes().to_vec() }
}

fn insert_at(at: usize, text: &str) -> Replacement {
    set(oneform::brace::Span { start: at, end: at }, text)
}

/// Atom `item` of boundary row `row`.
fn row_atom(p: NodeRef, base: usize, row: usize, item: usize) -> oneform::brace::Span {
    p.child(base + 8 + row).unwrap().atom(item).unwrap().1
}

fn apply_to(s: &SpannedMetadata, edited: &str) -> Result<PatchedForm, ExtractionError> {
    plan_form_patch(s, baseline("DonorForm").as_bytes(), edited.as_bytes()).and_then(|p| p.apply())
}

#[test]
fn panel_resize_patches_derived_page_boundaries_in_both_fixtures() {
    let cases: [(i64, Geom); 14] = [
        (PANEL1, [None, None, Some(300), Some(170)]),
        (PANEL1, [None, None, Some(200), None]),
        (PANEL1, [None, None, None, Some(300)]),
        (PANEL1, [None, None, Some(3), Some(21)]), // smallest: both page areas shrink to 1x1
        (PANEL1, [None, None, Some(1000), Some(900)]),
        (PANEL1, [Some(430), Some(224), Some(300), Some(170)]), // move + resize
        (PANEL1, [Some(-5), None, None, None]),                 // move only: no page change
        (PANEL1, [None, None, Some(278), Some(154)]),           // same size: no edit at all
        (PANEL2, [None, None, Some(400), Some(300)]),
        (PANEL2, [None, None, Some(39), Some(3)]),
        (PANEL2, [None, None, Some(185), Some(500)]),
        (PANEL2, [None, None, Some(60), None]),
        (PANEL2, [Some(1), Some(2), Some(186), Some(105)]),
        (PANEL2, [None, None, Some(2000), Some(2000)]),
    ];
    for name in ["DonorForm", "DonorFormAlt"] {
        let base = baseline(name);
        let s = stream(name);
        for (id, geom) in cases {
            let edited = resized_panel(&base, id, geom);
            let old = panel_old(id);
            let size = (geom[2].unwrap_or(old[2]), geom[3].unwrap_or(old[3]));
            if edited == base {
                assert!(patch_form(&s, base.as_bytes(), edited.as_bytes()).unwrap().replacements.is_empty());
                continue;
            }
            let pf = round_trip(name, &edited);
            assert_panel_resized(name, s.source(), &pf.bytes, id, size);
            let resized = size != (old[2], old[3]);
            assert_eq!(pf.derived_pages.is_empty(), !resized, "{name} {id} {geom:?}");
            if resized {
                let decoded = decoded_panel(name, &pf.bytes, id);
                let pages = decoded["pages"].as_array().unwrap();
                assert_eq!(pf.derived_pages.len(), pages.len());
                for (d, p) in pf.derived_pages.iter().zip(pages) {
                    assert_eq!((d.panel_id, d.name.as_str()), (id, p["name"].as_str().unwrap()));
                    assert_eq!([d.left, d.top, d.width, d.height], page_geometry(p));
                }
            }
        }
    }
}

#[test]
fn panel_resize_writes_exactly_the_boundary_coordinates_and_the_panel_slots() {
    let name = "DonorForm";
    let base = baseline(name);
    // 278x154 -> 300x170: edge 3 (270) += 22 and edge 4 (128) += 16 for each of the two pages, panel right/bottom.
    let pf = round_trip(name, &resized_panel(&base, PANEL1, [None, None, Some(300), Some(170)]));
    assert_eq!(texts(&pf), ["292", "144", "292", "144", "720", "393"]);
    let props: Vec<Vec<Property>> = pf.replacements.iter().map(|r| r.properties.clone()).collect();
    let (w, h) = (vec![Property::Width], vec![Property::Height]);
    assert_eq!(props, [w.clone(), h.clone(), w.clone(), h.clone(), w, h]);
    assert!(pf.replacements.iter().all(|r| r.control_id == PANEL1));
    // Decoded: 276x134 pages at (1,19) become 298x150 at (1,19).
    let decoded = decoded_panel(name, &pf.bytes, PANEL1);
    for p in decoded["pages"].as_array().unwrap() {
        assert_eq!(page_geometry(p), [1, 19, 298, 150]);
    }
    // Only the height: only the edge-4 rows and the bottom move.
    let pf = round_trip(name, &resized_panel(&base, PANEL1, [None, None, None, Some(200)]));
    assert_eq!(texts(&pf), ["174", "174", "423"]);
    // Panel 2 (tabs at the left): page 147x102 at (37,1) -> 162x108.
    let pf = round_trip(name, &resized_panel(&base, PANEL2, [None, None, Some(200), Some(110)]));
    assert_eq!(texts(&pf), ["156", "102", "928", "551"]);
    assert_eq!(page_geometry(&decoded_panel(name, &pf.bytes, PANEL2)["pages"][0]), [37, 1, 162, 108]);
    // A move does not touch any page record.
    let pf = round_trip(name, &resized_panel(&base, PANEL1, [Some(430), Some(224), None, None]));
    assert_eq!(texts(&pf), ["430", "224", "708", "378"]);
}

#[test]
fn derived_page_values_are_not_user_editable_but_are_accounted_for_in_comparison() {
    let name = "DonorForm";
    let (s, base) = (stream(name), baseline(name));
    let edited = resized_panel(&base, PANEL1, [None, None, Some(300), Some(170)]);
    let pf = patch_form(&s, base.as_bytes(), edited.as_bytes()).unwrap();
    let requested = parse_form_xml(edited.as_bytes(), "requested").unwrap();
    let got = parse_form_xml(decode_xml(name, &pf.bytes).as_bytes(), "decoded").unwrap();
    // The stale page geometry of the requested XML is the only thing that differs from the decoded form ...
    let e = diff_forms(&requested, &got).unwrap_err();
    assert_eq!(e.params["kind"], "unsupported-panel", "{e}");
    assert!(e.params["problems"].as_array().unwrap().iter().all(|p| p.as_str().unwrap().contains("Page[name=")), "{e}");
    // ... and accounting for exactly the derived pages makes them equal.
    assert!(diff_forms(&requested.with_derived_pages(&pf.derived_pages), &got).unwrap().is_empty());
    // A wrong derived value is not accepted.
    let mut wrong = pf.derived_pages.clone();
    wrong[0].width += 1;
    assert!(diff_forms(&requested.with_derived_pages(&wrong), &got).is_err());
    // Editing a page's geometry together with the panel stays rejected.
    let both = edited.replace("<Width>276</Width>", "<Width>298</Width>");
    assert_eq!(patch_rejected(name, &both).0, "unsupported-panel");
}

#[test]
fn panel_resize_works_for_every_tab_placement_and_preserves_page_flags() {
    let name = "DonorForm";
    let base = baseline(name);
    for (id, size) in [(PANEL1, (333, 222)), (PANEL2, (400, 300))] {
        let edited = resized_panel(&base, id, [None, None, Some(size.0), Some(size.1)]);
        for show_tabs in ["0", "1", "2", "3", "4", "7"] {
            let s = tweak_panel(name, id, |_, p, base| vec![set(p.atom(base + 1).unwrap().1, show_tabs)]);
            let pf = apply_to(&s, &edited).unwrap();
            assert_only_authorized_changes(s.source(), &pf);
            assert_panel_resized(name, s.source(), &pf.bytes, id, size);
            let after = decoded_panel(name, &pf.bytes, id);
            assert_eq!(after["showTabs"], show_tabs.parse::<i64>().unwrap());
            if id == PANEL2 {
                // dx = 38 > dy = 2: the position is the decoder's function of the placement; it must not move.
                let expect = if show_tabs == "0" { [19, 1] } else { [37, 1] };
                assert_eq!(page_geometry(&after["pages"][0])[..2], expect, "showTabs {show_tabs}");
                assert_eq!(page_geometry(&after["pages"][0])[2..], [362, 298]);
            }
        }
    }
    // Pages hidden / disabled in the stream keep their flags, titles and names.
    let edited = resized_panel(&base, PANEL1, [None, None, Some(333), Some(222)]);
    let s = tweak_panel(name, PANEL1, |_, p, base| {
        let table = p.child(base + 3).unwrap();
        vec![set(table.child(2).unwrap().atom(4).unwrap().1, "0"), set(table.child(3).unwrap().atom(5).unwrap().1, "0")]
    });
    let before = decoded_panel(name, s.source(), PANEL1);
    let flags: Vec<(bool, bool)> =
        before["pages"].as_array().unwrap().iter().map(|p| (p["visible"] == true, p["enabled"] == true)).collect();
    assert_eq!(flags, [(false, true), (true, false)]);
    let pf = apply_to(&s, &edited).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    assert_panel_resized(name, s.source(), &pf.bytes, PANEL1, (333, 222));
    // Visibility of the panel itself is independent of the resize.
    let hidden = edit_ctl(&edited, "Панель1", &[("<TabOrder>15</TabOrder>", "<TabOrder>15</TabOrder><Visible>false</Visible>")]);
    let pf = round_trip(name, &hidden);
    assert_eq!(decoded_panel(name, &pf.bytes, PANEL1)["visible"], false);
}

#[test]
fn panel_with_three_pages_and_pages_without_boundary_records() {
    let name = "DonorForm";
    let base = baseline(name);
    let edited = resized_panel(&base, PANEL1, [None, None, Some(250), Some(260)]);
    // A third page: a copy of page 2's record plus a copy of its four boundary rows for page index 2.
    let s = tweak_panel(name, PANEL1, |s, p, base| {
        let table = p.child(base + 3).unwrap();
        let rec1 = table.child(3).unwrap().span.span;
        let copy = text_of(s, rec1).replace("Страница2", "Страница3");
        let mut rows = String::new();
        for k in 4..8 {
            let mut parts: Vec<String> = text_of(s, p.child(base + 8 + k).unwrap().span.span).split(',').map(String::from).collect();
            parts[5] = "2".to_string();
            rows.push_str(&format!(",\n{}", parts.join(",")));
        }
        vec![
            set(table.atom(1).unwrap().1, "3"),
            insert_at(rec1.end, &format!(",\n{copy}")),
            set(p.atom(base + 7).unwrap().1, "12"),
            insert_at(p.child(base + 15).unwrap().span.span.end, &rows),
        ]
    });
    let before = decoded_panel(name, s.source(), PANEL1);
    let names: Vec<&str> = before["pages"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Страница1", "Страница2", "Страница3"]);
    let pf = apply_to(&s, &edited).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    assert_panel_resized(name, s.source(), &pf.bytes, PANEL1, (250, 260));
    assert_eq!(pf.derived_pages.len(), 3);
    assert_eq!(texts(&pf).len(), 3 * 2 + 2); // 3 pages x (edge 3, edge 4), panel right and bottom
    assert_eq!(page_geometry(&decoded_panel(name, &pf.bytes, PANEL1)["pages"][2]), [1, 19, 248, 240]);

    // A page without an edge-3 row is derived from the panel size alone and needs no record patch
    // (showTabs != 0: left 1, top 19, width - 2, height - 20).
    let s = tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 6, 4), "9")]);
    let pf = apply_to(&s, &edited).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    let page = decoded_panel(name, &pf.bytes, PANEL1)["pages"][1].clone();
    assert_eq!(page_geometry(&page), [1, 19, 248, 240]);
    assert_eq!(texts(&pf).len(), 4); // page 0's edges and the panel's right/bottom only
    assert_panel_resized(name, s.source(), &pf.bytes, PANEL1, (250, 260));
}

#[test]
fn panel_resize_rejects_impossible_sizes_with_the_minimum() {
    let base = baseline("DonorForm");
    for (id, geom, minimum) in [
        (PANEL1, [None, None, Some(2), None], "minimum is 3"),
        (PANEL1, [None, None, None, Some(20)], "minimum is 21"),
        (PANEL1, [None, None, Some(0), Some(0)], "minimum is"),
        (PANEL2, [None, None, Some(38), None], "minimum is 39"),
        (PANEL2, [None, None, None, Some(2)], "minimum is 3"),
    ] {
        let (kind, text) = patch_rejected("DonorForm", &resized_panel(&base, id, geom));
        assert_eq!(kind, "invalid-geometry", "{text}");
        assert!(text.contains(panel_name(id)) && text.contains("no client area") && text.contains(minimum), "{text}");
    }
    let (kind, text) = patch_rejected("DonorForm", &resized_panel(&base, PANEL1, [None, None, Some(-1), None]));
    assert_eq!(kind, "invalid-geometry", "{text}");
}

#[test]
fn panel_resize_rejects_unknown_shapes_atomically() {
    let name = "DonorForm";
    let base = baseline(name);
    // A good label edit next to the resize: the whole patch is refused, nothing is produced.
    let edited = edit_ctl(&resized_panel(&base, PANEL1, [None, None, Some(300), Some(170)]), "Надпись1", &[("<Left>79</Left>", "<Left>80</Left>")]);
    let shape = |s: &SpannedMetadata, needle: &str| {
        let (kind, text) = rejected_on(s, &edited);
        assert_eq!(kind, "unexpected-shape", "{text}");
        assert!(text.contains("Панель1") && text.contains(needle), "{needle:?} missing in {text}");
        // The same stream still accepts edits that do not resize the panel.
        assert!(apply_to(s, &resized_panel(&base, PANEL1, [Some(430), None, None, None])).is_ok());
    };
    // The page area is not an unclamped plain value.
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 2, 1), "9999")]), "clamped");
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 2, 1), "-100")]), "clamped");
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 3, 1), "1000")]), "clamped");
    // Two edge-3 rows for one page.
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 0, 4), "3")]), "ambiguous");
    // Non-integer slots.
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 3, 1), "\"128\"")]), "bare integer");
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(row_atom(p, base, 3, 7), "x")]), "bare integer");
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(p.atom(base + 1).unwrap().1, "\"2\"")]), "bare integer");
    // Counts that do not match the stream.
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(p.child(base + 3).unwrap().atom(1).unwrap().1, "3")]), "page");
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(p.atom(base + 7).unwrap().1, "9")]), "boundary row");
    shape(&tweak_panel(name, PANEL1, |_, p, base| vec![set(p.atom(base + 7).unwrap().1, "99")]), "out of range");
    shape(&tweak_panel(name, PANEL1, |_, p, _| vec![set(p.atom(2).unwrap().1, "99")]), "out of range");
    shape(&tweak_panel(name, PANEL1, |_, p, _| vec![set(p.atom(3).unwrap().1, "x")]), "bare integer");
    // Both panels resized, only the second one has an unusable shape: still nothing is produced.
    let s = tweak_panel(name, PANEL2, |_, p, base| vec![set(row_atom(p, base, 2, 1), "9999")]);
    let both = resized_panel(&edited, PANEL2, [None, None, Some(300), None]);
    let (kind, text) = rejected_on(&s, &both);
    assert_eq!(kind, "unexpected-shape", "{text}");
    assert!(text.contains("Панель2"), "{text}");
}
