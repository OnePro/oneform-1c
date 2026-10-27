//! Offline 1:1 HTML preview of an ordinary form (`FormPreview.html`) and the `.oneform/` workspace viewer.
//!
//! Port of `scripts/epf_extract/preview.py`; output is byte-identical to the Reference. The three static assets
//! are embedded from `scripts/epf_extract/assets/` (single source of truth). The output is deterministic: the only
//! nondeterministic part (the temp directory name used for atomic publishing) never reaches the written bytes.
//!
//! Known approximations: Python `str.casefold()` is emulated by `to_lowercase()` plus the `ß`/`ς`/`ſ` special
//! cases; Python `int()` accepts only ASCII digits here.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::canonical::{dumps_with, COMPACT_SORTED, PRETTY};
use crate::encoding::decode_payload;
use crate::errors::{ExtractionError, Result, FORM_PREVIEW_FAILED, IO_ERROR, OUTPUT_NOT_OWNED};

pub const FORM_PREVIEW_NAME: &str = "FormPreview.html";
/// `ordinaryFormVersion` values the preview reads (the format only grew by attributes it ignores).
const VERSIONS: [&str; 2] = ["2.1", "2.2"];

pub const WORKSPACE_DIR: &str = ".oneform";
pub const WORKSPACE_SCHEMA_VERSION: i64 = 1;
const GENERATOR: &str = "oneform";
const UNKNOWN_CLASS: &str = "Other";
const UNKNOWN_TITLE: &str = "Прочие формы";

/// `formbin.OBJECT_CLASS_TITLES` (order is significant: it is the group order of the viewer).
pub const OBJECT_CLASS_TITLES: [(&str, &str); 18] = [
    ("CommonForms", "Общие формы"),
    ("Catalogs", "Справочники"),
    ("Documents", "Документы"),
    ("DocumentJournals", "Журналы документов"),
    ("Reports", "Отчёты"),
    ("DataProcessors", "Обработки"),
    ("ExternalDataProcessors", "Внешние обработки"),
    ("InformationRegisters", "Регистры сведений"),
    ("AccumulationRegisters", "Регистры накопления"),
    ("AccountingRegisters", "Регистры бухгалтерии"),
    ("CalculationRegisters", "Регистры расчёта"),
    ("ChartsOfAccounts", "Планы счетов"),
    ("ChartsOfCharacteristicTypes", "Планы видов характеристик"),
    ("ChartsOfCalculationTypes", "Планы видов расчёта"),
    ("ExchangePlans", "Планы обмена"),
    ("FilterCriteria", "Критерии отбора"),
    ("BusinessProcesses", "Бизнес-процессы"),
    ("Tasks", "Задачи"),
];

/// Python `open(..., encoding="utf-8").read()`: universal newlines.
fn universal_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn theme_css() -> &'static str {
    static S: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    S.get_or_init(|| universal_newlines(include_str!("../assets/form_theme.css")))
}

fn hljs_js() -> &'static str {
    static S: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    S.get_or_init(|| universal_newlines(include_str!("../assets/hljs_1c_xml.js")))
}

fn render_js() -> &'static str {
    static S: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    S.get_or_init(|| universal_newlines(include_str!("../assets/form_render.js")))
}

fn proc_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\s*(Процедура|Функция|Procedure|Function)\s+(\w+)").expect("static regex"))
}

/// `html.escape(s, quote=True)`.
pub fn html_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#x27;"),
            c => o.push(c),
        }
    }
    o
}

/// Approximation of Python `str.casefold()`.
pub fn casefold(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ß' => o.push_str("ss"),
            'ς' => o.push('σ'),
            'ſ' => o.push('s'),
            c => o.extend(c.to_lowercase()),
        }
    }
    o
}

/// `preview._json`: compact sorted JSON, safe inside an inline `<script>` (`< > & U+2028 U+2029` escaped).
pub fn json_embed(v: &Value) -> Result<String> {
    let s = dumps_with(v, &COMPACT_SORTED)?;
    // One pass instead of five `str.replace` calls (same result: the replaced characters never overlap).
    if !s.bytes().any(|b| matches!(b, b'<' | b'>' | b'&' | 0xe2)) {
        return Ok(s);
    }
    let mut out = String::with_capacity(s.len() + 64);
    let mut start = 0;
    for (i, c) in s.char_indices() {
        let rep = match c {
            '<' => "\\u003c",
            '>' => "\\u003e",
            '&' => "\\u0026",
            '\u{2028}' => "\\u2028",
            '\u{2029}' => "\\u2029",
            _ => continue,
        };
        out.push_str(&s[start..i]);
        out.push_str(rep);
        start = i + c.len_utf8();
    }
    out.push_str(&s[start..]);
    Ok(out)
}

// ---------------------------------------------------------------- XML helpers (ElementTree semantics)

fn parse_xml(text: &str) -> std::result::Result<roxmltree::Document<'_>, roxmltree::Error> {
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    roxmltree::Document::parse_with_options(text.trim_start_matches('\u{feff}'), opts)
}

fn is_tag(n: roxmltree::Node, tag: &str) -> bool {
    n.is_element() && n.tag_name().namespace().is_none() && n.tag_name().name() == tag
}

