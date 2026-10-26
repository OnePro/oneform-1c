//! Property decoding of ordinary-form controls (appearance, value types, buttons) for `oof`
//! (port of `adapters/oof_props.py`).
//!
//! Everything here reads fixed indices of the control `info` brace tree. A block that does not have the
//! expected shape yields an [`OofError`]; the caller turns it into a `FORM_PROPERTY_UNREADABLE` warning
//! and skips the group. Python `IndexError`/`ValueError` are folded into [`OofError`] as well.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Map, Value};

use crate::brace::{Item, Node, BARE, QUOTED};
use crate::canonical::py_round;
use crate::errors::ExtractionError;
use crate::metaref::{ANY_REF_GUID, VALUE_LIST_GUID, VALUE_TABLE_GUID, VALUE_TREE_GUID};
use crate::naming::is_uuid;

pub const EVENT_GUID: &str = "e1692cc2-605b-4535-84dd-28440238746c";
pub const STANDARD_ACTION_GUID: &str = "fbe38877-b914-4fd5-8540-07dde06ba2e1";
pub const SUBMENU_ACTION_GUID: &str = "abde0c9a-18a6-4e0c-bbaa-af26b911b3e6";
pub const STD_SUBMENU_ACTION_GUID: &str = "c93a51ed-b2d1-47f1-948d-38373f8710af";
pub const OPEN_MOVEMENTS_GUID: &str = "fb3652fd-bf0c-4e8d-b542-dd1312ca02a7";
pub const NULL_GROUP_GUID: &str = "9d0a2e40-b978-11d4-84b6-008048da06df";
pub const ZERO_GUID: &str = "00000000-0000-0000-0000-000000000000";

/// Structural mismatch of a form stream; becomes a warning, never a fatal error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OofError(pub String);

impl OofError {
    pub fn new(message: impl Into<String>) -> OofError {
        OofError(message.into())
    }
}

impl std::fmt::Display for OofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OofError {}

/// Only the message of an `ExtractionError` is ever shown by the Reference (`e.message`).
impl From<ExtractionError> for OofError {
    fn from(e: ExtractionError) -> OofError {
        OofError(e.message)
    }
}

/// Python `IndexError` text.
pub fn index_error() -> OofError {
    OofError::new("list index out of range")
}

fn unexpected() -> OofError {
    OofError::new("unexpected structure")
}

// ---------------------------------------------------------------- node helpers

/// `n.children[i]` with an error instead of a panic.
pub fn child(n: &Node, i: usize) -> Result<&Item, OofError> {
    n.children.get(i).ok_or_else(index_error)
}

/// Python `list[i]` with a possibly negative index.
pub fn py_get(items: &[Item], i: i64) -> Option<&Item> {
    let len = items.len() as i64;
    let j = if i < 0 { i + len } else { i };
    if (0..len).contains(&j) {
        items.get(j as usize)
    } else {
        None
    }
}

/// Python `list[start:stop]` (negative bounds count from the end, out-of-range bounds are clamped).
pub fn py_slice(items: &[Item], start: i64, stop: i64) -> &[Item] {
    let len = items.len() as i64;
    let norm = |v: i64| if v < 0 { (v + len).max(0) } else { v.min(len) };
    let (s, e) = (norm(start), norm(stop));
    if s >= e {
        &[]
    } else {
        &items[s as usize..e as usize]
    }
}

/// `_node`: a list with at least `min_len` children.
pub fn node<'a>(n: &'a Item, what: &str, min_len: usize) -> Result<&'a Node, OofError> {
    match n {
        Item::Node(x) if x.len() >= min_len => Ok(x),
        _ => Err(OofError(format!("{what}: unexpected structure"))),
    }
}

/// `_node` for a value that may be absent (Python `None`).
pub fn node_opt<'a>(n: Option<&'a Item>, what: &str, min_len: usize) -> Result<&'a Node, OofError> {
    match n {
        Some(n) => node(n, what, min_len),
        None => Err(OofError(format!("{what}: unexpected structure"))),
    }
}

/// Python `int(str)` for the ASCII forms (`+5`, `-5`, `1_000`).
fn py_int(s: &str) -> Option<i64> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if body.is_empty() || body.starts_with('_') || body.ends_with('_') || body.contains("__") {
        return None;
    }
    let digits: String = body.chars().filter(|c| *c != '_').collect();
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

pub fn int(v: &Item, what: &str) -> Result<i64, OofError> {
    if let Item::Atom(a) = v {
        if a.kind == BARE {
            if let Some(i) = py_int(&a.value) {
                return Ok(i);
            }
        }
    }
    Err(OofError(format!("{what}: integer expected")))
}

fn is_num(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if b.first() == Some(&b'-') {
        i += 1;
    }
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > start
    };
    if !digits(&mut i) {
        return false;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        if !digits(&mut i) {
            return false;
        }
    }
    if matches!(b.get(i), Some(b'e') | Some(b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+') | Some(b'-')) {
            i += 1;
        }
        if !digits(&mut i) {
            return false;
        }
    }
    i == b.len()
}

/// Number atom: integers, decimals and scientific notation (`1e2`, `1.5e2`, `33.00000000000001`).
pub fn num(v: &Item) -> Result<f64, OofError> {
    if let Item::Atom(a) = v {
        if a.kind == BARE && is_num(&a.value) {
            if let Ok(f) = a.value.parse::<f64>() {
                return Ok(f);
            }
        }
    }
    Err(OofError::new("number expected"))
}

/// An id stored either as an atom or as a single-element list `{id}`.
pub fn int_or_first(v: &Item, what: &str) -> Result<i64, OofError> {
    match v {
        Item::Node(n) if !n.is_empty() => int(&n.children[0], what),
        _ => int(v, what),
    }
}

/// `_str`: a quoted atom.
pub fn str_val<'a>(v: &'a Item, what: &str) -> Result<&'a str, OofError> {
    if let Item::Atom(a) = v {
        if a.kind == QUOTED {
            return Ok(&a.value);
        }
    }
    Err(OofError(format!("{what}: string expected")))
}

pub fn is_bare(v: &Item, value: &str) -> bool {
    matches!(v, Item::Atom(a) if a.kind == BARE && a.value == value)
}

pub fn is_guid(v: &Item) -> bool {
    match v {
        Item::Atom(a) if a.kind == BARE => is_guid_str(&a.value),
        _ => false,
    }
}

fn is_guid_str(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                *c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(c)
            }
        })
}

/// `n[i][j]...` with a structural error instead of an index error.
pub fn at_n<'a>(n: &'a Node, idx: &[i64]) -> Result<&'a Item, OofError> {
    let mut cur = n;
    let mut last: Option<&Item> = None;
    for (k, &i) in idx.iter().enumerate() {
        let it = py_get(&cur.children, i).ok_or_else(|| {
            if i >= cur.children.len() as i64 {
                unexpected()
            } else {
                index_error()
            }
        })?;
        last = Some(it);
        if k + 1 < idx.len() {
            cur = match it {
                Item::Node(x) => x,
                Item::Atom(_) => return Err(unexpected()),
            };
        }
    }
    last.ok_or_else(unexpected)
}

