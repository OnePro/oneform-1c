//! Tasks 3.1-3.3 and 4.1-4.6: semantic `Form.xml` models, control identity index, the conservative diff and the
//! lossless property patchers (`oneform::form_patch`). Uses the real ordinary-form fixtures; patched streams are
//! decoded by the normal OOF decoder and compared with the requested XML.

mod common;

use std::path::{Path, PathBuf};

use oneform::brace::{parse_spanned, Replacement, SpannedMetadata};
use oneform::errors::{ExtractionError, GENERATED_FILE_MODIFIED, METADATA_LAYOUT_UNSUPPORTED};
use oneform::form_patch::{
    anchor_graph_violations, diff_forms, enrich_ext_properties, parse_form_xml, patch_form, plan_form_patch, ControlIndex, FormPatchPlan, FormXml, NodeRef, PatchedForm, Property,
};
use oneform::oof::{decode_oof_stream, render_form_xml};

fn dir(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let forms = root.join("forms").join(name);
    if forms.is_dir() { forms } else { root.join("patch").join(name) }
}

fn stream(name: &str) -> SpannedMetadata {
    parse_spanned(&std::fs::read(dir(name).join("FormStream.bin")).unwrap()).unwrap()
}

/// `xml` without the `steps="..."` attributes.
fn strip_steps(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(at) = rest.find(" steps=\"") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + " steps=\"".len()..];
        rest = &tail[tail.find('"').unwrap() + 1..];
    }
    out.push_str(rest);
    out
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
    // Deleted and added controls of donor-supported types are now structural edits, not errors.
    let (deleted, _) = cut_block(&baseline("DonorForm"), "<LabelDecoration name=\"Надпись1\"", "</LabelDecoration>");
    let s = stream("DonorForm");
    let p = plan(&s, &deleted).unwrap();
    assert_eq!(p.diff.deletions.len(), 1);
    assert_eq!(p.diff.deletions[0].name, "Надпись1");
    let new_ctl = |id: u32| {
        format!(
            "<LabelDecoration name=\"Новая\" id=\"{id}\"><Left>0</Left><Top>0</Top><Width>1</Width><Height>1</Height><ZOrder>0</ZOrder></LabelDecoration>\n        <Table name=\"ТабличноеПоле1\""
        )
    };
    let p = plan(&s, &mutate("<Table name=\"ТабличноеПоле1\"", &new_ctl(999))).unwrap();
    assert_eq!(p.diff.additions.len(), 1);
    assert_eq!(p.diff.additions[0].name, "Новая");
    // Duplicate id is still an error.
    let (kind, text) = rejected(&mutate("<Table name=\"ТабличноеПоле1\"", &new_ctl(5)));
    assert_eq!(kind, "duplicate-control");
    assert!(text.contains("id 5"), "{text}");
    // A type without a donor prototype is rejected.
    let unknown = "<UnknownControl name=\"Чужой\" id=\"999\" guid=\"aaaaaaaa-0000-0000-0000-000000000001\"><Left>0</Left><Top>0</Top><Width>1</Width><Height>1</Height><ZOrder>0</ZOrder></UnknownControl>\n        <Table name=\"ТабличноеПоле1\"";
    let (kind, text) = rejected(&mutate("<Table name=\"ТабличноеПоле1\"", unknown));
    assert_eq!(kind, "added-control");
    assert!(text.contains("Чужой"), "{text}");

    // Renaming a control by id is now a supported edit; empty or duplicate names are rejected.
    let p = plan(&s, &mutate(r#"name="Надпись1" id="5""#, r#"name="Другое" id="5""#)).unwrap();
    assert_eq!(p.diff.edits.len(), 1);
    assert_eq!(p.diff.edits[0].changed, vec![Property::Name]);
    let (kind, text) = rejected(&mutate(r#"name="Надпись1" id="5""#, r#"name="" id="5""#));
    assert_eq!(kind, "renamed-control");
    assert!(text.contains("id 5"), "{text}");
    let (kind, text) = rejected(&mutate(r#"name="Надпись1" id="5""#, r#"name="Кнопка1" id="5""#));
    assert_eq!(kind, "renamed-control");
    assert!(text.contains("Кнопка1"), "{text}");
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
    xml = replace_once(&xml, "<TabOrder>4</TabOrder>\n          <Anchors bottom=\"Top\" bottomTarget=\"7\"", "<TabOrder>40</TabOrder>\n          <Anchors bottom=\"Top\" bottomTarget=\"7\"");
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
    assert_unsupported(mutate("<Width>937</Width>", "<Width>-1</Width>"), "unsupported-form-value", &["form 'Форма'", "Width"]);
    assert_unsupported(
        mutate("<Item language=\"ru\">Обработка  ВнешняяОбработка1</Item>", "<Item>X</Item>"),
        "unsupported-form-value",
        &["Caption"],
    );
    assert_unsupported(mutate("<MainAttribute attributeId=\"1\"/>", "<MainAttribute attributeId=\"2\"/>"), "unsupported-form-value", &["MainAttribute"]);
    assert_unsupported(mutate("name=\"ОбработкаОбъект\"", "name=\"Другое\""), "unsupported-form-value", &["Attributes"]);
    assert_unsupported(mutate("ordinaryFormVersion=\"2.2\"", "ordinaryFormVersion=\"2.3\""), "unsupported-form-attribute", &["ordinaryFormVersion"]);
    assert_unsupported(mutate("reconstructionComplete=\"false\"", "reconstructionComplete=\"true\""), "unsupported-form-attribute", &["reconstructionComplete"]);
}

#[test]
fn event_differences_are_rejected() {
    assert_unsupported(mutate("<BeforeOpen id=\"70000\">", "<BeforeOpen id=\"70098\">"), "unsupported-event", &["form 'Форма'", "Events"]);
    assert_unsupported(mutate("<OnOpen id=\"70001\">", "<OnOpen id=\"70099\">"), "unsupported-event", &["Events"]);
    assert_unsupported(mutate("<Click id=\"0\">Надпись1Нажатие</Click>", "<Click id=\"999\">Надпись1Нажатие</Click>"), "unsupported-event", &["Надпись1", "Events"]);
}

#[test]
fn panel_table_and_command_bar_differences_are_rejected() {
    assert_unsupported(
        mutate_in(
            "<Panel name=\"Панель1\"",
            "<Page name=\"Страница2\">",
            "<Page name=\"Страница2\" extra=\"1\">",
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
    assert_unsupported(mutate_in("<Table name=\"ТабличноеПоле1\"", "<Editor>InputField</Editor>", "<Editor>Other</Editor>"), "unsupported-table", &["ТабличноеПоле1", "Columns"]);
    assert_unsupported(
        mutate_in(
            "<CommandBar name=\"ОсновныеДействияФормы\"",
            "representation=\"Text\"",
            "representation=\"Bad\"",
        ),
        "unsupported-command-bar",
        &["ОсновныеДействияФормы", "Buttons"],
    );
}

#[test]
fn property_attribute_and_unknown_element_differences_are_rejected() {
    assert_unsupported(
        mutate("<DataPath attributeId=\"3\">ПолеВвода1</DataPath>", "<DataPath attributeId=\"4\">ПолеВвода1</DataPath>"),
        "unsupported-property",
        &["ПолеВвода1", "DataPath"],
    );
    assert_unsupported(
        mutate_in(
            r#"<InputField name="ПолеВвода1" id="7">"#,
            "<ValueType kind=\"ValueList\" composite=\"false\"/>",
            "<ValueType kind=\"Unknown\" composite=\"false\"/>",
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
    let combined = replace_once(
        &mutate_in(
            r#"<InputField name="ПолеВвода1" id="7">"#,
            "<ValueType kind=\"ValueList\" composite=\"false\"/>",
            "<ValueType kind=\"Unknown\" composite=\"false\"/>",
        ),
        "<Left>79</Left>",
        "<Left>80</Left>",
    );
    assert_unsupported(combined, "unsupported-property", &["ValueType"]);
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
        // `extract` now shows the extended properties: a property line the edit inserts replaces the baseline's own
        let prop_indent = format!("{indent}  ");
        for l in to.split('\n') {
            let Some(rest) = l.strip_prefix(prop_indent.as_str()).and_then(|r| r.strip_prefix('<')) else { continue };
            let tag: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            if tag.is_empty() || from.contains(&format!("<{tag}")) {
                continue;
            }
            let line = regex::Regex::new(&format!(r"\n{prop_indent}<{tag}( [^>]*)?(/>|>[^<]*</{tag}>)")).unwrap();
            let found = line.find(&block).map(|m| m.range());
            if let Some(r) = found {
                block.replace_range(r, "");
            }
        }
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
    let xml = render_form_xml(&common::form_info(id, fname, raw), &model).unwrap();
    String::from_utf8(enrich_ext_properties(raw, &xml).unwrap()).unwrap().replace("\r\n", "\n")
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
    let xml = xml.replace("<DataPath attributeId=\"3\">ПолеВвода1</DataPath>", "<DataPath attributeId=\"4\">ПолеВвода1</DataPath>");
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

// ---------------------------------------------------------------- structural edits (add/remove)

/// Round-trip a structural edit: patch, decode, verify semantics, preservation and id allocation.
fn structural_round_trip(name: &str, edited: &str) -> (PatchedForm, FormXml) {
    let s = stream(name);
    let pf = patch_form(&s, baseline(name).as_bytes(), edited.as_bytes()).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    let decoded = decode_xml(name, &pf.bytes);
    let got = parse_form_xml(decoded.as_bytes(), "decoded").unwrap();
    (pf, got)
}

fn added_label(page_xml: Option<&str>) -> String {
    // A new LabelDecoration placed after the existing Надпись1.
    let mut xml = baseline("DonorForm");
    let label = "<LabelDecoration name=\"Добавленная\" id=\"999\">\n          <Caption><Item language=\"ru\">Новая надпись</Item></Caption>\n          <Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>\n        </LabelDecoration>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    if page_xml.is_none() {
        assert!(xml.contains(anchor));
        xml = xml.replacen(anchor, &(label.to_string() + anchor), 1);
    }
    xml
}

#[test]
fn add_control_clones_the_donor_prototype() {
    let edited = added_label(None);
    let (pf, got) = structural_round_trip("DonorForm", &edited);
    assert_eq!(pf.allocated.len(), 1);
    let (xml_id, new_id) = pf.allocated[0];
    assert_eq!(xml_id, 999);
    assert_eq!(new_id, 37); // donor max id 36 + 1
    // max-id slot updated in the stream
    let s = parse_spanned(&pf.bytes).unwrap();
    let idx = oneform::form_patch::ControlIndex::build(&s).unwrap();
    assert_eq!(oneform::donor::max_control_id(&s).unwrap(), 37);
    let e = idx.get(37).unwrap();
    assert_eq!((e.name.as_str(), e.type_name), ("Добавленная", "LabelDecoration"));
    // decoded values match the requested ones
    let c = got.control(37).unwrap();
    assert_eq!(c.values.left, 500);
    assert_eq!(c.values.top, 500);
    assert_eq!(c.values.width, 80);
    assert_eq!(c.values.height, 20);
    assert_eq!(c.values.caption, vec![("ru".to_string(), "Новая надпись".to_string())]);
    assert!(c.values.visible);
    // untouched controls still there
    assert!(got.control(5).is_some());
    let _ = edited;
}

#[test]
fn add_control_with_event_handler() {
    let mut edited = added_label(None);
    edited = edited.replacen(
        "<ZOrder>50</ZOrder><TabOrder>50</TabOrder>",
        "<ZOrder>50</ZOrder><TabOrder>50</TabOrder><Events><Click id=\"0\">ДобавленнаяНажатие</Click></Events>",
        1,
    );
    let (_pf, got) = structural_round_trip("DonorForm", &edited);
    let c = got.control(37).unwrap();
    assert_eq!(c.values.events, vec![(0, "ДобавленнаяНажатие".to_string())]);
}

#[test]
fn add_control_to_a_panel_page() {
    let mut xml = baseline("DonorForm");
    let label = "<LabelDecoration name=\"НаВторойСтранице\" id=\"999\">\n              <Caption><Item language=\"ru\">Вторая</Item></Caption>\n              <Left>10</Left><Top>10</Top><Width>50</Width><Height>20</Height>\n              <ZOrder>9</ZOrder><TabOrder>9</TabOrder>\n            </LabelDecoration>\n            ";
    let anchor = "<Table name=\"ТабличноеПоле2\"";
    assert!(xml.contains(anchor));
    xml = xml.replacen(anchor, &(label.to_string() + anchor), 1);
    let (pf, got) = structural_round_trip("DonorForm", &xml);
    let c = got.control(pf.allocated[0].1).unwrap();
    assert_eq!(c.page.as_deref(), Some("Страница2"));
    // parent is Панель1 (id 13)
    assert_eq!(c.parent, Some(13));
}

#[test]
fn remove_control_removes_record_and_links() {
    // ПолеВвода1 (id 7) has a DataPath link in the attributes section.
    let (edited, _) = cut_block(&baseline("DonorForm"), "<InputField name=\"ПолеВвода1\"", "</InputField>");
    let s = stream("DonorForm");
    let pf = patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes()).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    let reparsed = parse_spanned(&pf.bytes).unwrap();
    let idx = oneform::form_patch::ControlIndex::build(&reparsed).unwrap();
    assert!(idx.get(7).is_none(), "control still present");
    // no dangling reference to id 7 in the links section: the link row is removed
    // (the form ATTRIBUTE named ПолеВвода1 stays — removing attributes is out of scope)
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    assert!(got.control(7).is_none());
    assert!(got.control(5).is_some());
}

#[test]
fn add_remove_and_property_edit_combine() {
    let mut edited = added_label(None);
    // remove Надпись1 (id 5)
    let (without, _) = cut_block(&edited, "<LabelDecoration name=\"Надпись1\"", "</LabelDecoration>");
    edited = without;
    // and change an untouched control's caption
    edited = replace_once(&edited, "<Item language=\"ru\">Кнопка1</Item>", "<Item language=\"ru\">Кнопка-новая</Item>");
    let (pf, got) = structural_round_trip("DonorForm", &edited);
    assert!(got.control(5).is_none());
    let c = got.control(pf.allocated[0].1).unwrap();
    assert_eq!(c.name, "Добавленная");
    let btn = got.control(4).unwrap();
    assert_eq!(btn.values.caption, vec![("ru".to_string(), "Кнопка-новая".to_string())]);
}

#[test]
fn add_under_command_bar_is_rejected() {
    let mut xml = baseline("DonorForm");
    let label = "<LabelDecoration name=\"Плохая\" id=\"999\"><Left>0</Left><Top>0</Top><Width>1</Width><Height>1</Height><ZOrder>0</ZOrder></LabelDecoration>\n            ";
    let anchor = "<Buttons>";
    assert!(xml.contains(anchor));
    xml = xml.replacen(anchor, &format!("{label}{anchor}"), 1);
    let e = patch_form(&stream("DonorForm"), baseline("DonorForm").as_bytes(), xml.as_bytes()).unwrap_err();
    assert!(format!("{e}").contains("CommandBar"), "{e}");
}

#[test]
fn two_additions_get_monotonic_ids() {
    let mut edited = added_label(None);
    let second = "<LabelDecoration name=\"Вторая\" id=\"1000\"><Left>600</Left><Top>500</Top><Width>80</Width><Height>20</Height><ZOrder>51</ZOrder><TabOrder>51</TabOrder></LabelDecoration>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(second.to_string() + anchor), 1);
    let (pf, got) = structural_round_trip("DonorForm", &edited);
    assert_eq!(pf.allocated, vec![(999, 37), (1000, 38)]); // XML document order
    assert!(got.control(37).is_some() && got.control(38).is_some());
    assert_eq!(oneform::donor::max_control_id(&parse_spanned(&pf.bytes).unwrap()).unwrap(), 38);
}

#[test]
fn discovered_appearance_and_inputfield_properties_round_trip() {
    let base = baseline("DonorForm");
    let edited = edit_ctl(
        &base,
        "ПолеВвода1",
        &[
            ("<ChoiceButton>true</ChoiceButton>", "<Enabled>false</Enabled><TransparentBack>true</TransparentBack><Tooltip><Item language=\"ru\">Новая подсказка</Item></Tooltip><ChoiceButton>false</ChoiceButton>"),
            ("<ClearButton>true</ClearButton>", "<ClearButton>false</ClearButton>"),
            ("<OpenButton>true</OpenButton>", "<OpenButton>false</OpenButton><DropListButton>false</DropListButton><SpinButton>false</SpinButton><ReadOnly>true</ReadOnly><MarkNegatives>true</MarkNegatives><AutoCellHeight>false</AutoCellHeight><AutoChoiceIncomplete>false</AutoChoiceIncomplete><ChoiceListMode>true</ChoiceListMode>"),
            ("<AutoMarkIncomplete>true</AutoMarkIncomplete>", "<AutoMarkIncomplete>false</AutoMarkIncomplete>"),
        ],
    );
    let pf = round_trip("DonorForm", &edited);
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    let c = got.control(7).unwrap();
    assert!(!c.values.enabled);
    assert!(c.values.transparent_back);
    assert!(c.values.read_only);
    assert_eq!(c.values.choice_button, Some(false));
    assert_eq!(c.values.clear_button, Some(false));
    assert_eq!(c.values.open_button, Some(false));
    assert!(!c.values.auto_mark_incomplete);
    assert!(c.values.mark_negatives);
    assert!(c.values.choice_list_mode);
}

#[test]
fn discovered_label_checkbox_table_panel_and_commandbar_properties_round_trip() {
    let base = baseline("DonorForm");
    let mut edited = edit_ctl(
        &base,
        "Надпись1",
        &[("<TabOrder>3</TabOrder>", "<TabOrder>3</TabOrder><Enabled>false</Enabled><HorizontalAlign>Center</HorizontalAlign><Hyperlink>true</Hyperlink>")],
    );
    edited = edit_ctl(
        &edited,
        "Флажок1",
        &[("<TabOrder>5</TabOrder>", "<TabOrder>5</TabOrder><Enabled>false</Enabled><TitleLocation>Left</TitleLocation><ThreeState>true</ThreeState>")],
    );
    edited = edit_ctl(
        &edited,
        "ТабличноеПоле1",
        &[("<Footer>false</Footer>", "<Footer>true</Footer><SelectionMode>Multi</SelectionMode>")],
    );
    edited = edit_ctl(
        &edited,
        "ОсновныеДействияФормы",
        &[
            ("<AutoFill>false</AutoFill>", "<AutoFill>true</AutoFill>"),
            ("<HorizontalAlign>Right</HorizontalAlign>", "<HorizontalAlign>Left</HorizontalAlign>"),
        ],
    );
    edited = edit_ctl(
        &edited,
        "Панель1",
        &[("<ShowTabs type=\"ShowTabs\" member=\"Unknown\" code=\"2\"/>", "<ShowTabs type=\"ShowTabs\" member=\"Top\" code=\"1\"/>")],
    );
    let pf = round_trip("DonorForm", &edited);
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    let lbl = got.control(5).unwrap();
    assert!(!lbl.values.enabled);
    assert_eq!(lbl.values.horizontal_align, "Center");
    assert!(lbl.values.hyperlink);
    let chk = got.control(8).unwrap();
    assert!(!chk.values.enabled);
    assert_eq!(chk.values.title_location, "Left");
    assert!(chk.values.three_state);
    let tbl = got.control(6).unwrap();
    assert_eq!(tbl.values.footer, Some(true));
    assert_eq!(tbl.values.selection_mode, "Multi");
    let bar = got.control(2).unwrap();
    assert_eq!(bar.values.auto_fill, Some(true));
    assert_eq!(bar.values.horizontal_align, "");
    let pnl = got.control(13).unwrap();
    assert_eq!(pnl.values.show_tabs, Some(1));
}

#[test]
fn donor_added_controls_accept_discovered_properties() {
    let mut edited = baseline("DonorForm");
    let new_inp = "<InputField name=\"НовоеПоле\" id=\"999\">\n          <Left>500</Left><Top>500</Top><Width>120</Width><Height>20</Height>\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>\n          <Enabled>false</Enabled><ReadOnly>true</ReadOnly><ChoiceButton>true</ChoiceButton><MarkNegatives>true</MarkNegatives>\n        </InputField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(new_inp.to_string() + anchor), 1);
    let (pf, got) = structural_round_trip("DonorForm", &edited);
    let c = got.control(pf.allocated[0].1).unwrap();
    assert!(!c.values.enabled);
    assert!(c.values.read_only);
    assert_eq!(c.values.choice_button, Some(true));
    assert_eq!(c.values.clear_button, Some(false));
    assert!(c.values.mark_negatives);
}

#[test]
fn existing_control_rename_and_events_round_trip() {
    let mut edited = baseline("DonorForm");
    // Rename Надпись1 -> НадписьПереименованная and change its Click handler
    edited = edited
        .replacen(r#"<LabelDecoration name="Надпись1" id="5">"#, r#"<LabelDecoration name="НадписьПереименованная" id="5">"#, 1)
        .replacen("<Click id=\"0\">Надпись1Нажатие</Click>", "<Click id=\"0\">НовыйОбработчикНажатия</Click>", 1);
    // Remove Events from Кнопка1 (id 4)
    edited = edit_ctl(
        &edited,
        "Кнопка1",
        &[("<Click id=\"0\">Кнопка1Нажатие</Click>", "")],
    );
    let pf = round_trip("DonorForm", &edited);
    let step1_xml = decode_xml("DonorForm", &pf.bytes);
    let got = parse_form_xml(step1_xml.as_bytes(), "decoded").unwrap();
    let lbl = got.control(5).unwrap();
    assert_eq!(lbl.name, "НадписьПереименованная");
    assert_eq!(lbl.values.events, vec![(0, "НовыйОбработчикНажатия".to_string())]);
    let btn = got.control(4).unwrap();
    assert!(btn.values.events.is_empty());

    // Now add <Events> back onto Кнопка1 (which currently has an empty {0} event list in pf.bytes!)
    let re_added = step1_xml.replacen(
        "<Button name=\"Кнопка1\" id=\"4\">",
        "<Button name=\"Кнопка1\" id=\"4\">\n          <Events>\n            <Click id=\"0\">Кнопка1СноваНажатие</Click>\n          </Events>",
        1,
    );
    let step1_src = parse_spanned(&pf.bytes).unwrap();
    let pf2 = patch_form(&step1_src, step1_xml.as_bytes(), re_added.as_bytes()).unwrap();
    let got2 = parse_form_xml(decode_xml("DonorForm", &pf2.bytes).as_bytes(), "decoded").unwrap();
    assert_eq!(got2.control(4).unwrap().values.events, vec![(0, "Кнопка1СноваНажатие".to_string())]);
}

#[test]
fn panel_pages_and_table_columns_round_trip() {
    let mut edited = baseline("DonorForm");
    // Edit Панель1 Page Страница2: change Title, make Visible=true (remove <Visible>false</Visible>), set Enabled=false
    edited = edited.replacen(
        "<Page name=\"Страница2\">\r\n              <Title>\r\n                <Item language=\"ru\">Страница2</Item>",
        "<Page name=\"Страница2\">\r\n              <Title>\r\n                <Item language=\"ru\">Вкладка 2</Item>",
        1,
    );
    if !edited.contains("Вкладка 2") {
        edited = edited.replacen(
            "<Page name=\"Страница2\">\n              <Title>\n                <Item language=\"ru\">Страница2</Item>",
            "<Page name=\"Страница2\">\n              <Title>\n                <Item language=\"ru\">Вкладка 2</Item>",
            1,
        );
    }
    edited = edited.replacen("              <Visible>false</Visible>", "              <Enabled>false</Enabled>", 1);

    // Edit ТабличноеПоле1 Column Колонка1: rename, change Header, FooterText, Tooltip, Width, Visible, ReadOnly
    edited = edited
        .replacen(r#"<Column name="Колонка1" id="0">"#, r#"<Column name="КолонкаОсновная" id="0">"#, 1)
        .replacen("<Item language=\"ru\">Колонка1</Item>", "<Item language=\"ru\">Заголовок 1</Item>", 1)
        .replacen(
            "<Width>100</Width>",
            "<FooterText><Item language=\"ru\">Итого</Item></FooterText><Tooltip><Item language=\"ru\">Подсказка колонки</Item></Tooltip><Width>155</Width><Visible>false</Visible><ReadOnly>true</ReadOnly>",
            1,
        );
    let pf = round_trip("DonorForm", &edited);
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    let pnl = got.control(13).unwrap();
    assert_eq!(pnl.values.pages[1].title, vec![("ru".to_string(), "Вкладка 2".to_string())]);
    assert!(pnl.values.pages[0].visible);
    assert!(!pnl.values.pages[0].enabled);

    let tbl = got.control(6).unwrap();
    let col = &tbl.values.columns[0];
    assert_eq!(col.name, "КолонкаОсновная");
    assert_eq!(col.header, vec![("ru".to_string(), "Заголовок 1".to_string())]);
    assert_eq!(col.footer_text, vec![("ru".to_string(), "Итого".to_string())]);
    assert_eq!(col.tooltip, vec![("ru".to_string(), "Подсказка колонки".to_string())]);
    assert_eq!(col.width, 155);
    assert!(!col.visible);
    assert!(col.read_only);
}

#[test]
fn form_attributes_and_control_datapath_round_trip() {
    let mut edited = baseline("DonorForm");
    // 1. Add a new attribute id="16" name="НовыйСтроковыйРеквизит"
    edited = edited.replacen(
        "  </Attributes>",
        "    <Attribute id=\"16\" name=\"НовыйСтроковыйРеквизит\">\n      <Main>true</Main>\n      <ValueType kind=\"String\" composite=\"false\"/>\n    </Attribute>\n  </Attributes>",
        1,
    );
    // 2. Rebind ПолеВвода1 (id 7) from attributeId="3" (ПолеВвода1) to attributeId="16" (НовыйСтроковыйРеквизит)
    edited = edited.replacen(
        "<DataPath attributeId=\"3\">ПолеВвода1</DataPath>",
        "<DataPath attributeId=\"16\">НовыйСтроковыйРеквизит</DataPath>",
        1,
    );
    // 3. Add a new InputField bound to attributeId="14" (ПолеВвода7)
    let new_inp = "<InputField name=\"НовоеПолеСПривязкой\" id=\"999\">\n          <DataPath attributeId=\"14\">ПолеВвода7</DataPath>\n          <Left>520</Left><Top>500</Top><Width>120</Width><Height>20</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n        </InputField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(new_inp.to_string() + anchor), 1);

    let (pf, got) = structural_round_trip("DonorForm", &edited);
    assert!(got.attributes.iter().any(|a| a.id == 16 && a.name == "НовыйСтроковыйРеквизит" && a.kind == "String"));
    assert_eq!(got.control(7).unwrap().values.data_path, Some((16, "НовыйСтроковыйРеквизит".to_string())));
    assert_eq!(
        got.control(pf.allocated[0].1).unwrap().values.data_path,
        Some((14, "ПолеВвода7".to_string()))
    );
}

#[test]
fn usualgroup_choicefield_and_spreadsheet_events_round_trip() {
    let mut edited = baseline("DonorForm");
    // 1. Add a new UsualGroup (with BorderStyle SingleLine) and a new ChoiceField (with OnChange event)
    let additions = "<UsualGroup name=\"ГруппаНовая\" id=\"998\">\n          <Caption><Item language=\"ru\">Группа 1</Item></Caption>\n          <Left>500</Left><Top>500</Top><Width>200</Width><Height>80</Height>\n          <ZOrder>56</ZOrder><TabOrder>56</TabOrder>\n          <BorderStyle code=\"4\">SingleLine</BorderStyle>\n        </UsualGroup>\n        <ChoiceField name=\"ПолеВыбораНовое\" id=\"999\">\n          <Left>510</Left><Top>520</Top><Width>140</Width><Height>20</Height>\n          <ZOrder>57</ZOrder><TabOrder>57</TabOrder>\n          <Events>\n            <OnChange id=\"2147483647\">ПолеВыбораНовоеПриИзменении</OnChange>\n          </Events>\n        </ChoiceField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(additions.to_string() + anchor), 1);
    // 2. Add DragStart (id=3) event onto existing ПолеТабличногоДокумента1 (id 15)
    edited = edit_ctl(
        &edited,
        "ПолеТабличногоДокумента1",
        &[(
            "<TabOrder>19</TabOrder>",
            "<TabOrder>19</TabOrder><Events><DragStart id=\"3\">ТабДокНачалоПеретаскивания</DragStart></Events>",
        )],
    );
    let (pf, got) = structural_round_trip("DonorForm", &edited);
    let grp = got.control(pf.allocated[0].1).unwrap();
    assert_eq!(grp.type_name, "UsualGroup");
    assert_eq!(grp.values.border_style, Some(4));
    let cf = got.control(pf.allocated[1].1).unwrap();
    assert_eq!(cf.type_name, "ChoiceField");
    assert_eq!(cf.values.events, vec![(2147483647, "ПолеВыбораНовоеПриИзменении".to_string())]);
    let sp = got.control(15).unwrap();
    assert_eq!(sp.values.events, vec![(3, "ТабДокНачалоПеретаскивания".to_string())]);
}

#[test]
fn remaining_colors_fonts_scrollbars_valuetype_buttons_and_form_header_round_trip() {
    let mut edited = baseline("DonorForm");
    // 1. Form-level Caption and Events
    edited = edited
        .replacen(
            "<Item language=\"ru\">Обработка  ВнешняяОбработка1</Item>",
            "<Item language=\"ru\">Новый заголовок формы</Item>",
            1,
        )
        .replacen(
            "<BeforeOpen id=\"70000\">ПередОткрытием</BeforeOpen>",
            "<BeforeOpen id=\"70000\">ПередОткрытиемНовый</BeforeOpen>",
            1,
        );
    // 2. InputField ПолеВвода1: BackColor, TextColor, BorderColor, Font, ValueType, HorizontalScrollBar, VerticalScrollBar
    edited = edit_ctl(
        &edited,
        "ПолеВвода1",
        &[(
            "<ValueType kind=\"ValueList\" composite=\"false\"/>",
            "<BackColor kind=\"rgb\" value=\"#FF0000\"/><TextColor kind=\"rgb\" value=\"#0000FF\"/><BorderColor kind=\"rgb\" value=\"#00FF00\"/><Font bold=\"true\" source=\"bin\"/><ValueType kind=\"String\" composite=\"false\"/><HorizontalScrollBar>Use</HorizontalScrollBar><VerticalScrollBar>Auto</VerticalScrollBar>",
        )],
    );
    // 3. Table ТабличноеПоле1: InitialTreeView, Column DataPath & Events
    edited = edited
        .replacen(
            "<Footer>false</Footer>",
            "<Footer>false</Footer><InitialTreeView>ExpandTopLevel</InitialTreeView>",
            1,
        )
        .replacen(
            "<DataPath attributeId=\"2\">ТабличноеПоле1.Колонка1</DataPath>",
            "<DataPath attributeId=\"2\">ТабличноеПоле1.КолонкаПервая</DataPath>",
            1,
        )
        .replacen(
            "<StartChoice id=\"2\">ТабличноеПоле1Колонка1НачалоВыбора</StartChoice>",
            "<StartChoice id=\"2\">НовыйОбработчикКолонки</StartChoice>",
            1,
        );
    // 4. CommandBar ОсновныеДействияФормы: Button caption and action
    edited = edited
        .replacen(
            "action=\"КнопкаВыполнитьНажатие\"",
            "action=\"КнопкаВыполнитьНовоеНажатие\"",
            1,
        )
        .replacen(
            "<Item language=\"ru\">Выполнить</Item>\n                <Item language=\"ro\">Выполнить</Item>",
            "<Item language=\"ru\">Запустить</Item>\n                <Item language=\"ro\">Выполнить</Item>",
            1,
        );
    // 5. Add ChoiceField with ChoiceList
    let new_cf = "<ChoiceField name=\"ПолеВыбораСоСписком\" id=\"999\">\n          <Left>520</Left><Top>520</Top><Width>140</Width><Height>20</Height>\n          <ZOrder>58</ZOrder><TabOrder>58</TabOrder>\n          <ChoiceList>\n            <ChoiceItem value=\"Зн1\">\n              <Presentation>\n                <Item language=\"ru\">Первый вариант</Item>\n              </Presentation>\n            </ChoiceItem>\n          </ChoiceList>\n        </ChoiceField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(new_cf.to_string() + anchor), 1);

    let (pf, got) = structural_round_trip("DonorForm", &edited);
    assert_eq!(got.form_caption, vec![("ru".to_string(), "Новый заголовок формы".to_string())]);
    assert!(got.form_events.contains(&(70000, "ПередОткрытиемНовый".to_string())));
    let inp = got.control(7).unwrap();
    assert_eq!(inp.values.horizontal_scroll_bar, "Use");
    assert_eq!(inp.values.vertical_scroll_bar, "Auto");
    assert_eq!(inp.values.value_type.as_ref().map(|vt| vt.kind.as_str()), Some("String"));
    assert!(inp.values.back_color.is_some());
    assert!(inp.values.font.as_ref().is_some_and(|f| f.bold));
    let tbl = got.control(6).unwrap();
    assert_eq!(tbl.values.initial_tree_view, "ExpandTopLevel");
    assert_eq!(tbl.values.columns[0].field_name.as_deref(), Some("КолонкаПервая"));
    assert!(tbl.values.columns[0].events.contains(&(2, "НовыйОбработчикКолонки".to_string())));
    let bar = got.control(3).unwrap();
    assert_eq!(bar.values.buttons[0].action, "КнопкаВыполнитьНовоеНажатие");
    let cf = got.control(pf.allocated[0].1).unwrap();
    assert_eq!(cf.values.choice_list.len(), 1);
    assert_eq!(cf.values.choice_list[0].value, "Зн1");
}

const GUID_A: &str = "47e6dd42-cd5c-4e3a-9bc4-14816ed53d38";
const GUID_B: &str = "44a01d39-c2b6-4751-85e6-06fdc2f23043";

#[test]
fn concrete_reference_and_qualified_valuetypes_round_trip() {
    let mut edited = baseline("DonorForm");
    let new_attrs = format!(
        "    <Attribute id=\"16\" name=\"ОднаСсылка\">\n      <ValueType kind=\"Ref\" composite=\"false\" refGuid=\"{GUID_A}\"/>\n    </Attribute>\n\
         \x20   <Attribute id=\"17\" name=\"НесколькоСсылок\">\n      <ValueType kind=\"Ref\" composite=\"true\" refGuid=\"{GUID_A};{GUID_B}\"/>\n    </Attribute>\n\
         \x20   <Attribute id=\"18\" name=\"Сумма\">\n      <ValueType kind=\"Number\" composite=\"false\" digits=\"15\" fractionDigits=\"2\"/>\n    </Attribute>\n\
         \x20   <Attribute id=\"19\" name=\"Строка\">\n      <ValueType kind=\"String\" composite=\"false\" length=\"50\"/>\n    </Attribute>\n  </Attributes>"
    );
    edited = edited.replacen("  </Attributes>", &new_attrs, 1);
    let vt = |from: &str, to: String| (from.to_string(), to);
    let ctl_edits: [(&str, (String, String)); 4] = [
        ("ПолеВвода1", vt("<ValueType kind=\"ValueList\" composite=\"false\"/>", format!("<ValueType kind=\"Ref\" composite=\"false\" refGuid=\"{GUID_A}\"/>"))),
        ("ПолеВвода7", vt("<ValueType kind=\"String\" composite=\"false\"/>", "<ValueType kind=\"String\" composite=\"false\" length=\"50\"/>".to_string())),
        ("ПолеВвода2", vt("<ValueType kind=\"Number\" composite=\"false\" fractionDigits=\"0\"/>", "<ValueType kind=\"Number\" composite=\"false\" digits=\"15\" fractionDigits=\"2\"/>".to_string())),
        ("ПолеВвода6", vt("<ValueType kind=\"Ref\" composite=\"true\"/>", format!("<ValueType kind=\"Ref\" composite=\"true\" refGuid=\"{GUID_A};{GUID_B}\"/>"))),
    ];
    for (name, (from, to)) in &ctl_edits {
        edited = edit_ctl(&edited, name, &[(from, to)]);
    }

    let (_, got) = structural_round_trip("DonorForm", &edited);
    let attr = |id: i64| got.attributes.iter().find(|a| a.id == id).unwrap();
    assert_eq!((attr(16).kind.as_str(), attr(16).composite), ("Ref", false));
    assert_eq!(attr(16).ref_guids, vec![GUID_A]);
    assert_eq!((attr(17).kind.as_str(), attr(17).composite), ("Ref", true));
    assert_eq!(attr(17).ref_guids, vec![GUID_A, GUID_B]);
    assert_eq!((attr(18).digits, attr(18).fraction_digits), (Some(15), Some(2)));
    assert_eq!(attr(19).length, Some(50));

    let value_type = |id: i64| got.control(id).unwrap().values.value_type.clone().unwrap();
    let (vt7, vt33, vt28, vt32) = (value_type(7), value_type(33), value_type(28), value_type(32));
    assert_eq!((vt7.kind.as_str(), vt7.composite), ("Ref", false));
    assert_eq!(vt7.ref_guids, vec![GUID_A]);
    assert_eq!((vt33.kind.as_str(), vt33.length), ("String", Some(50)));
    assert_eq!((vt28.digits, vt28.fraction_digits), (Some(15), Some(2)));
    assert_eq!((vt32.composite, vt32.ref_guids), (true, vec![GUID_A.to_string(), GUID_B.to_string()]));
}

#[test]
fn inconsistent_valuetype_qualifiers_are_not_editable() {
    let base = baseline("DonorForm");
    let from = "<ValueType kind=\"String\" composite=\"false\"/>";
    for bad in [
        "<ValueType kind=\"String\" composite=\"false\" digits=\"5\"/>",
        "<ValueType kind=\"String\" composite=\"false\" length=\"0\"/>",
        "<ValueType kind=\"String\" composite=\"false\" length=\"x\"/>",
        "<ValueType kind=\"String\" composite=\"false\" refGuid=\"47e6dd42-cd5c-4e3a-9bc4-14816ed53d38\"/>",
        "<ValueType kind=\"Ref\" composite=\"true\" refGuid=\"47e6dd42-cd5c-4e3a-9bc4-14816ed53d38\"/>",
        "<ValueType kind=\"Ref\" composite=\"false\" refGuid=\"acf6192e-81ca-46ef-93a6-5a6968b78663\"/>",
    ] {
        let (kind, text) = patch_rejected("DonorForm", &edit_ctl(&base, "ПолеВвода7", &[(from, bad)]));
        assert_eq!(kind, "unsupported-property", "{bad}: {text}");
    }
}

/// `(target, side, offset)` of the four anchor edges (top, bottom, left, right) of control `id` in `bytes`.
fn anchor_edges(bytes: &[u8], id: i64) -> [Option<(i64, i64, i64)>; 4] {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let geom = idx.get(id).unwrap().geom().unwrap().node;
    [6, 7, 8, 9].map(|i| oneform::form_patch::read_anchor_edge(geom, i).map(|e| (e.target, e.side, e.offset)))
}

#[test]
fn secondary_anchor_edges_round_trip() {
    // ПолеВвода1 (id 7): the primary right edge stays, only the secondary attachment of the bottom edge changes.
    let mut edited = baseline("DonorForm");
    edited = edit_ctl(
        &edited,
        "ПолеВвода1",
        &[(
            "<Anchors bottom=\"Top\" bottomTarget=\"7\" bottomOffset=\"19\" right=\"Left\" rightTarget=\"14\" rightOffset=\"-288\"/>",
            "<Anchors bottom=\"Top\" bottomTarget=\"7\" bottomOffset=\"19\" bottomAlt=\"Bottom\" bottomAltOffset=\"-40\" right=\"Left\" rightTarget=\"14\" rightOffset=\"-288\"/>",
        )],
    );
    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let alt = |bytes: &[u8]| {
        let s = parse_spanned(bytes).unwrap();
        let idx = ControlIndex::build(&s).unwrap();
        let geom = idx.get(7).unwrap().geom().unwrap().node;
        oneform::form_patch::read_anchor_edge_alt(geom, 7).map(|e| (e.target, e.side, e.offset))
    };
    let before = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();
    assert_eq!(alt(&before), Some((-1, 6, 0)));
    assert_eq!(alt(&pf.bytes), Some((0, 1, -40)));
    assert_eq!(anchor_edges(&pf.bytes, 7), anchor_edges(&before, 7), "primary edges are untouched");
}

#[test]
fn control_anchors_round_trip() {
    let before = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();

    let mut edited = baseline("DonorForm");
    edited = edit_ctl(
        &edited,
        "ТабличноеПоле1",
        &[(
            "<Anchors bottom=\"Top\" bottomOffset=\"218\" bottomAlt=\"Bottom\" bottomAltOffset=\"-455\" right=\"Left\" rightOffset=\"214\" rightAlt=\"Left\" rightAltTarget=\"14\" rightAltOffset=\"-489\"/>",
            "<Anchors left=\"Left\" top=\"Top\" right=\"Right\" bottom=\"Bottom\" leftOffset=\"6\" topOffset=\"24\" rightOffset=\"-6\" bottomOffset=\"-6\"/>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеВвода1",
        &[(
            "<Anchors bottom=\"Top\" bottomTarget=\"7\" bottomOffset=\"19\" right=\"Left\" rightTarget=\"14\" rightOffset=\"-288\"/>",
            "<Anchors right=\"Right\" rightOffset=\"-10\"/>",
        )],
    );
    let button = "<Button name=\"Добавленная\" id=\"999\">\n          <Caption><Item language=\"ru\">ОК</Item></Caption>\n          <Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>\n          <Anchors top=\"Bottom\" bottom=\"Bottom\" left=\"Right\" right=\"Right\" topOffset=\"-25\" bottomOffset=\"-5\" leftOffset=\"-80\" rightOffset=\"-5\"/>\n        </Button>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(button.to_string() + anchor), 1);

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let table = anchor_edges(&pf.bytes, 6);
    assert_eq!(table, [Some((0, 0, 24)), Some((0, 1, -6)), Some((0, 2, 6)), Some((0, 3, -6))]);

    // Partial: only the right edge of ПолеВвода1 changes.
    let (old, new) = (anchor_edges(&before, 7), anchor_edges(&pf.bytes, 7));
    assert_eq!(new[3], Some((0, 3, -10)));
    assert_eq!(new[..3], old[..3]);

    let added = pf.allocated[0].1;
    assert_eq!(anchor_edges(&pf.bytes, added), [Some((0, 1, -25)), Some((0, 1, -5)), Some((0, 3, -80)), Some((0, 3, -5))]);
}

#[test]
fn malformed_anchors_are_not_editable() {
    let base = baseline("DonorForm");
    let from = "<Anchors bottom=\"Top\" bottomTarget=\"7\" bottomOffset=\"19\" right=\"Left\" rightTarget=\"14\" rightOffset=\"-288\"/>";
    for bad in [
        "<Anchors/>",
        "<Anchors rightOffset=\"-10\"/>",
        "<Anchors right=\"Right\" rightOffset=\"x\"/>",
        "<Anchors right=\"Right\" target=\"x\"/>",
        "<Anchors right=\"Middle\"/>",
        "<Anchors right=\"None\" rightOffset=\"3\"/>",
    ] {
        let xml = edit_ctl(&base, "ПолеВвода1", &[(from, bad)]);
        let (kind, text) = patch_rejected("DonorForm", &xml);
        assert_eq!(kind, "unsupported-unknown-element", "{bad}: {text}");
    }
}

#[test]
fn edited_attribute_keeps_the_original_type_pattern_bytes() {
    // The stream pattern `{"N",10,0,1}` carries a sign flag that `<ValueType>` cannot express.
    let edited = replace_once(
        &baseline("DonorForm"),
        "<Attribute id=\"7\" name=\"ПолосаРегулирования1\">\n      <Main>true</Main>",
        "<Attribute id=\"7\" name=\"ПолосаРегулирования1\">\n      <Main>true</Main>\n      <StoredData>true</StoredData>",
    );
    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    let at = text.find("{{7},1,1,1,\"ПолосаРегулирования1\",").expect("stored flag not set");
    let tail: String = text[at..].chars().filter(|c| !c.is_whitespace()).collect();
    assert!(tail.contains("\"ПолосаРегулирования1\",{\"Pattern\",{\"N\",10,0,1}}"), "pattern bytes changed");
}

#[test]
fn inputfield_flags_alignment_typelink_and_meta_flags_round_trip() {
    let mut edited = baseline("DonorForm");
    // 1. ПолеВвода7 (id 33, String): MultiLine, PasswordMode, ExtendedEdit, Mask, ChooseType, TextEdit, HorizontalAlign, VerticalAlign, ModifiesData, SkipOnInput, FirstInGroup
    edited = edit_ctl(
        &edited,
        "ПолеВвода7",
        &[(
            "<OpenButton>false</OpenButton>",
            "<OpenButton>false</OpenButton>\n          <ModifiesData>true</ModifiesData>\n          <SkipOnInput>true</SkipOnInput>\n          <FirstInGroup>true</FirstInGroup>\n          <MultiLine>true</MultiLine>\n          <PasswordMode>true</PasswordMode>\n          <ExtendedEdit>true</ExtendedEdit>\n          <Mask>@@@-999</Mask>\n          <ChooseType>false</ChooseType>\n          <TextEdit>false</TextEdit>\n          <HorizontalAlign>Center</HorizontalAlign>\n          <VerticalAlign>Bottom</VerticalAlign>",
        )],
    );
    // 2. ПолеВвода6 (id 32, AnyRef): TypeLink to attributeId="14" (ПолеВвода7)
    edited = edit_ctl(
        &edited,
        "ПолеВвода6",
        &[(
            "<OpenButton>false</OpenButton>",
            "<OpenButton>false</OpenButton>\n          <TypeLink attributeId=\"14\">ПолеВвода7</TypeLink>",
        )],
    );
    // 3. Added InputField with meta flags, alignment, mask, and TypeLink with fieldGuid
    let added = "<InputField name=\"ПолеНовое\" id=\"999\">\n          <Left>510</Left><Top>510</Top><Width>120</Width><Height>20</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n          <ModifiesData>true</ModifiesData>\n          <SkipOnInput>true</SkipOnInput>\n          <MultiLine>true</MultiLine>\n          <Mask>99-99</Mask>\n          <HorizontalAlign>Right</HorizontalAlign>\n          <VerticalAlign>Center</VerticalAlign>\n          <TypeLink attributeId=\"1\" fieldGuid=\"3efc6139-7638-42a9-9b06-9d9d953b97c3\">ОбработкаОбъект.Поле</TypeLink>\n        </InputField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(added.to_string() + anchor), 1);

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("{14,\"ПолеВвода7\",4294967295,1,1,1}"), "meta flags not patched on ПолеВвода7");
    assert!(compact.contains("9a7643d2-19e9-45e2-8893-280bc9195a97,{4,{\"U\"},{\"U\"},1,\"@@@-999\",1,1}"), "9a7643d2 string ext not patched");
    assert!(compact.contains("{1,1,{0,c866d8e7-76ff-4334-9d02-144f1efe5e1d,{2,1,{14}}}}"), "TypeLink attr not patched");
    assert!(compact.contains("{1,1,{0,c866d8e7-76ff-4334-9d02-144f1efe5e1d,{2,2,{1},{0,3efc6139-7638-42a9-9b06-9d9d953b97c3}}}}"), "TypeLink fieldGuid not patched");
}

#[test]
fn invalid_typelink_and_vertical_align_are_rejected() {
    let base = baseline("DonorForm");
    let from = "<OpenButton>false</OpenButton>";
    for bad in [
        "<TypeLink attributeId=\"14\">НеТоИмя</TypeLink>",
        "<TypeLink attributeId=\"999\">ПолеВвода7</TypeLink>",
        "<TypeLink attributeId=\"1\" fieldGuid=\"not-a-uuid\">ОбработкаОбъект.Поле</TypeLink>",
        "<VerticalAlign>Middle</VerticalAlign>",
    ] {
        let xml = edit_ctl(&base, "ПолеВвода7", &[(from, &format!("{from}\n          {bad}"))]);
        let (kind, _) = patch_rejected("DonorForm", &xml);
        assert!(
            matches!(kind.as_str(), "invalid-xml" | "unsupported-property" | "unsupported-unknown-element"),
            "{bad}: got {kind}"
        );
    }
}

#[test]
fn table_grid_flags_scrollbars_and_behavior_round_trip() {
    let mut edited = baseline("DonorForm");
    // 1. Patch existing ТабличноеПоле1 (id 6): multiple bitmask flags simultaneously + discrete slots
    edited = edit_ctl(
        &edited,
        "ТабличноеПоле1",
        &[(
            "<Footer>false</Footer>",
            "<Footer>true</Footer>\n          <Header>false</Header>\n          <HorizontalLines>false</HorizontalLines>\n          <VerticalLines>false</VerticalLines>\n          <ReadOnly>true</ReadOnly>\n          <AutoInsertNewRow>true</AutoInsertNewRow>\n          <ChangeRowSet>false</ChangeRowSet>\n          <ChangePositionOfColumns>false</ChangePositionOfColumns>\n          <ChangeSettingOfColumns>false</ChangeSettingOfColumns>\n          <HorizontalScrollBar>DontUse</HorizontalScrollBar>\n          <VerticalScrollBar>Use</VerticalScrollBar>\n          <InitialListView>End</InitialListView>\n          <Output>Enable</Output>",
        )],
    );
    // 2. Add a new Table from donor with custom flags
    let added = "<Table name=\"ТаблицаНовая\" id=\"999\">\n          <Left>500</Left><Top>500</Top><Width>200</Width><Height>100</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n          <Footer>true</Footer>\n          <Header>false</Header>\n          <ReadOnly>true</ReadOnly>\n          <ChangeRowSet>false</ChangeRowSet>\n          <InitialListView>Auto</InitialListView>\n          <Output>Disable</Output>\n        </Table>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(added.to_string() + anchor), 1);

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let expected_flags = (117644301i64 | 2 | 32768) & !1 & !4 & !8 & !1024 & !33554432 & !67108864;
    let text = String::from_utf8(pf.bytes).unwrap();
    assert!(text.contains(&format!("{{19,{expected_flags},")), "coalesced table bitmask not patched: expected {expected_flags}");
}

#[test]
fn extended_catalog_properties_across_priority_and_secondary_controls_round_trip() {
    let mut edited = baseline("DonorForm");
    // 1. Button (Кнопка1): HorizontalAlign, VerticalAlign, PictureLocation, PictureSize, MultiLine, ButtonBackColor, ButtonTextColor
    edited = edit_ctl(
        &edited,
        "Кнопка1",
        &[(
            "<TabOrder>2</TabOrder>",
            "<TabOrder>2</TabOrder>\n          <ButtonBackColor kind=\"rgb\" value=\"#FF0000\"/>\n          <ButtonTextColor kind=\"rgb\" value=\"#00FF00\"/>\n          <HorizontalAlign>Left</HorizontalAlign>\n          <VerticalAlign>Top</VerticalAlign>\n          <PictureLocation>Right</PictureLocation>\n          <PictureSize>Stretch</PictureSize>\n          <MultiLine>true</MultiLine>",
        )],
    );
    // 2. LabelDecoration (Надпись1): VerticalAlign, MarkNegatives, Format, PictureSize
    edited = edit_ctl(
        &edited,
        "Надпись1",
        &[(
            "<TabOrder>3</TabOrder>",
            "<TabOrder>3</TabOrder>\n          <VerticalAlign>Bottom</VerticalAlign>\n          <MarkNegatives>true</MarkNegatives>\n          <PictureSize>Stretch</PictureSize>\n          <Format>\n            <Item language=\"ru\">ЧДЦ=2</Item>\n          </Format>",
        )],
    );
    // 3. CheckBox (Флажок1): HorizontalAlign, VerticalAlign, FieldBackColor, FieldTextColor
    edited = edit_ctl(
        &edited,
        "Флажок1",
        &[(
            "<TabOrder>5</TabOrder>",
            "<TabOrder>5</TabOrder>\n          <FieldBackColor kind=\"rgb\" value=\"#112233\"/>\n          <FieldTextColor kind=\"rgb\" value=\"#445566\"/>\n          <HorizontalAlign>Center</HorizontalAlign>\n          <VerticalAlign>Bottom</VerticalAlign>",
        )],
    );
    // 4. RadioButton (Переключатель1): TitleLocation, HorizontalAlign, VerticalAlign, ValueType, SelectionValue
    edited = edit_ctl(
        &edited,
        "Переключатель1",
        &[(
            "<TabOrder>10</TabOrder>",
            "<TabOrder>10</TabOrder>\n          <ValueType kind=\"Number\" composite=\"false\" fractionDigits=\"0\"/>\n          <TitleLocation>Left</TitleLocation>\n          <HorizontalAlign>Right</HorizontalAlign>\n          <VerticalAlign>Top</VerticalAlign>\n          <SelectionValue>1</SelectionValue>",
        )],
    );
    // 5. InputField (ПолеВвода2): MinValue, MaxValue, ChoiceListWidth, Format, FieldBackColor, FieldTextColor
    edited = edit_ctl(
        &edited,
        "ПолеВвода2",
        &[(
            "<OpenButton>false</OpenButton>",
            "<OpenButton>false</OpenButton>\n          <FieldBackColor kind=\"rgb\" value=\"#FF0000\"/>\n          <FieldTextColor kind=\"rgb\" value=\"#00FF00\"/>\n          <MinValue>5</MinValue>\n          <MaxValue>95</MaxValue>\n          <ChoiceListWidth>25</ChoiceListWidth>\n          <Format>\n            <Item language=\"ru\">ЧГ=0</Item>\n          </Format>",
        )],
    );
    // 6. Panel (Панель1): SeparateByPage, AutoTabOrder, ScrollPageMode
    edited = edit_ctl(
        &edited,
        "Панель1",
        &[(
            "<ShowTabs type=\"ShowTabs\" member=\"Unknown\" code=\"2\"/>",
            "<ShowTabs type=\"ShowTabs\" member=\"Unknown\" code=\"2\"/>\n          <SeparateByPage>false</SeparateByPage>\n          <AutoTabOrder>false</AutoTabOrder>\n          <ScrollPageMode>true</ScrollPageMode>",
        )],
    );
    // 7. CommandBar (КоманднаяПанель2): Orientation, Secondary
    edited = edit_ctl(
        &edited,
        "КоманднаяПанель2",
        &[(
            "<AutoFill>false</AutoFill>",
            "<AutoFill>false</AutoFill>\n          <Orientation>Vertical</Orientation>\n          <Secondary>false</Secondary>",
        )],
    );
    // 8. PictureDecoration (ПолеКартинки1), ListBox (ПолеСписка1), TrackBar (ПолосаРегулирования1), ProgressBar (Индикатор1), Splitter (Разделитель1), CalendarField (ПолеКалендаря1)
    edited = edit_ctl(
        &edited,
        "ПолеКартинки1",
        &[(
            "<TabOrder>13</TabOrder>",
            "<TabOrder>13</TabOrder>\n          <Zoomable>false</Zoomable>\n          <UseContextMenu>false</UseContextMenu>\n          <EnableStartDrag>true</EnableStartDrag>\n          <EnableDrag>true</EnableDrag>\n          <NonselectedPictureText>\n            <Item language=\"ru\">Нет картинки</Item>\n          </NonselectedPictureText>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеСписка1",
        &[(
            "<TabOrder>11</TabOrder>",
            "<TabOrder>11</TabOrder>\n          <ShowPicture>true</ShowPicture>\n          <ShowCheckBox>true</ShowCheckBox>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолосаРегулирования1",
        &[(
            "<TabOrder>12</TabOrder>",
            "<TabOrder>12</TabOrder>\n          <MinValue>10</MinValue>\n          <MaxValue>80</MaxValue>\n          <Step>2</Step>\n          <LargeStep>15</LargeStep>\n          <Orientation>Vertical</Orientation>\n          <MarkingAppearance>DontShow</MarkingAppearance>\n          <MarkingStep>8</MarkingStep>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "Индикатор1",
        &[(
            "<TabOrder>16</TabOrder>",
            "<TabOrder>16</TabOrder>\n          <MinValue>5</MinValue>\n          <MaxValue>90</MaxValue>\n          <Step>5</Step>\n          <ShowStyle>Broken</ShowStyle>\n          <ShowPercent>true</ShowPercent>\n          <Orientation>Vertical</Orientation>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "Разделитель1",
        &[(
            "<TabOrder>6</TabOrder>",
            "<TabOrder>6</TabOrder>\n          <Orientation>Vertical</Orientation>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеКалендаря1",
        &[(
            "<TabOrder>7</TabOrder>",
            "<TabOrder>7</TabOrder>\n          <SelectionBackColor kind=\"rgb\" value=\"#FF0000\"/>\n          <SelectionTextColor kind=\"rgb\" value=\"#00FF00\"/>\n          <HolidayColor kind=\"rgb\" value=\"#0000FF\"/>\n          <ShowCurrentDate>false</ShowCurrentDate>\n          <CalendarNavigation>false</CalendarNavigation>\n          <SelectionMode>Multi</SelectionMode>\n          <EnableStartDrag>true</EnableStartDrag>\n          <EnableDrag>true</EnableDrag>",
        )],
    );
    // 9. Added ChoiceField with ReadOnly, ChoiceButton, ClearButton, SpinButton, OpenButton, ChoiceListWidth
    let added = "<ChoiceField name=\"ПолеВыбораНовое\" id=\"999\">\n          <Left>500</Left><Top>500</Top><Width>150</Width><Height>20</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n          <ReadOnly>true</ReadOnly>\n          <ChoiceButton>true</ChoiceButton>\n          <ClearButton>true</ClearButton>\n          <SpinButton>true</SpinButton>\n          <OpenButton>true</OpenButton>\n          <ChoiceListWidth>30</ChoiceListWidth>\n        </ChoiceField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(added.to_string() + anchor), 1);

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("{\"N\",1}"), "RadioButton SelectionValue not patched");
    assert!(compact.contains("{\"N\",5},{\"N\",95}"), "InputField MinValue/MaxValue not patched");
    assert!(text.contains("\"Нет картинки\""), "PictureDecoration NonselectedPictureText not patched");
}

#[test]
fn table_and_column_catalog_properties_round_trip() {
    let mut edited = baseline("DonorForm");
    // Patch Table (ТабличноеПоле1) and its Column (Колонка1 & Колонка2)
    edited = edit_ctl(
        &edited,
        "ТабличноеПоле1",
        &[
            (
                "<Footer>false</Footer>",
                "<Footer>false</Footer>\n          <UseAlternationRowColor>true</UseAlternationRowColor>\n          <EnableStartDrag>true</EnableStartDrag>\n          <HeaderBackColor kind=\"rgb\" value=\"#112233\"/>\n          <HeaderTextColor kind=\"rgb\" value=\"#223344\"/>\n          <FooterBackColor kind=\"rgb\" value=\"#334455\"/>\n          <FooterTextColor kind=\"rgb\" value=\"#445566\"/>\n          <SelectionBackColor kind=\"rgb\" value=\"#556677\"/>\n          <SelectionTextColor kind=\"rgb\" value=\"#667788\"/>\n          <AlternationRowBackColor kind=\"rgb\" value=\"#778899\"/>\n          <FixedLeft>1</FixedLeft>\n          <FixedRight>2</FixedRight>\n          <RowInputMode>AtWindowBottom</RowInputMode>\n          <RowSelectionMode>Row</RowSelectionMode>\n          <HeaderHeight>2</HeaderHeight>\n          <FooterHeight>3</FooterHeight>\n          <HeaderFont bold=\"true\" source=\"bin\" face=\"Arial\" size=\"10\"/>\n          <FooterFont bold=\"true\" source=\"bin\" face=\"Tahoma\" size=\"9\"/>",
            ),
            (
                "<Editor>InputField</Editor>\n              <Events>",
                "<Editor>InputField</Editor>\n              <ShowHierarchy>true</ShowHierarchy>\n              <CheckBoxThreeState>true</CheckBoxThreeState>\n              <ShowInHeader>false</ShowInHeader>\n              <ShowInFooter>false</ShowInFooter>\n              <ChangeSetting>false</ChangeSetting>\n              <ChangePosition>false</ChangePosition>\n              <FieldBackColor kind=\"rgb\" value=\"#AABBCC\"/>\n              <FieldTextColor kind=\"rgb\" value=\"#BBCCDD\"/>\n              <HeaderBackColor kind=\"rgb\" value=\"#CCDDEE\"/>\n              <HeaderTextColor kind=\"rgb\" value=\"#DDEEFF\"/>\n              <FooterBackColor kind=\"rgb\" value=\"#EEFF00\"/>\n              <FooterTextColor kind=\"rgb\" value=\"#FF0011\"/>\n              <TextFont bold=\"true\" source=\"bin\" face=\"Verdana\" size=\"8\"/>\n              <HeaderFont bold=\"true\" source=\"bin\" face=\"Courier\" size=\"10\"/>\n              <FooterFont bold=\"true\" source=\"bin\" face=\"Georgia\" size=\"9\"/>\n              <HeaderHorizontalAlign>Center</HeaderHorizontalAlign>\n              <HorizontalAlignInColumn>Right</HorizontalAlignInColumn>\n              <FooterHorizontalAlign>Center</FooterHorizontalAlign>\n              <CellHeight>4</CellHeight>\n              <Format>\n                <Item language=\"ru\">ЧДЦ=2</Item>\n              </Format>\n              <Events>",
            ),
        ],
    );

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    let expected_tbl_flags = 117644301i64 | 16 | 16384;
    let expected_col_flags = (12590592i64 | 1 | 8) & !2048 & !4096 & !4194304 & !8388608;
    assert!(text.contains(&format!("{{19,{expected_tbl_flags},")), "Table bitmask not patched: {expected_tbl_flags}");
    assert!(text.contains(&format!(",{expected_col_flags},")), "Column bitmask not patched: {expected_col_flags}");
}

#[test]
fn universal_border_style_and_document_schema_field_properties_round_trip() {
    let mut edited = baseline("DonorForm");
    edited = edit_ctl(
        &edited,
        "Кнопка1",
        &[(
            "<TabOrder>2</TabOrder>",
            "<TabOrder>2</TabOrder>\n          <BorderStyle code=\"4\">SingleLine</BorderStyle>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеТабличногоДокумента1",
        &[(
            "<TabOrder>19</TabOrder>",
            "<TabOrder>19</TabOrder>\n          <BorderColor kind=\"rgb\" value=\"#FF0000\"/>\n          <BorderStyle code=\"4\">SingleLine</BorderStyle>\n          <HorizontalScrollBar>DontUse</HorizontalScrollBar>\n          <VerticalScrollBar>DontUse</VerticalScrollBar>\n          <EnableStartDrag>false</EnableStartDrag>\n          <EnableDrag>false</EnableDrag>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеСписка1",
        &[(
            "<TabOrder>11</TabOrder>",
            "<TabOrder>11</TabOrder>\n          <BorderStyle code=\"0\">None</BorderStyle>\n          <ReadOnly>true</ReadOnly>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеТекстовогоДокумента1",
        &[(
            "<TabOrder>8</TabOrder>",
            "<TabOrder>8</TabOrder>\n          <BorderStyle code=\"4\">SingleLine</BorderStyle>\n          <ReadOnly>true</ReadOnly>\n          <Output>Enable</Output>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеHTMLДокумента1",
        &[(
            "<TabOrder>18</TabOrder>",
            "<TabOrder>18</TabOrder>\n          <BorderColor kind=\"rgb\" value=\"#00FF00\"/>\n          <BorderStyle code=\"6\">Frame</BorderStyle>\n          <Output>Disable</Output>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеГрафическойСхемы1",
        &[(
            "<TabOrder>1</TabOrder>",
            "<TabOrder>1</TabOrder>\n          <BorderStyle code=\"0\">None</BorderStyle>\n          <ReadOnly>true</ReadOnly>\n          <Output>Enable</Output>",
        )],
    );

    let _ = structural_round_trip("DonorForm", &edited);
}

#[test]
fn font_shortcut_picture_and_remaining_control_slots_round_trip() {
    let mut edited = baseline("DonorForm");
    edited = edit_ctl(
        &edited,
        "Кнопка1",
        &[(
            "<TabOrder>2</TabOrder>",
            "<TabOrder>2</TabOrder>\n          <Font bold=\"true\" italic=\"true\" underline=\"true\" strikeout=\"true\" scale=\"120\" face=\"Arial\" size=\"11\"/>\n          <MenuMode>false</MenuMode>\n          <Shortcut key=\"3\" shift=\"true\" ctrl=\"true\"/>\n          <Picture kind=\"1\" refGuid=\"785362cb-3756-48ed-87d2-292ded17054a\"/>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "Надпись1",
        &[(
            "<TabOrder>3</TabOrder>",
            "<TabOrder>3</TabOrder>\n          <Shortcut key=\"4\" alt=\"true\"/>\n          <ScrollingText>Slow</ScrollingText>\n          <Picture kind=\"2\" stdId=\"42\"/>\n          <LabelPictureLocation>Right</LabelPictureLocation>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "Панель1",
        &[(
            "<TabOrder>15</TabOrder>",
            "<TabOrder>15</TabOrder>\n          <Picture kind=\"1\" refGuid=\"785362cb-3756-48ed-87d2-292ded17054a\"/>\n          <PictureSize>Proportionally</PictureSize>\n          <PanelPictureLocation>TopLeft</PanelPictureLocation>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеКартинки1",
        &[(
            "<TabOrder>13</TabOrder>",
            "<TabOrder>13</TabOrder>\n          <PictureSize>AutoSize</PictureSize>\n          <Hyperlink>true</Hyperlink>\n          <Picture kind=\"2\" stdId=\"7\"/>\n          <Shortcut key=\"5\"/>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеВвода2",
        &[(
            "<OpenButton>false</OpenButton>",
            "<OpenButton>false</OpenButton>\n          <ChoiceListHeight>12</ChoiceListHeight>\n          <Picture kind=\"2\" stdId=\"9\"/>\n          <ChoiceButtonPicture kind=\"2\" stdId=\"10\"/>\n          <Shortcut key=\"2\" ctrl=\"true\"/>",
        )],
    );
    let added = "<ChoiceField name=\"ПолеВыбораНовое\" id=\"999\">\n          <Left>500</Left><Top>500</Top><Width>150</Width><Height>20</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n          <ChoiceListHeight>15</ChoiceListHeight>\n          <Picture kind=\"0\"/>\n          <ChoiceButtonPicture kind=\"1\" refGuid=\"785362cb-3756-48ed-87d2-292ded17054a\"/>\n          <Shortcut key=\"6\" mods=\"3\"/>\n        </ChoiceField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(added.to_string() + anchor), 1);
    edited = edit_ctl(
        &edited,
        "ПолеТабличногоДокумента1",
        &[(
            "<TabOrder>19</TabOrder>",
            "<TabOrder>19</TabOrder>\n          <ShowSelection>Always</ShowSelection>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеКалендаря1",
        &[(
            "<TabOrder>7</TabOrder>",
            "<TabOrder>7</TabOrder>\n          <BeginOfDisplayPeriod>20260101000000</BeginOfDisplayPeriod>\n          <EndOfDisplayPeriod>20261231000000</EndOfDisplayPeriod>",
        )],
    );

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    assert!(text.contains("{7,1,60,\"Arial\",{-22},700,1,1,1,0,120}"));
    assert!(text.contains("{3,3,0}"));
    assert!(text.contains("{4,1,{0,785362cb-3756-48ed-87d2-292ded17054a},\"\",-1,-1,1,0,\"\"}"));
    assert!(text.contains("20260101000000,20261231000000"));
}

#[test]
fn form_resize_and_qualifier_flags_round_trip() {
    let mut edited = baseline("DonorForm");
    edited = edited.replacen(
        "<Width>937</Width>\n  <Height>693</Height>",
        "<Width>1020</Width>\n  <Height>760</Height>",
        1,
    );
    edited = edit_ctl(
        &edited,
        "ПолеВвода2",
        &[(
            "<ValueType kind=\"Number\" composite=\"false\" fractionDigits=\"0\"/>",
            "<ValueType kind=\"Number\" composite=\"false\" digits=\"12\" fractionDigits=\"2\" nonNegative=\"true\"/>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеВвода7",
        &[(
            "<ValueType kind=\"String\" composite=\"false\"/>",
            "<ValueType kind=\"String\" composite=\"false\" length=\"25\" fixed=\"true\"/>",
        )],
    );

    let pf = round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("{\"N\",12,2,1}"), "Number nonNegative not patched");
    assert!(compact.contains("{\"S\",25,1}"), "String fixed not patched");
}

#[test]
fn remaining_table_column_page_button_and_form_root_properties_round_trip() {
    let mut edited = baseline("DonorForm");
    edited = edited.replacen(
        "<Form name=\"ФормаДокумента\">",
        "<Form name=\"ФормаДокумента\">\n  <AutoTitle>false</AutoTitle>\n  <WindowState>Float</WindowState>\n  <SizeChange>DontChange</SizeChange>\n  <ConnectableWindow>true</ConnectableWindow>\n  <AllowJoinWindow>true</AllowJoinWindow>\n  <CaptionPicture kind=\"1\" refGuid=\"785362cb-3756-48ed-87d2-292ded17054a\"/>\n  <WindowAppearanceMode>Maximized</WindowAppearanceMode>\n  <WindowLocation>Center</WindowLocation>\n  <ChangeWindowAppearanceMode>Allow</ChangeWindowAppearanceMode>\n  <DesktopMode>true</DesktopMode>\n  <AllowClose>false</AllowClose>\n  <EnterKeyBehavior>DefaultButton</EnterKeyBehavior>\n  <AllowNormalState>false</AllowNormalState>\n  <AllowDockedState>true</AllowDockedState>\n  <AllowAutohideState>false</AllowAutohideState>\n  <AllowFloatState>true</AllowFloatState>\n  <WindowDockLocation>2</WindowDockLocation>",
        1,
    );
    edited = edit_ctl(
        &edited,
        "ТабличноеПоле1",
        &[
            (
                "<TabOrder>9</TabOrder>",
                "<TabOrder>9</TabOrder>\n          <EnableDrag>true</EnableDrag>\n          <ChoiceMode>true</ChoiceMode>",
            ),
            (
                "<Editor>InputField</Editor>\n              <Events>",
                "<Editor>InputField</Editor>\n              <MarkNegatives>true</MarkNegatives>\n              <ChangeVisible>false</ChangeVisible>\n              <AutoMarkIncomplete>true</AutoMarkIncomplete>\n              <AutoCellHeight>true</AutoCellHeight>\n              <Hyperlink>true</Hyperlink>\n              <CheckData>2</CheckData>\n              <RowsPictures kind=\"2\" stdId=\"11\"/>\n              <HeaderPicture kind=\"1\" refGuid=\"785362cb-3756-48ed-87d2-292ded17054a\"/>\n              <FooterPicture kind=\"2\" stdId=\"12\"/>\n              <Location>OnNextLine</Location>\n              <SizeChange>DontChange</SizeChange>\n              <EditMode>Directly</EditMode>\n              <Events>",
            ),
        ],
    );
    edited = edit_ctl(
        &edited,
        "Панель1",
        &[(
            "<Page name=\"Страница1\">\n              <Title>",
            "<Page name=\"Страница1\">\n              <TitlePicture kind=\"2\" stdId=\"5\"/>\n              <Title>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "КоманднаяПанель2",
        &[(
            "<Button name=\"НоваяКнопкаНаПанели\" id=\"1\" kind=\"Action\" checked=\"false\" actionKind=\"Handler\" action=\"КоманднаяПанель2НоваяКнопкаНаПанели\">",
            "<Button name=\"НоваяКнопкаНаПанели\" id=\"1\" kind=\"Action\" checked=\"false\" actionKind=\"Handler\" action=\"КоманднаяПанель2НоваяКнопкаНаПанели\">\n              <Picture kind=\"2\" stdId=\"8\"/>\n              <Shortcut key=\"3\" shift=\"true\"/>\n              <DefaultButton>true</DefaultButton>\n              <ButtonOrder>Ascending</ButtonOrder>\n              <Description>\n                <Item language=\"ru\">Пояснение кнопки</Item>\n                <Item language=\"ro\">Пояснение кнопки</Item>\n              </Description>",
        )],
    );

    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let text = String::from_utf8(pf.bytes).unwrap();
    let expected_tbl_flags = 117644301i64 | (1 << 9) | (1 << 20);
    let expected_col_flags = 12590592i64 | (1 << 15) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21);
    assert!(text.contains(&format!("{{19,{expected_tbl_flags},")));
    assert!(text.contains(&format!(",{expected_col_flags},")));
}

#[test]
fn final_catalog_extensions_choiceitem_and_spreadsheet_doc_round_trip() {
    const STYLE: &str = "6b2c1a56-4e1f-4f27-8d93-2f0b5f4e9a10";
    let mut edited = baseline("DonorForm");
    edited = replace_once(
        &edited,
        "reconstructionComplete=\"false\">",
        &format!("reconstructionComplete=\"false\">\n  <Style ref=\"{STYLE}\"/>\n  <DefaultControlId>4</DefaultControlId>"),
    );
    edited = edit_ctl(
        &edited,
        "Кнопка1",
        &[("<TabOrder>2</TabOrder>", "<TabOrder>2</TabOrder>\n          <DefaultButton>true</DefaultButton>\n          <ContextMenu>12</ContextMenu>")],
    );
    edited = edit_ctl(
        &edited,
        "КоманднаяПанель1",
        &[("<TabOrder>1</TabOrder>", "<TabOrder>1</TabOrder>\n          <ActionSource controlId=\"6\"/>")],
    );
    edited = edit_ctl(
        &edited,
        "Надпись1",
        &[(
            "<TabOrder>3</TabOrder>",
            "<TabOrder>3</TabOrder>\n          <ContextMenu controlId=\"7\"/>\n          <AutoContextMenu>false</AutoContextMenu>\n          <Collapse>Left</Collapse>\n          <EmbeddedIntoCell>true</EmbeddedIntoCell>\n          <Print>false</Print>",
        )],
    );
    edited = edit_ctl(
        &edited,
        "ПолеТабличногоДокумента1",
        &[(
            "<TabOrder>19</TabOrder>",
            "<TabOrder>19</TabOrder>\n          <ReadOnly>true</ReadOnly>\n          <Protection>true</Protection>\n          <ShowGrid>false</ShowGrid>\n          <ShowHeaders>false</ShowHeaders>\n          <ShowGroups>false</ShowGroups>\n          <BlackAndWhiteView>true</BlackAndWhiteView>\n          <TotalsRight>true</TotalsRight>\n          <TotalsBelow>true</TotalsBelow>\n          <PrintScale>90</PrintScale>\n          <FitToPage>true</FitToPage>\n          <BlackAndWhite>true</BlackAndWhite>\n          <Collate>true</Collate>\n          <Copies>3</Copies>\n          <PerPage>2</PerPage>\n          <PageOrientation>Landscape</PageOrientation>\n          <BottomMargin>11</BottomMargin>\n          <TopMargin>12</TopMargin>\n          <LeftMargin>13</LeftMargin>\n          <RightMargin>14</RightMargin>\n          <HeaderSize>15</HeaderSize>\n          <FooterSize>16</FooterSize>\n          <DuplexPrinting>1</DuplexPrinting>\n          <PrintAccuracy>300</PrintAccuracy>\n          <StepDirection>1</StepDirection>\n          <SaveViewProperties>true</SaveViewProperties>\n          <FixedBackground>true</FixedBackground>",
        )],
    );
    let added = "<ChoiceField name=\"ПолеВыбораСПометкой\" id=\"999\">\n          <Left>520</Left><Top>520</Top><Width>140</Width><Height>20</Height>\n          <ZOrder>58</ZOrder><TabOrder>58</TabOrder>\n          <ChoiceList>\n            <ChoiceItem value=\"Зн1\" checked=\"true\" pictureId=\"7\"/>\n            <ChoiceItem value=\"Зн2\" checked=\"false\"/>\n          </ChoiceList>\n        </ChoiceField>\n        ";
    let anchor = "        <Table name=\"ТабличноеПоле1\"";
    edited = edited.replacen(anchor, &(added.to_string() + anchor), 1);

    let (pf, got) = structural_round_trip("DonorForm", &edited);
    let s = parse_spanned(&pf.bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let int = |n: &oneform::brace::Node, path: &[usize]| -> i64 {
        let (last, parents) = path.split_last().unwrap();
        let mut cur = n;
        for &i in parents {
            cur = cur.node(i).unwrap();
        }
        cur.atom(*last).unwrap().value.parse().unwrap()
    };
    let info = |id: i64| idx.get(id).unwrap().info().unwrap().node;
    let meta = |id: i64| idx.get(id).unwrap().meta().unwrap().node;
    // DefaultButton: the button flag and the coupled form header slot.
    assert_eq!(int(info(4), &[1, 5]), 1);
    assert_eq!(int(&s.root, &[1, 1, 2]), 4);
    assert_eq!(int(meta(4), &[2]), 12);
    assert_eq!(int(meta(2), &[2]), 6);
    assert_eq!(int(meta(5), &[2]), 7);
    // AutoContextMenu / Collapse / EmbeddedIntoCell / Print live in the appearance block.
    assert_eq!(int(info(5), &[1, 0, 16]), 2);
    assert_eq!(int(info(5), &[1, 0, 14]), 3);
    assert_eq!(int(info(5), &[1, 0, 13]), 1);
    assert_eq!(int(info(5), &[1, 0, 17]), 0);
    // Form style and default control.
    let text = String::from_utf8(pf.bytes.clone()).unwrap();
    assert_eq!(s.root.node(12).unwrap().atom(1).unwrap().value, STYLE);
    let panel = s.root.node(1).unwrap().node(2).unwrap().node(1).unwrap().node(1).unwrap();
    assert!(panel.children.iter().any(|c| c.as_atom().is_some_and(|a| a.value == "4")));
    // SpreadsheetDocumentField view and print properties.
    let sheet = info(15);
    for (path, want) in [
        (&[12usize][..], 1),
        (&[13], 1),
        (&[18], 0),
        (&[19], 0),
        (&[20], 0),
        (&[21], 1),
        (&[22], 1),
        (&[23], 1),
        (&[14, 3], 90),
        (&[14, 4], 1),
        (&[14, 5], 1),
        (&[14, 6], 1),
        (&[14, 7], 3),
        (&[14, 8], 2),
        (&[14, 9], 1),
        (&[14, 10], 11),
        (&[14, 11], 12),
        (&[14, 12], 13),
        (&[14, 13], 14),
        (&[14, 14], 15),
        (&[14, 15], 16),
        (&[14, 16], 1),
        (&[14, 17], 300),
        (&[14, 18], 1),
        (&[14, 19], 1),
        (&[14, 20], 1),
    ] {
        assert_eq!(int(sheet, path), want, "SpreadsheetDocumentField info{path:?}");
    }
    // ChoiceItem checked / pictureId.
    assert!(text.contains("{0,7,1,{\"S\",\"Зн1\"}"));
    assert!(text.contains("{0,0,0,{\"S\",\"Зн2\"}"));
    assert_eq!(got.control(pf.allocated[0].1).unwrap().values.choice_list.len(), 2);
}

/// Editing one attribute of a compound value must keep the parts of it that the XML does not state.
#[test]
fn font_size_edit_keeps_italic_underline_strikeout_and_scale() {
    let name = "DonorForm";
    let mut first = baseline(name);
    first = edit_ctl(
        &first,
        "Кнопка1",
        &[(
            "<TabOrder>2</TabOrder>",
            "<TabOrder>2</TabOrder>\n          <Font bold=\"true\" italic=\"true\" underline=\"true\" strikeout=\"true\" scale=\"120\" face=\"Arial\" size=\"11\"/>",
        )],
    );
    let (pf, _) = structural_round_trip(name, &first);
    // the next edit starts from what `extract` shows for the patched stream
    let s2 = parse_spanned(&pf.bytes).unwrap();
    let base2 = decode_xml(name, &pf.bytes);
    let font_line = base2.lines().find(|l| l.contains("<Font ") && l.contains("Arial")).expect("font in extract").to_string();
    let edited = base2.replacen(&font_line, &font_line.replace("size=\"11\"", "size=\"13\""), 1);
    assert_ne!(edited, base2);
    let pf2 = patch_form(&s2, base2.as_bytes(), edited.as_bytes()).unwrap();
    let text = String::from_utf8(pf2.bytes).unwrap();
    assert!(text.contains("{7,1,60,\"Arial\",{-26},700,1,1,1,0,120}"), "font flags lost: {}", &text[text.find("Arial").unwrap_or(0).saturating_sub(20)..][..60]);
}

// ---------------------------------------------------------------- anchor graph of added controls

type InList = [Vec<(i64, i64)>; 6];

/// The six counted incoming lists starting at `start` of `node`.
fn read_in_lists(node: &oneform::brace::Node, start: usize) -> InList {
    let mut i = start;
    let mut out: InList = Default::default();
    for list in &mut out {
        let n: usize = node.atom(i).unwrap().value.parse().unwrap();
        for j in 0..n {
            let e = node.node(i + 1 + j).unwrap();
            list.push((e.atom(1).unwrap().value.parse().unwrap(), e.atom(2).unwrap().value.parse().unwrap()));
        }
        i += 1 + n;
    }
    out
}

/// Incoming lists in the geometry node of control `id`.
fn geom_lists(bytes: &[u8], id: i64) -> InList {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    read_in_lists(idx.get(id).unwrap().geom().unwrap().node, 12)
}

/// Incoming lists of the container panel: the root panel (`None`) or the Panel control `id`.
fn panel_lists(bytes: &[u8], id: Option<i64>) -> InList {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let params = match id {
        None => idx.root_panel().unwrap().child(1).unwrap().child(1).unwrap(),
        Some(id) => idx.get(id).unwrap().info().unwrap().child(1).unwrap(),
    };
    read_in_lists(params.node, 2)
}

fn violations(bytes: &[u8]) -> Vec<String> {
    anchor_graph_violations(&parse_spanned(bytes).unwrap())
}

fn label_xml(name: &str, id: i64, geometry: &str, anchors: &str) -> String {
    format!("<LabelDecoration name=\"{name}\" id=\"{id}\">\n          {geometry}\n          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>\n          {anchors}\n        </LabelDecoration>\n        ")
}

fn insert_before_table1(xml: &str, block: &str) -> String {
    xml.replacen("        <Table name=\"ТабличноеПоле1\"", &format!("{block}        <Table name=\"ТабличноеПоле1\""), 1)
}

#[test]
fn an_added_control_with_unsupported_content_is_refused_not_silently_trimmed() {
    // `Object.Field` data paths and unknown elements cannot be written into a clone: both used to vanish.
    let geometry = "<Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>";
    for extra in ["<DataPath attributeId=\"3\">ПолеВвода1.Поле</DataPath>", "<NoSuchProperty>1</NoSuchProperty>"] {
        let block = format!(
            "<LabelDecoration name=\"Новая\" id=\"999\">
          {geometry}
          {extra}
          <ZOrder>50</ZOrder><TabOrder>50</TabOrder>
        </LabelDecoration>
        "
        );
        let edited = insert_before_table1(&baseline("DonorForm"), &block);
        let e = patch_form(&stream("DonorForm"), baseline("DonorForm").as_bytes(), edited.as_bytes()).unwrap_err();
        assert!(e.to_string().contains("added-control") || format!("{e:?}").contains("added-control"), "{extra}: {e:?}");
    }
}

#[test]
fn fixtures_have_consistent_anchor_graphs() {
    for name in ["DonorForm", "DonorFormAlt", "DonorFormV2"] {
        let s = stream(name);
        assert_eq!(anchor_graph_violations(&s), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn added_control_without_anchors_gets_designer_defaults_and_list_entries() {
    let before = panel_lists(stream("DonorForm").source(), None);
    let edited = added_label(None);
    let (pf, got) = structural_round_trip("DonorForm", &edited);
    let id = pf.allocated[0].1;
    assert_eq!(violations(&pf.bytes), Vec::<String>::new());
    let (top, left, w, h) = (500, 500, 80, 20);
    let a = got.control(id).unwrap().values.anchors.clone().expect("decoded anchors");
    let at = |target, side, offset| Some(oneform::form_patch::AnchorEdge { target, side, offset });
    // the label hangs below the nearest control above it (id 13, a panel ending at 377) and has no left neighbour
    assert_eq!((a.top, a.bottom, a.left, a.right), (at(13, 1, top - 377), at(id, 0, h), None, at(id, 2, w)));
    assert_eq!(a.alt, [None; 4]);
    assert_eq!(anchor_edges(&pf.bytes, id), [Some((13, 1, top - 377)), Some((id, 0, h)), Some((-1, 6, 0)), Some((id, 2, w))]);
    let _ = left;
    // nothing is attached to the form itself, the neighbour's list takes the top edge
    assert_eq!(panel_lists(&pf.bytes, None), before);
    assert_eq!(geom_lists(&pf.bytes, 13)[1].last(), Some(&(id, 0)));
    let own = geom_lists(&pf.bytes, id);
    assert_eq!((&own[0], &own[2]), (&vec![(id, 1)], &vec![(id, 3)]));
    assert!(own[1].is_empty() && own[3].is_empty() && own[4].is_empty() && own[5].is_empty());
}

/// The new label (500,500 80x20) of `added_label`-like XML with panel 13 edited by `edit`; its resolved top edge.
fn top_edge_of_label_next_to_edited_panel(from: &str, to: &str, anchors: &str) -> Option<(i64, i64, i64)> {
    let moved = mutate_in("<Panel name=\"Панель1\" id=\"13\">", from, to);
    let geometry = "<Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>";
    let edited = insert_before_table1(&moved, &label_xml("Добавленная", 999, geometry, anchors));
    let (pf, _) = structural_round_trip("DonorForm", &edited);
    assert_eq!(violations(&pf.bytes), Vec::<String>::new());
    anchor_edges(&pf.bytes, pf.allocated[0].1)[0]
}

#[test]
fn automatic_anchors_of_an_added_control_use_the_edited_geometry_of_existing_controls() {
    // the panel (id 13, bottom 377) is the control above the new label in the stream; the same edit moves it below
    // the label, so the label hangs below the next control above (the list box id 10, bottom 218)
    assert_eq!(top_edge_of_label_next_to_edited_panel("<Top>223</Top>", "<Top>600</Top>", ""), Some((10, 1, 500 - 218)));
    // the panel grows: it still is above the label, the gap is measured from its new bottom (223 + 200)
    assert_eq!(top_edge_of_label_next_to_edited_panel("<Height>154</Height>", "<Height>200</Height>", ""), Some((13, 1, 500 - 423)));
    // a shift of the panel that keeps it above the label changes nothing
    assert_eq!(top_edge_of_label_next_to_edited_panel("<Left>420</Left>", "<Left>421</Left>", ""), Some((13, 1, 500 - 377)));
    // explicit anchors of the new control still win
    let explicit = "<Anchors top=\"Top\" topOffset=\"500\"/>";
    assert_eq!(top_edge_of_label_next_to_edited_panel("<Top>223</Top>", "<Top>600</Top>", explicit), Some((0, 0, 500)));
}

#[test]
fn explicit_anchors_to_an_existing_control_update_its_list() {
    let before = geom_lists(stream("DonorForm").source(), 7);
    let anchors = "<Anchors left=\"Left\" leftOffset=\"12\" top=\"Bottom\" topTarget=\"7\" topOffset=\"5\"/>";
    let geometry = "<Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>";
    let edited = insert_before_table1(&baseline("DonorForm"), &label_xml("Добавленная", 999, geometry, anchors));
    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let id = pf.allocated[0].1;
    assert_eq!(violations(&pf.bytes), Vec::<String>::new());
    assert_eq!(anchor_edges(&pf.bytes, id), [Some((7, 1, 5)), Some((id, 0, 20)), Some((0, 2, 12)), Some((id, 2, 80))]);
    let after = geom_lists(&pf.bytes, 7);
    let mut want = before.clone();
    want[1].push((id, 0));
    assert_eq!(after, want);
    // the form is not touched elsewhere: left edge goes to the root panel list 2
    assert_eq!(panel_lists(&pf.bytes, None)[2].last(), Some(&(id, 2)));
}

#[test]
fn unknown_or_new_anchor_targets_are_rejected() {
    let geometry = "<Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>";
    for (anchors, needle) in [
        ("<Anchors top=\"Bottom\" topTarget=\"4242\" topOffset=\"5\"/>", "does not exist"),
        ("<Anchors top=\"Bottom\" topTarget=\"1000\" topOffset=\"5\"/>", "another new control"),
    ] {
        let mut edited = insert_before_table1(&baseline("DonorForm"), &label_xml("Первая", 999, geometry, anchors));
        edited = insert_before_table1(&edited, &label_xml("Вторая", 1000, geometry, ""));
        let (kind, text) = patch_rejected("DonorForm", &edited);
        assert_eq!(kind, "invalid-xml", "{text}");
        assert!(text.contains(needle) && text.contains("Первая"), "{text}");
    }
}

#[test]
fn two_added_controls_share_container_lists() {
    let before = panel_lists(stream("DonorForm").source(), None);
    let g1 = "<Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>";
    let g2 = "<Left>600</Left><Top>520</Top><Width>70</Width><Height>18</Height>";
    let to_form = "<Anchors top=\"Top\" topOffset=\"500\" left=\"Left\" leftOffset=\"500\"/>";
    let mut edited = insert_before_table1(&baseline("DonorForm"), &label_xml("Первая", 999, g1, to_form));
    edited = insert_before_table1(&edited, &label_xml("Вторая", 1000, g2, "<Anchors top=\"Bottom\" topTarget=\"7\" topOffset=\"3\" left=\"Left\" leftOffset=\"600\"/>"));
    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let (a, b) = (pf.allocated[0].1, pf.allocated[1].1);
    assert_eq!(violations(&pf.bytes), Vec::<String>::new());
    let root = panel_lists(&pf.bytes, None);
    assert_eq!(root[0].len(), before[0].len() + 1);
    assert_eq!(root[2].len(), before[2].len() + 2);
    assert_eq!(root[2][root[2].len() - 2..], [(a, 2), (b, 2)]);
    assert_eq!(root[0].last(), Some(&(a, 0)));
    assert_eq!(geom_lists(&pf.bytes, 7)[1].last(), Some(&(b, 0)));
}

#[test]
fn control_added_to_a_panel_page_uses_the_panel_as_container() {
    let mut xml = baseline("DonorForm");
    let label = "<LabelDecoration name=\"НаВторойСтранице\" id=\"999\">\n              <Left>10</Left><Top>10</Top><Width>50</Width><Height>20</Height>\n              <ZOrder>9</ZOrder><TabOrder>9</TabOrder>\n              <Anchors top=\"Top\" topOffset=\"10\" left=\"Left\" leftOffset=\"10\"/>\n            </LabelDecoration>\n            ";
    xml = xml.replacen("<Table name=\"ТабличноеПоле2\"", &format!("{label}<Table name=\"ТабличноеПоле2\""), 1);
    let root_before = panel_lists(stream("DonorForm").source(), None);
    let panel_before = panel_lists(stream("DonorForm").source(), Some(13));
    let (pf, _) = structural_round_trip("DonorForm", &xml);
    let id = pf.allocated[0].1;
    assert_eq!(violations(&pf.bytes), Vec::<String>::new());
    let panel = panel_lists(&pf.bytes, Some(13));
    assert_eq!(panel[0].last(), Some(&(id, 0)));
    assert_eq!(panel[2].last(), Some(&(id, 2)));
    assert_eq!(panel[0].len(), panel_before[0].len() + 1);
    assert_eq!(panel_lists(&pf.bytes, None), root_before);
}

#[test]
fn anchor_checker_names_the_control_with_a_missing_list_entry() {
    // Надпись1 (id 5): the bottom edge is re-pointed at ПолеВвода1 (id 7), whose list does not know about it.
    let s = tweak_stream("DonorForm", |s, idx| {
        let edge = idx.get(5).unwrap().geom().unwrap().child(7).unwrap().child(1).unwrap();
        let target = edge.atom(1).unwrap().1;
        assert_eq!(text_of(s, target), "5");
        Replacement { range: target, bytes: b"7".to_vec() }
    });
    let v = anchor_graph_violations(&s);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("'Надпись1' (id 5)") && v[0].contains("edge 1") && v[0].contains("ПолеВвода1"), "{v:?}");
}

// ---------------------------------------------------------------- anchor graph of removed controls

/// The controls (other than `id` itself) that have an edge attached to control `id`: `(source, edge, part, side, offset)`.
fn dependants(bytes: &[u8], id: i64) -> Vec<(i64, usize, usize, i64, i64)> {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let mut out = Vec::new();
    for e in &idx.entries {
        let Some(g) = e.geom().filter(|g| g.node.len() >= 18) else { continue };
        for edge in 0..6 {
            for (part, a) in [(1, oneform::form_patch::read_anchor_edge(g.node, 6 + edge)), (2, oneform::form_patch::read_anchor_edge_alt(g.node, 6 + edge))] {
                if let Some(a) = a.filter(|a| a.target == id && e.id != id) {
                    out.push((e.id, edge, part, a.side, a.offset));
                }
            }
        }
    }
    out
}

/// The rectangle `[left, top, right, bottom]` of control `id`.
fn rect_of(bytes: &[u8], id: i64) -> [i64; 4] {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let g = idx.get(id).unwrap().geom().unwrap();
    [1, 2, 3, 4].map(|i| g.atom(i).unwrap().0.value.parse().unwrap())
}

fn remove_controls(name: &str, blocks: &[(&str, &str)]) -> (PatchedForm, String) {
    let mut edited = baseline(name);
    for (open, close) in blocks {
        edited = cut_block(&edited, open, close).0;
    }
    let s = stream(name);
    let pf = patch_form(&s, baseline(name).as_bytes(), edited.as_bytes()).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    (pf, edited)
}

const INPUT7: (&str, &str) = ("<InputField name=\"ПолеВвода1\"", "</InputField>");
const SPLITTER14: (&str, &str) = ("<Splitter name=\"Разделитель1\"", "</Splitter>");

/// Every survivor keeps its rectangle and no edge, list entry or id of the removed controls is left.
fn assert_clean_removal(before: &[u8], pf: &PatchedForm, removed: &[i64]) {
    assert_eq!(violations(&pf.bytes), Vec::<String>::new());
    let (old_s, new_s) = (parse_spanned(before).unwrap(), parse_spanned(&pf.bytes).unwrap());
    let (old_idx, new_idx) = (ControlIndex::build(&old_s).unwrap(), ControlIndex::build(&new_s).unwrap());
    for e in &old_idx.entries {
        if removed.contains(&e.id) {
            assert!(new_idx.get(e.id).is_none(), "control {} is still present", e.id);
            continue;
        }
        if e.geom().is_some_and(|g| g.node.len() >= 18) {
            assert_eq!(rect_of(&pf.bytes, e.id), rect_of(before, e.id), "rectangle of control {}", e.id);
        }
    }
    for &r in removed {
        assert_eq!(dependants(&pf.bytes, r), Vec::new(), "edges still attached to {r}");
    }
    for e in &new_idx.entries {
        for list in geom_lists(&pf.bytes, e.id) {
            assert!(list.iter().all(|(src, _)| !removed.contains(src)), "control {} still lists a removed control: {list:?}", e.id);
        }
    }
    assert!(panel_lists(&pf.bytes, None).iter().flatten().all(|(src, _)| !removed.contains(src)));
}

#[test]
fn removed_target_hands_its_dependants_over_to_its_own_target() {
    let before = stream("DonorForm").source().to_vec();
    let (pf, _) = remove_controls("DonorForm", &[INPUT7]);
    assert_clean_removal(&before, &pf, &[7]);
    // Флажок1 (8) was attached to the right side of ПолеВвода1 (7) with offset 5; that side hangs at Разделитель1 (14) left - 288
    assert_eq!(anchor_edges(&pf.bytes, 8)[2], Some((14, 2, -283)));
    let lists = geom_lists(&pf.bytes, 14);
    assert!(lists[2].contains(&(8, 2)) && !lists[2].contains(&(7, 3)), "{lists:?}");
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    assert!(got.control(7).is_none());
    assert_eq!(got.control(8).unwrap().values.anchors.as_ref().unwrap().left, Some(oneform::form_patch::AnchorEdge { target: 14, side: 2, offset: -283 }));
}

/// Edge record `edge` (0..6) of control `id`, primary (`part` 1) or secondary (`part` 2) attachment.
fn edge_part(bytes: &[u8], id: i64, edge: usize, part: usize) -> oneform::form_patch::AnchorEdge {
    let s = parse_spanned(bytes).unwrap();
    let idx = ControlIndex::build(&s).unwrap();
    let g = idx.get(id).unwrap().geom().unwrap().node;
    if part == 1 { oneform::form_patch::read_anchor_edge(g, 6 + edge) } else { oneform::form_patch::read_anchor_edge_alt(g, 6 + edge) }.unwrap()
}

/// Every edge that was attached to a removed control now either points to a surviving control or to the container;
/// an edge to the container sits at the control's own coordinate.
fn assert_dependants_in_place(before: &[u8], after: &[u8], removed: &[i64]) {
    let mut seen = 0;
    for &r in removed {
        for (src, edge, part, ..) in dependants(before, r) {
            if removed.contains(&src) {
                continue;
            }
            seen += 1;
            let now = edge_part(after, src, edge, part);
            assert!(!removed.contains(&now.target) && now.target != -1, "control {src} edge {edge}: {now:?}");
            if now.target == 0 {
                let rect = rect_of(after, src);
                let coordinate = [rect[1], rect[3], rect[0], rect[2]][edge];
                assert_eq!(now.offset, coordinate, "control {src} edge {edge} to the form must sit at its coordinate");
            }
        }
    }
    assert!(seen > 0);
}

#[test]
fn removed_target_with_a_fixed_size_edge_is_followed_through_itself() {
    // Разделитель1 (14): left = own right - 12, right = form + 715; eleven edges hang at its sides.
    let before = stream("DonorForm").source().to_vec();
    assert_eq!(dependants(&before, 14).len(), 11);
    let (pf, _) = remove_controls("DonorForm", &[SPLITTER14]);
    assert_clean_removal(&before, &pf, &[14]);
    assert_dependants_in_place(&before, &pf.bytes, &[14]);
    let at = |id, edge, part| {
        let e = edge_part(&pf.bytes, id, edge, part);
        (e.target, e.side, e.offset)
    };
    assert_eq!(at(13, 3, 1), (0, 2, 698)); // was 14.left - 5
    assert_eq!(at(7, 3, 1), (0, 2, 415)); // was 14.left - 288
    assert_eq!(at(15, 0, 1), (0, 0, 382)); // was 14.bottom + 5
    assert_eq!(at(16, 2, 1), (0, 2, 728)); // was 14.right + 13
    assert_eq!(at(6, 3, 2), (0, 2, 214)); // the secondary attachment of the right edge, was 14.left - 489
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    assert!(got.control(14).is_none() && got.control(13).is_some());
}

#[test]
fn removing_a_chain_of_targets_resolves_to_the_surviving_ancestor() {
    // Флажок1 (8) hangs at ПолеВвода1 (7), which hangs at Разделитель1 (14): both go.
    let before = stream("DonorForm").source().to_vec();
    let (pf, _) = remove_controls("DonorForm", &[INPUT7, SPLITTER14]);
    assert_clean_removal(&before, &pf, &[7, 14]);
    assert_dependants_in_place(&before, &pf.bytes, &[7, 14]);
    // 8.left = 7.right + 5 = 14.left - 283 = (14.right - 12) - 283
    let e = edge_part(&pf.bytes, 8, 2, 1);
    assert_eq!((e.target, e.side, e.offset), (0, 2, 420));
}

#[test]
fn two_removed_controls_attached_to_each_other_leave_no_trace() {
    let before = stream("DonorForm").source().to_vec();
    let (pf, _) = remove_controls("DonorForm", &[INPUT7, ("<CheckBox name=\"Флажок1\"", "</CheckBox>")]);
    assert_clean_removal(&before, &pf, &[7, 8]);
    // the entry of 7's right edge leaves the list of 14; the survivors are not re-anchored
    let (old, new) = (geom_lists(&before, 14), geom_lists(&pf.bytes, 14));
    assert!(old[2].contains(&(7, 3)) && !new[2].contains(&(7, 3)));
    assert_eq!(new[2].len(), old[2].len() - 1);
    assert_eq!(anchor_edges(&pf.bytes, 13), anchor_edges(&before, 13));
}

#[test]
fn removing_the_first_items_of_a_list_together_cuts_one_clean_range() {
    // КоманднаяПанель1 (2) and ОсновныеДействияФормы (3) are the first two items of the page's list: their
    // removal ranges used to overlap on the separator between them.
    let before = stream("DonorForm").source().to_vec();
    let bars = [("<CommandBar name=\"КоманднаяПанель1\"", "</CommandBar>"), ("<CommandBar name=\"ОсновныеДействияФормы\"", "</CommandBar>")];
    let (pf, _) = remove_controls("DonorForm", &bars);
    assert_clean_removal(&before, &pf, &[2, 3]);
}

#[test]
fn removing_a_panel_removes_its_children_and_their_list_entries() {
    let before = stream("DonorForm").source().to_vec();
    let panel = ("<Panel name=\"Панель1\"", "</Panel>");
    let (pf, _) = remove_controls("DonorForm", &[panel]);
    assert_clean_removal(&before, &pf, &[13, 34, 35]);
    // Панель1 hung at ПолеСписка1 (10), КоманднаяПанель2 (12) and Разделитель1 (14)
    assert!(geom_lists(&before, 10)[1].iter().any(|e| e.0 == 13));
    assert!(geom_lists(&pf.bytes, 10)[1].iter().all(|e| e.0 != 13));
    assert!(geom_lists(&pf.bytes, 14)[2].iter().all(|e| e.0 != 13));
}

/// A new label attached to ПолеВвода1 (7), packed as a first step; returns the patched stream and its XML.
fn with_label_on_input7(anchors: &str) -> (PatchedForm, String) {
    let geometry = "<Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>";
    let edited = insert_before_table1(&baseline("DonorForm"), &label_xml("Привязанная", 999, geometry, anchors));
    let (pf, _) = structural_round_trip("DonorForm", &edited);
    let xml = decode_xml("DonorForm", &pf.bytes);
    (pf, xml)
}

#[test]
fn dependant_of_an_unattached_side_falls_back_to_the_form_at_its_absolute_position() {
    // The label hangs below and to the left of 7: 7's left side is unattached, its bottom is "own top + 19".
    let (step1, xml1) = with_label_on_input7("<Anchors top=\"Bottom\" topTarget=\"7\" topOffset=\"5\" left=\"Left\" leftTarget=\"7\" leftOffset=\"363\"/>");
    let id = step1.allocated[0].1;
    assert_eq!(dependants(&step1.bytes, 7).iter().filter(|d| d.0 == id).count(), 2);
    let s1 = parse_spanned(&step1.bytes).unwrap();
    let cut = cut_block(&xml1, "<InputField name=\"ПолеВвода1\"", "</InputField>").0;
    let pf = patch_form(&s1, xml1.as_bytes(), cut.as_bytes()).unwrap();
    assert_only_authorized_changes(s1.source(), &pf);
    assert_clean_removal(&step1.bytes, &pf, &[7]);
    let rect = rect_of(&pf.bytes, id);
    assert_eq!(rect, [500, 500, 580, 520]);
    let at = |edge| {
        let e = edge_part(&pf.bytes, id, edge, 1);
        (e.target, e.side, e.offset)
    };
    assert_eq!((at(0), at(2)), ((0, 0, 500), (0, 2, 500)));
    // the form's lists carry the new entries once
    let root = panel_lists(&pf.bytes, None);
    assert_eq!(root[0].iter().filter(|e| **e == (id, 0)).count(), 1);
    assert_eq!(root[2].iter().filter(|e| **e == (id, 2)).count(), 1);
    // the XML of the result is what the decoder makes of it
    let got = parse_form_xml(decode_xml("DonorForm", &pf.bytes).as_bytes(), "decoded").unwrap();
    assert!(got.control(7).is_none());
    assert_eq!(got.control(id).unwrap().values.anchors.as_ref().unwrap().left, Some(oneform::form_patch::AnchorEdge { target: 0, side: 2, offset: 500 }));
}

#[test]
fn explicit_anchor_to_a_control_removed_by_the_same_edit_is_rejected() {
    let edited = cut_block(&baseline("DonorForm"), INPUT7.0, INPUT7.1).0;
    let edited = replace_once(&edited, "left=\"Right\" leftTarget=\"7\" leftOffset=\"5\"", "left=\"Right\" leftTarget=\"7\" leftOffset=\"6\"");
    let (kind, text) = patch_rejected("DonorForm", &edited);
    assert_eq!(kind, "invalid-xml", "{text}");
    assert!(text.contains("removed by the same edit") && text.contains("Флажок1"), "{text}");
}

/// `docs/xml-reference.md`: the property tags of every control type, generated from the patcher itself.
/// `GOLDEN_BLESS=1 cargo test --test form_patch xml_reference` rewrites it.
#[test]
fn xml_reference_is_current() {
    let mut out = String::from(
        "# Form.xml: property tags per control type\n\n\
Generated by `cargo test --test form_patch xml_reference` (`GOLDEN_BLESS=1` rewrites this file). Every tag listed can be\n\
changed in an extracted `Form.xml` and packed back; the tags a control currently shows are only those whose value\n\
differs from a freshly added control of the type. An absent tag means \"do not touch\".\n\n",
    );
    for (_guid, name) in oneform::oof::CONTROL_GUIDS {
        let tags = oneform::form_patch::supported_tags_for(name);
        out.push_str(&format!("## {name}\n\n{}\n\n", tags.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ")));
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs").join("xml-reference.md");
    if std::env::var("GOLDEN_BLESS").is_ok() {
        std::fs::write(&path, out.as_bytes()).unwrap();
    } else {
        let have = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert_eq!(have, out, "docs/xml-reference.md is stale: run with GOLDEN_BLESS=1");
    }
}

// ---------------------------------------------------------------- binding to attributes of the object

mod object_binding {
    use super::*;
    use oneform::object_schema::{with_object_schema, ObjectSchema};

    const INN: &str = "11111111-0000-0000-0000-000000000001";
    const KPP: &str = "11111111-0000-0000-0000-000000000002";

    fn object_xml(kind: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.17">
  <{kind} uuid="aaaaaaaa-0000-0000-0000-000000000001">
    <Properties><Name>Объект</Name></Properties>
    <ChildObjects>
      <Attribute uuid="{INN}"><Properties><Name>ИНН</Name></Properties></Attribute>
      <Attribute uuid="{KPP}"><Properties><Name>КПП</Name></Properties></Attribute>
      <Attribute uuid="11111111-0000-0000-0000-000000000003"><Properties><Name>Двойной</Name></Properties></Attribute>
      <Attribute uuid="11111111-0000-0000-0000-000000000004"><Properties><Name>Двойной</Name></Properties></Attribute>
      <TabularSection uuid="22222222-0000-0000-0000-000000000001">
        <Properties><Name>Контакты</Name></Properties>
        <ChildObjects>
          <Attribute uuid="33333333-0000-0000-0000-000000000001"><Properties><Name>Значение</Name></Properties></Attribute>
        </ChildObjects>
      </TabularSection>
    </ChildObjects>
  </{kind}>
</MetaDataObject>"#
        )
    }

    fn schema(kind: &str) -> ObjectSchema {
        ObjectSchema::from_object_xml(&object_xml(kind)).unwrap()
    }

    /// `uuid -> name` of the schema plus its kind: what `extract` knows about the object.
    fn lookup(kind: &str) -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("@kind".to_string(), kind.to_string());
        for (name, uuid) in &schema(kind).attrs {
            m.insert(uuid.to_lowercase(), name.clone());
        }
        m
    }

    fn decode_with(raw: &[u8], lookup: &std::collections::HashMap<String, String>) -> FormXml {
        let base = baseline("DonorForm");
        let doc = roxmltree::Document::parse(&base).unwrap();
        let (id, fname) = (doc.root_element().attribute("id").unwrap(), doc.root_element().attribute("name").unwrap());
        let model = decode_oof_stream(raw, id, Some(lookup)).unwrap();
        let xml = render_form_xml(&common::form_info(id, fname, raw), &model).unwrap();
        let xml = String::from_utf8(enrich_ext_properties(raw, &xml).unwrap()).unwrap().replace("\r\n", "\n");
        parse_form_xml(xml.as_bytes(), "decoded").unwrap()
    }

    /// The link row body of control `id`, without whitespace.
    fn link_of(bytes: &[u8], id: i64) -> Option<String> {
        let s = parse_spanned(bytes).unwrap();
        let links = s.root.node(2)?.node(3)?;
        for item in links.children.iter().skip(1) {
            let oneform::brace::Item::Node(row) = item else { continue };
            if row.children[0].as_atom().and_then(|a| a.value.parse::<i64>().ok()) == Some(id) {
                let mut out = Vec::new();
                oneform::donor::write_node(row.node(1)?, &mut out);
                return Some(String::from_utf8(out).unwrap().split_whitespace().collect());
            }
        }
        None
    }

    fn bind(path: &str) -> String {
        format!("<DataPath attributeId=\"1\">ОбработкаОбъект.{path}</DataPath>")
    }

    fn rebind_existing(path: &str) -> String {
        mutate("<DataPath attributeId=\"3\">ПолеВвода1</DataPath>", &bind(path))
    }

    fn with_added_field(name: &str, id: i64, data_path: &str, xml: &str) -> String {
        let block = format!(
            "<InputField name=\"{name}\" id=\"{id}\">\n          {data_path}\n          <Left>520</Left><Top>500</Top><Width>120</Width><Height>20</Height>\n          <ZOrder>55</ZOrder><TabOrder>55</TabOrder>\n        </InputField>\n        "
        );
        insert_before_table1(xml, &block)
    }

    fn patch_with(kind: &str, edited: &str) -> PatchedForm {
        let s = stream("DonorForm");
        let pf = with_object_schema(Some(schema(kind)), || patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes())).unwrap();
        assert_only_authorized_changes(s.source(), &pf);
        pf
    }

    #[test]
    fn a_changed_control_is_bound_to_an_object_attribute() {
        let pf = patch_with("Catalog", &rebind_existing("ИНН"));
        assert_eq!(link_of(&pf.bytes, 7).as_deref(), Some(format!("{{2,{{1}},{{0,{INN}}}}}").as_str()));
        let got = decode_with(&pf.bytes, &lookup("Catalog"));
        assert_eq!(got.control(7).unwrap().values.data_path, Some((1, "ОбработкаОбъект.ИНН".to_string())));
    }

    #[test]
    fn an_added_control_is_bound_to_an_object_attribute() {
        let edited = with_added_field("НовоеПоле", 999, &bind("КПП"), &baseline("DonorForm"));
        let pf = patch_with("Catalog", &edited);
        let id = pf.allocated[0].1;
        assert_eq!(link_of(&pf.bytes, id).as_deref(), Some(format!("{{2,{{1}},{{0,{KPP}}}}}").as_str()));
        let got = decode_with(&pf.bytes, &lookup("Catalog"));
        assert_eq!(got.control(id).unwrap().values.data_path, Some((1, "ОбработкаОбъект.КПП".to_string())));
    }

    /// An object whose attributes have the types of the selection-button defaults.
    fn typed_schema(kind: &str) -> ObjectSchema {
        let attrs: String = [
            ("Контрагент", "cfg:CatalogRef.Контрагенты"),
            ("Основание", "cfg:DocumentRef.Заказ"),
            ("Вид", "cfg:EnumRef.Виды"),
            ("Счёт", "cfg:ChartOfAccountsRef.Основной"),
            ("Срок", "xs:dateTime"),
            ("Сумма", "xs:decimal"),
            ("Комментарий", "xs:string"),
            ("Флаг", "xs:boolean"),
            ("Разное", "cfg:CatalogRef.Контрагенты</v8:Type><v8:Type>xs:string"),
        ]
        .iter()
        .enumerate()
        .map(|(i, (name, ty))| {
            format!(
                "<Attribute uuid=\"11111111-0000-0000-0000-0000000001{i:02}\"><Properties><Name>{name}</Name><Type><v8:Type>{ty}</v8:Type></Type></Properties></Attribute>"
            )
        })
        .collect();
        let xml = object_xml(kind).replace("<MetaDataObject ", "<MetaDataObject xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ").replace("<TabularSection", &format!("{attrs}<TabularSection"));
        ObjectSchema::from_object_xml(&xml).unwrap()
    }

    /// The buttons `(ChoiceButton, OpenButton, ClearButton)` of the field that the pack of `extra` adds, read back from the stream.
    fn added_buttons(kind: &str, name: &str, extra: &str) -> (Option<bool>, Option<bool>, Option<bool>) {
        let s = stream("DonorForm");
        let edited = with_added_field("НовоеПоле", 999, &format!("{}{extra}", bind(name)), &baseline("DonorForm"));
        let pf = with_object_schema(Some(typed_schema(kind)), || patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes())).unwrap();
        assert_only_authorized_changes(s.source(), &pf);
        let id = pf.allocated[0].1;
        let mut lookup = lookup(kind);
        for (n, u) in &typed_schema(kind).attrs {
            lookup.insert(u.to_lowercase(), n.clone());
        }
        let form = decode_with(&pf.bytes, &lookup);
        let v = &form.control(id).unwrap().values;
        (v.choice_button, v.open_button, v.clear_button)
    }

    #[test]
    fn an_added_field_bound_to_a_reference_attribute_gets_the_selection_buttons() {
        for name in ["Контрагент", "Основание"] {
            assert_eq!(added_buttons("Document", name, ""), (Some(true), Some(true), Some(false)), "{name}");
        }
        for name in ["Вид", "Счёт", "Срок"] {
            assert_eq!(added_buttons("Document", name, ""), (Some(true), Some(false), Some(false)), "{name}");
        }
        // the standard attribute `Дата` of a document is a date
        assert_eq!(added_buttons("Document", "Дата", ""), (Some(true), Some(false), Some(false)));
    }

    /// The staged-result check of `oneform pack` for a field added with the defaults: the decoded `Form.xml` holds
    /// the buttons that were built, and the report lists the edit.
    #[test]
    fn the_pack_verification_expects_the_defaulted_buttons() {
        let s = stream("DonorForm");
        let base = baseline("DonorForm");
        let edited = with_added_field("НовоеПоле", 999, &bind("Контрагент"), &base);
        let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
        let tree = |xml: &str, bytes: &[u8]| -> oneform::transaction::Tree {
            [("Form.xml".to_string(), xml.as_bytes().to_vec()), ("FormStream.bin".to_string(), bytes.to_vec())].into_iter().collect()
        };
        let lp = with_object_schema(Some(typed_schema("Document")), || {
            oneform::pack::plan_layout_patch(&layout, &tree(&edited, s.source()), &tree(&base, s.source()))
        })
        .unwrap()
        .expect("a layout edit");
        assert!(lp.summary().iter().any(|l| l.contains("add InputField 'НовоеПоле'")), "{:?}", lp.summary());
        let mut lookup = lookup("Document");
        for (n, u) in &typed_schema("Document").attrs {
            lookup.insert(u.to_lowercase(), n.clone());
        }
        let staged_xml = {
            let doc = roxmltree::Document::parse(&base).unwrap();
            let (id, fname) = (doc.root_element().attribute("id").unwrap(), doc.root_element().attribute("name").unwrap());
            let model = decode_oof_stream(&lp.patched.bytes, id, Some(&lookup)).unwrap();
            let xml = render_form_xml(&common::form_info(id, fname, &lp.patched.bytes), &model).unwrap();
            String::from_utf8(enrich_ext_properties(&lp.patched.bytes, &xml).unwrap()).unwrap().replace("\r\n", "\n")
        };
        assert!(staged_xml.contains("<ChoiceButton>true</ChoiceButton>"));
        lp.verify_staged(&tree(&staged_xml, &lp.patched.bytes)).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
        // the same XML with the buttons turned off explicitly is a different request: the built field has them on
        let off = with_added_field("НовоеПоле", 999, &format!("{}<ChoiceButton>false</ChoiceButton><OpenButton>false</OpenButton>", bind("Контрагент")), &base);
        let lp_off = with_object_schema(Some(typed_schema("Document")), || {
            oneform::pack::plan_layout_patch(&layout, &tree(&off, s.source()), &tree(&base, s.source()))
        })
        .unwrap()
        .expect("a layout edit");
        assert!(lp_off.verify_staged(&tree(&staged_xml, &lp.patched.bytes)).is_err(), "buttons built on, requested off");
    }

    #[test]
    fn an_explicit_button_value_wins_over_the_default() {
        let off = "<ChoiceButton>false</ChoiceButton>";
        assert_eq!(added_buttons("Document", "Контрагент", off), (Some(false), Some(true), Some(false)));
        let both = "<ChoiceButton>false</ChoiceButton><OpenButton>false</OpenButton>";
        assert_eq!(added_buttons("Catalog", "Контрагент", both), (Some(false), Some(false), Some(false)));
        let open = "<OpenButton>true</OpenButton><ClearButton>true</ClearButton>";
        assert_eq!(added_buttons("Catalog", "Вид", open), (Some(true), Some(true), Some(true)));
    }

    #[test]
    fn an_added_field_of_a_plain_or_unknown_type_keeps_no_buttons() {
        for name in ["Сумма", "Комментарий", "Флаг", "Разное", "ИНН", "Код"] {
            let kind = if name == "Код" { "Catalog" } else { "Document" };
            assert_eq!(added_buttons(kind, name, ""), (Some(false), Some(false), Some(false)), "{name}");
        }
    }

    #[test]
    fn the_button_defaults_do_not_touch_existing_fields() {
        let s = stream("DonorForm");
        let edited = rebind_existing("Контрагент");
        let pf = with_object_schema(Some(typed_schema("Document")), || patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes())).unwrap();
        let mut lookup = lookup("Document");
        for (n, u) in &typed_schema("Document").attrs {
            lookup.insert(u.to_lowercase(), n.clone());
        }
        let form = decode_with(&pf.bytes, &lookup);
        let v = &form.control(7).unwrap().values;
        assert_eq!((v.choice_button, v.open_button), baseline_buttons(7), "a rebound field keeps its buttons");
    }

    fn baseline_buttons(id: i64) -> (Option<bool>, Option<bool>) {
        let form = parse_form_xml(baseline("DonorForm").as_bytes(), "baseline").unwrap();
        let v = &form.control(id).unwrap().values;
        (v.choice_button, v.open_button)
    }

    const PARTNER_TYPE: &str = "47e6dd42-cd5c-4e3a-9bc4-14816ed53d38";

    /// An object whose attributes have the types of the own value type of a field (with qualifiers and a type lookup).
    fn own_type_schema(kind: &str) -> ObjectSchema {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Catalogs")).unwrap();
        std::fs::write(
            dir.path().join("Catalogs").join("Контрагенты.xml"),
            format!(
                r#"<Obj xmlns:xr="http://v8.1c.ru/8.3/xcf/readable"><InternalInfo><xr:GeneratedType name="CatalogRef.Контрагенты" category="Ref"><xr:TypeId>{PARTNER_TYPE}</xr:TypeId></xr:GeneratedType></InternalInfo></Obj>"#
            ),
        )
        .unwrap();
        let lookup = oneform::metaref::ConfigTypeLookup::from_config_dir(dir.path());
        let string = |len: i64, allowed: &str| format!("<v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>{len}</v8:Length><v8:AllowedLength>{allowed}</v8:AllowedLength></v8:StringQualifiers>");
        let date = |fractions: &str| format!("<v8:Type>xs:dateTime</v8:Type><v8:DateQualifiers><v8:DateFractions>{fractions}</v8:DateFractions></v8:DateQualifiers>");
        let attrs: String = [
            ("Комментарий", string(40, "Variable")),
            ("ФиксСтрока", string(5, "Fixed")),
            ("Срок", date("Date")),
            ("Момент", date("DateTime")),
            ("Сумма", "<v8:Type>xs:decimal</v8:Type><v8:NumberQualifiers><v8:Digits>15</v8:Digits><v8:FractionDigits>2</v8:FractionDigits><v8:AllowedSign>Nonnegative</v8:AllowedSign></v8:NumberQualifiers>".to_string()),
            ("Флаг", "<v8:Type>xs:boolean</v8:Type>".to_string()),
            ("Контрагент", "<v8:Type>cfg:CatalogRef.Контрагенты</v8:Type>".to_string()),
            ("Чужой", "<v8:Type>cfg:CatalogRef.НетТакого</v8:Type>".to_string()),
            ("Разное", "<v8:Type>cfg:CatalogRef.Контрагенты</v8:Type><v8:Type>xs:string</v8:Type>".to_string()),
        ]
        .iter()
        .enumerate()
        .map(|(i, (name, ty))| {
            format!("<Attribute uuid=\"11111111-0000-0000-0000-0000000002{i:02}\"><Properties><Name>{name}</Name><Type>{ty}</Type></Properties></Attribute>")
        })
        .collect();
        let xml = object_xml(kind)
            .replace("<MetaDataObject ", "<MetaDataObject xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ")
            .replace(
                "<Properties><Name>Объект</Name></Properties>",
                "<Properties><Name>Объект</Name><CodeLength>9</CodeLength><CodeType>String</CodeType><CodeAllowedLength>Fixed</CodeAllowedLength><DescriptionLength>50</DescriptionLength><NumberLength>11</NumberLength><NumberType>String</NumberType><NumberAllowedLength>Variable</NumberAllowedLength></Properties>",
            )
            .replace("<TabularSection", &format!("{attrs}<TabularSection"));
        ObjectSchema::from_object_xml_with(&xml, Some(&lookup)).unwrap()
    }

    /// The patched stream of the pack that adds a field bound to `name` with `extra` XML.
    fn patch_added_own_type(kind: &str, name: &str, extra: &str) -> PatchedForm {
        let s = stream("DonorForm");
        let edited = with_added_field("НовоеПоле", 999, &format!("{}{extra}", bind(name)), &baseline("DonorForm"));
        let pf = with_object_schema(Some(own_type_schema(kind)), || patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes())).unwrap();
        assert_only_authorized_changes(s.source(), &pf);
        pf
    }

    /// The `ValueType` of the field that the pack of `extra` adds, read back from the stream.
    fn added_value_type(kind: &str, name: &str, extra: &str) -> Option<oneform::form_patch::ValueTypeSpec> {
        let pf = patch_added_own_type(kind, name, extra);
        let id = pf.allocated[0].1;
        let mut lookup = lookup(kind);
        for (n, u) in &own_type_schema(kind).attrs {
            lookup.insert(u.to_lowercase(), n.clone());
        }
        decode_with(&pf.bytes, &lookup).control(id).unwrap().values.value_type.clone()
    }

    fn vt(kind: &str, f: impl FnOnce(&mut oneform::form_patch::ValueTypeSpec)) -> Option<oneform::form_patch::ValueTypeSpec> {
        let mut v = oneform::form_patch::ValueTypeSpec { kind: kind.to_string(), ..Default::default() };
        f(&mut v);
        Some(v)
    }

    #[test]
    fn an_added_field_gets_the_own_value_type_of_its_attribute() {
        // dates: `Date` and `DateTime` are different patterns
        assert_eq!(added_value_type("Document", "Срок", ""), vt("Date", |_| {}));
        assert_eq!(added_value_type("Document", "Момент", ""), vt("DateTime", |_| {}));
        assert_eq!(added_value_type("Document", "Дата", ""), vt("DateTime", |_| {}));
        // numbers: digits and fraction (the sign flag is not part of `Form.xml`)
        assert_eq!(added_value_type("Document", "Сумма", ""), vt("Number", |v| (v.digits, v.fraction_digits) = (Some(15), Some(2))));
        // strings: the length
        assert_eq!(added_value_type("Document", "Комментарий", ""), vt("String", |v| v.length = Some(40)));
        assert_eq!(added_value_type("Document", "Флаг", ""), vt("Boolean", |_| {}));
        // a reference to a catalog of the configuration
        assert_eq!(added_value_type("Document", "Контрагент", ""), vt("Ref", |v| v.ref_guids = vec![PARTNER_TYPE.to_string()]));
        // standard attributes: the length that the object states
        assert_eq!(added_value_type("Document", "Номер", ""), vt("String", |v| v.length = Some(11)));
        assert_eq!(added_value_type("Catalog", "Код", ""), vt("String", |v| v.length = Some(9)));
        assert_eq!(added_value_type("Catalog", "Наименование", ""), vt("String", |v| v.length = Some(50)));
    }

    #[test]
    fn the_flags_of_the_own_value_type_reach_the_stream() {
        let patterns = |kind: &str, name: &str| {
            let pf = patch_added_own_type(kind, name, "");
            let text = String::from_utf8_lossy(&pf.bytes).split_whitespace().collect::<String>();
            ["{\"Pattern\",{\"N\",15,2,1}}", "{\"Pattern\",{\"S\",5,0}}", "{\"Pattern\",{\"S\",40,1}}", "{\"Pattern\",{\"S\",9,0}}"].map(|p| text.contains(p))
        };
        assert_eq!(patterns("Document", "Сумма"), [true, false, false, false], "non-negative number");
        assert_eq!(patterns("Document", "ФиксСтрока"), [false, true, false, false], "fixed string");
        assert_eq!(patterns("Document", "Комментарий"), [false, false, true, false], "variable string");
        assert_eq!(patterns("Catalog", "Код"), [false, false, false, true], "fixed length code");
    }

    #[test]
    fn an_explicit_value_type_wins_over_the_attribute_type() {
        let own = r#"<ValueType kind="String" composite="false" length="7"/>"#;
        assert_eq!(added_value_type("Document", "Комментарий", own), vt("String", |v| v.length = Some(7)));
        assert_eq!(added_value_type("Document", "Контрагент", own), vt("String", |v| v.length = Some(7)));
    }

    #[test]
    fn a_composite_or_unknown_type_leaves_the_value_type_of_the_donor() {
        let donor = added_value_type("Document", "ИНН", "");
        assert_eq!(donor.as_ref().map(|v| v.kind.as_str()), Some("ValueList"), "the donor field carries a value list");
        for name in ["Разное", "Чужой"] {
            assert_eq!(added_value_type("Document", name, ""), donor, "{name}");
        }
        // a standard attribute whose type the object XML does not state
        assert_eq!(added_value_type("Catalog", "Родитель", ""), donor);
    }

    #[test]
    fn the_pack_verification_expects_the_defaulted_value_type() {
        let s = stream("DonorForm");
        let base = baseline("DonorForm");
        let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
        let tree = |xml: &str, bytes: &[u8]| -> oneform::transaction::Tree {
            [("Form.xml".to_string(), xml.as_bytes().to_vec()), ("FormStream.bin".to_string(), bytes.to_vec())].into_iter().collect()
        };
        let plan = |edited: &str| {
            with_object_schema(Some(own_type_schema("Document")), || {
                oneform::pack::plan_layout_patch(&layout, &tree(edited, s.source()), &tree(&base, s.source()))
            })
            .unwrap()
            .expect("a layout edit")
        };
        let edited = with_added_field("НовоеПоле", 999, &bind("Комментарий"), &base);
        let lp = plan(&edited);
        let mut lookup = lookup("Document");
        for (n, u) in &own_type_schema("Document").attrs {
            lookup.insert(u.to_lowercase(), n.clone());
        }
        let staged_xml = {
            let doc = roxmltree::Document::parse(&base).unwrap();
            let (id, fname) = (doc.root_element().attribute("id").unwrap(), doc.root_element().attribute("name").unwrap());
            let model = decode_oof_stream(&lp.patched.bytes, id, Some(&lookup)).unwrap();
            let xml = render_form_xml(&common::form_info(id, fname, &lp.patched.bytes), &model).unwrap();
            String::from_utf8(enrich_ext_properties(&lp.patched.bytes, &xml).unwrap()).unwrap().replace("\r\n", "\n")
        };
        assert!(staged_xml.contains("<ValueType kind=\"String\" composite=\"false\" length=\"40\"/>"), "{staged_xml}");
        lp.verify_staged(&tree(&staged_xml, &lp.patched.bytes)).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
        // a request for another length is a different request: the built field has 40
        let other = with_added_field("НовоеПоле", 999, &format!("{}<ValueType kind=\"String\" composite=\"false\" length=\"10\"/>", bind("Комментарий")), &base);
        assert!(plan(&other).verify_staged(&tree(&staged_xml, &lp.patched.bytes)).is_err(), "length 40 built, 10 requested");
    }

    #[test]
    fn standard_attributes_are_written_with_the_code_of_the_object_kind() {
        for (kind, name, code) in [("Catalog", "Код", -2), ("Catalog", "Наименование", -3), ("Document", "Номер", -2), ("Document", "Дата", -3)] {
            let edited = with_added_field("НовоеПоле", 999, &bind(name), &baseline("DonorForm"));
            let pf = patch_with(kind, &edited);
            let id = pf.allocated[0].1;
            assert_eq!(link_of(&pf.bytes, id).as_deref(), Some(format!("{{2,{{1}},{{{code}}}}}").as_str()), "{kind}.{name}");
            let got = decode_with(&pf.bytes, &lookup(kind));
            assert_eq!(got.control(id).unwrap().values.data_path, Some((1, format!("ОбработкаОбъект.{name}"))), "{kind}.{name}");

            let pf = patch_with(kind, &rebind_existing(name));
            assert_eq!(link_of(&pf.bytes, 7).as_deref(), Some(format!("{{2,{{1}},{{{code}}}}}").as_str()), "{kind}.{name}");
        }
    }

    /// A form whose links list is empty (`{0}`): the first inserted link needs its own separator.
    #[test]
    fn a_link_can_be_inserted_into_an_empty_links_list() {
        let base = baseline("DonorFormNoLinks");
        let s = stream("DonorFormNoLinks");
        for (path, expect) in [("ИНН", format!("{{2,{{0}},{{0,{INN}}}}}")), ("", "{1,{0}}".to_string())] {
            let text = if path.is_empty() { "ОбработкаОбъект".to_string() } else { format!("ОбработкаОбъект.{path}") };
            let block = format!(
                "    <InputField name=\"Новое\" id=\"900\">
      <DataPath attributeId=\"0\">{text}</DataPath>
      <Left>8</Left><Top>270</Top><Width>200</Width><Height>19</Height>
      <ZOrder>5</ZOrder><TabOrder>5</TabOrder>
    </InputField>
  </ChildItems>"
            );
            let edited = replace_once(&base, "  </ChildItems>", &block);
            let pf = with_object_schema(Some(schema("Catalog")), || patch_form(&s, base.as_bytes(), edited.as_bytes())).unwrap();
            assert_only_authorized_changes(s.source(), &pf);
            let id = pf.allocated[0].1;
            assert_eq!(link_of(&pf.bytes, id).as_deref(), Some(expect.as_str()), "{text}");
        }
    }

    #[test]
    fn a_form_attribute_binding_is_still_written_as_before() {
        let edited = mutate("<DataPath attributeId=\"3\">ПолеВвода1</DataPath>", "<DataPath attributeId=\"4\">Флажок1</DataPath>");
        let pf = patch_with("Catalog", &edited);
        assert_eq!(link_of(&pf.bytes, 7).as_deref(), Some("{1,{4}}"));
    }

    fn refusal(schema: Option<ObjectSchema>, edited: &str) -> String {
        let s = stream("DonorForm");
        let e = with_object_schema(schema, || plan(&s, edited)).expect_err("must be refused");
        assert_eq!(e.code, GENERATED_FILE_MODIFIED, "{e}");
        format!("{e} {}", e.params.get("problems").map(|p| p.to_string()).unwrap_or_default())
    }

    #[test]
    fn unsupported_bindings_are_refused_with_the_line_and_the_path() {
        let cases: [(Option<ObjectSchema>, &str, &str); 5] = [
            (Some(schema("Catalog")), "НетТакого", "no attribute or standard attribute 'НетТакого'"),
            (Some(schema("Catalog")), "Двойной", "several attributes named 'Двойной'"),
            (Some(schema("Catalog")), "ИНН.Код", "three or more segments"),
            (Some(schema("Catalog")), "Контакты", "tabular section"),
            (None, "ИНН", "object metadata not available"),
        ];
        for (schema, name, why) in cases {
            for edited in [rebind_existing(name), with_added_field("НовоеПоле", 999, &bind(name), &baseline("DonorForm"))] {
                let text = refusal(schema.clone(), &edited);
                assert!(text.contains(why), "{name}: {text}");
                assert!(text.contains(&format!("ОбработкаОбъект.{name}")) && text.contains("line "), "{name}: {text}");
            }
        }
        // the hint lists what the object has
        let text = refusal(Some(schema("Catalog")), &rebind_existing("НетТакого"));
        assert!(text.contains("ИНН") && text.contains("Код"), "{text}");
        // a root that is not the main attribute
        let edited = mutate("<DataPath attributeId=\"3\">ПолеВвода1</DataPath>", "<DataPath attributeId=\"3\">ПолеВвода1.ИНН</DataPath>");
        let text = refusal(Some(schema("Catalog")), &edited);
        assert!(text.contains("DataPath"), "{text}");
        // a standard attribute of another kind
        let text = refusal(Some(schema("Document")), &rebind_existing("Наименование"));
        assert!(text.contains("'Наименование'"), "{text}");
    }

    #[test]
    fn unchanged_dotted_links_keep_their_bytes_even_without_a_schema() {
        // Control 7 is bound to a path of three steps that `extract` shows only partly: `ОбработкаОбъект.ИНН`.
        let row = "{7,\r\n{1,\r\n{3}\r\n}\r\n}";
        let wide = format!("{{7,\r\n{{3,\r\n{{1}},\r\n{{0,{INN}}},\r\n{{0,{KPP}}}\r\n}}\r\n}}");
        let mut raw = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();
        let at = raw.windows(row.len()).position(|w| w == row.as_bytes()).expect("link row of control 7");
        raw.splice(at..at + row.len(), wide.bytes());
        let s = parse_spanned(&raw).unwrap();

        let base = rebind_existing("ИНН");
        let edited = edit_ctl(&base, "ПолеВвода1", &[("<Width>278</Width>", "<Width>279</Width>")]);
        let plan = plan_form_patch(&s, base.as_bytes(), edited.as_bytes()).unwrap();
        assert!(!plan.is_empty());
        let pf = patch_form(&s, base.as_bytes(), edited.as_bytes()).unwrap();
        assert!(String::from_utf8_lossy(&pf.bytes).contains(&wide), "the link row must be byte for byte the same");
        assert_only_authorized_changes(s.source(), &pf);
    }

    #[test]
    fn a_changed_dotted_link_is_replaced_and_verified() {
        let wide = format!("{{7,\r\n{{2,\r\n{{1}},\r\n{{0,{INN}}}\r\n}}\r\n}}");
        let row = "{7,\r\n{1,\r\n{3}\r\n}\r\n}";
        let mut raw = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();
        let at = raw.windows(row.len()).position(|w| w == row.as_bytes()).unwrap();
        raw.splice(at..at + row.len(), wide.bytes());
        let s = parse_spanned(&raw).unwrap();
        let base = rebind_existing("ИНН");
        let edited = replace_once(&base, &bind("ИНН"), &bind("Код"));
        let pf = with_object_schema(Some(schema("Catalog")), || patch_form(&s, base.as_bytes(), edited.as_bytes())).unwrap();
        assert_eq!(link_of(&pf.bytes, 7).as_deref(), Some("{2,{1},{-2}}"));
    }

    const MULTI: &str = "editing multi-step links is not supported yet";
    const K3: &str = "ОбработкаОбъект.ИНН.КПП";
    const K4: &str = "ОбработкаОбъект.ИНН.КПП.[-2]";

    /// The DonorForm stream whose link row of control 7 (`ПолеВвода1`) has `steps` after `{1}`, and a baseline
    /// `Form.xml` that shows that link as `steps` + `text`.
    fn linked(steps: &[&str], text: &str) -> (SpannedMetadata, String) {
        let row = "{7,\r\n{1,\r\n{3}\r\n}\r\n}";
        let braces: String = steps.iter().map(|t| format!(",\r\n{}", oneform::oof::DataStep::parse_token(t).unwrap().brace())).collect();
        let wide = format!("{{7,\r\n{{{},\r\n{{1}}{braces}\r\n}}\r\n}}", steps.len() + 1);
        let mut raw = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();
        let at = raw.windows(row.len()).position(|w| w == row.as_bytes()).expect("link row of control 7");
        raw.splice(at..at + row.len(), wide.bytes());
        (parse_spanned(&raw).unwrap(), shown("1", steps, text))
    }

    fn shown(aid: &str, steps: &[&str], text: &str) -> String {
        mutate(
            "<DataPath attributeId=\"3\">ПолеВвода1</DataPath>",
            &format!("<DataPath attributeId=\"{aid}\" steps=\"{}\">{text}</DataPath>", steps.join(" ")),
        )
    }

    fn steps_of(n: usize) -> Vec<&'static str> {
        [INN, KPP, "-2"][..n].to_vec()
    }

    fn refused(s: &SpannedMetadata, base: &str, edited: &str) -> String {
        let Err(e) = with_object_schema(Some(schema("Catalog")), || plan_form_patch(s, base.as_bytes(), edited.as_bytes())) else {
            panic!("must be refused");
        };
        assert_eq!(e.code, GENERATED_FILE_MODIFIED, "{e}");
        format!("{e} {}", e.params.get("problems").map(|p| p.to_string()).unwrap_or_default())
    }

    #[test]
    fn an_unrelated_edit_keeps_the_rows_of_links_with_three_to_five_rows() {
        for (steps, text) in [(steps_of(2), K3), (steps_of(3), K4)] {
            let (s, base) = linked(&steps, text);
            let edited = edit_ctl(&base, "ПолеВвода1", &[("<Width>278</Width>", "<Width>279</Width>")]);
            let pf = with_object_schema(Some(schema("Catalog")), || patch_form(&s, base.as_bytes(), edited.as_bytes())).unwrap();
            assert_only_authorized_changes(s.source(), &pf);
            assert_eq!(link_of(&pf.bytes, 7), link_of(s.source(), 7), "{text}");
            assert!(pf.bytes != s.source(), "the width must change");
        }
    }

    #[test]
    fn editing_steps_or_the_text_of_a_longer_link_is_refused() {
        for (steps, text) in [(steps_of(2), K3), (steps_of(3), K4)] {
            let (s, base) = linked(&steps, text);
            let shorter = steps[..steps.len() - 1].join(" ");
            let short_text = text.rsplit_once('.').unwrap().0;
            let cases = [
                ("steps edited", replace_once(&base, &steps.join(" "), &shorter)),
                ("steps and text edited", replace_once(&replace_once(&base, &steps.join(" "), &shorter), text, short_text)),
                ("text edited", replace_once(&base, text, "ОбработкаОбъект.КПП.ИНН")),
                ("link removed", replace_once(&base, &format!("<DataPath attributeId=\"1\" steps=\"{}\">{text}</DataPath>
          ", steps.join(" ")), "")),
            ];
            for (what, edited) in cases {
                let why = refused(&s, &base, &edited);
                assert!(why.contains(MULTI) && why.contains("line ") && why.contains("ПолеВвода1"), "{what}: {why}");
                if what != "link removed" {
                    assert!(why.contains("ОбработкаОбъект."), "{what}: {why}");
                }
            }
        }
    }

    #[test]
    fn the_text_of_a_link_with_two_rows_is_still_resolved_by_name() {
        // `steps` stays, only the text moves: the text decides (as before `steps` existed)
        let (s, base) = linked(&[INN], "ОбработкаОбъект.ИНН");
        let edited = replace_once(&base, ">ОбработкаОбъект.ИНН<", ">ОбработкаОбъект.КПП<");
        let pf = with_object_schema(Some(schema("Catalog")), || patch_form(&s, base.as_bytes(), edited.as_bytes())).unwrap();
        assert_eq!(link_of(&pf.bytes, 7).as_deref(), Some(format!("{{2,{{1}},{{0,{KPP}}}}}").as_str()));
        // `steps` dropped: the same
        let edited = replace_once(&edited, &format!(" steps=\"{INN}\""), "");
        let pf = with_object_schema(Some(schema("Catalog")), || patch_form(&s, base.as_bytes(), edited.as_bytes())).unwrap();
        assert_eq!(link_of(&pf.bytes, 7).as_deref(), Some(format!("{{2,{{1}},{{0,{KPP}}}}}").as_str()));
        // `steps` changed: refused
        let edited = replace_once(&base, &format!("steps=\"{INN}\""), &format!("steps=\"{KPP}\""));
        assert!(refused(&s, &base, &edited).contains(MULTI));
        // a single-row link may not gain steps
        let (s, base) = (stream("DonorForm"), baseline("DonorForm"));
        let edited = replace_once(&base, "<DataPath attributeId=\"3\">ПолеВвода1<", &format!("<DataPath attributeId=\"3\" steps=\"{INN}\">ПолеВвода1<"));
        let why = refused(&s, &base, &edited);
        assert!(why.contains("DataPath") && why.contains("line "), "{why}");
    }

    #[test]
    fn a_copy_with_two_rows_is_bound_by_its_text_and_a_longer_one_is_refused() {
        let (s, base) = linked(&[INN], "ОбработкаОбъект.ИНН");
        let field = |path: &str, steps: &str| format!("<DataPath attributeId=\"1\" steps=\"{steps}\">{path}</DataPath>");
        let added = with_added_field("НовоеПоле", 999, &field("ОбработкаОбъект.КПП", INN), &base);
        let pf = with_object_schema(Some(schema("Catalog")), || patch_form(&s, base.as_bytes(), added.as_bytes())).unwrap();
        assert_eq!(link_of(&pf.bytes, pf.allocated[0].1).as_deref(), Some(format!("{{2,{{1}},{{0,{KPP}}}}}").as_str()));
        for (path, steps) in [(K3, format!("{INN} {KPP}")), (K4, format!("{INN} {KPP} -2"))] {
            let added = with_added_field("НовоеПоле", 999, &field(path, &steps), &base);
            let why = refused(&s, &base, &added);
            assert!(why.contains(MULTI) && why.contains("line ") && why.contains(path), "{why}");
        }
    }

    #[test]
    fn column_check_and_picture_paths_cannot_be_edited_yet() {
        let (s, base) = (stream("DonorForm"), baseline("DonorForm"));
        let column = "<DataPath attributeId=\"2\">ТабличноеПоле1.Колонка1</DataPath>";
        for tag in ["CheckBoxDataPath", "PictureDataPath"] {
            let edited = replace_once(&base, column, &format!("{column}
              <{tag} attributeId=\"2\" steps=\"{INN}\">ТабличноеПоле1.Колонка1</{tag}>"));
            let why = refused(&s, &base, &edited);
            assert!(why.contains(MULTI) && why.contains(tag) && why.contains("Колонка1") && why.contains("line "), "{tag}: {why}");
        }
    }

    #[test]
    fn xml_of_an_older_version_is_refused_with_a_hint() {
        let (s, base) = (stream("DonorForm"), baseline("DonorForm"));
        let old = mutate("ordinaryFormVersion=\"2.2\"", "ordinaryFormVersion=\"2.1\"");
        for (b, e) in [(&old, &base), (&base, &old)] {
            let Err(err) = plan_form_patch(&s, b.as_bytes(), e.as_bytes()) else { panic!("must be refused") };
            assert_eq!(err.code, GENERATED_FILE_MODIFIED);
            assert_eq!(err.params["kind"], "stale-form-xml");
            assert!(err.to_string().contains("oneform extract") && err.to_string().contains("2.1"), "{err}");
        }
    }
}

// ---------------------------------------------------------------- tables bound to tabular sections of the object

/// The editor blob (base64) of the first column of the table `ТабличноеПоле1` (id 6) of a stream.
fn first_column_blob(bytes: &[u8]) -> String {
    let reparsed = parse_spanned(bytes).unwrap();
    let index = ControlIndex::build(&reparsed).unwrap();
    let info = index.get(6).unwrap().control.node.node(2).unwrap();
    let props = info.node(2).unwrap().node(1).unwrap().node(23).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap();
    props.node(39).unwrap().atom(0).unwrap().value.clone()
}

#[test]
fn an_edited_column_event_keeps_a_valid_editor_blob() {
    // the u64 length prefix of the blob is the length of the new inner text
    let original = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();
    assert!(oneform::ts_table::decode_blob(&first_column_blob(&original)).is_some(), "the platform blob has a valid length prefix");
    let edited = replace_once(
        &baseline("DonorForm"),
        "<StartChoice id=\"2\">ТабличноеПоле1Колонка1НачалоВыбора</StartChoice>",
        "<StartChoice id=\"2\">ОченьДлинныйНовыйОбработчикНачалаВыбораКолонкиТаблицы</StartChoice>",
    );
    let s = stream("DonorForm");
    let pf = patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes()).unwrap();
    assert_only_authorized_changes(s.source(), &pf);
    let text = oneform::ts_table::decode_blob(&first_column_blob(&pf.bytes)).expect("the length prefix of the patched blob is wrong");
    assert!(text.contains("ОченьДлинныйНовыйОбработчикНачалаВыбораКолонкиТаблицы"), "{text}");
}

#[test]
fn the_pattern_of_an_editor_blob_is_replaced_and_everything_else_stays() {
    let original = std::fs::read(dir("DonorForm").join("FormStream.bin")).unwrap();
    let blob = first_column_blob(&original);
    let pattern = "{\"Pattern\",{\"N\",15,2,0}}";
    let rebound = oneform::ts_table::rebind_blob(&blob, pattern).unwrap();
    assert_eq!(oneform::ts_table::blob_pattern(&rebound).as_deref(), Some(pattern));
    let (was, now) = (oneform::ts_table::decode_blob(&blob).unwrap(), oneform::ts_table::decode_blob(&rebound).expect("valid length prefix"));
    let old_pattern = oneform::ts_table::blob_pattern(&blob).unwrap();
    let flat = |t: &str| t.replace(['\r', '\n'], "");
    // the events and the parameters are in the text still: only the pattern differs
    assert!(flat(&was).contains("ТабличноеПоле1Колонка1НачалоВыбора") && flat(&now).contains("ТабличноеПоле1Колонка1НачалоВыбора"));
    assert_eq!(flat(&was).replacen(&old_pattern, pattern, 1), flat(&now));
}

mod tabular_binding {
    use super::*;
    use oneform::object_schema::{with_object_schema, ObjectSchema};
    use oneform::ts_table::read_table;

    const GOODS: &str = "22222222-0000-0000-0000-000000000001";
    const GOODS_TYPE: &str = "32dc563e-0b51-4621-9936-5bfc9fd58ef3";
    const NAME: &str = "33333333-0000-0000-0000-000000000001";
    const QTY: &str = "33333333-0000-0000-0000-000000000002";
    const NOTE: &str = "33333333-0000-0000-0000-000000000003";
    const ITEM: &str = "33333333-0000-0000-0000-000000000006";
    const KIND: &str = "33333333-0000-0000-0000-000000000007";
    const FLAG: &str = "33333333-0000-0000-0000-000000000004";
    const MARK: &str = "33333333-0000-0000-0000-00000000000a";
    const ITEM_TYPE: &str = "47e6dd42-cd5c-4e3a-9bc4-14816ed53d38";
    const KIND_TYPE: &str = "52c49a35-3374-4995-9803-5fc37bb7d7cf";

    fn object_xml() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.17">
  <Document uuid="aaaaaaaa-0000-0000-0000-000000000001">
    <Properties><Name>Объект</Name></Properties>
    <ChildObjects>
      <TabularSection uuid="{GOODS}">
        <InternalInfo>
          <xr:GeneratedType name="DocumentTabularSection.Объект.Товары" category="TabularSection">
            <xr:TypeId>{GOODS_TYPE}</xr:TypeId>
            <xr:ValueId>44444444-0000-0000-0000-000000000001</xr:ValueId>
          </xr:GeneratedType>
        </InternalInfo>
        <Properties><Name>Товары</Name></Properties>
        <ChildObjects>
          <Attribute uuid="{NAME}"><Properties><Name>Наименование</Name><Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>20</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type></Properties></Attribute>
          <Attribute uuid="{QTY}"><Properties><Name>Количество</Name><Type><v8:Type>xs:decimal</v8:Type><v8:NumberQualifiers><v8:Digits>15</v8:Digits><v8:FractionDigits>2</v8:FractionDigits><v8:AllowedSign>Any</v8:AllowedSign></v8:NumberQualifiers></Type></Properties></Attribute>
          <Attribute uuid="{NOTE}"><Properties><Name>Комментарий</Name><Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>50</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type></Properties></Attribute>
          <Attribute uuid="{FLAG}"><Properties><Name>Флаг</Name><Type><v8:Type>xs:boolean</v8:Type></Type></Properties></Attribute>
          <Attribute uuid="33333333-0000-0000-0000-000000000005"><Properties><Name>Свободный</Name><Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>0</v8:Length></v8:StringQualifiers></Type></Properties></Attribute>
          <Attribute uuid="{ITEM}"><Properties><Name>Ссылка</Name><Type><v8:Type>cfg:CatalogRef.Номенклатура</v8:Type></Type></Properties></Attribute>
          <Attribute uuid="{KIND}"><Properties><Name>Вид</Name><Type><v8:Type>cfg:EnumRef.ВидыЦен</v8:Type></Type></Properties></Attribute>
          <Attribute uuid="33333333-0000-0000-0000-000000000008"><Properties><Name>Узел</Name><Type><v8:Type>cfg:ExchangePlanRef.Обмен</v8:Type></Type></Properties></Attribute>
          <Attribute uuid="33333333-0000-0000-0000-000000000009"><Properties><Name>Чужая</Name><Type><v8:Type>cfg:CatalogRef.НетТакого</v8:Type></Type></Properties></Attribute>
          <Attribute uuid="{MARK}"><Properties><Name>Признак</Name><Type><v8:Type>xs:boolean</v8:Type></Type></Properties></Attribute>
        </ChildObjects>
      </TabularSection>
      <TabularSection uuid="22222222-0000-0000-0000-000000000002">
        <Properties><Name>Контакты</Name></Properties>
        <ChildObjects>
          <Attribute uuid="33333333-0000-0000-0000-000000000011"><Properties><Name>Значение</Name><Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>10</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type></Properties></Attribute>
        </ChildObjects>
      </TabularSection>
      <TabularSection uuid="22222222-0000-0000-0000-000000000003"><Properties><Name>Дубль</Name></Properties></TabularSection>
      <TabularSection uuid="22222222-0000-0000-0000-000000000004"><Properties><Name>Дубль</Name></Properties></TabularSection>
    </ChildObjects>
  </Document>
</MetaDataObject>"#
        )
    }

    /// The generated reference types of the configuration the object belongs to.
    fn type_lookup() -> oneform::metaref::ConfigTypeLookup {
        let dir = tempfile::tempdir().unwrap();
        for (sub, name, type_id) in [
            ("Catalogs", "CatalogRef.Номенклатура", ITEM_TYPE),
            ("Enums", "EnumRef.ВидыЦен", KIND_TYPE),
            ("ExchangePlans", "ExchangePlanRef.Обмен", "5d6b7a50-0f5e-4e53-a5b7-1d3a4d1f0e11"),
        ] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            let xml = format!(
                r#"<Obj xmlns:xr="http://v8.1c.ru/8.3/xcf/readable"><InternalInfo><xr:GeneratedType name="{name}" category="Ref"><xr:TypeId>{type_id}</xr:TypeId></xr:GeneratedType></InternalInfo></Obj>"#
            );
            std::fs::write(dir.path().join(sub).join(format!("{name}.xml")), xml).unwrap();
        }
        oneform::metaref::ConfigTypeLookup::from_config_dir(dir.path())
    }

    fn schema() -> ObjectSchema {
        ObjectSchema::from_object_xml_with(&object_xml(), Some(&type_lookup())).unwrap()
    }

    /// `uuid -> name` of the object: what `extract` knows about it.
    fn lookup() -> std::collections::HashMap<String, String> {
        let schema = schema();
        let mut m = std::collections::HashMap::new();
        m.insert("@kind".to_string(), "Document".to_string());
        for t in &schema.tabular {
            m.insert(t.uuid.to_lowercase(), t.name.clone());
            for (name, uuid) in &t.columns {
                m.insert(uuid.to_lowercase(), name.clone());
            }
        }
        m
    }

    fn col(name: &str, id: i64, path: &str, extra: &str) -> String {
        format!(
            "<Column name=\"{name}\" id=\"{id}\"><DataPath attributeId=\"1\">{path}</DataPath><Header><Item language=\"ru\">{name}</Item></Header><Width>90</Width>{extra}</Column>\n"
        )
    }

    fn table(name: &str, id: i64, data_path: &str, columns: Option<&str>) -> String {
        let dp = if data_path.is_empty() { String::new() } else { format!("<DataPath attributeId=\"1\">{data_path}</DataPath>") };
        let cols = columns.map(|c| format!("<Columns>\n{c}</Columns>")).unwrap_or_default();
        format!(
            "<Table name=\"{name}\" id=\"{id}\">{dp}<Left>300</Left><Top>300</Top><Width>400</Width><Height>150</Height><ZOrder>56</ZOrder><TabOrder>56</TabOrder>{cols}</Table>\n        "
        )
    }

    fn good_columns() -> String {
        [
            col("НомерСтроки", 0, "ОбработкаОбъект.Товары.НомерСтроки", "<ReadOnly>true</ReadOnly>"),
            col("Наименование", 1, "ОбработкаОбъект.Товары.Наименование", "<Editor>InputField</Editor>"),
            col("Количество", 2, "ОбработкаОбъект.Товары.Количество", ""),
            col("Комментарий", 7, "ОбработкаОбъект.Товары.Комментарий", "<ReadOnly>true</ReadOnly><Visible>false</Visible>"),
        ]
        .concat()
    }

    fn oneform_text(n: &oneform::brace::Node) -> String {
        let mut out = Vec::new();
        oneform::donor::write_node(n, &mut out);
        String::from_utf8(out).unwrap().split_whitespace().collect()
    }

    /// `(name, id, read_only, editor, visible, width)` of the columns of the table `name` of a parsed `Form.xml`.
    type ColumnRow = (String, Option<i64>, bool, Option<String>, bool, i64);

    fn column_values(xml: &[u8], name: &str) -> Vec<ColumnRow> {
        let parsed = parse_form_xml(xml, "columns").unwrap();
        let t = parsed.controls.iter().find(|c| c.name == name).unwrap();
        t.values.columns.iter().map(|c| (c.name.clone(), c.id, c.read_only, c.editor.clone(), c.visible, c.width)).collect()
    }

    fn with_table(block: &str) -> String {
        insert_before_table1(&baseline("DonorForm"), block)
    }

    fn patch_with(edited: &str) -> PatchedForm {
        let s = stream("DonorForm");
        let pf = with_object_schema(Some(schema()), || patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes())).unwrap();
        assert_only_authorized_changes(s.source(), &pf);
        pf
    }

    fn refusal(edited: &str) -> String {
        let s = stream("DonorForm");
        let e = with_object_schema(Some(schema()), || plan(&s, edited)).expect_err("must be refused");
        assert_eq!(e.code, GENERATED_FILE_MODIFIED, "{e}");
        format!("{e} {}", e.params.get("problems").map(|p| p.to_string()).unwrap_or_default())
    }

    fn link_of(bytes: &[u8], id: i64) -> Option<String> {
        let s = parse_spanned(bytes).unwrap();
        let links = s.root.node(2)?.node(3)?;
        for item in links.children.iter().skip(1) {
            let oneform::brace::Item::Node(row) = item else { continue };
            if row.children[0].as_atom().and_then(|a| a.value.parse::<i64>().ok()) == Some(id) {
                let mut out = Vec::new();
                oneform::donor::write_node(row.node(1)?, &mut out);
                return Some(String::from_utf8(out).unwrap().split_whitespace().collect());
            }
        }
        None
    }

    fn decoded_xml(raw: &[u8]) -> String {
        let base = baseline("DonorForm");
        let doc = roxmltree::Document::parse(&base).unwrap();
        let (id, fname) = (doc.root_element().attribute("id").unwrap(), doc.root_element().attribute("name").unwrap());
        let model = decode_oof_stream(raw, id, Some(&lookup())).unwrap();
        let xml = render_form_xml(&common::form_info(id, fname, raw), &model).unwrap();
        String::from_utf8(enrich_ext_properties(raw, &xml).unwrap()).unwrap().replace("\r\n", "\n")
    }

    /// The table record (`control[2]`) of control `id` in `bytes`.
    fn table_info(bytes: &[u8], id: i64) -> oneform::brace::Node {
        let reparsed = parse_spanned(bytes).unwrap();
        let index = ControlIndex::build(&reparsed).unwrap();
        let info = index.get(id).unwrap().control.node.node(2).unwrap().clone();
        info
    }

    /// The `<Column name="Количество">` block of the `extract` of `bytes` where its three steps (data, check box,
    /// picture) are replaced by `steps`.
    fn quantity_column(bytes: &[u8], steps: &str) -> String {
        let text = String::from_utf8_lossy(bytes).to_string();
        let was = format!("{{0,{QTY}}},{{-1}},{{-1}}}}");
        assert_eq!(text.matches(&was).count(), 1);
        let xml = decoded_xml(text.replace(&was, &format!("{steps}}}")).as_bytes());
        let at = xml.find("<Column name=\"Количество\"").expect("the column");
        xml[at..at + xml[at..].find("</Column>").unwrap()].to_string()
    }

    #[test]
    fn column_steps_are_shown_as_data_check_box_and_picture_paths() {
        let pf = patch_with(&with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&good_columns()))));
        let (q, n, path) = (QTY, NOTE, "ОбработкаОбъект.Товары");
        let unknown = "99999999-0000-0000-0000-000000000009";
        let data = |tag: &str, steps: &str, name: &str| format!("<{tag} attributeId=\"1\" steps=\"{steps}\">{path}.{name}</{tag}>");
        // (steps in the stream, elements that must be there, tags that must not)
        let cases: [(String, Vec<String>, Vec<&str>); 7] = [
            (format!("{{0,{q}}},{{-1}},{{-1}}"), vec![data("DataPath", q, "Количество")], vec!["CheckBoxDataPath", "PictureDataPath"]),
            // the check-box step does not stand in for the data step
            (format!("{{-1}},{{0,{q}}},{{-1}}"), vec![data("CheckBoxDataPath", q, "Количество")], vec!["<DataPath", "PictureDataPath"]),
            (format!("{{-1}},{{-1}},{{0,{q}}}"), vec![data("PictureDataPath", q, "Количество")], vec!["<DataPath", "CheckBoxDataPath"]),
            (
                format!("{{0,{n}}},{{-2}},{{0,{q}}}"),
                vec![data("DataPath", n, "Комментарий"), data("CheckBoxDataPath", "-2", "НомерСтроки"), data("PictureDataPath", q, "Количество")],
                vec![],
            ),
            (format!("{{2,{q}}},{{-1}},{{-1}}"), vec![data("DataPath", &format!("2:{q}"), "Количество")], vec!["CheckBoxDataPath"]),
            (format!("{{0,{unknown}}},{{-1}},{{-1}}"), vec![data("DataPath", unknown, &format!("[{unknown}]"))], vec![]),
            ("{-1},{-1},{-1}".to_string(), vec![], vec!["<DataPath", "CheckBoxDataPath", "PictureDataPath"]),
        ];
        for (steps, want, absent) in cases {
            let block = quantity_column(&pf.bytes, &steps);
            for w in &want {
                assert!(block.contains(w.as_str()), "{steps}: {w} is missing in
{block}");
            }
            for tag in absent {
                assert!(!block.contains(tag), "{steps}: {tag} must not be there
{block}");
            }
        }
        // the data step of the row-number column
        let xml = decoded_xml(&pf.bytes);
        assert!(xml.contains(&data("DataPath", "-2", "НомерСтроки")), "{xml}");
    }

    #[test]
    fn an_added_table_is_bound_to_a_tabular_section_with_its_columns() {
        let edited = with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&good_columns())));
        let pf = patch_with(&edited);
        let id = pf.allocated[0].1;
        assert_eq!(link_of(&pf.bytes, id).as_deref(), Some(format!("{{2,{{1}},{{0,{GOODS}}}}}").as_str()));

        let (value_type, row_source, columns) = read_table(&table_info(&pf.bytes, id)).unwrap();
        assert_eq!(value_type, format!("{{\"Pattern\",{{\"#\",{GOODS_TYPE}}}}}"));
        assert_eq!(row_source, oneform::ts_table::ROW_SOURCE);
        let got: Vec<(&str, i64, i64, &str, usize)> = columns.iter().map(|c| (c.name.as_str(), c.id, c.ordinal, c.data.as_str(), c.slots)).collect();
        let (d1, d2, d3) = (format!("{{0,{NAME}}}"), format!("{{0,{QTY}}}"), format!("{{0,{NOTE}}}"));
        assert_eq!(
            got,
            [
                ("НомерСтроки", 0, -2, "{-2}", 45),
                ("Наименование", 1, 0, d1.as_str(), 46),
                ("Количество", 2, 1, d2.as_str(), 46),
                ("Комментарий", 7, 2, d3.as_str(), 45),
            ]
        );
        // a read-only column has no editor: no editor guid, no editor blob, the flags of an editable column
        let info = table_info(&pf.bytes, id);
        let column = |i: usize| info.node(2).unwrap().node(1).unwrap().node(23).unwrap().node(i).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap().clone();
        let (editable, read_only, row_number) = (column(2), column(4), column(1));
        assert_eq!(read_only.atom(38).unwrap().value, "00000000-0000-0000-0000-000000000000");
        assert_eq!(editable.atom(38).unwrap().value, "381ed624-9217-4e63-85db-c4c3cb87daae");
        assert_eq!((read_only.atom(26).unwrap().value.as_str(), read_only.atom(9).unwrap().value.as_str()), ("0", "12590592"));
        assert_eq!(read_only.atom(6).unwrap().value, "2");
        assert_eq!(read_only.atom(44).unwrap().value, "90");
        assert_eq!(oneform_text(read_only.node(35).unwrap()), "{\"Pattern\",{\"S\",50,1}}");
        assert_eq!(row_number.len(), 45);

        // `extract` of the patched stream shows the same table and columns
        let xml = decoded_xml(&pf.bytes);
        let at = xml.find("<Table name=\"Товары\"").expect("the table is extracted");
        let block = strip_steps(&xml[at..at + xml[at..].find("</Table>").unwrap()]);
        for want in [
            "<DataPath attributeId=\"1\">ОбработкаОбъект.Товары</DataPath>",
            "<Column name=\"НомерСтроки\" id=\"0\">",
            "<DataPath attributeId=\"1\">ОбработкаОбъект.Товары.НомерСтроки</DataPath>",
            "<Column name=\"Наименование\" id=\"1\">",
            "<DataPath attributeId=\"1\">ОбработкаОбъект.Товары.Наименование</DataPath>",
            "<Column name=\"Количество\" id=\"2\">",
            "<DataPath attributeId=\"1\">ОбработкаОбъект.Товары.Количество</DataPath>",
            "<Column name=\"Комментарий\" id=\"7\">",
            "<DataPath attributeId=\"1\">ОбработкаОбъект.Товары.Комментарий</DataPath>",
        ] {
            assert!(block.contains(want), "{want} is missing in:\n{block}");
        }
        let positions: Vec<usize> = ["НомерСтроки\" id", "Наименование\" id", "Количество\" id", "Комментарий\" id"]
            .iter()
            .map(|n| block.find(n).unwrap())
            .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "the columns keep their order");
        let parsed = parse_form_xml(xml.as_bytes(), "decoded").unwrap();
        let t = parsed.controls.iter().find(|c| c.name == "Товары").unwrap();
        let cols: Vec<(&str, bool, bool, i64)> = t.values.columns.iter().map(|c| (c.name.as_str(), c.read_only, c.visible, c.width)).collect();
        assert_eq!(
            cols,
            [("НомерСтроки", true, true, 90), ("Наименование", false, true, 90), ("Количество", false, true, 90), ("Комментарий", true, false, 90)]
        );
        // the extracted columns equal the requested ones: an editable column without `<Editor>` is an InputField,
        // a read-only one has none
        let requested = column_values(edited.as_bytes(), "Товары");
        assert_eq!(requested, column_values(xml.as_bytes(), "Товары"));
        let editors: Vec<Option<&str>> = requested.iter().map(|c| c.3.as_deref()).collect();
        assert_eq!(editors, [None, Some("InputField"), Some("InputField"), None]);
    }

    /// The schema of a processor container: the patterns of the columns are known, the kinds of the references are not.
    fn schema_without_kinds() -> ObjectSchema {
        let mut s = schema();
        for t in &mut s.tabular {
            t.column_ref_kinds = vec![None; t.columns.len()];
        }
        s
    }

    #[test]
    fn a_read_only_reference_column_needs_no_kind_and_an_editable_one_is_refused_with_the_way_out() {
        let s = stream("DonorForm");
        let columns = col("Ссылка", 1, "ОбработкаОбъект.Товары.Ссылка", "<ReadOnly>true</ReadOnly>");
        let edited = with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&columns)));
        let pf = with_object_schema(Some(schema_without_kinds()), || patch_form(&s, baseline("DonorForm").as_bytes(), edited.as_bytes())).unwrap();
        let info = table_info(&pf.bytes, pf.allocated[0].1);
        let (_, _, got) = read_table(&info).unwrap();
        assert_eq!(got.iter().map(|c| (c.name.as_str(), c.slots)).collect::<Vec<_>>(), [("Ссылка", 45)]);

        let columns = col("Ссылка", 1, "ОбработкаОбъект.Товары.Ссылка", "<Editor>InputField</Editor>");
        let edited = with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&columns)));
        let e = with_object_schema(Some(schema_without_kinds()), || plan(&s, &edited)).expect_err("must be refused");
        let text = format!("{e} {}", e.params.get("problems").map(|p| p.to_string()).unwrap_or_default());
        assert!(text.contains("does not tell its kind") && text.contains("ONEFORM_CONFIG_ROOT") && text.contains("read-only"), "{text}");
    }

    #[test]
    fn reference_columns_are_built_with_the_pattern_and_the_editor_of_their_kind() {
        let columns = [
            col("Ссылка", 1, "ОбработкаОбъект.Товары.Ссылка", ""),
            col("Вид", 2, "ОбработкаОбъект.Товары.Вид", "<Editor>InputField</Editor>"),
            col("ВидТолькоЧтение", 3, "ОбработкаОбъект.Товары.Вид", "<ReadOnly>true</ReadOnly>"),
        ]
        .concat();
        let edited = with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&columns)));
        let pf = patch_with(&edited);
        let info = table_info(&pf.bytes, pf.allocated[0].1);
        let (_, _, got) = read_table(&info).unwrap();
        let (d1, d2) = (format!("{{0,{ITEM}}}"), format!("{{0,{KIND}}}"));
        assert_eq!(
            got.iter().map(|c| (c.name.as_str(), c.ordinal, c.data.as_str(), c.slots)).collect::<Vec<_>>(),
            [("Ссылка", 5, d1.as_str(), 46), ("Вид", 6, d2.as_str(), 46), ("ВидТолькоЧтение", 6, d2.as_str(), 45)]
        );
        let props = |i: usize| info.node(2).unwrap().node(1).unwrap().node(23).unwrap().node(i).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap().clone();
        for (i, type_id, params) in [(1, ITEM_TYPE, "8585207b-9ca7-425b-9385-e8fd67c4e148"), (2, KIND_TYPE, "30ee7b7c-8dac-4e20-b58e-e7f7742ae8d8")] {
            let p = props(i);
            let pattern = format!("{{\"Pattern\",{{\"#\",{type_id}}}}}");
            assert_eq!(oneform_text(p.node(35).unwrap()), pattern);
            assert_eq!(p.atom(38).unwrap().value, "381ed624-9217-4e63-85db-c4c3cb87daae");
            let blob = p.node(39).unwrap().atom(0).unwrap();
            let text = oneform::ts_table::decode_blob(&blob.value).expect("the editor blob has a valid header and length prefix");
            let text: String = text.split_whitespace().collect();
            assert!(text.starts_with(&format!("{{9,{pattern},")), "{text}");
            assert!(text.contains(&format!("{{1,{{{params},")), "{text}");
        }
        assert_eq!(oneform_text(props(3).node(35).unwrap()), format!("{{\"Pattern\",{{\"#\",{KIND_TYPE}}}}}"));

        // `extract` of the patched stream shows the same columns
        let xml = decoded_xml(&pf.bytes);
        let requested = column_values(edited.as_bytes(), "Товары");
        assert_eq!(requested, column_values(xml.as_bytes(), "Товары"));
        assert_eq!(requested.iter().map(|c| c.3.as_deref()).collect::<Vec<_>>(), [Some("InputField"), Some("InputField"), None]);
    }

    #[test]
    fn a_read_only_field_column_has_no_editor_and_reads_back() {
        let columns = [
            col("НомерСтроки", 0, "ОбработкаОбъект.Товары.НомерСтроки", "<ReadOnly>true</ReadOnly>"),
            col("Количество", 2, "ОбработкаОбъект.Товары.Количество", "<ReadOnly>false</ReadOnly>"),
            col("Комментарий", 7, "ОбработкаОбъект.Товары.Комментарий", "<ReadOnly>true</ReadOnly>"),
            col("Наименование", 8, "ОбработкаОбъект.Товары.Наименование", ""),
        ]
        .concat();
        let edited = with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&columns)));
        let pf = patch_with(&edited);
        let (_, _, got) = read_table(&table_info(&pf.bytes, pf.allocated[0].1)).unwrap();
        assert_eq!(got.iter().map(|c| c.slots).collect::<Vec<_>>(), [45, 46, 45, 46]);
        let xml = decoded_xml(&pf.bytes);
        let requested = column_values(edited.as_bytes(), "Товары");
        assert_eq!(requested, column_values(xml.as_bytes(), "Товары"));
        assert_eq!(requested[2], ("Комментарий".to_string(), Some(7), true, None, true, 90));
        assert_eq!(requested[1].3.as_deref(), Some("InputField"));
        assert_eq!(requested[3].3.as_deref(), Some("InputField"));
    }

    #[test]
    fn an_added_table_without_columns_is_bound() {
        let pf = patch_with(&with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", None)));
        let id = pf.allocated[0].1;
        let (_, _, columns) = read_table(&table_info(&pf.bytes, id)).unwrap();
        assert!(columns.is_empty());
        assert_eq!(link_of(&pf.bytes, id).as_deref(), Some(format!("{{2,{{1}},{{0,{GOODS}}}}}").as_str()));
    }

    #[test]
    fn a_table_without_a_data_path_keeps_the_donor_record() {
        let pf = patch_with(&with_table(&table("Товары", 900, "", None)));
        let id = pf.allocated[0].1;
        assert_eq!(link_of(&pf.bytes, id), None);
        let (value_type, _, columns) = read_table(&table_info(&pf.bytes, id)).unwrap();
        assert!(!value_type.contains(GOODS_TYPE) && columns.len() == 2, "{value_type}");
    }

    #[test]
    fn unsupported_tables_and_columns_are_refused_with_the_line_and_the_reason() {
        let one = |path: &str, extra: &str| col("Колонка", 1, path, extra);
        let goods = "ОбработкаОбъект.Товары";
        let cases: Vec<(String, &str)> = vec![
            (table("Т", 900, "ОбработкаОбъект.НетТакой", None), "no attribute or standard attribute 'НетТакой'"),
            (table("Т", 900, "ОбработкаОбъект.Дубль", None), "several tabular sections named 'Дубль'"),
            (table("Т", 900, "ОбработкаОбъект.Товары.Наименование", None), "three or more segments"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Нет", ""))), "the column 'Нет' does not exist in the tabular section 'Товары'"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Контакты.Значение", ""))), "belongs to the tabular section 'Контакты'"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Флаг", ""))), "Boolean"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Свободный", ""))), "unlimited"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Чужая", ""))), "value type of the column 'Чужая' is not known"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Узел", ""))), "kind ExchangePlanRef"),
            (table("Т", 900, goods, Some(&one("Товары.Наименование", ""))), "must be ОбработкаОбъект.Товары.<column>"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Наименование", "<Editor>CheckBox</Editor>"))), "editor CheckBox"),
            (
                table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Наименование", "<ReadOnly>true</ReadOnly><Editor>InputField</Editor>"))),
                "a read-only column has no editor",
            ),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.НомерСтроки", "<Editor>InputField</Editor>"))), "НомерСтроки is read-only and has no editor"),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.НомерСтроки", ""))), "НомерСтроки is read-only and has no editor"),
            (
                table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Наименование", "<Events><OnChange id=\"2147483647\">Изм</OnChange></Events>"))),
                "<Events> is not supported",
            ),
            (table("Т", 900, goods, Some(&one("ОбработкаОбъект.Товары.Наименование", "<ValueType kind=\"String\"/>"))), "<ValueType> is not supported"),
            (
                table(
                    "Т",
                    900,
                    goods,
                    Some(&[col("А", 1, "ОбработкаОбъект.Товары.Наименование", ""), col("Б", 1, "ОбработкаОбъект.Товары.Количество", "")].concat()),
                ),
                "the id 1 is used by another column",
            ),
            (table("Т", 900, "", Some(&one("ОбработкаОбъект.Товары.Наименование", ""))), "this element is not supported on an added control"),
        ];
        for (block, why) in cases {
            let text = refusal(&with_table(&block));
            assert!(text.contains(why), "{why}: {text}");
            assert!(text.contains("line "), "{why}: {text}");
        }
        // a tabular section is not a field: only a Table can be bound to it
        let field = "<InputField name=\"Поле\" id=\"901\"><DataPath attributeId=\"1\">ОбработкаОбъект.Товары</DataPath><Left>520</Left><Top>500</Top><Width>120</Width><Height>20</Height><ZOrder>55</ZOrder><TabOrder>55</TabOrder></InputField>\n        ";
        let text = refusal(&with_table(field));
        assert!(text.contains("only a Table can be bound"), "{text}");
    }
    // ------------------------------------------------------------------ columns of an existing table

    /// The stream with the table `Товары` (id 37) added and its `Form.xml` as `extract` shows it: the starting point
    /// of the edits of an existing table.
    struct Existing {
        first: PatchedForm,
        xml: String,
        id: i64,
    }

    fn existing() -> Existing {
        existing_with(&good_columns())
    }

    fn existing_with(columns: &str) -> Existing {
        let first = patch_with(&with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(columns))));
        let id = first.allocated[0].1;
        let xml = decoded_xml(&first.bytes);
        Existing { first, xml, id }
    }

    impl Existing {
        fn try_patch(&self, edited: &str) -> Result<PatchedForm, ExtractionError> {
            let s = parse_spanned(&self.first.bytes).unwrap();
            with_object_schema(Some(schema()), || patch_form(&s, self.xml.as_bytes(), edited.as_bytes()))
        }

        /// Patches the stream; only the authorized ranges change and the pack verification of the result passes.
        fn patch(&self, edited: &str) -> PatchedForm {
            let pf = self.try_patch(edited).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
            assert_only_authorized_changes(&self.first.bytes, &pf);
            self.verify_pack(edited, &pf);
            pf
        }

        fn refusal(&self, edited: &str) -> String {
            let e = self.try_patch(edited).expect_err("must be refused");
            assert_eq!(e.code, GENERATED_FILE_MODIFIED, "{e}");
            format!("{e} {}", e.params.get("problems").map(|p| p.to_string()).unwrap_or_default())
        }

        /// The staged-result check of `oneform pack`: the stream holds exactly the authorized changes and the
        /// `Form.xml` decoded from it holds the requested edit.
        fn verify_pack(&self, edited: &str, pf: &PatchedForm) {
            let layout = serde_json::json!({"xml": "Form.xml", "binary": {"path": "FormStream.bin"}});
            let tree = |xml: &str| -> oneform::transaction::Tree {
                [("Form.xml".to_string(), xml.as_bytes().to_vec()), ("FormStream.bin".to_string(), self.first.bytes.clone())].into_iter().collect()
            };
            let (current, existing) = (tree(&self.xml), tree(edited));
            let lp = with_object_schema(Some(schema()), || oneform::pack::plan_layout_patch(&layout, &existing, &current)).unwrap().expect("a layout edit");
            assert_eq!(lp.patched.bytes, pf.bytes);
            assert!(!lp.summary().is_empty(), "the pack report lists the edits, tables bound to a tabular section included");
            let mut staged = tree(&decoded_xml(&pf.bytes));
            staged.insert("FormStream.bin".to_string(), pf.bytes.clone());
            lp.verify_staged(&staged).unwrap_or_else(|e| panic!("{e} {:?}", e.params));
        }

        fn columns(&self, pf: &PatchedForm) -> Vec<oneform::ts_table::ColumnRead> {
            read_table(&table_info(&pf.bytes, self.id)).unwrap().2
        }
    }

    /// The `DataPath` of the column `column` set to `field` of the section `Товары`.
    fn rebound(xml: &str, column: &str, path: &str) -> String {
        let start = xml.find(&format!("<Column name=\"{column}\"")).expect("the column");
        let open = start + xml[start..].find("<DataPath attributeId=\"1\"").unwrap();
        let from = open + xml[open..].find('>').unwrap() + 1;
        let to = from + xml[from..].find("</DataPath>").unwrap();
        format!("{}{path}{}", &xml[..from], &xml[to..])
    }

    /// The text of the `<Column>` element named `column` (the whole lines).
    fn column_block(xml: &str, column: &str) -> (usize, usize) {
        let at = xml.find(&format!("<Column name=\"{column}\"")).expect("the column");
        let start = xml[..at].rfind('\n').map_or(0, |i| i + 1);
        let end = at + xml[at..].find("</Column>").unwrap() + "</Column>".len();
        let end = end + xml[end..].find('\n').map_or(0, |i| i + 1);
        (start, end)
    }

    fn without_column(xml: &str, column: &str) -> String {
        let (start, end) = column_block(xml, column);
        format!("{}{}", &xml[..start], &xml[end..])
    }

    fn with_column_after(xml: &str, column: Option<&str>, block: &str) -> String {
        let at = match column {
            Some(c) => column_block(xml, c).1,
            None => {
                let table = xml.find("<Table name=\"Товары\"").expect("the table");
                table + xml[table..].find("<Columns>").unwrap() + "<Columns>".len() + 1
            }
        };
        format!("{}{block}{}", &xml[..at], &xml[at..])
    }

    fn new_column(name: &str, id: i64, field: &str, extra: &str) -> String {
        col(name, id, &format!("ОбработкаОбъект.Товары.{field}"), extra)
    }

    fn step_props(c: &oneform::ts_table::ColumnRead) -> (String, i64, String, Option<String>) {
        (c.data.clone(), c.ordinal, c.pattern.clone(), c.editor_pattern.clone())
    }

    #[test]
    fn a_column_is_rebound_to_a_column_of_the_same_kind() {
        let e = existing();
        let before = e.columns(&e.first);
        // Наименование (String 20) -> Комментарий (String 50)
        let edited = rebound(&e.xml, "Наименование", "ОбработкаОбъект.Товары.Комментарий");
        let pf = e.patch(&edited);
        // data step, ordinal, pattern and the pattern inside the editor: nothing else
        assert_eq!(pf.replacements.len(), 4);
        let after = e.columns(&pf);
        assert_eq!(after.len(), 4);
        let (d, s50) = (format!("{{0,{NOTE}}}"), "{\"Pattern\",{\"S\",50,1}}".to_string());
        assert_eq!(step_props(&after[1]), (d, 2, s50.clone(), Some(s50)));
        for i in [0, 2, 3] {
            assert_eq!(after[i].record, before[i].record, "column {i} is untouched");
        }
        assert_eq!((after[1].id, after[1].name.as_str(), after[1].slots), (1, "Наименование", 46));
        // the editor blob is a valid one: the length prefix is the length of the new text
        let info = table_info(&pf.bytes, e.id);
        let blob = info.node(2).unwrap().node(1).unwrap().node(23).unwrap().node(2).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap().node(39).unwrap().atom(0).unwrap().value.clone();
        assert!(oneform::ts_table::decode_blob(&blob).is_some(), "the length prefix of the editor blob is wrong");
        // `extract` shows the new DataPath
        let xml = decoded_xml(&pf.bytes);
        let at = xml.find("<Column name=\"Наименование\"").unwrap();
        let head = strip_steps(&xml[at..at + 300]);
        assert!(head.contains("<DataPath attributeId=\"1\">ОбработкаОбъект.Товары.Комментарий</DataPath>"), "{head}");
        // the stale <ValueType> of the stub is tolerated: the column is rebound and the stub follows its field
        assert!(xml[at..].contains("length=\"50\""));
    }

    #[test]
    fn a_column_is_rebound_to_a_column_of_another_primitive_type() {
        let e = existing();
        let before = e.columns(&e.first);
        let mut edited = rebound(&e.xml, "Наименование", "ОбработкаОбъект.Товары.Количество");
        // the user may update the value type of the stub as well
        edited = edited.replacen("<ValueType kind=\"String\" composite=\"false\" length=\"20\"/>", "<ValueType kind=\"Number\" composite=\"false\" digits=\"15\" fractionDigits=\"2\"/>", 1);
        let pf = e.patch(&edited);
        let after = e.columns(&pf);
        let (d, n) = (format!("{{0,{QTY}}}"), "{\"Pattern\",{\"N\",15,2,0}}".to_string());
        assert_eq!(step_props(&after[1]), (d.clone(), 1, n.clone(), Some(n.clone())));
        assert_eq!(after[0].record, before[0].record);
        // a read-only column has no editor: three pieces
        let edited = rebound(&e.xml, "Комментарий", "ОбработкаОбъект.Товары.Количество");
        let pf = e.patch(&edited);
        assert_eq!(pf.replacements.len(), 3);
        let after = e.columns(&pf);
        assert_eq!((after[3].data.clone(), after[3].ordinal, after[3].pattern.clone(), after[3].slots, after[3].editor_pattern.clone()), (d, 1, n, 45, None));
    }

    #[test]
    fn a_column_is_added_in_the_middle_removed_and_both() {
        let e = existing();
        let before = e.columns(&e.first);
        // add: an editable reference column after Наименование, a free id
        let block = new_column("Ссылка", 9, "Ссылка", "");
        let edited = with_column_after(&e.xml, Some("Наименование"), &block);
        let pf = e.patch(&edited);
        assert_eq!(pf.replacements.len(), 1, "the list is one replacement");
        let after = e.columns(&pf);
        assert_eq!(after.iter().map(|c| (c.name.as_str(), c.id)).collect::<Vec<_>>(), [("НомерСтроки", 0), ("Наименование", 1), ("Ссылка", 9), ("Количество", 2), ("Комментарий", 7)]);
        for (got, was) in [(0, 0), (1, 1), (3, 2), (4, 3)] {
            assert_eq!(after[got].record, before[was].record, "column {got} keeps its bytes");
        }
        assert_eq!((after[2].slots, after[2].ordinal, after[2].data.clone()), (46, 5, format!("{{0,{ITEM}}}")));
        assert_eq!(after[2].pattern, format!("{{\"Pattern\",{{\"#\",{ITEM_TYPE}}}}}"));
        let xml = decoded_xml(&pf.bytes);
        assert!(xml.contains("<Column name=\"Ссылка\" id=\"9\">"), "{xml}");
        assert!(xml.find("name=\"Ссылка\"").unwrap() < xml.find("name=\"Количество\"").unwrap());

        // add at the start and as a read-only НомерСтроки-less field; the XML may carry a <ValueType>
        let block = new_column("Первая", 3, "Количество", "<ReadOnly>true</ReadOnly><ValueType kind=\"Number\" composite=\"false\" digits=\"15\" fractionDigits=\"2\"/>");
        let pf = e.patch(&with_column_after(&e.xml, None, &block));
        let after = e.columns(&pf);
        assert_eq!((after[0].name.as_str(), after[0].id, after[0].slots), ("Первая", 3, 45));
        assert_eq!(after[1].record, before[0].record);

        // remove: the others keep their bytes and ids
        let pf = e.patch(&without_column(&e.xml, "Количество"));
        let after = e.columns(&pf);
        assert_eq!(after.iter().map(|c| (c.name.as_str(), c.id)).collect::<Vec<_>>(), [("НомерСтроки", 0), ("Наименование", 1), ("Комментарий", 7)]);
        assert_eq!(after.iter().map(|c| c.record.clone()).collect::<Vec<_>>(), [0, 1, 3].iter().map(|i| before[*i].record.clone()).collect::<Vec<_>>());
        let xml = decoded_xml(&pf.bytes);
        assert!(!xml.contains("name=\"Количество\""));

        // remove the first and the last, add one: all in one list
        let edited = without_column(&without_column(&e.xml, "НомерСтроки"), "Комментарий");
        let edited = with_column_after(&edited, Some("Количество"), &new_column("Свободная", 2, "Комментарий", ""));
        let pf = e.try_patch(&edited);
        // id 2 is the column Количество: this is a changed column and not a new one
        assert!(pf.is_err());
        let edited = without_column(&without_column(&e.xml, "НомерСтроки"), "Комментарий");
        let edited = with_column_after(&edited, Some("Количество"), &new_column("Свободная", 5, "Комментарий", ""));
        let pf = e.patch(&edited);
        let after = e.columns(&pf);
        assert_eq!(after.iter().map(|c| (c.name.as_str(), c.id)).collect::<Vec<_>>(), [("Наименование", 1), ("Количество", 2), ("Свободная", 5)]);
        assert_eq!(after[0].record, before[1].record);

        // the order of the columns is the order of the XML
        let moved = without_column(&e.xml, "Комментарий");
        let kept = column_block(&e.xml, "Комментарий");
        let moved = with_column_after(&moved, None, &e.xml[kept.0..kept.1]);
        let pf = e.patch(&moved);
        let after = e.columns(&pf);
        assert_eq!(after.iter().map(|c| c.id).collect::<Vec<_>>(), [7, 0, 1, 2]);
        assert_eq!(after[0].record, before[3].record);
    }

    #[test]
    fn added_and_removed_columns_read_back_like_the_added_table_ones() {
        let e = existing();
        let fresh = e.patch(&with_column_after(&e.xml, Some("Количество"), &new_column("Вид", 4, "Вид", "<Editor>InputField</Editor>")));
        let cols = e.columns(&fresh);
        // the same record the table builder makes for an added table with this column
        let columns = [good_columns(), new_column("Вид", 4, "Вид", "<Editor>InputField</Editor>")].concat();
        let reference = patch_with(&with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&columns))));
        let (_, _, built) = read_table(&table_info(&reference.bytes, reference.allocated[0].1)).unwrap();
        assert_eq!(cols[3].record, built[4].record);
    }

    #[test]
    fn column_edits_that_cannot_be_written_are_refused_with_the_reason() {
        let e = existing();
        let goods = |f: &str| format!("ОбработкаОбъект.Товары.{f}");
        let cases: Vec<(String, &str)> = vec![
            // shape changes
            (rebound(&e.xml, "Наименование", &goods("НомерСтроки")), "НомерСтроки"),
            (rebound(&e.xml, "НомерСтроки", &goods("Наименование")), "НомерСтроки cannot be rebound"),
            (rebound(&e.xml, "Наименование", &goods("Флаг")), "Boolean"),
            // the editor of a reference differs from the editor of a string
            (rebound(&e.xml, "Наименование", &goods("Ссылка")), "does not fit"),
            // another section, unknown and not resolvable columns
            (rebound(&e.xml, "Наименование", "ОбработкаОбъект.Контакты.Значение"), "belongs to the tabular section 'Контакты'"),
            (rebound(&e.xml, "Наименование", &goods("Нет")), "does not exist"),
            (rebound(&e.xml, "Наименование", &goods("Чужая")), "not known"),
            (rebound(&e.xml, "Наименование", &goods("Свободный")), "unlimited"),
            (rebound(&e.xml, "Наименование", "Другой.Товары.Комментарий"), "starts with 'Другой'"),
            // a prefix that changes alone is not ignored
            (rebound(&e.xml, "Количество", "ОбработкаОбъект.Контакты.Количество"), "belongs to the tabular section 'Контакты'"),
            // new columns
            (with_column_after(&e.xml, None, &new_column("Н", 9, "Нет", "")), "does not exist"),
            (with_column_after(&e.xml, None, &new_column("Н", 9, "Флаг", "")), "Boolean"),
            (with_column_after(&e.xml, None, &col("Н", 9, "ОбработкаОбъект.Контакты.Значение", "")), "belongs to the tabular section 'Контакты'"),
            (with_column_after(&e.xml, None, &new_column("Н", 9, "Комментарий", "<Events><OnChange id=\"2147483647\">Изм</OnChange></Events>")), "<Events>"),
            (with_column_after(&e.xml, None, &new_column("Н", 9, "Комментарий", "<FooterText><Item language=\"ru\">Итог</Item></FooterText>")), "<FooterText>"),
            (with_column_after(&e.xml, None, &new_column("Н", 9, "НомерСтроки", "")), "НомерСтроки is read-only"),
            (with_column_after(&e.xml, None, &new_column("Н", 9, "Комментарий", "<Editor>CheckBox</Editor>")), "CheckBox"),
            (with_column_after(&e.xml, None, &new_column("Н", 9, "Комментарий", "<Foo/>")), "<Foo>"),
            (with_column_after(&e.xml, None, &new_column("Н", -1, "Комментарий", "")), "negative"),
            (with_column_after(&with_column_after(&e.xml, None, &new_column("Н", 9, "Комментарий", "")), None, &new_column("М", 9, "Комментарий", "")), "used by two columns"),
            // rebinding and adding in one run
            (
                with_column_after(&rebound(&e.xml, "Наименование", &goods("Комментарий")), None, &new_column("Н", 9, "Комментарий", "")),
                "edit columns in two steps",
            ),
            (with_column_after(&without_column(&e.xml, "Комментарий"), None, &new_column("Н", 9, "Комментарий", "")).replacen("name=\"Количество\" id=\"2\"", "name=\"Кол\" id=\"2\"", 1), "edit columns in two steps"),
        ];
        for (edited, why) in cases {
            let text = e.refusal(&edited);
            assert!(text.contains(why), "{why}: {text}");
        }
    }

    #[test]
    fn other_column_edits_of_a_section_table_need_no_object_metadata() {
        let e = existing();
        let (start, end) = column_block(&e.xml, "Количество");
        let block = e.xml[start..end].replacen("<Item language=\"ru\">Количество</Item>", "<Item language=\"ru\">Кол-во</Item>", 1);
        let edited = format!("{}{block}{}", &e.xml[..start], &e.xml[end..]);
        let s = parse_spanned(&e.first.bytes).unwrap();
        let pf = patch_form(&s, e.xml.as_bytes(), edited.as_bytes()).unwrap();
        assert_only_authorized_changes(&e.first.bytes, &pf);
        assert_eq!(pf.replacements.len(), 1);
        // a rebinding does need them
        let edited = rebound(&e.xml, "Наименование", "ОбработкаОбъект.Товары.Комментарий");
        assert!(patch_form(&s, e.xml.as_bytes(), edited.as_bytes()).is_err());
    }

    #[test]
    fn columns_of_other_tables_cannot_be_added_or_removed() {
        // the value-table table of the donor form: its columns are bound by name, not to a section
        let base = baseline("DonorForm");
        let s = stream("DonorForm");
        let edited = base.replacen("</Columns>", "<Column name=\"Колонка3\" id=\"7\"><Header><Item language=\"ru\">К3</Item></Header><Width>90</Width></Column></Columns>", 1);
        assert_ne!(edited, base);
        let err = with_object_schema(Some(schema()), || plan_form_patch(&s, base.as_bytes(), edited.as_bytes()).and_then(|p| p.apply())).expect_err("must be refused");
        assert_eq!(err.code, GENERATED_FILE_MODIFIED, "{err}");
        // remove the first column of the value table
        let start = base.find("<Column name=\"Колонка1\"").unwrap();
        let end = start + base[start..].find("</Column>").unwrap() + "</Column>".len();
        let edited = format!("{}{}", &base[..start], &base[end..]);
        let err = with_object_schema(Some(schema()), || plan_form_patch(&s, base.as_bytes(), edited.as_bytes()).and_then(|p| p.apply())).expect_err("must be refused");
        assert_eq!(err.code, GENERATED_FILE_MODIFIED, "{err}");
    }

    // ------------------------------------------------------------------ Boolean columns: check box data

    /// A column bound to a Boolean field of the section as check box data, the way `extract` shows such columns.
    fn check_col(name: &str, id: i64, path: &str, extra: &str) -> String {
        format!(
            "<Column name=\"{name}\" id=\"{id}\"><CheckBoxDataPath attributeId=\"1\">{path}</CheckBoxDataPath><Header><Item language=\"ru\">{name}</Item></Header><Width>60</Width><Visible>false</Visible><ReadOnly>true</ReadOnly>{extra}</Column>\n"
        )
    }

    fn flag_columns() -> String {
        [good_columns(), check_col("Флаг", 3, "ОбработкаОбъект.Товары.Флаг", "")].concat()
    }

    fn check_element(steps: Option<&str>, path: &str) -> String {
        let steps = steps.map(|s| format!(" steps=\"{s}\"")).unwrap_or_default();
        format!("<CheckBoxDataPath attributeId=\"1\"{steps}>{path}</CheckBoxDataPath>")
    }

    /// The `CheckBoxDataPath` element of the column `column` replaced by `element`.
    fn with_check_element(xml: &str, column: &str, element: &str) -> String {
        let start = xml.find(&format!("<Column name=\"{column}\"")).expect("the column");
        let open = start + xml[start..].find("<CheckBoxDataPath").unwrap();
        let close = open + xml[open..].find("</CheckBoxDataPath>").unwrap() + "</CheckBoxDataPath>".len();
        format!("{}{element}{}", &xml[..open], &xml[close..])
    }

    /// The property record `P` of column `i` (0-based) of the table `id`.
    fn column_props(bytes: &[u8], id: i64, i: usize) -> oneform::brace::Node {
        let info = table_info(bytes, id);
        info.node(2).unwrap().node(1).unwrap().node(23).unwrap().node(1 + i).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap().clone()
    }

    #[test]
    fn an_added_table_gets_a_check_box_column_for_a_boolean_field() {
        let edited = with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&flag_columns())));
        let pf = patch_with(&edited);
        let id = pf.allocated[0].1;
        let (_, _, columns) = read_table(&table_info(&pf.bytes, id)).unwrap();
        let flag = &columns[4];
        assert_eq!((flag.name.as_str(), flag.id, flag.slots), ("Флаг", 3, 45));
        // the field is in P[7], the data step is not set, the check box step is the column
        assert_eq!((flag.ordinal, flag.check_ordinal), (-1, 3));
        assert_eq!((flag.data.as_str(), flag.picture_data.as_str()), ("{-1}", "{-1}"));
        assert_eq!(flag.check_data, format!("{{0,{FLAG}}}"));
        assert_eq!(flag.pattern, "{\"Pattern\",{\"S\",10,1}}");
        assert_eq!((flag.editor_guid.as_str(), flag.editor_pattern.as_deref()), ("00000000-0000-0000-0000-000000000000", None));
        let props = column_props(&pf.bytes, id, 4);
        let slots: Vec<&str> = [4, 5, 6, 7, 8, 9, 25, 26, 27, 29, 32, 44].iter().map(|i| props.atom(*i).unwrap().value.as_str()).collect();
        assert_eq!(slots, ["60", "3", "-1", "3", "-1", "12590592", "0", "0", "0", "0", "9", "60"]);
        // the data columns are as before
        assert_eq!(columns[1].check_data, "{-1}");
        assert_eq!(columns[1].check_ordinal, -1);

        // `extract` shows the same CheckBoxDataPath and no DataPath on the column
        let xml = decoded_xml(&pf.bytes);
        let (start, end) = column_block(&xml, "Флаг");
        let block = &xml[start..end];
        assert!(block.contains(&format!("<CheckBoxDataPath attributeId=\"1\" steps=\"{FLAG}\">ОбработкаОбъект.Товары.Флаг</CheckBoxDataPath>")), "{block}");
        assert!(!block.contains("<DataPath") && !block.contains("PictureDataPath"), "{block}");
        assert!(block.contains("<Visible>false</Visible>") && block.contains("<ReadOnly>true</ReadOnly>"), "{block}");
        assert_eq!(column_values(edited.as_bytes(), "Товары"), column_values(xml.as_bytes(), "Товары"));

        // `steps` is optional on input; when it is there it is the uuid of the column
        let with_steps = edited.replace("<CheckBoxDataPath attributeId=\"1\">", &format!("<CheckBoxDataPath attributeId=\"1\" steps=\"{FLAG}\">"));
        assert_ne!(with_steps, edited);
        assert_eq!(patch_with(&with_steps).bytes, pf.bytes);
    }

    #[test]
    fn a_check_box_column_is_added_to_an_existing_table_and_removed() {
        let e = existing();
        let before = e.columns(&e.first);
        let block = check_col("Флаг", 3, "ОбработкаОбъект.Товары.Флаг", "");
        let pf = e.patch(&with_column_after(&e.xml, Some("Наименование"), &block));
        assert_eq!(pf.replacements.len(), 1, "the list is one replacement");
        let after = e.columns(&pf);
        assert_eq!(
            after.iter().map(|c| (c.name.as_str(), c.id)).collect::<Vec<_>>(),
            [("НомерСтроки", 0), ("Наименование", 1), ("Флаг", 3), ("Количество", 2), ("Комментарий", 7)]
        );
        for (got, was) in [(0, 0), (1, 1), (3, 2), (4, 3)] {
            assert_eq!(after[got].record, before[was].record, "column {got} keeps its bytes");
        }
        // the record of the table builder
        let reference = patch_with(&with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(&flag_columns()))));
        let (_, _, built) = read_table(&table_info(&reference.bytes, reference.allocated[0].1)).unwrap();
        assert_eq!(after[2].record, built[4].record);
        let xml = decoded_xml(&pf.bytes);
        assert!(xml.contains(&format!("<CheckBoxDataPath attributeId=\"1\" steps=\"{FLAG}\">")), "{xml}");

        // removed again: the other columns keep their bytes
        let e = existing_with(&flag_columns());
        let before = e.columns(&e.first);
        assert_eq!(before.len(), 5);
        let pf = e.patch(&without_column(&e.xml, "Флаг"));
        let after = e.columns(&pf);
        assert_eq!(after.iter().map(|c| c.record.clone()).collect::<Vec<_>>(), before[..4].iter().map(|c| c.record.clone()).collect::<Vec<_>>());
        // another Boolean column is added next to it
        let block = check_col("Признак", 4, "ОбработкаОбъект.Товары.Признак", "");
        let pf = e.patch(&with_column_after(&e.xml, Some("Флаг"), &block));
        let after = e.columns(&pf);
        assert_eq!((after[5].name.as_str(), after[5].check_data.clone(), after[5].check_ordinal), ("Признак", format!("{{0,{MARK}}}"), 9));
    }

    #[test]
    fn a_check_box_column_is_rebound_to_another_boolean_column_of_the_section() {
        let e = existing_with(&flag_columns());
        let before = e.columns(&e.first);
        assert!(e.xml.contains(&format!("<CheckBoxDataPath attributeId=\"1\" steps=\"{FLAG}\">")), "{}", e.xml);
        let path = "ОбработкаОбъект.Товары.Признак";
        // the stale `steps` of the extract does not fit the new column; the new uuid or none does
        let text = e.refusal(&with_check_element(&e.xml, "Флаг", &check_element(Some(FLAG), path)));
        assert!(text.contains("are not the uuid"), "{text}");
        let pf = e.patch(&with_check_element(&e.xml, "Флаг", &check_element(Some(MARK), path)));
        // the check box step and P[7]: nothing else
        assert_eq!(pf.replacements.len(), 2);
        assert_eq!(e.patch(&with_check_element(&e.xml, "Флаг", &check_element(None, path))).bytes, pf.bytes);
        let after = e.columns(&pf);
        assert_eq!((after[4].check_data.clone(), after[4].check_ordinal, after[4].ordinal, after[4].data.as_str()), (format!("{{0,{MARK}}}"), 9, -1, "{-1}"));
        assert_eq!((after[4].name.as_str(), after[4].id, after[4].slots, after[4].pattern.as_str()), ("Флаг", 3, 45, "{\"Pattern\",{\"S\",10,1}}"));
        for i in 0..4 {
            assert_eq!(after[i].record, before[i].record, "column {i} is untouched");
        }
        let xml = decoded_xml(&pf.bytes);
        let (start, end) = column_block(&xml, "Флаг");
        assert!(xml[start..end].contains(&format!("<CheckBoxDataPath attributeId=\"1\" steps=\"{MARK}\">{path}</CheckBoxDataPath>")), "{}", &xml[start..end]);
        // and back
        let back = Existing { first: pf, xml, id: e.id };
        let pf = back.patch(&with_check_element(&back.xml, "Флаг", &check_element(None, "ОбработкаОбъект.Товары.Флаг")));
        assert_eq!(back.columns(&pf)[4].record, before[4].record);
    }

    #[test]
    fn check_box_columns_that_cannot_be_written_are_refused_with_the_reason() {
        let goods = |f: &str| format!("ОбработкаОбъект.Товары.{f}");
        let flag = |extra: &str| check_col("Ф", 3, &goods("Флаг"), extra);
        let table_with = |columns: &str| with_table(&table("Товары", 900, "ОбработкаОбъект.Товары", Some(columns)));
        let cases: Vec<(String, &str)> = vec![
            (check_col("Ф", 3, &goods("Наименование"), ""), "not Boolean"),
            (check_col("Ф", 3, &goods("НомерСтроки"), ""), "not a Boolean column"),
            (check_col("Ф", 3, &goods("Нет"), ""), "does not exist"),
            (check_col("Ф", 3, "ОбработкаОбъект.Контакты.Значение", ""), "belongs to the tabular section 'Контакты'"),
            (check_col("Ф", 3, "Товары.Флаг", ""), "CheckBoxDataPath 'Товары.Флаг' must be ОбработкаОбъект.Товары.<column>"),
            (flag(&format!("<DataPath attributeId=\"1\">{}</DataPath>", goods("Флаг"))), "<DataPath> together with <CheckBoxDataPath>"),
            (flag(&format!("<PictureDataPath attributeId=\"1\">{}</PictureDataPath>", goods("Флаг"))), "picture path"),
            (flag("<Editor>InputField</Editor>"), "has no editor"),
            (flag("<Foo/>"), "<Foo>"),
            (flag("").replace("<Visible>false</Visible>", ""), "<Visible>false</Visible>"),
            (flag("").replace("<ReadOnly>true</ReadOnly>", ""), "<ReadOnly>true</ReadOnly>"),
            (flag("").replace("<CheckBoxDataPath attributeId=\"1\">", &format!("<CheckBoxDataPath attributeId=\"1\" steps=\"{NAME}\">")), "are not the uuid"),
            (flag("").replace("<CheckBoxDataPath attributeId=\"1\">", "<CheckBoxDataPath attributeId=\"2\">"), "attributeId must be 1"),
        ];
        for (columns, why) in cases {
            let text = refusal(&table_with(&columns));
            assert!(text.contains(why), "{why}: {text}");
            assert!(text.contains("line "), "{why}: {text}");
        }
        // existing tables
        let e = existing_with(&flag_columns());
        let path = goods("Признак");
        let data_of = |column: &str| {
            let start = e.xml.find(&format!("<Column name=\"{column}\"")).unwrap();
            let open = start + e.xml[start..].find("<DataPath").unwrap();
            (open, open + e.xml[open..].find("</DataPath>").unwrap() + "</DataPath>".len())
        };
        let (from, to) = data_of("Наименование");
        let cases: Vec<(String, &str)> = vec![
            // the shape of the column cannot change, both ways
            (format!("{}{}{}", &e.xml[..from], check_element(None, &goods("Флаг")), &e.xml[to..]), "<CheckBoxDataPath> is edited"),
            (with_check_element(&e.xml, "Флаг", &format!("<DataPath attributeId=\"1\">{}</DataPath>", goods("Флаг"))), "<CheckBoxDataPath> is edited"),
            (with_check_element(&e.xml, "Флаг", ""), "<CheckBoxDataPath> is edited"),
            (format!("{}{}{}", &e.xml[..to], check_element(None, &goods("Флаг")), &e.xml[to..]), "<CheckBoxDataPath> is edited"),
            // both paths on a column
            (
                with_check_element(
                    &e.xml,
                    "Флаг",
                    &format!("{}<DataPath attributeId=\"1\">{}</DataPath>", check_element(Some(FLAG), &goods("Флаг")), goods("Наименование")),
                ),
                "<DataPath> together with <CheckBoxDataPath>",
            ),
            // the target
            (with_check_element(&e.xml, "Флаг", &check_element(None, &goods("Количество"))), "not Boolean"),
            (with_check_element(&e.xml, "Флаг", &check_element(None, &goods("НомерСтроки"))), "not a Boolean column"),
            (with_check_element(&e.xml, "Флаг", &check_element(None, "ОбработкаОбъект.Контакты.Значение")), "belongs to the tabular section 'Контакты'"),
            (with_check_element(&e.xml, "Флаг", &check_element(None, &goods("Нет"))), "does not exist"),
            (with_check_element(&e.xml, "Флаг", &check_element(None, "Другой.Товары.Признак")), "starts with 'Другой'"),
            // the attribute and the steps alone
            (with_check_element(&e.xml, "Флаг", &format!("<CheckBoxDataPath attributeId=\"2\">{path}</CheckBoxDataPath>")), "<CheckBoxDataPath> is edited"),
            (with_check_element(&e.xml, "Флаг", &check_element(Some(MARK), &goods("Флаг"))), "<CheckBoxDataPath> is edited"),
            // new columns
            (with_column_after(&e.xml, None, &check_col("Н", 9, &path, "").replace("<Visible>false</Visible>", "")), "<Visible>false</Visible>"),
            (with_column_after(&e.xml, None, &check_col("Н", 9, &goods("Количество"), "")), "not Boolean"),
            (with_column_after(&e.xml, None, &check_col("Н", 9, &path, "<Events><OnChange id=\"2147483647\">Изм</OnChange></Events>")), "<Events>"),
            (with_column_after(&e.xml, None, &check_col("Н", 9, &path, &format!("<PictureDataPath attributeId=\"1\">{path}</PictureDataPath>"))), "picture path"),
            (
                with_column_after(&e.xml, None, &check_col("Н", 9, &path, &format!("<DataPath attributeId=\"1\">{path}</DataPath>"))),
                "<DataPath> together with <CheckBoxDataPath>",
            ),
        ];
        for (edited, why) in cases {
            let text = e.refusal(&edited);
            assert!(text.contains(why), "{why}: {text}");
        }
    }
}