fn children<'a, 'i>(n: roxmltree::Node<'a, 'i>, tag: &'a str) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    n.children().filter(move |c| is_tag(*c, tag))
}

/// ET `.text`: text before the first child element.
fn et_text<'a>(n: roxmltree::Node<'a, '_>) -> &'a str {
    match n.first_child() {
        Some(c) if c.is_text() => c.text().unwrap_or(""),
        _ => "",
    }
}

fn et_tag(n: roxmltree::Node) -> String {
    match n.tag_name().namespace() {
        Some(ns) => format!("{{{}}}{}", ns, n.tag_name().name()),
        None => n.tag_name().name().to_string(),
    }
}

/// Python `int(s.strip())` for ASCII digits (optional sign, single underscores between digits); 0 on error.
fn py_int(s: &str) -> i128 {
    let t = s.trim();
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let b = body.as_bytes();
    if b.is_empty() || !b[0].is_ascii_digit() || !b[b.len() - 1].is_ascii_digit() {
        return 0;
    }
    let mut digits = String::new();
    let mut prev_us = false;
    for &c in b {
        if c == b'_' {
            if prev_us {
                return 0;
            }
            prev_us = true;
        } else if c.is_ascii_digit() {
            prev_us = false;
            digits.push(c as char);
        } else {
            return 0;
        }
    }
    match digits.parse::<i128>() {
        Ok(v) => {
            if neg {
                -v
            } else {
                v
            }
        }
        Err(_) => 0,
    }
}

fn int_child(e: roxmltree::Node, tag: &str) -> i128 {
    match children(e, tag).next() {
        Some(c) => py_int(et_text(c)),
        None => 0,
    }
}

/// `./Caption/Item` (or `./Title/Item` for `Page`) mapped `language -> text`; the last item of a language wins.
fn loc(e: roxmltree::Node) -> BTreeMap<String, String> {
    let holder = if e.tag_name().name() == "Page" && e.tag_name().namespace().is_none() { "Title" } else { "Caption" };
    let mut m = BTreeMap::new();
    for h in children(e, holder) {
        for i in children(h, "Item") {
            m.insert(i.attribute("language").unwrap_or("").to_string(), et_text(i).to_string());
        }
    }
    m
}

/// True when `data` is a `Form.xml` of an ordinary form (`<Form ordinaryFormVersion="2.2">`).
pub fn is_ordinary_form_xml(data: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(data) else { return false };
    let Ok(doc) = parse_xml(text) else { return false };
    let root = doc.root_element();
    is_tag(root, "Form") && root.attribute("ordinaryFormVersion").is_some_and(|v| VERSIONS.contains(&v))
}

fn normalize_nl(s: &str) -> String {
    universal_newlines(s)
}

