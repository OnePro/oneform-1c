//! Optional ordinary-form XML (`Form.xml`) in the `onec-ordinary-forms` (oof) style
//! (port of `adapters/ordinary_oof.py`).
//!
//! Built-in decoder of the `{27,{18,...}}` brace-list form stream, optionally preceded by an external
//! `oof dump` run. The result is a partial reference projection (`reconstructionComplete="false"`);
//! it is produced on unpack only and never packed back. Any failure is returned as a warning
//! diagnostic so that the caller can fall back to the basic reference without `Form.xml`.
//!
//! The decoded model is a [`serde_json::Value`] with exactly the shape (and key order) of the Python dict,
//! so that `FormDefinition.json` and the XML writer stay byte-identical to the Reference.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use crate::brace::{parse_metadata, Item, Node, BARE, BASE64};
use crate::container::serialize_container;
use crate::errors::{
    diagnostic, FORM_CONTROL_UNKNOWN, FORM_OOF_EXTERNAL_FAILED, FORM_PROJECTION_INCOMPLETE, FORM_PROPERTY_UNREADABLE,
    FORM_XML_EXTRACTION_FAILED,
};
use crate::metadata::FormInfo;
pub use crate::oof_props::{
    at, at_n, atom, child, control_caption, decode_color, decode_column_props, decode_font, decode_pattern,
    decode_props, int, int_or_first, is_bare, is_guid, localized, localized_value, node, node_opt, num,
    resolve_column_step, resolve_data_path, str_val, OofError, EVENT_GUID,
};
use crate::oof_props::{index_error, py_get, py_slice};

pub const FORM_XML_NAME: &str = "Form.xml";
pub const ORDINARY_FORM_VERSION: &str = "2.1";
pub const EXTERNAL_TIMEOUT_SECONDS: u64 = 60;

/// `lowercase metadata uuid -> name` (see `formbin.read_meta_lookup`).
pub type MetaLookup = HashMap<String, String>;

pub const CONTROL_GUIDS: [(&str, &str); 27] = [
    ("09ccdc77-ea1a-4a6d-ab1c-3435eada2433", "Panel"),
    ("e69bf21d-97b2-4f37-86db-675aea9ec2cb", "CommandBar"),
    ("6ff79819-710e-4145-97cd-1618da79e3e2", "Button"),
    ("151ef23e-6bb2-4681-83d0-35bc2217230c", "PictureDecoration"),
    ("35af3d93-d7c7-4a2e-a8eb-bac87a1a3f26", "CheckBox"),
    ("64483e7f-3833-48e2-8c75-2c31aac49f6e", "ChoiceField"),
    ("782e569a-79a7-4a4f-a936-b48d013936ec", "RadioButton"),
    ("381ed624-9217-4e63-85db-c4c3cb87daae", "InputField"),
    ("90db814a-c75f-4b54-bc96-df62e554d67d", "UsualGroup"),
    ("36e52348-5d60-4770-8e89-a16ed50a2006", "Splitter"),
    ("a8b97779-1a4b-4059-b09c-807f86d2a461", "Chart"),
    ("a26da99e-184a-4823-b0d6-62816d38dc4e", "PivotChart"),
    ("e5fdc112-5c84-4a16-9728-72b85692b6e2", "GanttChart"),
    ("984981b1-622d-4ebc-94f7-885f0cdfb59a", "Dendrogram"),
    ("d92a805c-98ae-4750-9158-d9ce7cec2f20", "HTMLDocumentField"),
    ("19f8b798-314e-4b4e-8121-905b2a7a03f5", "ListBox"),
    ("b1db1f86-abbb-4cf0-8852-fe6ae21650c2", "ProgressBar"),
    ("6c06cd5d-8481-4b6f-a90a-7a97a8bb8bef", "TrackBar"),
    ("e3c063d8-ef92-41be-9c89-b70290b5368b", "CalendarField"),
    ("14c4a229-bfc3-42fe-9ce1-2da049fd0109", "TextDocumentField"),
    ("ad37194e-555e-4305-b718-5dca84baf145", "GeographicalSchemaField"),
    ("42248403-7748-49da-b782-e4438fd7bff3", "GraphicalSchemaField"),
    ("ea83fe3a-ac3c-4cce-8045-3dddf35b28b1", "Table"),
    ("236a17b3-7f44-46d9-a907-75f9cdc61ab5", "SpreadsheetDocumentField"),
    ("0fc7e20d-f241-460c-bdf4-5ad88e5474a5", "LabelDecoration"),
    ("621e95f1-064f-11d4-9400-008048da11f9", "ActiveXControl"),
    ("48537cf6-4901-4b8f-9869-e4b1564af79c", "PDFDocumentField"),
];

pub const EXTENSION_GUID: &str = "59d6c227-97d3-46f6-84a0-584c5a2807e1";
pub const COLUMN_GUID: &str = "c2cf1953-2796-4fe2-b78c-ff84140b124e";
pub const LIST_COLUMN_GUID: &str = "48a6ebc3-fcc8-4f8f-b399-459fb32aa46b";
pub const VT_COLUMN_GUID: &str = "737535a4-21e6-4971-8513-3e3173a9fedd";
pub const VL_COLUMN_GUID: &str = "fbacf851-e6ff-4107-9e08-4302dda1d14d";

fn is_column_guid(item: &Item) -> bool {
    matches!(
        item,
        Item::Atom(a)
            if a.kind == BARE
                && matches!(
                    a.value.as_str(),
                    COLUMN_GUID | LIST_COLUMN_GUID | VT_COLUMN_GUID | VL_COLUMN_GUID
                )
    )
}

pub const FORM_EVENTS: [(i64, &str); 13] = [
    (70000, "BeforeOpen"),
    (70001, "OnOpen"),
    (70002, "BeforeClose"),
    (70003, "OnClose"),
    (70004, "ChoiceProcessing"),
    (70005, "ActivationProcessing"),
    (70006, "NewObjectWriteProcessing"),
    (70007, "NotificationProcessing"),
    (70008, "OnReopen"),
    (70009, "RefreshDisplay"),
    (70010, "ExternalEvent"),
    (70011, "ExternalAddInDisconnectOnError"),
    (70012, "FillCheckProcessing"),
];
pub const EXT_DP_EVENTS: [(i64, &str); 2] = [
    (80000, "BeforeSaveValues"),
    (80001, "AfterRestoreValues"),
];
pub const EXT_DOC_EVENTS: [(i64, &str); 4] = [
    (80000, "BeforeWrite"),
    (80001, "OnWrite"),
    (80002, "AfterWrite"),
    (80003, "OnRead"),
];
pub const CLICK_TYPES: [&str; 2] = ["Button", "LabelDecoration"];
pub const CHANGE_TYPES: [&str; 5] = ["InputField", "CheckBox", "ChoiceField", "RadioButton", "TrackBar"];
pub const PICTURE_EVENTS: [(i64, &str); 5] = [
    (0, "Click"),
    (3, "DragStart"),
    (4, "DragCheck"),
    (5, "DragEnd"),
    (6, "Drag"),
];
pub const CALENDAR_EVENTS: [(i64, &str); 8] = [
    (0, "OnActivateDate"),
    (1, "Selection"),
    (2, "OnPeriodOutput"),
    (3, "DragStart"),
    (4, "DragCheck"),
    (5, "DragEnd"),
    (6, "Drag"),
    (2147483647, "OnChange"),
];
pub const CHART_EVENTS: [(i64, &str); 4] = [
    (0, "Selection"),
    (1, "DetailProcessing"),
    (2, "OnActivate"),
    (3, "OnValueEdit"),
];
pub const GANTT_EVENTS: [(i64, &str); 7] = [
    (0, "Selection"),
    (1, "DetailProcessing"),
    (2, "OnIntervalEditEnd"),
    (3, "OnValueActivate"),
    (4, "OnIntervalActivate"),
    (7, "BeforeCollapse"),
    (8, "BeforeExpand"),
];
pub const FIELD_EVENTS: [(i64, &str); 10] = [
    (0, "OnChange"),
    (2147483647, "OnChange"),
    (1, "StartListChoice"),
    (2, "StartChoice"),
    (3, "Clearing"),
    (4, "Tuning"),
    (5, "Opening"),
    (7, "ChoiceProcessing"),
    (10, "TextEditEnd"),
    (11, "AutoComplete"),
];
pub const TABLE_EVENTS: [(i64, &str); 21] = [
    (34, "Selection"),
    (35, "OnActivateRow"),
    (36, "OnActivateColumn"),
    (37, "OnActivateCell"),
    (40, "BeforeAddRow"),
    (41, "BeforeRowChange"),
    (42, "BeforeDeleteRow"),
    (43, "OnStartEdit"),
    (44, "BeforeEditEnd"),
    (45, "OnCheckChange"),
    (47, "OnRowOutput"),
    (48, "ValueChoice"),
    (49, "OnEditEnd"),
    (50, "NewWriteProcessing"),
    (51, "AfterDeleteRow"),
    (52, "ChoiceProcessing"),
    (53, "OnGetData"),
    (900, "DragStart"),
    (901, "DragCheck"),
    (902, "DragEnd"),
    (903, "Drag"),
];
pub const LIST_BOX_EVENTS: [(i64, &str); 4] = [
    (0, "OnChange"),
    (17, "OnCheckChange"),
    (34, "Selection"),
    (37, "OnActivateRow"),
];
pub const SPREADSHEET_EVENTS: [(i64, &str); 5] = [
    (0, "Selection"),
    (1, "DetailProcessing"),
    (2, "OnActivateArea"),
    (6, "Drag"),
    (7, "OnChangeAreaContent"),
];