// ---------------------------------------------------------------- command bar buttons (root group, sub-menus)

/// Patch `base` into `edited`, read the result back and return the decoded XML; the XML differs from the
/// baseline only by the `(from, to)` text replacements.
fn button_edit_round_trip(name: &str, edits: &[(&str, &str)]) -> PatchedForm {
    let base = baseline(name);
    let mut edited = base.clone();
    for (from, to) in edits {
        edited = replace_once(&edited, from, to);
    }
    let pf = round_trip(name, &edited);
    let mut expected = base;
    for (from, to) in edits {
        expected = replace_once(&expected, from, to);
    }
    assert_eq!(decode_xml(name, &pf.bytes), expected, "only the requested texts change");
    pf
}

#[test]
fn root_button_is_patched_when_the_root_group_is_not_last() {
    // `ДействияФормы`: the root group (b78f2e80,4) comes first, the standard sub-menu group (b78f2e80,55) last
    let pf = button_edit_round_trip(
        "MailGroupForm",
        &[("<Item language=\"#\">Подменю</Item>", "<Item language=\"#\">Меню действий</Item>")],
    );
    assert_eq!(pf.replacements.len(), 1);
}

#[test]
fn sub_menu_button_of_a_standard_sub_menu_is_patched_with_a_root_group_that_is_not_last() {
    button_edit_round_trip(
        "MailGroupForm",
        &[("<Item language=\"ru\">Редактировать код</Item>", "<Item language=\"ru\">Править код</Item>")],
    );
}