/// `_at` starting from an arbitrary item (an atom has no children).
pub fn at<'a>(n: &'a Item, idx: &[i64]) -> Result<&'a Item, OofError> {
    match n {
        Item::Node(x) => at_n(x, idx),
        Item::Atom(_) => Err(unexpected()),
    }
}

/// `_atom`: the value of an atom, `""` for a list.
pub fn atom(n: &Item) -> &str {
    match n {
        Item::Atom(a) => &a.value,
        Item::Node(_) => "",
    }
}

/// `{1,N,{"lang","text"},...}` -> items; `None` when `n` has another shape.
pub fn localized(n: &Item) -> Option<Vec<(String, String)>> {
    let Item::Node(n) = n else { return None };
    if n.len() < 2 || !is_bare(&n.children[0], "1") {
        return None;
    }
    let Item::Atom(cnt) = &n.children[1] else { return None };
    if cnt.kind != BARE || cnt.value.is_empty() || !cnt.value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let count: usize = cnt.value.parse().unwrap_or(usize::MAX);
    let mut items = Vec::new();
    for item in n.children.iter().skip(2).take(count) {
        let Item::Node(item) = item else { return None };
        if item.len() != 2 {
            return None;
        }
        match (&item.children[0], &item.children[1]) {
            (Item::Atom(a), Item::Atom(b)) if a.kind == QUOTED && b.kind == QUOTED => {
                items.push((a.value.clone(), b.value.clone()))
            }
            _ => return None,
        }
    }
    Some(items)
}

/// A localized string list as the JSON-able model value (`[[lang, text], ...]`).
pub fn localized_value(items: &[(String, String)]) -> Value {
    Value::Array(items.iter().map(|(l, t)| json!([l, t])).collect())
}

// ---------------------------------------------------------------- colors and fonts

/// 1C 8.2 standard style items referenced by GUID (`name`, `#RRGGBB`); a heuristic map.
fn style_color(guid: &str) -> Option<(&'static str, &'static str)> {
    match guid {
        "3cb25caa-4631-42d5-9227-674603f1dea0" => Some(("ЦветИформационныхСтрок", "#0000FF")),
        "addd5ba5-2652-411d-affb-a6f0a31fcbbc" => Some(("ЦветАвтора", "#993300")),
        "08084979-466f-49c4-9078-f71b102ba7c4" => Some(("ЦветФонаАвтора", "#F5F2DD")),
        _ => None,
    }
}

/// Style fonts that the Configurator draws bold.
fn is_bold_style_font(guid: &str) -> bool {
    matches!(guid, "5477b48d-c6e4-4ba3-a715-24bd7ad690bd" | "5e811f73-d81f-43f6-9f30-1be1a87d7795")
}

const FONT_WEIGHT_BOLD: &str = "700";
const FONT_MASK_WEIGHT: i64 = 4;

pub fn app_path(type_name: &str) -> Option<&'static [i64]> {
    Some(match type_name {
        "LabelDecoration" | "UsualGroup" | "Button" | "Panel" | "CommandBar" | "ChoiceField" => &[1, 0],
        "InputField" | "RadioButton" => &[2, 0, 0],
        "CheckBox" => &[1, 0, 0],
        "Table" | "TableBox" => &[2, 0],
        _ => return None,
    })
}

/// The common appearance block of a control; `None` for types without a known location.
fn app<'a>(type_name: &str, info: &'a Node) -> Result<Option<&'a Node>, OofError> {
    let Some(path) = app_path(type_name) else { return Ok(None) };
    let found = at_n(info, path)?;
    match found {
        Item::Node(n)
            if !n.is_empty()
                && matches!(atom(&n.children[0]), "10" | "11" | "12" | "13" | "14" | "15" | "16") =>
        {
            Ok(Some(n))
        }
        _ => Err(OofError::new("appearance block: unexpected version")),
    }
}

/// `{3,kind,{...}}` -> `{kind, value, name?, ref?}`; `None` for automatic and system colors.
pub fn decode_color(n: &Item) -> Result<Option<Map<String, Value>>, OofError> {
    let n = node(n, "color", 3)?;
    let kind = atom(&n.children[1]);
    let body = node(&n.children[2], "color", 1)?;
    let mut out = Map::new();
    if kind == "0" || kind == "2" {
        let rgb = int(&body.children[0], "rgb color")?;
        if rgb < 0 {
            return Ok(None);
        }
        out.insert("kind".into(), "rgb".into());
        out.insert("value".into(), format!("#{:02X}{:02X}{:02X}", rgb & 0xFF, (rgb >> 8) & 0xFF, (rgb >> 16) & 0xFF).into());
        return Ok(Some(out));
    }
    if kind == "3" && body.len() >= 2 && is_bare(&body.children[0], "0") && is_guid(&body.children[1]) {
        let r = atom(&body.children[1]);
        out.insert("kind".into(), "style".into());
        out.insert("ref".into(), r.into());
        if let Some((name, value)) = style_color(r) {
            out.insert("name".into(), name.into());
            out.insert("value".into(), value.into());
        }
        return Ok(Some(out));
    }
    Ok(None) // kind 4 = automatic, `{-N}` = system color
}