/// Control type name of a control GUID (`None` for unknown GUIDs).
pub fn control_type(guid: &str) -> Option<&'static str> {
    CONTROL_GUIDS.iter().find(|(g, _)| *g == guid).map(|(_, n)| *n)
}

/// Result of [`build_oof`]; `xml` is `None` when the form could not be decoded.
#[derive(Debug, Clone, Default)]
pub struct OofResult {
    pub xml: Option<Vec<u8>>,
    pub attributes: Vec<Value>,
    pub events: Vec<Value>,
    pub controls: Vec<Value>,
    /// `"builtin"` | `"external"`
    pub engine: String,
    pub diagnostics: Vec<Value>,
}

type Res<T> = Result<T, OofError>;

// ---------------------------------------------------------------- node helpers

/// `{tag,<event guid>,{3,"Handler",...}}` -> (tag, handler).
fn event(n: &Node) -> Res<Option<(i64, String)>> {
    if n.len() >= 3 && is_bare(&n.children[1], EVENT_GUID) {
        if let Item::Node(body) = &n.children[2] {
            if body.len() >= 2 && is_bare(&body.children[0], "3") {
                let tag = int(&n.children[0], "event tag")?;
                return Ok(Some((tag, str_val(&body.children[1], "event handler")?.to_string())));
            }
        }
    }
    Ok(None)
}

fn collect_events(n: &Node) -> Res<Vec<(i64, String)>> {
    if !n.is_empty() && is_column_guid(&n.children[0]) {
        return Ok(Vec::new());
    }
    if let Some(ev) = event(n)? {
        return Ok(vec![ev]);
    }
    let mut found = Vec::new();
    for c in &n.children {
        if let Item::Node(c) = c {
            found.extend(collect_events(c)?);
        }
    }
    Ok(found)
}