#[test]
fn sub_menu_button_caption_is_patched() {
    let ru = "<Item language=\"ru\">Действие1</Item>\n                    <Item language=\"ro\">Действие1</Item>\n                  </Caption>";
    let pf = button_edit_round_trip(
        "DonorForm",
        &[(ru, "<Item language=\"ru\">Другое действие</Item>\n                    <Item language=\"ro\">Действие1</Item>\n                  </Caption>")],
    );
    assert_eq!(pf.replacements.len(), 1);
}

#[test]
fn sub_menu_button_description_is_patched() {
    let base = baseline("DonorForm");
    let anchor = "<Button name=\"Действие1\" id=\"5\" kind=\"Action\" checked=\"false\" actionKind=\"Handler\" action=\"КоманднаяПанель2Действие1\">";
    assert_eq!(base.matches(anchor).count(), 1);
    let edited = base.replace(
        anchor,
        &format!("{anchor}\n                  <Description>\n                    <Item language=\"ru\">Пояснение</Item>\n                  </Description>"),
    );
    let pf = round_trip("DonorForm", &edited);
    assert_eq!(pf.replacements.len(), 1);
}

#[test]
fn sub_menu_button_checked_flag_is_patched() {
    let anchor = "<Button name=\"Действие\" id=\"4\" kind=\"Action\" checked=\"false\"";
    button_edit_round_trip("DonorForm", &[(anchor, "<Button name=\"Действие\" id=\"4\" kind=\"Action\" checked=\"true\"")]);
}

