//! `extract` of data links: `steps="..."` on every link of two or more steps, `[token]` for what has no name,
//! the pseudo root `-2`, the standard codes `-8` (Движения of a document) and `-60001` (Отбор), K = 1..5.

mod common;

use std::collections::HashMap;
use std::path::Path;

use oneform::oof::{decode_oof_stream, render_form_xml};

const U1: &str = "32c6f63b-a441-4745-a7a0-4b173052935c";
const U2: &str = "17fd118e-1906-418c-8ae8-3b5f8518eb01";
const UNKNOWN: &str = "116ed66f-3305-4f06-88b0-6c2538664b23";

fn fixture() -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm/FormStream.bin")).unwrap()
}

/// `extract` of the DonorForm stream whose link row of control 7 (`ПолеВвода1`, form attribute 3) is `body`:
/// `(attributeId, steps, text)` of its `<DataPath>`.
fn shown(body: &str) -> (String, Option<String>, String) {
    let row = "{7,\r\n{1,\r\n{3}\r\n}\r\n}";
    let mut raw = fixture();
    let at = raw.windows(row.len()).position(|w| w == row.as_bytes()).expect("link row of control 7");
    raw.splice(at..at + row.len(), format!("{{7,{body}}}").bytes());
    let lookup: HashMap<String, String> =
        [("@kind", "Document"), (U1, "Организация"), (U2, "Наименование2")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let model = decode_oof_stream(&raw, "x", Some(&lookup)).unwrap();
    let xml = render_form_xml(&common::form_info("x", "Форма", &raw), &model).unwrap();
    let xml = String::from_utf8(xml).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let control = doc.descendants().find(|n| n.has_tag_name("InputField") && n.attribute("id") == Some("7")).expect("control 7");
    let dp = control.children().find(|n| n.has_tag_name("DataPath")).expect("DataPath");
    (
        dp.attribute("attributeId").unwrap().to_string(),
        dp.attribute("steps").map(str::to_string),
        dp.text().unwrap_or("").to_string(),
    )
}

fn case(body: &str, aid: &str, steps: Option<&str>, text: &str) {
    assert_eq!(shown(body), (aid.to_string(), steps.map(str::to_string), text.to_string()), "{body}");
}

#[test]
fn links_of_one_to_five_rows_keep_every_step() {
    // K = 1: no steps
    case("{1,{1}}", "1", None, "ОбработкаОбъект");
    // K = 2: a standard attribute of the form owner (Document: -3 is Дата), a known attribute, an unknown one
    case("{2,{1},{-3}}", "1", Some("-3"), "ОбработкаОбъект.Дата");
    case(&format!("{{2,{{1}},{{0,{U1}}}}}"), "1", Some(U1), "ОбработкаОбъект.Организация");
    case(&format!("{{2,{{1}},{{0,{UNKNOWN}}}}}"), "1", Some(UNKNOWN), &format!("ОбработкаОбъект.[{UNKNOWN}]"));
    // K = 3: a reference field of a reference; the standard code -8 of a document is Движения
    case(&format!("{{3,{{1}},{{0,{U1}}},{{0,{U2}}}}}"), "1", Some(&format!("{U1} {U2}")), "ОбработкаОбъект.Организация.Наименование2");
    case(&format!("{{3,{{1}},{{-8}},{{0,{UNKNOWN}}}}}"), "1", Some(&format!("-8 {UNKNOWN}")), &format!("ОбработкаОбъект.Движения.[{UNKNOWN}]"));
    // K = 4: -60001 is Отбор; the last code belongs to a kind that is not known
    case(
        &format!("{{4,{{1}},{{-60001}},{{0,{U1}}},{{-6}}}}"),
        "1",
        Some(&format!("-60001 {U1} -6")),
        "ОбработкаОбъект.Отбор.Организация.[-6]",
    );
    // K = 5: positive ordinals are never named
    case(
        &format!("{{5,{{1}},{{0,{UNKNOWN}}},{{0}},{{1}},{{0,{U1}}}}}"),
        "1",
        Some(&format!("{UNKNOWN} 0 1 {U1}")),
        &format!("ОбработкаОбъект.[{UNKNOWN}].[0].[1].Организация"),
    );
}

#[test]
fn the_row_root_is_shown_as_the_form_tables_and_a_missing_attribute_is_bracketed() {
    // `-2` is not a form attribute: the current row of the table with the id of the first step (table 6)
    case("{3,{-2},{6},{0}}", "-2", Some("6 0"), "ЭлементыФормы.ТабличноеПоле1.[0]");
    case(&format!("{{5,{{-2}},{{6}},{{0}},{{0,{U1}}},{{-3}}}}"), "-2", Some(&format!("6 0 {U1} -3")), "ЭлементыФормы.ТабличноеПоле1.[0].Организация.[-3]");
    // the step is not a table of this form: the step stays visible
    case("{2,{-2},{9999}}", "-2", Some("9999"), "ЭлементыФормы.[9999]");
    // a root that is not an attribute of the form
    case("{2,{99},{-2}}", "99", Some("-2"), "[99].[-2]");
}

#[test]
fn the_tokens_of_q_steps_and_standard_codes_are_kept_verbatim() {
    case(&format!("{{3,{{1}},{{2,{U1}}},{{3,{U2}}}}}"), "1", Some(&format!("2:{U1} 3:{U2}")), "ОбработкаОбъект.Организация.Наименование2");
    case("{4,{1},{-2},{-8},{-60001}}", "1", Some("-2 -8 -60001"), "ОбработкаОбъект.Номер.[-8].Отбор");
}