fn event_name(type_name: Option<&str>, tag: i64) -> &'static str {
    let Some(type_name) = type_name else {
        return FORM_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    };
    if tag == 0 && CLICK_TYPES.contains(&type_name) {
        return "Click";
    }
    if tag == 0 && type_name == "Panel" {
        return "OnCurrentPageChange";
    }
    if type_name == "PictureDecoration" {
        return PICTURE_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if type_name == "CalendarField" {
        return CALENDAR_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if CHANGE_TYPES.contains(&type_name) {
        return FIELD_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if type_name == "Table" {
        return TABLE_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if type_name == "ListBox" {
        return LIST_BOX_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if type_name == "SpreadsheetDocumentField" {
        return SPREADSHEET_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if matches!(type_name, "Chart" | "Dendrogram") {
        return CHART_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if type_name == "GanttChart" {
        return GANTT_EVENTS.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
    }
    if type_name == "GeographicalSchemaField" && tag == 2 {
        return "OnActivate";
    }
    if type_name == "GraphicalSchemaField" {
        return match tag {
            0 => "Selection",
            1 => "OnActivate",
            _ => "Event",
        };
    }
    if type_name == "HTMLDocumentField" {
        return match tag {
            0 => "DocumentComplete",
            2 => "OnClick",
            _ => "Event",
        };
    }
    "Event"
}

fn events_of(n: &Node, type_name: Option<&str>) -> Res<Vec<Value>> {
    Ok(collect_events(n)?
        .into_iter()
        .map(|(tag, handler)| json!({"name": event_name(type_name, tag), "id": tag, "handler": handler}))
        .collect())
}

// ---------------------------------------------------------------- decoder

/// `_table`: a counted list `{N,item1,...,itemN}`.
pub fn table<'a>(n: Option<&'a Item>, what: &str) -> Res<&'a [Item]> {
    let n = node_opt(n, what, 1)?;
    let count = int(&n.children[0], what)?;
    let items = py_slice(&n.children, 1, 1 + count);
    if items.len() as i64 != count {
        return Err(OofError(format!("{what}: count does not match the items")));
    }
    Ok(items)
}

fn column_editor_events(props: &Node, editor_type: Option<&str>) -> Vec<Value> {
    let Some(editor_type) = editor_type else { return Vec::new() };
    let Some(Item::Node(bnode)) = props.children.get(39) else { return Vec::new() };
    let Some(Item::Atom(b0)) = bnode.children.first() else { return Vec::new() };
    if b0.kind != BASE64 {
        return Vec::new();
    }
    let Ok(raw) = b0.decode_base64() else { return Vec::new() };
    if raw.len() <= 18 {
        return Vec::new();
    }
    let mut inflated = Vec::new();
    if flate2::read::DeflateDecoder::new(&raw[18..]).read_to_end(&mut inflated).is_err() {
        return Vec::new();
    }
    if inflated.len() <= 8 {
        return Vec::new();
    }
    let Ok(tree) = parse_metadata(&inflated[8..]) else { return Vec::new() };
    events_of(&tree, Some(editor_type)).unwrap_or_default()
}

fn column(n: &Node, meta_lookup: Option<&MetaLookup>) -> Res<Value> {
    let col_guid = atom(&n.children[0]);
    let wrapper = node_opt(n.children.get(1), "column", 2)?;
    let (x, props) = if col_guid == VL_COLUMN_GUID {
        (None, node(&wrapper.children[1], "column", 39)?)
    } else {
        let x = node(&wrapper.children[1], "column", 2)?;
        let props = node(&x.children[1], "column", 39)?;
        (Some(x), props)
    };
    let mut editor_node = &props.children[38];
    if let Item::Node(e) = editor_node {
        if e.len() >= 2 {
            editor_node = &e.children[1];
        }
    }
    let editor_type = match editor_node {
        Item::Atom(a) => control_type(&a.value),
        Item::Node(_) => None,
    };
    let mut field_name: Option<String> = None;
    if matches!(col_guid, COLUMN_GUID | LIST_COLUMN_GUID) {
        if let Some(x) = x {
            if x.len() > 2 {
                let is_tab = col_guid == COLUMN_GUID;
                field_name = resolve_column_step(x.children.get(2), is_tab, meta_lookup);
                if field_name.is_none() && x.len() > 3 {
                    field_name = resolve_column_step(x.children.get(3), is_tab, meta_lookup);
                }
            }
        }
    } else if col_guid == VT_COLUMN_GUID {
        let w2 = wrapper.children.get(2).map(atom).unwrap_or("");
        let w3 = wrapper.children.get(3).map(atom).unwrap_or("");
        if !w2.is_empty() {
            field_name = Some(w2.to_string());
        } else if !w3.is_empty() {
            field_name = Some(w3.to_string());
        }
    }
    let mut out = Map::new();
    out.insert("name".into(), str_val(&props.children[30], "column name")?.into());
    out.insert("header".into(), localized_value(&localized(&props.children[1]).unwrap_or_default()));
    out.insert("editor".into(), editor_type.map(Value::from).unwrap_or(Value::Null));
    out.extend(decode_column_props(props)?);
    if let Some(f) = field_name {
        out.insert("_field".into(), f.into());
    }
    let ev = column_editor_events(props, editor_type);
    if !ev.is_empty() {
        out.insert("events".into(), Value::Array(ev));
    }
    Ok(Value::Object(out))
}

fn columns(info: &Node, meta_lookup: Option<&MetaLookup>) -> Res<Vec<Value>> {
    fn walk(n: &Node, meta_lookup: Option<&MetaLookup>, cols: &mut Vec<Value>) -> Res<()> {
        if !n.is_empty() && is_column_guid(&n.children[0]) {
            cols.push(column(n, meta_lookup)?);
            return Ok(());
        }
        for c in &n.children {
            if let Item::Node(c) = c {
                walk(c, meta_lookup, cols)?;
            }
        }
        Ok(())
    }
    let mut cols = Vec::new();
    walk(info, meta_lookup, &mut cols)?;
    Ok(cols)
}

/// (page index, z-order, tab order) of a control; `(0, 0, 0)` when the geometry tail is unusual.
///
/// The tail follows the six counted incoming lists: `page, z, tab, flagA, flagB`.
pub fn geom_page(geom: &Node) -> (i64, i64, i64) {
    fn inner(geom: &Node) -> Res<(i64, i64, i64)> {
        let mut idx: i64 = 12;
        for _ in 0..6 {
            let it = py_get(&geom.children, idx).ok_or_else(index_error)?;
            idx += 1 + int(it, "incoming list size")?;
        }
        let mut v = [0i64; 3];
        for (i, slot) in v.iter_mut().enumerate() {
            let it = py_get(&geom.children, idx + i as i64).ok_or_else(index_error)?;
            *slot = int(it, "geometry tail")?;
        }
        Ok((v[0], v[1], v[2]))
    }
    inner(geom).unwrap_or((0, 0, 0))
}

/// (showTabs, pages) of a `Panel` control; `(0, [])` when the panel has an unexpected shape.
pub fn panel_pages(info: &Node, width: i64, height: i64) -> (i64, Vec<Value>) {
    panel_pages_strict(info, width, height).unwrap_or((0, Vec::new()))
}

fn panel_pages_strict(info: &Node, width: i64, height: i64) -> Res<(i64, Vec<Value>)> {
    let p = node(child(info, 1)?, "panel", 4)?;
    let get = |i: i64| py_get(&p.children, i).ok_or_else(index_error);
    let mut idx: i64 = 2;
    for _ in 0..6 {
        idx += 1 + int(get(idx)?, "panel list size")?;
    }
    let base = idx;
    let show_tabs = int(get(base + 1)?, "showTabs")?;
    let page_table = node(get(base + 3)?, "panel pages", 2)?;
    let page_count = int(&page_table.children[1], "page count")?;
    // page index -> edge (1..=4) -> (coord, margin)
    let mut edges: HashMap<i64, HashMap<i64, (i64, i64)>> = HashMap::new();
    let row_count = int(get(base + 7)?, "row count")?;
    for row in py_slice(&p.children, base + 8, base + 8 + row_count) {
        let row = node(row, "page boundary", 8)?;
        let edge = int(&row.children[4], "edge")?;
        if (1..=4).contains(&edge) {
            let v = (int(&row.children[1], "coord")?, int(&row.children[7], "margin")?);
            edges.entry(int(&row.children[5], "page idx")?).or_default().insert(edge, v);
        }
    }
    let mut pages = Vec::new();
    for (i, rec) in py_slice(&page_table.children, 2, 2 + page_count).iter().enumerate() {
        let rec = node(rec, "page", 7)?;
        let e = edges.get(&(i as i64));
        let (left, top, client_w, client_h);
        if let Some((e3, e4)) = e.and_then(|e| Some((e.get(&3)?, e.get(&4)?))) {
            client_w = (e3.0 + e3.1).max(1).min(width);
            client_h = (e4.0 + e4.1).max(1).min(height);
            let (dx, dy) = ((width - client_w).max(0), (height - client_h).max(0));
            if show_tabs == 0 {
                left = dx / 2;
                top = dy / 2;
            } else if dy >= dx {
                let b = dx / 2;
                left = b;
                top = dy - b;
            } else {
                let b = dy / 2;
                left = dx - b;
                top = b;
            }
        } else if show_tabs != 0 {
            left = 1;
            top = 19;
            client_w = (width - 2).max(1);
            client_h = (height - 20).max(1);
        } else {
            left = 0;
            top = 0;
            client_w = width;
            client_h = height;
        }
        let name = str_val(&rec.children[6], "page name")?;
        let title = localized(&rec.children[1]).unwrap_or_default();
        pages.push(json!({
            "name": name,
            "title": localized_value(&title),
            "visible": int(&rec.children[4], "page visible")? != 0,
            "enabled": int(&rec.children[5], "page enabled")? != 0,
            "left": left,
            "top": top,
            "width": client_w,
            "height": client_h,
        }));
    }
    Ok((show_tabs, pages))
}

/// `(node, type name, info, geometry, meta, children node)` of a control record.
pub struct ControlParts<'a> {
    pub node: &'a Node,
    pub type_name: Option<&'static str>,
    pub info: &'a Node,
    pub geom: &'a Node,
    pub meta: &'a Node,
    pub children: &'a Item,
}

pub fn control_parts(n: &Item) -> Res<ControlParts<'_>> {
    let n = node(n, "control", 6)?;
    let guid = &n.children[0];
    if !is_guid(guid) {
        return Err(OofError::new("control: guid expected"));
    }
    let len = n.len();
    let geom = node(&n.children[len - 3], "control geometry", 6)?;
    let meta = node(&n.children[len - 2], "control meta", 2)?;
    let (c2, c3) = (&n.children[2], &n.children[3]);
    let c3_ok = len >= 7 && matches!(c3, Item::Node(x) if x.len() >= 2);
    let c2_short = !matches!(c2, Item::Node(x) if x.len() >= 2);
    let info_item = if c3_ok && c2_short { c3 } else { c2 };
    let info = node(info_item, "control info", 1)?;
    Ok(ControlParts { node: n, type_name: control_type(atom(guid)), info, geom, meta, children: &n.children[len - 1] })
}

fn diag_component(form_uuid: &str) -> String {
    format!("Forms/{form_uuid}")
}

fn control(n: &Item, warnings: &mut Vec<Value>, form_uuid: &str, meta_lookup: Option<&MetaLookup>) -> Res<Value> {
    let ControlParts { node: n, type_name, info, geom, meta, children: children_node } = control_parts(n)?;
    let guid = atom(&n.children[0]);
    let name = str_val(&meta.children[1], "control name")?;
    if type_name.is_none() {
        warnings.push(diagnostic(
            FORM_CONTROL_UNKNOWN,
            "warning",
            &diag_component(form_uuid),
            &[("control", name.into()), ("guid", guid.into())],
        ));
    }
    let left = int(&geom.children[1], "left")?;
    let top = int(&geom.children[2], "top")?;
    let caption = match control_caption(type_name, info) {
        Ok(Some(c)) => c,
        _ => {
            let mut found = Vec::new();
            if let Some(Item::Node(props)) = info.children.get(1) {
                for c in &props.children {
                    if let Some(loc) = localized(c) {
                        if !loc.is_empty() {
                            found = loc;
                            break;
                        }
                    }
                }
            }
            found
        }
    };
    let width = int(&geom.children[3], "right")? - left;
    let height = int(&geom.children[4], "bottom")? - top;
    let (page, z_order, tab_order) = geom_page(geom);

    let id = int(&n.children[1], "control id")?;
    let visible = int(&geom.children[5], "visible")? != 0;
    let mut events = events_of(info, type_name)?;
    if n.len() >= 7 {
        if let (Item::Node(c2), Item::Node(c3)) = (&n.children[2], &n.children[3]) {
            if std::ptr::eq(info, c2) {
                events.extend(events_of(c3, type_name)?);
            }
        }
    }
    let cols = if type_name == Some("Table") { columns(info, meta_lookup)? } else { Vec::new() };
    let props = match type_name {
        Some(t) => {
            let mut unreadable = |prop: &str| {
                warnings.push(diagnostic(
                    FORM_PROPERTY_UNREADABLE,
                    "warning",
                    &diag_component(form_uuid),
                    &[("control", name.into()), ("property", prop.into()), ("type", t.into())],
                ));
            };
            decode_props(t, info, &mut unreadable)
        }
        None => Map::new(),
    };
    let mut children = Vec::new();
    for c in table(Some(children_node), "control children")? {
        children.push(control(c, warnings, form_uuid, meta_lookup)?);
    }
    let mut ctrl = Map::new();
    ctrl.insert("id".into(), id.into());
    ctrl.insert("name".into(), name.into());
    ctrl.insert("type".into(), type_name.unwrap_or("UnknownControl").into());
    ctrl.insert("guid".into(), guid.into());
    ctrl.insert("caption".into(), localized_value(&caption));
    ctrl.insert("left".into(), left.into());
    ctrl.insert("top".into(), top.into());
    ctrl.insert("width".into(), width.into());
    ctrl.insert("height".into(), height.into());
    ctrl.insert("visible".into(), visible.into());
    ctrl.insert("page".into(), page.into());
    ctrl.insert("ordinal".into(), z_order.into());
    ctrl.insert("tabOrder".into(), tab_order.into());
    ctrl.insert("events".into(), Value::Array(events));
    ctrl.insert("columns".into(), Value::Array(cols));
    ctrl.insert("props".into(), Value::Object(props));
    ctrl.insert("children".into(), Value::Array(children));
    if type_name == Some("Panel") {
        let (show_tabs, pages) = panel_pages(info, width, height);
        ctrl.insert("showTabs".into(), show_tabs.into());
        ctrl.insert("pages".into(), Value::Array(pages));
    }
    Ok(Value::Object(ctrl))
}

type Attributes = (Option<i64>, Vec<Value>, HashMap<i64, Value>);

fn attributes(section: &Node, meta_lookup: Option<&MetaLookup>) -> Res<Attributes> {
    let main_id = int_or_first(child(section, 0)?, "main attribute id")?;
    let mut attrs: Vec<Value> = Vec::new();
    for rec in table(Some(child(section, 2)?), "attributes")? {
        let rec = node(rec, "attribute", 5)?;
        let attr_id = int_or_first(&rec.children[0], "attribute id")?;
        let name = str_val(&rec.children[4], "attribute name")?;
        let main = (main_id > 0 && attr_id == main_id) || int(&rec.children[1], "main flag")? != 0;
        let stored = int(&rec.children[2], "stored flag")? != 0;
        let mut attr = Map::new();
        attr.insert("id".into(), attr_id.into());
        attr.insert("name".into(), name.into());
        attr.insert("main".into(), main.into());
        attr.insert("stored".into(), stored.into());
        if rec.len() > 5 {
            if let Ok(Some(vt)) = decode_pattern(&rec.children[5]) {
                attr.insert("valueType".into(), Value::Object(vt));
            }
        }
        attrs.push(Value::Object(attr));
    }
    let mut names: HashMap<i64, String> = HashMap::new();
    for a in &attrs {
        names.insert(a["id"].as_i64().unwrap_or(0), a["name"].as_str().unwrap_or("").to_string());
    }
    let mut links: HashMap<i64, Value> = HashMap::new();
    if section.len() > 3 {
        for link in table(Some(&section.children[3]), "data paths")? {
            let link = node(link, "data path", 2)?;
            let body = node(&link.children[1], "data path", 2)?;
            if is_bare(&body.children[0], "1") {
                let attr_id = int_or_first(&body.children[1], "attribute id")?;
                let resolved = resolve_data_path(attr_id, None, &names, meta_lookup);
                links.insert(int(&link.children[0], "control id")?, resolved);
            } else if is_bare(&body.children[0], "2") {
                let attr_id = int_or_first(&body.children[1], "attribute id")?;
                let step = body.children.get(2);
                let resolved = resolve_data_path(attr_id, step, &names, meta_lookup);
                links.insert(int(&link.children[0], "control id")?, resolved);
            }
        }
    }
    Ok((if main_id > 0 { Some(main_id) } else { None }, attrs, links))
}

fn apply_links(ctrls: &mut [Value], links: &HashMap<i64, Value>) {
    for c in ctrls {
        let Some(obj) = c.as_object_mut() else { continue };
        if let Some(link) = obj.get("id").and_then(Value::as_i64).and_then(|id| links.get(&id)) {
            obj.insert("dataPath".into(), link.clone());
        }
        let parent_dp = obj.get("dataPath").cloned();
        if let Some(Value::Array(cols)) = obj.get_mut("columns") {
            for col in cols {
                let Some(col_obj) = col.as_object_mut() else { continue };
                if let Some(Value::String(field)) = col_obj.remove("_field") {
                    if let Some(dp) = &parent_dp {
                        let mut cdp = Map::new();
                        cdp.insert("attributeId".into(), dp["attributeId"].clone());
                        if let Some(root_path) = dp.get("path").and_then(Value::as_str) {
                            cdp.insert("path".into(), format!("{root_path}.{field}").into());
                        }
                        col_obj.insert("dataPath".into(), Value::Object(cdp));
                    } else {
                        let mut cdp = Map::new();
                        cdp.insert("path".into(), field.into());
                        col_obj.insert("dataPath".into(), Value::Object(cdp));
                    }
                }
            }
        }
        if let Some(Value::Array(ch)) = obj.get_mut("children") {
            apply_links(ch, links);
        }
    }
}

pub fn decode_form(info: &FormInfo, meta_lookup: Option<&MetaLookup>) -> Res<Value> {
    decode_oof_stream(&info.layout_payload.raw, &info.uuid, meta_lookup)
}

/// Decode a `{27,{18,...}}` form stream; `meta_lookup` maps lowercase metadata uuids to names.
///
/// The returned model has the keys `caption, width, height, mainAttribute, extension, events, attributes,
/// controls, warnings` (the last one holds the `FORM_CONTROL_UNKNOWN`/`FORM_PROPERTY_UNREADABLE` diagnostics).
pub fn decode_oof_stream(raw: &[u8], form_uuid: &str, meta_lookup: Option<&MetaLookup>) -> Res<Value> {
    let root = parse_metadata(raw)?;
    if root.len() < 5 || !(is_bare(&root.children[0], "27") || is_bare(&root.children[0], "26")) {
        return Err(OofError::new("form stream does not start with {27,...}"));
    }
    let section = node(&root.children[1], "form section", 5)?;
    let version_ok = matches!(&section.children[0], Item::Atom(a)
        if a.kind == BARE && matches!(a.value.as_str(), "16" | "17" | "18"));
    if !version_ok {
        return Err(OofError::new("form section does not start with 16, 17 or 18"));
    }
    let head = node(&section.children[1], "form header", 1)?;
    let caption = localized(&head.children[0]).unwrap_or_default();
    let panel = node(&section.children[2], "root panel", 3)?;
    let width = int(&section.children[3], "width")?;
    let height = int(&section.children[4], "height")?;
    let mut warnings: Vec<Value> = Vec::new();
    let mut controls: Vec<Value> = Vec::new();
    for c in table(Some(&panel.children[2]), "root panel children")? {
        controls.push(control(c, &mut warnings, form_uuid, meta_lookup)?);
    }
    let (root_show_tabs, root_pages) = match &panel.children[1] {
        Item::Node(pinfo) => panel_pages(pinfo, width, height),
        Item::Atom(_) => (0, Vec::new()),
    };
    let (main_id, attrs, links) = attributes(node(&root.children[2], "attributes section", 3)?, meta_lookup)?;
    apply_links(&mut controls, &links);
    let ext = node(&root.children[3], "extension section", 1)?;
    let is_dp_ext = is_bare(&ext.children[0], EXTENSION_GUID);
    let mut events: Vec<Value> = collect_events(node(&root.children[4], "events section", 1)?)?
        .into_iter()
        .map(|(tag, handler)| json!({"name": event_name(None, tag), "id": tag, "handler": handler}))
        .collect();
    if let Item::Node(pinfo) = &panel.children[1] {
        for (tag, handler) in collect_events(pinfo)? {
            events.push(json!({"name": event_name(Some("Panel"), tag), "id": tag, "handler": handler}));
        }
    }
    let ext_map: &[(i64, &str)] = if is_dp_ext { &EXT_DP_EVENTS } else { &EXT_DOC_EVENTS };
    for (tag, handler) in collect_events(ext)? {
        let ename = ext_map.iter().find(|(t, _)| *t == tag).map(|(_, n)| *n).unwrap_or("Event");
        events.push(json!({"name": ename, "id": tag, "handler": handler}));
    }
    let mut out = Map::new();
    out.insert("caption".into(), localized_value(&caption));
    out.insert("width".into(), width.into());
    out.insert("height".into(), height.into());
    out.insert("mainAttribute".into(), main_id.map(Value::from).unwrap_or(Value::Null));
    out.insert("extension".into(), is_dp_ext.into());
    out.insert("events".into(), Value::Array(events));
    out.insert("attributes".into(), Value::Array(attrs));
    out.insert("controls".into(), Value::Array(controls));
    out.insert("warnings".into(), Value::Array(warnings));
    if root_pages.len() > 1 || root_show_tabs != 0 {
        out.insert("showTabs".into(), root_show_tabs.into());
        out.insert("pages".into(), Value::Array(root_pages));
    }
    Ok(Value::Object(out))
}

// ---------------------------------------------------------------- XML writer

/// `xml.sax.saxutils.escape`.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// `xml.sax.saxutils.quoteattr`.
fn quoteattr(s: &str) -> String {
    let mut data = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '&' => data.push_str("&amp;"),
            '<' => data.push_str("&lt;"),
            '>' => data.push_str("&gt;"),
            '\n' => data.push_str("&#10;"),
            '\r' => data.push_str("&#13;"),
            '\t' => data.push_str("&#9;"),
            _ => data.push(c),
        }
    }
    if data.contains('"') {
        if data.contains('\'') {
            format!("\"{}\"", data.replace('"', "&quot;"))
        } else {
            format!("'{data}'")
        }
    } else {
        format!("\"{data}\"")
    }
}