/// Render `FormPreview.html` for a `Form.xml` (+ optional form/object module sources).
pub fn render_form_preview_html(
    form_xml: &[u8],
    module_bsl: &[u8],
    object_module_bsl: &[u8],
    form_alias: &str,
) -> Result<Vec<u8>> {
    let xml_lossy = String::from_utf8_lossy(form_xml).into_owned();
    let doc = parse_xml(&xml_lossy)
        .map_err(|e| ExtractionError::new(FORM_PREVIEW_FAILED, format!("Form.xml is not well-formed: {e}")))?;
    let root = doc.root_element();
    let width = int_child(root, "Width");
    let height = int_child(root, "Height");
    let name = match root.attribute("name") {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => form_alias.to_string(),
    };
    let caption = loc(root).get("ru").cloned().unwrap_or_default();

    let mod_text = String::from_utf8_lossy(module_bsl).into_owned();
    let obj_text = String::from_utf8_lossy(object_module_bsl).into_owned();
    let re = proc_re();
    let mut procs = Map::new();
    for (i, line) in normalize_nl(&mod_text).split('\n').enumerate() {
        if let Some(pm) = re.captures(line) {
            let key = casefold(&pm[2]);
            if !procs.contains_key(&key) {
                procs.insert(key, Value::from(i as u64 + 1));
            }
        }
    }
    let mut form_events = Vec::new();
    for ev in children(root, "Events") {
        for e in ev.children().filter(|c| c.is_element()) {
            form_events.push(json!({"name": et_tag(e), "handler": et_text(e).trim()}));
        }
    }
    let alias = if form_alias.is_empty() { name.as_str() } else { form_alias };
    let data = json!({
        "xml": xml_lossy,
        "formModule": mod_text,
        "objectModule": obj_text,
        "alias": alias,
        "procs": Value::Object(procs),
        "formEvents": form_events,
    });
    let obj_tab =
        if obj_text.is_empty() { "" } else { r#"<button class="tab" data-tab="object">Модуль объекта</button>"# };
    let obj_pane = if obj_text.is_empty() { "" } else { r#"<div class="pane" id="p-object"></div>"# };
    let name_e = html_escape(&name);
    let caption_e = html_escape(&caption);
    let theme = theme_css();
    let hljs = hljs_js();
    let render = render_js();
    let data_json = json_embed(&data)?;
    let page = format!(
        r##"<!DOCTYPE html>
<html lang="ru"><head><meta charset="utf-8"><title>{name_e} — FormPreview</title>
<style>{theme}{CODE_CSS}{CSS}</style></head>
<body class="insp">
<div class="topbar"><span class="ti">{name_e}</span><span>{caption_e}</span><span class="sz">{width} × {height}</span>
<button class="tab on" data-tab="form">Форма (1:1)</button><button class="tab" data-tab="module">Модуль формы</button>{obj_tab}<button class="tab" data-tab="xml">Form.xml</button>
<select id="lang"></select>
<select id="mode"><option value="designer">Конфигуратор</option><option value="runtime">Предприятие</option></select>
<label><input type="checkbox" id="hid"> Показывать скрытые</label>
<select id="zoom"><option value="100">100%</option><option value="125">125%</option><option value="150">150%</option><option value="fit">Вписать</option></select>
<label><input type="checkbox" id="insp" checked> Режим инспектора</label></div>
<div id="main"><div id="view">
<div class="pane on" id="p-form"></div>
<div class="pane" id="p-module"></div>
{obj_pane}
<div class="pane" id="p-xml"></div>
</div>
<div id="side" class="insp"><h3>Инспектор элементов Form.xml</h3><input id="q" placeholder="Поиск"><div id="tree"></div>
<div id="info"></div><div style="padding:4px 8px;border-top:1px solid #aaa"><b>События формы</b><div id="fev"></div></div></div></div>
<script type="application/json" id="data">{data_json}</script>
<script>{hljs}</script>
<script>{render}</script>
<script>{JS}</script>
</body></html>
"##
    );
    Ok(page.into_bytes())
}

// ---------------------------------------------------------------- workspace viewer (.oneform/)

/// One form of the workspace viewer (mirrors the item dicts of `oneform_cli.collect_items`).
///
/// Empty strings stand for "absent" (Python `None`/`""`), except `alias`/`form_id`, where `None` matters for the
/// form key hash (Python formats a missing value as `None`). Paths are workspace-relative POSIX paths.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceItem {
    pub class_name: String,
    pub rel_path: String,
    pub object_id: String,
    pub object_name: String,
    pub object_synonym: String,
    pub form_id: Option<String>,
    pub form_name: String,
    pub alias: Option<String>,
    pub synonym: String,
    pub warning_count: i64,
    pub xml_path: String,
    pub module_path: String,
    pub preview_path: String,
    pub object_module_path: String,
}

fn io_err(e: std::io::Error, what: &Path) -> ExtractionError {
    ExtractionError::new(IO_ERROR, format!("{}: {e}", what.display()))
}

/// LF-normalized text of a workspace file ('' when missing or unreadable).
fn read_text(root: &Path, rel: &str) -> String {
    if rel.is_empty() {
        return String::new();
    }
    let Ok(raw) = fs::read(root.join(rel)) else { return String::new() };
    match decode_payload(&raw, rel, "auto") {
        Ok(d) => d.text,
        Err(_) => universal_newlines(String::from_utf8_lossy(&raw).trim_start_matches('\u{feff}')),
    }
}

/// `(width, height, control count)` of a `Form.xml` text; zeros when unavailable.
fn form_geometry(xml: &str) -> (i128, i128, usize) {
    if xml.is_empty() {
        return (0, 0, 0);
    }
    let Ok(doc) = parse_xml(xml) else { return (0, 0, 0) };
    let root = doc.root_element();
    let count = root
        .descendants()
        .filter(|n| is_tag(*n, "ChildItems"))
        .map(|ci| ci.children().filter(|c| c.is_element()).count())
        .sum();
    (int_child(root, "Width"), int_child(root, "Height"), count)
}

fn check_workspace_owner(out: &Path) -> Result<()> {
    if !out.exists() {
        return Ok(());
    }
    let ok = fs::read(out.join("owner.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|v| v.as_object().is_some_and(|o| o.get("generator").and_then(Value::as_str) == Some(GENERATOR)))
        .unwrap_or(false);
    if !ok {
        let busy = !out.is_dir() || fs::read_dir(out).map(|mut r| r.next().is_some()).unwrap_or(true);
        if busy {
            return Err(ExtractionError::new(
                OUTPUT_NOT_OWNED,
                format!("{WORKSPACE_DIR} exists and is not owned by oneform"),
            ));
        }
    }
    Ok(())
}

/// SHA-1 (only for the Reference-compatible short ids; not security relevant).
fn sha1_hex(data: &[u8]) -> String {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, b) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().fold(String::new(), |mut s, v| {
        let _ = write!(s, "{v:08x}");
        s
    })
}

struct FormRec {
    key: String,
    id: String,
    name: String,
    alias: String,
    synonym: String,
    width: i128,
    height: i128,
    control_count: usize,
    warning_count: i64,
    xml_path: String,
    module_path: String,
    preview_path: String,
    data_script: String,
}

struct ObjRec {
    id: String,
    name: String,
    synonym: String,
    class_name: String,
    class_title: String,
    rel_path: String,
    object_module_path: String,
    has_object_module: bool,
    forms: Vec<FormRec>,
}

fn num(v: i128) -> Value {
    match i64::try_from(v) {
        Ok(i) => Value::from(i),
        Err(_) => Value::from(v as f64),
    }
}

fn form_full(f: &FormRec) -> Value {
    json!({
        "key": f.key, "id": f.id, "name": f.name, "alias": f.alias, "synonym": f.synonym,
        "width": num(f.width), "height": num(f.height), "controlCount": f.control_count,
        "warningCount": f.warning_count, "xmlPath": f.xml_path, "modulePath": f.module_path,
        "previewPath": f.preview_path, "dataScript": f.data_script,
    })
}

fn form_slim(f: &FormRec) -> Value {
    json!({
        "key": f.key, "id": f.id, "name": f.name, "alias": f.alias, "synonym": f.synonym,
        "width": num(f.width), "height": num(f.height), "controlCount": f.control_count,
        "warningCount": f.warning_count, "previewPath": f.preview_path, "dataScript": f.data_script,
    })
}

fn obj_head(o: &ObjRec) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), o.id.clone().into());
    m.insert("name".into(), o.name.clone().into());
    m.insert("synonym".into(), o.synonym.clone().into());
    m.insert("className".into(), o.class_name.clone().into());
    m
}

