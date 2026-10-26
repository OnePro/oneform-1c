//! Ordinary-form decoder: the six fixtures (`tests/fixtures/forms/*`) must reproduce the Reference `Form.xml`
//! byte for byte, the white-box cases of `tests/test_oof.py` that need the fixtures, and the `props`
//! golden cases (`tests/golden/props-*`).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oneform::oof::{decode_oof_stream, render_form_xml};
use serde_json::{json, Value};

const FIXTURE_NAMES: [&str; 2] = ["DonorForm", "DonorFormAlt"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn fixtures() -> PathBuf {
    repo().join("tests/fixtures/forms")
}

fn fixture_form(name: &str) -> Value {
    let d = fixtures().join(name);
    let lookup = common::read_meta_lookup(&d.join("Document.xml"));
    decode_oof_stream(&std::fs::read(d.join("FormStream.bin")).unwrap(), "u", Some(&lookup)).unwrap()
}

fn find_ctrl<'a>(controls: &'a Value, name: &str) -> Option<&'a Value> {
    if let Some(arr) = controls.as_array() {
        for c in arr {
            if c["name"] == name {
                return Some(c);
            }
            if let Some(found) = find_ctrl(&c["children"], name) {
                return Some(found);
            }
            if let Some(pages) = c["pages"].as_array() {
                for p in pages {
                    if let Some(found) = find_ctrl(&p["children"], name) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn find_model_ctrl<'a>(model: &'a Value, name: &str) -> Option<&'a Value> {
    if let Some(found) = find_ctrl(&model["controls"], name) {
        return Some(found);
    }
    if let Some(pages) = model["pages"].as_array() {
        for p in pages {
            if let Some(found) = find_ctrl(&p["children"], name) {
                return Some(found);
            }
        }
    }
    None
}

fn find_el<'a, 'i>(e: roxmltree::Node<'a, 'i>, tag: &str) -> roxmltree::Node<'a, 'i> {
    e.children().find(|c| c.has_tag_name(tag)).unwrap()
}

fn ctrl<'a>(model: &'a Value, name: &str) -> &'a Value {
    find_model_ctrl(model, name).unwrap_or_else(|| panic!("control {name} not found"))
}

#[test]
fn fixture_forms_are_byte_identical_to_the_reference() {
    for name in FIXTURE_NAMES {
        let d = fixtures().join(name);
        let expected = std::fs::read(d.join("Form.xml")).unwrap();
        let text = String::from_utf8(expected.clone()).unwrap();
        let doc = roxmltree::Document::parse(&text).unwrap();
        let root = doc.root_element();
        let (fid, fname) = (root.attribute("id").unwrap(), root.attribute("name").unwrap());
        let raw = std::fs::read(d.join("FormStream.bin")).unwrap();
        let lookup = common::read_meta_lookup(&d.join("Document.xml"));
        let model = decode_oof_stream(&raw, fid, Some(&lookup)).unwrap();
        let info = common::form_info(fid, fname, &raw);
        let xml = render_form_xml(&info, &model).unwrap();
        let xml = oneform::form_patch::enrich_ext_properties(&raw, &xml).unwrap();
        // The fixtures are stored LF-normalized (`*.xml text eol=lf`), while text values with embedded
        // CRLF (tooltips) are emitted verbatim by the Reference; so compare with CRLF folded to LF.
        let xml = String::from_utf8(xml).unwrap().replace("\r\n", "\n");
        if std::env::var("GOLDEN_BLESS").is_ok() {
            std::fs::write(d.join("Form.xml"), xml.as_bytes()).unwrap();
            continue;
        }
        assert!(xml.as_bytes() == expected.as_slice(), "{name}: Form.xml differs");
    }
}

// port: tests/test_oof.py::test_fixture_forms_decode_without_warnings
#[test]
fn fixture_forms_decode_without_warnings() {
    for name in FIXTURE_NAMES {
        assert_eq!(fixture_form(name)["warnings"], json!([]), "{name}");
    }
}

#[test]
fn donor_controls_events_and_attributes() {
    let model = fixture_form("DonorForm");
    let b = ctrl(&model, "Кнопка1");
    assert_eq!(b["type"], "Button");
    assert_eq!(b["width"], 53);
    assert_eq!(b["height"], 17);

    let chk = ctrl(&model, "Флажок1");
    assert_eq!(chk["type"], "CheckBox");
    assert!(chk["caption"].as_array().unwrap().contains(&json!(["ru", "Флажок1"])));

    let tbl = ctrl(&model, "ТабличноеПоле1");
    assert_eq!(tbl["type"], "Table");
    assert_eq!(tbl["dataPath"], json!({"attributeId": 2, "path": "ТабличноеПоле1"}));
    let cols = tbl["columns"].as_array().unwrap();
    assert_eq!(cols.len(), 2);
    assert_eq!(cols[0]["name"], "Колонка1");
    assert_eq!(cols[0]["width"], 100);
    assert!(!cols[0]["events"].as_array().unwrap().is_empty());

    let inp = ctrl(&model, "ПолеВвода1");
    assert_eq!(inp["type"], "InputField");
    assert_eq!(inp["dataPath"], json!({"attributeId": 3, "path": "ПолеВвода1"}));
    assert_eq!(inp["props"]["choiceButton"], true);
    assert_eq!(inp["props"]["openButton"], true);
    assert_eq!(inp["props"]["clearButton"], true);
    assert_eq!(inp["props"]["autoMarkIncomplete"], true);
    assert_eq!(inp["events"].as_array().unwrap().len(), 9);

    let bar = &ctrl(&model, "ОсновныеДействияФормы")["props"];
    assert_eq!(bar["autoFill"], false);
    assert_eq!(bar["horizontalAlign"], "Right");
    let buttons = bar["buttons"].as_array().unwrap();
    assert!(buttons.iter().any(|x| x["name"] == "ОсновныеДействияФормыВыполнить"));
    assert!(buttons.iter().any(|x| x["name"] == "ОсновныеДействияФормыЗакрыть"));

    let bar2 = &ctrl(&model, "КоманднаяПанель2")["props"];
    let b2 = bar2["buttons"].as_array().unwrap();
    let sub = b2.iter().find(|x| x["name"] == "Подменю").unwrap();
    assert_eq!(sub["kind"], "Submenu");
    assert!(!sub["buttons"].as_array().unwrap().is_empty());

    let chart = ctrl(&model, "Диаграмма1");
    assert_eq!(chart["type"], "Chart");
    assert_eq!(chart["events"].as_array().unwrap().len(), 4);

    let pdf = ctrl(&model, "ПолеPDFДокумента1");
    assert_eq!(pdf["type"], "PDFDocumentField");
}