/// Python `str(value)` of a model value.
fn pystr(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn geti(v: &Value, key: &str, default: i64) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn gets<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

type Attrs<'a> = Vec<(&'a str, String)>;

struct Writer {
    lines: Vec<String>,
    depth: usize,
}

impl Writer {
    fn new() -> Writer {
        Writer { lines: vec!["<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_string()], depth: 0 }
    }

    fn attrs(attrs: &[(&str, String)]) -> String {
        attrs.iter().map(|(k, v)| format!(" {k}={}", quoteattr(v))).collect()
    }

    fn pad(&self) -> String {
        "  ".repeat(self.depth)
    }

    fn open(&mut self, tag: &str, attrs: &[(&str, String)]) {
        let line = format!("{}<{tag}{}>", self.pad(), Self::attrs(attrs));
        self.lines.push(line);
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth = self.depth.saturating_sub(1);
        let line = format!("{}</{tag}>", self.pad());
        self.lines.push(line);
    }

    fn leaf(&mut self, tag: &str, text: Option<&str>, attrs: &[(&str, String)]) {
        let pad = self.pad();
        let line = match text {
            None => format!("{pad}<{tag}{}/>", Self::attrs(attrs)),
            Some(t) => format!("{pad}<{tag}{}>{}</{tag}>", Self::attrs(attrs), escape(t)),
        };
        self.lines.push(line);
    }

    /// `items` is a list of `[language, text]` pairs.
    fn localized(&mut self, tag: &str, items: &[Value]) {
        if items.is_empty() {
            return;
        }
        self.open(tag, &[]);
        for item in items {
            let (lang, text) = (pystr(&item[0]), pystr(&item[1]));
            self.leaf("Item", Some(&text), &[("language", lang)]);
        }
        self.close(tag);
    }

    fn events(&mut self, events: &[Value]) {
        if events.is_empty() {
            return;
        }
        self.open("Events", &[]);
        for e in events {
            self.leaf(gets(e, "name"), Some(gets(e, "handler")), &[("id", pystr(&e["id"]))]);
        }
        self.close("Events");
    }

    fn value_type(&mut self, vt: &Value) {
        let mut attrs: Attrs = vec![("kind", pystr(&vt["kind"])), ("composite", pystr(&vt["composite"]).to_lowercase())];
        if let Some(f) = vt.get("fractionDigits") {
            attrs.push(("fractionDigits", pystr(f)));
        }
        self.leaf("ValueType", None, &attrs);
    }

    fn props(&mut self, p: &Value) {
        for (tag, key) in [("TextColor", "textColor"), ("BackColor", "backColor"), ("BorderColor", "borderColor")] {
            if let Some(color) = p.get(key) {
                let attrs: Attrs = ["kind", "value", "name", "ref"]
                    .iter()
                    .filter_map(|k| color.get(*k).map(|v| (*k, pystr(v))))
                    .collect();
                self.leaf(tag, None, &attrs);
            }
        }
        if let Some(f) = p.get("font") {
            let attrs: Attrs = ["bold", "source", "face", "size"]
                .iter()
                .filter_map(|k| f.get(*k).map(|v| (*k, if *k == "bold" { pystr(v).to_lowercase() } else { pystr(v) })))
                .collect();
            self.leaf("Font", None, &attrs);
        }
        self.localized("Tooltip", arr(p, "tooltip"));
        if let Some(v) = p.get("horizontalAlign") {
            self.leaf("HorizontalAlign", Some(&pystr(v)), &[]);
        }
        if truthy(p.get("hyperlink")) {
            self.leaf("Hyperlink", Some("true"), &[]);
        }
        if truthy(p.get("picture")) {
            self.leaf("Picture", Some("true"), &[]);
        }
        if let Some(bs) = p.get("borderStyle") {
            self.leaf("BorderStyle", Some(&pystr(&bs["name"])), &[("code", pystr(&bs["code"]))]);
        }
        if let Some(vt) = p.get("valueType") {
            self.value_type(vt);
        }
        for (tag, key) in [("ChoiceButton", "choiceButton"), ("ClearButton", "clearButton"), ("OpenButton", "openButton")] {
            if let Some(v) = p.get(key) {
                self.leaf(tag, Some(&pystr(v).to_lowercase()), &[]);
            }
        }
        if truthy(p.get("autoMarkIncomplete")) {
            self.leaf("AutoMarkIncomplete", Some("true"), &[]);
        }
        if truthy(p.get("choiceList")) {
            self.open("ChoiceList", &[]);
            for item in arr(p, "choiceList") {
                let pres = arr(item, "presentation");
                let attrs = [("value", pystr(&item["value"]))];
                if !pres.is_empty() {
                    self.open("ChoiceItem", &attrs);
                    self.localized("Presentation", pres);
                    self.close("ChoiceItem");
                } else {
                    self.leaf("ChoiceItem", None, &attrs);
                }
            }
            self.close("ChoiceList");
        }
        if let Some(v) = p.get("footer") {
            self.leaf("Footer", Some(&pystr(v).to_lowercase()), &[]);
        }
        if let Some(v) = p.get("autoFill") {
            self.leaf("AutoFill", Some(&pystr(v).to_lowercase()), &[]);
        }
        if truthy(p.get("buttons")) {
            self.buttons(arr(p, "buttons"));
        }
    }

    fn buttons(&mut self, items: &[Value]) {
        self.open("Buttons", &[]);
        for b in items {
            let mut attrs: Attrs = vec![("name", pystr(&b["name"]))];
            if b.get("id").is_some_and(|v| !v.is_null()) {
                attrs.push(("id", pystr(&b["id"])));
            }
            attrs.push(("kind", pystr(&b["kind"])));
            attrs.push(("checked", pystr(&b["checked"]).to_lowercase()));
            let repr = b.get("representation").map(pystr).unwrap_or_else(|| "Auto".into());
            if repr != "Auto" {
                attrs.push(("representation", repr));
            }
            if truthy(b.get("hasPicture")) {
                attrs.push(("hasPicture", "true".into()));
            }
            if truthy(b.get("actionKind")) {
                attrs.push(("actionKind", pystr(&b["actionKind"])));
            }
            if truthy(b.get("action")) {
                attrs.push(("action", pystr(&b["action"])));
            }
            let has_children = truthy(b.get("caption")) || truthy(b.get("tooltip")) || truthy(b.get("buttons"));
            if has_children {
                self.open("Button", &attrs);
                self.localized("Caption", arr(b, "caption"));
                self.localized("Tooltip", arr(b, "tooltip"));
                if truthy(b.get("buttons")) {
                    self.buttons(arr(b, "buttons"));
                }
                self.close("Button");
            } else {
                self.leaf("Button", None, &attrs);
            }
        }
        self.close("Buttons");
    }

    fn pages(&mut self, c: &Value, pages: &[Value]) {
        let mut by_page: Vec<Vec<&Value>> = vec![Vec::new(); pages.len()];
        for ch in arr(c, "children") {
            let idx = geti(ch, "page", 0);
            let slot = if (0..pages.len() as i64).contains(&idx) { idx as usize } else { 0 };
            by_page[slot].push(ch);
        }
        self.open("ChildItems", &[]);
        for (idx, p) in pages.iter().enumerate() {
            self.open("Page", &[("name", pystr(&p["name"]))]);
            self.localized("Title", arr(p, "title"));
            for key in ["left", "top", "width", "height"] {
                self.leaf(&capitalize(key), Some(&pystr(&p[key])), &[]);
            }
            if !truthy(p.get("visible")) {
                self.leaf("Visible", Some("false"), &[]);
            }
            if !truthy(p.get("enabled")) {
                self.leaf("Enabled", Some("false"), &[]);
            }
            let mut members = by_page[idx].clone();
            members.sort_by_key(|ch| (geti(ch, "ordinal", 0), geti(ch, "id", 0)));
            if !members.is_empty() {
                self.open("ChildItems", &[]);
                for ch in members {
                    self.control(ch);
                }
                self.close("ChildItems");
            }
            self.close("Page");
        }
        self.close("ChildItems");
    }

    fn control(&mut self, c: &Value) {
        let tag = gets(c, "type");
        let mut attrs: Attrs = vec![("name", pystr(&c["name"])), ("id", pystr(&c["id"]))];
        if tag == "UnknownControl" {
            attrs.push(("guid", pystr(&c["guid"])));
        }
        self.open(tag, &attrs);
        if let Some(dp) = c.get("dataPath") {
            let path = dp.get("path").filter(|v| !v.is_null()).map(pystr);
            self.leaf("DataPath", path.as_deref(), &[("attributeId", pystr(&dp["attributeId"]))]);
        }
        self.localized("Caption", arr(c, "caption"));
        for key in ["left", "top", "width", "height"] {
            self.leaf(&capitalize(key), Some(&pystr(&c[key])), &[]);
        }
        self.leaf("ZOrder", Some(&geti(c, "ordinal", 0).to_string()), &[]);
        if geti(c, "tabOrder", 0) > 0 {
            self.leaf("TabOrder", Some(&geti(c, "tabOrder", 0).to_string()), &[]);
        }
        if !truthy(c.get("visible")) {
            self.leaf("Visible", Some("false"), &[]);
        }
        let pages: &[Value] = if tag == "Panel" { arr(c, "pages") } else { &[] };
        if !pages.is_empty() {
            let show_tabs = geti(c, "showTabs", 0);
            let member = match show_tabs {
                0 => "None",
                1 => "Top",
                _ => "Unknown",
            };
            self.leaf(
                "ShowTabs",
                None,
                &[("type", "ShowTabs".into()), ("member", member.into()), ("code", show_tabs.to_string())],
            );
        }
        self.props(c.get("props").unwrap_or(&Value::Null));
        let cols = arr(c, "columns");
        if !cols.is_empty() {
            self.open("Columns", &[]);
            for col in cols {
                let mut col_attrs: Attrs = vec![("name", pystr(&col["name"]))];
                if col.get("id").is_some_and(|v| !v.is_null()) {
                    col_attrs.push(("id", pystr(&col["id"])));
                }
                self.open("Column", &col_attrs);
                if let Some(dp) = col.get("dataPath") {
                    let path = dp.get("path").filter(|v| !v.is_null()).map(pystr);
                    let mut dp_attrs: Attrs = Vec::new();
                    if dp.get("attributeId").is_some_and(|v| !v.is_null()) {
                        dp_attrs.push(("attributeId", pystr(&dp["attributeId"])));
                    }
                    self.leaf("DataPath", path.as_deref(), &dp_attrs);
                }
                self.localized("Header", arr(col, "header"));
                self.localized("FooterText", arr(col, "footerText"));
                self.localized("Tooltip", arr(col, "tooltip"));
                if let Some(w) = col.get("width") {
                    self.leaf("Width", Some(&pystr(w)), &[]);
                }
                if col.get("visible") == Some(&Value::Bool(false)) {
                    self.leaf("Visible", Some("false"), &[]);
                }
                if let Some(vt) = col.get("valueType") {
                    self.value_type(vt);
                }
                if truthy(col.get("readOnly")) {
                    self.leaf("ReadOnly", Some("true"), &[]);
                }
                if truthy(col.get("editor")) {
                    self.leaf("Editor", Some(&pystr(&col["editor"])), &[]);
                }
                self.events(arr(col, "events"));
                self.close("Column");
            }
            self.close("Columns");
        }
        self.events(arr(c, "events"));
        let children = arr(c, "children");
        if !pages.is_empty() && (pages.len() > 1 || geti(c, "showTabs", 0) != 0 || !children.is_empty()) {
            self.pages(c, pages);
        } else if !children.is_empty() {
            self.open("ChildItems", &[]);
            for ch in children {
                self.control(ch);
            }
            self.close("ChildItems");
        }
        self.close(tag);
    }
}

/// `ET.fromstring` stand-in: the writer only emits escaped text, so the sole way to produce a document
/// that is not well-formed is a character outside the XML `Char` production.
fn check_well_formed(text: &str) -> Res<()> {
    let (mut line, mut col) = (1usize, 0usize);
    let mut prev_cr = false;
    for c in text.chars() {
        let ok = matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}');
        if !ok {
            return Err(OofError(format!("not well-formed (invalid token): line {line}, column {col}")));
        }
        match c {
            '\n' => {
                if !prev_cr {
                    line += 1;
                }
                col = 0;
                prev_cr = false;
            }
            '\r' => {
                line += 1;
                col = 0;
                prev_cr = true;
            }
            _ => {
                col += 1;
                prev_cr = false;
            }
        }
    }
    Ok(())
}

/// Render the decoded model as `Form.xml` (UTF-8, LF, trailing newline).
pub fn render_form_xml(info: &FormInfo, model: &Value) -> Res<Vec<u8>> {
    let mut w = Writer::new();
    w.open(
        "Form",
        &[
            ("id", info.uuid.clone()),
            ("name", info.name.clone()),
            ("ordinaryFormVersion", ORDINARY_FORM_VERSION.into()),
            ("reconstructionComplete", "false".into()),
        ],
    );
    w.localized("Caption", arr(model, "caption"));
    w.leaf("Width", Some(&pystr(&model["width"])), &[]);
    w.leaf("Height", Some(&pystr(&model["height"])), &[]);
    if !model["mainAttribute"].is_null() {
        w.leaf("MainAttribute", None, &[("attributeId", pystr(&model["mainAttribute"]))]);
    }
    if truthy(model.get("extension")) {
        w.leaf("DataProcessorFormExtension", None, &[]);
    }
    w.events(arr(model, "events"));
    let attrs = arr(model, "attributes");
    if !attrs.is_empty() {
        w.open("Attributes", &[]);
        for a in attrs {
            let (id, name) = (pystr(&a["id"]), pystr(&a["name"]));
            let (main, stored, has_vt) = (truthy(a.get("main")), truthy(a.get("stored")), a.get("valueType").is_some());
            if !(main || stored || has_vt) {
                w.leaf("Attribute", None, &[("id", id), ("name", name)]);
                continue;
            }
            w.open("Attribute", &[("id", id), ("name", name)]);
            if main {
                w.leaf("Main", Some("true"), &[]);
            }
            if stored {
                w.leaf("StoredData", Some("true"), &[]);
            }
            if let Some(vt) = a.get("valueType") {
                w.value_type(vt);
            }
            w.close("Attribute");
        }
        w.close("Attributes");
    }
    let root_pages = arr(model, "pages");
    let controls = arr(model, "controls");
    if !root_pages.is_empty() {
        let show_tabs = geti(model, "showTabs", 0);
        let member = match show_tabs {
            0 => "None",
            1 => "Top",
            _ => "Unknown",
        };
        w.leaf(
            "ShowTabs",
            None,
            &[("type", "ShowTabs".into()), ("member", member.into()), ("code", show_tabs.to_string())],
        );
        w.pages(&json!({"children": controls}), root_pages);
    } else if !controls.is_empty() {
        w.open("ChildItems", &[]);
        for c in controls {
            w.control(c);
        }
        w.close("ChildItems");
    }
    w.close("Form");
    let text = w.lines.join("\n") + "\n";
    check_well_formed(&text)?; // the output must be well-formed XML
    Ok(text.into_bytes())
}

// ---------------------------------------------------------------- external oof

fn xml_tag(n: roxmltree::Node) -> String {
    let t = n.tag_name();
    match t.namespace() {
        Some(ns) => format!("{{{ns}}}{}", t.name()),
        None => t.name().to_string(),
    }
}

/// Child elements with the given ElementTree tag (all of them for `None`).
fn child_elements<'a, 'i>(n: roxmltree::Node<'a, 'i>, tag: Option<&str>) -> Vec<roxmltree::Node<'a, 'i>> {
    n.children().filter(|c| c.is_element() && tag.is_none_or(|t| t == xml_tag(*c))).collect()
}

/// ElementTree `Element.text`.
fn xml_text(n: roxmltree::Node) -> Option<String> {
    match n.first_child() {
        Some(c) if c.is_text() => c.text().map(str::to_string),
        _ => None,
    }
}

fn opt_str(v: Option<&str>) -> Value {
    v.map(Value::from).unwrap_or(Value::Null)
}

type Summary = (Vec<Value>, Vec<Value>, Vec<Value>);

fn summarize_xml(data: &[u8]) -> Result<Summary, ()> {
    let text = std::str::from_utf8(data).map_err(|_| ())?;
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(text, opts).map_err(|_| ())?;
    let root = doc.root_element();

    let flag = |a: roxmltree::Node, tag: &str| -> bool {
        child_elements(a, Some(tag))
            .first()
            .map(|e| xml_text(*e).unwrap_or_default().trim() == "true")
            .unwrap_or(false)
    };
    let mut attributes = Vec::new();
    for group in child_elements(root, Some("Attributes")) {
        for a in child_elements(group, Some("Attribute")) {
            attributes.push(json!({
                "id": opt_str(a.attribute("id")),
                "name": opt_str(a.attribute("name")),
                "main": flag(a, "Main"),
                "stored": flag(a, "StoredData"),
            }));
        }
    }
    let mut events = Vec::new();
    for group in child_elements(root, Some("Events")) {
        for e in child_elements(group, None) {
            events.push(json!({
                "name": xml_tag(e),
                "id": opt_str(e.attribute("id")),
                "handler": xml_text(e).unwrap_or_default().trim(),
            }));
        }
    }

    fn ctrl(e: roxmltree::Node) -> Value {
        let mut out = Map::new();
        out.insert("type".into(), xml_tag(e).into());
        out.insert("name".into(), opt_str(e.attribute("name")));
        out.insert("id".into(), opt_str(e.attribute("id")));
        if let Some(dp) = child_elements(e, Some("DataPath")).first() {
            let path = xml_text(*dp).unwrap_or_default().trim().to_string();
            let path = if path.is_empty() { Value::Null } else { path.into() };
            out.insert("dataPath".into(), json!({"attributeId": opt_str(dp.attribute("attributeId")), "path": path}));
        }
        let mut kids: Vec<roxmltree::Node> = Vec::new();
        for ci in child_elements(e, Some("ChildItems")) {
            kids.extend(child_elements(ci, None).into_iter().filter(|x| xml_tag(*x) != "Page"));
        }
        for ci in child_elements(e, Some("ChildItems")) {
            for page in child_elements(ci, Some("Page")) {
                for pci in child_elements(page, Some("ChildItems")) {
                    kids.extend(child_elements(pci, None));
                }
            }
        }
        out.insert("children".into(), Value::Array(kids.into_iter().map(ctrl).collect()));
        Value::Object(out)
    }

    let mut controls = Vec::new();
    for ci in child_elements(root, Some("ChildItems")) {
        for c in child_elements(ci, None) {
            controls.push(ctrl(c));
        }
    }
    Ok((attributes, events, controls))
}

/// OS error text without the Rust ` (os error N)` suffix (what Python shows as `strerror`).
fn os_error_text(e: &std::io::Error) -> String {
    let s = e.to_string();
    let s = match s.rfind(" (os error ") {
        Some(i) => s[..i].to_string(),
        None => s,
    };
    let s = s.trim_end_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    if s.is_empty() {
        "OSError".to_string()
    } else {
        s
    }
}

fn unique_temp_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("oof-{}-{nanos:x}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)))
}

fn read_pipe<R: Read + Send + 'static>(r: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        buf
    })
}