/// Compute all files (relative name -> bytes) of the workspace viewer, reading sources under `root`.
pub fn workspace_files(root: &Path, items: &[WorkspaceItem]) -> Result<BTreeMap<String, Vec<u8>>> {
    let order: BTreeMap<&str, usize> = OBJECT_CLASS_TITLES.iter().enumerate().map(|(i, (c, _))| (*c, i)).collect();
    let title_of = |c: &str| OBJECT_CLASS_TITLES.iter().find(|(k, _)| *k == c).map(|(_, t)| *t);

    let mut objects: Vec<ObjRec> = Vec::new();
    let mut index: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut form_data: BTreeMap<String, Value> = BTreeMap::new();
    let mut omod_cache: BTreeMap<usize, String> = BTreeMap::new();
    for it in items {
        let cls = if title_of(&it.class_name).is_some() { it.class_name.as_str() } else { UNKNOWN_CLASS };
        let rel = it.rel_path.as_str();
        let oi = *index.entry((cls.to_string(), rel.to_string())).or_insert_with(|| {
            let om_path = it.object_module_path.clone();
            let omod = read_text(root, &om_path);
            let idx = objects.len();
            objects.push(ObjRec {
                id: if it.object_id.is_empty() {
                    sha1_hex(format!("{cls}|{rel}").as_bytes())[..12].to_string()
                } else {
                    it.object_id.clone()
                },
                name: if it.object_name.is_empty() {
                    rel.rsplit('/').next().unwrap_or("").to_string()
                } else {
                    it.object_name.clone()
                },
                synonym: it.object_synonym.clone(),
                class_name: cls.to_string(),
                class_title: title_of(cls).unwrap_or(UNKNOWN_TITLE).to_string(),
                rel_path: rel.to_string(),
                object_module_path: om_path,
                has_object_module: !omod.is_empty(),
                forms: Vec::new(),
            });
            omod_cache.insert(idx, omod);
            idx
        });
        let xml = read_text(root, &it.xml_path);
        let omod = omod_cache.get(&oi).cloned().unwrap_or_default();
        let (width, height, count) = form_geometry(&xml);
        let alias_s = it.alias.clone().unwrap_or_else(|| "None".into());
        let id_s = it.form_id.clone().unwrap_or_else(|| "None".into());
        let key = sha1_hex(format!("{rel}\0{alias_s}\0{id_s}").as_bytes())[..12].to_string();
        let alias = it.alias.clone().unwrap_or_default();
        objects[oi].forms.push(FormRec {
            key: key.clone(),
            id: it.form_id.clone().unwrap_or_default(),
            name: if it.form_name.is_empty() { alias.clone() } else { it.form_name.clone() },
            alias,
            synonym: it.synonym.clone(),
            width,
            height,
            control_count: count,
            warning_count: it.warning_count,
            xml_path: it.xml_path.clone(),
            module_path: it.module_path.clone(),
            preview_path: it.preview_path.clone(),
            data_script: format!("forms/{key}.data.js"),
        });
        form_data.insert(
            key,
            json!({
                "xml": xml,
                "formModule": read_text(root, &it.module_path),
                "objectModule": omod,
                "objectModulePath": objects[oi].object_module_path,
            }),
        );
    }

    objects.sort_by(|a, b| {
        let ka = (order.get(a.class_name.as_str()).copied().unwrap_or(order.len()), casefold(&a.name), &a.rel_path);
        let kb = (order.get(b.class_name.as_str()).copied().unwrap_or(order.len()), casefold(&b.name), &b.rel_path);
        ka.cmp(&kb)
    });
    for o in &mut objects {
        o.forms.sort_by(|a, b| (casefold(&a.name), &a.alias, &a.id).cmp(&(casefold(&b.name), &b.alias, &b.id)));
    }
    let total_forms: usize = objects.iter().map(|o| o.forms.len()).sum();
    let total_warnings: i64 = objects.iter().flat_map(|o| o.forms.iter()).map(|f| f.warning_count).sum();

    let mut groups: Vec<Value> = Vec::new();
    let mut last_class: Option<&str> = None;
    for o in &objects {
        if last_class != Some(o.class_name.as_str()) {
            groups.push(json!({"className": o.class_name, "title": o.class_title, "objects": []}));
            last_class = Some(o.class_name.as_str());
        }
        let mut slim = obj_head(o);
        slim.insert("relPath".into(), o.rel_path.clone().into());
        slim.insert("hasObjectModule".into(), o.has_object_module.into());
        slim.insert("forms".into(), Value::Array(o.forms.iter().map(form_slim).collect()));
        if let Some(Value::Array(list)) = groups.last_mut().and_then(|g| g.get_mut("objects")) {
            list.push(Value::Object(slim));
        }
    }
    let totals = json!({"objects": objects.len(), "forms": total_forms, "warnings": total_warnings});

    let manifest_objects: Vec<Value> = objects
        .iter()
        .map(|o| {
            let mut m = obj_head(o);
            m.insert("classTitle".into(), o.class_title.clone().into());
            m.insert("relPath".into(), o.rel_path.clone().into());
            m.insert("objectModulePath".into(), o.object_module_path.clone().into());
            m.insert("hasObjectModule".into(), o.has_object_module.into());
            m.insert("forms".into(), Value::Array(o.forms.iter().map(form_full).collect()));
            Value::Object(m)
        })
        .collect();
    let manifest = json!({
        "schemaVersion": WORKSPACE_SCHEMA_VERSION,
        "objects": manifest_objects,
        "totalObjects": objects.len(),
        "totalForms": total_forms,
        "totalWarnings": total_warnings,
    });

    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    files.insert(
        "owner.json".into(),
        format!("{{\"generator\": \"{GENERATOR}\", \"schemaVersion\": {WORKSPACE_SCHEMA_VERSION}}}\n").into_bytes(),
    );
    files.insert("manifest.json".into(), (dumps_with(&manifest, &PRETTY)? + "\n").into_bytes());
    let index_js = json_embed(&json!({"schemaVersion": WORKSPACE_SCHEMA_VERSION, "groups": groups, "totals": totals}))?;
    files.insert("index.js".into(), format!("window.__ONEFORM_INDEX__ = {index_js};\n").into_bytes());
    files.insert("index.html".into(), workspace_html().into_bytes());
    for (key, data) in &form_data {
        files.insert(
            format!("forms/{key}.data.js"),
            format!(
                "(window.__ONEFORM_FORMS__ = window.__ONEFORM_FORMS__ || {{}})[\"{key}\"] = {};\n",
                json_embed(data)?
            )
            .into_bytes(),
        );
    }
    Ok(files)
}