#[test]
fn buttons_without_the_checked_attribute_equal_the_baseline() {
    let base = baseline("DonorForm");
    let edited = base.replace(" checked=\"false\"", "");
    assert!(edited.len() < base.len());
    assert!(plan(&stream("DonorForm"), &edited).unwrap().is_empty());
    // and a missing attribute next to a real edit
    let edited = replace_once(
        &edited,
        "<Item language=\"ru\">Действие1</Item>\n                    <Item language=\"ro\">Действие1</Item>\n                  </Caption>",
        "<Item language=\"ru\">Другое</Item>\n                    <Item language=\"ro\">Действие1</Item>\n                  </Caption>",
    );
    let pf = patch("DonorForm", &edited).unwrap();
    assert_eq!(pf.replacements.len(), 1);
    // a wrong value is still not understood
    let bad = base.replacen(" checked=\"false\"", " checked=\"maybe\"", 1);
    assert!(plan(&stream("DonorForm"), &bad).is_err());
}

#[test]
fn buttons_of_a_shared_sub_menu_group_are_edited_together() {
    // two sub-menu buttons (depth 2 and depth 3) show the same group: an edit of one copy alone is refused, the same
    // edit of both is one replacement
    let name = "SharedSubmenuForm";
    let from = "<Item language=\"ru\">Все документы</Item>";
    let to = "<Item language=\"ru\">Все виды</Item>";
    let base = baseline(name);
    assert_eq!(base.matches(from).count(), 2);
    let one = base.replacen(from, to, 1);
    let (kind, text) = patch_rejected(name, &one);
    assert_eq!(kind, "verification-failed", "{text}");
    let both = base.replace(from, to);
    let pf = round_trip(name, &both);
    assert_eq!(pf.replacements.len(), 1);
    assert_eq!(decode_xml(name, &pf.bytes), both);
}