/// First `OOF\d{4}` in the output.
fn find_oof_code(output: &str) -> Option<String> {
    let b = output.as_bytes();
    (0..b.len().saturating_sub(6))
        .find(|&i| &b[i..i + 3] == b"OOF" && b[i + 3..i + 7].iter().all(u8::is_ascii_digit))
        .map(|i| output[i..i + 7].to_string())
}

type External = (Vec<u8>, Vec<Value>, Vec<Value>, Vec<Value>);

fn run_external(info: &FormInfo, oof_path: &str) -> Res<External> {
    let container = serialize_container(
        &[
            ("form", info.layout_payload.raw.as_slice(), None),
            ("module", info.module_payload.raw.as_slice(), None),
        ],
        0x200,
    )?;
    let tmp = unique_temp_dir();
    std::fs::create_dir_all(&tmp)
        .map_err(|e| OofError(format!("cannot create a temporary directory: {}", os_error_text(&e))))?;
    let result = run_external_in(&tmp, &container, oof_path);
    let _ = std::fs::remove_dir_all(&tmp);
    let raw = result?;
    let decode = || -> Result<External, ()> {
        let text = std::str::from_utf8(&raw).map_err(|_| ())?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let data = (text.trim_end_matches('\n').to_string() + "\n").into_bytes();
        let (a, e, c) = summarize_xml(&data)?;
        Ok((data, a, e, c))
    };
    decode().map_err(|_| OofError::new("external oof produced invalid Form.xml"))
}

