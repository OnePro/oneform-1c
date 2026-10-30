//! Added form attributes: the record shape follows the stream version (version <= 26 has no `kind` slot) and the
//! structural check of the attribute section (`form_patch::attr_check`) accepts the result and rejects broken streams.
//! `AbcShapeForm` is the report form ABCАнализПродаж of УТ 10.3 (stream version 26); the platform refused it with
//! «Ошибка формата потока» when an attribute record of the version-27 shape was added.

use std::path::{Path, PathBuf};

use oneform::brace::parse_spanned;
use oneform::form_patch::attr_check::{attribute_record_len, stream_attribute_issues};
use oneform::form_patch::patch_form;

fn dir(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let forms = root.join("forms").join(name);
    if forms.is_dir() {
        forms
    } else {
        root.join("patch").join(name)
    }
}

fn stream(name: &str) -> Vec<u8> {
    std::fs::read(dir(name).join("FormStream.bin")).unwrap()
}

fn baseline(name: &str) -> String {
    std::fs::read_to_string(dir(name).join("Form.xml")).unwrap().replace("\r\n", "\n")
}

const NEW_ATTRIBUTE: &str =
    "    <Attribute id=\"200\" name=\"Nov\"><ValueType kind=\"String\" length=\"100\" composite=\"false\"/></Attribute>\n  </Attributes>";

fn with_attribute(name: &str) -> Vec<u8> {
    let source = stream(name);
    let s = parse_spanned(&source).unwrap();
    let base = baseline(name);
    let edited = base.replacen("  </Attributes>", NEW_ATTRIBUTE, 1);
    assert_ne!(base, edited);
    patch_form(&s, base.as_bytes(), edited.as_bytes()).unwrap().bytes
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap().trim_start_matches('\u{feff}').to_string()
}