#[test]
fn nested_sub_menu_button_name_and_checked_flag_are_patched() {
    let name = "SharedSubmenuForm";
    let from = "<Button name=\"Действие6\" id=\"40\" kind=\"Action\" checked=\"false\"";
    let to = "<Button name=\"Поиск\" id=\"40\" kind=\"Action\" checked=\"true\"";
    button_edit_round_trip(name, &[(from, to)]);
}

// ---------------------------------------------------------------- command bar buttons: add, remove, reorder

mod command_bar_edit {
    use super::*;
    use oneform::form_patch::CommandBarButton;

    const V2: &str = "DonorFormV2";
    /// `КоманднаяПанель1` of the V2 donor: empty, AutoFill, format X9 / act7 / btn8.
    const EMPTY: &str = "КоманднаяПанель1";
    /// `ОсновныеДействияФормы`: a handler, a separator and `Standard:142`.
    const MAIN: &str = "ОсновныеДействияФормы";
    /// `КоманднаяПанель2`: handlers and a user sub-menu.
    const SUB: &str = "КоманднаяПанель2";

    fn raw(name: &str) -> Vec<u8> {
        std::fs::read(dir(name).join("FormStream.bin")).unwrap()
    }

    /// The V2 donor ships no `Form.xml`: the decoder's own projection is the baseline.
    fn decode(name: &str, bytes: &[u8]) -> String {
        if name != V2 {
            return decode_xml(name, bytes);
        }
        let model = decode_oof_stream(bytes, "x", None).unwrap();
        assert_eq!(model["warnings"], serde_json::json!([]));
        let xml = render_form_xml(&common::form_info("x", "Форма", bytes), &model).unwrap();
        String::from_utf8(enrich_ext_properties(bytes, &xml).unwrap()).unwrap().replace("\r\n", "\n")
    }