#[test]
fn form_xml_has_geometry_order_and_properties() {
    let model = fixture_form("DonorForm");
    let xml = render_form_xml(&common::form_info("u", "F", b""), &model).unwrap();
    let text = String::from_utf8(xml).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let iter = |tag: &'static str| doc.descendants().filter(move |e| e.is_element() && e.tag_name().name() == tag);
    let named = |tag: &'static str, name: &str| iter(tag).find(|e| e.attribute("name") == Some(name)).unwrap();
    let find_text = |e: roxmltree::Node<'_, '_>, tag: &str| {
        e.children().find(|c| c.has_tag_name(tag)).map(|c| c.text().unwrap_or("").to_string())
    };
    let btn = named("Button", "Кнопка1");
    assert_eq!(find_text(btn, "Width").as_deref(), Some("53"));
    let dp = iter("DataPath").find(|e| e.text() == Some("ПолеВвода1")).unwrap();
    assert_eq!(dp.attribute("attributeId"), Some("3"));
    let col = named("Column", "Колонка1");
    assert_eq!(find_text(col, "Width").as_deref(), Some("100"));
    let bar = named("CommandBar", "КоманднаяПанель2");
    let buttons = find_el(bar, "Buttons");
    assert!(buttons.children().any(|b| b.has_tag_name("Button") && b.attribute("kind") == Some("Submenu")));
}

// ---------------------------------------------------------------- props golden cases

type Tree = BTreeMap<String, Vec<u8>>;

fn read_tree(base: &Path) -> Tree {
    fn walk(base: &Path, dir: &Path, out: &mut Tree) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if p.is_file() {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = Tree::new();
    walk(base, base, &mut out);
    out
}

struct Case {
    dir: PathBuf,
    cmd: Value,
}

impl Case {
    fn load(name: &str) -> Case {
        let dir = repo().join("tests/golden").join(name);
        let cmd = serde_json::from_slice(&std::fs::read(dir.join("cmd.json")).unwrap()).unwrap();
        Case { dir, cmd }
    }

    fn strings(&self, key: &str) -> Vec<String> {
        self.cmd
            .get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    }

    fn input_tree(&self) -> Tree {
        let mut t = match self.cmd.get("base").and_then(Value::as_str) {
            Some(b) => result_tree(&Case::load(b)),
            None => Tree::new(),
        };
        for p in self.strings("input_deleted") {
            t.remove(&p);
        }
        t.extend(read_tree(&self.dir.join("input")));
        t
    }
}

fn result_tree(c: &Case) -> Tree {
    let mut t = c.input_tree();
    for p in c.strings("deleted") {
        t.remove(&p);
    }
    t.extend(read_tree(&c.dir.join("expected")));
    t
}

#[test]
fn props_golden_cases() {
    let golden = repo().join("tests/golden");
    let mut names: Vec<String> = std::fs::read_dir(&golden)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("props-"))
        .collect();
    names.sort();
    assert!(names.len() >= 5, "{names:?}");
    for name in names {
        let case = Case::load(&name);
        let work = std::env::temp_dir().join(format!("props-golden-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        for (rel, data) in case.input_tree() {
            let p = work.join(&rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        }
        let prefix = format!("{}/", work.to_string_lossy().replace('\\', "/"));
        let argv = case.strings("argv");
        assert_eq!(argv[1], "props");
        // file arguments are relative to the (virtual) working directory
        let args: Vec<String> = argv[2..]
            .iter()
            .map(|a| if a.ends_with(".bin") { format!("{prefix}{a}") } else { a.clone() })
            .collect();
        let (code, out, err) = oneform::props::run(&args);
        let (out, err) = (out.replace(&prefix, ""), err.replace(&prefix, ""));
        let want_code: i32 = std::fs::read_to_string(case.dir.join("exit_code")).unwrap().trim().parse().unwrap();
        let want_out = std::fs::read(case.dir.join("stdout.txt")).unwrap();
        let want_err = std::fs::read(case.dir.join("stderr.txt")).unwrap();
        assert_eq!(code, want_code, "{name}: exit code");
        assert!(out.as_bytes() == want_out.as_slice(), "{name}: stdout differs");
        assert!(err.as_bytes() == want_err.as_slice(), "{name}: stderr differs: {err:?}");
        let _ = std::fs::remove_dir_all(&work);
    }
}
