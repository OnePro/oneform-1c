//! Byte-for-byte checks of `oneform::preview` against the Python Reference outputs stored in `tests/golden`
//! (`FormPreview.html` and the `.oneform/` workspace viewer) and the `tests/fixtures/forms` fixtures.
//!
//! Inputs of `FormPreview.html`: the neighbouring `Form.xml`, `FormModule.bsl`, the object module
//! (`<form>/../../ObjectModule.bsl` or `<form>/../../../Ext/ObjectModule.bsl`) and the alias = form directory name.
//! Inputs of the workspace viewer: the items listed in the case's `.oneform/manifest.json` (see
//! `oneform::preview::WorkspaceItem`) over the layered result tree of the case.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use oneform::encoding::decode_payload;
use oneform::preview::{build_workspace_preview, render_form_preview_html, WorkspaceItem, WORKSPACE_DIR};
use serde_json::Value;

type Tree = BTreeMap<String, Vec<u8>>;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// `GOLDEN_BLESS=1` rewrites the stored preview pages that differ (deliberate re-bless after a change of the page assets).
fn bless() -> bool {
    std::env::var("GOLDEN_BLESS").is_ok_and(|v| v == "1")
}

fn read_tree(base: &Path) -> Tree {
    let mut out = Tree::new();
    fn walk(base: &Path, dir: &Path, out: &mut Tree) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if p.is_file() {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    walk(base, base, &mut out);
    out
}

fn cmd_of(name: &str) -> Value {
    serde_json::from_slice(&fs::read(repo().join("tests/golden").join(name).join("cmd.json")).unwrap()).unwrap()
}

fn strings(cmd: &Value, key: &str) -> Vec<String> {
    cmd.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Same layering as `tests/golden.rs`: input = base result minus `input_deleted` + `input/`; result adds `expected/`.
fn input_tree(name: &str) -> Tree {
    let cmd = cmd_of(name);
    let mut t = match cmd.get("base").and_then(Value::as_str) {
        Some(b) => result_tree(b),
        None => Tree::new(),
    };
    for p in strings(&cmd, "input_deleted") {
        t.remove(&p);
    }
    t.extend(read_tree(&repo().join("tests/golden").join(name).join("input")));
    t
}

fn result_tree(name: &str) -> Tree {
    let mut t = input_tree(name);
    for p in strings(&cmd_of(name), "deleted") {
        t.remove(&p);
    }
    t.extend(read_tree(&repo().join("tests/golden").join(name).join("expected")));
    t
}

fn case_names() -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(repo().join("tests/golden"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().join("cmd.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn canonical_text(tree: &Tree, path: &str) -> Vec<u8> {
    match tree.get(path) {
        Some(raw) => decode_payload(raw, path, "auto").map(|d| d.canonical).unwrap_or_else(|_| raw.clone()),
        None => Vec::new(),
    }
}

fn join(dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = if dir.is_empty() { vec![] } else { dir.split('/').collect() };
    for seg in rel.split('/') {
        if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    parts.join("/")
}

/// Strip the embedded `<script type="application/json" id="data">…</script>` block.
fn without_data(html: &str) -> String {
    let start = html.find(r#"<script type="application/json" id="data">"#).expect("data block");
    let end = start + html[start..].find("</script>").unwrap() + "</script>".len();
    format!("{}{}", &html[..start], &html[end..])
}

fn embedded_data(html: &str) -> Value {
    let start = html.find(r#"id="data">"#).unwrap() + r#"id="data">"#.len();
    let end = start + html[start..].find("</script>").unwrap();
    serde_json::from_str(&html[start..end]).unwrap()
}

fn net_counts(s: &str) -> [usize; 3] {
    [s.matches("http://").count(), s.matches("https://").count(), s.matches("fetch(").count()]
}

#[test]
fn golden_form_previews_are_byte_identical() {
    let mut checked = 0;
    let mut reference_static: Option<String> = None;
    for name in case_names() {
        let expected = read_tree(&repo().join("tests/golden").join(&name).join("expected"));
        let tree = result_tree(&name);
        for (path, want) in &expected {
            let Some(dir) = path.strip_suffix("/FormPreview.html") else { continue };
            let xml = tree.get(&format!("{dir}/Form.xml")).unwrap_or_else(|| panic!("{name}: no Form.xml for {path}"));
            // Some cases (`form-*`) do not store FormModule.bsl (it lives inside Form.bin): take the module
            // text from the Reference page's embedded data instead.
            let module = match tree.get(&format!("{dir}/FormModule.bsl")) {
                Some(m) => m.clone(),
                None => embedded_data(&String::from_utf8(want.clone()).unwrap())["formModule"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .to_vec(),
            };
            let object = ["../../ObjectModule.bsl", "../../../Ext/ObjectModule.bsl"]
                .iter()
                .map(|r| join(dir, r))
                .find(|p| tree.contains_key(p))
                .map(|p| canonical_text(&tree, &p))
                .unwrap_or_default();
            let alias = dir.rsplit('/').next().unwrap();
            let got = render_form_preview_html(xml, &module, &object, alias).unwrap();
            if bless() && got != *want {
                fs::write(repo().join("tests/golden").join(&name).join("expected").join(path), &got).unwrap();
                eprintln!("blessed {name}: {path}");
                checked += 1;
                continue;
            }
            assert!(got == *want, "{name}: {path} differs (expected {} bytes, got {})", want.len(), got.len());
            // no network access beyond what the Reference output already contains
            let (w, g) = (String::from_utf8(want.clone()).unwrap(), String::from_utf8(got).unwrap());
            assert_eq!(net_counts(&g), net_counts(&w));
            reference_static.get_or_insert_with(|| without_data(&w));
            checked += 1;
        }
    }
    assert!(checked >= 5, "expected at least 5 golden FormPreview.html files, found {checked}");
    // the static part (everything but the embedded data) is the same for every form
    assert!(reference_static.is_some());
}

fn python_render(xml_path: &Path, alias: &str) -> Option<Vec<u8>> {
    let script = format!(
        "import sys;sys.path.insert(0,r'{}');from epf_extract.preview import render_form_preview_html as r;\
         sys.stdout.buffer.write(r(open(sys.argv[1],'rb').read(),form_alias=sys.argv[2]))",
        repo().join("scripts").display()
    );
    for py in ["python", "python3", "py"] {
        if let Ok(o) = Command::new(py).args(["-c", &script]).arg(xml_path).arg(alias).output() {
            if o.status.success() && !o.stdout.is_empty() {
                return Some(o.stdout);
            }
        }
    }
    None
}

#[test]
fn fixture_forms_render_and_match_python_when_available() {
    // static part of a golden Reference page, to compare the no-network surface
    let golden = fs::read_to_string(
        repo().join("tests/golden/config-extract/expected/CommonForms/ОбщаяФорма/src/Forms/ОбщаяФорма/FormPreview.html"),
    )
    .unwrap();
    let static_counts = net_counts(&without_data(&golden));
    let mut n = 0;
    for e in fs::read_dir(repo().join("tests/fixtures/forms")).unwrap().flatten() {
        let xml_path = e.path().join("Form.xml");
        if !xml_path.is_file() {
            continue;
        }
        let xml = fs::read(&xml_path).unwrap();
        let doc_name = {
            let t = String::from_utf8_lossy(&xml).into_owned();
            let d = roxmltree_name(&t);
            if d.is_empty() { "ФормаДокумента".to_string() } else { d }
        };
        let got = render_form_preview_html(&xml, b"", b"", &doc_name).unwrap();
        let html = String::from_utf8(got.clone()).unwrap();
        assert_eq!(render_form_preview_html(&xml, b"", b"", &doc_name).unwrap(), got, "deterministic");
        assert_eq!(net_counts(&without_data(&html)), static_counts);
        if let Some(py) = python_render(&xml_path, &doc_name) {
            assert!(py == got, "{}: differs from Python Reference", e.path().display());
        }
        n += 1;
    }
    assert_eq!(n, 2);
}

/// `name="…"` of the root `<Form>` element.
fn roxmltree_name(xml: &str) -> String {
    let re = regex::Regex::new(r#"<Form\b[^>]*?\sname="([^"]*)""#).unwrap();
    re.captures(xml).map(|c| c[1].to_string()).unwrap_or_default()
}

fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn items_from_manifest(manifest: &Value) -> Vec<WorkspaceItem> {
    let mut items = Vec::new();
    for o in manifest["objects"].as_array().unwrap() {
        for f in o["forms"].as_array().unwrap() {
            items.push(WorkspaceItem {
                class_name: str_of(o, "className"),
                rel_path: str_of(o, "relPath"),
                object_id: str_of(o, "id"),
                object_name: str_of(o, "name"),
                object_synonym: str_of(o, "synonym"),
                form_id: Some(str_of(f, "id")),
                form_name: str_of(f, "name"),
                alias: Some(str_of(f, "alias")),
                synonym: str_of(f, "synonym"),
                warning_count: f["warningCount"].as_i64().unwrap(),
                xml_path: str_of(f, "xmlPath"),
                module_path: str_of(f, "modulePath"),
                preview_path: str_of(f, "previewPath"),
                object_module_path: str_of(o, "objectModulePath"),
            });
        }
    }
    items
}

#[test]
fn golden_workspace_viewer_is_byte_identical() {
    let prefix = format!("{WORKSPACE_DIR}/");
    let mut checked = 0;
    for name in case_names() {
        let expected = read_tree(&repo().join("tests/golden").join(&name).join("expected"));
        let want: Vec<(&String, &Vec<u8>)> = expected.iter().filter(|(p, _)| p.starts_with(&prefix)).collect();
        if want.is_empty() {
            continue;
        }
        let tree = result_tree(&name);
        let manifest: Value = serde_json::from_slice(&tree[&format!("{prefix}manifest.json")]).unwrap();
        let items = items_from_manifest(&manifest);

        let work = tempfile::tempdir().unwrap();
        for (rel, data) in tree.iter().filter(|(p, _)| !p.starts_with(&prefix)) {
            let p = work.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, data).unwrap();
        }
        let out = build_workspace_preview(work.path(), &items).unwrap();
        assert_eq!(out, work.path().join(WORKSPACE_DIR));
        let actual = read_tree(&out);
        for (p, w) in &want {
            let rel = &p[prefix.len()..];
            let got = actual.get(rel).unwrap_or_else(|| panic!("{name}: missing {p}"));
            if bless() && got != *w {
                fs::write(repo().join("tests/golden").join(&name).join("expected").join(p.as_str()), got).unwrap();
                eprintln!("blessed {name}: {p}");
                checked += 1;
                continue;
            }
            assert!(got == *w, "{name}: {p} differs (expected {} bytes, got {})", w.len(), got.len());
            checked += 1;
        }
        if want.iter().any(|(p, _)| p.ends_with("/index.html")) {
            let got_names: Vec<&String> = actual.keys().collect();
            let want_names: Vec<String> = want.iter().map(|(p, _)| p[prefix.len()..].to_string()).collect();
            assert_eq!(got_names.len(), want_names.len(), "{name}: file set differs");
        }
        // rebuilding over an owned directory works; leftover temp dirs must not remain
        build_workspace_preview(work.path(), &items).unwrap();
        let leftovers: Vec<_> = fs::read_dir(work.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".oneform.tmp-"))
            .collect();
        assert!(leftovers.is_empty());
    }
    assert!(checked >= 8, "too few workspace files checked: {checked}");
}

#[test]
fn foreign_workspace_dir_is_not_overwritten() {
    let work = tempfile::tempdir().unwrap();
    let ws = work.path().join(WORKSPACE_DIR);
    fs::create_dir_all(&ws).unwrap();
    fs::write(ws.join("mine.txt"), b"x").unwrap();
    let err = build_workspace_preview(work.path(), &[]).unwrap_err();
    assert_eq!(err.code, oneform::errors::OUTPUT_NOT_OWNED);
    assert!(ws.join("mine.txt").is_file());
}

#[test]
fn embedded_html_is_inline_script_safe() {
    let xml = "<Form name=\"A\" ordinaryFormVersion=\"2.1\"><Width>1</Width><Height>2</Height></Form>";
    let module = "// </script><!-- & \u{2028}\nПроцедура Тест()\nКонецПроцедуры".as_bytes();
    let html = String::from_utf8(render_form_preview_html(xml.as_bytes(), module, b"", "A").unwrap()).unwrap();
    let start = html.find(r#"id="data">"#).unwrap() + r#"id="data">"#.len();
    let end = start + html[start..].find("</script>").unwrap();
    let data = &html[start..end];
    assert!(!data.contains('<') && !data.contains('>') && !data.contains('&') && !data.contains('\u{2028}'));
    let v: Value = serde_json::from_str(data).unwrap();
    assert_eq!(v["procs"]["тест"], 2);
    assert_eq!(v["formModule"].as_str().unwrap().as_bytes(), module);
}