    fn base(name: &str) -> String {
        if name == V2 {
            decode(name, &raw(name))
        } else {
            baseline(name)
        }
    }

    fn run(name: &str, edited: &str) -> Result<PatchedForm, ExtractionError> {
        patch_form(&parse_spanned(&raw(name)).unwrap(), base(name).as_bytes(), edited.as_bytes())
    }

    fn panel_id(name: &str, panel: &str) -> i64 {
        let m = parse_form_xml(base(name).as_bytes(), "baseline").unwrap();
        m.controls.iter().find(|c| c.name == panel).unwrap().id
    }

    fn buttons_of(name: &str, bytes: &[u8], panel: &str) -> Vec<CommandBarButton> {
        let m = parse_form_xml(decode(name, bytes).as_bytes(), "decoded").unwrap();
        m.controls.iter().find(|c| c.name == panel).unwrap().values.buttons.clone()
    }

    fn panel_bounds(xml: &str, panel: &str) -> (usize, usize) {
        let start = xml.find(&format!("<CommandBar name=\"{panel}\"")).unwrap();
        (start, start + xml[start..].find("</CommandBar>").unwrap())
    }

    /// The `<Button>` elements of the `<Buttons>` of `panel` (top level only).
    fn blocks(xml: &str, panel: &str) -> Vec<String> {
        let (start, end) = panel_bounds(xml, panel);
        let part = &xml[start..end];
        let Some(open) = part.find("<Buttons>") else { return Vec::new() };
        let close = part.rfind("</Buttons>").unwrap();
        split_buttons(&part[open + "<Buttons>".len()..close])
    }