/// `{7,kind,mask,...}` -> `{bold?, source?, face?, size?}`; `None` when the font is the default one.
pub fn decode_font(n: &Item) -> Result<Option<Map<String, Value>>, OofError> {
    let n = node(n, "font", 3)?;
    let kind = atom(&n.children[1]);
    let mask = int(&n.children[2], "font mask")?;
    let mut font = Map::new();
    let tail = &n.children[3..];
    if mask & FONT_MASK_WEIGHT != 0 && tail.iter().any(|c| is_bare(c, FONT_WEIGHT_BOLD)) {
        font.insert("bold".into(), true.into());
        font.insert("source".into(), "bin".into());
    } else if kind == "2" && n.len() > 3 {
        if let Item::Node(r) = &n.children[3] {
            if r.len() >= 2 && is_bare(&r.children[0], "0") && is_bold_style_font(atom(&r.children[1])) {
                font.insert("bold".into(), true.into());
                font.insert("source".into(), "style".into());
            }
        }
    }
    let face = tail.iter().find_map(|c| match c {
        Item::Atom(a) if a.kind == QUOTED && !a.value.is_empty() => Some(a.value.clone()),
        _ => None,
    });
    if let Some(face) = face {
        font.insert("face".into(), face.into());
    }
    if kind == "1" {
        // an explicit font stores its logical height as `{-h}`
        for c in tail {
            if let Item::Node(cn) = c {
                if cn.len() == 1 && matches!(cn.children[0], Item::Atom(_)) {
                    let h = num(&cn.children[0])?;
                    if h < 0.0 {
                        font.insert("size".into(), py_round(h.abs() * 8.0 / 16.0).into());
                        break;
                    }
                }
            }
        }
        // `{7,1,mask,[face,]{h},weight,italic,underline,strikeout,auto,scale}`: the flags follow the `{h}` node
        if let Some(h_pos) = n.children[3..].iter().position(|c| matches!(c, Item::Node(_))) {
            let after_h = &n.children[3 + h_pos + 1..];
            if after_h.len() >= 4 {
                for (bit, idx, key) in [(8, 1, "italic"), (16, 2, "underline"), (32, 3, "strikeout")] {
                    if mask & bit != 0 && is_bare(&after_h[idx], "1") {
                        font.insert(key.into(), true.into());
                        font.insert("source".into(), "bin".into());
                    }
                }
            }
        }
    }
    if n.len() >= 5 && is_bare(&n.children[0], "7") {
        if let Some(Item::Atom(a)) = n.children.last() {
            if a.kind == BARE {
                if let Some(s) = py_int(&a.value).filter(|s| *s != 100 && *s > 0) {
                    font.insert("scale".into(), s.into());
                }
            }
        }
    }
    Ok(if font.is_empty() { None } else { Some(font) })
}

// ---------------------------------------------------------------- value types

/// `{"Pattern",{type}...}` -> `{kind, composite, ...qualifiers}`; `None` for an empty pattern.
///
/// Qualifiers: `digits` (non-default `Number` precision), `fractionDigits`, `nonNegative`, `length`, `fixed`;
/// `refGuid`/`refGuids` for concrete references (`;`-joined for a composite of references only);
/// `mixedComposite` for a composite that is not a plain list of concrete references.
pub fn decode_pattern(pattern: &Item) -> Result<Option<Map<String, Value>>, OofError> {
    let pattern = node(pattern, "type pattern", 1)?;
    if str_val(&pattern.children[0], "type pattern")? != "Pattern" {
        return Err(OofError::new("type pattern: unexpected structure"));
    }
    let branches = &pattern.children[1..];
    if branches.is_empty() {
        return Ok(None);
    }
    let first = node(&branches[0], "type", 1)?;
    let code = str_val(&first.children[0], "type code")?;
    let mut vt = Map::new();
    if code == "D" {
        let sub = if first.len() > 1 { Some(str_val(&first.children[1], "date part")?) } else { None };
        let kind = match sub {
            Some("D") => "Date",
            Some("T") => "Time",
            _ => "DateTime",
        };
        vt.insert("kind".into(), kind.into());
    } else if code == "N" {
        vt.insert("kind".into(), "Number".into());
        if first.len() > 1 {
            let digits = int(&first.children[1], "number digits")?;
            if digits != 10 && digits > 0 {
                vt.insert("digits".into(), digits.into());
            }
        }
        if first.len() > 2 {
            vt.insert("fractionDigits".into(), int(&first.children[2], "fraction digits")?.into());
        }
        if first.len() > 3 && int(&first.children[3], "number sign")? != 0 {
            vt.insert("nonNegative".into(), true.into());
        }
    } else if code == "#" {
        let ref_guid = if first.len() > 1 { atom(&first.children[1]).to_lowercase() } else { String::new() };
        let kind = match ref_guid.as_str() {
            VALUE_TABLE_GUID => "ValueTable",
            VALUE_TREE_GUID => "ValueTree",
            VALUE_LIST_GUID => "ValueList",
            _ => "Ref",
        };
        vt.insert("kind".into(), kind.into());
    } else {
        let kind = match code {
            "S" => {
                if first.len() > 1 {
                    let length = int(&first.children[1], "string length")?;
                    if length > 0 {
                        vt.insert("length".into(), length.into());
                    }
                }
                if first.len() > 2 && int(&first.children[2], "string fixed")? != 0 {
                    vt.insert("fixed".into(), true.into());
                }
                "String"
            }
            "B" => "Boolean",
            _ => "Unknown",
        };
        vt.insert("kind".into(), kind.into());
    }
    let is_any_ref = code == "#" && first.len() > 1 && atom(&first.children[1]).eq_ignore_ascii_case(ANY_REF_GUID);
    vt.insert("composite".into(), (branches.len() > 1 || is_any_ref).into());

    // Concrete references: `{"#", <uuid>}` branches with a uuid that is not a platform built-in type.
    let concrete: Vec<Option<String>> = branches.iter().map(concrete_ref_guid).collect();
    if concrete.iter().all(Option::is_some) {
        let guids: Vec<String> = concrete.into_iter().flatten().collect();
        if guids.len() == 1 {
            vt.insert("refGuid".into(), guids[0].clone().into());
        } else {
            vt.insert("refGuid".into(), guids.join(";").into());
            vt.insert("refGuids".into(), Value::Array(guids.into_iter().map(Value::String).collect()));
        }
    } else if branches.len() > 1 {
        vt.insert("mixedComposite".into(), true.into());
    }
    // A composite that `kind`/`refGuid` cannot describe is listed branch by branch (lossless raw ids and qualifiers).
    if vt.get("mixedComposite") == Some(&Value::Bool(true)) {
        let mut types = Vec::new();
        for b in branches {
            types.push(Value::Object(decode_branch(b)?));
        }
        for key in ["refGuid", "refGuids", "digits", "fractionDigits", "length", "nonNegative", "fixed"] {
            vt.remove(key);
        }
        vt.insert("types".into(), Value::Array(types));
    }
    Ok(Some(vt))
}