fn run_external_in(tmp: &std::path::Path, container: &[u8], oof_path: &str) -> Res<Vec<u8>> {
    let (form_bin, form_xml) = (tmp.join("Form.bin"), tmp.join(FORM_XML_NAME));
    std::fs::write(&form_bin, container)
        .map_err(|e| OofError(format!("cannot write {}: {}", form_bin.display(), os_error_text(&e))))?;
    let mut cmd = Command::new(oof_path);
    cmd.arg("dump").arg(&form_bin).arg(&form_xml).arg("--json").stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut proc =
        cmd.spawn().map_err(|e| OofError(format!("external oof cannot be started: {}", os_error_text(&e))))?;
    let out_h = read_pipe(proc.stdout.take());
    let err_h = read_pipe(proc.stderr.take());
    let deadline = Instant::now() + Duration::from_secs(EXTERNAL_TIMEOUT_SECONDS);
    let status = loop {
        match proc.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = proc.kill();
                    let _ = proc.wait();
                    return Err(OofError::new("external oof timed out"));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(OofError(format!("external oof cannot be started: {}", os_error_text(&e)))),
        }
    };
    let stdout = out_h.join().unwrap_or_default();
    let stderr = err_h.join().unwrap_or_default();
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        let output = String::from_utf8_lossy(&stdout).into_owned() + &String::from_utf8_lossy(&stderr);
        let suffix = find_oof_code(&output).map(|c| format!(" ({c})")).unwrap_or_default();
        return Err(OofError(format!("external oof exit code {code}{suffix}")));
    }
    if !form_xml.is_file() {
        return Err(OofError::new("external oof produced no Form.xml"));
    }
    std::fs::read(&form_xml)
        .map_err(|e| OofError(format!("cannot read {}: {}", form_xml.display(), os_error_text(&e))))
}