/// The static `index.html` of the workspace viewer.
pub fn workspace_html() -> String {
    let mut s = String::new();
    s.push_str(WS_HTML_A);
    s.push_str(theme_css());
    s.push_str(CODE_CSS);
    s.push_str(WS_CSS);
    s.push_str(WS_HTML_B);
    s.push_str(hljs_js());
    s.push_str(WS_HTML_C);
    s.push_str(render_js());
    s.push_str(WS_HTML_D);
    s.push_str(WS_JS);
    s.push_str(WS_HTML_E);
    s
}

/// Write the offline viewer `<target_root>/.oneform/` for `items` (one per form); returns its path.
///
/// Published atomically (temp dir + rename). An existing `.oneform` that lacks the `owner.json` marker of
/// `oneform` and is not an empty directory yields `OUTPUT_NOT_OWNED`.
pub fn build_workspace_preview(target_root: &Path, items: &[WorkspaceItem]) -> Result<PathBuf> {
    let root = target_root;
    let out = root.join(WORKSPACE_DIR);
    check_workspace_owner(&out)?;
    let files = workspace_files(root, items)?;

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        ^ std::process::id().rotate_left(16);
    let tmp = root.join(format!("{WORKSPACE_DIR}.tmp-{nonce:08x}"));
    let res = (|| -> Result<()> {
        fs::create_dir_all(tmp.join("forms")).map_err(|e| io_err(e, &tmp))?;
        for (rel, data) in &files {
            let p = tmp.join(rel);
            fs::write(&p, data).map_err(|e| io_err(e, &p))?;
        }
        if out.exists() {
            fs::remove_dir_all(&out).map_err(|e| io_err(e, &out))?;
        }
        fs::rename(&tmp, &out).map_err(|e| io_err(e, &out))
    })();
    if tmp.exists() {
        let _ = fs::remove_dir_all(&tmp);
    }
    res?;
    Ok(out)
}