/// One `{type}` branch of a pattern as `{kind, qualifiers, refGuid}` (`refGuid` is the raw `#` uuid, built-ins too).
fn decode_branch(branch: &Item) -> Result<Map<String, Value>, OofError> {
    let b = node(branch, "type branch", 1)?;
    let code = str_val(&b.children[0], "type code")?;
    let mut t = Map::new();
    match code {
        "#" => {
            let g = if b.len() > 1 { atom(&b.children[1]).to_lowercase() } else { String::new() };
            let kind = match g.as_str() {
                VALUE_TABLE_GUID => "ValueTable",
                VALUE_TREE_GUID => "ValueTree",
                VALUE_LIST_GUID => "ValueList",
                ANY_REF_GUID => "AnyRef",
                _ => "Ref",
            };
            t.insert("kind".into(), kind.into());
            t.insert("refGuid".into(), g.into());
        }
        "S" => {
            t.insert("kind".into(), "String".into());
            if b.len() > 1 {
                let length = int(&b.children[1], "string length")?;
                if length > 0 {
                    t.insert("length".into(), length.into());
                }
            }
            if b.len() > 2 && int(&b.children[2], "string fixed")? != 0 {
                t.insert("fixed".into(), true.into());
            }
        }
        "N" => {
            t.insert("kind".into(), "Number".into());
            if b.len() > 1 {
                t.insert("digits".into(), int(&b.children[1], "number digits")?.into());
            }
            if b.len() > 2 {
                t.insert("fractionDigits".into(), int(&b.children[2], "fraction digits")?.into());
            }
            if b.len() > 3 && int(&b.children[3], "number sign")? != 0 {
                t.insert("nonNegative".into(), true.into());
            }
        }
        "D" => {
            let kind = match if b.len() > 1 { Some(str_val(&b.children[1], "date part")?) } else { None } {
                Some("D") => "Date",
                Some("T") => "Time",
                _ => "DateTime",
            };
            t.insert("kind".into(), kind.into());
        }
        "B" => {
            t.insert("kind".into(), "Boolean".into());
        }
        _ => {
            t.insert("kind".into(), "Unknown".into());
            t.insert("code".into(), code.into());
        }
    }
    Ok(t)
}

/// Lowercase uuid of a `{"#", <uuid>}` branch that is a concrete (not platform built-in) reference type.
fn concrete_ref_guid(branch: &Item) -> Option<String> {
    let b = branch.as_node()?;
    if b.len() < 2 || str_val(&b.children[0], "branch code").ok()? != "#" {
        return None;
    }
    let g = atom(&b.children[1]).to_lowercase();
    (is_uuid(&g) && !matches!(g.as_str(), VALUE_TABLE_GUID | VALUE_TREE_GUID | VALUE_LIST_GUID | ANY_REF_GUID)).then_some(g)
}

// ---------------------------------------------------------------- per-control decoding

fn border_style(code: &str) -> Option<&'static str> {
    match code {
        "6" => Some("Frame"),
        "4" => Some("SingleLine"),
        "0" => Some("None"),
        _ => None,
    }
}

fn button_kind(code: &str) -> &'static str {
    match code {
        "1" => "Submenu",
        "2" => "Separator",
        _ => "Action",
    }
}

fn representation(code: &str) -> &'static str {
    match code {
        "1" => "Text",
        "2" => "Picture",
        "3" => "PictureAndText",
        _ => "Auto",
    }
}

pub(crate) fn std_element(code: i64) -> Option<&'static str> {
    Some(match code {
        -2 => "Номер",
        -3 => "Дата",
        -4 => "ПометкаУдаления",
        -5 => "Проведен",
        -6 => "Родитель",
        -7 => "Владелец",
        -9 => "Наименование",
        _ => return None,
    })
}

/// Path of the localized caption inside the info node for types with a known location.
pub fn caption_path(type_name: Option<&str>) -> Option<&'static [i64]> {
    match type_name.unwrap_or("") {
        "LabelDecoration" | "UsualGroup" => Some(&[1, 2]),
        "CheckBox" => Some(&[1, 0, 2]),
        "RadioButton" => Some(&[2, 0, 2]),
        _ => None,
    }
}

/// The caption of types with a known location (empty when blank); `None` to let the caller guess.
pub fn control_caption(type_name: Option<&str>, info: &Node) -> Result<Option<Vec<(String, String)>>, OofError> {
    let Some(path) = caption_path(type_name) else { return Ok(None) };
    Ok(Some(localized(at_n(info, path)?).unwrap_or_default()))
}

fn appearance(props: &mut Map<String, Value>, app: &Node) -> Result<(), OofError> {
    if is_bare(at_n(app, &[1])?, "0") {
        props.insert("enabled".into(), false.into());
    }
    for (key, idx) in [("backColor", 2), ("textColor", 3), ("borderColor", 6)] {
        if let Some(color) = decode_color(at_n(app, &[idx])?)? {
            props.insert(key.into(), Value::Object(color));
        }
    }
    if let Some(font) = decode_font(at_n(app, &[4])?)? {
        props.insert("font".into(), Value::Object(font));
    }
    if is_bare(at_n(app, &[5])?, "1") {
        props.insert("transparentBack".into(), true.into());
    }
    let tip = if app.len() > 12 { localized(&app.children[12]) } else { None };
    if let Some(tip) = tip {
        if !tip.is_empty() {
            props.insert("tooltip".into(), localized_value(&tip));
        }
    }
    Ok(())
}