    /// The `<Button>` elements at the top level of `inner`.
    fn split_buttons(inner: &str) -> Vec<String> {
        let (mut out, mut depth, mut from) = (Vec::new(), 0usize, 0usize);
        let re = regex::Regex::new(r"<Button\b[^>]*?/>|<Button\b[^>]*>|</Button>").unwrap();
        for m in re.find_iter(inner) {
            let t = m.as_str();
            if t.starts_with("</") {
                depth -= 1;
                if depth == 0 {
                    out.push(inner[from..m.end()].to_string());
                }
            } else if t.ends_with("/>") {
                if depth == 0 {
                    out.push(t.to_string());
                }
            } else {
                if depth == 0 {
                    from = m.start();
                }
                depth += 1;
            }
        }
        out
    }

    /// The buttons nested in a sub-menu `block`.
    fn kids(block: &str) -> Vec<String> {
        match (block.find("<Buttons>"), block.rfind("</Buttons>")) {
            (Some(a), Some(b)) => split_buttons(&block[a + "<Buttons>".len()..b]),
            _ => Vec::new(),
        }
    }

    /// `block` with its nested buttons replaced by `list`.
    fn with_kids(block: &str, list: &[String]) -> String {
        let inner = format!("<Buttons>
{}
</Buttons>", list.join("
"));
        match (block.find("<Buttons>"), block.rfind("</Buttons>")) {
            (Some(a), Some(b)) => format!("{}{inner}{}", &block[..a], &block[b + "</Buttons>".len()..]),
            _ if block.ends_with("/>") => format!("{}>{inner}</Button>", &block[..block.len() - 2]),
            _ => block.replace("</Button>", &format!("{inner}</Button>")),
        }
    }

    /// `xml` with the `<Buttons>` of `panel` replaced by `list` (the element is created for an empty panel).
    fn set_buttons(xml: &str, panel: &str, list: &[String]) -> String {
        let (start, end) = panel_bounds(xml, panel);
        let part = &xml[start..end];
        let inner = format!("<Buttons>\n{}\n</Buttons>", list.join("\n"));
        let part = match (part.find("<Buttons>"), part.rfind("</Buttons>")) {
            (Some(a), Some(b)) => format!("{}{inner}{}", &part[..a], &part[b + "</Buttons>".len()..]),
            _ => format!("{part}{inner}\n"),
        };
        format!("{}{part}{}", &xml[..start], &xml[end..])
    }

    fn handler(name: &str, proc: &str, caption: &str) -> String {
        format!("<Button name=\"{name}\" kind=\"Action\" action=\"{proc}\"><Caption><Item language=\"ru\">{caption}</Item></Caption></Button>")
    }

    fn separator(name: &str) -> String {
        format!("<Button name=\"{name}\" kind=\"Separator\"/>")
    }

    fn names(list: &[CommandBarButton]) -> Vec<&str> {
        list.iter().map(|b| b.name.as_str()).collect()
    }

    /// What the stream of panel `id` says, read independently of the patcher.
    struct Table {
        t2: i64,
        n_act: usize,
        /// `(id, kind code)` of the root buttons in order
        root: Vec<(i64, String)>,
        pos: i64,
        subs: Vec<i64>,
        refs: Vec<i64>,
    }

    impl Table {
        fn ids(&self) -> Vec<i64> {
            self.root.iter().map(|r| r.0).collect()
        }
    }

    fn atom(n: &oneform::brace::Node, i: usize) -> String {
        n.atom(i).unwrap_or_else(|| panic!("atom {i} of {n:?}")).value.clone()
    }

    /// Reads the table and asserts the invariants of the layout (unique action references, counters, versions, tails).
    fn table(bytes: &[u8], id: i64) -> Table {
        let s = parse_spanned(bytes).unwrap();
        let idx = ControlIndex::build(&s).unwrap();
        let x = idx.get(id).unwrap().info().unwrap().node.node(1).unwrap();
        let t = x.node(7).unwrap();
        let (t1, t2, n_act) = (atom(t, 1), atom(t, 2).parse::<i64>().unwrap(), atom(t, 4).parse::<usize>().unwrap());
        let n_groups: usize = atom(t, 5 + n_act).parse().unwrap();
        assert_eq!(t.len(), 6 + n_act + n_groups, "table length");
        let mut guids = std::collections::HashSet::new();
        let acts: Vec<_> = (0..n_act).map(|i| t.node(5 + i).unwrap()).collect();
        for a in &acts {
            assert!(guids.insert(atom(a, 1)), "action guid listed twice");
        }
        let versions: std::collections::HashSet<String> = acts.iter().map(|a| atom(a, 0)).collect();
        assert!(versions.len() <= 1, "mixed action versions {versions:?}");
        let (mut used, mut ids, mut shapes) = (std::collections::HashSet::new(), Vec::new(), std::collections::HashSet::new());
        let key = (atom(x, 8), atom(x, 9));
        let mut out = None;
        for gi in 0..n_groups {
            let g = t.node(6 + n_act + gi).unwrap();
            let k: usize = atom(g, 4).parse().unwrap();
            assert_eq!(g.len(), 6 + 2 * k, "group length");
            for i in 0..k {
                let r = atom(g, 5 + 2 * i);
                assert!(guids.contains(&r), "unknown action {r}");
                assert!(used.insert(r), "action used twice");
                let b = g.node(6 + 2 * i).unwrap();
                assert_eq!(atom(b, 6), t1, "button carries the table guid");
                ids.push(atom(b, 7).parse::<i64>().unwrap());
                shapes.insert((atom(b, 0), b.len()));
            }
            if (atom(g, 1), atom(g, 2)) == key {
                let tail = g.node(5 + 2 * k).unwrap();
                let pos: i64 = atom(tail, 0).parse().unwrap();
                assert!((-1..=k as i64).contains(&pos), "tail position {pos} with {k} buttons");
                let nr: usize = atom(tail, 1).parse().unwrap();
                let sub = tail.node(2 + 2 * nr).unwrap();
                let ns: usize = atom(sub, 0).parse().unwrap();
                assert_eq!(sub.len(), 1 + 3 * ns);
                let root: Vec<(i64, String)> = (0..k)
                    .map(|i| {
                        let b = g.node(6 + 2 * i).unwrap();
                        (atom(b, 7).parse().unwrap(), atom(b, 9))
                    })
                    .collect();
                let subs: Vec<i64> = (0..ns).map(|i| atom(sub, 2 + 3 * i).parse().unwrap()).collect();
                let refs: Vec<i64> = (0..nr).map(|i| atom(tail, 3 + 2 * i).parse().unwrap()).collect();
                let order: Vec<usize> = subs.iter().map(|s| root.iter().position(|(id, _)| id == s).expect("sub-menu list names root buttons")).collect();
                assert!(order.windows(2).all(|w| w[0] < w[1]), "sub-menu list in button order");
                out = Some((root, pos, subs, refs));
            }
        }
        assert_eq!(used.len(), n_act, "one action per button");
        assert!(shapes.len() <= 1, "mixed button versions {shapes:?}");
        let mut seen = std::collections::HashSet::new();
        assert!(ids.iter().all(|i| seen.insert(*i)), "button ids are unique");
        assert!(ids.iter().all(|i| *i <= t2), "T2 {t2} covers every id {ids:?}");
        let (root, pos, subs, refs) = out.expect("root group");
        Table { t2, n_act, root, pos, subs, refs }
    }

    /// Patch, check the byte preservation, decode the result and return it with the original and patched tables.
    fn edit(name: &str, panel: &str, list: &[String]) -> (PatchedForm, Table, Table, Vec<CommandBarButton>) {
        let edited = set_buttons(&base(name), panel, list);
        let pf = run(name, &edited).unwrap_or_else(|e| panic!("{e}"));
        let original = raw(name);
        assert_only_authorized_changes(&original, &pf);
        assert_eq!(pf.replacements.len(), 1, "the table is one replacement");
        let id = panel_id(name, panel);
        let (before, after) = (table(&original, id), table(&pf.bytes, id));
        let got = buttons_of(name, &pf.bytes, panel);
        (pf, before, after, got)
    }

    fn refused(name: &str, panel: &str, list: &[String], needle: &str) {
        let edited = set_buttons(&base(name), panel, list);
        let Err(e) = run(name, &edited) else { panic!("must be refused: {needle}") };
        let text = e.to_string();
        assert!(text.contains(needle), "{needle}: {text}");
        assert_eq!(e.params["kind"].as_str(), Some("unsupported-button"), "{text}");
    }

    #[test]
    fn two_handlers_and_a_separator_are_added_to_the_empty_donor_panel() {
        let list = [
            handler("Выполнить", "ВыполнитьНажатие", "Выполнить"),
            separator("Разделитель"),
            "<Button name=\"Заполнить\" kind=\"Action\" action=\"ЗаполнитьНажатие\"><Caption><Item language=\"ru\">Заполнить</Item><Item language=\"ro\">Заполнить</Item></Caption><Tooltip><Item language=\"ru\">Заполнить таблицу</Item></Tooltip></Button>".to_string(),
        ];
        let (pf, before, after, got) = edit(V2, EMPTY, &list);
        assert_eq!((before.n_act, before.t2), (0, 30));
        assert_eq!((after.n_act, after.t2), (3, 33));
        assert_eq!(after.ids(), vec![31, 32, 33]);
        assert_eq!(after.root.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), vec!["0", "2", "0"]);
        assert_eq!(after.pos, 3, "the position that sat at the end of the empty group follows it");
        assert_eq!(names(&got), vec!["Выполнить", "Разделитель", "Заполнить"]);
        assert_eq!(got[0].id, Some(31));
        assert_eq!((got[0].kind.as_str(), got[0].action_kind.as_str(), got[0].action.as_str()), ("Action", "Handler", "ВыполнитьНажатие"));
        assert_eq!(got[1].kind, "Separator");
        assert_eq!(got[2].tooltip, vec![("ru".to_string(), "Заполнить таблицу".to_string())]);
        assert_eq!(got[2].caption.len(), 2);
        assert_eq!(pf.button_lists.len(), 1);
        assert_eq!(pf.button_lists[0].1.iter().map(|b| b.id.unwrap()).collect::<Vec<_>>(), vec![31, 32, 33]);
        // every new action guid appears twice in the stream (its record and its button) and nowhere in the original
        let before_text = String::from_utf8_lossy(&raw(V2)).to_lowercase();
        let after_text = String::from_utf8_lossy(&pf.bytes).to_lowercase();
        let re = regex::Regex::new(r"\{7,([0-9a-f-]{36}),1,e1692cc2").unwrap();
        let fresh: Vec<&str> = re.captures_iter(&after_text).map(|c| c.get(1).unwrap().as_str()).filter(|g| !before_text.contains(g)).collect();
        assert_eq!(fresh.len(), 2);
        for g in fresh {
            assert_eq!(after_text.matches(g).count(), 2, "guid {g}");
        }
    }

    #[test]
    fn the_added_panel_extracts_to_the_same_buttons_with_the_allocated_ids() {
        let list = [handler("Выполнить", "ВыполнитьНажатие", "Выполнить"), separator("Разделитель")];
        let (pf, ..) = edit(V2, EMPTY, &list);
        let xml = decode(V2, &pf.bytes);
        assert!(xml.contains("<Button name=\"Выполнить\" id=\"31\" kind=\"Action\" checked=\"false\" actionKind=\"Handler\" action=\"ВыполнитьНажатие\">"));
        assert!(xml.contains("<Button name=\"Разделитель\" id=\"32\" kind=\"Separator\" checked=\"false\"/>"));
        // the extracted XML against the patched stream is no edit
        let again = patch_form(&parse_spanned(&pf.bytes).unwrap(), xml.as_bytes(), xml.as_bytes()).unwrap();
        assert!(again.replacements.is_empty());
    }

    #[test]
    fn a_handler_is_added_in_the_middle_of_a_flat_panel() {
        let b = blocks(&base(V2), MAIN);
        assert_eq!(b.len(), 3);
        let list = [b[0].clone(), handler("Заполнить", "ЗаполнитьНажатие", "Заполнить"), b[1].clone(), b[2].clone()];
        let (_, before, after, got) = edit(V2, MAIN, &list);
        assert_eq!((before.n_act, before.t2), (3, 3));
        assert_eq!((after.n_act, after.t2), (4, 4));
        assert_eq!(after.ids(), vec![1, 4, 2, 3]);
        assert_eq!(names(&got), vec!["ОсновныеДействияФормыВыполнить", "Заполнить", "Разделитель", "ОсновныеДействияФормыЗакрыть"]);
        // the kept buttons decode as before
        let was = buttons_of(V2, &raw(V2), MAIN);
        assert_eq!(got[0], was[0]);
        assert_eq!(got[2], was[1]);
        assert_eq!(got[3], was[2]);
        assert_eq!((before.pos, after.pos), (-1, -1), "-1 stays");
    }

    /// `(code, body atoms 1..=3 and the origin group)` of every standard action of panel `id`, in table order.
    fn standard_actions(bytes: &[u8], id: i64) -> Vec<(i64, Vec<String>)> {
        let s = parse_spanned(bytes).unwrap();
        let idx = ControlIndex::build(&s).unwrap();
        let t = idx.get(id).unwrap().info().unwrap().node.node(1).unwrap().node(7).unwrap();
        let n_act: usize = atom(t, 4).parse().unwrap();
        (0..n_act)
            .map(|i| t.node(5 + i).unwrap())
            .filter(|a| atom(a, 3) == "fbe38877-b914-4fd5-8540-07dde06ba2e1")
            .map(|a| {
                let body = a.node(4).unwrap();
                assert_eq!((atom(a, 2), atom(body, 0), atom(a, 5)), ("1".into(), "6".into(), "0".into()), "{a:?}");
                let origin = body.node(4).unwrap();
                let mut v: Vec<String> = [1, 2].iter().map(|i| atom(body, *i)).collect();
                v.extend((1..=4).map(|i| atom(origin, i)));
                (atom(body, 3).parse().unwrap(), v)
            })
            .collect()
    }

    fn standard(name: &str, code: u32) -> String {
        format!("<Button name=\"{name}\" kind=\"Action\" action=\"Standard:{code}\"/>")
    }

    #[test]
    fn standard_commands_are_added_to_a_flat_and_to_an_empty_panel() {
        use oneform::cb_standard::{CODES, TEMPLATES};
        let template = |code: u32| {
            TEMPLATES.iter().find(|t| t.code == code && t.class == oneform::cb_standard::form_class("")).or_else(|| TEMPLATES.iter().find(|t| t.code == code && t.class.is_empty())).unwrap()
        };
        let caption = |code: u32| CODES.iter().find(|c| c.code == code).unwrap().caption.to_string();
        // a flat panel: the new commands go to the beginning and the end, a given caption is kept
        let b = blocks(&base(V2), MAIN);
        let own = "<Button name=\"Провести2\" kind=\"Action\" action=\"Standard:24\"><Caption><Item language=\"ru\">Провести документ</Item></Caption></Button>".to_string();
        let list = [own, b[0].clone(), b[1].clone(), b[2].clone(), standard("ОК2", 141)];
        let (pf, before, after, got) = edit(V2, MAIN, &list);
        assert_eq!((before.n_act, after.n_act, after.t2), (3, 5, 5));
        assert_eq!(after.ids(), vec![4, 1, 2, 3, 5]);
        assert_eq!((got[0].kind.as_str(), got[0].action_kind.as_str(), got[0].action.as_str()), ("Action", "Standard", "Standard:24"));
        assert_eq!(got[0].caption, vec![("ru".to_string(), "Провести документ".to_string())]);
        assert_eq!(got[4].action, "Standard:141");
        assert_eq!(got[4].caption, vec![("ru".to_string(), caption(141))], "the mined caption");
        assert!(got[4].tooltip.is_empty());
        let std = standard_actions(&pf.bytes, panel_id(V2, MAIN));
        assert_eq!(std.iter().map(|s| s.0).collect::<Vec<_>>(), vec![142, 24, 141]);
        for (code, atoms) in &std[1..] {
            let t = template(*code as u32);
            assert_eq!(atoms.iter().map(String::as_str).collect::<Vec<_>>(), t.body.to_vec(), "code {code}");
        }
        // the empty panel: the same two
        let (pf, _, after, got) = edit(V2, EMPTY, &[standard("Провести", 24), separator("Р"), standard("Закрыть", 142)]);
        assert_eq!(after.ids(), vec![31, 32, 33]);
        assert_eq!(names(&got), vec!["Провести", "Р", "Закрыть"]);
        assert_eq!((got[0].action.as_str(), got[2].action.as_str()), ("Standard:24", "Standard:142"));
        assert_eq!(standard_actions(&pf.bytes, panel_id(V2, EMPTY)).iter().map(|s| s.0).collect::<Vec<_>>(), vec![24, 142]);
        // extract of the result and a pack of it again change nothing
        let xml = decode(V2, &pf.bytes);
        assert!(xml.contains("actionKind=\"Standard\" action=\"Standard:24\""));
        assert!(patch_form(&parse_spanned(&pf.bytes).unwrap(), xml.as_bytes(), xml.as_bytes()).unwrap().replacements.is_empty());
    }

    #[test]
    fn every_template_decodes_to_its_standard_action() {
        use oneform::cb_standard::{form_class, CODES, TEMPLATES};
        assert!(CODES.len() >= 50);
        for code in [140, 141, 142, 24, 29, 38, 40, 80, 82, 83, 84, 85, 86, 90, 95, 300, 143, 144, 145, 146, 147] {
            assert!(CODES.iter().any(|c| c.code == code), "code {code} has no template");
        }
        assert!(CODES.iter().all(|c| c.seen >= 10 && TEMPLATES.iter().any(|t| t.code == c.code && t.class.is_empty())));
        assert!(TEMPLATES.iter().all(|t| ["", "Document", "Catalog", "List", "Other"].contains(&t.class)));
        assert_eq!([form_class("ДокументОбъект"), form_class("СправочникСписок"), form_class("ЖурналДокументов"), form_class("ОбработкаОбъект"), form_class("")], ["Document", "List", "List", "Other", "Other"]);
        // all codes, in two panels of different shapes
        let list: Vec<String> = CODES.iter().map(|c| standard(&format!("Кнопка{}", c.code), c.code)).collect();
        for (panel, offset) in [(EMPTY, 0), (MAIN, 3)] {
            let mut all = if offset == 0 { Vec::new() } else { blocks(&base(V2), panel) };
            all.extend(list.iter().cloned());
            let (pf, _, after, got) = edit(V2, panel, &all);
            assert_eq!(got.len(), CODES.len() + offset);
            for (c, b) in CODES.iter().zip(&got[offset..]) {
                assert_eq!((b.action.as_str(), b.action_kind.as_str()), (format!("Standard:{}", c.code).as_str(), "Standard"));
                assert_eq!(b.caption.is_empty(), c.caption.is_empty(), "code {}", c.code);
            }
            assert_eq!(after.root.len(), CODES.len() + offset);
            let codes: Vec<i64> = standard_actions(&pf.bytes, panel_id(V2, panel)).iter().map(|s| s.0).collect();
            assert_eq!(codes.len(), CODES.len() + usize::from(offset > 0), "{codes:?}");
        }
    }

    #[test]
    fn a_standard_command_of_an_unknown_code_lists_the_supported_ones() {
        refused(V2, EMPTY, &[standard("Х", 9999)], "supported codes: 1, 2, 6");
        refused(V2, EMPTY, &[standard("Х", 0)], "no template for Standard:0");
    }

    #[test]
    fn a_separator_and_a_handler_are_removed() {
        let b = blocks(&base(V2), MAIN);
        let (_, _, after, got) = edit(V2, MAIN, &[b[0].clone(), b[2].clone()]);
        assert_eq!((after.n_act, after.t2), (2, 3), "T2 never shrinks");
        assert_eq!(after.ids(), vec![1, 3]);
        assert_eq!(names(&got), vec!["ОсновныеДействияФормыВыполнить", "ОсновныеДействияФормыЗакрыть"]);
        let (_, _, after, got) = edit(V2, MAIN, &[b[1].clone(), b[2].clone()]);
        assert_eq!(after.ids(), vec![2, 3]);
        assert_eq!(names(&got), vec!["Разделитель", "ОсновныеДействияФормыЗакрыть"]);
        // all buttons removed: the empty group stays
        let (_, _, after, got) = edit(V2, MAIN, &[]);
        assert_eq!((after.n_act, after.root.len()), (0, 0));
        assert!(got.is_empty());
    }

    #[test]
    fn buttons_are_reordered_keeping_their_records() {
        let b = blocks(&base(V2), MAIN);
        let (_, _, after, got) = edit(V2, MAIN, &[b[2].clone(), b[1].clone(), b[0].clone()]);
        assert_eq!(after.ids(), vec![3, 2, 1]);
        assert_eq!((after.n_act, after.t2), (3, 3), "no new ids");
        let was = buttons_of(V2, &raw(V2), MAIN);
        assert_eq!(got, vec![was[2].clone(), was[1].clone(), was[0].clone()]);
    }

    #[test]
    fn add_remove_reorder_and_a_caption_edit_combine() {
        let b = blocks(&base(V2), MAIN);
        let renamed = b[2].replace("<Item language=\"ru\">Закрыть</Item>", "<Item language=\"ru\">Выйти</Item>");
        assert_ne!(renamed, b[2]);
        let list = [renamed, handler("Заполнить", "ЗаполнитьНажатие", "Заполнить"), b[0].clone()];
        let (_, _, after, got) = edit(V2, MAIN, &list);
        assert_eq!(after.ids(), vec![3, 4, 1]);
        assert_eq!(after.n_act, 3);
        assert_eq!(got[0].caption[0], ("ru".to_string(), "Выйти".to_string()));
        assert_eq!(got[0].action, "Standard:142");
        assert_eq!(got[1].id, Some(4));
        // a changed handler name of a kept button is applied inside the rebuilt record too
        let moved = b[0].replace("action=\"КнопкаВыполнитьНажатие\"", "action=\"ДругаяПроцедура\"");
        let (_, _, _, got) = edit(V2, MAIN, &[b[2].clone(), moved]);
        assert_eq!(got[1].action, "ДругаяПроцедура");
    }

    #[test]
    fn tail_position_follows_its_button_and_sub_menu_lists_survive() {
        // the root group of this panel ends with a standard sub-menu whose tail is `{pos,1,T1,id,{1,T1,id,0}}`
        let name = "MailGroupForm";
        let xml = base(name);
        let m = parse_form_xml(xml.as_bytes(), "b").unwrap();
        let (panel, was) = m
            .controls
            .iter()
            .find(|c| c.type_name == "CommandBar" && c.values.buttons.iter().any(|b| b.kind == "Submenu"))
            .map(|c| (c.name.clone(), c.values.buttons.clone()))
            .unwrap();
        let b = blocks(&xml, &panel);
        assert_eq!(b.len(), was.len());
        let at = was.iter().position(|b| b.kind == "Submenu").unwrap();
        let mut list = vec![handler("НоваяКнопкаПочты", "НоваяКнопкаПочтыНажатие", "Новая")];
        list.extend(b.iter().cloned());
        let (_, before, after, got) = edit(name, &panel, &list);
        assert_eq!(after.root.len(), before.root.len() + 1);
        assert_eq!(got[at + 1].kind, "Submenu");
        assert_eq!(got[at + 1].buttons, was[at].buttons, "the standard sub-menu group is untouched");
        assert_eq!((&after.subs, &after.refs), (&before.subs, &before.refs));
        if before.pos >= 0 && (before.pos as usize) < before.root.len() {
            assert_eq!(after.root[after.pos as usize].0, before.root[before.pos as usize].0, "the position stays on its button");
        }
        // the position sits on the standard sub-menu of the other panel and follows it
        let name = "SharedSubmenuForm";
        let xml = base(name);
        let m = parse_form_xml(xml.as_bytes(), "b").unwrap();
        let c = m.controls.iter().find(|c| c.name == "ДействияФормы").unwrap();
        let b = blocks(&xml, &c.name);
        let before = table(&raw(name), c.id);
        assert!(before.pos >= 0 && (before.pos as usize) < before.root.len(), "fixture: the position sits on a button");
        let anchor = before.pos as usize;
        // and the position follows its button when buttons are inserted before it
        let mut more = b.clone();
        more.insert(0, handler("Первая", "ПерваяНажатие", "Первая"));
        more.insert(0, separator("Первый"));
        let (_, _, after, _) = edit(name, &c.name, &more);
        assert_eq!(after.pos as usize, anchor + 2);
        assert_eq!(after.root[anchor + 2].0, before.root[anchor].0);
    }