#[test]
fn the_fixtures_pass_the_structural_check() {
    for name in ["AbcShapeForm", "MailGroupForm", "DonorForm"] {
        assert_eq!(stream_attribute_issues(&stream(name)), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn an_attribute_added_to_a_version_26_stream_has_the_five_item_record() {
    let source = stream("AbcShapeForm");
    assert!(text(&source).starts_with("{26,"));
    let patched = with_attribute("AbcShapeForm");
    let t = text(&patched);
    assert!(t.contains("{{200},0,0,\"Nov\",{\"Pattern\",{\"S\",100,1}}}"), "{t}");
    assert!(!t.contains("{{200},0,0,1,"), "version-27 record shape in a version-26 stream");
    assert_eq!(stream_attribute_issues(&patched), Vec::<String>::new());
    // the next-id counter moved past the new id
    assert!(t.contains("{0},201,"), "{t}");
}

#[test]
fn an_attribute_added_to_a_version_27_stream_keeps_the_kind_slot() {
    let source = stream("MailGroupForm");
    assert!(text(&source).starts_with("{27,"));
    let patched = with_attribute("MailGroupForm");
    assert!(text(&patched).contains("{{200},0,0,1,\"Nov\",{\"Pattern\",{\"S\",100,1}}}"));
    assert_eq!(stream_attribute_issues(&patched), Vec::<String>::new());
}

#[test]
fn the_record_length_follows_the_stream_version() {
    assert_eq!(attribute_record_len(19), Some(5));
    assert_eq!(attribute_record_len(26), Some(5));
    assert_eq!(attribute_record_len(27), Some(6));
    assert_eq!(attribute_record_len(28), None);
    assert_eq!(attribute_record_len(18), None);
}

fn broken(from: &str, to: &str) -> Vec<String> {
    let t = text(&stream("AbcShapeForm"));
    assert_eq!(t.matches(from).count(), 1, "anchor must be unique: {from}");
    stream_attribute_issues(t.replacen(from, to, 1).as_bytes())
}

#[test]
fn the_check_rejects_the_streams_that_the_platform_refuses() {
    // a record of the version-27 shape in a version-26 stream (the defect of the UT live check)
    let issues = broken("{9},0,1,\"", "{9},0,1,1,\"");
    assert!(issues.iter().any(|m| m.contains("6 items") && m.contains("expected 5")), "{issues:?}");
    // the count differs from the number of records
    let issues = broken("{2,\r\n{\r\n{0},0,1,\"", "{3,\r\n{\r\n{0},0,1,\"");
    assert!(issues.iter().any(|m| m.contains("count")), "{issues:?}");
    // the next-id counter is not above the greatest id
    let issues = broken("{0},10,\r\n{2,", "{0},9,\r\n{2,");
    assert!(issues.iter().any(|m| m.contains("next attribute id")), "{issues:?}");
    // repeated id
    let issues = broken("{9},0,1,\"", "{0},0,1,\"");
    assert!(issues.iter().any(|m| m.contains("repeated")), "{issues:?}");
    // a link to an attribute that does not exist
    let issues = broken("{15,\r\n{2,\r\n{0},", "{15,\r\n{2,\r\n{5},");
    assert!(issues.iter().any(|m| m.contains("does not exist")), "{issues:?}");
    // a version that is not known
    let t = text(&stream("AbcShapeForm"));
    let issues = stream_attribute_issues(t.replacen("{26,", "{29,", 1).as_bytes());
    assert!(issues.iter().any(|m| m.contains("unknown stream version")), "{issues:?}");
}

// ---- controls added to forms of older stream versions (OpenSpec controls-in-older-stream-versions)

fn with_controls(name: &str, attribute: bool, controls: &str) -> Result<Vec<u8>, oneform::errors::ExtractionError> {
    let source = stream(name);
    let s = parse_spanned(&source).unwrap();
    let base = baseline(name);
    let mut edited = base.clone();
    if attribute {
        edited = edited.replacen("  </Attributes>", NEW_ATTRIBUTE, 1);
    }
    let at = edited.rfind("  </ChildItems>").unwrap();
    edited.insert_str(at, controls);
    patch_form(&s, base.as_bytes(), edited.as_bytes()).map(|p| p.bytes)
}

const GEO: &str = "<Left>420</Left><Top>5</Top><Width>120</Width><Height>19</Height><ZOrder>90</ZOrder><TabOrder>90</TabOrder>";

#[test]
fn a_field_bound_to_a_new_string_attribute_repeats_the_pattern_of_the_attribute() {
    // the field of a version 26 form crashed the platform when its own pattern was `{"S",100,0}` and the attribute's `{"S",100,1}`
    let field = format!("    <InputField name=\"PoleX\" id=\"9001\"><DataPath attributeId=\"200\">Nov</DataPath>{GEO}</InputField>\n");
    let t = text(&with_controls("AbcShapeForm", true, &field).unwrap());
    assert!(t.contains("{9,{\"Pattern\",{\"S\",100,1}}"), "{t}");
    assert!(!t.contains("{\"S\",100,0}"), "{t}");
    assert!(t.contains("{{200},0,0,\"Nov\",{\"Pattern\",{\"S\",100,1}}}"));
}

#[test]
fn the_verified_kinds_are_added_to_a_version_26_form() {
    let label = format!("    <LabelDecoration name=\"NadpisX\" id=\"9002\"><Caption><Item language=\"ru\">T</Item></Caption>{GEO}</LabelDecoration>\n");
    let check = format!("    <CheckBox name=\"FlagX\" id=\"9003\"><Caption><Item language=\"ru\">F</Item></Caption>{GEO}</CheckBox>\n");
    let button = format!("    <Button name=\"KnopkaX\" id=\"9004\"><Caption><Item language=\"ru\">B</Item></Caption>{GEO}</Button>\n");
    for xml in [label, check, button] {
        assert!(with_controls("AbcShapeForm", false, &xml).is_ok(), "{xml}");
    }
}

#[test]
fn an_unverified_kind_is_refused_in_a_version_26_form() {
    let table = format!("    <Table name=\"TablX\" id=\"9005\">{GEO}</Table>\n");
    let e = with_controls("AbcShapeForm", false, &table).expect_err("refused");
    assert_eq!(e.params["kind"].as_str(), Some("unsupported-stream-version"), "{e}");
    assert!(e.to_string().contains("adding Table to forms of stream version 26 is not supported"), "{e}");
    // a verified kind next to it does not help
    let both = format!("    <LabelDecoration name=\"NadpisX\" id=\"9002\">{GEO}</LabelDecoration>\n{table}");
    assert!(with_controls("AbcShapeForm", false, &both).is_err());
}

#[test]
fn every_kind_is_still_added_to_a_version_27_form() {
    let table = format!("    <Table name=\"TablX\" id=\"9005\">{GEO}</Table>\n");
    let result = with_controls("MailGroupForm", false, &table);
    if let Err(e) = result {
        assert_ne!(e.params["kind"].as_str(), Some("unsupported-stream-version"), "{e}");
    }
}

// ---- pilot-findings: `composite` of a ValueType is optional

#[test]
fn an_attribute_value_type_without_composite_is_accepted() {
    let source = stream("MailGroupForm");
    let s = parse_spanned(&source).unwrap();
    let base = baseline("MailGroupForm");
    let plain = NEW_ATTRIBUTE.replace(" composite=\"false\"", "");
    assert!(!plain.contains("composite"));
    let edited = base.replacen("  </Attributes>", &plain, 1);
    let patched = patch_form(&s, base.as_bytes(), edited.as_bytes()).unwrap().bytes;
    assert!(text(&patched).contains("{{200},0,0,1,\"Nov\",{\"Pattern\",{\"S\",100,1}}}"));
    // a wrong value of composite is refused with a message that says what to check
    let wrong = NEW_ATTRIBUTE.replace("composite=\"false\"", "composite=\"yes\"");
    let edited = base.replacen("  </Attributes>", &wrong, 1);
    let e = patch_form(&s, base.as_bytes(), edited.as_bytes()).expect_err("refused");
    assert!(e.to_string().contains("composite is true or false when given"), "{e}");
}

#[test]
fn the_attribute_example_of_the_manual_is_accepted_by_pack() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = stream("MailGroupForm");
    let s = parse_spanned(&source).unwrap();
    let base = baseline("MailGroupForm");
    for doc in ["docs/experimental-form-editing.md", "skills/oneform-forms-experimental/SKILL.md"] {
        let manual = std::fs::read_to_string(root.join(doc)).unwrap();
        let from = manual.find("<Attribute id=\"200\"").unwrap_or_else(|| panic!("{doc}: no Attribute example"));
        let example = &manual[from..from + manual[from..].find("</Attribute>").unwrap() + "</Attribute>".len()];
        let edited = base.replacen("  </Attributes>", &format!("    {example}
  </Attributes>"), 1);
        let patched = patch_form(&s, base.as_bytes(), edited.as_bytes()).unwrap_or_else(|e| panic!("{doc}: {example}: {e}")).bytes;
        assert!(text(&patched).contains("\"Примечание\""), "{doc}");
    }
}