/// `{8,?,{4,1,{0,guid}},...}` with a non-zero guid.
fn has_picture(pic: &Item) -> bool {
    let Item::Node(pic) = pic else { return false };
    for c in &pic.children {
        if let Item::Node(c) = c {
            if c.len() >= 3 && is_bare(&c.children[0], "4") {
                if let Item::Node(r) = &c.children[2] {
                    if r.len() >= 2 && is_guid(&r.children[1]) && atom(&r.children[1]) != ZERO_GUID {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn label(props: &mut Map<String, Value>, info: &Node) -> Result<(), OofError> {
    let p = node(at_n(info, &[1])?, "label", 13)?;
    match atom(&p.children[3]) {
        "1" => {
            props.insert("horizontalAlign".into(), "Center".into());
        }
        "2" => {
            props.insert("horizontalAlign".into(), "Right".into());
        }
        _ => {}
    }
    if is_bare(&p.children[5], "1") {
        props.insert("hyperlink".into(), true.into());
    }
    if has_picture(&p.children[12]) {
        props.insert("picture".into(), true.into());
    }
    Ok(())
}

fn group(props: &mut Map<String, Value>, info: &Node) -> Result<(), OofError> {
    let code = atom(at_n(info, &[1, 3, 3])?);
    if let Some(name) = border_style(code) {
        let n: i64 = code.parse().unwrap_or(0);
        props.insert("borderStyle".into(), json!({"code": n, "name": name}));
    }
    Ok(())
}

fn input_field(props: &mut Map<String, Value>, info: &Node) -> Result<(), OofError> {
    if let Some(vt) = decode_pattern(at_n(info, &[1])?)? {
        props.insert("valueType".into(), Value::Object(vt));
    }
    let q = node(at_n(info, &[2, 0])?, "input field", 37)?;
    for (idx, key) in [(2usize, "horizontalScrollBar"), (3usize, "verticalScrollBar")] {
        match atom(&q.children[idx]) {
            "1" => {
                props.insert(key.into(), "Use".into());
            }
            "2" => {
                props.insert(key.into(), "Auto".into());
            }
            _ => {}
        }
    }
    props.insert("choiceButton".into(), (int(&q.children[7], "choice button")? != 0).into());
    props.insert("clearButton".into(), (int(&q.children[8], "clear button")? != 0).into());
    props.insert("openButton".into(), (int(&q.children[10], "open button")? != 0).into());
    if is_bare(&q.children[13], "1") {
        props.insert("readOnly".into(), true.into());
    }
    if is_bare(&q.children[27], "1") {
        props.insert("markNegatives".into(), true.into());
    }
    if is_bare(&q.children[35], "1") {
        props.insert("autoMarkIncomplete".into(), true.into());
    }
    if info.len() > 9 && is_bare(&info.children[9], "1") {
        props.insert("choiceListMode".into(), true.into());
    }
    Ok(())
}

fn check_box(props: &mut Map<String, Value>, info: &Node) -> Result<(), OofError> {
    let p = node(at_n(info, &[1])?, "checkbox", 3)?;
    let sub = node(at_n(p, &[0])?, "checkbox sub", 4)?;
    if is_bare(&sub.children[3], "0") {
        props.insert("titleLocation".into(), "Left".into());
    }
    if is_bare(&p.children[2], "1") {
        props.insert("threeState".into(), true.into());
    }
    Ok(())
}

#[derive(Default, Clone)]
struct ActionInfo {
    action: String,
    action_kind: &'static str,
    group: Option<(String, String)>,
    tooltip: Vec<(String, String)>,
    has_picture: bool,
}

/// `{7|6,guid,1,kindGuid,body,mask,...}` -> (guid, ActionInfo).
fn action_info(a: &Item) -> Result<(String, ActionInfo), OofError> {
    let a = node(a, "action", 5)?;
    let guid = atom(&a.children[1]).to_string();
    if !is_guid(&a.children[1]) {
        return Err(OofError::new("action: guid expected"));
    }
    let kind = atom(&a.children[3]);
    let mut info = ActionInfo { has_picture: action_has_picture(Some(a)), ..Default::default() };
    if kind == EVENT_GUID {
        info.action = str_val(at(&a.children[4], &[1])?, "action handler")?.to_string();
        info.action_kind = "Handler";
        if let Item::Node(body) = &a.children[4] {
            if let Some(Item::Node(meta)) = body.children.get(2) {
                if let Some(tip_node) = meta.children.get(3) {
                    info.tooltip = localized(tip_node).unwrap_or_default();
                }
            }
        }
    } else if kind == STANDARD_ACTION_GUID {
        info.action = format!("Standard:{}", int(at(&a.children[4], &[3])?, "standard action")?);
        info.action_kind = "Standard";
    } else if kind == SUBMENU_ACTION_GUID || kind == STD_SUBMENU_ACTION_GUID {
        if let Item::Node(body) = &a.children[4] {
            if body.len() >= 3 {
                let (gg, gc) = (atom(&body.children[1]), atom(&body.children[2]));
                if gg != NULL_GROUP_GUID || gc != "0" {
                    info.group = Some((gg.to_string(), gc.to_string()));
                    info.action_kind = if kind == STD_SUBMENU_ACTION_GUID { "StdSubmenu" } else { "Submenu" };
                }
            }
        }
    } else if kind == OPEN_MOVEMENTS_GUID {
        info.action_kind = "OpenMovements";
    }
    if let Some(Item::Atom(ma)) = a.children.get(5) {
        if ma.kind == BARE {
            let mask: i64 = ma.value.parse().unwrap_or(0);
            let mut idx = 6usize;
            if mask & 2 != 0 {
                idx += 1;
            }
            if mask & 4 != 0 {
                if let Some(tip_item) = a.children.get(idx) {
                    if let Some(loc_tip) = localized(tip_item) {
                        if !loc_tip.is_empty() {
                            info.tooltip = loc_tip;
                        }
                    }
                }
            }
        }
    }
    Ok((guid, info))
}

/// An action `{7|6,...}` carries a picture node `{4,k,...}` with `k != 0`.
fn action_has_picture(a: Option<&Node>) -> bool {
    let Some(a) = a else { return false };
    a.children.iter().skip(5).any(|c| match c {
        Item::Node(c) => c.len() >= 2 && is_bare(&c.children[0], "4") && atom(&c.children[1]) != "0",
        Item::Atom(_) => false,
    })
}

/// The `X[7]` tree `{5,src,n,flag,count,actions...,groupCount,groups...}` -> root group buttons
/// with recursively resolved `buttons` for submenus.
fn buttons(table: &Item, root: Option<(&str, &str)>) -> Result<Vec<Value>, OofError> {
    let t = node(table, "command bar items", 5)?;
    let count = int(&t.children[4], "action count")?;
    let action_items = py_slice(&t.children, 5, 5 + count);
    let mut actions: HashMap<String, ActionInfo> = HashMap::new();
    for a in action_items {
        let (g, d) = action_info(a)?;
        actions.insert(g, d);
    }
    let group_count = int(at_i(t, 5 + count)?, "group count")?;
    let mut all_groups: Vec<&Node> = Vec::new();
    for g in py_slice(&t.children, 6 + count, 6 + count + group_count) {
        all_groups.push(node(g, "button group", 5)?);
    }
    let mut by_key: HashMap<(String, String), &Node> = HashMap::new();
    for g in &all_groups {
        by_key.insert((atom(&g.children[1]).to_string(), atom(&g.children[2]).to_string()), *g);
    }
    fn last<'a>(groups: &[&'a Node]) -> Vec<&'a Node> {
        groups.last().map(|g| vec![*g]).unwrap_or_default()
    }
    let root_groups = match root {
        Some((src, code)) => {
            let matched: Vec<&Node> =
                all_groups.iter().copied().filter(|g| atom(&g.children[1]) == src && atom(&g.children[2]) == code).collect();
            if matched.is_empty() {
                last(&all_groups)
            } else {
                matched
            }
        }
        None => last(&all_groups),
    };

    fn build_group(
        g: &Node,
        actions: &HashMap<String, ActionInfo>,
        by_key: &HashMap<(String, String), &Node>,
        visited: &mut HashSet<(String, String)>,
    ) -> Result<Vec<Value>, OofError> {
        let mut items = Vec::new();
        let n = int(&g.children[4], "button count")?;
        for i in 0..n.max(0) {
            let guid = atom(at_n(g, &[5 + 2 * i])?);
            let b = node(at_n(g, &[6 + 2 * i])?, "button", 13)?;
            if !matches!(atom(&b.children[0]), "7" | "8") {
                return Err(OofError::new("button: unexpected structure"));
            }
            let ainfo = actions.get(guid).cloned().unwrap_or_default();
            let kind = button_kind(atom(&b.children[9]));
            let caption = localized(&b.children[4]).unwrap_or_default();
            let mut btn = Map::new();
            btn.insert("name".into(), str_val(&b.children[1], "button name")?.into());
            btn.insert("id".into(), int(&b.children[7], "button id")?.into());
            btn.insert("kind".into(), kind.into());
            btn.insert("checked".into(), is_bare(&b.children[12], "1").into());
            btn.insert("action".into(), ainfo.action.into());
            btn.insert("actionKind".into(), ainfo.action_kind.into());
            btn.insert("caption".into(), localized_value(&caption));
            btn.insert("tooltip".into(), localized_value(&ainfo.tooltip));
            btn.insert("representation".into(), representation(atom(&b.children[10])).into());
            btn.insert("hasPicture".into(), ainfo.has_picture.into());
            if kind == "Submenu" {
                if let Some(ref sub_key) = ainfo.group {
                    if !visited.contains(sub_key) {
                        if let Some(&sub_g) = by_key.get(sub_key) {
                            visited.insert(sub_key.clone());
                            let sub_items = build_group(sub_g, actions, by_key, visited)?;
                            visited.remove(sub_key);
                            if !sub_items.is_empty() {
                                btn.insert("buttons".into(), Value::Array(sub_items));
                            }
                        }
                    }
                }
            }
            items.push(Value::Object(btn));
        }
        Ok(items)
    }

    let mut out = Vec::new();
    for g in root_groups {
        let mut visited = HashSet::new();
        visited.insert((atom(&g.children[1]).to_string(), atom(&g.children[2]).to_string()));
        out.extend(build_group(g, &actions, &by_key, &mut visited)?);
    }
    Ok(out)
}

/// `_at(t, i)` with a computed (possibly negative) index.
fn at_i(t: &Node, i: i64) -> Result<&Item, OofError> {
    at_n(t, &[i])
}

/// `info[2][1][1]` is a bit mask: 1 = header (`Шапка`), 2 = footer (`Подвал`).
fn table(props: &mut Map<String, Value>, info: &Node) -> Result<(), OofError> {
    let t = node(at_n(info, &[2, 1])?, "table", 16)?;
    let flags = int(&t.children[1], "table flags")?;
    props.insert("footer".into(), (flags & 2 != 0).into());
    if is_bare(&t.children[15], "1") {
        props.insert("selectionMode".into(), "Multi".into());
    }
    // Row-source descriptor `info[3]`: the data handler of the table (value table, tabular section, dynamic list of
    // a catalog/document/register with its hierarchy and filter links). It has no smaller XML form yet, so it is
    // carried verbatim unless it is the plain value-table default.
    if let Some(Item::Node(d)) = info.children.get(3) {
        let mut raw = Vec::new();
        crate::donor::write_node(d, &mut raw);
        let raw = String::from_utf8_lossy(&raw).into_owned();
        if raw != "{342cf854-134c-42bb-8af9-a2103d5d9723,{5,0,0,1}}" {
            props.insert("rowSource".into(), raw.into());
        }
    }
    if t.len() > 21 {
        match atom(&t.children[21]) {
            "1" => {
                props.insert("initialTreeView".into(), "ExpandTopLevel".into());
            }
            "2" => {
                props.insert("initialTreeView".into(), "ExpandAllLevels".into());
            }
            _ => {}
        }
    }
    Ok(())
}

/// Signature of a button list as written to `Form.xml` (FNV-1a 64 of a canonical text): ids, names, kinds,
/// actions, captions and tooltips, recursively. A compiler recomputes it from the XML to detect edits.
pub fn buttons_sig(items: &[Value]) -> String {
    fn text(items: &[Value], out: &mut String) {
        let s = |v: &Value| match v {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        for b in items {
            let repr = b.get("representation").map(s).unwrap_or_default();
            let repr = if repr.is_empty() { "Auto".to_string() } else { repr };
            out.push_str(&format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|",
                b.get("id").map(s).unwrap_or_default(),
                b.get("name").map(s).unwrap_or_default(),
                b.get("kind").map(s).unwrap_or_default(),
                b.get("checked").map(s).unwrap_or_default().to_lowercase(),
                repr,
                b.get("hasPicture").and_then(Value::as_bool).unwrap_or(false),
                b.get("actionKind").map(s).unwrap_or_default(),
                b.get("action").map(s).unwrap_or_default(),
            ));
            for (tag, key) in [("c", "caption"), ("t", "tooltip")] {
                for it in b.get(key).and_then(Value::as_array).into_iter().flatten() {
                    out.push_str(&format!("{tag}:{}={};", s(&it[0]), s(&it[1])));
                }
            }
            if let Some(sub) = b.get("buttons").and_then(Value::as_array) {
                out.push('[');
                text(sub, out);
                out.push(']');
            }
            out.push('\n');
        }
    }
    let mut t = String::new();
    text(items, &mut t);
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in t.as_bytes() {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn command_bar(props: &mut Map<String, Value>, info: &Node, warn: &mut dyn FnMut(&str)) -> Result<(), OofError> {
    let x = node(at_n(info, &[1])?, "command bar", 5)?;
    let code = atom(&x.children[3]);
    if code == "0" || code == "1" {
        props.insert("autoFill".into(), (code == "1").into());
    }
    if is_bare(&x.children[4], "2") {
        props.insert("horizontalAlign".into(), "Right".into());
    }
    if x.len() > 7 {
        let root = if x.len() > 9 { Some((atom(&x.children[8]), atom(&x.children[9]))) } else { None };
        match buttons(&x.children[7], root) {
            Err(_) => warn("Buttons"),
            Ok(b) => {
                // The action/group tree carries platform bookkeeping (standard command ids, group codes, source
                // ranges) that the button list cannot express: keep it verbatim, with a signature of the button
                // list so a compiler can tell whether the list was edited since.
                if let Item::Node(tree) = &x.children[7] {
                    let mut raw = Vec::new();
                    crate::donor::write_node(tree, &mut raw);
                    props.insert("buttonsRaw".into(), String::from_utf8_lossy(&raw).into_owned().into());
                    props.insert("buttonsSig".into(), buttons_sig(&b).into());
                }
                if !b.is_empty() {
                    props.insert("buttons".into(), Value::Array(b));
                }
            }
        }
    }
    Ok(())
}

fn choice_field(props: &mut Map<String, Value>, info: &Node) -> Result<(), OofError> {
    let p = node(at_n(info, &[1])?, "choice field", 27)?;
    if let Item::Node(app) = &p.children[0] {
        if app.len() > 12 {
            if let Some(tip) = localized(&app.children[12]) {
                if !tip.is_empty() {
                    props.insert("tooltip".into(), localized_value(&tip));
                }
            }
        }
    }
    let Item::Node(vl) = &p.children[26] else { return Ok(()) };
    if vl.len() < 3 || !matches!(atom(&vl.children[0]), "8" | "9") {
        return Ok(());
    }
    let Item::Node(dn) = &vl.children[2] else { return Ok(()) };
    if dn.len() <= 6 {
        return Ok(());
    }
    let Item::Node(inod) = &dn.children[6] else { return Ok(()) };
    if inod.len() < 2 || !is_bare(&inod.children[0], "1") {
        return Ok(());
    }
    let Item::Atom(cnt_atom) = &inod.children[1] else { return Ok(()) };
    if !cnt_atom.value.bytes().all(|b| b.is_ascii_digit()) || cnt_atom.value.is_empty() {
        return Ok(());
    }
    let cnt: usize = cnt_atom.value.parse().unwrap_or(0);
    let mut items = Vec::new();
    for it in inod.children.iter().skip(2).take(cnt) {
        let Item::Node(it) = it else { continue };
        if it.len() < 5 {
            continue;
        }
        let val = match &it.children[3] {
            Item::Node(vn) if vn.len() > 1 => atom(&vn.children[1]).to_string(),
            _ => String::new(),
        };
        let mut pres: Vec<(String, String)> = Vec::new();
        if let Item::Node(pn_wrap) = &it.children[4] {
            if pn_wrap.len() >= 3 {
                if let Item::Node(pn) = &pn_wrap.children[2] {
                    if let Some(Item::Atom(la)) = pn.children.first() {
                        if !la.value.is_empty() && la.value.bytes().all(|b| b.is_ascii_digit()) {
                            let lcnt: usize = la.value.parse().unwrap_or(0);
                            for j in 0..lcnt {
                                if 2 + 2 * j < pn.len() {
                                    if let (Item::Atom(l), Item::Atom(t)) = (&pn.children[1 + 2 * j], &pn.children[2 + 2 * j]) {
                                        if l.kind == QUOTED && t.kind == QUOTED {
                                            pres.push((l.value.clone(), t.value.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        items.push(json!({"value": val, "presentation": localized_value(&pres)}));
    }
    if !items.is_empty() {
        props.insert("choiceList".into(), Value::Array(items));
    }
    Ok(())
}

/// Decoded properties of one control; `warn(property)` is called for every unreadable group.
pub fn decode_props(type_name: &str, info: &Node, warn: &mut dyn FnMut(&str)) -> Map<String, Value> {
    let mut props = Map::new();
    let appearance_block = match app(type_name, info) {
        Ok(a) => a,
        Err(_) => {
            warn("Appearance");
            None
        }
    };
    if let Some(a) = appearance_block {
        if appearance(&mut props, a).is_err() {
            warn("Appearance");
        }
        // The block also holds system colors, font details and other slots `Form.xml` does not express: keep it
        // verbatim for a compiler (see `appearance_sig` in oof.rs for the staleness check).
        let mut raw = Vec::new();
        crate::donor::write_node(a, &mut raw);
        props.insert("appearanceRaw".into(), String::from_utf8_lossy(&raw).into_owned().into());
    }
    let (result, group_name) = match type_name {
        "LabelDecoration" => (label(&mut props, info), "LabelDecoration"),
        "UsualGroup" => (group(&mut props, info), "BorderStyle"),
        "InputField" => (input_field(&mut props, info), "InputField"),
        "CheckBox" => (check_box(&mut props, info), "CheckBox"),
        "ChoiceField" => (choice_field(&mut props, info), "ChoiceField"),
        "Table" => (table(&mut props, info), "Table"),
        "CommandBar" => (command_bar(&mut props, info, warn), "CommandBar"),
        _ => (Ok(()), ""),
    };
    if result.is_err() {
        warn(group_name);
    }
    props
}

/// Extra properties of a table column `props` list: id, width, visibility, tooltip, value type, read-only.
pub fn decode_column_props(props: &Node) -> Result<Map<String, Value>, OofError> {
    let mut out = Map::new();
    out.insert("width".into(), py_round(num(at_n(props, &[4])?)?).into());
    if let Some(Item::Atom(a)) = props.children.get(5) {
        if a.kind == BARE {
            if let Some(cid) = py_int(&a.value) {
                out.insert("id".into(), cid.into());
            }
        }
    }
    let footer_text = if props.len() > 2 { localized(&props.children[2]) } else { None };
    if let Some(ft) = footer_text {
        if !ft.is_empty() {
            out.insert("footerText".into(), localized_value(&ft));
        }
    }
    let tooltip = if props.len() > 3 { localized(&props.children[3]) } else { None };
    if let Some(tip) = tooltip {
        if !tip.is_empty() {
            out.insert("tooltip".into(), localized_value(&tip));
        }
    }
    if is_bare(at_n(props, &[25])?, "0") {
        out.insert("visible".into(), false.into());
    }
    if let Some(vt) = decode_pattern(at_n(props, &[35])?)? {
        out.insert("valueType".into(), Value::Object(vt));
    }
    let mut editor = at_n(props, &[38])?;
    if let Item::Node(e) = editor {
        if e.len() >= 2 {
            editor = &e.children[1];
        }
    }
    if atom(editor) == ZERO_GUID || is_bare(at_n(props, &[26])?, "1") {
        out.insert("readOnly".into(), true.into());
    }
    Ok(out)
}

pub fn resolve_column_step(
    step: Option<&Item>,
    is_tabular: bool,
    meta_lookup: Option<&HashMap<String, String>>,
) -> Option<String> {
    let Some(Item::Node(step)) = step else { return None };
    let first = atom(step.children.first()?);
    if first == "-2" && is_tabular {
        return Some("НомерСтроки".to_string());
    }
    let b = first.as_bytes();
    if b.len() >= 2 && b[0] == b'-' && b[1..].iter().all(|c| c.is_ascii_digit()) {
        return first.parse::<i64>().ok().and_then(std_element).map(str::to_string);
    }
    if first == "0" && step.len() > 1 {
        return meta_lookup.and_then(|m| m.get(&atom(&step.children[1]).to_lowercase()).cloned());
    }
    None
}

// ---------------------------------------------------------------- data paths

/// `{attributeId, path?}` of a link `{2,{attrId},{elem}}`; `path` is `Attribute[.Element]`.
pub fn resolve_data_path(
    attr_id: i64,
    step: Option<&Item>,
    names: &HashMap<i64, String>,
    meta_lookup: Option<&HashMap<String, String>>,
) -> Value {
    let mut out = Map::new();
    out.insert("attributeId".into(), attr_id.into());
    let Some(root) = names.get(&attr_id) else { return Value::Object(out) };
    out.insert("path".into(), root.clone().into());
    if let Some(Item::Node(step)) = step {
        if let Some(first) = step.children.first() {
            let first = atom(first);
            let mut name: Option<String> = None;
            let b = first.as_bytes();
            if b.len() >= 2 && b[0] == b'-' && b[1..].iter().all(|c| c.is_ascii_digit()) {
                name = first.parse::<i64>().ok().and_then(std_element).map(str::to_string);
            } else if first == "0" && step.len() > 1 {
                name = meta_lookup.and_then(|m| m.get(&atom(&step.children[1]).to_lowercase()).cloned());
            }
            if let Some(name) = name {
                if !name.is_empty() {
                    out.insert("path".into(), format!("{root}.{name}").into());
                }
            }
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brace::{parse_metadata, Atom};

    fn item(s: &str) -> Item {
        Item::Node(parse_metadata(s.as_bytes()).unwrap())
    }

    // port: tests/test_oof.py::test_num_parses_scientific_notation_and_rejects_text
    #[test]
    fn num_parses_scientific_notation_and_rejects_text() {
        for (text, value) in [
            ("1e2", 100.0),
            ("1.5e2", 150.0),
            ("8e1", 80.0),
            ("33.00000000000001", 33.00000000000001),
            ("75", 75.0),
            ("-20", -20.0),
        ] {
            assert_eq!(num(&Item::Atom(Atom::new(BARE, text))).unwrap(), value);
        }
        for bad in [Atom::new(QUOTED, "1"), Atom::new(BARE, "abc"), Atom::new(BARE, "nan"), Atom::new(BARE, "")] {
            assert!(num(&Item::Atom(bad)).is_err());
        }
    }

    // port: tests/test_oof.py::test_decode_color_and_font
    #[test]
    fn decode_color_and_font_cases() {
        let color = |t: &str| decode_color(&item(t)).unwrap().map(Value::Object);
        assert_eq!(color("{3,4,{0}}"), None);
        assert_eq!(color("{3,3,{-22}}"), None);
        assert_eq!(color("{3,0,{255}}"), Some(json!({"kind": "rgb", "value": "#FF0000"})));
        assert_eq!(color("{3,2,{16711680}}"), Some(json!({"kind": "rgb", "value": "#0000FF"})));
        assert_eq!(color("{3,3,{0,addd5ba5-2652-411d-affb-a6f0a31fcbbc}}").unwrap()["value"], "#993300");
        assert_eq!(
            color("{3,3,{0,11111111-2222-3333-4444-555555555555}}"),
            Some(json!({"kind": "style", "ref": "11111111-2222-3333-4444-555555555555"}))
        );

        let font = |t: &str| decode_font(&item(t)).unwrap().map(Value::Object);
        assert_eq!(font("{7,3,0,1,100}"), None);
        assert_eq!(font("{7,1,60,{0},700,0,0,0,0,100}"), Some(json!({"bold": true, "source": "bin"})));
        assert_eq!(
            font("{7,2,0,{0,5477b48d-c6e4-4ba3-a715-24bd7ad690bd},1,100}"),
            Some(json!({"bold": true, "source": "style"}))
        );
        assert_eq!(font("{7,1,0,\"Arial\",{-20},0,100}"), Some(json!({"face": "Arial", "size": 10})));
    }

    // port: tests/test_oof.py::test_decode_pattern_kinds
    #[test]
    fn decode_pattern_kinds() {
        let vt = |t: &str| decode_pattern(&item(t)).unwrap().map(Value::Object);
        assert_eq!(vt("{\"Pattern\",{\"D\"}}"), Some(json!({"kind": "DateTime", "composite": false})));
        assert_eq!(vt("{\"Pattern\",{\"D\",\"D\"}}").unwrap()["kind"], "Date");
        assert_eq!(vt("{\"Pattern\",{\"D\",\"T\"}}").unwrap()["kind"], "Time");
        assert_eq!(
            vt("{\"Pattern\",{\"N\",15,3,0}}"),
            Some(json!({"kind": "Number", "digits": 15, "fractionDigits": 3, "composite": false}))
        );
        assert_eq!(vt("{\"Pattern\",{\"S\",10,1}}").unwrap()["kind"], "String");
        assert_eq!(vt("{\"Pattern\",{\"S\",10,1}}").unwrap()["length"], 10);
        assert_eq!(vt("{\"Pattern\",{\"B\"}}").unwrap()["kind"], "Boolean");
        assert_eq!(
            vt("{\"Pattern\",{\"#\",0b635ac9-cea4-4531-88db-24fe88ba0690},{\"S\"}}"),
            Some(json!({"kind": "Ref", "composite": true, "mixedComposite": true, "types": [
                {"kind": "Ref", "refGuid": "0b635ac9-cea4-4531-88db-24fe88ba0690"},
                {"kind": "String"}
            ]}))
        );
        assert_eq!(
            vt("{\"Pattern\",{\"#\",0b635ac9-cea4-4531-88db-24fe88ba0690},{\"#\",1b635ac9-cea4-4531-88db-24fe88ba0690}}"),
            Some(json!({"kind": "Ref", "composite": true, "refGuid": "0b635ac9-cea4-4531-88db-24fe88ba0690;1b635ac9-cea4-4531-88db-24fe88ba0690", "refGuids": ["0b635ac9-cea4-4531-88db-24fe88ba0690", "1b635ac9-cea4-4531-88db-24fe88ba0690"]}))
        );
        assert_eq!(
            vt("{\"Pattern\",{\"#\",0b635ac9-cea4-4531-88db-24fe88ba0690}}"),
            Some(json!({"kind": "Ref", "composite": false, "refGuid": "0b635ac9-cea4-4531-88db-24fe88ba0690"}))
        );
        assert_eq!(
            vt("{\"Pattern\",{\"N\",12,2,1}}"),
            Some(json!({"kind": "Number", "digits": 12, "fractionDigits": 2, "nonNegative": true, "composite": false}))
        );
        assert_eq!(
            vt("{\"Pattern\",{\"S\",20,1}}"),
            Some(json!({"kind": "String", "length": 20, "fixed": true, "composite": false}))
        );
        assert_eq!(vt("{\"Pattern\"}"), None);
    }

    // port: tests/test_oof.py::test_unreadable_property_is_a_warning_not_a_failure
    #[test]
    fn unreadable_property_is_a_warning_not_a_failure() {
        let info = parse_metadata(b"{9,{\"Pattern\"},{{1}}}").unwrap();
        let mut seen: Vec<String> = Vec::new();
        let props = decode_props("InputField", &info, &mut |p| seen.push(p.to_string()));
        assert!(props.is_empty());
        assert_eq!(seen, ["Appearance", "InputField"]);
    }

    #[test]
    fn python_slices_and_indexes() {
        let n = parse_metadata(b"{1,2,3,4}").unwrap();
        assert_eq!(py_slice(&n.children, 1, 3).len(), 2);
        assert_eq!(py_slice(&n.children, 1, -1).len(), 2);
        assert_eq!(py_slice(&n.children, 3, 1).len(), 0);
        assert_eq!(py_slice(&n.children, 2, 99).len(), 2);
        assert!(py_get(&n.children, -4).is_some());
        assert!(py_get(&n.children, -5).is_none());
        assert_eq!(py_int("1_0"), Some(10));
        assert_eq!(py_int("+5"), Some(5));
        assert_eq!(py_int("_1"), None);
    }
}