const CSS: &str = r####"
html,body{height:100%}
body{height:100vh;display:flex;flex-direction:column}
#main{flex:1;display:flex;min-height:0}
#view{flex:1;overflow:auto;padding:12px}
#side{width:340px;border-left:1px solid #888;background:#F5F3EE;display:flex;flex-direction:column;min-height:0}
#side h3{margin:0;padding:4px 8px;background:#E9E6DC;border-bottom:1px solid #aaa;font-size:12px}
#q{margin:4px;padding:2px}
#tree{flex:1;overflow:auto;min-height:80px;border-bottom:1px solid #aaa}
#tree div{padding:1px 4px;cursor:pointer;white-space:nowrap}
#tree div.hl{background:#FFF3B8}
#tree div.sel{background:#F0CF55}
#info{flex:1;overflow:auto;padding:6px}
#info table{border-collapse:collapse;width:100%}
#info td{border-bottom:1px solid #ddd;padding:1px 3px;vertical-align:top}
#info td:first-child{color:#555;width:90px}
body:not(.insp) #side{display:none}
"####;
const CODE_CSS: &str = r####"
.code{background:#fff;border:1px solid #888;font:12px Consolas,monospace;padding:4px 0}
.l{white-space:pre;padding-right:8px}
.l i{display:inline-block;width:44px;padding-right:8px;text-align:right;color:#999;font-style:normal;user-select:none}
.l.proc{background:#fbf7de}
.l.flash{background:#ffd54a}
.kw{color:#00f}
"####;
const JS: &str = r####"
(function(){
var D=JSON.parse(document.getElementById('data').textContent),rendered={};
var Q=new URLSearchParams(location.search);
function $(i){return document.getElementById(i)}
if(Q.get('chrome')==='0')document.body.classList.add('chrome-0');
function tab(n){
 if(!rendered[n]){var map={module:['p-module',D.formModule,'ml-','1c'],object:['p-object',D.objectModule,'ol-','1c'],xml:['p-xml',D.xml,'xl-','xml']}[n];
  if(map&&$(map[0]))$(map[0]).innerHTML='<div class="code hljs'+(map[3]==='xml'?' t-xml':'')+'">'+OneFormRender.code(map[1]||'',map[2],null,map[3])+'</div>';rendered[n]=1}
 document.querySelectorAll('.tab').forEach(function(t){t.classList.toggle('on',t.dataset.tab===n)});
 document.querySelectorAll('.pane').forEach(function(p){p.classList.toggle('on',p.id==='p-'+n)});if(n==='form')V.applyZoom()}
function goProc(name){tab('module');var n=D.procs[name.toLowerCase()];var el=n&&$('ml-'+n);if(!el)return;
 el.scrollIntoView({block:'center'});el.classList.add('flash');setTimeout(function(){el.classList.remove('flash')},1500)}
var V=OneFormRender.create({host:$('p-form'),tree:$('tree'),query:$('q'),info:$('info'),langSel:$('lang'),modeSel:$('mode'),hidSel:$('hid'),zoomSel:$('zoom'),
 inspecting:function(){return document.body.classList.contains('insp')},onProc:goProc});
var S=V.state;
if(Q.get('mode')==='runtime'||Q.get('mode')==='designer'){S.mode=Q.get('mode');$('mode').value=S.mode}
if(Q.get('theme')==='xp'||Q.get('theme')==='v82')S.theme=Q.get('theme');
V.load(D.xml,{alias:D.alias,page:Q.get('page')});
document.querySelectorAll('.tab').forEach(function(t){t.onclick=function(){tab(t.dataset.tab)}});
$('insp').onchange=function(){document.body.classList.toggle('insp',$('insp').checked)};
$('fev').innerHTML=OneFormRender.evs(D.formEvents);
})();
"####;
const WS_CSS: &str = r####"
html,body{height:100%}
body{display:block;overflow:hidden}
#app{display:grid;grid-template-columns:300px minmax(0,1fr) 340px;height:100vh}
body:not(.insp) #app{grid-template-columns:300px minmax(0,1fr) 0}
body:not(.insp) #right{display:none}
#left,#right{background:#F5F3EE;display:flex;flex-direction:column;min-height:0;min-width:0}
#left{border-right:1px solid #888}
#right{border-left:1px solid #888}
#center{display:flex;flex-direction:column;min-height:0;min-width:0}
.ph{padding:4px 8px;background:#E9E6DC;border-bottom:1px solid #aaa;font-weight:bold}
#q,#cq{margin:4px;padding:2px}
#otree,#ctree{flex:1;overflow:auto;min-height:80px}
#ctree{flex:0 0 38%;border-bottom:1px solid #aaa}
#otree div,#ctree div{padding:1px 4px;cursor:pointer;white-space:nowrap}
#otree .g{font-weight:bold;background:#ECE9D8;border-bottom:1px solid #ddd}
#otree .o{padding-left:14px}
#otree .f{padding-left:32px}
#otree .f.on{background:#F0CF55}
#otree .o:hover,#otree .f:hover,#ctree div:hover{background:#FFF3B8}
.sy{color:#777}
.sz{color:#555}
.empty{padding:8px;color:#777}
#ctree div.hl{background:#FFF3B8}
#ctree div.sel{background:#F0CF55}
#hdr{background:#F5F3EE;border-bottom:1px solid #aaa;padding:4px 8px;display:flex;gap:10px;align-items:center;flex-wrap:wrap}
#crumb{font-weight:bold}
#tabs{background:#F5F3EE;padding:4px 8px 0;border-bottom:1px solid #aaa;display:flex;gap:4px}
#view{flex:1;overflow:auto;padding:12px;background:#d8d5cc}
#log{margin-top:10px;max-height:140px;overflow:auto;background:#fff;border:1px solid #888;font:12px Consolas,monospace;padding:2px 6px}
#log:empty::before{content:"Журнал событий";color:#999}
#info{flex:1;overflow:auto;padding:6px}
#info table{border-collapse:collapse;width:100%}
#info td{border-bottom:1px solid #ddd;padding:1px 3px;vertical-align:top}
#info td:first-child{color:#555;width:90px}
#open{margin-left:auto}
"####;
const WS_JS: &str = r####"
(function(){
var IDX=window.__ONEFORM_INDEX__||{groups:[]};
var openState={},curKey=null,procs={},D=null,rendered={};
var PROC=/^\s*(Процедура|Функция|Procedure|Function)\s+([\p{L}\p{N}_]+)/iu;
var KW=/^(\s*)(Процедура|Функция|КонецПроцедуры|КонецФункции|Procedure|Function|EndProcedure|EndFunction)(?![\p{L}\p{N}_])/iu;
var MAXLINES=20000;
function $(i){return document.getElementById(i)}
var esc=OneFormRender.esc;
function tab(n){
 if(!rendered[n]&&D){var map={module:['p-module',D.formModule,'ml-','1c'],object:['p-object',D.objectModule,'ol-','1c'],xml:['p-xml',D.xml,'xl-','xml']}[n];
  if(map)$(map[0]).innerHTML='<div class="code hljs'+(map[3]==='xml'?' t-xml':'')+'">'+OneFormRender.code(map[1]||'',map[2],null,map[3])+'</div>';rendered[n]=1}
 document.querySelectorAll('.tab').forEach(function(t){t.classList.toggle('on',t.dataset.tab===n)});
 document.querySelectorAll('.pane').forEach(function(p){p.classList.toggle('on',p.id==='p-'+n)});if(n==='form')V.applyZoom()}
document.querySelectorAll('.tab').forEach(function(t){t.onclick=function(){tab(t.dataset.tab)}});
$('insp').onchange=function(){document.body.classList.toggle('insp',$('insp').checked)};
function log(m){var d=document.createElement('div');
 d.textContent=new Date().toLocaleTimeString()+'  '+m;$('log').insertBefore(d,$('log').firstChild)}
function fmatch(f,q){return (f.name+' '+f.alias+' '+f.synonym).toLowerCase().indexOf(q)>=0}
function omatch(o,q){return (o.name+' '+o.synonym).toLowerCase().indexOf(q)>=0}
function renderTree(){
 var q=$('q').value.trim().toLowerCase(),h='';
 IDX.groups.forEach(function(g,gi){
  var oh='',cnt=0;
  g.objects.forEach(function(o,oi){
   var om=!q||omatch(o,q),fs=[];
   o.forms.forEach(function(f,fi){if(om||fmatch(f,q))fs.push([f,fi])});
   if(!fs.length&&!om)return;
   cnt++;
   var oo=q||openState['o'+gi+'_'+oi];
   oh+='<div class="o" data-g="'+gi+'" data-o="'+oi+'">'+(oo?'▾ ':'▸ ')+esc(o.name)+(o.synonym?' <span class="sy">'+esc(o.synonym)+'</span>':'')+'</div>';
   if(oo)fs.forEach(function(p){oh+='<div class="f'+(p[0].key===curKey?' on':'')+'" data-g="'+gi+'" data-o="'+oi+'" data-f="'+p[1]+'">'+esc(p[0].name)+(p[0].synonym?' <span class="sy">'+esc(p[0].synonym)+'</span>':'')+'</div>'})});
  if(!cnt)return;
  var go=q||openState['g'+gi];
  h+='<div class="g" data-g="'+gi+'">'+(go?'▾ ':'▸ ')+esc(g.title)+' ('+cnt+')</div>'+(go?oh:'')});
 $('otree').innerHTML=h||'<div class="empty">Ничего не найдено</div>'}
$('q').oninput=renderTree;
$('otree').onclick=function(ev){
 var e=ev.target.closest('[data-g]');if(!e)return;
 var gi=+e.dataset.g;
 if(e.classList.contains('g')){openState['g'+gi]=!openState['g'+gi];renderTree();return}
 var oi=+e.dataset.o;
 if(e.classList.contains('o')){openState['o'+gi+'_'+oi]=!openState['o'+gi+'_'+oi];renderTree();return}
 openForm(gi,oi,+e.dataset.f)};
function loadData(f,cb){
 var store=window.__ONEFORM_FORMS__=window.__ONEFORM_FORMS__||{};
 if(store[f.key])return cb(store[f.key]);
 var s=document.createElement('script');s.src=f.dataScript;
 s.onload=function(){cb(store[f.key]||null)};s.onerror=function(){cb(null)};
 document.head.appendChild(s)}
function goProc(name){tab('module');var n=procs[name.toLowerCase()],el=n&&$('ml-'+n);if(!el)return;
 el.scrollIntoView({block:'center'});el.classList.add('flash');setTimeout(function(){el.classList.remove('flash')},1500)}
var V=OneFormRender.create({host:$('p-form'),tree:$('ctree'),query:$('cq'),info:$('info'),langSel:$('lang'),modeSel:$('mode'),hidSel:$('hid'),zoomSel:$('zoom'),
 alwaysSelect:true,inspecting:function(){return document.body.classList.contains('insp')},onProc:goProc,
 onClickCanvas:function(c){
  if(c.events.length)c.events.forEach(function(v){log(v.name+' → '+c.name+(v.handler?': Процедура '+v.handler+'()':''))});
  else log('Выбран элемент '+c.name+' ('+c.tag+')')},
 onSelect:function(){
  if(D&&D.formEvents&&D.formEvents.length)$('info').insertAdjacentHTML('beforeend','<b>События формы</b><div>'+OneFormRender.evs(D.formEvents)+'</div>')}});
function openForm(gi,oi,fi){
 var g=IDX.groups[gi],o=g.objects[oi],f=o.forms[fi];
 curKey=f.key;openState['g'+gi]=true;openState['o'+gi+'_'+oi]=true;renderTree();
 $('crumb').textContent=g.title+' / '+o.name+' / '+f.name;
 $('size').textContent='('+f.width+' × '+f.height+')';
 loadData(f,function(d){
  if(curKey!==f.key)return;
  D=d||{xml:'',formModule:'',objectModule:'',objectModulePath:''};rendered={};
  var op=$('open');op.style.display=f.previewPath?'':'none';
  if(f.previewPath)op.href=encodeURI('../'+f.previewPath);
  $('tab-object').style.display=D.objectModule?'':'none';
  procs={};OneFormRender.code(D.formModule||'','',procs,'1c');
  var m=V.load(D.xml,{alias:f.name});
  D.formEvents=m.error?[]:m.events;
  $('log').innerHTML='';tab('form')})}
var t=IDX.totals||{};
$('stat').textContent=(t.objects||0)+' объектов, '+(t.forms||0)+' форм';
renderTree();
outer:for(var gi=0;gi<IDX.groups.length;gi++){var g=IDX.groups[gi];
 for(var oi=0;oi<g.objects.length;oi++){if(g.objects[oi].forms.length){openForm(gi,oi,0);break outer}}}
})();
"####;
const WS_HTML_A: &str = r####"<!DOCTYPE html>
<html lang="ru"><head><meta charset="utf-8"><title>oneform — просмотр конфигурации</title>
<style>"####;
const WS_HTML_B: &str = r####"</style></head>
<body class="insp">
<div id="app">
<div id="left"><div class="ph">Дерево объектов конфигурации</div><input id="q" placeholder="Поиск объектов и форм"><div id="otree"></div><div class="ph" id="stat"></div></div>
<div id="center">
<div id="hdr"><span id="crumb"></span><span class="sz" id="size"></span>
<select id="lang"></select>
<select id="mode"><option value="designer">Конфигуратор</option><option value="runtime">Предприятие</option></select>
<label><input type="checkbox" id="hid"> Показывать скрытые</label>
<select id="zoom"><option value="100">100%</option><option value="125">125%</option><option value="150">150%</option><option value="fit">Вписать</option></select>
<label><input type="checkbox" id="insp" checked> Режим инспектора</label>
<a id="open" target="_blank" rel="noopener" style="display:none">Открыть FormPreview.html ↗</a></div>
<div id="tabs"><button class="tab on" data-tab="form">Форма (1:1)</button><button class="tab" data-tab="module">Модуль формы</button><button class="tab" id="tab-object" data-tab="object" style="display:none">Модуль объекта</button><button class="tab" data-tab="xml">Form.xml</button></div>
<div id="view"><div class="pane on" id="p-form"></div><div class="pane" id="p-module"></div><div class="pane" id="p-object"></div><div class="pane" id="p-xml"></div><div id="log"></div></div>
</div>
<div id="right"><div class="ph">Инспектор элементов</div><input id="cq" placeholder="Поиск элементов"><div id="ctree"></div><div id="info"></div></div>
</div>
<script src="index.js"></script>
<script>"####;
const WS_HTML_C: &str = r####"</script>
<script>"####;
const WS_HTML_D: &str = r####"</script>
<script>"####;
const WS_HTML_E: &str = r####"</script>
</body></html>
"####;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_matches_python() {
        assert_eq!(html_escape(r#"a&b<c>"d'e"#), "a&amp;b&lt;c&gt;&quot;d&#x27;e");
        assert_eq!(html_escape("Привет"), "Привет");
    }

    #[test]
    fn json_embedding_is_script_safe() {
        let s = json_embed(&json!({"b": "</script><!--&\u{2028}\u{2029}", "a": 1})).unwrap();
        assert_eq!(s, r#"{"a":1,"b":"\u003c/script\u003e\u003c!--\u0026\u2028\u2029"}"#);
        assert!(!s.contains('<') && !s.contains('>') && !s.contains('&'));
    }

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn python_int_rules() {
        assert_eq!(py_int(" 12 "), 12);
        assert_eq!(py_int("-3"), -3);
        assert_eq!(py_int("1_0"), 10);
        assert_eq!(py_int("1__0"), 0);
        assert_eq!(py_int("x"), 0);
        assert_eq!(py_int(""), 0);
    }

    #[test]
    fn ordinary_form_detection() {
        assert!(is_ordinary_form_xml(br#"<Form ordinaryFormVersion="2.1"/>"#));
        assert!(is_ordinary_form_xml(br#"<Form ordinaryFormVersion="2.2"/>"#));
        assert!(!is_ordinary_form_xml(br#"<Form ordinaryFormVersion="2.0"/>"#));
        assert!(!is_ordinary_form_xml(b"<Form"));
        assert!(!is_ordinary_form_xml(br#"<Other ordinaryFormVersion="2.1"/>"#));
    }

    #[test]
    fn render_rejects_malformed_xml() {
        let e = render_form_preview_html(b"<Form", b"", b"", "x").unwrap_err();
        assert_eq!(e.code, FORM_PREVIEW_FAILED);
    }
}