// ---------------------------------------------------------------- entry points

/// Build `Form.xml` for one form; failures are warnings in `diagnostics` and `xml` is `None`.
pub fn build_oof(info: &FormInfo, alias: &str, oof_path: Option<&str>, meta_lookup: Option<&MetaLookup>) -> OofResult {
    let component = diag_component(&info.uuid);
    let mut diags: Vec<Value> = Vec::new();
    if let Some(path) = oof_path.filter(|p| !p.is_empty()) {
        match run_external(info, path) {
            Err(e) => diags.push(diagnostic(
                FORM_OOF_EXTERNAL_FAILED,
                "warning",
                &component,
                &[("alias", alias.into()), ("backend", "oof".into()), ("reason", e.0.into())],
            )),
            Ok((xml, attrs, events, controls)) => {
                diags.push(diagnostic(
                    FORM_PROJECTION_INCOMPLETE,
                    "warning",
                    &component,
                    &[("alias", alias.into()), ("backend", "oof".into())],
                ));
                return OofResult {
                    xml: Some(xml),
                    attributes: attrs,
                    events,
                    controls,
                    engine: "external".into(),
                    diagnostics: diags,
                };
            }
        }
    }
    let built = decode_form(info, meta_lookup).and_then(|model| render_form_xml(info, &model).map(|xml| (model, xml)));
    match built {
        Err(e) => {
            diags.push(diagnostic(
                FORM_XML_EXTRACTION_FAILED,
                "warning",
                &component,
                &[("alias", alias.into()), ("backend", "oof".into()), ("reason", e.0.into())],
            ));
            OofResult { xml: None, engine: "builtin".into(), diagnostics: diags, ..Default::default() }
        }
        Ok((model, xml)) => {
            diags.extend(arr(&model, "warnings").iter().cloned());
            diags.push(diagnostic(
                FORM_PROJECTION_INCOMPLETE,
                "warning",
                &component,
                &[("alias", alias.into()), ("backend", "oof".into())],
            ));
            let take = |k: &str| arr(&model, k).to_vec();
            OofResult {
                xml: Some(xml),
                attributes: take("attributes"),
                events: take("events"),
                controls: take("controls"),
                engine: "builtin".into(),
                diagnostics: diags,
            }
        }
    }
}