    /// `MAIN` with its tail position set to `pos`.
    fn with_pos(pos: i64) -> SpannedMetadata {
        let id = panel_id(V2, MAIN);
        tweak_stream(V2, |_, idx| {
            let t = idx.get(id).unwrap().info().unwrap().child(1).unwrap().child(7).unwrap();
            let tail = t.child(9).unwrap().child(11).unwrap();
            Replacement { range: tail.atom(0).unwrap().1, bytes: pos.to_string().into_bytes() }
        })
    }

    #[test]
    fn tail_position_moves_to_the_next_survivor_or_the_end() {
        let xml = base(V2);
        let b = blocks(&xml, MAIN);
        let id = panel_id(V2, MAIN);
        let run_pos = |pos: i64, list: &[String]| {
            let s = with_pos(pos);
            assert_eq!(table(&s.emit(), id).pos, pos);
            let pf = patch_form(&s, xml.as_bytes(), set_buttons(&xml, MAIN, list).as_bytes()).unwrap_or_else(|e| panic!("{e}"));
            table(&pf.bytes, id)
        };
        // the separator (index 1) is removed: Закрыть is the next survivor and moves to index 1
        assert_eq!(run_pos(1, &[b[0].clone(), b[2].clone()]).pos, 1);
        // the last button (index 2) is removed: nobody follows, the position is the new end
        assert_eq!(run_pos(2, &[b[0].clone(), b[1].clone()]).pos, 2);
        // the end position follows the end
        assert_eq!(run_pos(3, &[b[0].clone(), b[1].clone(), b[2].clone(), handler("Н", "Н1", "Н")]).pos, 4);
        assert_eq!(run_pos(3, &[b[0].clone()]).pos, 1);
        // a reorder carries the position with its button
        assert_eq!(run_pos(0, &[b[2].clone(), b[1].clone(), b[0].clone()]).pos, 2);
        // -1 stays
        assert_eq!(run_pos(-1, &[b[2].clone()]).pos, -1);
    }

    #[test]
    fn buttons_of_real_panels_are_added_removed_and_reordered() {
        let mut panels = 0;
        for name in ["MailGroupForm", "SharedSubmenuForm"] {
            let xml = base(name);
            let m = parse_form_xml(xml.as_bytes(), "b").unwrap();
            for c in m.controls.iter().filter(|c| c.type_name == "CommandBar" && !c.values.buttons.is_empty()) {
                panels += 1;
                let b = blocks(&xml, &c.name);
                assert_eq!(b.len(), c.values.buttons.len(), "{name} {}", c.name);
                // append a handler
                let mut list = b.clone();
                list.push(handler("ДобавленнаяКнопка", "ДобавленнаяКнопкаНажатие", "Добавленная"));
                let (_, before, after, got) = edit(name, &c.name, &list);
                assert_eq!(after.root.len(), before.root.len() + 1);
                assert_eq!(got.last().unwrap().id, Some(before.t2 + 1));
                assert_eq!((&after.subs, &after.refs), (&before.subs, &before.refs));
                // remove the first flat button
                if let Some(i) = c.values.buttons.iter().position(|x| x.kind != "Submenu") {
                    let mut list = b.clone();
                    list.remove(i);
                    let (_, before, after, _) = edit(name, &c.name, &list);
                    assert_eq!((after.root.len() + 1, after.n_act + 1, after.t2), (before.root.len(), before.n_act, before.t2));
                }
                // reverse a panel without sub-menus
                if c.values.buttons.iter().all(|x| x.kind != "Submenu") && b.len() > 1 {
                    let mut list = b.clone();
                    list.reverse();
                    let (_, before, after, _) = edit(name, &c.name, &list);
                    let mut ids = before.ids();
                    ids.reverse();
                    assert_eq!(after.ids(), ids);
                }
            }
        }
        assert!(panels >= 2);
    }

    #[test]
    fn unsupported_buttons_are_refused_with_their_line_and_reason() {
        let ok = handler("Выполнить", "ВыполнитьНажатие", "Выполнить");
        let cap = "<Caption><Item language=\"ru\">П</Item></Caption>";
        let cases: Vec<(String, &str)> = vec![
            (format!("<Button name=\"Закрыть2\" kind=\"Action\" actionKind=\"Standard\" action=\"Проц\">{cap}</Button>"), "Standard:N"),
            (format!("<Button name=\"Закрыть2\" kind=\"Action\" actionKind=\"Handler\" action=\"Standard:24\">{cap}</Button>"), "Standard:N"),
            (format!("<Button name=\"Закрыть2\" kind=\"Action\" action=\"Standard:abc\">{cap}</Button>"), "Standard:N"),
            (format!("<Button name=\"Закрыть2\" kind=\"Action\" action=\"Standard:9999\">{cap}</Button>"), "no template for Standard:9999; supported codes: 1, 2, 6"),
            (format!("<Button name=\"Закрыть2\" kind=\"Action\" action=\"Standard:24\">{cap}<Tooltip><Item language=\"ru\">П</Item></Tooltip></Button>"), "tooltip"),
            (format!("<Button name=\"Меню\" kind=\"Action\" actionKind=\"StdSubmenu\" action=\"x\">{cap}</Button>"), "handler buttons"),
            (format!("<Button name=\"Меню\" kind=\"Submenu\" actionKind=\"StdSubmenu\">{cap}</Button>"), "platform sub-menus"),
            (format!("<Button name=\"Меню\" kind=\"Submenu\" action=\"Проц\">{cap}</Button>"), "user sub-menu"),
            ("<Button name=\"Меню\" kind=\"Submenu\"/>".to_string(), "needs a <Caption>"),
            (format!("<Button name=\"Меню\" kind=\"Submenu\">{cap}<Tooltip><Item language=\"ru\">П</Item></Tooltip></Button>"), "no tooltip"),
            (format!("<Button name=\"Движения\" kind=\"Action\" actionKind=\"OpenMovements\">{cap}</Button>"), "handler buttons"),
            (format!("<Button name=\"Пустая\" kind=\"Action\">{cap}</Button>"), "needs action"),
            ("<Button name=\"БезПодписи\" kind=\"Action\" action=\"Проц\"/>".to_string(), "needs a <Caption>"),
            (format!("<Button name=\"Плохое Имя\" kind=\"Action\" action=\"Проц\">{cap}</Button>"), "identifier"),
            (format!("<Button name=\"Кнопка\" kind=\"Action\" action=\"1Проц\">{cap}</Button>"), "procedure name"),
            (format!("<Button name=\"Картинка\" kind=\"Action\" action=\"Проц\">{cap}<Picture kind=\"2\" stdId=\"1\"/></Button>"), "picture"),
            ("<Button name=\"Разд\" kind=\"Separator\" action=\"Проц\"/>".to_string(), "separator holds no"),
            (handler("ОсновныеДействияФормыЗакрыть", "Проц", "Закрыть"), "name exists"),
        ];
        for (extra, needle) in cases {
            // the baseline buttons stay, the new one goes in the middle
            let mut list = blocks(&base(V2), MAIN);
            list.insert(1, extra.clone());
            let edited = set_buttons(&base(V2), MAIN, &list);
            let Err(e) = run(V2, &edited) else { panic!("{extra}") };
            let text = e.to_string();
            assert!(text.contains(needle), "{needle}: {text}");
            assert!(text.contains("line "), "{text}");
        }
        // two new buttons with one name
        refused(V2, EMPTY, &[ok.clone(), ok], "name exists");
        // the same id twice
        let b = blocks(&base(V2), MAIN);
        refused(V2, MAIN, &[b[0].clone(), b[0].clone()], "used twice");
    }

    fn submenu(name: &str, caption: &str, list: &[String]) -> String {
        format!(
            "<Button name=\"{name}\" kind=\"Submenu\"><Caption><Item language=\"ru\">{caption}</Item></Caption><Buttons>\n{}\n</Buttons></Button>",
            list.join("\n")
        )
    }

    /// One group of a panel as the stream holds it.
    #[derive(Debug, PartialEq)]
    struct G {
        key: (String, String),
        ids: Vec<i64>,
        subs: Vec<i64>,
        refs: Vec<i64>,
        pos: i64,
    }

    /// All groups of panel `id` in stream order; asserts the invariants of a sub-menu layout.
    fn layout(bytes: &[u8], id: i64) -> Vec<G> {
        let s = parse_spanned(bytes).unwrap();
        let idx = ControlIndex::build(&s).unwrap();
        let x = idx.get(id).unwrap().info().unwrap().node.node(1).unwrap();
        let t = x.node(7).unwrap();
        let n_act: usize = atom(t, 4).parse().unwrap();
        let n_groups: usize = atom(t, 5 + n_act).parse().unwrap();
        let t1 = atom(t, 1);
        // action guid -> group key of a sub-menu action
        let acts: std::collections::HashMap<String, Option<(String, String)>> = (0..n_act)
            .map(|i| {
                let a = t.node(5 + i).unwrap();
                let body = a.node(4).unwrap();
                let key = (atom(a, 3).starts_with("abde0c9a") || atom(a, 3).starts_with("c93a51ed"))
                    .then(|| (atom(body, 1), atom(body, 2)))
                    .filter(|k| k.1 != "0");
                (atom(a, 1), key)
            })
            .collect();
        let root_key = (atom(x, 8), atom(x, 9));
        let mut groups = Vec::new();
        let mut buttons: std::collections::HashMap<i64, (String, String)> = std::collections::HashMap::new();
        for gi in 0..n_groups {
            let g = t.node(6 + n_act + gi).unwrap();
            let k: usize = atom(g, 4).parse().unwrap();
            let tail = g.node(5 + 2 * k).unwrap();
            let nr: usize = atom(tail, 1).parse().unwrap();
            let sub = tail.node(2 + 2 * nr).unwrap();
            let ids: Vec<i64> = (0..k).map(|i| atom(g.node(6 + 2 * i).unwrap(), 7).parse().unwrap()).collect();
            for (i, bid) in ids.iter().enumerate() {
                buttons.insert(*bid, (atom(g, 5 + 2 * i), atom(g.node(6 + 2 * i).unwrap(), 9)));
            }
            let subs: Vec<i64> = (0..atom(sub, 0).parse::<usize>().unwrap()).map(|i| atom(sub, 2 + 3 * i).parse().unwrap()).collect();
            let refs = (0..nr).map(|i| atom(tail, 3 + 2 * i).parse().unwrap()).collect();
            groups.push(G { key: (atom(g, 1), atom(g, 2)), ids, subs, refs, pos: atom(tail, 0).parse().unwrap() });
        }
        // every listed sub-menu is a sub-menu button of its group, in button order
        for g in &groups {
            let at: Vec<usize> = g.subs.iter().map(|s| g.ids.iter().position(|i| i == s).expect("listed sub-menu is in the group")).collect();
            assert!(at.windows(2).all(|w| w[0] < w[1]), "sub-menu list in button order {g:?}");
            assert!(g.subs.iter().all(|s| buttons[s].1 == "1"), "listed buttons are sub-menus {g:?}");
        }
        // every user sub-menu button has its group, every user group is shown by exactly one button
        let mut shown = std::collections::HashMap::new();
        for (guid, _) in buttons.values() {
            if let Some(key) = acts[guid].clone().filter(|k| k.0 == t1) {
                assert!(groups.iter().any(|g| g.key == key), "group {key:?} of a sub-menu button exists");
                *shown.entry(key).or_insert(0) += 1;
            }
        }
        for g in groups.iter().filter(|g| g.key.0 == t1) {
            assert_eq!(shown.get(&g.key), Some(&1), "group {:?} is shown once", g.key);
        }
        assert!(groups.iter().filter(|g| g.key == root_key).count() == 1);
        groups
    }

    const ROOT_GUID: &str = "b78f2e80-ec68-11d4-9dcf-0050bae2bc79";

    fn group<'a>(groups: &'a [G], code: &str) -> &'a G {
        groups.iter().find(|g| g.key.1 == code && g.key.0 != ROOT_GUID).unwrap_or_else(|| panic!("group {code} in {groups:?}"))
    }

    fn root(groups: &[G]) -> &G {
        groups.iter().find(|g| g.key.0 == ROOT_GUID).unwrap()
    }

    /// The buttons of a decoded panel as `name[child, child]` text.
    fn shape(list: &[CommandBarButton]) -> String {
        list.iter()
            .map(|b| if b.buttons.is_empty() { b.name.clone() } else { format!("{}[{}]", b.name, shape(&b.buttons)) })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The extracted XML of the result, packed again, changes nothing.
    fn idempotent(pf: &PatchedForm, name: &str) {
        let xml = decode(name, &pf.bytes);
        let again = patch_form(&parse_spanned(&pf.bytes).unwrap(), xml.as_bytes(), xml.as_bytes()).unwrap();
        assert!(again.replacements.is_empty());
    }

    fn handlers(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| handler(n, &format!("{n}Нажатие"), n)).collect()
    }

    fn table_guid(bytes: &[u8], id: i64) -> String {
        let s = parse_spanned(bytes).unwrap();
        let idx = ControlIndex::build(&s).unwrap();
        atom(idx.get(id).unwrap().info().unwrap().node.node(1).unwrap().node(7).unwrap(), 1)
    }

    #[test]
    fn a_submenu_with_two_handlers_is_added_to_the_empty_donor_panel() {
        let list = [submenu("Действия", "Действия", &handlers(&["Первая", "Вторая"]))];
        let (pf, before, after, got) = edit(V2, EMPTY, &list);
        assert_eq!((before.n_act, before.t2), (0, 30));
        // the button takes 31, its group code 32, the buttons below 33 and 34
        assert_eq!((after.n_act, after.t2), (3, 34));
        assert_eq!(after.ids(), vec![31]);
        assert_eq!(after.subs, vec![31]);
        assert_eq!(shape(&got), "Действия[Первая, Вторая]");
        assert_eq!((got[0].id, got[0].kind.as_str(), got[0].action_kind.as_str()), (Some(31), "Submenu", "Submenu"));
        assert_eq!(got[0].buttons.iter().map(|b| b.id.unwrap()).collect::<Vec<_>>(), vec![33, 34]);
        let id = panel_id(V2, EMPTY);
        let groups = layout(&pf.bytes, id);
        assert_eq!(groups.len(), 2);
        let sub = group(&groups, "32");
        assert_eq!((sub.ids.clone(), sub.subs.clone(), sub.pos), (vec![33, 34], vec![], -1));
        assert_eq!(sub.key.0, table_guid(&pf.bytes, id));
        assert_eq!(pf.button_lists[0].1[0].buttons.len(), 2);
        idempotent(&pf, V2);
    }

    #[test]
    fn submenus_nest_up_to_three_levels() {
        let h = handlers(&["А", "Б", "В"]);
        let two = submenu("Верх", "Верх", &[h[0].clone(), submenu("Низ", "Низ", &[h[1].clone()])]);
        let (pf, _, after, got) = edit(V2, EMPTY, &[two]);
        assert_eq!(shape(&got), "Верх[А, Низ[Б]]");
        // Верх 31, group 32, А 33, Низ 34, group 35, Б 36
        assert_eq!(after.t2, 36);
        assert_eq!((got[0].id, got[0].buttons[1].id, got[0].buttons[1].buttons[0].id), (Some(31), Some(34), Some(36)));
        let groups = layout(&pf.bytes, panel_id(V2, EMPTY));
        assert_eq!(groups.len(), 3);
        assert_eq!((group(&groups, "32").ids.clone(), group(&groups, "32").subs.clone()), (vec![33, 34], vec![34]));
        assert_eq!(group(&groups, "35").ids, vec![36]);
        assert_eq!(root(&groups).subs, vec![31]);
        idempotent(&pf, V2);
        // a sibling sub-menu and a separator
        let both = [
            submenu("Один", "Один", &[h[0].clone(), h[1].clone()]),
            separator("Р"),
            submenu("Два", "Два", &[submenu("Внутри", "Внутри", &[h[2].clone()])]),
        ];
        let (pf, _, after, got) = edit(V2, EMPTY, &both);
        assert_eq!(shape(&got), "Один[А, Б], Р, Два[Внутри[В]]");
        let groups = layout(&pf.bytes, panel_id(V2, EMPTY));
        // Один 31/32, А 33, Б 34, Р 35, Два 36/37, Внутри 38/39, В 40
        assert_eq!((root(&groups).ids.clone(), root(&groups).subs.clone()), (vec![31, 35, 36], vec![31, 36]));
        assert_eq!(after.t2, 40);
        idempotent(&pf, V2);
        // a fourth level is refused
        let four = submenu("А1", "А1", &[submenu("А2", "А2", &[submenu("А3", "А3", &[h[0].clone()])])]);
        refused(V2, EMPTY, &[four], "deeper than three levels");
    }

    #[test]
    fn handlers_move_into_and_out_of_a_submenu() {
        let xml = base(V2);
        let b = blocks(&xml, SUB);
        let (id, was) = (panel_id(V2, SUB), buttons_of(V2, &raw(V2), SUB));
        assert_eq!(shape(&was), "НоваяКнопкаНаПанели, Подменю[Действие, Действие1]");
        let k = kids(&b[1]);
        assert_eq!(k.len(), 2);
        // into: the root handler becomes the first button of the sub-menu
        let (pf, _, after, got) = edit(V2, SUB, &[with_kids(&b[1], &[b[0].clone(), k[0].clone(), k[1].clone()])]);
        assert_eq!(shape(&got), "Подменю[НоваяКнопкаНаПанели, Действие, Действие1]");
        assert_eq!(after.t2, 5, "no ids are allocated");
        assert_eq!(after.n_act, 4);
        let groups = layout(&pf.bytes, id);
        assert_eq!(group(&groups, "3").ids, vec![1, 4, 5]);
        assert_eq!((root(&groups).ids.clone(), root(&groups).subs.clone()), (vec![2], vec![2]));
        // the button record and its action are the old ones
        assert_eq!(got[0].buttons[0], was[0]);
        assert_eq!(got[0].buttons[1..], was[1].buttons[..]);
        idempotent(&pf, V2);
        // out: a nested handler goes to the root, between the old root button and the sub-menu
        let (pf, _, after, got) = edit(V2, SUB, &[b[0].clone(), k[0].clone(), with_kids(&b[1], &[k[1].clone()])]);
        assert_eq!(shape(&got), "НоваяКнопкаНаПанели, Действие, Подменю[Действие1]");
        assert_eq!(after.ids(), vec![1, 4, 2]);
        let groups = layout(&pf.bytes, id);
        assert_eq!(group(&groups, "3").ids, vec![5]);
        assert_eq!(root(&groups).subs, vec![2]);
        assert_eq!(got[1], was[1].buttons[0]);
        idempotent(&pf, V2);
        // moves and a new button in one edit
        let (_, _, after, got) = edit(V2, SUB, &[with_kids(&b[1], &[k[1].clone(), handler("Н", "Н1", "Н")]), k[0].clone(), b[0].clone()]);
        assert_eq!(shape(&got), "Подменю[Действие1, Н], Действие, НоваяКнопкаНаПанели");
        assert_eq!(after.t2, 6);
    }

    #[test]
    fn a_submenu_is_removed_with_its_group_and_actions() {
        let b = blocks(&base(V2), SUB);
        let id = panel_id(V2, SUB);
        let was = buttons_of(V2, &raw(V2), SUB);
        let (pf, before, after, got) = edit(V2, SUB, &b[..1]);
        assert_eq!((before.n_act, before.t2, after.n_act, after.t2), (4, 5, 1, 5), "T2 never shrinks");
        assert_eq!(got, vec![was[0].clone()], "the other button is identical");
        let groups = layout(&pf.bytes, id);
        assert_eq!(groups.len(), 1);
        assert_eq!((root(&groups).ids.clone(), root(&groups).subs.clone()), (vec![1], vec![]));
        let text = String::from_utf8_lossy(&pf.bytes);
        assert!(!text.contains("КоманднаяПанель2Действие1"), "the actions of the sub-menu are gone");
        idempotent(&pf, V2);
        // the first button goes, the sub-menu stays; then everything goes
        let (pf, _, after, got) = edit(V2, SUB, &b[1..]);
        assert_eq!((after.n_act, shape(&got)), (3, "Подменю[Действие, Действие1]".to_string()));
        layout(&pf.bytes, id);
        let (pf, _, after, got) = edit(V2, SUB, &[]);
        assert_eq!((after.n_act, got.len(), layout(&pf.bytes, id).len()), (0, 0, 1));
        // a sub-menu is removed but one of its buttons is moved out
        let k = kids(&b[1]);
        let (pf, _, after, got) = edit(V2, SUB, &[b[0].clone(), k[1].clone()]);
        assert_eq!((after.n_act, shape(&got)), (2, "НоваяКнопкаНаПанели, Действие1".to_string()));
        assert_eq!(layout(&pf.bytes, id).len(), 1);
        // an emptied sub-menu stays as a sub-menu with an empty group
        let (pf, _, _, got) = edit(V2, SUB, &[b[0].clone(), with_kids(&b[1], &[])]);
        assert_eq!(shape(&got), "НоваяКнопкаНаПанели, Подменю");
        assert_eq!(group(&layout(&pf.bytes, id), "3").ids, Vec::<i64>::new());
        idempotent(&pf, V2);
    }

    #[test]
    fn buttons_are_added_removed_and_reordered_inside_a_submenu() {
        let b = blocks(&base(V2), SUB);
        let k = kids(&b[1]);
        let id = panel_id(V2, SUB);
        let with = |list: &[String]| vec![b[0].clone(), with_kids(&b[1], list)];
        // reorder
        let (pf, _, after, got) = edit(V2, SUB, &with(&[k[1].clone(), k[0].clone()]));
        assert_eq!((shape(&got), after.t2), ("НоваяКнопкаНаПанели, Подменю[Действие1, Действие]".to_string(), 5));
        assert_eq!(group(&layout(&pf.bytes, id), "3").ids, vec![5, 4]);
        idempotent(&pf, V2);
        // add a handler, a separator and a nested sub-menu
        let extra = [k[0].clone(), handler("Нов", "Нов1", "Нов"), separator("Р"), submenu("Вн", "Вн", &handlers(&["Я"])), k[1].clone()];
        let (pf, _, after, got) = edit(V2, SUB, &with(&extra));
        assert_eq!(shape(&got), "НоваяКнопкаНаПанели, Подменю[Действие, Нов, Р, Вн[Я], Действие1]");
        // Нов 6, Р 7, Вн 8 with the code 9, Я 10
        assert_eq!(after.t2, 10);
        let groups = layout(&pf.bytes, id);
        assert_eq!(group(&groups, "3").ids, vec![4, 6, 7, 8, 5]);
        assert_eq!(group(&groups, "3").subs, vec![8]);
        assert_eq!(group(&groups, "9").ids, vec![10]);
        idempotent(&pf, V2);
        // remove one
        let (pf, _, after, got) = edit(V2, SUB, &with(&k[1..]));
        assert_eq!((shape(&got), after.n_act), ("НоваяКнопкаНаПанели, Подменю[Действие1]".to_string(), 3));
        assert_eq!(group(&layout(&pf.bytes, id), "3").ids, vec![5]);
        // a renamed caption of a nested button is applied in the same pass
        let renamed = k[0].replacen("<Item language=\"ru\">Действие</Item>", "<Item language=\"ru\">Другое</Item>", 1);
        assert_ne!(renamed, k[0]);
        let (_, _, _, got) = edit(V2, SUB, &with(&[k[1].clone(), renamed]));
        assert_eq!(got[1].buttons[1].caption[0].1, "Другое");
    }

    #[test]
    fn a_kept_submenu_moves_into_a_new_one_and_out_again() {
        let b = blocks(&base(V2), SUB);
        let id = panel_id(V2, SUB);
        let was = buttons_of(V2, &raw(V2), SUB);
        let (pf, _, after, got) = edit(V2, SUB, &[b[0].clone(), submenu("Внешнее", "Внешнее", &[b[1].clone()])]);
        assert_eq!(shape(&got), "НоваяКнопкаНаПанели, Внешнее[Подменю[Действие, Действие1]]");
        // Внешнее 6 with the code 7
        assert_eq!((after.t2, after.n_act), (7, 5));
        assert_eq!(got[1].buttons[0], was[1]);
        let groups = layout(&pf.bytes, id);
        assert_eq!(group(&groups, "7").ids, vec![2]);
        assert_eq!(group(&groups, "7").subs, vec![2]);
        assert_eq!(root(&groups).subs, vec![6]);
        idempotent(&pf, V2);
        let again = decode(V2, &pf.bytes);
        let nb = blocks(&again, SUB);
        let inner = kids(&nb[1]);
        let pf2 = patch_form(&parse_spanned(&pf.bytes).unwrap(), again.as_bytes(), set_buttons(&again, SUB, &[nb[0].clone(), inner[0].clone()]).as_bytes()).unwrap();
        assert_eq!(shape(&buttons_of(V2, &pf2.bytes, SUB)), "НоваяКнопкаНаПанели, Подменю[Действие, Действие1]");
        assert_eq!(layout(&pf2.bytes, id).len(), 2);
    }

    #[test]
    fn platform_submenus_are_kept_and_moved_but_not_created_or_changed() {
        let name = "MailGroupForm";
        let xml = base(name);
        let m = parse_form_xml(xml.as_bytes(), "b").unwrap();
        let c = m.controls.iter().find(|c| c.type_name == "CommandBar" && c.values.buttons.iter().any(|b| b.kind == "Submenu")).unwrap();
        let (panel, was) = (c.name.clone(), c.values.buttons.clone());
        let b = blocks(&xml, &panel);
        let at = was.iter().position(|b| b.kind == "Submenu").unwrap();
        // moved to the end with its buttons behind a new handler: the group stays as it is
        let mut list = b.clone();
        let sub = list.remove(at);
        list.insert(0, handler("Нов0", "Нов01", "Нов0"));
        list.push(sub);
        let (pf, before, after, got) = edit(name, &panel, &list);
        assert_eq!(got.last().unwrap(), &was[at]);
        assert_eq!(after.root.len(), before.root.len() + 1);
        let id = panel_id(name, &panel);
        assert_eq!(layout(&pf.bytes, id).len(), layout(&raw(name), id).len());
        // a button cannot be added to it, removed from it or moved out of it
        if !kids(&b[at]).is_empty() {
            let mut list = b.clone();
            list[at] = with_kids(&b[at], &[]);
            refused(name, &panel, &list, "platform sub-menu");
            let mut list = b.clone();
            list[at] = with_kids(&b[at], &[kids(&b[at]), vec![handler("Нов", "Нов1", "Нов")]].concat());
            refused(name, &panel, &list, "platform sub-menu");
        }
        // it cannot be removed either
        let mut list = b.clone();
        list.remove(at);
        let e = run(name, &set_buttons(&xml, &panel, &list)).expect_err("removal");
        assert!(e.to_string().contains("removing a platform sub-menu"), "{e}");
    }

    #[test]
    fn an_edit_of_a_nested_button_and_a_new_root_button_combine() {
        let b = blocks(&base(V2), SUB);
        let k = kids(&b[1]);
        let renamed = k[1].replacen("<Item language=\"ru\">Действие1</Item>", "<Item language=\"ru\">Иначе</Item>", 1);
        let list = [handler("Первая", "Первая1", "Первая"), b[0].clone(), with_kids(&b[1], &[k[0].clone(), renamed])];
        let (pf, _, after, got) = edit(V2, SUB, &list);
        assert_eq!((shape(&got), got[2].buttons[1].caption[0].1.as_str()), ("Первая, НоваяКнопкаНаПанели, Подменю[Действие, Действие1]".to_string(), "Иначе"));
        assert_eq!(after.t2, 6);
        layout(&pf.bytes, panel_id(V2, SUB));
    }

    #[test]
    fn invalid_submenu_edits_are_refused() {
        let b = blocks(&base(V2), SUB);
        let k = kids(&b[1]);
        // the same button twice
        refused(V2, SUB, &[b[0].clone(), with_kids(&b[1], &[k[0].clone(), k[0].clone()])], "used twice");
        // a button of the root that is also inside
        refused(V2, SUB, &[b[0].clone(), with_kids(&b[1], &[b[0].clone()])], "used twice");
        // the kind of a kept button changes
        let sep = "<Button name=\"Действие\" id=\"4\" kind=\"Separator\" checked=\"false\"/>".to_string();
        let more = handler("Ещё", "Ещё1", "Ещё");
        refused(V2, SUB, &[b[0].clone(), with_kids(&b[1], &[sep, k[1].clone(), more])], "kind of an existing button");
        // a new name that exists
        refused(V2, SUB, &[b[0].clone(), with_kids(&b[1], &[k[0].clone(), k[1].clone(), handler("Подменю", "П", "П")])], "name exists");
        // the name of a removed button is free again
        let (_, _, _, got) = edit(V2, SUB, &[b[0].clone(), with_kids(&b[1], &[k[1].clone(), handler("Действие", "Новое1", "Новое")])]);
        assert_eq!(shape(&got), "НоваяКнопкаНаПанели, Подменю[Действие1, Действие]");
    }

    #[test]
    fn unchanged_order_keeps_the_per_button_patch_path() {
        let xml = base(V2);
        let edited = replace_once(&xml, "<Item language=\"ru\">Закрыть</Item>", "<Item language=\"ru\">Выйти</Item>");
        let pf = run(V2, &edited).unwrap();
        assert_eq!(pf.replacements.len(), 1);
        assert!(pf.button_lists.is_empty());
        assert!(pf.replacements[0].bytes.len() < 40, "an in-place text edit, not a table rebuild");
    }

    #[test]
    fn a_corrupt_table_is_not_rebuilt() {
        // negative control: the checker is live and refuses a panel whose layout it cannot trust
        let b = blocks(&base(V2), MAIN);
        let edited = set_buttons(&base(V2), MAIN, &[b[0].clone(), b[2].clone()]);
        let id = panel_id(V2, MAIN);
        let s = tweak_stream(V2, |_, idx| {
            let t = idx.get(id).unwrap().info().unwrap().child(1).unwrap().child(7).unwrap();
            let (_, span) = t.atom(2).unwrap();
            Replacement { range: span, bytes: b"1".to_vec() }
        });
        let e = patch_form(&s, base(V2).as_bytes(), edited.as_bytes()).expect_err("id counter below a button id");
        assert!(e.to_string().contains("id counter"), "{e}");
        let s = tweak_stream(V2, |s, idx| {
            let t = idx.get(id).unwrap().info().unwrap().child(1).unwrap().child(7).unwrap();
            let first = text_of(s, t.child(5).unwrap().atom(1).unwrap().1);
            Replacement { range: t.child(6).unwrap().atom(1).unwrap().1, bytes: first.into_bytes() }
        });
        let e = patch_form(&s, base(V2).as_bytes(), edited.as_bytes()).expect_err("duplicate action");
        assert!(e.to_string().contains("listed twice"), "{e}");
        // the property edit path is not affected by the checker
        assert!(patch_form(&parse_spanned(&raw(V2)).unwrap(), base(V2).as_bytes(), base(V2).as_bytes()).unwrap().replacements.is_empty());
    }

    #[test]
    fn the_rest_of_the_stream_is_preserved_byte_for_byte() {
        let (pf, ..) = edit(V2, EMPTY, &[handler("Выполнить", "ВыполнитьНажатие", "Выполнить")]);
        let original = raw(V2);
        let r = &pf.replacements[0];
        assert_eq!(&pf.bytes[..r.range.start], &original[..r.range.start]);
        assert_eq!(&pf.bytes[r.range.start + r.bytes.len()..], &original[r.range.end..]);
        // the replacement is the button table `{5,...}` of the panel only
        assert!(r.bytes.starts_with(b"{5,") && r.bytes.ends_with(b"}"));
        assert!(original[r.range.start..].starts_with(b"{5,"));
    }

    #[test]
    fn handler_helpers_find_new_procedures_and_declarations() {
        let module = "Процедура ВыполнитьНажатие(Кнопка)\n КонецПроцедуры\nФункция Другая ()\nКонецФункции\n// Процедура Закомментированная(";
        assert!(oneform::form_patch::module_declares(module, "ВыполнитьНажатие"));
        assert!(oneform::form_patch::module_declares(module, "другая"));
        assert!(!oneform::form_patch::module_declares(module, "Выполнить"));
        assert!(!oneform::form_patch::module_declares(module, "Отсутствует"));
        let b = blocks(&base(V2), MAIN);
        let list = [b[0].replace("КнопкаВыполнитьНажатие", "Переименована"), b[1].clone(), handler("Новая", "Новая1", "Н")];
        let edited = set_buttons(&base(V2), MAIN, &list);
        let source = parse_spanned(&raw(V2)).unwrap();
        let plan = plan_form_patch(&source, base(V2).as_bytes(), edited.as_bytes()).unwrap();
        let got: Vec<(String, String)> = plan.diff.edits.iter().flat_map(oneform::form_patch::new_handlers).collect();
        assert_eq!(
            got,
            vec![("ОсновныеДействияФормыВыполнить".to_string(), "Переименована".to_string()), ("Новая".to_string(), "Новая1".to_string())]
        );
    }
}