/// Upgrade a basic `FormDefinition` object in place with the decoded `Form.xml` data.
pub fn apply_projection(definition: &mut Map<String, Value>, result: &OofResult) {
    definition.insert(
        "projection".into(),
        json!({
            "backend": "oof",
            "status": "partial",
            "referenceOnly": true,
            "controlsKnown": true,
            "formXml": FORM_XML_NAME,
            "unknownSections": ["UnsupportedControlProperties"],
        }),
    );
    definition.insert("attributes".into(), Value::Array(result.attributes.clone()));
    definition.insert("events".into(), Value::Array(result.events.clone()));
    definition.insert("controls".into(), Value::Array(result.controls.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Payload;

    fn payload(raw: &[u8]) -> Payload {
        Payload { locator: "p".into(), raw: raw.to_vec(), sha256: String::new() }
    }

    fn form_info(uuid: &str, name: &str, layout: &[u8]) -> FormInfo {
        FormInfo {
            uuid: uuid.into(),
            name: name.into(),
            synonym: String::new(),
            kind_code: 0,
            header_payload: payload(b""),
            layout_payload: payload(layout),
            module_payload: payload(b""),
            kind: "OrdinaryForm".into(),
        }
    }

    fn parsed(s: &str) -> Node {
        parse_metadata(s.as_bytes()).unwrap()
    }

    const ANCHORS: &str = "{0,{2,-1,6,0}},{0,{2,-1,6,0}},{0,{2,-1,6,0}},{0,{2,-1,6,0}},{0,{2,-1,6,0}},{0,{2,-1,6,0}}";

    // port: tests/test_oof.py::test_geom_page_reads_tail_and_falls_back
    #[test]
    fn geom_page_reads_tail_and_falls_back() {
        let lists = "0,2,{0,54,0},{0,56,0},0,0,0,0"; // six counted incoming lists
        let geom = parsed(&format!("{{8,8,125,692,394,1,{ANCHORS},{lists},3,15,14,1,0}}"));
        assert_eq!(geom_page(&geom), (3, 15, 14));
        assert_eq!(geom_page(&parsed("{8,8,125,692,394,1}")), (0, 0, 0));
        assert_eq!(geom_page(&parsed(&format!("{{8,8,125,692,394,1,{ANCHORS},9,0}}"))), (0, 0, 0));
    }

    // port: tests/test_oof.py::test_panel_pages_unexpected_shape_is_empty
    #[test]
    fn panel_pages_unexpected_shape_is_empty() {
        assert_eq!(panel_pages(&parsed("{1,{0},{0}}"), 100, 100), (0, vec![]));
        assert_eq!(panel_pages(&parsed("{1}"), 100, 100), (0, vec![]));
    }

    fn page(name: &str, visible: bool, enabled: bool) -> Value {
        json!({"name": name, "title": [["ru", name]], "visible": visible, "enabled": enabled,
               "left": 1, "top": 19, "width": 98, "height": 80})
    }

    fn ctrl(kind: &str, name: &str, cid: i64) -> Map<String, Value> {
        let Value::Object(m) = json!({"type": kind, "name": name, "id": cid, "guid": "g", "caption": [], "left": 0,
            "top": 0, "width": 5, "height": 5, "visible": true, "page": 0, "ordinal": 0, "events": [],
            "columns": [], "children": []})
        else {
            unreachable!()
        };
        m
    }

    fn with(mut m: Map<String, Value>, kv: Value) -> Value {
        for (k, v) in kv.as_object().unwrap() {
            m.insert(k.clone(), v.clone());
        }
        Value::Object(m)
    }

    // port: tests/test_oof.py::test_writer_groups_panel_children_by_page
    #[test]
    fn writer_groups_panel_children_by_page() {
        let panel = with(
            ctrl("Panel", "P", 1),
            json!({"showTabs": 1, "pages": [page("A", true, true), page("B", false, false)], "children": [
                with(ctrl("Button", "B2", 4), json!({"page": 1, "ordinal": 1})),
                with(ctrl("Button", "B1", 3), json!({"page": 1, "ordinal": 0})),
                with(ctrl("Button", "A1", 2), json!({"page": 0})),
                with(ctrl("Button", "Lost", 5), json!({"page": 7})),
            ]}),
        );
        let mut w = Writer::new();
        w.lines.clear();
        w.control(&panel);
        let text = w.lines.join("\n");
        let doc = roxmltree::Document::parse(&text).unwrap();
        let root = doc.root_element();
        let names = |n: roxmltree::Node<'_, '_>, tag: &str| -> Vec<String> {
            n.children()
                .filter(|c| c.is_element() && c.tag_name().name() == tag)
                .map(|c| c.attribute("name").unwrap_or("").to_string())
                .collect()
        };
        let show = root.children().find(|c| c.has_tag_name("ShowTabs")).unwrap();
        assert_eq!(show.attribute("type"), Some("ShowTabs"));
        assert_eq!(show.attribute("member"), Some("Top"));
        assert_eq!(show.attribute("code"), Some("1"));
        let items = root.children().find(|c| c.has_tag_name("ChildItems")).unwrap();
        let pages: Vec<_> = items.children().filter(|c| c.has_tag_name("Page")).collect();
        assert_eq!(names(items, "Page"), ["A", "B"]);
        let buttons = |p: roxmltree::Node<'_, '_>| {
            let ci = p.children().find(|c| c.has_tag_name("ChildItems")).unwrap();
            names(ci, "Button")
        };
        assert_eq!(buttons(pages[0]), ["A1", "Lost"]);
        assert_eq!(buttons(pages[1]), ["B1", "B2"]);
        let text_of = |p: roxmltree::Node<'_, '_>, tag: &str| {
            p.children().find(|c| c.has_tag_name(tag)).map(|c| c.text().unwrap_or("").to_string())
        };
        assert_eq!(text_of(pages[1], "Visible").as_deref(), Some("false"));
        assert_eq!(text_of(pages[1], "Enabled").as_deref(), Some("false"));
        assert_eq!(text_of(pages[0], "Visible"), None);
        assert_eq!(text_of(pages[0], "Top").as_deref(), Some("19"));
    }

    // port: tests/test_oof.py::test_summarize_xml_flattens_pages
    #[test]
    fn summarize_xml_flattens_pages() {
        let panel = with(
            ctrl("Panel", "P", 1),
            json!({"showTabs": 1, "pages": [page("A", true, true)], "children": [ctrl("Button", "A1", 2)]}),
        );
        let mut w = Writer::new();
        w.lines.clear();
        w.open("Form", &[]);
        w.open("ChildItems", &[]);
        w.control(&panel);
        w.close("ChildItems");
        w.close("Form");
        let (_, _, ctrls) = summarize_xml(w.lines.join("\n").as_bytes()).unwrap();
        assert_eq!(ctrls[0]["children"][0]["name"], "A1");
    }

    #[test]
    fn quoteattr_and_escape_match_saxutils() {
        assert_eq!(quoteattr("a"), "\"a\"");
        assert_eq!(quoteattr("a\"b"), "'a\"b'");
        assert_eq!(quoteattr("a\"b'c"), "\"a&quot;b'c\"");
        assert_eq!(quoteattr("x\n\r\t<&>"), "\"x&#10;&#13;&#9;&lt;&amp;&gt;\"");
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;\"c\"");
    }

    #[test]
    fn control_guids_are_complete() {
        assert_eq!(CONTROL_GUIDS.len(), 27);
        assert_eq!(control_type("ea83fe3a-ac3c-4cce-8045-3dddf35b28b1"), Some("Table"));
        assert_eq!(control_type("48537cf6-4901-4b8f-9869-e4b1564af79c"), Some("PDFDocumentField"));
        assert_eq!(control_type("nope"), None);
    }

    // port: tests/test_oof.py::test_broken_form_stream_warns (build_oof level)
    #[test]
    fn broken_stream_is_a_warning() {
        let info = form_info("u1", "F", b"{1,2}");
        let r = build_oof(&info, "Alias", None, None);
        assert!(r.xml.is_none());
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.diagnostics[0]["code"], FORM_XML_EXTRACTION_FAILED);
        assert_eq!(r.diagnostics[0]["component"], "Forms/u1");
        assert_eq!(r.diagnostics[0]["params"]["reason"], "form stream does not start with {27,...}");
        let r = build_oof(&form_info("u1", "F", b"nope"), "Alias", None, None);
        assert_eq!(r.diagnostics[0]["params"]["reason"], "value outside of the root list");
    }

    #[test]
    fn invalid_xml_char_is_reported_like_expat() {
        assert!(check_well_formed("<a>\u{1}</a>").is_err());
        assert_eq!(
            check_well_formed("<a>\n  \u{1}</a>").unwrap_err().0,
            "not well-formed (invalid token): line 2, column 2"
        );
        assert!(check_well_formed("<a>\t\r\n</a>").is_ok());
    }

    const GOOD_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form id=\"x\" name=\"Mock\" ordinaryFormVersion=\"2.1\" reconstructionComplete=\"false\">\r\n  <Attributes><Attribute id=\"1\" name=\"A\"><Main>true</Main></Attribute></Attributes>\r\n  <ChildItems><Button name=\"B\" id=\"2\"/></ChildItems>\r\n</Form>\r\n";

    /// Executable stand-in for `oof dump <bin> <xml> --json`.
    fn mock_oof(dir: &std::path::Path, exit_code: i32, write: bool) -> String {
        if cfg!(windows) {
            let path = dir.join("oof.bat");
            std::fs::write(dir.join("good.xml"), GOOD_XML).unwrap();
            let mut body = String::from("@echo off\r\n");
            if write {
                body.push_str("copy /y \"%~dp0good.xml\" \"%3\" >nul\r\n");
            }
            body.push_str(&format!("exit /b {exit_code}\r\n"));
            std::fs::write(&path, body).unwrap();
            path.to_string_lossy().into_owned()
        } else {
            let path = dir.join("oof");
            let mut body = String::from("#!/bin/sh\n");
            if write {
                body.push_str(&format!("printf '%s' '{GOOD_XML}' > \"$3\"\n"));
            }
            body.push_str(&format!("exit {exit_code}\n"));
            std::fs::write(&path, body).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            path.to_string_lossy().into_owned()
        }
    }

    fn fixture_info() -> FormInfo {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/forms/DonorForm/FormStream.bin");
        form_info("u2", "F", &std::fs::read(p).unwrap())
    }

    // port: tests/test_oof.py::test_external_oof_failure_falls_back
    #[test]
    fn external_oof_failure_falls_back() {
        let dir = std::env::temp_dir().join(format!("oof-test-fail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (d1, d2) = (dir.join("exit"), dir.join("nofile"));
        std::fs::create_dir_all(&d1).unwrap();
        std::fs::create_dir_all(&d2).unwrap();
        let cases = [
            dir.join("nope").join("oof").to_string_lossy().into_owned(),
            mock_oof(&d1, 3, true),
            mock_oof(&d2, 0, false),
        ];
        let reasons = [None, Some("external oof exit code 3"), Some("external oof produced no Form.xml")];
        for (oof, want) in cases.iter().zip(reasons) {
            let r = build_oof(&fixture_info(), "A", Some(oof), None);
            let first = &r.diagnostics[0];
            assert_eq!(first["code"], FORM_OOF_EXTERNAL_FAILED);
            let reason = first["params"]["reason"].as_str().unwrap();
            match want {
                Some(w) => assert_eq!(reason, w),
                None => assert!(reason.starts_with("external oof cannot be started: "), "{reason}"),
            }
            assert_eq!(r.engine, "builtin");
            assert!(String::from_utf8(r.xml.unwrap()).unwrap().contains("<Table")); // built-in decoder
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // port: tests/test_oof.py::test_external_oof_success
    #[test]
    fn external_oof_success() {
        let dir = std::env::temp_dir().join(format!("oof-test-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let oof = mock_oof(&dir, 0, true);
        let r = build_oof(&fixture_info(), "A", Some(&oof), None);
        let xml = String::from_utf8(r.xml.clone().unwrap()).unwrap();
        assert!(!xml.contains('\r') && xml.contains("name=\"Mock\"") && xml.ends_with("</Form>\n"));
        assert_eq!(r.engine, "external");
        assert_eq!(r.diagnostics.len(), 1);
        assert_eq!(r.diagnostics[0]["code"], FORM_PROJECTION_INCOMPLETE);
        assert_eq!(r.attributes[0]["name"], "A");
        assert_eq!(r.attributes[0]["main"], true);
        assert_eq!(r.controls[0]["name"], "B");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
