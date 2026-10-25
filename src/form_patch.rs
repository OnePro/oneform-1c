//! Semantic `Form.xml` models, control identity indexing and the conservative supported-vs-unsupported
//! diff behind `pack-existing-form-xml` (OpenSpec tasks 3.1-3.3) and the lossless source-range patchers
//! for the supported properties (tasks 4.1-4.6).
//!
//! Planning ([`plan_form_patch`]) never mutates a stream: every [`ControlEdit`] names the changed
//! [`Property`] values and [`FormPatchPlan::entry`] resolves the edit to the original binary control
//! (`Node` + `SpanNode`). [`FormPatchPlan::apply`] / [`patch_form`] then replace only the source ranges of the
//! changed atoms (or the one localized caption node) and return a [`PatchedForm`]: the new bytes plus every
//! authorized replacement, so callers can prove byte equality outside those ranges ([`verify_preserved`]).
//! Unexpected node shapes, stale baselines and invalid values are errors; nothing is guessed.
//!
//! Supported edit set (existing controls only): `Caption`, `Left`, `Top`, `Width`, `Height`, `Visible`,
//! `ZOrder`, `TabOrder`. Every other difference between the baseline and edited XML projections - form
//! values and attributes, events, panels/pages, tables/columns, command bars, unknown elements, control
//! additions, deletions, renames, retypes, moves and sibling reordering - is an error; nothing is ignored.
//! Only XML comments, processing instructions and inter-element formatting whitespace are not semantic.
//!
//! Resizing a `Panel` (`Width`/`Height`) also patches the page-boundary records the decoder derives its pages
//! from: for each page with right (edge 3) and bottom (edge 4) records the coordinate moves by the size change,
//! so page `Left`/`Top` stay and page `Width`/`Height` follow the panel. Those page values in `Form.xml` are
//! derived, not user-editable (editing them is still rejected); [`PatchedForm::derived_pages`] reports what the
//! decoder produces so requested-vs-decoded comparisons can account for exactly these changes.
//!
//! All errors use [`GENERATED_FILE_MODIFIED`] (the code pack already uses for rejected `Form.xml` edits)
//! with `kind`, `form`, `source` and a `problems` list; each problem names control, property and line.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde_json::Value;

use crate::brace::{parse_spanned, Atom, AtomKind, Item, Node, Replacement, Span, SpanItem, SpanNode, SpannedMetadata, BARE, QUOTED};
use crate::errors::{ExtractionError, Result, GENERATED_FILE_MODIFIED, METADATA_LAYOUT_UNSUPPORTED};
use crate::metaref::{ConfigTypeLookup, ANY_REF_GUID, VALUE_LIST_GUID, VALUE_TABLE_GUID, VALUE_TREE_GUID};
use crate::naming::is_uuid;
use crate::oof::{control_parts, control_type, geom_page, int, panel_pages, str_val, table, CONTROL_GUIDS};
use crate::oof_props::{atom, caption_path, localized};

pub const UNKNOWN_CONTROL: &str = "UnknownControl";
/// Tag of the placeholder that stands for a nested control inside a [`FormXml::shell`].
pub const CONTROL_STUB: &str = "#control";
/// `meta[2]` of a control without a context menu / a command bar without an explicit action source.
const UNSET_CONTROL_REF: i64 = 4_294_967_295;
const STRING_EXT_GUID: &str = "9a7643d2-19e9-45e2-8893-280bc9195a97";
const TYPE_LINK_GUID: &str = "c866d8e7-76ff-4334-9d02-144f1efe5e1d";
const SUPPORTED_TAGS: [&str; 156] = [
    "DataPath",
    "Caption",
    "Left",
    "Top",
    "Width",
    "Height",
    "Visible",
    "ZOrder",
    "TabOrder",
    "Anchors",
    "Events",
    "Enabled",
    "TransparentBack",
    "BackColor",
    "TextColor",
    "BorderColor",
    "Font",
    "Tooltip",
    "ValueType",
    "HorizontalScrollBar",
    "VerticalScrollBar",
    "ReadOnly",
    "ChoiceButton",
    "ClearButton",
    "OpenButton",
    "DropListButton",
    "SpinButton",
    "AutoMarkIncomplete",
    "AutoChoiceIncomplete",
    "MarkNegatives",
    "AutoCellHeight",
    "ChoiceListMode",
    "HorizontalAlign",
    "Hyperlink",
    "ThreeState",
    "TitleLocation",
    "ChoiceList",
    "ShowTabs",
    "Footer",
    "SelectionMode",
    "InitialTreeView",
    "AutoFill",
    "Buttons",
    "BorderStyle",
    "ModifiesData",
    "SkipOnInput",
    "FirstInGroup",
    "MultiLine",
    "PasswordMode",
    "ExtendedEdit",
    "Mask",
    "ChooseType",
    "TextEdit",
    "VerticalAlign",
    "TypeLink",
    "Header",
    "HorizontalLines",
    "VerticalLines",
    "AutoInsertNewRow",
    "ChangeRowSet",
    "ChangePositionOfColumns",
    "ChangeSettingOfColumns",
    "InitialListView",
    "Output",
    "FieldBackColor",
    "FieldTextColor",
    "ButtonBackColor",
    "ButtonTextColor",
    "PictureLocation",
    "PictureSize",
    "Format",
    "SelectionValue",
    "ChoiceListWidth",
    "MinValue",
    "MaxValue",
    "SeparateByPage",
    "AutoTabOrder",
    "ScrollPageMode",
    "Orientation",
    "Secondary",
    "Zoomable",
    "UseContextMenu",
    "NonselectedPictureText",
    "EnableStartDrag",
    "EnableDrag",
    "ChoiceMode",
    "ShowPicture",
    "ShowCheckBox",
    "Step",
    "LargeStep",
    "MarkingStep",
    "MarkingAppearance",
    "ShowStyle",
    "ShowPercent",
    "SelectionBackColor",
    "SelectionTextColor",
    "HolidayColor",
    "ShowCurrentDate",
    "CalendarNavigation",
    "UseAlternationRowColor",
    "HeaderBackColor",
    "HeaderTextColor",
    "FooterBackColor",
    "FooterTextColor",
    "AlternationRowBackColor",
    "FixedLeft",
    "FixedRight",
    "RowInputMode",
    "RowSelectionMode",
    "HeaderHeight",
    "FooterHeight",
    "HeaderFont",
    "FooterFont",
    "Shortcut",
    "Picture",
    "ChoiceButtonPicture",
    "MenuMode",
    "ScrollingText",
    "LabelPictureLocation",
    "PanelPictureLocation",
    "ChoiceListHeight",
    "ShowSelection",
    "BeginOfDisplayPeriod",
    "EndOfDisplayPeriod",
    "DefaultButton",
    "ContextMenu",
    "ActionSource",
    "AutoContextMenu",
    "Collapse",
    "EmbeddedIntoCell",
    "Print",
    "Protection",
    "ShowGrid",
    "ShowHeaders",
    "ShowGroups",
    "BlackAndWhiteView",
    "TotalsRight",
    "TotalsBelow",
    "PrintScale",
    "FitToPage",
    "BlackAndWhite",
    "Collate",
    "Copies",
    "PerPage",
    "PageOrientation",
    "BottomMargin",
    "TopMargin",
    "LeftMargin",
    "RightMargin",
    "HeaderSize",
    "FooterSize",
    "DuplexPrinting",
    "PrintAccuracy",
    "StepDirection",
    "SaveViewProperties",
    "FixedBackground",
];
const MAX_REPORTED: usize = 50;

// ------------------------------------------------------------------ problems

#[derive(Debug, Clone)]
struct Problem {
    kind: &'static str,
    message: String,
}

fn problem(kind: &'static str, message: String) -> Problem {
    Problem { kind, message }
}

fn fail(form: &str, source: &str, problems: Vec<Problem>) -> ExtractionError {
    let first = &problems[0];
    let shown: Vec<Value> = problems.iter().take(MAX_REPORTED).map(|p| Value::from(p.message.clone())).collect();
    let more = if problems.len() > 1 { format!(" (+{} more)", problems.len() - 1) } else { String::new() };
    ExtractionError::new(
        GENERATED_FILE_MODIFIED,
        format!("{source} Form.xml of form '{form}': {}{more}", first.message),
    )
    .with("kind", first.kind)
    .with("form", form)
    .with("source", source)
    .with("count", problems.len() as u64)
    .with("problems", Value::Array(shown))
}

// ------------------------------------------------------------------ 3.1 semantic XML model

/// Whitespace-insensitive element: attributes are order-free, formatting whitespace between child
/// elements is dropped. Leaf text is verbatim. `line` is informational and never compared.
#[derive(Debug, Clone)]
pub struct XmlElement {
    pub tag: String,
    pub attrs: BTreeMap<String, String>,
    pub text: String,
    pub children: Vec<XmlElement>,
    pub line: u32,
}

impl XmlElement {
    fn new(tag: &str, line: u32) -> XmlElement {
        XmlElement { tag: tag.to_string(), attrs: BTreeMap::new(), text: String::new(), children: Vec::new(), line }
    }
}

type LocItems = Vec<(String, String)>;
type EventItems = Vec<(i64, String)>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValueTypeSpec {
    pub kind: String,
    pub composite: bool,
    pub fraction_digits: Option<i64>,
    pub digits: Option<i64>,
    pub length: Option<i64>,
    pub non_negative: bool,
    pub fixed: bool,
    pub ref_guids: Vec<String>,
    /// Read from a binary pattern that carries flags `<ValueType>` cannot express (`mixedComposite`);
    /// such a value never equals one parsed from `Form.xml`.
    pub lossy: bool,
}

impl ValueTypeSpec {
    pub fn to_attr(&self) -> FormAttribute {
        FormAttribute {
            id: 0,
            name: String::new(),
            main: false,
            stored: false,
            kind: self.kind.clone(),
            composite: self.composite,
            fraction_digits: self.fraction_digits,
            digits: self.digits,
            length: self.length,
            non_negative: self.non_negative,
            fixed: self.fixed,
            ref_guids: self.ref_guids.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormAttribute {
    pub id: i64,
    pub name: String,
    pub main: bool,
    pub stored: bool,
    pub kind: String,
    pub composite: bool,
    pub fraction_digits: Option<i64>,
    pub digits: Option<i64>,
    pub length: Option<i64>,
    pub non_negative: bool,
    pub fixed: bool,
    pub ref_guids: Vec<String>,
}

impl FormAttribute {
    fn same_value_type(&self, other: &FormAttribute) -> bool {
        (
            &self.kind,
            self.composite,
            self.fraction_digits,
            self.digits,
            self.length,
            self.non_negative,
            self.fixed,
            &self.ref_guids,
        ) == (
            &other.kind,
            other.composite,
            other.fraction_digits,
            other.digits,
            other.length,
            other.non_negative,
            other.fixed,
            &other.ref_guids,
        )
    }
}

fn format_attr_pattern(a: &FormAttribute) -> Option<String> {
    match (a.kind.as_str(), a.composite) {
        ("", false) => Some("{\"Pattern\"}".to_string()),
        ("String", false) => {
            if let Some(l) = a.length {
                Some(format!("{{\"Pattern\",{{\"S\",{l},{}}}}}", i32::from(a.fixed)))
            } else if !a.fixed {
                Some("{\"Pattern\",{\"S\"}}".to_string())
            } else {
                None
            }
        }
        ("Boolean", false) => Some("{\"Pattern\",{\"B\"}}".to_string()),
        ("Number", false) => {
            let d = a.digits.unwrap_or(10);
            let f = a.fraction_digits.unwrap_or(0);
            Some(format!("{{\"Pattern\",{{\"N\",{d},{f},{}}}}}", i32::from(a.non_negative)))
        }
        ("DateTime", false) => Some("{\"Pattern\",{\"D\"}}".to_string()),
        ("Date", false) => Some("{\"Pattern\",{\"D\",\"D\"}}".to_string()),
        ("Time", false) => Some("{\"Pattern\",{\"D\",\"T\"}}".to_string()),
        ("ValueTable", false) => Some("{\"Pattern\",{\"#\",acf6192e-81ca-46ef-93a6-5a6968b78663}}".to_string()),
        ("ValueTree", false) => Some("{\"Pattern\",{\"#\",e603c0f2-92fb-4d47-8f38-a44a381cf235}}".to_string()),
        ("ValueList", false) => Some("{\"Pattern\",{\"#\",4772b3b4-f4a3-49c0-a1a5-8cb5961511a3}}".to_string()),
        ("Ref", _) => {
            if !a.ref_guids.is_empty() {
                let branches: Vec<String> = a.ref_guids.iter().map(|g| format!("{{\"#\",{g}}}")).collect();
                Some(format!("{{\"Pattern\",{}}}", branches.join(",")))
            } else if a.composite {
                Some("{\"Pattern\",{\"#\",e61ef7b8-f3e1-4f4b-8ac7-676e90524997}}".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

type LookupStack = Vec<Arc<ConfigTypeLookup>>;

// Type lookups that resolve `ref="..."` names of `<ValueType>` while a configuration form is packed.
thread_local! {
    // The initializer already is `const`; clippy 0.1.99 still reports this static.
    #[allow(clippy::missing_const_for_thread_local)]
    static ACTIVE_TYPE_LOOKUP: RefCell<LookupStack> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` with `lookup` resolving the `ref="..."` names of `<ValueType>` elements parsed meanwhile
/// (without it only the platform built-in names resolve).
pub fn with_type_lookup<R>(lookup: Arc<ConfigTypeLookup>, f: impl FnOnce() -> R) -> R {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            ACTIVE_TYPE_LOOKUP.with(|l| l.borrow_mut().pop());
        }
    }
    ACTIVE_TYPE_LOOKUP.with(|l| l.borrow_mut().push(lookup));
    let _pop = Pop;
    f()
}

fn resolve_type_name(name: &str) -> Option<String> {
    ACTIVE_TYPE_LOOKUP.with(|l| match l.borrow().last() {
        Some(lookup) => lookup.resolve_name(name).map(str::to_string),
        None => ConfigTypeLookup::new().resolve_name(name).map(str::to_string),
    })
}

fn is_builtin_type_guid(guid: &str) -> bool {
    matches!(guid, VALUE_TABLE_GUID | VALUE_TREE_GUID | VALUE_LIST_GUID | ANY_REF_GUID)
}

fn parse_opt_int(e: &XmlElement, attr: &str) -> Option<Option<i64>> {
    match e.attrs.get(attr) {
        Some(s) => Some(Some(s.trim().parse::<i64>().ok()?)),
        None => Some(None),
    }
}

/// Parses a `<ValueType>` element; `None` when it is not a plain, fully supported type
/// (the element then stays an opaque, unchangeable part of the form).
fn parse_value_type_spec(e: &XmlElement) -> Option<ValueTypeSpec> {
    if !e.children.is_empty() || !e.text.trim().is_empty() {
        return None;
    }
    if !e.attrs.keys().all(|k| {
        matches!(
            k.as_str(),
            "kind" | "composite" | "fractionDigits" | "digits" | "length" | "nonNegative" | "fixed" | "ref" | "refGuid" | "title"
        )
    }) {
        return None;
    }
    let kind = e.attrs.get("kind")?.clone();
    let composite = match e.attrs.get("composite")?.as_str() {
        "true" => true,
        "false" => false,
        _ => return None,
    };
    let bool_attr = |attr: &str| -> Option<bool> {
        match e.attrs.get(attr).map(String::as_str) {
            Some("true") => Some(true),
            Some("false") | None => Some(false),
            _ => None,
        }
    };
    let non_negative = bool_attr("nonNegative")?;
    let fixed = bool_attr("fixed")?;
    let fraction_digits = parse_opt_int(e, "fractionDigits")?;
    let mut digits = parse_opt_int(e, "digits")?;
    let length = parse_opt_int(e, "length")?;
    if length.is_some_and(|l| l <= 0 || kind != "String") || (fixed && (kind != "String" || length.is_none())) {
        return None;
    }
    if non_negative && kind != "Number" {
        return None;
    }
    if digits.is_some() || fraction_digits.is_some() {
        if kind != "Number" || digits.is_some_and(|d| d <= 0) || fraction_digits.is_some_and(|f| f < 0) {
            return None;
        }
        if digits == Some(10) {
            digits = None;
        }
    }
    let has_ref_attr = e.attrs.contains_key("refGuid") || e.attrs.contains_key("ref") || e.attrs.contains_key("title");
    if has_ref_attr && kind != "Ref" {
        return None;
    }
    let mut ref_guids = Vec::new();
    if let Some(rg) = e.attrs.get("refGuid") {
        // `title` is an output annotation only; the GUIDs are authoritative.
        for part in rg.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            if !is_uuid(part) {
                return None;
            }
            ref_guids.push(part.to_lowercase());
        }
    } else if let Some(names) = e.attrs.get("ref").or_else(|| e.attrs.get("title")) {
        for part in names.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            ref_guids.push(resolve_type_name(part)?);
        }
    }
    if ref_guids.iter().any(|g| is_builtin_type_guid(g)) {
        return None;
    }
    let composite_ok = match (kind.as_str(), ref_guids.len()) {
        ("Ref", 1) => !composite,
        ("Ref", _) => composite,
        _ => !composite,
    };
    if !composite_ok {
        return None;
    }
    let spec = ValueTypeSpec { kind, composite, fraction_digits, digits, length, non_negative, fixed, ref_guids, lossy: false };
    format_attr_pattern(&spec.to_attr()).map(|_| spec)
}

/// `raw_pattern` is the unchanged original pattern text of the attribute; it is kept byte for byte
/// (flags such as `nonNegative`/`fixed` or mixed composites are not representable in `Form.xml`).
fn format_attr_record(a: &FormAttribute, raw_pattern: Option<&str>) -> Result<String> {
    let pat = match raw_pattern {
        Some(raw) => raw.to_string(),
        None => format_attr_pattern(a).ok_or_else(|| layout_err(format!("unsupported attribute ValueType '{}' (composite={})", a.kind, a.composite)))?,
    };
    Ok(format!("{{{{{}}},{},{},1,{},{}}}", a.id, i32::from(a.main), i32::from(a.stored), quote(&a.name), pat))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageValues {
    pub name: String,
    pub title: Vec<(String, String)>,
    pub visible: bool,
    pub enabled: bool,
    /// Write-only `<TitlePicture>` (`rec[2][2]`); verified in binary, not emitted to `Form.xml`.
    pub title_picture: Option<PictureSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorSpec {
    Rgb(i64, String),
    Style(String),
}

fn parse_color_elem(e: &XmlElement) -> Option<ColorSpec> {
    if !e.children.is_empty() || !e.text.trim().is_empty() {
        return None;
    }
    match e.attrs.get("kind").map(String::as_str)? {
        "rgb" => {
            if e.attrs.len() != 2 {
                return None;
            }
            let hex = e.attrs.get("value")?.trim().strip_prefix('#')?;
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let r = i64::from_str_radix(&hex[0..2], 16).ok()?;
            let g = i64::from_str_radix(&hex[2..4], 16).ok()?;
            let b = i64::from_str_radix(&hex[4..6], 16).ok()?;
            let rgb = (b << 16) | (g << 8) | r;
            Some(ColorSpec::Rgb(rgb, format!("#{:02X}{:02X}{:02X}", r, g, b)))
        }
        "style" => {
            let r = e.attrs.get("ref")?.trim().to_lowercase();
            if r.len() != 36 {
                return None;
            }
            Some(ColorSpec::Style(r))
        }
        _ => None,
    }
}

fn format_color_node(c: Option<&ColorSpec>) -> String {
    match c {
        None => "{3,4,{0}}".to_string(),
        Some(ColorSpec::Rgb(rgb, _)) => format!("{{3,0,{{{rgb}}}}}"),
        Some(ColorSpec::Style(r)) => format!("{{3,3,{{0,{r}}}}}"),
    }
}

fn read_color_spec(item: &Item) -> Option<ColorSpec> {
    let m = crate::oof_props::decode_color(item).ok()??;
    match m.get("kind")?.as_str()? {
        "rgb" => {
            let val = m.get("value")?.as_str()?.to_string();
            let hex = val.strip_prefix('#')?;
            let r = i64::from_str_radix(&hex[0..2], 16).ok()?;
            let g = i64::from_str_radix(&hex[2..4], 16).ok()?;
            let b = i64::from_str_radix(&hex[4..6], 16).ok()?;
            Some(ColorSpec::Rgb((b << 16) | (g << 8) | r, val))
        }
        "style" => Some(ColorSpec::Style(m.get("ref")?.as_str()?.to_lowercase())),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSpec {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub style_source: bool,
    pub face: Option<String>,
    pub size: Option<i64>,
    pub scale: Option<i64>,
}

fn parse_font_elem(e: &XmlElement) -> Option<FontSpec> {
    if !e.children.is_empty() || !e.text.trim().is_empty() || e.attrs.is_empty() {
        return None;
    }
    for k in e.attrs.keys() {
        if !matches!(k.as_str(), "bold" | "italic" | "underline" | "strikeout" | "source" | "face" | "size" | "scale") {
            return None;
        }
    }
    let flag = |name: &str| -> Option<bool> {
        match e.attrs.get(name).map(String::as_str) {
            Some("true") => Some(true),
            Some("false") | None => Some(false),
            _ => None,
        }
    };
    let (bold, italic, underline, strikeout) = (flag("bold")?, flag("italic")?, flag("underline")?, flag("strikeout")?);
    let style_source = match e.attrs.get("source").map(String::as_str) {
        Some("style") => true,
        Some("bin") | None => false,
        _ => return None,
    };
    let face = e.attrs.get("face").map(|s| s.to_string()).filter(|s| !s.is_empty());
    let size = match e.attrs.get("size") {
        Some(s) => Some(s.trim().parse::<i64>().ok().filter(|&v| v > 0)?),
        None => None,
    };
    let scale = match e.attrs.get("scale") {
        Some(s) => Some(s.trim().parse::<i64>().ok().filter(|&v| v > 0)?).filter(|&v| v != 100),
        None => None,
    };
    if !bold && !italic && !underline && !strikeout && face.is_none() && size.is_none() && scale.is_none() {
        return None;
    }
    Some(FontSpec {
        bold,
        italic,
        underline,
        strikeout,
        style_source: style_source && !italic && !underline && !strikeout && face.is_none() && size.is_none() && scale.is_none(),
        face,
        size,
        scale,
    })
}

fn format_font_node(f: Option<&FontSpec>) -> String {
    let Some(f) = f else {
        return "{7,3,0,1,100}".to_string();
    };
    if f.style_source
        && f.bold
        && !f.italic
        && !f.underline
        && !f.strikeout
        && f.face.is_none()
        && f.size.is_none()
        && f.scale.is_none()
    {
        return "{7,2,0,{0,5477b48d-c6e4-4ba3-a715-24bd7ad690bd},1,100}".to_string();
    }
    let mask = if f.bold || f.italic || f.underline || f.strikeout { 60 } else { 0 };
    let weight = if f.bold { 700 } else { 400 };
    let (it, ul, st) = (i32::from(f.italic), i32::from(f.underline), i32::from(f.strikeout));
    let sc = f.scale.unwrap_or(100);
    let h = f.size.map(|s| -(s * 2)).unwrap_or(0);
    let face_part = f.face.as_deref().map(|fc| format!("{},", quote(fc))).unwrap_or_default();
    format!("{{7,1,{mask},{face_part}{{{h}}},{weight},{it},{ul},{st},0,{sc}}}")
}

fn read_font_spec(item: &Item) -> Option<FontSpec> {
    let m = crate::oof_props::decode_font(item).ok()??;
    let flag = |key: &str| m.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    let style_source = m.get("source").and_then(|v| v.as_str()) == Some("style");
    let face = m.get("face").and_then(|v| v.as_str()).map(str::to_string);
    let size = m.get("size").and_then(|v| v.as_i64());
    let scale = m.get("scale").and_then(|v| v.as_i64());
    Some(FontSpec {
        bold: flag("bold"),
        italic: flag("italic"),
        underline: flag("underline"),
        strikeout: flag("strikeout"),
        style_source,
        face,
        size,
        scale,
    })
}

/// `<Shortcut key=".." [mods=".."] [shift|ctrl|alt="true|false"]/>`: virtual key code and modifier bit set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub key: i64,
    pub mods: i64,
}

fn parse_shortcut_elem(e: &XmlElement) -> Option<ShortcutSpec> {
    if !e.children.is_empty() || !e.text.trim().is_empty() {
        return None;
    }
    if !e.attrs.keys().all(|k| matches!(k.as_str(), "key" | "mods" | "shift" | "ctrl" | "alt")) {
        return None;
    }
    let key = e.attrs.get("key")?.trim().parse::<i64>().ok().filter(|k| (0..=65535).contains(k))?;
    let flag = |name: &str| -> Option<i64> {
        match e.attrs.get(name).map(String::as_str) {
            Some("true") => Some(1),
            Some("false") | None => Some(0),
            _ => None,
        }
    };
    let mods = match e.attrs.get("mods") {
        Some(m) => {
            if ["shift", "ctrl", "alt"].iter().any(|k| e.attrs.contains_key(*k)) {
                return None;
            }
            m.trim().parse::<i64>().ok().filter(|m| *m >= 0)?
        }
        None => flag("shift")? | (flag("ctrl")? << 1) | (flag("alt")? << 2),
    };
    Some(ShortcutSpec { key, mods })
}

fn format_shortcut_node(s: &ShortcutSpec) -> String {
    format!("{{{},{},0}}", s.key, s.mods)
}

fn read_shortcut_spec(item: &Item) -> Option<ShortcutSpec> {
    let n = item.as_node().filter(|n| n.len() == 3)?;
    let num = |i: usize| strict_int(&n.atom(i)?.value);
    Some(ShortcutSpec { key: num(0)?, mods: num(1)? })
}

/// `<Picture kind="0|1|2" [refGuid=".."] [stdId=".."]/>`: none, a configuration picture or a standard picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureSpec {
    pub kind: i64,
    pub ref_guid: Option<String>,
    pub std_id: Option<i64>,
}

fn parse_picture_elem(e: &XmlElement) -> Option<PictureSpec> {
    if !e.children.is_empty() || !e.text.trim().is_empty() {
        return None;
    }
    if !e.attrs.keys().all(|k| matches!(k.as_str(), "kind" | "refGuid" | "stdId")) {
        return None;
    }
    let kind = e.attrs.get("kind")?.trim().parse::<i64>().ok()?;
    let ref_guid = e.attrs.get("refGuid").map(|g| g.trim().to_lowercase());
    let std_id = match e.attrs.get("stdId") {
        Some(s) => Some(s.trim().parse::<i64>().ok()?),
        None => None,
    };
    match kind {
        0 if ref_guid.is_none() && std_id.is_none() => Some(PictureSpec { kind, ref_guid, std_id }),
        1 if std_id.is_none() && ref_guid.as_deref().is_some_and(is_uuid) => Some(PictureSpec { kind, ref_guid, std_id }),
        2 if ref_guid.is_none() && std_id.is_some() => Some(PictureSpec { kind, ref_guid, std_id }),
        _ => None,
    }
}

fn format_picture_node(p: &PictureSpec) -> String {
    match p.kind {
        1 => format!("{{4,1,{{0,{}}},\"\",-1,-1,1,0,\"\"}}", p.ref_guid.as_deref().unwrap_or_default().to_lowercase()),
        2 => format!("{{4,2,{{1,{}}},\"\",-1,-1,1,0,\"\"}}", p.std_id.unwrap_or_default()),
        _ => "{4,0,{0},\"\",-1,-1,1,0,\"\"}".to_string(),
    }
}

/// The picture record `{4|3,kind,ref,...}` as a [`PictureSpec`]; `None` for shapes that are not recognised.
fn read_picture_spec(item: &Item) -> Option<PictureSpec> {
    let n = item.as_node().filter(|n| n.len() >= 3)?;
    if !matches!(n.atom(0)?.value.as_str(), "4" | "3") {
        return None;
    }
    let kind = strict_int(&n.atom(1)?.value)?;
    let reference = n.node(2)?;
    match kind {
        0 => Some(PictureSpec { kind, ref_guid: None, std_id: None }),
        1 if reference.atom(0)?.value == "0" => {
            Some(PictureSpec { kind, ref_guid: Some(reference.atom(1)?.value.to_lowercase()), std_id: None })
        }
        2 if reference.atom(0)?.value == "1" => {
            Some(PictureSpec { kind, ref_guid: None, std_id: Some(strict_int(&reference.atom(1)?.value)?) })
        }
        _ => None,
    }
}

/// One anchored edge of a control: the `{2,target,side,offset}` record of `geom[6..9]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorEdge {
    /// Id of the control the edge is attached to; `0` is the form, `-1` marks an unattached edge.
    pub target: i64,
    /// Side of the target: `0` Top, `1` Bottom, `2` Left, `3` Right, `6` none.
    pub side: i64,
    pub offset: i64,
}

impl AnchorEdge {
    pub const UNATTACHED: Self = Self { target: -1, side: 6, offset: 0 };
}

/// `<Anchors .../>`: only the edges that are present are written; absent edges stay as they are in the form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnchorsSpec {
    pub top: Option<AnchorEdge>,
    pub bottom: Option<AnchorEdge>,
    pub left: Option<AnchorEdge>,
    pub right: Option<AnchorEdge>,
}

impl AnchorsSpec {
    /// `(geom index, requested edge)` of the four edges.
    fn edges(&self) -> [(usize, Option<AnchorEdge>); 4] {
        [(6, self.top), (7, self.bottom), (8, self.left), (9, self.right)]
    }
}

fn format_edge_target(e: &AnchorEdge) -> String {
    format!("{{2,{},{},{}}}", e.target, e.side, e.offset)
}

/// The `{2,target,side,offset}` sub-record of the edge record `geom[idx]` (`6` top, `7` bottom, `8` left, `9` right).
pub fn read_anchor_edge(geom: &Node, idx: usize) -> Option<AnchorEdge> {
    let t = geom.children.get(idx)?.as_node()?.children.get(1)?.as_node()?;
    if t.children.len() < 4 {
        return None;
    }
    let num = |i: usize| atom(&t.children[i]).parse::<i64>().ok();
    Some(AnchorEdge { target: num(1)?, side: num(2)?, offset: num(3)? })
}

fn parse_anchors_elem(e: &XmlElement) -> Option<AnchorsSpec> {
    const EDGES: [&str; 4] = ["top", "bottom", "left", "right"];
    if !e.children.is_empty() || !e.text.trim().is_empty() {
        return None;
    }
    let known = |k: &str| {
        k == "target"
            || EDGES.iter().any(|edge| k == *edge || k.strip_prefix(edge).is_some_and(|rest| rest == "Offset" || rest == "Target"))
    };
    if !e.attrs.keys().all(|k| known(k)) || !EDGES.iter().any(|edge| e.attrs.contains_key(*edge)) {
        return None;
    }
    let default_target = parse_opt_int(e, "target")?.unwrap_or(0);
    let edge = |name: &str| -> Option<Option<AnchorEdge>> {
        let (offset_attr, target_attr) = (format!("{name}Offset"), format!("{name}Target"));
        let Some(side) = e.attrs.get(name) else {
            // Offset/target of an edge that is not requested would be silently ignored.
            return if e.attrs.contains_key(&offset_attr) || e.attrs.contains_key(&target_attr) { None } else { Some(None) };
        };
        let side = match side.trim() {
            "Top" | "0" => 0,
            "Bottom" | "1" => 1,
            "Left" | "2" => 2,
            "Right" | "3" => 3,
            "None" | "6" => {
                return if e.attrs.contains_key(&offset_attr) || e.attrs.contains_key(&target_attr) {
                    None
                } else {
                    Some(Some(AnchorEdge::UNATTACHED))
                };
            }
            _ => return None,
        };
        let target = parse_opt_int(e, &target_attr)?.unwrap_or(default_target);
        let offset = parse_opt_int(e, &offset_attr)?.unwrap_or(0);
        Some(Some(AnchorEdge { target, side, offset }))
    };
    Some(AnchorsSpec { top: edge("top")?, bottom: edge("bottom")?, left: edge("left")?, right: edge("right")? })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceItemSpec {
    pub value: String,
    pub presentation: Vec<(String, String)>,
    pub checked: Option<bool>,
    pub picture_id: Option<i64>,
}

fn format_choice_list_node(items: &[ChoiceItemSpec]) -> String {
    if items.is_empty() {
        return "0".to_string();
    }
    let mut parts = Vec::with_capacity(items.len());
    for it in items {
        let mut pres_parts = Vec::with_capacity(1 + 2 * it.presentation.len());
        pres_parts.push(it.presentation.len().to_string());
        for (l, t) in &it.presentation {
            pres_parts.push(quote(l));
            pres_parts.push(quote(t));
        }
        let pic = it.picture_id.unwrap_or(0);
        let chk = i32::from(it.checked.unwrap_or(false));
        parts.push(format!(
            "{{0,{pic},{chk},{{\"S\",{}}},{{0,0,{{{}}}}}}}",
            quote(&it.value),
            pres_parts.join(",")
        ));
    }
    format!("{{8,0,{{0,0,0,0,0,0,{{1,{},{}}}}}}}", items.len(), parts.join(","))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBarButton {
    pub name: String,
    pub id: Option<i64>,
    pub kind: String,
    pub checked: bool,
    pub representation: String,
    pub has_picture: bool,
    pub action_kind: String,
    pub action: String,
    pub caption: Vec<(String, String)>,
    pub tooltip: Vec<(String, String)>,
    pub buttons: Vec<CommandBarButton>,
    /// Write-only action picture (`action[4][2][5]`); verified in binary, not emitted to `Form.xml`.
    pub picture: Option<PictureSpec>,
    /// Write-only action shortcut (`action[4][2][6]`); verified in binary, not emitted to `Form.xml`.
    pub shortcut: Option<ShortcutSpec>,
    /// Write-only status-bar description (`action[4][2][4]`).
    pub description: Vec<(String, String)>,
    /// Write-only default-button flag (`bnode[13]`).
    pub default_button: Option<bool>,
    /// Write-only submenu button order (`bnode[14]`: `0` DontOrder, `1` Ascending).
    pub button_order: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnValues {
    pub id: Option<i64>,
    pub name: String,
    pub field_name: Option<String>,
    pub header: Vec<(String, String)>,
    pub footer_text: Vec<(String, String)>,
    pub tooltip: Vec<(String, String)>,
    pub width: i64,
    pub visible: bool,
    pub read_only: bool,
    pub editor: Option<String>,
    pub events: Vec<(i64, String)>,
    pub ext: ColumnExt,
}

/// Write-only extended catalog properties of a table column (verified in binary, not emitted to `Form.xml`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnExt {
    pub show_hierarchy: Option<bool>,
    pub check_box_three_state: Option<bool>,
    pub show_in_header: Option<bool>,
    pub show_in_footer: Option<bool>,
    pub change_setting: Option<bool>,
    pub change_position: Option<bool>,
    pub mark_negatives: Option<bool>,
    pub change_visible: Option<bool>,
    pub auto_mark_incomplete: Option<bool>,
    pub auto_cell_height: Option<bool>,
    pub hyperlink: Option<bool>,
    pub check_data: Option<i64>,
    pub rows_pictures: Option<PictureSpec>,
    pub header_picture: Option<PictureSpec>,
    pub footer_picture: Option<PictureSpec>,
    pub location: Option<String>,
    pub size_change: Option<String>,
    pub edit_mode: Option<String>,
    pub field_back_color: Option<ColorSpec>,
    pub field_text_color: Option<ColorSpec>,
    pub header_back_color: Option<ColorSpec>,
    pub header_text_color: Option<ColorSpec>,
    pub footer_back_color: Option<ColorSpec>,
    pub footer_text_color: Option<ColorSpec>,
    pub text_font: Option<FontSpec>,
    pub header_font: Option<FontSpec>,
    pub footer_font: Option<FontSpec>,
    pub header_horizontal_align: Option<String>,
    pub horizontal_align_in_column: Option<String>,
    pub footer_horizontal_align: Option<String>,
    pub format: LocItems,
    pub cell_height: Option<i64>,
}

/// Values of the supported property set, with the projection defaults applied
/// (`Visible` true, `TabOrder` 0, empty `Caption` when the element is absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlValues {
    pub name: String,
    pub data_path: Option<(i64, String)>,
    /// `(language, text)` pairs in document order.
    pub caption: Vec<(String, String)>,
    pub left: i64,
    pub top: i64,
    pub width: i64,
    pub height: i64,
    pub visible: bool,
    pub z_order: i64,
    pub tab_order: i64,
    pub events: Vec<(i64, String)>,
    pub enabled: bool,
    pub transparent_back: bool,
    pub back_color: Option<ColorSpec>,
    pub text_color: Option<ColorSpec>,
    pub border_color: Option<ColorSpec>,
    pub font: Option<FontSpec>,
    pub tooltip: Vec<(String, String)>,
    pub value_type: Option<ValueTypeSpec>,
    pub horizontal_scroll_bar: String,
    pub vertical_scroll_bar: String,
    pub read_only: bool,
    pub table_read_only: Option<bool>,
    pub choice_button: Option<bool>,
    pub clear_button: Option<bool>,
    pub open_button: Option<bool>,
    pub drop_list_button: Option<bool>,
    pub spin_button: Option<bool>,
    pub auto_mark_incomplete: bool,
    pub auto_choice_incomplete: Option<bool>,
    pub mark_negatives: bool,
    pub auto_cell_height: Option<bool>,
    pub choice_list_mode: bool,
    pub horizontal_align: String,
    pub hyperlink: bool,
    pub three_state: bool,
    pub title_location: String,
    pub choice_list: Vec<ChoiceItemSpec>,
    pub show_tabs: Option<i64>,
    pub pages: Vec<PageValues>,
    pub footer: Option<bool>,
    pub header: Option<bool>,
    pub horizontal_lines: Option<bool>,
    pub vertical_lines: Option<bool>,
    pub auto_insert_new_row: Option<bool>,
    pub change_row_set: Option<bool>,
    pub change_position_of_columns: Option<bool>,
    pub change_setting_of_columns: Option<bool>,
    pub initial_list_view: Option<String>,
    pub output: Option<String>,
    pub selection_mode: String,
    pub initial_tree_view: String,
    pub columns: Vec<ColumnValues>,
    pub auto_fill: Option<bool>,
    pub buttons: Vec<CommandBarButton>,
    pub border_style: Option<i64>,
    pub anchors: Option<AnchorsSpec>,
    pub modifies_data: Option<bool>,
    pub skip_on_input: Option<bool>,
    pub first_in_group: Option<bool>,
    pub multi_line: Option<bool>,
    pub password_mode: Option<bool>,
    pub extended_edit: Option<bool>,
    pub mask: Option<String>,
    pub choose_type: Option<bool>,
    pub text_edit: Option<bool>,
    pub vertical_align: Option<String>,
    pub type_link: Option<TypeLinkSpec>,
    pub field_back_color: Option<ColorSpec>,
    pub field_text_color: Option<ColorSpec>,
    pub button_back_color: Option<ColorSpec>,
    pub button_text_color: Option<ColorSpec>,
    pub picture_location: Option<String>,
    pub picture_size: Option<String>,
    pub format: LocItems,
    pub selection_value: Option<String>,
    pub choice_list_width: Option<i64>,
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    pub separate_by_page: Option<bool>,
    pub auto_tab_order: Option<bool>,
    pub scroll_page_mode: Option<bool>,
    pub orientation: Option<String>,
    pub secondary: Option<bool>,
    pub zoomable: Option<bool>,
    pub use_context_menu: Option<bool>,
    pub nonselected_picture_text: LocItems,
    pub enable_start_drag: Option<bool>,
    pub enable_drag: Option<bool>,
    pub choice_mode: Option<bool>,
    pub show_picture: Option<bool>,
    pub show_check_box: Option<bool>,
    pub step: Option<i64>,
    pub large_step: Option<i64>,
    pub marking_step: Option<i64>,
    pub marking_appearance: Option<String>,
    pub show_style: Option<String>,
    pub show_percent: Option<bool>,
    pub selection_back_color: Option<ColorSpec>,
    pub selection_text_color: Option<ColorSpec>,
    pub holiday_color: Option<ColorSpec>,
    pub show_current_date: Option<bool>,
    pub calendar_navigation: Option<bool>,
    pub use_alternation_row_color: Option<bool>,
    pub header_back_color: Option<ColorSpec>,
    pub header_text_color: Option<ColorSpec>,
    pub footer_back_color: Option<ColorSpec>,
    pub footer_text_color: Option<ColorSpec>,
    pub alternation_row_back_color: Option<ColorSpec>,
    pub fixed_left: Option<i64>,
    pub fixed_right: Option<i64>,
    pub row_input_mode: Option<String>,
    pub row_selection_mode: Option<String>,
    pub header_height: Option<i64>,
    pub footer_height: Option<i64>,
    pub header_font: Option<FontSpec>,
    pub footer_font: Option<FontSpec>,
    pub shortcut: Option<ShortcutSpec>,
    pub picture: Option<PictureSpec>,
    pub choice_button_picture: Option<PictureSpec>,
    pub menu_mode: Option<bool>,
    pub scrolling_text: Option<String>,
    pub label_picture_location: Option<String>,
    pub panel_picture_location: Option<String>,
    pub choice_list_height: Option<i64>,
    pub show_selection: Option<String>,
    pub begin_of_display_period: Option<i64>,
    pub end_of_display_period: Option<i64>,
    pub default_button: Option<bool>,
    pub context_menu: Option<i64>,
    pub action_source: Option<i64>,
    pub auto_context_menu: Option<bool>,
    pub collapse: Option<String>,
    pub embedded_into_cell: Option<bool>,
    pub print: Option<bool>,
    pub protection: Option<bool>,
    pub show_grid: Option<bool>,
    pub show_headers: Option<bool>,
    pub show_groups: Option<bool>,
    pub black_and_white_view: Option<bool>,
    pub totals_right: Option<bool>,
    pub totals_below: Option<bool>,
    pub print_scale: Option<i64>,
    pub fit_to_page: Option<bool>,
    pub black_and_white: Option<bool>,
    pub collate: Option<bool>,
    pub copies: Option<i64>,
    pub per_page: Option<i64>,
    pub page_orientation: Option<String>,
    pub bottom_margin: Option<i64>,
    pub top_margin: Option<i64>,
    pub left_margin: Option<i64>,
    pub right_margin: Option<i64>,
    pub header_size: Option<i64>,
    pub footer_size: Option<i64>,
    pub duplex_printing: Option<i64>,
    pub print_accuracy: Option<i64>,
    pub step_direction: Option<i64>,
    pub save_view_properties: Option<bool>,
    pub fixed_background: Option<bool>,
}

/// `<TypeLink>` of an `InputField`: the attribute (and optional field GUID) whose type the field follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLinkSpec {
    pub attr_id: i64,
    pub field_guid: Option<String>,
    pub path: String,
}

impl ControlValues {
    fn empty() -> ControlValues {
        ControlValues {
            name: String::new(),
            data_path: None,
            caption: Vec::new(),
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            visible: true,
            z_order: 0,
            tab_order: 0,
            events: Vec::new(),
            enabled: true,
            transparent_back: false,
            back_color: None,
            text_color: None,
            border_color: None,
            font: None,
            tooltip: Vec::new(),
            value_type: None,
            horizontal_scroll_bar: String::new(),
            vertical_scroll_bar: String::new(),
            read_only: false,
            table_read_only: None,
            choice_button: None,
            clear_button: None,
            open_button: None,
            drop_list_button: None,
            spin_button: None,
            auto_mark_incomplete: false,
            auto_choice_incomplete: None,
            mark_negatives: false,
            auto_cell_height: None,
            choice_list_mode: false,
            horizontal_align: String::new(),
            hyperlink: false,
            three_state: false,
            title_location: String::new(),
            choice_list: Vec::new(),
            show_tabs: None,
            pages: Vec::new(),
            footer: None,
            header: None,
            horizontal_lines: None,
            vertical_lines: None,
            auto_insert_new_row: None,
            change_row_set: None,
            change_position_of_columns: None,
            change_setting_of_columns: None,
            initial_list_view: None,
            output: None,
            selection_mode: String::new(),
            initial_tree_view: String::new(),
            columns: Vec::new(),
            auto_fill: None,
            buttons: Vec::new(),
            border_style: None,
            anchors: None,
            modifies_data: None,
            skip_on_input: None,
            first_in_group: None,
            multi_line: None,
            password_mode: None,
            extended_edit: None,
            mask: None,
            choose_type: None,
            text_edit: None,
            vertical_align: None,
            type_link: None,
            field_back_color: None,
            field_text_color: None,
            button_back_color: None,
            button_text_color: None,
            picture_location: None,
            picture_size: None,
            format: Vec::new(),
            selection_value: None,
            choice_list_width: None,
            min_value: None,
            max_value: None,
            separate_by_page: None,
            auto_tab_order: None,
            scroll_page_mode: None,
            orientation: None,
            secondary: None,
            zoomable: None,
            use_context_menu: None,
            nonselected_picture_text: Vec::new(),
            enable_start_drag: None,
            enable_drag: None,
            choice_mode: None,
            show_picture: None,
            show_check_box: None,
            step: None,
            large_step: None,
            marking_step: None,
            marking_appearance: None,
            show_style: None,
            show_percent: None,
            selection_back_color: None,
            selection_text_color: None,
            holiday_color: None,
            show_current_date: None,
            calendar_navigation: None,
            use_alternation_row_color: None,
            header_back_color: None,
            header_text_color: None,
            footer_back_color: None,
            footer_text_color: None,
            alternation_row_back_color: None,
            fixed_left: None,
            fixed_right: None,
            row_input_mode: None,
            row_selection_mode: None,
            header_height: None,
            footer_height: None,
            header_font: None,
            footer_font: None,
            shortcut: None,
            picture: None,
            choice_button_picture: None,
            menu_mode: None,
            scrolling_text: None,
            label_picture_location: None,
            panel_picture_location: None,
            choice_list_height: None,
            show_selection: None,
            begin_of_display_period: None,
            end_of_display_period: None,
            default_button: None,
            context_menu: None,
            action_source: None,
            auto_context_menu: None,
            collapse: None,
            embedded_into_cell: None,
            print: None,
            protection: None,
            show_grid: None,
            show_headers: None,
            show_groups: None,
            black_and_white_view: None,
            totals_right: None,
            totals_below: None,
            print_scale: None,
            fit_to_page: None,
            black_and_white: None,
            collate: None,
            copies: None,
            per_page: None,
            page_orientation: None,
            bottom_margin: None,
            top_margin: None,
            left_margin: None,
            right_margin: None,
            header_size: None,
            footer_size: None,
            duplex_printing: None,
            print_accuracy: None,
            step_direction: None,
            save_view_properties: None,
            fixed_background: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Property {
    Name,
    DataPath,
    Caption,
    Left,
    Top,
    Width,
    Height,
    Visible,
    ZOrder,
    TabOrder,
    Events,
    Enabled,
    TransparentBack,
    BackColor,
    TextColor,
    BorderColor,
    Font,
    Tooltip,
    ValueType,
    HorizontalScrollBar,
    VerticalScrollBar,
    ReadOnly,
    ChoiceButton,
    ClearButton,
    OpenButton,
    DropListButton,
    SpinButton,
    AutoMarkIncomplete,
    AutoChoiceIncomplete,
    MarkNegatives,
    AutoCellHeight,
    ChoiceListMode,
    HorizontalAlign,
    Hyperlink,
    ThreeState,
    TitleLocation,
    ChoiceList,
    ShowTabs,
    Pages,
    Footer,
    Header,
    HorizontalLines,
    VerticalLines,
    AutoInsertNewRow,
    ChangeRowSet,
    ChangePositionOfColumns,
    ChangeSettingOfColumns,
    InitialListView,
    Output,
    SelectionMode,
    InitialTreeView,
    Columns,
    AutoFill,
    Buttons,
    BorderStyle,
    Anchors,
    ModifiesData,
    SkipOnInput,
    FirstInGroup,
    MultiLine,
    PasswordMode,
    ExtendedEdit,
    Mask,
    ChooseType,
    TextEdit,
    VerticalAlign,
    TypeLink,
    FieldBackColor,
    FieldTextColor,
    ButtonBackColor,
    ButtonTextColor,
    PictureLocation,
    PictureSize,
    Format,
    SelectionValue,
    ChoiceListWidth,
    MinValue,
    MaxValue,
    SeparateByPage,
    AutoTabOrder,
    ScrollPageMode,
    Orientation,
    Secondary,
    Zoomable,
    UseContextMenu,
    NonselectedPictureText,
    EnableStartDrag,
    EnableDrag,
    ChoiceMode,
    ShowPicture,
    ShowCheckBox,
    Step,
    LargeStep,
    MarkingStep,
    MarkingAppearance,
    ShowStyle,
    ShowPercent,
    SelectionBackColor,
    SelectionTextColor,
    HolidayColor,
    ShowCurrentDate,
    CalendarNavigation,
    UseAlternationRowColor,
    HeaderBackColor,
    HeaderTextColor,
    FooterBackColor,
    FooterTextColor,
    AlternationRowBackColor,
    FixedLeft,
    FixedRight,
    RowInputMode,
    RowSelectionMode,
    HeaderHeight,
    FooterHeight,
    HeaderFont,
    FooterFont,
    Shortcut,
    Picture,
    ChoiceButtonPicture,
    MenuMode,
    ScrollingText,
    LabelPictureLocation,
    PanelPictureLocation,
    ChoiceListHeight,
    ShowSelection,
    BeginOfDisplayPeriod,
    EndOfDisplayPeriod,
    DefaultButton,
    ContextMenu,
    ActionSource,
    AutoContextMenu,
    Collapse,
    EmbeddedIntoCell,
    Print,
    Protection,
    ShowGrid,
    ShowHeaders,
    ShowGroups,
    BlackAndWhiteView,
    TotalsRight,
    TotalsBelow,
    PrintScale,
    FitToPage,
    BlackAndWhite,
    Collate,
    Copies,
    PerPage,
    PageOrientation,
    BottomMargin,
    TopMargin,
    LeftMargin,
    RightMargin,
    HeaderSize,
    FooterSize,
    DuplexPrinting,
    PrintAccuracy,
    StepDirection,
    SaveViewProperties,
    FixedBackground,
}

impl Property {
    pub fn xml_name(self) -> &'static str {
        match self {
            Property::Name => "name",
            Property::DataPath => "DataPath",
            Property::Caption => "Caption",
            Property::Left => "Left",
            Property::Top => "Top",
            Property::Width => "Width",
            Property::Height => "Height",
            Property::Visible => "Visible",
            Property::ZOrder => "ZOrder",
            Property::TabOrder => "TabOrder",
            Property::Events => "Events",
            Property::Enabled => "Enabled",
            Property::TransparentBack => "TransparentBack",
            Property::BackColor => "BackColor",
            Property::TextColor => "TextColor",
            Property::BorderColor => "BorderColor",
            Property::Font => "Font",
            Property::Tooltip => "Tooltip",
            Property::ValueType => "ValueType",
            Property::HorizontalScrollBar => "HorizontalScrollBar",
            Property::VerticalScrollBar => "VerticalScrollBar",
            Property::ReadOnly => "ReadOnly",
            Property::ChoiceButton => "ChoiceButton",
            Property::ClearButton => "ClearButton",
            Property::OpenButton => "OpenButton",
            Property::DropListButton => "DropListButton",
            Property::SpinButton => "SpinButton",
            Property::AutoMarkIncomplete => "AutoMarkIncomplete",
            Property::AutoChoiceIncomplete => "AutoChoiceIncomplete",
            Property::MarkNegatives => "MarkNegatives",
            Property::AutoCellHeight => "AutoCellHeight",
            Property::ChoiceListMode => "ChoiceListMode",
            Property::HorizontalAlign => "HorizontalAlign",
            Property::Hyperlink => "Hyperlink",
            Property::ThreeState => "ThreeState",
            Property::TitleLocation => "TitleLocation",
            Property::ChoiceList => "ChoiceList",
            Property::ShowTabs => "ShowTabs",
            Property::Pages => "Pages",
            Property::Footer => "Footer",
            Property::Header => "Header",
            Property::HorizontalLines => "HorizontalLines",
            Property::VerticalLines => "VerticalLines",
            Property::AutoInsertNewRow => "AutoInsertNewRow",
            Property::ChangeRowSet => "ChangeRowSet",
            Property::ChangePositionOfColumns => "ChangePositionOfColumns",
            Property::ChangeSettingOfColumns => "ChangeSettingOfColumns",
            Property::InitialListView => "InitialListView",
            Property::Output => "Output",
            Property::SelectionMode => "SelectionMode",
            Property::InitialTreeView => "InitialTreeView",
            Property::Columns => "Columns",
            Property::AutoFill => "AutoFill",
            Property::Buttons => "Buttons",
            Property::BorderStyle => "BorderStyle",
            Property::Anchors => "Anchors",
            Property::ModifiesData => "ModifiesData",
            Property::SkipOnInput => "SkipOnInput",
            Property::FirstInGroup => "FirstInGroup",
            Property::MultiLine => "MultiLine",
            Property::PasswordMode => "PasswordMode",
            Property::ExtendedEdit => "ExtendedEdit",
            Property::Mask => "Mask",
            Property::ChooseType => "ChooseType",
            Property::TextEdit => "TextEdit",
            Property::VerticalAlign => "VerticalAlign",
            Property::TypeLink => "TypeLink",
            Property::FieldBackColor => "FieldBackColor",
            Property::FieldTextColor => "FieldTextColor",
            Property::ButtonBackColor => "ButtonBackColor",
            Property::ButtonTextColor => "ButtonTextColor",
            Property::PictureLocation => "PictureLocation",
            Property::PictureSize => "PictureSize",
            Property::Format => "Format",
            Property::SelectionValue => "SelectionValue",
            Property::ChoiceListWidth => "ChoiceListWidth",
            Property::MinValue => "MinValue",
            Property::MaxValue => "MaxValue",
            Property::SeparateByPage => "SeparateByPage",
            Property::AutoTabOrder => "AutoTabOrder",
            Property::ScrollPageMode => "ScrollPageMode",
            Property::Orientation => "Orientation",
            Property::Secondary => "Secondary",
            Property::Zoomable => "Zoomable",
            Property::UseContextMenu => "UseContextMenu",
            Property::NonselectedPictureText => "NonselectedPictureText",
            Property::EnableStartDrag => "EnableStartDrag",
            Property::EnableDrag => "EnableDrag",
            Property::ChoiceMode => "ChoiceMode",
            Property::ShowPicture => "ShowPicture",
            Property::ShowCheckBox => "ShowCheckBox",
            Property::Step => "Step",
            Property::LargeStep => "LargeStep",
            Property::MarkingStep => "MarkingStep",
            Property::MarkingAppearance => "MarkingAppearance",
            Property::ShowStyle => "ShowStyle",
            Property::ShowPercent => "ShowPercent",
            Property::SelectionBackColor => "SelectionBackColor",
            Property::SelectionTextColor => "SelectionTextColor",
            Property::HolidayColor => "HolidayColor",
            Property::ShowCurrentDate => "ShowCurrentDate",
            Property::CalendarNavigation => "CalendarNavigation",
            Property::UseAlternationRowColor => "UseAlternationRowColor",
            Property::HeaderBackColor => "HeaderBackColor",
            Property::HeaderTextColor => "HeaderTextColor",
            Property::FooterBackColor => "FooterBackColor",
            Property::FooterTextColor => "FooterTextColor",
            Property::AlternationRowBackColor => "AlternationRowBackColor",
            Property::FixedLeft => "FixedLeft",
            Property::FixedRight => "FixedRight",
            Property::RowInputMode => "RowInputMode",
            Property::RowSelectionMode => "RowSelectionMode",
            Property::HeaderHeight => "HeaderHeight",
            Property::FooterHeight => "FooterHeight",
            Property::HeaderFont => "HeaderFont",
            Property::FooterFont => "FooterFont",
            Property::Shortcut => "Shortcut",
            Property::Picture => "Picture",
            Property::ChoiceButtonPicture => "ChoiceButtonPicture",
            Property::MenuMode => "MenuMode",
            Property::ScrollingText => "ScrollingText",
            Property::LabelPictureLocation => "LabelPictureLocation",
            Property::PanelPictureLocation => "PanelPictureLocation",
            Property::ChoiceListHeight => "ChoiceListHeight",
            Property::ShowSelection => "ShowSelection",
            Property::BeginOfDisplayPeriod => "BeginOfDisplayPeriod",
            Property::EndOfDisplayPeriod => "EndOfDisplayPeriod",
            Property::DefaultButton => "DefaultButton",
            Property::ContextMenu => "ContextMenu",
            Property::ActionSource => "ActionSource",
            Property::AutoContextMenu => "AutoContextMenu",
            Property::Collapse => "Collapse",
            Property::EmbeddedIntoCell => "EmbeddedIntoCell",
            Property::Print => "Print",
            Property::Protection => "Protection",
            Property::ShowGrid => "ShowGrid",
            Property::ShowHeaders => "ShowHeaders",
            Property::ShowGroups => "ShowGroups",
            Property::BlackAndWhiteView => "BlackAndWhiteView",
            Property::TotalsRight => "TotalsRight",
            Property::TotalsBelow => "TotalsBelow",
            Property::PrintScale => "PrintScale",
            Property::FitToPage => "FitToPage",
            Property::BlackAndWhite => "BlackAndWhite",
            Property::Collate => "Collate",
            Property::Copies => "Copies",
            Property::PerPage => "PerPage",
            Property::PageOrientation => "PageOrientation",
            Property::BottomMargin => "BottomMargin",
            Property::TopMargin => "TopMargin",
            Property::LeftMargin => "LeftMargin",
            Property::RightMargin => "RightMargin",
            Property::HeaderSize => "HeaderSize",
            Property::FooterSize => "FooterSize",
            Property::DuplexPrinting => "DuplexPrinting",
            Property::PrintAccuracy => "PrintAccuracy",
            Property::StepDirection => "StepDirection",
            Property::SaveViewProperties => "SaveViewProperties",
            Property::FixedBackground => "FixedBackground",
        }
    }
}

impl ControlValues {
    /// Supported properties whose value differs from `other`.
    pub fn changed_from(&self, other: &ControlValues) -> Vec<Property> {
        [
            (self.name == other.name, Property::Name),
            (self.data_path == other.data_path, Property::DataPath),
            (self.caption == other.caption, Property::Caption),
            (self.left == other.left, Property::Left),
            (self.top == other.top, Property::Top),
            (self.width == other.width, Property::Width),
            (self.height == other.height, Property::Height),
            (self.visible == other.visible, Property::Visible),
            (self.z_order == other.z_order, Property::ZOrder),
            (self.tab_order == other.tab_order, Property::TabOrder),
            (self.events == other.events, Property::Events),
            (self.enabled == other.enabled, Property::Enabled),
            (self.transparent_back == other.transparent_back, Property::TransparentBack),
            (self.back_color == other.back_color, Property::BackColor),
            (self.text_color == other.text_color, Property::TextColor),
            (self.border_color == other.border_color, Property::BorderColor),
            (self.font == other.font, Property::Font),
            (self.tooltip == other.tooltip, Property::Tooltip),
            (self.value_type == other.value_type, Property::ValueType),
            (self.horizontal_scroll_bar == other.horizontal_scroll_bar, Property::HorizontalScrollBar),
            (self.vertical_scroll_bar == other.vertical_scroll_bar, Property::VerticalScrollBar),
            (
                self.read_only == other.read_only && self.table_read_only == other.table_read_only,
                Property::ReadOnly,
            ),
            (self.choice_button == other.choice_button, Property::ChoiceButton),
            (self.clear_button == other.clear_button, Property::ClearButton),
            (self.open_button == other.open_button, Property::OpenButton),
            (self.drop_list_button == other.drop_list_button, Property::DropListButton),
            (self.spin_button == other.spin_button, Property::SpinButton),
            (self.auto_mark_incomplete == other.auto_mark_incomplete, Property::AutoMarkIncomplete),
            (self.auto_choice_incomplete == other.auto_choice_incomplete, Property::AutoChoiceIncomplete),
            (self.mark_negatives == other.mark_negatives, Property::MarkNegatives),
            (self.auto_cell_height == other.auto_cell_height, Property::AutoCellHeight),
            (self.choice_list_mode == other.choice_list_mode, Property::ChoiceListMode),
            (self.horizontal_align == other.horizontal_align, Property::HorizontalAlign),
            (self.hyperlink == other.hyperlink, Property::Hyperlink),
            (self.three_state == other.three_state, Property::ThreeState),
            (self.title_location == other.title_location, Property::TitleLocation),
            (self.choice_list == other.choice_list, Property::ChoiceList),
            (self.show_tabs == other.show_tabs, Property::ShowTabs),
            (self.pages == other.pages, Property::Pages),
            (self.footer == other.footer, Property::Footer),
            (self.header == other.header, Property::Header),
            (self.horizontal_lines == other.horizontal_lines, Property::HorizontalLines),
            (self.vertical_lines == other.vertical_lines, Property::VerticalLines),
            (self.auto_insert_new_row == other.auto_insert_new_row, Property::AutoInsertNewRow),
            (self.change_row_set == other.change_row_set, Property::ChangeRowSet),
            (self.change_position_of_columns == other.change_position_of_columns, Property::ChangePositionOfColumns),
            (self.change_setting_of_columns == other.change_setting_of_columns, Property::ChangeSettingOfColumns),
            (self.initial_list_view == other.initial_list_view, Property::InitialListView),
            (self.output == other.output, Property::Output),
            (self.selection_mode == other.selection_mode, Property::SelectionMode),
            (self.initial_tree_view == other.initial_tree_view, Property::InitialTreeView),
            (self.columns == other.columns, Property::Columns),
            (self.auto_fill == other.auto_fill, Property::AutoFill),
            (self.buttons == other.buttons, Property::Buttons),
            (self.border_style == other.border_style, Property::BorderStyle),
            (self.anchors == other.anchors, Property::Anchors),
            (self.modifies_data == other.modifies_data, Property::ModifiesData),
            (self.skip_on_input == other.skip_on_input, Property::SkipOnInput),
            (self.first_in_group == other.first_in_group, Property::FirstInGroup),
            (self.multi_line == other.multi_line, Property::MultiLine),
            (self.password_mode == other.password_mode, Property::PasswordMode),
            (self.extended_edit == other.extended_edit, Property::ExtendedEdit),
            (self.mask == other.mask, Property::Mask),
            (self.choose_type == other.choose_type, Property::ChooseType),
            (self.text_edit == other.text_edit, Property::TextEdit),
            (self.vertical_align == other.vertical_align, Property::VerticalAlign),
            (self.type_link == other.type_link, Property::TypeLink),
            (self.field_back_color == other.field_back_color, Property::FieldBackColor),
            (self.field_text_color == other.field_text_color, Property::FieldTextColor),
            (self.button_back_color == other.button_back_color, Property::ButtonBackColor),
            (self.button_text_color == other.button_text_color, Property::ButtonTextColor),
            (self.picture_location == other.picture_location, Property::PictureLocation),
            (self.picture_size == other.picture_size, Property::PictureSize),
            (self.format == other.format, Property::Format),
            (self.selection_value == other.selection_value, Property::SelectionValue),
            (self.choice_list_width == other.choice_list_width, Property::ChoiceListWidth),
            (self.min_value == other.min_value, Property::MinValue),
            (self.max_value == other.max_value, Property::MaxValue),
            (self.separate_by_page == other.separate_by_page, Property::SeparateByPage),
            (self.auto_tab_order == other.auto_tab_order, Property::AutoTabOrder),
            (self.scroll_page_mode == other.scroll_page_mode, Property::ScrollPageMode),
            (self.orientation == other.orientation, Property::Orientation),
            (self.secondary == other.secondary, Property::Secondary),
            (self.zoomable == other.zoomable, Property::Zoomable),
            (self.use_context_menu == other.use_context_menu, Property::UseContextMenu),
            (self.nonselected_picture_text == other.nonselected_picture_text, Property::NonselectedPictureText),
            (self.enable_start_drag == other.enable_start_drag, Property::EnableStartDrag),
            (self.enable_drag == other.enable_drag, Property::EnableDrag),
            (self.choice_mode == other.choice_mode, Property::ChoiceMode),
            (self.show_picture == other.show_picture, Property::ShowPicture),
            (self.show_check_box == other.show_check_box, Property::ShowCheckBox),
            (self.step == other.step, Property::Step),
            (self.large_step == other.large_step, Property::LargeStep),
            (self.marking_step == other.marking_step, Property::MarkingStep),
            (self.marking_appearance == other.marking_appearance, Property::MarkingAppearance),
            (self.show_style == other.show_style, Property::ShowStyle),
            (self.show_percent == other.show_percent, Property::ShowPercent),
            (self.selection_back_color == other.selection_back_color, Property::SelectionBackColor),
            (self.selection_text_color == other.selection_text_color, Property::SelectionTextColor),
            (self.holiday_color == other.holiday_color, Property::HolidayColor),
            (self.show_current_date == other.show_current_date, Property::ShowCurrentDate),
            (self.calendar_navigation == other.calendar_navigation, Property::CalendarNavigation),
            (self.use_alternation_row_color == other.use_alternation_row_color, Property::UseAlternationRowColor),
            (self.header_back_color == other.header_back_color, Property::HeaderBackColor),
            (self.header_text_color == other.header_text_color, Property::HeaderTextColor),
            (self.footer_back_color == other.footer_back_color, Property::FooterBackColor),
            (self.footer_text_color == other.footer_text_color, Property::FooterTextColor),
            (self.alternation_row_back_color == other.alternation_row_back_color, Property::AlternationRowBackColor),
            (self.fixed_left == other.fixed_left, Property::FixedLeft),
            (self.fixed_right == other.fixed_right, Property::FixedRight),
            (self.row_input_mode == other.row_input_mode, Property::RowInputMode),
            (self.row_selection_mode == other.row_selection_mode, Property::RowSelectionMode),
            (self.header_height == other.header_height, Property::HeaderHeight),
            (self.footer_height == other.footer_height, Property::FooterHeight),
            (self.header_font == other.header_font, Property::HeaderFont),
            (self.footer_font == other.footer_font, Property::FooterFont),
            (self.shortcut == other.shortcut, Property::Shortcut),
            (self.picture == other.picture, Property::Picture),
            (self.choice_button_picture == other.choice_button_picture, Property::ChoiceButtonPicture),
            (self.menu_mode == other.menu_mode, Property::MenuMode),
            (self.scrolling_text == other.scrolling_text, Property::ScrollingText),
            (self.label_picture_location == other.label_picture_location, Property::LabelPictureLocation),
            (self.panel_picture_location == other.panel_picture_location, Property::PanelPictureLocation),
            (self.choice_list_height == other.choice_list_height, Property::ChoiceListHeight),
            (self.show_selection == other.show_selection, Property::ShowSelection),
            (self.begin_of_display_period == other.begin_of_display_period, Property::BeginOfDisplayPeriod),
            (self.end_of_display_period == other.end_of_display_period, Property::EndOfDisplayPeriod),
            (self.default_button == other.default_button, Property::DefaultButton),
            (self.context_menu == other.context_menu, Property::ContextMenu),
            (self.action_source == other.action_source, Property::ActionSource),
            (self.auto_context_menu == other.auto_context_menu, Property::AutoContextMenu),
            (self.collapse == other.collapse, Property::Collapse),
            (self.embedded_into_cell == other.embedded_into_cell, Property::EmbeddedIntoCell),
            (self.print == other.print, Property::Print),
            (self.protection == other.protection, Property::Protection),
            (self.show_grid == other.show_grid, Property::ShowGrid),
            (self.show_headers == other.show_headers, Property::ShowHeaders),
            (self.show_groups == other.show_groups, Property::ShowGroups),
            (self.black_and_white_view == other.black_and_white_view, Property::BlackAndWhiteView),
            (self.totals_right == other.totals_right, Property::TotalsRight),
            (self.totals_below == other.totals_below, Property::TotalsBelow),
            (self.print_scale == other.print_scale, Property::PrintScale),
            (self.fit_to_page == other.fit_to_page, Property::FitToPage),
            (self.black_and_white == other.black_and_white, Property::BlackAndWhite),
            (self.collate == other.collate, Property::Collate),
            (self.copies == other.copies, Property::Copies),
            (self.per_page == other.per_page, Property::PerPage),
            (self.page_orientation == other.page_orientation, Property::PageOrientation),
            (self.bottom_margin == other.bottom_margin, Property::BottomMargin),
            (self.top_margin == other.top_margin, Property::TopMargin),
            (self.left_margin == other.left_margin, Property::LeftMargin),
            (self.right_margin == other.right_margin, Property::RightMargin),
            (self.header_size == other.header_size, Property::HeaderSize),
            (self.footer_size == other.footer_size, Property::FooterSize),
            (self.duplex_printing == other.duplex_printing, Property::DuplexPrinting),
            (self.print_accuracy == other.print_accuracy, Property::PrintAccuracy),
            (self.step_direction == other.step_direction, Property::StepDirection),
            (self.save_view_properties == other.save_view_properties, Property::SaveViewProperties),
            (self.fixed_background == other.fixed_background, Property::FixedBackground),
        ]
        .into_iter()
        .filter(|(same, _)| !same)
        .map(|(_, p)| p)
        .collect()
    }
}

/// A control element (a control tag directly inside `ChildItems`).
#[derive(Debug, Clone)]
pub struct ControlXml {
    pub id: i64,
    pub name: String,
    /// XML tag: a type from `CONTROL_GUIDS` or [`UNKNOWN_CONTROL`].
    pub type_name: String,
    /// `guid` attribute (only `UnknownControl` carries one).
    pub guid: Option<String>,
    /// Id of the enclosing control, `None` for form level.
    pub parent: Option<i64>,
    /// Name of the enclosing `Page`, if any.
    pub page: Option<String>,
    pub line: u32,
    pub values: ControlValues,
    /// The control element without nested controls (replaced by [`CONTROL_STUB`]) and without the
    /// supported property elements; everything left here is unsupported to edit.
    pub rest: XmlElement,
}

impl ControlXml {
    /// `InputField 'Name' (id 5)`.
    pub fn describe(&self) -> String {
        format!("{} '{}' (id {})", self.type_name, self.name, self.id)
    }
}

/// Write-only root `<Form>` window/behavior properties (verified in binary, cleared in `in_decoder_order`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormRootProps {
    pub auto_title: Option<bool>,
    pub window_state: Option<i64>,
    pub size_change: Option<i64>,
    pub connectable_window: Option<bool>,
    pub allow_join_window: Option<bool>,
    pub caption_picture: Option<PictureSpec>,
    pub window_appearance_mode: Option<i64>,
    pub window_location: Option<i64>,
    pub change_window_appearance_mode: Option<i64>,
    pub desktop_mode: Option<bool>,
    pub allow_close: Option<bool>,
    pub enter_key_behavior: Option<i64>,
    pub allow_normal_state: Option<bool>,
    pub allow_docked_state: Option<bool>,
    pub allow_autohide_state: Option<bool>,
    pub allow_float_state: Option<bool>,
    pub window_dock_location: Option<i64>,
    /// `<Style ref="uuid"/>`: the style UUID, empty for `<Style/>` (no style).
    pub style_ref: Option<String>,
    pub default_control_id: Option<i64>,
}

/// Comparable semantic model of one `Form.xml`.
#[derive(Debug, Clone)]
pub struct FormXml {
    /// `baseline` / `edited`, used in messages.
    pub label: String,
    pub name: String,
    pub form_caption: Vec<(String, String)>,
    pub form_width: Option<i64>,
    pub form_height: Option<i64>,
    pub form_events: Vec<(i64, String)>,
    pub form_props: FormRootProps,
    /// Non-main form attributes with deterministic ValueType patterns.
    pub attributes: Vec<FormAttribute>,
    /// Controls in document order.
    pub controls: Vec<ControlXml>,
    /// The `Form` element with every control replaced by a [`CONTROL_STUB`] (`id` attribute only).
    pub shell: XmlElement,
}

impl FormXml {
    pub fn control(&self, id: i64) -> Option<&ControlXml> {
        self.controls.iter().find(|c| c.id == id)
    }

    /// The model where the `Page` geometry (`Left`/`Top`/`Width`/`Height`) of the resized panels holds the values
    /// the decoder derives ([`PatchedForm::derived_pages`]). Page geometry is not user-editable; compare a
    /// requested model with a decoded one only after applying this to the requested one. A page whose name does
    /// not match is left untouched, so a real mismatch still shows up as a difference.
    pub fn with_derived_pages(&self, derived: &[DerivedPage]) -> FormXml {
        let mut out = self.clone();
        let root_mine: Vec<&DerivedPage> = derived.iter().filter(|d| d.panel_id == 0).collect();
        if !root_mine.is_empty() {
            for items in out.shell.children.iter_mut().filter(|e| e.tag == "ChildItems") {
                let mut pages = items.children.iter_mut().filter(|e| e.tag == "Page");
                for d in &root_mine {
                    let Some(page) = pages.next().filter(|p| p.attrs.get("name") == Some(&d.name)) else { continue };
                    for (tag, v) in [("Left", d.left), ("Top", d.top), ("Width", d.width), ("Height", d.height)] {
                        if let Some(e) = page.children.iter_mut().find(|e| e.tag == tag) {
                            e.text = v.to_string();
                        }
                    }
                }
            }
        }
        for c in &mut out.controls {
            let mine: Vec<&DerivedPage> = derived.iter().filter(|d| d.panel_id == c.id).collect();
            for items in c.rest.children.iter_mut().filter(|e| e.tag == "ChildItems") {
                let mut pages = items.children.iter_mut().filter(|e| e.tag == "Page");
                for d in &mine {
                    let Some(page) = pages.next().filter(|p| p.attrs.get("name") == Some(&d.name)) else { continue };
                    for (tag, v) in [("Left", d.left), ("Top", d.top), ("Width", d.width), ("Height", d.height)] {
                        if let Some(e) = page.children.iter_mut().find(|e| e.tag == tag) {
                            e.text = v.to_string();
                        }
                    }
                }
            }
        }
        out
    }

    /// The model where controls added by the structural patcher carry their allocated stream ids
    /// (control `id` attributes, `parent` references and shell stubs rewritten by `map`).
    pub fn with_allocated_ids(&self, map: &[(i64, i64)]) -> FormXml {
        let mut out = self.clone();
        let lookup = |id: i64| map.iter().find(|(from, _)| *from == id).map(|(_, to)| *to).unwrap_or(id);
        for c in &mut out.controls {
            let new_id = lookup(c.id);
            c.id = new_id;
            c.rest.attrs.insert("id".into(), new_id.to_string());
            c.parent = c.parent.map(lookup);
        }
        fn stubs(el: &mut XmlElement, lookup: &dyn Fn(i64) -> i64) {
            if el.tag == CONTROL_STUB {
                if let Some(v) = el.attrs.get_mut("id") {
                    if let Ok(id) = v.parse::<i64>() {
                        *v = lookup(id).to_string();
                    }
                }
            }
            for c in &mut el.children {
                stubs(c, lookup);
            }
        }
        stubs(&mut out.shell, &lookup);
        out
    }

    /// The model with the controls of every `Page` ordered by `(ZOrder, id)`, the order in which the decoder's
    /// XML writer lists them. A `ZOrder` edit therefore moves a control inside its page when the patched stream
    /// is extracted again; compare a requested model with a decoded one only after applying this to it.
    pub fn in_decoder_order(&self) -> FormXml {
        fn reorder(el: &mut XmlElement, z: &HashMap<i64, i64>) {
            let in_page = el.tag == "Page";
            for child in &mut el.children {
                if in_page && child.tag == "ChildItems" {
                    let key = |c: &XmlElement| {
                        let id = c.attrs.get("id").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
                        (z.get(&id).copied().unwrap_or(0), id)
                    };
                    child.children.sort_by_key(key);
                }
                reorder(child, z);
            }
        }
        let z: HashMap<i64, i64> = self.controls.iter().map(|c| (c.id, c.values.z_order)).collect();
        let mut out = self.clone();
        out.form_props = FormRootProps::default();
        for a in &mut out.attributes {
            a.non_negative = false;
            a.fixed = false;
        }
        reorder(&mut out.shell, &z);
        for c in &mut out.controls {
            reorder(&mut c.rest, &z);
            if let Some(vt) = &mut c.values.value_type {
                vt.non_negative = false;
                vt.fixed = false;
            }
            // Write-only flags and anchors (verified in binary by read_back, not emitted by render_form_xml
            // so existing golden Form.xml files stay byte-identical) are cleared before XML comparison.
            c.values.drop_list_button = None;
            c.values.spin_button = None;
            c.values.auto_choice_incomplete = None;
            c.values.auto_cell_height = None;
            c.values.anchors = None;
            c.values.modifies_data = None;
            c.values.skip_on_input = None;
            c.values.first_in_group = None;
            c.values.multi_line = None;
            c.values.password_mode = None;
            c.values.extended_edit = None;
            c.values.mask = None;
            c.values.choose_type = None;
            c.values.text_edit = None;
            c.values.vertical_align = None;
            c.values.type_link = None;
            c.values.field_back_color = None;
            c.values.field_text_color = None;
            c.values.button_back_color = None;
            c.values.button_text_color = None;
            c.values.picture_location = None;
            c.values.picture_size = None;
            c.values.format.clear();
            c.values.selection_value = None;
            c.values.choice_list_width = None;
            c.values.min_value = None;
            c.values.max_value = None;
            c.values.separate_by_page = None;
            c.values.auto_tab_order = None;
            c.values.scroll_page_mode = None;
            c.values.orientation = None;
            c.values.secondary = None;
            c.values.zoomable = None;
            c.values.use_context_menu = None;
            c.values.nonselected_picture_text.clear();
            c.values.enable_start_drag = None;
            c.values.enable_drag = None;
            c.values.choice_mode = None;
            c.values.show_picture = None;
            c.values.show_check_box = None;
            c.values.step = None;
            c.values.large_step = None;
            c.values.marking_step = None;
            c.values.marking_appearance = None;
            c.values.show_style = None;
            c.values.show_percent = None;
            c.values.selection_back_color = None;
            c.values.selection_text_color = None;
            c.values.holiday_color = None;
            c.values.show_current_date = None;
            c.values.calendar_navigation = None;
            c.values.use_alternation_row_color = None;
            c.values.header_back_color = None;
            c.values.header_text_color = None;
            c.values.footer_back_color = None;
            c.values.footer_text_color = None;
            c.values.alternation_row_back_color = None;
            c.values.fixed_left = None;
            c.values.fixed_right = None;
            c.values.row_input_mode = None;
            c.values.row_selection_mode = None;
            c.values.header_height = None;
            c.values.footer_height = None;
            c.values.header_font = None;
            c.values.footer_font = None;
            if c.type_name == "LabelDecoration" {
                if let Some(pic) = &c.values.picture {
                    c.rest.children.retain(|ch| !(ch.tag == "Picture" && ch.attrs.is_empty()));
                    if pic.kind == 1 && pic.ref_guid.as_deref().is_some_and(|g| g != "00000000-0000-0000-0000-000000000000") {
                        let mut marker = XmlElement::new("Picture", 0);
                        marker.text = "true".to_string();
                        if let Some(pos) = c.rest.children.iter().position(|ch| ch.tag == "Events") {
                            c.rest.children.insert(pos, marker);
                        } else {
                            c.rest.children.push(marker);
                        }
                    }
                }
            }
            c.values.shortcut = None;
            c.values.picture = None;
            c.values.choice_button_picture = None;
            c.values.menu_mode = None;
            c.values.scrolling_text = None;
            c.values.label_picture_location = None;
            c.values.panel_picture_location = None;
            c.values.choice_list_height = None;
            c.values.show_selection = None;
            c.values.begin_of_display_period = None;
            c.values.end_of_display_period = None;
            for it in &mut c.values.choice_list {
                it.checked = None;
                it.picture_id = None;
            }
            c.values.default_button = None;
            c.values.auto_context_menu = None;
            c.values.collapse = None;
            c.values.embedded_into_cell = None;
            c.values.print = None;
            c.values.protection = None;
            c.values.show_grid = None;
            c.values.show_headers = None;
            c.values.show_groups = None;
            c.values.black_and_white_view = None;
            c.values.totals_right = None;
            c.values.totals_below = None;
            c.values.print_scale = None;
            c.values.fit_to_page = None;
            c.values.black_and_white = None;
            c.values.collate = None;
            c.values.copies = None;
            c.values.per_page = None;
            c.values.page_orientation = None;
            c.values.bottom_margin = None;
            c.values.top_margin = None;
            c.values.left_margin = None;
            c.values.right_margin = None;
            c.values.header_size = None;
            c.values.footer_size = None;
            c.values.duplex_printing = None;
            c.values.print_accuracy = None;
            c.values.step_direction = None;
            c.values.save_view_properties = None;
            c.values.fixed_background = None;
            for page in &mut c.values.pages {
                page.title_picture = None;
            }
            c.values.buttons = buttons_without_extras(&c.values.buttons);
            for col in &mut c.values.columns {
                col.read_only |= location_reads_as_read_only(&col.ext);
                col.ext = ColumnExt::default();
            }
            if crate::oof_props::app_path(&c.type_name).is_none() && patch_app_path(&c.type_name).is_some() {
                c.values.enabled = true;
                c.values.transparent_back = false;
                c.values.back_color = None;
                c.values.text_color = None;
                c.values.border_color = None;
                c.values.font = None;
                c.values.tooltip.clear();
            }
            // The decoder writes only `bold`, `source`, `face` and `size` of a font; a font that keeps none of
            // them is written as an opaque `<Font>` element.
            if let Some(f) = c.values.font.take() {
                let plain = FontSpec { italic: false, underline: false, strikeout: false, scale: None, ..f.clone() };
                if plain.bold || plain.face.is_some() || plain.size.is_some() {
                    c.values.font = Some(plain);
                } else {
                    let mut opaque = XmlElement::new("Font", 0);
                    if f.italic || f.underline || f.strikeout {
                        opaque.attrs.insert("source".into(), "bin".into());
                    }
                    c.rest.children.push(opaque);
                }
            }
            match c.type_name.as_str() {
                "LabelDecoration" => c.values.mark_negatives = false,
                "PictureField" | "PictureDecoration" => c.values.hyperlink = false,
                "RadioButton" => {
                    c.values.title_location.clear();
                    c.values.value_type = None;
                }
                "ChoiceField" => {
                    c.values.choice_button = None;
                    c.values.clear_button = None;
                    c.values.spin_button = None;
                    c.values.open_button = None;
                    c.values.read_only = false;
                }
                "ListBox" | "GraphicalSchemaField" | "TextDocumentField" => {
                    c.values.read_only = false;
                }
                "SpreadsheetDocumentField" => {
                    c.values.read_only = false;
                    c.values.horizontal_scroll_bar.clear();
                    c.values.vertical_scroll_bar.clear();
                }
                "CalendarField" => c.values.selection_mode.clear(),
                _ => {}
            }
            if c.type_name != "UsualGroup" {
                c.values.border_style = None;
            }
            if matches!(c.type_name.as_str(), "SpreadsheetDocumentField" | "HTMLDocumentField") {
                c.values.border_color = None;
            }
            if matches!(
                c.type_name.as_str(),
                "GraphicalSchemaField" | "TextDocumentField" | "HTMLDocumentField" | "SpreadsheetDocumentField"
            ) {
                c.values.output = None;
            }
            if matches!(c.type_name.as_str(), "InputField" | "Button" | "CheckBox" | "RadioButton") {
                c.values.horizontal_align = String::new();
            }
            if c.type_name == "Table" {
                c.values.header = None;
                c.values.vertical_lines = None;
                c.values.horizontal_lines = None;
                c.values.auto_insert_new_row = None;
                c.values.change_setting_of_columns = None;
                c.values.change_position_of_columns = None;
                c.values.change_row_set = None;
                c.values.initial_list_view = None;
                c.values.output = None;
                c.values.table_read_only = None;
                c.values.horizontal_scroll_bar.clear();
                c.values.vertical_scroll_bar.clear();
            }
        }
        out
    }
}

/// Appearance-block location for every control type whose appearance (and extended catalog colors) can be patched.
fn patch_app_path(type_name: &str) -> Option<&'static [i64]> {
    Some(match type_name {
        "LabelDecoration" | "UsualGroup" | "Button" | "Panel" | "CommandBar" | "ChoiceField" | "PictureDecoration"
        | "PictureField" | "ListBox" | "TrackBar" | "ProgressBar" | "Splitter" | "CalendarField" => &[1, 0],
        "TextDocumentField" | "GraphicalSchemaField" => &[0],
        "GeographicalSchemaField" => &[],
        "InputField" | "RadioButton" => &[2, 0, 0],
        "CheckBox" => &[1, 0, 0],
        "Table" | "TableBox" => &[2, 0],
        _ => return None,
    })
}

fn tag_supported_for(type_name: &str, tag: &str) -> bool {
    match tag {
        "DataPath" | "Caption" | "Left" | "Top" | "Width" | "Height" | "Visible" | "ZOrder" | "TabOrder" | "Anchors"
        | "ModifiesData" | "SkipOnInput" | "FirstInGroup" => true,
        "PasswordMode" | "ExtendedEdit" | "Mask" | "ChooseType" | "TextEdit" | "TypeLink" => type_name == "InputField",
        "MultiLine" => matches!(type_name, "InputField" | "Button"),
        "VerticalAlign" => matches!(type_name, "InputField" | "Button" | "LabelDecoration" | "CheckBox" | "RadioButton"),
        "Events" => crate::donor::has_event_list(type_name),
        "Enabled" | "TransparentBack" | "BackColor" | "TextColor" | "BorderColor" | "Font" | "Tooltip" | "FieldBackColor"
        | "FieldTextColor" | "ButtonBackColor" | "ButtonTextColor" => {
            patch_app_path(type_name).is_some()
                || (tag == "BorderColor" && matches!(type_name, "SpreadsheetDocumentField" | "HTMLDocumentField"))
        }
        "HorizontalScrollBar" | "VerticalScrollBar" if matches!(type_name, "Table" | "SpreadsheetDocumentField") => true,
        "ReadOnly"
            if matches!(type_name, "Table" | "ListBox" | "GraphicalSchemaField" | "TextDocumentField" | "SpreadsheetDocumentField") =>
        {
            true
        }
        "Output"
            if matches!(
                type_name,
                "Table" | "GraphicalSchemaField" | "TextDocumentField" | "HTMLDocumentField" | "SpreadsheetDocumentField"
            ) =>
        {
            true
        }
        "Header" | "HorizontalLines" | "VerticalLines" | "AutoInsertNewRow" | "ChangeRowSet" | "ChangePositionOfColumns"
        | "ChangeSettingOfColumns" | "InitialListView" => type_name == "Table",
        "ReadOnly" | "ChoiceButton" | "ClearButton" | "SpinButton" | "OpenButton" if type_name == "ChoiceField" => true,
        "ValueType" if type_name == "RadioButton" => true,
        "MarkNegatives" if type_name == "LabelDecoration" => true,
        "ValueType" | "HorizontalScrollBar" | "VerticalScrollBar" | "ReadOnly" | "ChoiceButton" | "ClearButton"
        | "OpenButton" | "DropListButton" | "SpinButton" | "AutoMarkIncomplete" | "AutoChoiceIncomplete"
        | "MarkNegatives" | "AutoCellHeight" | "ChoiceListMode" => type_name == "InputField",
        "HorizontalAlign" => matches!(
            type_name,
            "LabelDecoration" | "CommandBar" | "InputField" | "Button" | "CheckBox" | "RadioButton"
        ),
        "Hyperlink" => matches!(type_name, "LabelDecoration" | "PictureField" | "PictureDecoration"),
        "ThreeState" => type_name == "CheckBox",
        "TitleLocation" => matches!(type_name, "CheckBox" | "RadioButton"),
        "ChoiceList" => type_name == "ChoiceField",
        "ShowTabs" => type_name == "Panel",
        "SelectionMode" if type_name == "CalendarField" => true,
        "Footer" | "SelectionMode" | "InitialTreeView" | "ChoiceMode" => type_name == "Table",
        "AutoFill" | "Buttons" => type_name == "CommandBar",
        "BorderStyle" => {
            type_name == "UsualGroup"
                || patch_app_path(type_name).is_some()
                || matches!(type_name, "SpreadsheetDocumentField" | "HTMLDocumentField")
        }
        "PictureLocation" => type_name == "Button",
        "PictureSize" => matches!(type_name, "Button" | "LabelDecoration" | "PictureField" | "PictureDecoration" | "Panel"),
        "Format" => matches!(type_name, "LabelDecoration" | "InputField"),
        "SelectionValue" => type_name == "RadioButton",
        "ChoiceListWidth" => matches!(type_name, "InputField" | "ChoiceField"),
        "MinValue" | "MaxValue" => matches!(type_name, "InputField" | "TrackBar" | "ProgressBar"),
        "SeparateByPage" | "AutoTabOrder" | "ScrollPageMode" => type_name == "Panel",
        "Orientation" => matches!(type_name, "CommandBar" | "Splitter" | "TrackBar" | "ProgressBar"),
        "Secondary" => type_name == "CommandBar",
        "Zoomable" | "UseContextMenu" | "NonselectedPictureText" => matches!(type_name, "PictureDecoration" | "PictureField"),
        "EnableStartDrag" | "EnableDrag" => {
            matches!(
                type_name,
                "PictureDecoration" | "PictureField" | "CalendarField" | "Table" | "SpreadsheetDocumentField"
            )
        }
        "UseAlternationRowColor" | "HeaderBackColor" | "HeaderTextColor" | "FooterBackColor" | "FooterTextColor"
        | "AlternationRowBackColor" | "FixedLeft" | "FixedRight" | "RowInputMode" | "RowSelectionMode" | "HeaderHeight"
        | "FooterHeight" | "HeaderFont" | "FooterFont" => type_name == "Table",
        "ShowPicture" | "ShowCheckBox" => type_name == "ListBox",
        "Step" => matches!(type_name, "TrackBar" | "ProgressBar"),
        "LargeStep" | "MarkingStep" | "MarkingAppearance" => type_name == "TrackBar",
        "ShowStyle" | "ShowPercent" => type_name == "ProgressBar",
        "SelectionBackColor" | "SelectionTextColor" => matches!(type_name, "CalendarField" | "Table"),
        "HolidayColor" | "ShowCurrentDate" | "CalendarNavigation" => type_name == "CalendarField",
        "Shortcut" => matches!(
            type_name,
            "Button" | "LabelDecoration" | "PictureField" | "PictureDecoration" | "InputField" | "ChoiceField"
        ),
        "Picture" => matches!(
            type_name,
            "Button" | "InputField" | "ChoiceField" | "LabelDecoration" | "PictureField" | "PictureDecoration" | "Panel"
        ),
        "ChoiceButtonPicture" | "ChoiceListHeight" => matches!(type_name, "InputField" | "ChoiceField"),
        "MenuMode" => type_name == "Button",
        "ScrollingText" | "LabelPictureLocation" => type_name == "LabelDecoration",
        "PanelPictureLocation" => type_name == "Panel",
        "ShowSelection" => type_name == "SpreadsheetDocumentField",
        "BeginOfDisplayPeriod" | "EndOfDisplayPeriod" => type_name == "CalendarField",
        "DefaultButton" => type_name == "Button",
        "ContextMenu" => type_name != "CommandBar",
        "ActionSource" => type_name == "CommandBar",
        "AutoContextMenu" | "Collapse" | "EmbeddedIntoCell" | "Print" => patch_app_path(type_name).is_some(),
        "Protection" | "ShowGrid" | "ShowHeaders" | "ShowGroups" | "BlackAndWhiteView" | "TotalsRight" | "TotalsBelow"
        | "PrintScale" | "FitToPage" | "BlackAndWhite" | "Collate" | "Copies" | "PerPage" | "PageOrientation"
        | "BottomMargin" | "TopMargin" | "LeftMargin" | "RightMargin" | "HeaderSize" | "FooterSize" | "DuplexPrinting"
        | "PrintAccuracy" | "StepDirection" | "SaveViewProperties" | "FixedBackground" => type_name == "SpreadsheetDocumentField",
        _ => false,
    }
}

fn is_control_tag(tag: &str) -> bool {
    tag == UNKNOWN_CONTROL || CONTROL_GUIDS.iter().any(|(_, n)| *n == tag)
}

fn qname(ns: Option<&str>, name: &str) -> String {
    match ns {
        Some(ns) if !ns.is_empty() => format!("{{{ns}}}{name}"),
        _ => name.to_string(),
    }
}

fn convert(n: roxmltree::Node, doc: &roxmltree::Document) -> XmlElement {
    let tag = qname(n.tag_name().namespace(), n.tag_name().name());
    let mut e = XmlElement::new(&tag, doc.text_pos_at(n.range().start).row);
    for a in n.attributes() {
        e.attrs.insert(qname(a.namespace(), a.name()), a.value().to_string());
    }
    for c in n.children() {
        if c.is_element() {
            e.children.push(convert(c, doc));
        } else if c.is_text() {
            e.text.push_str(c.text().unwrap_or(""));
        }
    }
    if !e.children.is_empty() {
        e.text = e.text.trim().to_string();
    }
    e
}

struct Parser<'a> {
    label: &'a str,
    form: String,
    attr_names: HashMap<i64, String>,
    controls: Vec<ControlXml>,
}

impl Parser<'_> {
    fn bad(&self, line: u32, message: String) -> ExtractionError {
        fail(&self.form, self.label, vec![problem("invalid-xml", format!("line {line}: {message}"))])
    }

    fn identity(&self, c: &XmlElement) -> Result<(i64, String, Option<String>)> {
        let id = c
            .attrs
            .get("id")
            .and_then(|v| v.trim().parse::<i64>().ok())
            .ok_or_else(|| self.bad(c.line, format!("<{}> has a missing or non-integer 'id' attribute", c.tag)))?;
        let name = c
            .attrs
            .get("name")
            .cloned()
            .ok_or_else(|| self.bad(c.line, format!("<{}> id {id} has no 'name' attribute", c.tag)))?;
        let guid = c.attrs.get("guid").cloned();
        if c.tag == UNKNOWN_CONTROL && guid.is_none() {
            return Err(self.bad(c.line, format!("UnknownControl '{name}' (id {id}) has no 'guid' attribute")));
        }
        Ok((id, name, guid))
    }

    /// Clone of `el` where control children of a `ChildItems` become stubs and are registered.
    fn walk(&mut self, el: &XmlElement, parent: Option<i64>, page: Option<&str>) -> Result<XmlElement> {
        let mut shell = XmlElement { children: Vec::new(), ..el.clone() };
        for c in &el.children {
            if el.tag == "ChildItems" && is_control_tag(&c.tag) {
                let (id, name, guid) = self.identity(c)?;
                let slot = self.controls.len();
                self.controls.push(ControlXml {
                    id,
                    name,
                    type_name: c.tag.clone(),
                    guid,
                    parent,
                    page: page.map(str::to_string),
                    line: c.line,
                    values: ControlValues::empty(),
                    rest: XmlElement::new(&c.tag, c.line),
                });
                let inner = self.walk(c, Some(id), None)?;
                let (values, rest) = self.split_values(slot, &inner)?;
                self.controls[slot].values = values;
                self.controls[slot].rest = rest;
                let mut stub = XmlElement::new(CONTROL_STUB, c.line);
                stub.attrs.insert("id".into(), id.to_string());
                shell.children.push(stub);
            } else {
                let pg = if c.tag == "Page" { c.attrs.get("name").map(String::as_str) } else { page };
                shell.children.push(self.walk(c, parent, pg)?);
            }
        }
        Ok(shell)
    }

    fn split_values(&self, slot: usize, inner: &XmlElement) -> Result<(ControlValues, XmlElement)> {
        let who = self.controls[slot].describe();
        let type_name = self.controls[slot].type_name.clone();
        let ctrl_name = self.controls[slot].name.clone();
        let mut rest = XmlElement { children: Vec::new(), ..inner.clone() };
        rest.attrs.remove("name");
        let mut found: HashMap<&str, &XmlElement> = HashMap::new();
        for c in &inner.children {
            // `<Picture>true</Picture>` is the decoder's marker that a label has a picture, not a `PictureSpec`.
            let marker = c.tag == "Picture" && !c.attrs.contains_key("kind");
            if let Some(tag) = SUPPORTED_TAGS.iter().find(|t| !marker && **t == c.tag && tag_supported_for(&type_name, t)) {
                if found.insert(tag, c).is_some() {
                    return Err(self.bad(c.line, format!("{who}: duplicate <{tag}> element")));
                }
            } else {
                rest.children.push(c.clone());
            }
        }
        let int_of = |tag: &str, required: bool| -> Result<i64> {
            match found.get(tag) {
                None if required => Err(self.bad(inner.line, format!("{who}: required <{tag}> element is missing"))),
                None => Ok(0),
                Some(e) => {
                    if !e.children.is_empty() || !e.attrs.is_empty() {
                        return Err(self.bad(e.line, format!("{who}: <{tag}> must be a plain integer")));
                    }
                    e.text
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| self.bad(e.line, format!("{who}: <{tag}> is not an integer: '{}'", e.text.trim())))
                }
            }
        };
        let opt_bool_of = |tag: &str| -> Result<Option<bool>> {
            let Some(e) = found.get(tag) else { return Ok(None) };
            match e.text.trim() {
                "true" if e.children.is_empty() && e.attrs.is_empty() => Ok(Some(true)),
                "false" if e.children.is_empty() && e.attrs.is_empty() => Ok(Some(false)),
                other => Err(self.bad(e.line, format!("{who}: <{tag}> must be true or false, got '{other}'"))),
            }
        };
        let bool_of = |tag: &str, default: bool| -> Result<bool> { Ok(opt_bool_of(tag)?.unwrap_or(default)) };
        let opt_int_of = |tag: &str| -> Result<Option<i64>> {
            if found.contains_key(tag) { int_of(tag, false).map(Some) } else { Ok(None) }
        };
        let loc_of = |tag: &str| -> Result<Vec<(String, String)>> {
            let mut items: Vec<(String, String)> = Vec::new();
            if let Some(e) = found.get(tag) {
                if !e.attrs.is_empty() || !e.text.is_empty() {
                    return Err(self.bad(e.line, format!("{who}: <{tag}> must contain only <Item language=\"..\"> elements")));
                }
                for it in &e.children {
                    let lang = it.attrs.get("language");
                    if it.tag != "Item" || it.attrs.len() != 1 || lang.is_none() || !it.children.is_empty() {
                        return Err(self.bad(it.line, format!("{who}: unsupported <{tag}> structure (<{}>)", it.tag)));
                    }
                    let lang = lang.cloned().unwrap_or_default();
                    if items.iter().any(|(l, _)| *l == lang) {
                        return Err(self.bad(it.line, format!("{who}: duplicate {tag} language '{lang}'")));
                    }
                    items.push((lang, it.text.clone()));
                }
            }
            Ok(items)
        };
        let enum_of = |tag: &str, allowed: &[&str], default: &str| -> Result<String> {
            let Some(e) = found.get(tag) else { return Ok(String::new()) };
            if !e.children.is_empty() || !e.attrs.is_empty() {
                return Err(self.bad(e.line, format!("{who}: <{tag}> must be a plain text value")));
            }
            let v = e.text.trim();
            if !allowed.contains(&v) {
                return Err(self.bad(e.line, format!("{who}: <{tag}> must be one of {allowed:?}, got '{v}'")));
            }
            Ok(if v == default { String::new() } else { v.to_string() })
        };
        let opt_enum_of = |tag: &str, allowed: &[&str]| -> Result<Option<String>> {
            if found.contains_key(tag) {
                Ok(Some(enum_of(tag, allowed, " ")?))
            } else {
                Ok(None)
            }
        };
        let scroll_default = if matches!(type_name.as_str(), "Table" | "SpreadsheetDocumentField") {
            "\0"
        } else {
            "DontUse"
        };
        let is_inp = type_name == "InputField";
        let horizontal_align = match type_name.as_str() {
            "LabelDecoration" => enum_of("HorizontalAlign", &["Left", "Center", "Right"], "Left")?,
            "CommandBar" => enum_of("HorizontalAlign", &["Left", "Right"], "Left")?,
            "InputField" | "Button" | "CheckBox" | "RadioButton" => {
                enum_of("HorizontalAlign", &["Auto", "Left", "Center", "Right"], "")?
            }
            _ => String::new(),
        };
        let vertical_align = match found.get("VerticalAlign") {
            None => None,
            Some(e) => {
                let v = e.text.trim();
                if !e.children.is_empty() || !e.attrs.is_empty() || !["Top", "Center", "Bottom"].contains(&v) {
                    return Err(self.bad(e.line, format!("{who}: <VerticalAlign> must be one of Top, Center, Bottom, got '{v}'")));
                }
                Some(v.to_string())
            }
        };
        let mask = match found.get("Mask") {
            None => None,
            Some(e) => {
                if !e.children.is_empty() || !e.attrs.is_empty() {
                    return Err(self.bad(e.line, format!("{who}: <Mask> must be a plain text value")));
                }
                Some(e.text.clone())
            }
        };
        let type_link = match found.get("TypeLink") {
            None => None,
            Some(e) => {
                let path = e.text.trim();
                let attr_id = e.attrs.get("attributeId").and_then(|v| v.trim().parse::<i64>().ok()).filter(|v| *v >= 0);
                let field_guid = e.attrs.get("fieldGuid").map(|g| g.trim().to_string());
                let attrs_ok = e.attrs.keys().all(|k| k == "attributeId" || k == "fieldGuid");
                let Some(attr_id) = attr_id.filter(|_| attrs_ok && e.children.is_empty() && !path.is_empty()) else {
                    return Err(self.bad(
                        e.line,
                        format!("{who}: <TypeLink> needs a non-negative attributeId, optional fieldGuid and a non-empty path"),
                    ));
                };
                if field_guid.as_deref().is_some_and(|g| !is_uuid(g)) {
                    return Err(self.bad(e.line, format!("{who}: <TypeLink> fieldGuid is not a UUID")));
                }
                if field_guid.is_none() && !self.attr_names.is_empty() && self.attr_names.get(&attr_id).map(String::as_str) != Some(path) {
                    return Err(self.bad(e.line, format!("{who}: <TypeLink> attribute {attr_id} does not match path '{path}'")));
                }
                Some(TypeLinkSpec { attr_id, field_guid, path: path.to_string() })
            }
        };
        let show_tabs = if type_name == "Panel" {
            match found.get("ShowTabs") {
                None => Some(2),
                Some(e) => {
                    if !e.children.is_empty() || !e.text.is_empty() {
                        return Err(self.bad(e.line, format!("{who}: <ShowTabs> must be an empty element with a 'code' attribute")));
                    }
                    let code = e
                        .attrs
                        .get("code")
                        .and_then(|v| v.trim().parse::<i64>().ok())
                        .ok_or_else(|| self.bad(e.line, format!("{who}: <ShowTabs> has a missing or non-integer 'code' attribute")))?;
                    Some(code)
                }
            }
        } else {
            None
        };
        let mut events: Vec<(i64, String)> = Vec::new();
        if let Some(e) = found.get("Events") {
            let mut valid = e.attrs.is_empty() && e.text.trim().is_empty();
            if valid {
                for ev in &e.children {
                    let Some(tag) = ev.attrs.get("id").and_then(|v| v.trim().parse::<i64>().ok()) else {
                        valid = false;
                        break;
                    };
                    let expected_tag = crate::oof::event_name(Some(&type_name), tag);
                    let handler = ev.text.trim();
                    if ev.attrs.len() != 1 || !ev.children.is_empty() || handler.is_empty() || ev.tag != expected_tag {
                        valid = false;
                        break;
                    }
                    events.push((tag, handler.to_string()));
                }
            }
            if !valid {
                events.clear();
                rest.children.push((*e).clone());
            }
        }
        let parse_loc_elem = |e: &XmlElement| -> Option<Vec<(String, String)>> {
            if !e.attrs.is_empty() || !e.text.trim().is_empty() {
                return None;
            }
            let mut items = Vec::new();
            for it in &e.children {
                let lang = it.attrs.get("language")?;
                if it.tag != "Item" || it.attrs.len() != 1 || !it.children.is_empty() {
                    return None;
                }
                if items.iter().any(|(l, _)| l == lang) {
                    return None;
                }
                items.push((lang.clone(), it.text.clone()));
            }
            Some(items)
        };
        let parse_bool_elem = |e: &XmlElement| -> Option<bool> {
            if !e.children.is_empty() || !e.attrs.is_empty() {
                return None;
            }
            match e.text.trim() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        };
        let mut pages: Vec<PageValues> = Vec::new();
        if type_name == "Panel" {
            if let Some(ci) = rest.children.iter_mut().find(|e| e.tag == "ChildItems") {
                let mut ok = true;
                let mut extracted = Vec::new();
                for p in &ci.children {
                    if p.tag != "Page" {
                        continue;
                    }
                    let Some(pname) = p.attrs.get("name").filter(|n| !n.trim().is_empty()).cloned() else {
                        ok = false;
                        break;
                    };
                    let mut title = Vec::new();
                    let mut visible = true;
                    let mut enabled = true;
                    let mut title_picture = None;
                    let mut seen_tags = std::collections::HashSet::new();
                    for ch in &p.children {
                        match ch.tag.as_str() {
                            "Title" => {
                                if !seen_tags.insert("Title") {
                                    ok = false;
                                    break;
                                }
                                let Some(t) = parse_loc_elem(ch) else {
                                    ok = false;
                                    break;
                                };
                                title = t;
                            }
                            "Visible" => {
                                if !seen_tags.insert("Visible") {
                                    ok = false;
                                    break;
                                }
                                let Some(v) = parse_bool_elem(ch) else {
                                    ok = false;
                                    break;
                                };
                                visible = v;
                            }
                            "Enabled" => {
                                if !seen_tags.insert("Enabled") {
                                    ok = false;
                                    break;
                                }
                                let Some(v) = parse_bool_elem(ch) else {
                                    ok = false;
                                    break;
                                };
                                enabled = v;
                            }
                            "TitlePicture" => {
                                let Some(pic) = seen_tags.insert("TitlePicture").then(|| parse_picture_elem(ch)).flatten() else {
                                    ok = false;
                                    break;
                                };
                                title_picture = Some(pic);
                            }
                            _ => {}
                        }
                    }
                    if !ok {
                        break;
                    }
                    extracted.push(PageValues { name: pname, title, visible, enabled, title_picture });
                }
                if ok {
                    for p in &mut ci.children {
                        if p.tag == "Page" {
                            p.children.retain(|ch| !matches!(ch.tag.as_str(), "Title" | "Visible" | "Enabled" | "TitlePicture"));
                        }
                    }
                    pages = extracted;
                }
            }
        }
        let mut columns: Vec<ColumnValues> = Vec::new();
        if type_name == "Table" {
            if let Some(cols_el) = rest.children.iter_mut().find(|e| e.tag == "Columns") {
                let mut ok = cols_el.attrs.is_empty() && cols_el.text.trim().is_empty();
                let mut extracted = Vec::new();
                let mut seen_col_names = std::collections::HashSet::new();
                if ok {
                    for col_el in &cols_el.children {
                        if col_el.tag != "Column" {
                            ok = false;
                            break;
                        }
                        let Some(cname) = col_el.attrs.get("name").filter(|n| !n.trim().is_empty()).cloned() else {
                            ok = false;
                            break;
                        };
                        if !seen_col_names.insert(cname.clone()) {
                            ok = false;
                            break;
                        }
                        let cid = match col_el.attrs.get("id") {
                            Some(s) => match s.trim().parse::<i64>() {
                                Ok(v) => Some(v),
                                Err(_) => {
                                    ok = false;
                                    break;
                                }
                            },
                            None => None,
                        };
                        if col_el.attrs.keys().any(|k| k != "name" && k != "id") {
                            ok = false;
                            break;
                        }
                        let mut field_name: Option<String> = None;
                        let mut header = Vec::new();
                        let mut footer_text = Vec::new();
                        let mut tooltip = Vec::new();
                        let mut width: Option<i64> = None;
                        let mut visible = true;
                        let mut read_only = false;
                        let mut editor: Option<String> = None;
                        let mut col_events: Vec<(i64, String)> = Vec::new();
                        let mut ext = ColumnExt::default();
                        let mut seen_tags = std::collections::HashSet::new();
                        for ch in &col_el.children {
                            match ch.tag.as_str() {
                                "DataPath" => {
                                    if !seen_tags.insert("DataPath") || !ch.children.is_empty() || ch.text.trim().is_empty() {
                                        ok = false;
                                        break;
                                    }
                                    let p = ch.text.trim();
                                    field_name = Some(p.rsplit('.').next().unwrap_or(p).to_string());
                                }
                                "Header" => {
                                    if !seen_tags.insert("Header") {
                                        ok = false;
                                        break;
                                    }
                                    let Some(t) = parse_loc_elem(ch) else {
                                        ok = false;
                                        break;
                                    };
                                    header = t;
                                }
                                "FooterText" => {
                                    if !seen_tags.insert("FooterText") {
                                        ok = false;
                                        break;
                                    }
                                    let Some(t) = parse_loc_elem(ch) else {
                                        ok = false;
                                        break;
                                    };
                                    footer_text = t;
                                }
                                "Tooltip" => {
                                    if !seen_tags.insert("Tooltip") {
                                        ok = false;
                                        break;
                                    }
                                    let Some(t) = parse_loc_elem(ch) else {
                                        ok = false;
                                        break;
                                    };
                                    tooltip = t;
                                }
                                "Width" => {
                                    if !seen_tags.insert("Width") || !ch.children.is_empty() || !ch.attrs.is_empty() {
                                        ok = false;
                                        break;
                                    }
                                    let Ok(w) = ch.text.trim().parse::<i64>() else {
                                        ok = false;
                                        break;
                                    };
                                    width = Some(w);
                                }
                                "Visible" => {
                                    if !seen_tags.insert("Visible") {
                                        ok = false;
                                        break;
                                    }
                                    let Some(v) = parse_bool_elem(ch) else {
                                        ok = false;
                                        break;
                                    };
                                    visible = v;
                                }
                                "ReadOnly" => {
                                    if !seen_tags.insert("ReadOnly") {
                                        ok = false;
                                        break;
                                    }
                                    let Some(v) = parse_bool_elem(ch) else {
                                        ok = false;
                                        break;
                                    };
                                    read_only = v;
                                }
                                "Editor" => {
                                    if !seen_tags.insert("Editor") || !ch.children.is_empty() || !ch.attrs.is_empty() {
                                        ok = false;
                                        break;
                                    }
                                    let ed = ch.text.trim();
                                    if !CONTROL_GUIDS.iter().any(|(_, n)| *n == ed) {
                                        ok = false;
                                        break;
                                    }
                                    editor = Some(ed.to_string());
                                }
                                "Events" => {
                                    if !seen_tags.insert("Events") || !ch.attrs.is_empty() || !ch.text.trim().is_empty() {
                                        ok = false;
                                        break;
                                    }
                                    for ev in &ch.children {
                                        let Some(tag) = ev.attrs.get("id").and_then(|v| v.trim().parse::<i64>().ok()) else {
                                            ok = false;
                                            break;
                                        };
                                        let h = ev.text.trim();
                                        if ev.attrs.len() != 1 || !ev.children.is_empty() || h.is_empty() {
                                            ok = false;
                                            break;
                                        }
                                        col_events.push((tag, h.to_string()));
                                    }
                                    if !ok {
                                        break;
                                    }
                                }
                                "ShowHierarchy" | "CheckBoxThreeState" | "ShowInHeader" | "ShowInFooter" | "ChangeSetting"
                                | "ChangePosition" => {
                                    let tag = ch.tag.as_str();
                                    let Some(v) = seen_tags.insert(tag).then(|| parse_bool_elem(ch)).flatten() else {
                                        ok = false;
                                        break;
                                    };
                                    match tag {
                                        "ShowHierarchy" => ext.show_hierarchy = Some(v),
                                        "CheckBoxThreeState" => ext.check_box_three_state = Some(v),
                                        "ShowInHeader" => ext.show_in_header = Some(v),
                                        "ShowInFooter" => ext.show_in_footer = Some(v),
                                        "ChangeSetting" => ext.change_setting = Some(v),
                                        _ => ext.change_position = Some(v),
                                    }
                                }
                                "MarkNegatives" | "ChangeVisible" | "AutoMarkIncomplete" | "AutoCellHeight" | "Hyperlink" => {
                                    let tag = ch.tag.as_str();
                                    let Some(v) = seen_tags.insert(tag).then(|| parse_bool_elem(ch)).flatten() else {
                                        ok = false;
                                        break;
                                    };
                                    match tag {
                                        "MarkNegatives" => ext.mark_negatives = Some(v),
                                        "ChangeVisible" => ext.change_visible = Some(v),
                                        "AutoMarkIncomplete" => ext.auto_mark_incomplete = Some(v),
                                        "AutoCellHeight" => ext.auto_cell_height = Some(v),
                                        _ => ext.hyperlink = Some(v),
                                    }
                                }
                                "CheckData" => {
                                    if !seen_tags.insert("CheckData") || !ch.children.is_empty() || !ch.attrs.is_empty() {
                                        ok = false;
                                        break;
                                    }
                                    let Ok(v) = ch.text.trim().parse::<i64>() else {
                                        ok = false;
                                        break;
                                    };
                                    ext.check_data = Some(v);
                                }
                                "RowsPictures" | "HeaderPicture" | "FooterPicture" => {
                                    let tag = ch.tag.as_str();
                                    let Some(pic) = seen_tags.insert(tag).then(|| parse_picture_elem(ch)).flatten() else {
                                        ok = false;
                                        break;
                                    };
                                    match tag {
                                        "RowsPictures" => ext.rows_pictures = Some(pic),
                                        "HeaderPicture" => ext.header_picture = Some(pic),
                                        _ => ext.footer_picture = Some(pic),
                                    }
                                }
                                "Location" | "SizeChange" | "EditMode" => {
                                    let tag = ch.tag.as_str();
                                    let v = ch.text.trim();
                                    if !seen_tags.insert(tag)
                                        || !ch.children.is_empty()
                                        || !ch.attrs.is_empty()
                                        || column_enum_code(tag, v).is_none()
                                    {
                                        ok = false;
                                        break;
                                    }
                                    let slot = match tag {
                                        "Location" => &mut ext.location,
                                        "SizeChange" => &mut ext.size_change,
                                        _ => &mut ext.edit_mode,
                                    };
                                    *slot = Some(v.to_string());
                                }
                                "FieldBackColor" | "FieldTextColor" | "HeaderBackColor" | "HeaderTextColor" | "FooterBackColor"
                                | "FooterTextColor" => {
                                    let tag = ch.tag.as_str();
                                    let Some(c) = seen_tags.insert(tag).then(|| parse_color_elem(ch)).flatten() else {
                                        ok = false;
                                        break;
                                    };
                                    let slot = match tag {
                                        "FieldBackColor" => &mut ext.field_back_color,
                                        "FieldTextColor" => &mut ext.field_text_color,
                                        "HeaderBackColor" => &mut ext.header_back_color,
                                        "HeaderTextColor" => &mut ext.header_text_color,
                                        "FooterBackColor" => &mut ext.footer_back_color,
                                        _ => &mut ext.footer_text_color,
                                    };
                                    *slot = Some(c);
                                }
                                "TextFont" | "HeaderFont" | "FooterFont" => {
                                    let tag = ch.tag.as_str();
                                    let Some(f) = seen_tags.insert(tag).then(|| parse_font_elem(ch)).flatten() else {
                                        ok = false;
                                        break;
                                    };
                                    let slot = match tag {
                                        "TextFont" => &mut ext.text_font,
                                        "HeaderFont" => &mut ext.header_font,
                                        _ => &mut ext.footer_font,
                                    };
                                    *slot = Some(f);
                                }
                                "HeaderHorizontalAlign" | "HorizontalAlignInColumn" | "FooterHorizontalAlign" => {
                                    let tag = ch.tag.as_str();
                                    let v = ch.text.trim();
                                    if !seen_tags.insert(tag)
                                        || !ch.children.is_empty()
                                        || !ch.attrs.is_empty()
                                        || !["Left", "Center", "Right", "Auto"].contains(&v)
                                    {
                                        ok = false;
                                        break;
                                    }
                                    let slot = match tag {
                                        "HeaderHorizontalAlign" => &mut ext.header_horizontal_align,
                                        "HorizontalAlignInColumn" => &mut ext.horizontal_align_in_column,
                                        _ => &mut ext.footer_horizontal_align,
                                    };
                                    *slot = Some(v.to_string());
                                }
                                "Format" => {
                                    let Some(t) = seen_tags.insert("Format").then(|| parse_loc_elem(ch)).flatten() else {
                                        ok = false;
                                        break;
                                    };
                                    ext.format = t;
                                }
                                "CellHeight" => {
                                    if !seen_tags.insert("CellHeight") || !ch.children.is_empty() || !ch.attrs.is_empty() {
                                        ok = false;
                                        break;
                                    }
                                    let Ok(h) = ch.text.trim().parse::<i64>() else {
                                        ok = false;
                                        break;
                                    };
                                    ext.cell_height = Some(h);
                                }
                                _ => {}
                            }
                        }
                        let Some(width) = width else {
                            ok = false;
                            break;
                        };
                        if !ok {
                            break;
                        }
                        extracted.push(ColumnValues {
                            id: cid,
                            name: cname,
                            field_name,
                            header,
                            footer_text,
                            tooltip,
                            width,
                            visible,
                            read_only,
                            editor,
                            events: col_events,
                            ext,
                        });
                    }
                }
                if ok {
                    for (idx, col_el) in cols_el.children.iter_mut().enumerate() {
                        col_el.attrs.remove("name");
                        col_el.attrs.insert("_col".into(), idx.to_string());
                        col_el.children.retain(|ch| {
                            !matches!(
                                ch.tag.as_str(),
                                "DataPath"
                                    | "Header"
                                    | "FooterText"
                                    | "Tooltip"
                                    | "Width"
                                    | "Visible"
                                    | "ReadOnly"
                                    | "Editor"
                                    | "Events"
                                    | "ShowHierarchy"
                                    | "CheckBoxThreeState"
                                    | "ShowInHeader"
                                    | "ShowInFooter"
                                    | "ChangeSetting"
                                    | "ChangePosition"
                                    | "MarkNegatives"
                                    | "ChangeVisible"
                                    | "AutoMarkIncomplete"
                                    | "AutoCellHeight"
                                    | "Hyperlink"
                                    | "CheckData"
                                    | "RowsPictures"
                                    | "HeaderPicture"
                                    | "FooterPicture"
                                    | "Location"
                                    | "SizeChange"
                                    | "EditMode"
                                    | "FieldBackColor"
                                    | "FieldTextColor"
                                    | "HeaderBackColor"
                                    | "HeaderTextColor"
                                    | "FooterBackColor"
                                    | "FooterTextColor"
                                    | "TextFont"
                                    | "HeaderFont"
                                    | "FooterFont"
                                    | "HeaderHorizontalAlign"
                                    | "HorizontalAlignInColumn"
                                    | "FooterHorizontalAlign"
                                    | "Format"
                                    | "CellHeight"
                            )
                        });
                    }
                    columns = extracted;
                }
            }
        }
        let mut data_path: Option<(i64, String)> = None;
        if let Some(e) = found.get("DataPath") {
            let mut valid = e.children.is_empty() && e.attrs.len() == 1;
            if valid {
                if let Some(aid) = e.attrs.get("attributeId").and_then(|v| v.trim().parse::<i64>().ok()) {
                    let path = e.text.trim();
                    if self.attr_names.get(&aid).map(String::as_str) == Some(path) {
                        data_path = Some((aid, path.to_string()));
                    } else {
                        valid = false;
                    }
                } else {
                    valid = false;
                }
            }
            if !valid {
                rest.children.push((*e).clone());
            }
        }
        let color_of = |tag: &str, rest: &mut XmlElement| -> Option<ColorSpec> {
            let e = found.get(tag)?;
            match parse_color_elem(e) {
                Some(c) => Some(c),
                None => {
                    rest.children.push((*e).clone());
                    None
                }
            }
        };
        let back_color = color_of("BackColor", &mut rest);
        let text_color = color_of("TextColor", &mut rest);
        let border_color = color_of("BorderColor", &mut rest);
        let field_back_color = color_of("FieldBackColor", &mut rest);
        let field_text_color = color_of("FieldTextColor", &mut rest);
        let button_back_color = color_of("ButtonBackColor", &mut rest);
        let button_text_color = color_of("ButtonTextColor", &mut rest);
        let selection_back_color = color_of("SelectionBackColor", &mut rest);
        let selection_text_color = color_of("SelectionTextColor", &mut rest);
        let holiday_color = color_of("HolidayColor", &mut rest);
        let header_back_color = color_of("HeaderBackColor", &mut rest);
        let header_text_color = color_of("HeaderTextColor", &mut rest);
        let footer_back_color = color_of("FooterBackColor", &mut rest);
        let footer_text_color = color_of("FooterTextColor", &mut rest);
        let alternation_row_back_color = color_of("AlternationRowBackColor", &mut rest);
        let font_of = |tag: &str, rest: &mut XmlElement| -> Option<FontSpec> {
            let e = found.get(tag)?;
            match parse_font_elem(e) {
                Some(f) => Some(f),
                None => {
                    rest.children.push((*e).clone());
                    None
                }
            }
        };
        let header_font = font_of("HeaderFont", &mut rest);
        let footer_font = font_of("FooterFont", &mut rest);
        let font = match found.get("Font") {
            None => None,
            Some(e) => match parse_font_elem(e) {
                Some(f) => Some(f),
                None => {
                    rest.children.push((*e).clone());
                    None
                }
            },
        };
        fn spec_of<T>(
            found: &HashMap<&str, &XmlElement>,
            tag: &str,
            rest: &mut XmlElement,
            parse: fn(&XmlElement) -> Option<T>,
        ) -> Option<T> {
            let e = found.get(tag)?;
            let spec = parse(e);
            if spec.is_none() {
                rest.children.push((*e).clone());
            }
            spec
        }
        let shortcut = spec_of(&found, "Shortcut", &mut rest, parse_shortcut_elem);
        let picture = spec_of(&found, "Picture", &mut rest, parse_picture_elem);
        let choice_button_picture = spec_of(&found, "ChoiceButtonPicture", &mut rest, parse_picture_elem);
        let mut choice_list: Vec<ChoiceItemSpec> = Vec::new();
        if let Some(e) = found.get("ChoiceList") {
            let mut valid = e.attrs.is_empty() && e.text.trim().is_empty();
            if valid {
                for ci in &e.children {
                    if ci.tag != "ChoiceItem"
                        || !ci.text.trim().is_empty()
                        || ci.attrs.keys().any(|k| !matches!(k.as_str(), "value" | "checked" | "pictureId"))
                    {
                        valid = false;
                        break;
                    }
                    let checked = match ci.attrs.get("checked").map(|v| v.trim()) {
                        None => None,
                        Some("true") => Some(true),
                        Some("false") => Some(false),
                        Some(_) => {
                            valid = false;
                            break;
                        }
                    };
                    let picture_id = match ci.attrs.get("pictureId").map(|v| v.trim().parse::<i64>()) {
                        None => None,
                        Some(Ok(v)) => Some(v),
                        Some(Err(_)) => {
                            valid = false;
                            break;
                        }
                    };
                    let Some(val) = ci.attrs.get("value").cloned() else {
                        valid = false;
                        break;
                    };
                    let mut pres = Vec::new();
                    if ci.children.len() > 1 {
                        valid = false;
                        break;
                    }
                    if let Some(p_el) = ci.children.first() {
                        if p_el.tag != "Presentation" {
                            valid = false;
                            break;
                        }
                        let Some(loc) = parse_loc_elem(p_el) else {
                            valid = false;
                            break;
                        };
                        pres = loc;
                    }
                    choice_list.push(ChoiceItemSpec { value: val, presentation: pres, checked, picture_id });
                }
            }
            if !valid {
                choice_list.clear();
                rest.children.push((*e).clone());
            }
        }
        let mut value_type: Option<ValueTypeSpec> = None;
        if let Some(e) = found.get("ValueType") {
            if let Some(spec) = parse_value_type_spec(e) {
                value_type = Some(spec);
            } else {
                rest.children.push((*e).clone());
            }
        }
        let mut anchors: Option<AnchorsSpec> = None;
        if let Some(e) = found.get("Anchors") {
            if let Some(spec) = parse_anchors_elem(e) {
                anchors = Some(spec);
            } else {
                rest.children.push((*e).clone());
            }
        }
        fn parse_cb_buttons(
            e: &XmlElement,
            parse_loc: &dyn Fn(&XmlElement) -> Option<LocItems>,
        ) -> Option<Vec<CommandBarButton>> {
            if !e.attrs.is_empty() || !e.text.trim().is_empty() {
                return None;
            }
            let mut out = Vec::new();
            for b in &e.children {
                if b.tag != "Button" || !b.text.trim().is_empty() {
                    return None;
                }
                for k in b.attrs.keys() {
                    if !matches!(
                        k.as_str(),
                        "name" | "id" | "kind" | "checked" | "representation" | "hasPicture" | "actionKind" | "action"
                    ) {
                        return None;
                    }
                }
                let name = b.attrs.get("name")?.trim().to_string();
                if name.is_empty() {
                    return None;
                }
                let id = match b.attrs.get("id") {
                    Some(s) => Some(s.trim().parse::<i64>().ok()?),
                    None => None,
                };
                let kind = b.attrs.get("kind")?.to_string();
                if !matches!(kind.as_str(), "Action" | "Submenu" | "Separator") {
                    return None;
                }
                let checked = match b.attrs.get("checked").map(String::as_str)? {
                    "true" => true,
                    "false" => false,
                    _ => return None,
                };
                let representation = b.attrs.get("representation").cloned().unwrap_or_else(|| "Auto".to_string());
                if !matches!(representation.as_str(), "Auto" | "Text" | "Picture" | "PictureAndText") {
                    return None;
                }
                let has_picture = b.attrs.get("hasPicture").map(String::as_str) == Some("true");
                let action_kind = b.attrs.get("actionKind").cloned().unwrap_or_default();
                let action = b.attrs.get("action").cloned().unwrap_or_default();
                let mut caption = Vec::new();
                let mut tooltip = Vec::new();
                let mut description = Vec::new();
                let mut sub_buttons = Vec::new();
                let mut picture = None;
                let mut shortcut = None;
                let mut default_button = None;
                let mut button_order = None;
                for ch in &b.children {
                    match ch.tag.as_str() {
                        "Picture" if picture.is_none() => picture = Some(parse_picture_elem(ch)?),
                        "Shortcut" if shortcut.is_none() => shortcut = Some(parse_shortcut_elem(ch)?),
                        "Description" if description.is_empty() => description = parse_loc(ch)?,
                        "DefaultButton" if default_button.is_none() && ch.children.is_empty() && ch.attrs.is_empty() => {
                            default_button = Some(match ch.text.trim() {
                                "true" => true,
                                "false" => false,
                                _ => return None,
                            });
                        }
                        "ButtonOrder" if button_order.is_none() && ch.children.is_empty() && ch.attrs.is_empty() => {
                            button_order = Some(match ch.text.trim() {
                                "DontOrder" | "0" => 0,
                                "Ascending" | "1" => 1,
                                _ => return None,
                            });
                        }
                        "Caption" => caption = parse_loc(ch)?,
                        "Tooltip" => tooltip = parse_loc(ch)?,
                        "Buttons" => sub_buttons = parse_cb_buttons(ch, parse_loc)?,
                        _ => return None,
                    }
                }
                out.push(CommandBarButton {
                    name,
                    id,
                    kind,
                    checked,
                    representation,
                    has_picture,
                    action_kind,
                    action,
                    caption,
                    tooltip,
                    buttons: sub_buttons,
                    picture,
                    shortcut,
                    description,
                    default_button,
                    button_order,
                });
            }
            Some(out)
        }
        let mut buttons: Vec<CommandBarButton> = Vec::new();
        if let Some(e) = found.get("Buttons") {
            match parse_cb_buttons(e, &parse_loc_elem) {
                Some(b) => buttons = b,
                None => rest.children.push((*e).clone()),
            }
        }
        let selection_value = match found.get("SelectionValue") {
            None => None,
            Some(e) if e.children.is_empty() && e.attrs.is_empty() => Some(e.text.clone()),
            Some(e) => return Err(self.bad(e.line, format!("{who}: <SelectionValue> must be a plain text value"))),
        };
        let control_ref_of = |tag: &str| -> Result<Option<i64>> {
            let Some(e) = found.get(tag) else { return Ok(None) };
            let text = e.text.trim();
            let id = match e.attrs.get("controlId") {
                Some(v) if e.attrs.len() == 1 && text.is_empty() => v.trim().parse::<i64>().ok(),
                None if e.attrs.is_empty() => text.parse::<i64>().ok(),
                _ => None,
            };
            id.filter(|_| e.children.is_empty())
                .map(Some)
                .ok_or_else(|| self.bad(e.line, format!("{who}: <{tag}> must be an integer or have a 'controlId' attribute")))
        };
        let context_menu_id = control_ref_of("ContextMenu")?;
        let action_source_id = control_ref_of("ActionSource")?;
        let values = ControlValues {
            name: ctrl_name,
            data_path,
            caption: loc_of("Caption")?,
            left: int_of("Left", true)?,
            top: int_of("Top", true)?,
            width: int_of("Width", true)?,
            height: int_of("Height", true)?,
            visible: bool_of("Visible", true)?,
            z_order: int_of("ZOrder", true)?,
            tab_order: int_of("TabOrder", false)?,
            events,
            enabled: bool_of("Enabled", true)?,
            transparent_back: bool_of("TransparentBack", false)?,
            back_color,
            text_color,
            border_color,
            font,
            tooltip: loc_of("Tooltip")?,
            value_type,
            horizontal_scroll_bar: enum_of("HorizontalScrollBar", &["DontUse", "Use", "Auto"], scroll_default)?,
            vertical_scroll_bar: enum_of("VerticalScrollBar", &["DontUse", "Use", "Auto"], scroll_default)?,
            read_only: if type_name != "Table" { bool_of("ReadOnly", false)? } else { false },
            table_read_only: if type_name == "Table" { opt_bool_of("ReadOnly")? } else { None },
            choice_button: if is_inp { Some(opt_bool_of("ChoiceButton")?.unwrap_or(false)) } else { opt_bool_of("ChoiceButton")? },
            clear_button: if is_inp { Some(opt_bool_of("ClearButton")?.unwrap_or(false)) } else { opt_bool_of("ClearButton")? },
            open_button: if is_inp { Some(opt_bool_of("OpenButton")?.unwrap_or(false)) } else { opt_bool_of("OpenButton")? },
            drop_list_button: opt_bool_of("DropListButton")?,
            spin_button: opt_bool_of("SpinButton")?,
            auto_mark_incomplete: bool_of("AutoMarkIncomplete", false)?,
            auto_choice_incomplete: opt_bool_of("AutoChoiceIncomplete")?,
            mark_negatives: bool_of("MarkNegatives", false)?,
            auto_cell_height: opt_bool_of("AutoCellHeight")?,
            choice_list_mode: bool_of("ChoiceListMode", false)?,
            horizontal_align,
            hyperlink: bool_of("Hyperlink", false)?,
            three_state: bool_of("ThreeState", false)?,
            title_location: enum_of(
                "TitleLocation",
                &["Left", "Right"],
                if type_name == "RadioButton" { "\0" } else { "Right" },
            )?,
            choice_list,
            show_tabs,
            pages,
            footer: if type_name == "Table" { Some(opt_bool_of("Footer")?.unwrap_or(false)) } else { None },
            header: opt_bool_of("Header")?,
            horizontal_lines: opt_bool_of("HorizontalLines")?,
            vertical_lines: opt_bool_of("VerticalLines")?,
            auto_insert_new_row: opt_bool_of("AutoInsertNewRow")?,
            change_row_set: opt_bool_of("ChangeRowSet")?,
            change_position_of_columns: opt_bool_of("ChangePositionOfColumns")?,
            change_setting_of_columns: opt_bool_of("ChangeSettingOfColumns")?,
            initial_list_view: opt_enum_of("InitialListView", &["Begin", "End", "Auto"])?,
            output: opt_enum_of("Output", &["Auto", "Enable", "Disable"])?,
            selection_mode: if type_name == "CalendarField" {
                enum_of("SelectionMode", &["Single", "Multi", "Interval"], "\0")?
            } else {
                enum_of("SelectionMode", &["Single", "Multi"], "Single")?
            },
            initial_tree_view: enum_of("InitialTreeView", &["NoExpand", "ExpandTopLevel", "ExpandAllLevels"], "NoExpand")?,
            columns,
            auto_fill: if type_name == "CommandBar" { Some(opt_bool_of("AutoFill")?.unwrap_or(true)) } else { None },
            buttons,
            border_style: match found.get("BorderStyle") {
                None if type_name == "UsualGroup" => Some(6),
                None => None,
                Some(e) => {
                    let code = e.attrs.get("code").and_then(|v| v.trim().parse::<i64>().ok());
                    let expected_name = match code {
                        Some(6) => Some("Frame"),
                        Some(4) => Some("SingleLine"),
                        Some(0) => Some("None"),
                        _ => None,
                    };
                    if e.attrs.len() != 1 || !e.children.is_empty() || expected_name != Some(e.text.trim()) {
                        rest.children.push((*e).clone());
                        None
                    } else {
                        code
                    }
                }
            },
            anchors,
            modifies_data: opt_bool_of("ModifiesData")?,
            skip_on_input: opt_bool_of("SkipOnInput")?,
            first_in_group: opt_bool_of("FirstInGroup")?,
            multi_line: opt_bool_of("MultiLine")?,
            password_mode: opt_bool_of("PasswordMode")?,
            extended_edit: opt_bool_of("ExtendedEdit")?,
            mask,
            choose_type: opt_bool_of("ChooseType")?,
            text_edit: opt_bool_of("TextEdit")?,
            vertical_align,
            type_link,
            field_back_color,
            field_text_color,
            button_back_color,
            button_text_color,
            picture_location: opt_enum_of("PictureLocation", &["Left", "Right", "Top", "Bottom"])?,
            picture_size: opt_enum_of("PictureSize", &["RealSize", "Stretch", "AutoSize", "Proportionally", "Tile"])?,
            format: loc_of("Format")?,
            selection_value,
            choice_list_width: opt_int_of("ChoiceListWidth")?,
            min_value: opt_int_of("MinValue")?,
            max_value: opt_int_of("MaxValue")?,
            separate_by_page: opt_bool_of("SeparateByPage")?,
            auto_tab_order: opt_bool_of("AutoTabOrder")?,
            scroll_page_mode: opt_bool_of("ScrollPageMode")?,
            orientation: opt_enum_of("Orientation", &["Auto", "Vertical", "Horizontal"])?,
            secondary: opt_bool_of("Secondary")?,
            zoomable: opt_bool_of("Zoomable")?,
            use_context_menu: opt_bool_of("UseContextMenu")?,
            nonselected_picture_text: loc_of("NonselectedPictureText")?,
            enable_start_drag: opt_bool_of("EnableStartDrag")?,
            enable_drag: opt_bool_of("EnableDrag")?,
            choice_mode: opt_bool_of("ChoiceMode")?,
            show_picture: opt_bool_of("ShowPicture")?,
            show_check_box: opt_bool_of("ShowCheckBox")?,
            step: opt_int_of("Step")?,
            large_step: opt_int_of("LargeStep")?,
            marking_step: opt_int_of("MarkingStep")?,
            marking_appearance: opt_enum_of("MarkingAppearance", &["DontShow", "TopOrLeft", "BottomOrRight", "BothSides"])?,
            show_style: opt_enum_of("ShowStyle", &["Broken", "Smooth"])?,
            show_percent: opt_bool_of("ShowPercent")?,
            selection_back_color,
            selection_text_color,
            holiday_color,
            show_current_date: opt_bool_of("ShowCurrentDate")?,
            calendar_navigation: opt_bool_of("CalendarNavigation")?,
            use_alternation_row_color: opt_bool_of("UseAlternationRowColor")?,
            header_back_color,
            header_text_color,
            footer_back_color,
            footer_text_color,
            alternation_row_back_color,
            fixed_left: opt_int_of("FixedLeft")?,
            fixed_right: opt_int_of("FixedRight")?,
            row_input_mode: opt_enum_of("RowInputMode", &["AtListEnd", "AtWindowBottom", "AfterCurrentRow", "BeforeCurrentRow"])?,
            row_selection_mode: opt_enum_of("RowSelectionMode", &["Cell", "Row"])?,
            header_height: opt_int_of("HeaderHeight")?,
            footer_height: opt_int_of("FooterHeight")?,
            header_font,
            footer_font,
            shortcut,
            picture,
            choice_button_picture,
            menu_mode: opt_bool_of("MenuMode")?,
            scrolling_text: opt_enum_of("ScrollingText", &["DontUse", "VerySlow", "Slow", "Normal", "Fast", "VeryFast"])?,
            label_picture_location: opt_enum_of("LabelPictureLocation", &["Left", "Right", "Top", "Bottom"])?,
            panel_picture_location: opt_enum_of("PanelPictureLocation", &["TopLeft", "Center"])?,
            choice_list_height: opt_int_of("ChoiceListHeight")?,
            show_selection: opt_enum_of("ShowSelection", &["WhenActive", "Always"])?,
            begin_of_display_period: opt_int_of("BeginOfDisplayPeriod")?,
            end_of_display_period: opt_int_of("EndOfDisplayPeriod")?,
            default_button: opt_bool_of("DefaultButton")?,
            context_menu: context_menu_id,
            action_source: action_source_id,
            auto_context_menu: opt_bool_of("AutoContextMenu")?,
            collapse: opt_enum_of("Collapse", &["None", "Top", "Bottom", "Left", "Right"])?,
            embedded_into_cell: opt_bool_of("EmbeddedIntoCell")?,
            print: opt_bool_of("Print")?,
            protection: opt_bool_of("Protection")?,
            show_grid: opt_bool_of("ShowGrid")?,
            show_headers: opt_bool_of("ShowHeaders")?,
            show_groups: opt_bool_of("ShowGroups")?,
            black_and_white_view: opt_bool_of("BlackAndWhiteView")?,
            totals_right: opt_bool_of("TotalsRight")?,
            totals_below: opt_bool_of("TotalsBelow")?,
            print_scale: opt_int_of("PrintScale")?,
            fit_to_page: opt_bool_of("FitToPage")?,
            black_and_white: opt_bool_of("BlackAndWhite")?,
            collate: opt_bool_of("Collate")?,
            copies: opt_int_of("Copies")?,
            per_page: opt_int_of("PerPage")?,
            page_orientation: opt_enum_of("PageOrientation", &["Portrait", "Landscape"])?,
            bottom_margin: opt_int_of("BottomMargin")?,
            top_margin: opt_int_of("TopMargin")?,
            left_margin: opt_int_of("LeftMargin")?,
            right_margin: opt_int_of("RightMargin")?,
            header_size: opt_int_of("HeaderSize")?,
            footer_size: opt_int_of("FooterSize")?,
            duplex_printing: opt_int_of("DuplexPrinting")?,
            print_accuracy: opt_int_of("PrintAccuracy")?,
            step_direction: opt_int_of("StepDirection")?,
            save_view_properties: opt_bool_of("SaveViewProperties")?,
            fixed_background: opt_bool_of("FixedBackground")?,
        };
        Ok((values, rest))
    }
}

fn extract_form_attributes(shell: &mut XmlElement) -> (HashMap<i64, String>, Vec<FormAttribute>) {
    let main_attr_id = shell
        .children
        .iter()
        .find(|e| e.tag == "MainAttribute")
        .and_then(|e| e.attrs.get("attributeId"))
        .and_then(|v| v.trim().parse::<i64>().ok());
    let mut attr_names: HashMap<i64, String> = HashMap::new();
    let Some(attrs_el) = shell.children.iter_mut().find(|e| e.tag == "Attributes") else {
        return (attr_names, Vec::new());
    };
    if !attrs_el.attrs.is_empty() || !attrs_el.text.trim().is_empty() {
        return (attr_names, Vec::new());
    }
    for a in &attrs_el.children {
        if a.tag == "Attribute" {
            if let (Some(id), Some(name)) = (
                a.attrs.get("id").and_then(|v| v.trim().parse::<i64>().ok()),
                a.attrs.get("name").filter(|n| !n.trim().is_empty()),
            ) {
                attr_names.insert(id, name.clone());
            }
        }
    }
    let mut ok = true;
    let mut extracted = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_names = std::collections::HashSet::new();
    for a in &attrs_el.children {
        if a.tag != "Attribute" || a.attrs.len() != 2 || !a.text.trim().is_empty() {
            ok = false;
            break;
        }
        let Some(id) = a.attrs.get("id").and_then(|v| v.trim().parse::<i64>().ok()).filter(|&i| i > 0) else {
            ok = false;
            break;
        };
        let Some(name) = a.attrs.get("name").filter(|n| !n.trim().is_empty()).cloned() else {
            ok = false;
            break;
        };
        if !seen_ids.insert(id) || !seen_names.insert(name.clone()) {
            ok = false;
            break;
        }
        if Some(id) == main_attr_id {
            continue;
        }
        let mut main = false;
        let mut stored = false;
        let mut vt_info: Option<ValueTypeSpec> = None;
        let mut seen_tags = std::collections::HashSet::new();
        for ch in &a.children {
            match ch.tag.as_str() {
                "Main" => {
                    if !seen_tags.insert("Main") || !ch.children.is_empty() || !ch.attrs.is_empty() || (ch.text.trim() != "true" && ch.text.trim() != "false") {
                        ok = false;
                        break;
                    }
                    main = ch.text.trim() == "true";
                }
                "StoredData" => {
                    if !seen_tags.insert("StoredData") || !ch.children.is_empty() || !ch.attrs.is_empty() || (ch.text.trim() != "true" && ch.text.trim() != "false") {
                        ok = false;
                        break;
                    }
                    stored = ch.text.trim() == "true";
                }
                "ValueType" => {
                    if !seen_tags.insert("ValueType") {
                        ok = false;
                        break;
                    }
                    if let Some(spec) = parse_value_type_spec(ch) {
                        vt_info = Some(spec);
                    } else {
                        ok = false;
                        break;
                    }
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            break;
        }
        let vt = vt_info.unwrap_or_default();
        let fa = FormAttribute {
            id,
            name,
            main,
            stored,
            kind: vt.kind,
            composite: vt.composite,
            fraction_digits: vt.fraction_digits,
            digits: vt.digits,
            length: vt.length,
            non_negative: vt.non_negative,
            fixed: vt.fixed,
            ref_guids: vt.ref_guids,
        };
        if format_attr_pattern(&fa).is_none() {
            ok = false;
            break;
        }
        extracted.push(fa);
    }
    if ok {
        attrs_el
            .children
            .retain(|a| a.attrs.get("id").and_then(|v| v.trim().parse::<i64>().ok()) == main_attr_id);
        (attr_names, extracted)
    } else {
        (attr_names, Vec::new())
    }
}

fn extract_form_header(root: &mut XmlElement) -> (LocItems, Option<i64>, Option<i64>, EventItems) {
    let mut form_caption = Vec::new();
    let mut cap_ok = false;
    if let Some(cap_el) = root.children.iter().find(|e| e.tag == "Caption") {
        if cap_el.attrs.is_empty() && cap_el.text.trim().is_empty() {
            let mut items = Vec::new();
            let mut valid = true;
            for it in &cap_el.children {
                if let Some(lang) = it.attrs.get("language") {
                    if it.tag == "Item" && it.attrs.len() == 1 && it.children.is_empty() && !items.iter().any(|(l, _)| l == lang) {
                        items.push((lang.clone(), it.text.clone()));
                        continue;
                    }
                }
                valid = false;
                break;
            }
            if valid {
                form_caption = items;
                cap_ok = true;
            }
        }
    }
    if cap_ok {
        root.children.retain(|e| e.tag != "Caption");
    }

    let dim_of = |tag: &str, root: &mut XmlElement| -> Option<i64> {
        if root.children.iter().filter(|e| e.tag == tag).count() != 1 {
            return None;
        }
        let el = root.children.iter().find(|e| e.tag == tag)?;
        if !el.attrs.is_empty() || !el.children.is_empty() {
            return None;
        }
        let v = strict_int(el.text.trim()).filter(|&v| v > 0 && i32::try_from(v).is_ok())?;
        root.children.retain(|e| e.tag != tag);
        Some(v)
    };
    let form_width = dim_of("Width", root);
    let form_height = dim_of("Height", root);

    let mut form_events = Vec::new();
    let mut ev_ok = false;
    if let Some(ev_el) = root.children.iter().find(|e| e.tag == "Events") {
        if ev_el.attrs.is_empty() && ev_el.text.trim().is_empty() {
            let mut items = Vec::new();
            let mut valid = true;
            for ev in &ev_el.children {
                let Some(tag) = ev.attrs.get("id").and_then(|v| v.trim().parse::<i64>().ok()) else {
                    valid = false;
                    break;
                };
                let h = ev.text.trim();
                let tag_matches = crate::oof::FORM_EVENTS.iter().any(|(t, n)| *t == tag && *n == ev.tag)
                    || crate::oof::EXT_DP_EVENTS.iter().any(|(t, n)| *t == tag && *n == ev.tag)
                    || crate::oof::EXT_DOC_EVENTS.iter().any(|(t, n)| *t == tag && *n == ev.tag)
                    || (tag == 0 && ev.tag == "OnCurrentPageChange");
                if ev.attrs.len() != 1 || !ev.children.is_empty() || h.is_empty() || !tag_matches {
                    valid = false;
                    break;
                }
                items.push((tag, h.to_string()));
            }
            if valid {
                form_events = items;
                ev_ok = true;
            }
        }
    }
    if ev_ok {
        root.children.retain(|e| e.tag != "Events");
    }
    (form_caption, form_width, form_height, form_events)
}

fn extract_form_root_props(root: &mut XmlElement) -> FormRootProps {
    let mut fp = FormRootProps::default();
    let parse_bool = |e: &XmlElement| -> Option<bool> {
        if !e.children.is_empty() || !e.attrs.is_empty() {
            return None;
        }
        match e.text.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    };
    let parse_code = |e: &XmlElement, table: &[(&str, i64)]| -> Option<i64> {
        if !e.children.is_empty() {
            return None;
        }
        if let Some(c) = e.attrs.get("code") {
            return c.trim().parse::<i64>().ok().filter(|v| table.iter().any(|(_, tc)| tc == v));
        }
        if !e.attrs.is_empty() {
            return None;
        }
        let s = e.text.trim();
        table
            .iter()
            .find(|(n, _)| *n == s)
            .map(|(_, c)| *c)
            .or_else(|| s.parse::<i64>().ok().filter(|v| table.iter().any(|(_, tc)| tc == v)))
    };
    let mut seen_tags = std::collections::HashSet::new();
    let mut remove_idx = std::collections::HashSet::new();
    for (idx, e) in root.children.iter().enumerate() {
        if seen_tags.contains(e.tag.as_str()) {
            continue;
        }
        let ok = match e.tag.as_str() {
            "AutoTitle" => parse_bool(e).map(|v| fp.auto_title = Some(v)).is_some(),
            "ConnectableWindow" => parse_bool(e).map(|v| fp.connectable_window = Some(v)).is_some(),
            "AllowJoinWindow" => parse_bool(e).map(|v| fp.allow_join_window = Some(v)).is_some(),
            "DesktopMode" => parse_bool(e).map(|v| fp.desktop_mode = Some(v)).is_some(),
            "AllowClose" => parse_bool(e).map(|v| fp.allow_close = Some(v)).is_some(),
            "AllowNormalState" => parse_bool(e).map(|v| fp.allow_normal_state = Some(v)).is_some(),
            "AllowDockedState" => parse_bool(e).map(|v| fp.allow_docked_state = Some(v)).is_some(),
            "AllowAutohideState" => parse_bool(e).map(|v| fp.allow_autohide_state = Some(v)).is_some(),
            "AllowFloatState" => parse_bool(e).map(|v| fp.allow_float_state = Some(v)).is_some(),
            "CaptionPicture" => parse_picture_elem(e).map(|v| fp.caption_picture = Some(v)).is_some(),
            "WindowState" => parse_code(e, &[("Docked", 0), ("Autohide", 1), ("Float", 2), ("Normal", 4)])
                .map(|v| fp.window_state = Some(v))
                .is_some(),
            "SizeChange" => parse_code(e, &[("Change", 0), ("DontChange", 1)])
                .map(|v| fp.size_change = Some(v))
                .is_some(),
            "WindowAppearanceMode" => parse_code(e, &[("Normal", 0), ("Minimized", 1), ("Maximized", 2)])
                .map(|v| fp.window_appearance_mode = Some(v))
                .is_some(),
            "WindowLocation" => parse_code(e, &[("DontOverlapOwner", 0), ("Center", 1), ("Auto", 2)])
                .map(|v| fp.window_location = Some(v))
                .is_some(),
            "ChangeWindowAppearanceMode" => parse_code(e, &[("Auto", 0), ("Allow", 1), ("DontAllow", 2)])
                .map(|v| fp.change_window_appearance_mode = Some(v))
                .is_some(),
            "EnterKeyBehavior" => parse_code(e, &[("DefaultButton", 0), ("ControlNavigation", 1)])
                .map(|v| fp.enter_key_behavior = Some(v))
                .is_some(),
            "WindowDockLocation" => parse_code(e, &[("Left", 0), ("Top", 1), ("Right", 2), ("Bottom", 3), ("Auto", 4)])
                .map(|v| fp.window_dock_location = Some(v))
                .is_some(),
            "Style" => {
                let reference = match (e.attrs.len(), e.attrs.get("ref")) {
                    (0, _) => Some(String::new()),
                    (1, Some(g)) if is_uuid(g.trim()) => Some(g.trim().to_lowercase()),
                    _ => None,
                };
                reference
                    .filter(|_| e.children.is_empty() && e.text.trim().is_empty())
                    .map(|v| fp.style_ref = Some(v))
                    .is_some()
            }
            "DefaultControlId" => (e.children.is_empty() && e.attrs.is_empty())
                .then(|| e.text.trim().parse::<i64>().ok())
                .flatten()
                .map(|v| fp.default_control_id = Some(v))
                .is_some(),
            _ => false,
        };
        if ok {
            seen_tags.insert(e.tag.clone());
            remove_idx.insert(idx);
        }
    }
    let mut cur_idx = 0usize;
    root.children.retain(|_| {
        let keep = !remove_idx.contains(&cur_idx);
        cur_idx += 1;
        keep
    });
    fp
}

/// Parse a `Form.xml` (UTF-8, BOM optional) into a [`FormXml`]. `label` (`"baseline"`/`"edited"`) is
/// used in errors. Indentation, line endings, attribute order and inter-element whitespace do not
/// influence the result.
pub fn parse_form_xml(bytes: &[u8], label: &str) -> Result<FormXml> {
    let malformed = |m: String| {
        ExtractionError::new(GENERATED_FILE_MODIFIED, format!("{label} Form.xml is not valid: {m}"))
            .with("kind", "malformed-xml")
            .with("source", label)
    };
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|e| malformed(format!("not valid UTF-8 ({e})")))?;
    let doc = roxmltree::Document::parse(text).map_err(|e| malformed(e.to_string()))?;
    let mut root = convert(doc.root_element(), &doc);
    let form_name = root.attrs.get("name").cloned().unwrap_or_default();
    if root.tag != "Form" {
        let p = Parser { label, form: form_name, attr_names: HashMap::new(), controls: Vec::new() };
        return Err(p.bad(root.line, format!("root element must be <Form>, found <{}>", root.tag)));
    }
    let (form_caption, form_width, form_height, form_events) = extract_form_header(&mut root);
    let form_props = extract_form_root_props(&mut root);
    let (attr_names, attributes) = extract_form_attributes(&mut root);
    let mut p = Parser { label, form: form_name, attr_names, controls: Vec::new() };
    let shell = p.walk(&root, None, None)?;
    Ok(FormXml {
        label: label.to_string(),
        name: p.form,
        form_caption,
        form_width,
        form_height,
        form_events,
        form_props,
        attributes,
        controls: p.controls,
        shell,
    })
}

// ------------------------------------------------------------------ 3.2 control identity index

/// A node together with its span sidecar.
#[derive(Debug, Clone, Copy)]
pub struct NodeRef<'a> {
    pub node: &'a Node,
    pub span: &'a SpanNode,
}

/// An original binary control located by the layout of `oof::control_parts`.
#[derive(Debug, Clone)]
pub struct ControlEntry<'a> {
    pub id: i64,
    pub name: String,
    pub guid: String,
    /// A `CONTROL_GUIDS` type name or [`UNKNOWN_CONTROL`].
    pub type_name: &'static str,
    pub parent: Option<i64>,
    /// Page slot recorded in the geometry tail (`oof::geom_page`).
    pub page: i64,
    /// Child indexes from the stream root to the control node.
    pub path: Vec<usize>,
    /// Child indexes from the stream root to the counted list holding this control.
    pub list_path: Vec<usize>,
    /// Position of this control in its list (0-based, excluding the count atom).
    pub list_index: usize,
    /// The control record itself.
    pub control: NodeRef<'a>,
    info_index: usize,
}

impl<'a> ControlEntry<'a> {
    fn child(&self, i: usize) -> Option<NodeRef<'a>> {
        match (self.control.node.children.get(i)?, self.control.span.children.get(i)?) {
            (Item::Node(node), SpanItem::Node(span)) => Some(NodeRef { node, span }),
            _ => None,
        }
    }

    /// Geometry node (`left, top, right, bottom, visible, ...`).
    pub fn geom(&self) -> Option<NodeRef<'a>> {
        self.child(self.control.node.len().checked_sub(3)?)
    }

    /// Meta node (`.., name, ..`).
    pub fn meta(&self) -> Option<NodeRef<'a>> {
        self.child(self.control.node.len().checked_sub(2)?)
    }

    /// Info node (properties, events).
    pub fn info(&self) -> Option<NodeRef<'a>> {
        self.child(self.info_index)
    }

    /// Counted list of nested controls.
    pub fn children_list(&self) -> Option<NodeRef<'a>> {
        self.child(self.control.node.len().checked_sub(1)?)
    }
}

/// All controls of the original stream, indexed by numeric id (`entries` is in stream order).
#[derive(Debug, Clone)]
pub struct ControlIndex<'a> {
    pub entries: Vec<ControlEntry<'a>>,
    by_id: HashMap<i64, usize>,
    source: &'a SpannedMetadata,
}

fn layout_err(message: impl Into<String>) -> ExtractionError {
    ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, message)
}

fn span_node(s: &SpanNode, i: usize) -> Result<&SpanNode> {
    match s.children.get(i) {
        Some(SpanItem::Node(n)) => Ok(n),
        _ => Err(layout_err(format!("form stream: span of child {i} is not a node"))),
    }
}

impl<'a> ControlIndex<'a> {
    /// Index the controls of a `{27,{18,...}}` form stream. Fails on an unexpected layout or on duplicate ids.
    pub fn build(source: &'a SpannedMetadata) -> Result<ControlIndex<'a>> {
        let root = &source.root;
        let panel = root
            .node(1)
            .and_then(|s| s.node(2))
            .ok_or_else(|| layout_err("form stream: root panel not found at {27,{..,..,panel}}"))?;
        let panel_span = span_node(span_node(&source.spans, 1)?, 2)?;
        let mut idx = ControlIndex { entries: Vec::new(), by_id: HashMap::new(), source };
        idx.walk_list(panel, panel_span, 2, vec![1, 2], None)?;
        Ok(idx)
    }

    /// `holder.children[slot]` is a counted control list.
    fn walk_list(&mut self, holder: &'a Node, holder_span: &'a SpanNode, slot: usize, path: Vec<usize>, parent: Option<i64>) -> Result<()> {
        let items = table(holder.get(slot), "control list").map_err(|e| layout_err(format!("{e} at stream path {path:?}")))?;
        let list_span = span_node(holder_span, slot)?;
        for (k, item) in items.iter().enumerate() {
            let at = 1 + k;
            let (Item::Node(node), Some(SpanItem::Node(span))) = (item, list_span.children.get(at)) else {
                return Err(layout_err(format!("control list item {k} at stream path {path:?} is not a node")));
            };
            let mut cpath = path.clone();
            cpath.push(slot);
            cpath.push(at);
            self.add(item, node, span, cpath, parent, k)?;
        }
        Ok(())
    }

    fn add(&mut self, item: &'a Item, node: &'a Node, span: &'a SpanNode, path: Vec<usize>, parent: Option<i64>, list_index: usize) -> Result<()> {
        let at = format!("control at stream path {path:?}");
        let parts = control_parts(item).map_err(|e| layout_err(format!("{at}: {e}")))?;
        if span.children.len() != node.children.len() {
            return Err(layout_err(format!("{at}: span sidecar does not match the node")));
        }
        let id = int(&node.children[1], "control id").map_err(|e| layout_err(format!("{at}: {e}")))?;
        let name = str_val(&parts.meta.children[1], "control name")
            .map_err(|e| layout_err(format!("{at} (id {id}): {e}")))?
            .to_string();
        let guid = match &node.children[0] {
            Item::Atom(a) => a.value.clone(),
            Item::Node(_) => String::new(),
        };
        let info_index = node
            .children
            .iter()
            .position(|c| matches!(c, Item::Node(n) if std::ptr::eq(n, parts.info)))
            .ok_or_else(|| layout_err(format!("{at}: info node not found")))?;
        if let Some(&prev) = self.by_id.get(&id) {
            return Err(layout_err(format!(
                "duplicate control id {id} in the form stream ('{}' at {:?} and '{name}' at {path:?}); controls cannot be matched uniquely",
                self.entries[prev].name, self.entries[prev].path
            ))
            .with("id", id));
        }
        let (page, _, _) = geom_page(parts.geom);
        let children_index = node.len() - 1;
        self.by_id.insert(id, self.entries.len());
        self.entries.push(ControlEntry {
            id,
            name,
            type_name: control_type(&guid).unwrap_or(UNKNOWN_CONTROL),
            guid,
            parent,
            page,
            list_path: path[..path.len() - 1].to_vec(),
            list_index,
            path: path.clone(),
            control: NodeRef { node, span },
            info_index,
        });
        self.walk_list(node, span, children_index, path, Some(id))
    }

    pub fn get(&self, id: i64) -> Option<&ControlEntry<'a>> {
        self.by_id.get(&id).map(|&i| &self.entries[i])
    }

    /// The original stream behind this index.
    pub fn source(&self) -> &'a SpannedMetadata {
        self.source
    }

    /// Walk the span sidecar / semantic tree along `path` (child indexes from the root).
    fn at_path(&self, path: &[usize]) -> Result<NodeRef<'a>> {
        let mut node = &self.source.root;
        let mut span = &self.source.spans;
        for &i in path {
            match (node.children.get(i), span.children.get(i)) {
                (Some(Item::Node(n)), Some(SpanItem::Node(s))) => {
                    node = n;
                    span = s;
                }
                _ => return Err(layout_err(format!("form stream: path {path:?} is not a node"))),
            }
        }
        Ok(NodeRef { node, span })
    }

    /// The form's root panel (not itself a control entry).
    pub fn root_panel(&self) -> Result<NodeRef<'a>> {
        self.at_path(&[1, 2])
    }

    /// The counted children list of the root panel (form-level controls).
    pub fn root_children_list(&self) -> Result<NodeRef<'a>> {
        self.root_panel()?.child(2).ok_or_else(|| layout_err("form stream: root panel has no children list"))
    }

    /// The counted links list (`root[2][3]`: control id -> DataPath), when present.
    pub fn links_list(&self) -> Option<NodeRef<'a>> {
        self.at_path(&[2, 3]).ok()
    }

    /// Validate an XML projection against the binary controls: unique ids, and every control matched by
    /// numeric id with the same name, type and parent. Missing (deleted), extra (added), duplicate,
    /// renamed, retyped and moved controls are all errors when `structural_ok` is false; when true,
    /// added and deleted controls are left for the structural diff (donor-based add/remove).
    pub fn validate(&self, xml: &FormXml) -> Result<()> {
        self.validate_mode(xml, false)
    }

    pub fn validate_mode(&self, xml: &FormXml, structural_ok: bool) -> Result<()> {
        let mut problems = Vec::new();
        let mut seen: HashMap<i64, &ControlXml> = HashMap::new();
        let mut seen_names: HashMap<&str, &ControlXml> = HashMap::new();
        for c in &xml.controls {
            if let Some(prev) = seen.insert(c.id, c) {
                problems.push(problem(
                    "duplicate-control",
                    format!("line {}: duplicate control id {} ({} and {}); controls cannot be matched uniquely", c.line, c.id, prev.describe(), c.describe()),
                ));
                continue;
            }
            if c.name.trim().is_empty() {
                problems.push(problem(
                    "renamed-control",
                    format!("line {}: control id {} has an empty 'name' attribute", c.line, c.id),
                ));
            } else if let Some(prev) = seen_names.insert(c.name.as_str(), c) {
                problems.push(problem(
                    "renamed-control",
                    format!("line {}: duplicate control name '{}' (id {} and id {})", c.line, c.name, prev.id, c.id),
                ));
            }
            let Some(e) = self.get(c.id) else {
                if !structural_ok {
                    problems.push(problem(
                        "added-control",
                        format!("line {}: {} is not in the original form (adding controls is not supported)", c.line, c.describe()),
                    ));
                }
                continue;
            };
            if !structural_ok && c.name != e.name {
                problems.push(problem(
                    "renamed-control",
                    format!("line {}: control id {} renamed '{}' -> '{}' (renaming is not supported)", c.line, c.id, e.name, c.name),
                ));
            }
            if c.type_name != e.type_name {
                problems.push(problem(
                    "retyped-control",
                    format!("line {}: control '{}' (id {}) changed type {} -> {} (not supported)", c.line, e.name, c.id, e.type_name, c.type_name),
                ));
            } else if e.type_name == UNKNOWN_CONTROL && c.guid.as_deref() != Some(e.guid.as_str()) {
                problems.push(problem(
                    "retyped-control",
                    format!("line {}: UnknownControl '{}' (id {}) guid '{}' != original '{}'", c.line, e.name, c.id, c.guid.clone().unwrap_or_default(), e.guid),
                ));
            }
            if c.parent != e.parent {
                problems.push(problem(
                    "moved-control",
                    format!(
                        "line {}: control '{}' (id {}) is under parent {:?}, the original form has {:?}; moving controls is not supported",
                        c.line, e.name, c.id, c.parent, e.parent
                    ),
                ));
            }
        }
        for e in &self.entries {
            if !seen.contains_key(&e.id) && !structural_ok {
                problems.push(problem(
                    "deleted-control",
                    format!("{} '{}' (id {}) is missing from the XML (deleting controls is not supported)", e.type_name, e.name, e.id),
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(fail(&xml.name, &xml.label, problems))
        }
    }
}

// ------------------------------------------------------------------ 3.3 whole-projection diff

/// One changed control with the old and new supported values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlEdit {
    pub id: i64,
    pub name: String,
    pub type_name: String,
    pub old: ControlValues,
    pub new: ControlValues,
    /// Non-empty; ordered as [`Property`].
    pub changed: Vec<Property>,
}

/// Result of a clean diff: only supported edits, in document order. `additions`/`deletions` are
/// structural edits (donor-based add/remove) collected when `structural` mode is on.
#[derive(Debug, Clone, Default)]
pub struct FormDiff {
    pub edits: Vec<ControlEdit>,
    /// Controls present only in the edited XML (new, to be cloned from the donor).
    pub additions: Vec<ControlXml>,
    /// Controls present only in the baseline XML (to be removed).
    pub deletions: Vec<ControlXml>,
    pub form_caption_changed: bool,
    pub new_form_caption: Vec<(String, String)>,
    pub form_size_changed: bool,
    pub old_form_size: (Option<i64>, Option<i64>),
    pub new_form_size: (Option<i64>, Option<i64>),
    pub form_events_changed: bool,
    pub old_form_events: Vec<(i64, String)>,
    pub new_form_events: Vec<(i64, String)>,
    pub attributes_changed: bool,
    pub old_attributes: Vec<FormAttribute>,
    pub new_attributes: Vec<FormAttribute>,
    pub form_props_changed: bool,
    pub new_form_props: FormRootProps,
}

impl FormDiff {
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
            && self.additions.is_empty()
            && self.deletions.is_empty()
            && !self.form_caption_changed
            && !self.form_size_changed
            && !self.form_events_changed
            && !self.attributes_changed
            && !self.form_props_changed
    }
}

fn seg(e: &XmlElement) -> String {
    if e.tag == CONTROL_STUB {
        return format!("control#{}", e.attrs.get("id").map(String::as_str).unwrap_or("?"));
    }
    for key in ["name", "id", "language"] {
        if let Some(v) = e.attrs.get(key) {
            return format!("{}[{key}={v}]", e.tag);
        }
    }
    e.tag.clone()
}

fn show(s: &str) -> String {
    const MAX: usize = 60;
    let t: String = s.chars().take(MAX).collect();
    format!("{:?}{}", t, if s.chars().count() > MAX { "..." } else { "" })
}

fn join(path: &str, s: &str) -> String {
    if path.is_empty() {
        s.to_string()
    } else {
        format!("{path}/{s}")
    }
}

/// Every difference between two same-tag elements as `(path, message)`; order is significant.
fn diff_elements(a: &XmlElement, b: &XmlElement, path: &str, out: &mut Vec<(String, String)>) {
    let mut keys: Vec<&String> = a.attrs.keys().chain(b.attrs.keys()).collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        match (a.attrs.get(k), b.attrs.get(k)) {
            (Some(x), Some(y)) if x != y => out.push((path.into(), format!("attribute '{k}' changed {} -> {}", show(x), show(y)))),
            (Some(x), None) => out.push((path.into(), format!("attribute '{k}' removed (was {})", show(x)))),
            (None, Some(y)) => out.push((path.into(), format!("attribute '{k}' added ({})", show(y)))),
            _ => {}
        }
    }
    if a.text != b.text {
        out.push((path.into(), format!("text changed {} -> {}", show(&a.text), show(&b.text))));
    }
    // Align children by identity key (longest common subsequence) so an insertion or removal is reported
    // once instead of shifting every following sibling.
    let ka: Vec<String> = a.children.iter().map(key).collect();
    let kb: Vec<String> = b.children.iter().map(key).collect();
    let (n, m) = (ka.len(), kb.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if ka[i] == kb[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && ka[i] == kb[j] {
            diff_elements(&a.children[i], &b.children[j], &join(path, &seg(&a.children[i])), out);
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
            let x = &a.children[i];
            out.push((join(path, &seg(x)), format!("element <{}> removed", x.tag)));
            i += 1;
        } else {
            let y = &b.children[j];
            out.push((join(path, &seg(y)), format!("element <{}> added", y.tag)));
            j += 1;
        }
    }
}

/// Identity of a child among its siblings: tag plus `name`/`language` (or the id of a control stub).
fn key(e: &XmlElement) -> String {
    let attr = if e.tag == CONTROL_STUB { "id" } else { "name" };
    let by = e
        .attrs
        .get(attr)
        .or_else(|| e.attrs.get("_col"))
        .or_else(|| e.attrs.get("language"));
    format!("{}\u{0}{}", e.tag, by.map(String::as_str).unwrap_or(""))
}

/// The owner's direct child that contains the difference names its category.
fn category(owner_type: Option<&str>, path: &str) -> &'static str {
    if path.is_empty() {
        return "attribute";
    }
    let tag = path.split('/').next().unwrap_or("").split('[').next().unwrap_or("");
    match (owner_type, tag) {
        (_, "Events") => "event",
        (None, "Caption" | "Width" | "Height" | "MainAttribute" | "DataProcessorFormExtension" | "Attributes") => "form-value",
        (None | Some("Panel"), "ShowTabs" | "ChildItems" | "Page") => "panel",
        (Some("Table"), "Columns") => "table",
        (Some("CommandBar"), "Buttons" | "AutoFill" | "HorizontalAlign") => "command-bar",
        (Some(_), "ChildItems") => "child-items",
        (
            Some(_),
            "DataPath" | "TextColor" | "BackColor" | "BorderColor" | "Font" | "Tooltip" | "HorizontalAlign" | "Hyperlink" | "Picture"
            | "BorderStyle" | "ValueType" | "ChoiceButton" | "ClearButton" | "OpenButton" | "AutoMarkIncomplete" | "ChoiceList" | "Footer"
            | "Header" | "HorizontalLines" | "VerticalLines" | "AutoInsertNewRow" | "ChangeRowSet" | "ChangePositionOfColumns"
            | "ChangeSettingOfColumns" | "InitialListView" | "Output"
            | "AutoFill" | "Buttons" | "Columns",
        ) => "property",
        _ => "unknown-element",
    }
}

fn unsupported(owner: &str, owner_type: Option<&str>, line: u32, path: &str, msg: &str) -> Problem {
    let cat = category(owner_type, path);
    let kind = match cat {
        "attribute" if owner_type.is_none() => "unsupported-form-attribute",
        "attribute" => "unsupported-control-attribute",
        "event" => "unsupported-event",
        "form-value" => "unsupported-form-value",
        "panel" => "unsupported-panel",
        "table" => "unsupported-table",
        "command-bar" => "unsupported-command-bar",
        "unknown-element" => "unsupported-unknown-element",
        _ => "unsupported-property",
    };
    let prop = if path.is_empty() { String::new() } else { format!(", property '{path}'") };
    problem(
        kind,
        format!(
            "line {line}: {owner}{prop}: {msg} ({cat}; only Caption, Left, Top, Width, Height, Visible, ZOrder and TabOrder of existing controls can be edited)"
        ),
    )
}

/// Diff the ENTIRE projection of `edited` against `baseline`. In non-structural mode identity changes
/// (added/deleted/duplicate/renamed/retyped/moved) and every difference outside the supported property
/// set are errors. In structural mode, added controls of donor-supported types and deletions are
/// collected into [`FormDiff::additions`]/[`FormDiff::deletions`] instead. Supported property
/// differences are returned as [`ControlEdit`]s.
pub fn diff_forms(baseline: &FormXml, edited: &FormXml) -> Result<FormDiff> {
    diff_forms_mode(baseline, edited, false)
}

pub fn diff_forms_mode(baseline: &FormXml, edited: &FormXml, structural: bool) -> Result<FormDiff> {
    diff_forms_full(baseline, edited, structural, &std::collections::HashSet::new())
}

/// Full diff; controls whose id is in `skip_rest` compare identity and values only (their `rest` is
/// allowed to be sparse on one side — used for added controls in pack-time verification).
pub fn diff_forms_full(
    baseline: &FormXml,
    edited: &FormXml,
    structural: bool,
    skip_rest: &std::collections::HashSet<i64>,
) -> Result<FormDiff> {
    let mut problems: Vec<Problem> = Vec::new();
    let mut additions: Vec<ControlXml> = Vec::new();
    let mut deletions: Vec<ControlXml> = Vec::new();
    let mut base_ids: HashMap<i64, &ControlXml> = HashMap::new();
    for c in &baseline.controls {
        if base_ids.insert(c.id, c).is_some() {
            problems.push(problem("duplicate-control", format!("baseline has duplicate control id {}", c.id)));
        }
    }
    let mut seen: HashMap<i64, &ControlXml> = HashMap::new();
    let mut seen_names: HashMap<&str, &ControlXml> = HashMap::new();
    for c in &edited.controls {
        if let Some(prev) = seen.insert(c.id, c) {
            problems.push(problem(
                "duplicate-control",
                format!("line {}: duplicate control id {} ({} and {}); controls cannot be matched uniquely", c.line, c.id, prev.describe(), c.describe()),
            ));
            continue;
        }
        if c.name.trim().is_empty() {
            problems.push(problem("renamed-control", format!("line {}: control id {} has an empty 'name' attribute", c.line, c.id)));
        } else if let Some(prev) = seen_names.insert(c.name.as_str(), c) {
            problems.push(problem("renamed-control", format!("line {}: duplicate control name '{}' (id {} and id {})", c.line, c.name, prev.id, c.id)));
        }
        let Some(b) = base_ids.get(&c.id) else {
            if structural {
                additions.push(c.clone());
            } else {
                problems.push(problem("added-control", format!("line {}: {} is not in the baseline (adding controls is not supported)", c.line, c.describe())));
            }
            continue;
        };
        if b.type_name != c.type_name || b.guid != c.guid {
            problems.push(problem("retyped-control", format!("line {}: control '{}' (id {}) changed type {} -> {} (not supported)", c.line, b.name, c.id, b.type_name, c.type_name)));
        }
        if b.parent != c.parent || b.page != c.page {
            problems.push(problem("moved-control", format!("line {}: control '{}' (id {}) moved to another parent or page (not supported)", c.line, b.name, c.id)));
        }
    }
    for b in &baseline.controls {
        if !seen.contains_key(&b.id) {
            if structural {
                deletions.push(b.clone());
            } else {
                problems.push(problem("deleted-control", format!("{} is missing from the edited XML (deleting controls is not supported)", b.describe())));
            }
        }
    }
    if structural {
        // Structural validation of the collected changes: additions need a donor prototype and a
        // non-CommandBar parent; their events/unsupported inner structure are still diffed below.
        for c in &additions {
            if crate::donor::prototype(&c.type_name).is_err() {
                problems.push(problem(
                    "added-control",
                    format!("line {}: {} has no approved donor prototype (adding it is not supported)", c.line, c.describe()),
                ));
            }
            if let Some(_pid) = c.parent {
                // CommandBar parents are rejected later against the binary index (needs entry lookup);
                // here we only have the XML. The pack planner performs the binary-side check.
            }
        }
    }
    if !problems.is_empty() {
        return Err(fail(&baseline.name, &edited.label, problems));
    }

    // Shell diff ignores stubs of structurally added/removed controls; everything else must match.
    let skip: std::collections::HashSet<i64> =
        additions.iter().map(|c| c.id).chain(deletions.iter().map(|c| c.id)).collect();
    fn strip_stubs(e: &XmlElement, skip: &std::collections::HashSet<i64>) -> XmlElement {
        let mut out = e.clone();
        out.children = e
            .children
            .iter()
            .filter(|c| {
                !(c.tag == CONTROL_STUB
                    && c.attrs.get("id").and_then(|v| v.parse::<i64>().ok()).is_some_and(|id| skip.contains(&id)))
            })
            .map(|c| strip_stubs(c, skip))
            .collect();
        out
    }
    let (shell_a, shell_b) = if skip.is_empty() {
        (baseline.shell.clone(), edited.shell.clone())
    } else {
        (strip_stubs(&baseline.shell, &skip), strip_stubs(&edited.shell, &skip))
    };

    let mut diffs = Vec::new();
    diff_elements(&shell_a, &shell_b, "", &mut diffs);
    let owner = format!("form '{}'", baseline.name);
    for (path, msg) in diffs {
        problems.push(unsupported(&owner, None, edited.shell.line, &path, &msg));
    }

    let mut edits = Vec::new();
    for c in &edited.controls {
        let Some(b) = base_ids.get(&c.id) else { continue }; // additions: no baseline counterpart
        let mut diffs = Vec::new();
        let (rest_a, rest_b) = if skip.is_empty() {
            (b.rest.clone(), c.rest.clone())
        } else {
            (strip_stubs(&b.rest, &skip), strip_stubs(&c.rest, &skip))
        };
        // Added controls (sparse user XML vs full decoded projection) compare identity and values only.
        if !skip_rest.contains(&c.id) {
            diff_elements(&rest_a, &rest_b, "", &mut diffs);
        }
        let owner = format!("control {}", b.describe());
        for (path, msg) in diffs {
            problems.push(unsupported(&owner, Some(&b.type_name), c.line, &path, &msg));
        }
        let mut changed = c.values.changed_from(&b.values);
        if skip_rest.contains(&c.id) {
            if b.values.pages.is_empty() {
                changed.retain(|p| *p != Property::Pages);
            }
            if b.values.columns.is_empty() {
                changed.retain(|p| *p != Property::Columns);
            }
            if b.values.value_type.is_none() {
                changed.retain(|p| *p != Property::ValueType);
            }
            if b.values.buttons.is_empty() {
                changed.retain(|p| *p != Property::Buttons);
            }
        }
        if !changed.is_empty() {
            edits.push(ControlEdit {
                id: c.id,
                name: c.name.clone(),
                type_name: c.type_name.clone(),
                old: b.values.clone(),
                new: c.values.clone(),
                changed,
            });
        }
    }
    let form_caption_changed = baseline.form_caption != edited.form_caption;
    let old_form_size = (baseline.form_width, baseline.form_height);
    let new_form_size = (edited.form_width, edited.form_height);
    let form_size_changed = old_form_size != new_form_size && new_form_size.0.is_some() && new_form_size.1.is_some();
    let form_events_changed = baseline.form_events != edited.form_events;
    let attributes_changed = baseline.attributes != edited.attributes;
    let form_props_changed = baseline.form_props != edited.form_props;
    if problems.is_empty() {
        Ok(FormDiff {
            edits,
            additions,
            deletions,
            form_caption_changed,
            new_form_caption: edited.form_caption.clone(),
            form_size_changed,
            old_form_size,
            new_form_size,
            form_events_changed,
            old_form_events: baseline.form_events.clone(),
            new_form_events: edited.form_events.clone(),
            attributes_changed,
            old_attributes: baseline.attributes.clone(),
            new_attributes: edited.attributes.clone(),
            form_props_changed,
            new_form_props: edited.form_props.clone(),
        })
    } else {
        Err(fail(&baseline.name, &edited.label, problems))
    }
}

/// Everything the property patchers need: the verified supported edits plus the control index.
#[derive(Debug, Clone)]
pub struct FormPatchPlan<'a> {
    pub source: &'a SpannedMetadata,
    pub index: ControlIndex<'a>,
    pub diff: FormDiff,
}

impl<'a> FormPatchPlan<'a> {
    /// No layout edits: pack must leave the form stream untouched.
    pub fn is_empty(&self) -> bool {
        self.diff.is_empty()
    }

    /// The original binary control (`Node` + `SpanNode`) that `edit` applies to.
    pub fn entry(&self, edit: &ControlEdit) -> &ControlEntry<'a> {
        self.index.get(edit.id).expect("every edit was validated against the index")
    }

    /// Edits paired with their binary controls.
    pub fn targets(&self) -> impl Iterator<Item = (&ControlEdit, &ControlEntry<'a>)> {
        self.diff.edits.iter().map(|e| (e, self.entry(e)))
    }
}

/// Full planning pipeline: parse both XML documents, index the original stream, validate both
/// projections against it and diff the whole projection. No stream bytes are changed.
pub fn plan_form_patch<'a>(source: &'a SpannedMetadata, baseline_xml: &[u8], edited_xml: &[u8]) -> Result<FormPatchPlan<'a>> {
    let baseline = parse_form_xml(baseline_xml, "baseline")?;
    let edited = parse_form_xml(edited_xml, "edited")?;
    let index = ControlIndex::build(source)?;
    index.validate(&baseline)?;
    index.validate_mode(&edited, true)?;
    let diff = diff_forms_mode(&baseline, &edited, true)?;
    Ok(FormPatchPlan { source, index, diff })
}

// ------------------------------------------------------------------ structural edits (add/remove)

/// XML events of a control: `(numeric tag, handler)` from its `<Events>` element.
fn xml_events(c: &ControlXml) -> Result<Vec<(i64, String)>> {
    let mut out = c.values.events.clone();
    for e in &c.rest.children {
        if e.tag != "Events" {
            continue;
        }
        for ev in &e.children {
            let tag = ev
                .attrs
                .get("id")
                .and_then(|v| v.parse::<i64>().ok())
                .ok_or_else(|| layout_err(format!("{}: event <{}> has a missing or non-integer id", c.describe(), ev.tag)))?;
            out.push((tag, ev.text.trim().to_string()));
        }
    }
    Ok(out)
}

/// The page slot of a new control: the page NAME in XML resolved to the page index of the
/// parent panel (or the form's root panel for top-level pages).
fn resolve_page(index: &ControlIndex, parent: Option<i64>, page: Option<&str>) -> Result<i64> {
    let Some(page) = page else { return Ok(0) };
    // The root panel is not a full control record: its info node is children[1] and the form's
    // width/height live in the section at root[1][3..4]. Real panels go through control_parts.
    let (info, width, height) = match parent {
        Some(id) => {
            let entry = index
                .get(id)
                .ok_or_else(|| layout_err(format!("added control on page '{page}': parent id {id} not found")))?;
            let panel_item = Item::Node(entry.control.node.clone());
            let parts = control_parts(&panel_item).map_err(|e| layout_err(e.to_string()))?;
            let gi = |i: usize| int(&parts.geom.children[i], "geom").map_err(|e| layout_err(e.to_string()));
            let (left, top) = (gi(1)?, gi(2)?);
            (parts.info.clone(), gi(3)? - left, gi(4)? - top)
        }
        None => {
            let panel = index.root_panel()?;
            let info = panel
                .node
                .node(1)
                .ok_or_else(|| layout_err("form stream: root panel has no info node"))?
                .clone();
            let section = index.at_path(&[1])?;
            let gi = |i: usize| {
                section
                    .node
                    .children
                    .get(i)
                    .and_then(|c| c.as_atom())
                    .and_then(|a| a.value.parse::<i64>().ok())
                    .ok_or_else(|| layout_err("form stream: width/height missing"))
            };
            (info, gi(3)?, gi(4)?)
        }
    };
    let (_show, pages) = panel_pages(&info, width, height);
    pages
        .iter()
        .position(|p| p["name"].as_str() == Some(page))
        .map(|i| i as i64)
        .ok_or_else(|| layout_err(format!("added control: page '{page}' not found in its panel")))
}

fn bare(v: i64) -> Item {
    Item::Atom(Atom::new(BARE, v.to_string()))
}

/// Write the requested geometry/visibility/order/page into a cloned prototype (semantic, no spans).
fn set_clone_geometry(n: &mut Node, v: &ControlValues, page: i64) -> Result<()> {
    let len = n.len();
    let Some(Item::Node(g)) = n.children.get_mut(len - 3) else {
        return Err(layout_err("prototype: geometry node missing"));
    };
    if g.len() < 18 {
        return Err(layout_err("prototype: geometry node too short"));
    }
    let right = v.left + v.width;
    let bottom = v.top + v.height;
    for (i, val) in [(1, v.left), (2, v.top), (3, right), (4, bottom), (5, i64::from(v.visible))] {
        g.children[i] = bare(val);
    }
    let mut idx = 12usize;
    for _ in 0..6 {
        let Some(Item::Atom(a)) = g.children.get(idx) else {
            return Err(layout_err("prototype: geometry incoming list size missing"));
        };
        let size: usize = a.value.parse().map_err(|_| layout_err("prototype: bad incoming list size"))?;
        idx += 1 + size;
        if idx > g.len() {
            return Err(layout_err("prototype: incoming lists overrun the geometry node"));
        }
    }
    if idx + 3 > g.len() {
        return Err(layout_err("prototype: geometry tail missing"));
    }
    g.children[idx] = bare(page);
    g.children[idx + 1] = bare(v.z_order);
    g.children[idx + 2] = bare(v.tab_order);
    Ok(())
}

/// Build a localized node `{1,N,{"lang","text"}...}`.
fn localized_node(items: &[(String, String)]) -> Node {
    let mut n = Node { children: vec![Item::Atom(Atom::new(BARE, "1")), Item::Atom(Atom::new(BARE, items.len().to_string()))] };
    for (l, t) in items {
        n.children.push(Item::Node(Node {
            children: vec![Item::Atom(Atom::new(QUOTED, l)), Item::Atom(Atom::new(QUOTED, t))],
        }));
    }
    n
}

/// Write the caption into a cloned prototype at the decoder's caption location.
fn set_clone_caption(n: &mut Node, type_name: &str, caption: &[(String, String)]) -> Result<()> {
    let owned = Node { children: n.children.clone() };
    let owned_item = Item::Node(owned);
    let info_index = {
        let parts = control_parts(&owned_item).map_err(|e| layout_err(e.to_string()))?;
        // positions in the temporary clone are the same as in `n`
        let on = owned_item.as_node().unwrap();
        on.children
            .iter()
            .position(|c| matches!(c, Item::Node(x) if std::ptr::eq(x, parts.info)))
            .ok_or_else(|| layout_err("prototype: info node not found"))?
    };
    if let Some(path) = caption_path(Some(type_name)) {
        let mut cur: &mut Node = match n.children[info_index].as_node_mut() {
            Some(x) => x,
            None => return Err(layout_err("prototype: info node is not a node")),
        };
        for &i in path {
            cur = match cur.children.get_mut(i as usize).and_then(Item::as_node_mut) {
                Some(x) => x,
                None => return Err(layout_err(format!("prototype: caption path {path:?} missing"))),
            };
        }
        *cur = localized_node(caption);
        return Ok(());
    }
    // Fallback types (Button, ...): the first localized node among the properties block.
    let Item::Node(info) = &mut n.children[info_index] else { unreachable!() };
    let Some(Item::Node(props)) = info.children.get_mut(1) else {
        return Err(layout_err("prototype: properties block missing"));
    };
    let at = props.children.iter().position(|c| localized(c).is_some_and(|l| !l.is_empty()));
    match (at, caption.is_empty()) {
        (Some(i), _) => {
            props.children[i] = Item::Node(localized_node(caption));
            Ok(())
        }
        (None, true) => Ok(()),
        (None, false) => Err(layout_err(format!("prototype '{type_name}': no caption slot to write"))),
    }
}

fn clone_geom_mut(n: &mut Node) -> Result<&mut Node> {
    let owned = Item::Node(Node { children: n.children.clone() });
    let geom_index = {
        let parts = control_parts(&owned).map_err(|e| layout_err(e.to_string()))?;
        let on = owned.as_node().unwrap();
        on.children
            .iter()
            .position(|c| matches!(c, Item::Node(x) if std::ptr::eq(x, parts.geom)))
            .ok_or_else(|| layout_err("prototype: geom node not found"))?
    };
    n.children[geom_index]
        .as_node_mut()
        .ok_or_else(|| layout_err("prototype: geom is not a node"))
}

fn clone_info_mut(n: &mut Node) -> Result<&mut Node> {
    let owned = Item::Node(Node { children: n.children.clone() });
    let info_index = {
        let parts = control_parts(&owned).map_err(|e| layout_err(e.to_string()))?;
        let on = owned.as_node().unwrap();
        on.children
            .iter()
            .position(|c| matches!(c, Item::Node(x) if std::ptr::eq(x, parts.info)))
            .ok_or_else(|| layout_err("prototype: info node not found"))?
    };
    n.children[info_index]
        .as_node_mut()
        .ok_or_else(|| layout_err("prototype: info is not a node"))
}

fn nav_mut<'a>(mut cur: &'a mut Node, path: &[usize]) -> Result<&'a mut Node> {
    for &i in path {
        cur = cur
            .children
            .get_mut(i)
            .and_then(Item::as_node_mut)
            .ok_or_else(|| layout_err(format!("prototype: path {path:?} missing")))?;
    }
    Ok(cur)
}

fn format_type_link(tl: &TypeLinkSpec) -> String {
    match &tl.field_guid {
        None => format!("{{1,1,{{0,{TYPE_LINK_GUID},{{2,1,{{{}}}}}}}}}", tl.attr_id),
        Some(fg) => format!("{{1,1,{{0,{TYPE_LINK_GUID},{{2,2,{{{}}},{{0,{fg}}}}}}}}}", tl.attr_id),
    }
}

fn horizontal_align_code(s: &str) -> i64 {
    match s {
        "Center" => 1,
        "Right" => 2,
        "Auto" => 4,
        _ => 0,
    }
}

fn vertical_align_code(s: &str) -> i64 {
    match s {
        "Center" => 1,
        "Bottom" => 2,
        _ => 0,
    }
}

/// Child index of the string-extension sub-block inside the counted list `info[3]`.
fn string_ext_index(info: &Node) -> Option<usize> {
    let list = info.children.get(3)?.as_node()?;
    list.children.iter().enumerate().skip(1).find_map(|(i, it)| {
        let sub = it.as_node()?;
        (sub.children.first()?.as_atom()?.value == STRING_EXT_GUID).then_some(i)
    })
}

/// Body node (`{4, min, max, MultiLine, Mask, PasswordMode, ExtendedEdit}`) of the string extension, when present.
fn string_ext_body<'a>(info: NodeRef<'a>) -> Option<NodeRef<'a>> {
    let i = string_ext_index(info.node)?;
    info.child(3)?.child(i)?.child(1)
}

fn string_ext_body_mut(info: &mut Node) -> Option<&mut Node> {
    let i = string_ext_index(info)?;
    info.children.get_mut(3)?.as_node_mut()?.children.get_mut(i)?.as_node_mut()?.children.get_mut(1)?.as_node_mut()
}

/// Apply the `InputField` flags, alignment, mask and type link to a donor-cloned `info` node.
fn set_input_extras(info: &mut Node, v: &ControlValues) -> Result<()> {
    {
        let q = nav_mut(info, &[2, 0])?;
        for (idx, flag) in [(5, v.password_mode), (24, v.extended_edit), (26, v.multi_line)] {
            if let Some(b) = flag {
                q.children[idx] = bare(i64::from(b));
            }
        }
        if let Some(b) = v.text_edit {
            q.children[12] = bare(i64::from(!b));
        }
        if let Some(m) = &v.mask {
            q.children[21] = Item::Atom(Atom::new(QUOTED, m.clone()));
        }
        if !v.horizontal_align.is_empty() {
            q.children[17] = bare(horizontal_align_code(&v.horizontal_align));
        }
        if let Some(a) = &v.vertical_align {
            q.children[18] = bare(vertical_align_code(a));
        }
    }
    if let Some(b) = v.text_edit {
        info.children[5] = bare(i64::from(!b));
    }
    if let Some(b) = v.choose_type {
        info.children[6] = bare(i64::from(b));
    }
    if let Some(tl) = &v.type_link {
        if info.len() <= 8 {
            return Err(layout_err("prototype: info[8] (TypeLink) missing"));
        }
        info.children[8] =
            Item::Node(crate::brace::parse_metadata(format_type_link(tl).as_bytes()).map_err(|e| layout_err(e.to_string()))?);
    }
    if let Some(body) = string_ext_body_mut(info) {
        for (idx, flag) in [(3, v.multi_line), (5, v.password_mode), (6, v.extended_edit)] {
            if let Some(b) = flag {
                body.children[idx] = bare(i64::from(b));
            }
        }
        if let Some(m) = &v.mask {
            body.children[4] = Item::Atom(Atom::new(QUOTED, m.clone()));
        }
    }
    Ok(())
}

// ---- extended catalog slots (fixed `info` positions)

type CodeTable = &'static [(&'static str, i64)];

const H_ALIGN: CodeTable = &[("Left", 0), ("Center", 1), ("Right", 2), ("Auto", 4)];
const V_ALIGN: CodeTable = &[("Top", 0), ("Center", 1), ("Bottom", 2)];
const PICTURE_LOCATION: CodeTable = &[("Left", 0), ("Right", 1), ("Top", 2), ("Bottom", 3)];
const BUTTON_PICTURE_SIZE: CodeTable = &[("RealSize", 0), ("Stretch", 1), ("AutoSize", 2), ("Proportionally", 3), ("Tile", 4)];
const LABEL_PICTURE_SIZE: CodeTable = &[("RealSize", 0), ("Stretch", 1), ("Proportionally", 2), ("Tile", 3), ("AutoSize", 4)];
const ORIENTATION: CodeTable = &[("Vertical", 0), ("Horizontal", 1), ("Auto", 2)];
const MARKING_APPEARANCE: CodeTable = &[("DontShow", 0), ("TopOrLeft", 1), ("BottomOrRight", 2), ("BothSides", 3)];
const SHOW_STYLE: CodeTable = &[("Broken", 0), ("Smooth", 1)];
const ROW_INPUT_MODE: CodeTable = &[("AtListEnd", 0), ("AtWindowBottom", 1), ("AfterCurrentRow", 2), ("BeforeCurrentRow", 3)];
const ROW_SELECTION_MODE: CodeTable = &[("Cell", 0), ("Row", 1)];
const CALENDAR_SELECTION: CodeTable = &[("Single", 0), ("Multi", 1), ("Interval", 2)];
const SCROLLING_TEXT: CodeTable = &[
    ("DontUse", 0),
    ("VerySlow", 1),
    ("Slow", 2),
    ("Normal", 3),
    ("Fast", 4),
    ("VeryFast", 5),
];
const PANEL_PICTURE_LOCATION: CodeTable = &[("TopLeft", 0), ("Center", 1)];
const SHOW_SELECTION: CodeTable = &[("WhenActive", 0), ("Always", 1)];
const COLLAPSE_MODE: CodeTable = &[("None", 0), ("Top", 1), ("Bottom", 2), ("Left", 3), ("Right", 4)];
const PAGE_ORIENTATION: CodeTable = &[("Portrait", 0), ("Landscape", 1)];

/// Properties that live in a fixed `info` slot (`ext_slot`); the others keep their dedicated mapping.
const EXT_PROPS: &[Property] = &[
    Property::FieldBackColor,
    Property::FieldTextColor,
    Property::ButtonBackColor,
    Property::ButtonTextColor,
    Property::HorizontalAlign,
    Property::VerticalAlign,
    Property::MultiLine,
    Property::MarkNegatives,
    Property::TitleLocation,
    Property::ReadOnly,
    Property::ChoiceButton,
    Property::ClearButton,
    Property::SpinButton,
    Property::OpenButton,
    Property::SelectionMode,
    Property::PictureLocation,
    Property::PictureSize,
    Property::Format,
    Property::SelectionValue,
    Property::ChoiceListWidth,
    Property::MinValue,
    Property::MaxValue,
    Property::SeparateByPage,
    Property::AutoTabOrder,
    Property::ScrollPageMode,
    Property::Orientation,
    Property::Secondary,
    Property::Zoomable,
    Property::UseContextMenu,
    Property::NonselectedPictureText,
    Property::EnableStartDrag,
    Property::EnableDrag,
    Property::ChoiceMode,
    Property::ShowPicture,
    Property::ShowCheckBox,
    Property::Step,
    Property::LargeStep,
    Property::MarkingStep,
    Property::MarkingAppearance,
    Property::ShowStyle,
    Property::ShowPercent,
    Property::SelectionBackColor,
    Property::SelectionTextColor,
    Property::HolidayColor,
    Property::ShowCurrentDate,
    Property::CalendarNavigation,
    Property::HeaderBackColor,
    Property::HeaderTextColor,
    Property::FooterBackColor,
    Property::FooterTextColor,
    Property::AlternationRowBackColor,
    Property::FixedLeft,
    Property::FixedRight,
    Property::RowInputMode,
    Property::RowSelectionMode,
    Property::HeaderHeight,
    Property::FooterHeight,
    Property::HeaderFont,
    Property::FooterFont,
    Property::BorderStyle,
    Property::BorderColor,
    Property::Output,
    Property::HorizontalScrollBar,
    Property::VerticalScrollBar,
    Property::Shortcut,
    Property::Picture,
    Property::ChoiceButtonPicture,
    Property::MenuMode,
    Property::ScrollingText,
    Property::LabelPictureLocation,
    Property::PanelPictureLocation,
    Property::ChoiceListHeight,
    Property::ShowSelection,
    Property::BeginOfDisplayPeriod,
    Property::EndOfDisplayPeriod,
    Property::DefaultButton,
    Property::AutoContextMenu,
    Property::Collapse,
    Property::EmbeddedIntoCell,
    Property::Print,
    Property::Protection,
    Property::ShowGrid,
    Property::ShowHeaders,
    Property::ShowGroups,
    Property::BlackAndWhiteView,
    Property::TotalsRight,
    Property::TotalsBelow,
    Property::PrintScale,
    Property::FitToPage,
    Property::BlackAndWhite,
    Property::Collate,
    Property::Copies,
    Property::PerPage,
    Property::PageOrientation,
    Property::BottomMargin,
    Property::TopMargin,
    Property::LeftMargin,
    Property::RightMargin,
    Property::HeaderSize,
    Property::FooterSize,
    Property::DuplexPrinting,
    Property::PrintAccuracy,
    Property::StepDirection,
    Property::SaveViewProperties,
    Property::FixedBackground,
];

struct ExtSlot {
    path: Vec<usize>,
    idx: usize,
    table: CodeTable,
}

/// Value to store in an [`ExtSlot`].
enum Want {
    Atom(i64),
    Node(String),
    Color(Option<ColorSpec>),
    Font(Option<FontSpec>),
    Loc(LocItems),
    Shortcut(ShortcutSpec),
    Picture(PictureSpec),
}

/// Index of the `Page` list inside the panel parameters `info[1]` (after the six counted anchor lists).
fn panel_page_list_idx(p: &Node) -> Option<usize> {
    let mut idx = 2usize;
    for _ in 0..6 {
        let sz = p.children.get(idx)?.as_atom()?.value.parse::<usize>().ok()?;
        idx += 1 + sz;
    }
    Some(idx + 3)
}

/// Index of the default control slot (`tail[1]`) in the root panel parameters: after the page list, the three
/// panel flags and the counted rows list comes the tail whose second item holds the default control id.
fn default_control_slot(p: &Node) -> Option<usize> {
    let rows_idx = panel_page_list_idx(p)? + 4;
    let rows = p.children.get(rows_idx)?.as_atom()?.value.parse::<usize>().ok()?;
    Some(rows_idx + 1 + rows + 1)
}

/// Form style node: `{0}` without a style, `{0,<uuid>}` with one.
fn format_style_node(style: &str) -> String {
    if style.is_empty() {
        "{0}".to_string()
    } else {
        format!("{{0,{}}}", style.to_lowercase())
    }
}

fn ext_slot(type_name: &str, prop: Property, info: &Node) -> Option<ExtSlot> {
    use Property as P;
    let color_idx = match prop {
        P::FieldBackColor => Some(7),
        P::FieldTextColor => Some(8),
        P::ButtonBackColor => Some(9),
        P::ButtonTextColor => Some(10),
        _ => None,
    };
    if let Some(idx) = color_idx {
        let path = patch_app_path(type_name)?.iter().map(|&i| i as usize).collect();
        return Some(ExtSlot { path, idx, table: &[] });
    }
    let base_idx: Option<(usize, CodeTable)> = match prop {
        P::EmbeddedIntoCell => Some((13, &[])),
        P::Collapse => Some((14, COLLAPSE_MODE)),
        P::AutoContextMenu => Some((16, &[])),
        P::Print => Some((17, &[])),
        _ => None,
    };
    if let Some((idx, table)) = base_idx {
        let path = patch_app_path(type_name)?.iter().map(|&i| i as usize).collect();
        return Some(ExtSlot { path, idx, table });
    }
    if prop == P::BorderStyle && type_name != "UsualGroup" {
        let path = match type_name {
            "SpreadsheetDocumentField" => vec![10],
            "HTMLDocumentField" => vec![4],
            _ => {
                let mut p: Vec<usize> = patch_app_path(type_name)?.iter().map(|&i| i as usize).collect();
                p.push(11);
                p
            }
        };
        return Some(ExtSlot { path, idx: 3, table: &[] });
    }
    let (path, idx, table): (&[usize], usize, CodeTable) = match (type_name, prop) {
        ("SpreadsheetDocumentField", P::BorderColor) => (&[], 9, &[]),
        ("HTMLDocumentField", P::BorderColor) => (&[], 3, &[]),
        ("SpreadsheetDocumentField", P::EnableStartDrag) => (&[], 25, &[]),
        ("SpreadsheetDocumentField", P::EnableDrag) => (&[], 26, &[]),
        ("SpreadsheetDocumentField", P::ShowSelection) => (&[], 27, SHOW_SELECTION),
        ("SpreadsheetDocumentField", P::Output) => (&[], 28, &[]),
        ("SpreadsheetDocumentField", P::HorizontalScrollBar) => (&[], 31, &[]),
        ("SpreadsheetDocumentField", P::VerticalScrollBar) => (&[], 32, &[]),
        ("SpreadsheetDocumentField", P::ReadOnly) => (&[], 12, &[]),
        ("SpreadsheetDocumentField", P::Protection) => (&[], 13, &[]),
        ("SpreadsheetDocumentField", P::ShowGrid) => (&[], 18, &[]),
        ("SpreadsheetDocumentField", P::ShowHeaders) => (&[], 19, &[]),
        ("SpreadsheetDocumentField", P::ShowGroups) => (&[], 20, &[]),
        ("SpreadsheetDocumentField", P::BlackAndWhiteView) => (&[], 21, &[]),
        ("SpreadsheetDocumentField", P::TotalsRight) => (&[], 22, &[]),
        ("SpreadsheetDocumentField", P::TotalsBelow) => (&[], 23, &[]),
        ("SpreadsheetDocumentField", P::PrintScale) => (&[14], 3, &[]),
        ("SpreadsheetDocumentField", P::FitToPage) => (&[14], 4, &[]),
        ("SpreadsheetDocumentField", P::BlackAndWhite) => (&[14], 5, &[]),
        ("SpreadsheetDocumentField", P::Collate) => (&[14], 6, &[]),
        ("SpreadsheetDocumentField", P::Copies) => (&[14], 7, &[]),
        ("SpreadsheetDocumentField", P::PerPage) => (&[14], 8, &[]),
        ("SpreadsheetDocumentField", P::PageOrientation) => (&[14], 9, PAGE_ORIENTATION),
        ("SpreadsheetDocumentField", P::BottomMargin) => (&[14], 10, &[]),
        ("SpreadsheetDocumentField", P::TopMargin) => (&[14], 11, &[]),
        ("SpreadsheetDocumentField", P::LeftMargin) => (&[14], 12, &[]),
        ("SpreadsheetDocumentField", P::RightMargin) => (&[14], 13, &[]),
        ("SpreadsheetDocumentField", P::HeaderSize) => (&[14], 14, &[]),
        ("SpreadsheetDocumentField", P::FooterSize) => (&[14], 15, &[]),
        ("SpreadsheetDocumentField", P::DuplexPrinting) => (&[14], 16, &[]),
        ("SpreadsheetDocumentField", P::PrintAccuracy) => (&[14], 17, &[]),
        ("SpreadsheetDocumentField", P::StepDirection) => (&[14], 18, &[]),
        ("SpreadsheetDocumentField", P::SaveViewProperties) => (&[14], 19, &[]),
        ("SpreadsheetDocumentField", P::FixedBackground) => (&[14], 20, &[]),
        ("Button", P::DefaultButton) => (&[1], 5, &[]),
        ("ListBox", P::ReadOnly) => (&[1], 6, &[]),
        ("GraphicalSchemaField", P::ReadOnly) => (&[], 4, &[]),
        ("GraphicalSchemaField", P::Output) => (&[], 5, &[]),
        ("TextDocumentField", P::ReadOnly) => (&[], 5, &[]),
        ("TextDocumentField" | "HTMLDocumentField", P::Output) => (&[], 6, &[]),
        ("Button", P::HorizontalAlign) => (&[1], 3, H_ALIGN),
        ("CheckBox", P::HorizontalAlign) => (&[1, 0], 4, H_ALIGN),
        ("RadioButton", P::HorizontalAlign) => (&[2, 0], 4, H_ALIGN),
        ("Button" | "LabelDecoration", P::VerticalAlign) => (&[1], 4, V_ALIGN),
        ("CheckBox", P::VerticalAlign) => (&[1, 0], 5, V_ALIGN),
        ("RadioButton", P::VerticalAlign) => (&[2, 0], 5, V_ALIGN),
        ("Button", P::MenuMode) => (&[1], 5, &[]),
        ("Button", P::Picture) => (&[1], 8, &[]),
        ("Button", P::Shortcut) => (&[1], 9, &[]),
        ("Button", P::MultiLine) => (&[1], 10, &[]),
        ("LabelDecoration", P::MarkNegatives) => (&[1], 6, &[]),
        ("LabelDecoration", P::Shortcut) => (&[1], 8, &[]),
        ("LabelDecoration", P::ScrollingText) => (&[1], 9, SCROLLING_TEXT),
        ("LabelDecoration", P::Picture) => (&[1, 12], 2, &[]),
        ("LabelDecoration", P::LabelPictureLocation) => (&[1, 12], 6, PICTURE_LOCATION),
        ("RadioButton", P::TitleLocation) => (&[2, 0], 3, &[]),
        ("ChoiceField", P::ChoiceButton) => (&[1], 7, &[]),
        ("ChoiceField", P::ClearButton) => (&[1], 8, &[]),
        ("ChoiceField", P::SpinButton) => (&[1], 9, &[]),
        ("ChoiceField", P::OpenButton) => (&[1], 10, &[]),
        ("ChoiceField", P::ReadOnly) => (&[1], 13, &[]),
        ("ChoiceField", P::Picture) => (&[1], 28, &[]),
        ("ChoiceField", P::ChoiceButtonPicture) => (&[1], 29, &[]),
        ("ChoiceField", P::ChoiceListHeight) => (&[1], 31, &[]),
        ("ChoiceField", P::ChoiceListWidth) => (&[1], 32, &[]),
        ("ChoiceField", P::Shortcut) => (&[1], 33, &[]),
        ("InputField", P::Picture) => (&[2, 0], 28, &[]),
        ("InputField", P::ChoiceButtonPicture) => (&[2, 0], 29, &[]),
        ("InputField", P::ChoiceListHeight) => (&[2, 0], 31, &[]),
        ("InputField", P::ChoiceListWidth) => (&[2, 0], 32, &[]),
        ("InputField", P::Shortcut) => (&[2, 0], 33, &[]),
        ("CalendarField", P::BeginOfDisplayPeriod) => (&[1], 5, &[]),
        ("CalendarField", P::EndOfDisplayPeriod) => (&[1], 6, &[]),
        ("CalendarField", P::SelectionMode) => (&[1], 9, CALENDAR_SELECTION),
        ("Button", P::PictureLocation) => (&[1], 6, PICTURE_LOCATION),
        ("Button", P::PictureSize) => (&[1], 7, BUTTON_PICTURE_SIZE),
        ("LabelDecoration", P::PictureSize) => (&[1], 13, LABEL_PICTURE_SIZE),
        ("PictureDecoration" | "PictureField", P::PictureSize) => (&[1], 2, LABEL_PICTURE_SIZE),
        ("PictureDecoration" | "PictureField", P::Picture) => (&[1, 4], 2, &[]),
        ("PictureDecoration" | "PictureField", P::Shortcut) => (&[1], 5, &[]),
        ("LabelDecoration", P::Format) => (&[1], 10, &[]),
        ("InputField", P::Format) => (&[2, 0], 34, &[]),
        ("RadioButton", P::SelectionValue) => (&[], 4, &[]),
        ("InputField", P::MinValue) => (&[2, 0], 19, &[]),
        ("InputField", P::MaxValue) => (&[2, 0], 20, &[]),
        ("TrackBar" | "ProgressBar", P::MinValue) => (&[1], 2, &[]),
        ("TrackBar" | "ProgressBar", P::MaxValue) => (&[1], 3, &[]),
        ("CommandBar" | "Splitter", P::Orientation) => (&[1], 2, ORIENTATION),
        ("TrackBar", P::Orientation) => (&[1], 6, ORIENTATION),
        ("ProgressBar", P::Orientation) => (&[1], 7, ORIENTATION),
        ("CommandBar", P::Secondary) => (&[1], 5, &[]),
        ("PictureDecoration" | "PictureField", P::Zoomable) => (&[1], 6, &[]),
        ("PictureDecoration" | "PictureField", P::UseContextMenu) => (&[1], 7, &[]),
        ("PictureDecoration" | "PictureField", P::EnableStartDrag) => (&[1], 8, &[]),
        ("PictureDecoration" | "PictureField", P::EnableDrag) => (&[1], 9, &[]),
        ("PictureDecoration" | "PictureField", P::NonselectedPictureText) => (&[1], 10, &[]),
        ("CalendarField", P::EnableStartDrag) => (&[1], 10, &[]),
        ("CalendarField", P::EnableDrag) => (&[1], 11, &[]),
        ("CalendarField", P::SelectionBackColor) => (&[1], 2, &[]),
        ("Table", P::HeaderBackColor) => (&[2, 1], 2, &[]),
        ("Table", P::HeaderTextColor) => (&[2, 1], 3, &[]),
        ("Table", P::FooterBackColor) => (&[2, 1], 4, &[]),
        ("Table", P::FooterTextColor) => (&[2, 1], 5, &[]),
        ("Table", P::AlternationRowBackColor) => (&[2, 1], 8, &[]),
        ("Table", P::FixedLeft) => (&[2, 1], 11, &[]),
        ("Table", P::FixedRight) => (&[2, 1], 12, &[]),
        ("Table", P::RowInputMode) => (&[2, 1], 13, ROW_INPUT_MODE),
        ("Table", P::RowSelectionMode) => (&[2, 1], 14, ROW_SELECTION_MODE),
        ("Table", P::HeaderHeight) => (&[2, 1], 16, &[]),
        ("Table", P::FooterHeight) => (&[2, 1], 17, &[]),
        ("Table", P::HeaderFont) => (&[2, 1], 18, &[]),
        ("Table", P::FooterFont) => (&[2, 1], 19, &[]),
        ("Table", P::SelectionBackColor) => (&[2, 1], 6, &[]),
        ("Table", P::SelectionTextColor) => (&[2, 1], 7, &[]),
        ("CalendarField", P::SelectionTextColor) => (&[1], 3, &[]),
        ("CalendarField", P::HolidayColor) => (&[1], 4, &[]),
        ("CalendarField", P::ShowCurrentDate) => (&[1], 7, &[]),
        ("CalendarField", P::CalendarNavigation) => (&[1], 8, &[]),
        ("ListBox", P::ShowPicture) => (&[1], 4, &[]),
        ("ListBox", P::ShowCheckBox) => (&[1], 5, &[]),
        ("TrackBar" | "ProgressBar", P::Step) => (&[1], 4, &[]),
        ("TrackBar", P::LargeStep) => (&[1], 5, &[]),
        ("TrackBar", P::MarkingAppearance) => (&[1], 7, MARKING_APPEARANCE),
        ("TrackBar", P::MarkingStep) => (&[1], 8, &[]),
        ("ProgressBar", P::ShowStyle) => (&[1], 5, SHOW_STYLE),
        ("ProgressBar", P::ShowPercent) => (&[1], 6, &[]),
        ("Panel", P::Picture | P::PictureSize | P::PanelPictureLocation) => {
            let base = panel_page_list_idx(info.children.get(1)?.as_node()?)?.checked_sub(3)?;
            let (idx, table): (usize, CodeTable) = match prop {
                P::Picture => (2, &[]),
                P::PictureSize => (6, LABEL_PICTURE_SIZE),
                _ => (7, PANEL_PICTURE_LOCATION),
            };
            return Some(ExtSlot { path: vec![1, base], idx, table });
        }
        ("Panel", P::SeparateByPage | P::AutoTabOrder | P::ScrollPageMode) => {
            let page = panel_page_list_idx(info.children.get(1)?.as_node()?)?;
            let off = match prop {
                P::SeparateByPage => 1,
                P::AutoTabOrder => 2,
                _ => 3,
            };
            return Some(ExtSlot { path: vec![1], idx: page + off, table: &[] });
        }
        _ => return None,
    };
    Some(ExtSlot { path: path.to_vec(), idx, table })
}

/// `SelectionValue` text -> the `{"U"}` / `{"B",n}` / `{"N",n}` / `{"S","s"}` value node.
fn selection_value_node(s: &str) -> String {
    match s {
        "" => "{\"U\"}".to_string(),
        "true" => "{\"B\",1}".to_string(),
        "false" => "{\"B\",0}".to_string(),
        _ => match s.parse::<i64>() {
            Ok(n) => format!("{{\"N\",{n}}}"),
            Err(_) => format!("{{\"S\",{}}}", quote(s)),
        },
    }
}

/// Requested value for the slot; `None` when the model does not set the property.
fn ext_want(v: &ControlValues, prop: Property, slot: &ExtSlot) -> Option<Want> {
    use Property as P;
    let flag = |b: Option<bool>| b.map(|b| Want::Atom(i64::from(b)));
    let int = |n: Option<i64>| n.map(Want::Atom);
    let code = |s: Option<&str>| -> Option<Want> {
        let s = s.filter(|s| !s.is_empty())?;
        slot.table.iter().find(|(n, _)| *n == s).map(|(_, c)| Want::Atom(*c))
    };
    let bare_or_node = |n: Option<i64>| {
        if slot.path == [2, 0] {
            n.map(|n| Want::Node(format!("{{\"N\",{n}}}")))
        } else {
            int(n)
        }
    };
    match prop {
        P::FieldBackColor => Some(Want::Color(v.field_back_color.clone())),
        P::FieldTextColor => Some(Want::Color(v.field_text_color.clone())),
        P::ButtonBackColor => Some(Want::Color(v.button_back_color.clone())),
        P::ButtonTextColor => Some(Want::Color(v.button_text_color.clone())),
        P::SelectionBackColor => v.selection_back_color.clone().map(|c| Want::Color(Some(c))),
        P::SelectionTextColor => v.selection_text_color.clone().map(|c| Want::Color(Some(c))),
        P::HolidayColor => v.holiday_color.clone().map(|c| Want::Color(Some(c))),
        P::HeaderBackColor => v.header_back_color.clone().map(|c| Want::Color(Some(c))),
        P::HeaderTextColor => v.header_text_color.clone().map(|c| Want::Color(Some(c))),
        P::FooterBackColor => v.footer_back_color.clone().map(|c| Want::Color(Some(c))),
        P::FooterTextColor => v.footer_text_color.clone().map(|c| Want::Color(Some(c))),
        P::AlternationRowBackColor => v.alternation_row_back_color.clone().map(|c| Want::Color(Some(c))),
        P::HeaderFont => v.header_font.clone().map(|f| Want::Font(Some(f))),
        P::FooterFont => v.footer_font.clone().map(|f| Want::Font(Some(f))),
        P::FixedLeft => int(v.fixed_left),
        P::FixedRight => int(v.fixed_right),
        P::HeaderHeight => int(v.header_height),
        P::FooterHeight => int(v.footer_height),
        P::RowInputMode => code(v.row_input_mode.as_deref()),
        P::RowSelectionMode => code(v.row_selection_mode.as_deref()),
        P::HorizontalAlign => code(Some(&v.horizontal_align)),
        P::VerticalAlign => code(v.vertical_align.as_deref()),
        P::MultiLine => flag(v.multi_line),
        P::MarkNegatives => flag(Some(v.mark_negatives)),
        P::TitleLocation => Some(Want::Atom(if v.title_location == "Left" { 0 } else { 1 })),
        P::ReadOnly => flag(Some(v.read_only)),
        P::ChoiceButton => flag(v.choice_button),
        P::ClearButton => flag(v.clear_button),
        P::SpinButton => flag(v.spin_button),
        P::OpenButton => flag(v.open_button),
        P::SelectionMode => Some(Want::Atom(
            slot.table.iter().find(|(n, _)| *n == v.selection_mode).map_or(0, |(_, c)| *c),
        )),
        P::PictureLocation => code(v.picture_location.as_deref()),
        P::PictureSize => code(v.picture_size.as_deref()),
        P::Format => Some(Want::Loc(v.format.clone())),
        P::NonselectedPictureText => Some(Want::Loc(v.nonselected_picture_text.clone())),
        P::SelectionValue => v.selection_value.as_deref().map(|s| Want::Node(selection_value_node(s))),
        P::ChoiceListWidth => int(v.choice_list_width),
        P::MinValue => bare_or_node(v.min_value),
        P::MaxValue => bare_or_node(v.max_value),
        P::SeparateByPage => flag(v.separate_by_page),
        P::AutoTabOrder => flag(v.auto_tab_order),
        P::ScrollPageMode => flag(v.scroll_page_mode),
        P::Orientation => code(v.orientation.as_deref()),
        P::Secondary => flag(v.secondary),
        P::Zoomable => flag(v.zoomable),
        P::UseContextMenu => flag(v.use_context_menu),
        P::EnableStartDrag => flag(v.enable_start_drag),
        P::EnableDrag => flag(v.enable_drag),
        P::ShowPicture => flag(v.show_picture),
        P::ShowCheckBox => flag(v.show_check_box),
        P::Step => int(v.step),
        P::LargeStep => int(v.large_step),
        P::MarkingStep => int(v.marking_step),
        P::MarkingAppearance => code(v.marking_appearance.as_deref()),
        P::ShowStyle => code(v.show_style.as_deref()),
        P::ShowPercent => flag(v.show_percent),
        P::ShowCurrentDate => flag(v.show_current_date),
        P::CalendarNavigation => flag(v.calendar_navigation),
        P::BorderStyle => int(v.border_style),
        P::BorderColor => Some(Want::Color(v.border_color.clone())),
        P::Output => v.output.as_deref().map(|s| Want::Atom(output_code(s))),
        P::HorizontalScrollBar => Some(Want::Atom(i64::from(matches!(
            v.horizontal_scroll_bar.as_str(),
            "Use" | "Auto"
        )))),
        P::VerticalScrollBar => Some(Want::Atom(i64::from(matches!(
            v.vertical_scroll_bar.as_str(),
            "Use" | "Auto"
        )))),
        P::Shortcut => v.shortcut.map(Want::Shortcut),
        P::Picture => v.picture.clone().map(Want::Picture),
        P::ChoiceButtonPicture => v.choice_button_picture.clone().map(Want::Picture),
        P::MenuMode => flag(v.menu_mode),
        P::ScrollingText => code(v.scrolling_text.as_deref()),
        P::LabelPictureLocation => code(v.label_picture_location.as_deref()),
        P::PanelPictureLocation => code(v.panel_picture_location.as_deref()),
        P::ChoiceListHeight => int(v.choice_list_height),
        P::ShowSelection => code(v.show_selection.as_deref()),
        P::BeginOfDisplayPeriod => int(v.begin_of_display_period),
        P::EndOfDisplayPeriod => int(v.end_of_display_period),
        P::DefaultButton => flag(v.default_button),
        P::AutoContextMenu => v.auto_context_menu.map(|b| Want::Atom(if b { 0 } else { 2 })),
        P::Collapse => code(v.collapse.as_deref()),
        P::EmbeddedIntoCell => flag(v.embedded_into_cell),
        P::Print => flag(v.print),
        P::Protection => flag(v.protection),
        P::ShowGrid => flag(v.show_grid),
        P::ShowHeaders => flag(v.show_headers),
        P::ShowGroups => flag(v.show_groups),
        P::BlackAndWhiteView => flag(v.black_and_white_view),
        P::TotalsRight => flag(v.totals_right),
        P::TotalsBelow => flag(v.totals_below),
        P::PrintScale => int(v.print_scale),
        P::FitToPage => flag(v.fit_to_page),
        P::BlackAndWhite => flag(v.black_and_white),
        P::Collate => flag(v.collate),
        P::Copies => int(v.copies),
        P::PerPage => int(v.per_page),
        P::PageOrientation => code(v.page_orientation.as_deref()),
        P::BottomMargin => int(v.bottom_margin),
        P::TopMargin => int(v.top_margin),
        P::LeftMargin => int(v.left_margin),
        P::RightMargin => int(v.right_margin),
        P::HeaderSize => int(v.header_size),
        P::FooterSize => int(v.footer_size),
        P::DuplexPrinting => int(v.duplex_printing),
        P::PrintAccuracy => int(v.print_accuracy),
        P::StepDirection => int(v.step_direction),
        P::SaveViewProperties => flag(v.save_view_properties),
        P::FixedBackground => flag(v.fixed_background),
        _ => None,
    }
}

fn ext_child<'a>(site: &Site<'_, 'a>, slot: &ExtSlot, prop: Property) -> Result<NodeRef<'a>> {
    info_node_at(site, &slot.path, prop)?.child(slot.idx).ok_or_else(|| {
        site.layout("unexpected-shape", prop.xml_name(), format!("info slot {:?}[{}] is not a node", slot.path, slot.idx))
    })
}

fn ext_pieces(site: &Site, prop: Property, slot: &ExtSlot) -> Result<Vec<Piece>> {
    let name = prop.xml_name();
    let want = ext_want(&site.edit.new, prop, slot)
        .ok_or_else(|| site.invalid("invalid-xml", name, format!("cannot remove {name} or its value is not supported")))?;
    Ok(match want {
        Want::Atom(n) => vec![piece(info_int_slot(site, &slot.path, slot.idx, prop)?.1, n, &[prop])],
        Want::Node(text) => vec![piece(ext_child(site, slot, prop)?.span.span, text, &[prop])],
        Want::Color(c) => vec![piece(ext_child(site, slot, prop)?.span.span, format_color_node(c.as_ref()), &[prop])],
        Want::Font(f) => vec![piece(ext_child(site, slot, prop)?.span.span, format_font_node(f.as_ref()), &[prop])],
        Want::Shortcut(s) => vec![piece(ext_child(site, slot, prop)?.span.span, format_shortcut_node(&s), &[prop])],
        Want::Picture(p) => vec![piece(ext_child(site, slot, prop)?.span.span, format_picture_node(&p), &[prop])],
        Want::Loc(new) => {
            let node = ext_child(site, slot, prop)?;
            let items = strict_localized(node)
                .ok_or_else(|| site.layout("unexpected-shape", name, format!("{name} node is not a plain localized string")))?;
            let current: LocItems = items.iter().map(|i| (i.lang.clone(), i.text.clone())).collect();
            localized_pieces(site, node, &items, &current, &new, prop)?
        }
    })
}

/// Read the slot back from the patched stream; `true` when the model does not set the property.
fn ext_matches(site: &Site, prop: Property, slot: &ExtSlot) -> Result<bool> {
    let Some(want) = ext_want(&site.edit.new, prop, slot) else { return Ok(true) };
    Ok(match want {
        Want::Atom(n) => info_int_slot(site, &slot.path, slot.idx, prop)?.0 == n,
        Want::Node(text) => {
            let expected = crate::brace::parse_metadata(text.as_bytes()).ok();
            expected.is_some() && Some(ext_child(site, slot, prop)?.node) == expected.as_ref()
        }
        Want::Color(c) => {
            let parent = info_node_at(site, &slot.path, prop)?;
            parent.node.children.get(slot.idx).is_some_and(|item| read_color_spec(item) == c)
        }
        Want::Font(f) => {
            let parent = info_node_at(site, &slot.path, prop)?;
            parent.node.children.get(slot.idx).is_some_and(|item| read_font_spec(item) == f)
        }
        Want::Shortcut(s) => {
            let parent = info_node_at(site, &slot.path, prop)?;
            parent.node.children.get(slot.idx).is_some_and(|item| read_shortcut_spec(item) == Some(s))
        }
        Want::Picture(p) => {
            let parent = info_node_at(site, &slot.path, prop)?;
            parent.node.children.get(slot.idx).is_some_and(|item| read_picture_spec(item) == Some(p))
        }
        Want::Loc(new) => {
            let node = ext_child(site, slot, prop)?;
            strict_localized(node)
                .is_some_and(|items| items.into_iter().map(|i| (i.lang, i.text)).collect::<LocItems>() == new)
        }
    })
}

/// Write the model's extended-catalog values into a donor-cloned control (only values the model sets).
fn set_clone_ext(info: &mut Node, type_name: &str, v: &ControlValues) -> Result<()> {
    for &prop in EXT_PROPS {
        let Some(slot) = ext_slot(type_name, prop, info) else { continue };
        let item = match ext_want(v, prop, &slot) {
            None | Some(Want::Color(None) | Want::Font(None)) => continue,
            Some(Want::Loc(l)) if l.is_empty() => continue,
            Some(Want::Atom(n)) => bare(n),
            Some(Want::Node(text)) => {
                Item::Node(crate::brace::parse_metadata(text.as_bytes()).map_err(|e| layout_err(e.to_string()))?)
            }
            Some(Want::Color(c)) => Item::Node(
                crate::brace::parse_metadata(format_color_node(c.as_ref()).as_bytes()).map_err(|e| layout_err(e.to_string()))?,
            ),
            Some(Want::Font(f)) => Item::Node(
                crate::brace::parse_metadata(format_font_node(f.as_ref()).as_bytes()).map_err(|e| layout_err(e.to_string()))?,
            ),
            Some(Want::Shortcut(s)) => Item::Node(
                crate::brace::parse_metadata(format_shortcut_node(&s).as_bytes()).map_err(|e| layout_err(e.to_string()))?,
            ),
            Some(Want::Picture(p)) => Item::Node(
                crate::brace::parse_metadata(format_picture_node(&p).as_bytes()).map_err(|e| layout_err(e.to_string()))?,
            ),
            Some(Want::Loc(l)) => Item::Node(localized_node(&l)),
        };
        let parent = nav_mut(info, &slot.path)?;
        *parent
            .children
            .get_mut(slot.idx)
            .ok_or_else(|| layout_err(format!("prototype: {type_name} {} slot {} missing", prop.xml_name(), slot.idx)))? = item;
    }
    Ok(())
}

/// Apply all non-geometry, non-caption `ControlValues` properties to a donor-cloned control node.
fn set_clone_props(n: &mut Node, type_name: &str, v: &ControlValues) -> Result<()> {
    if let Some(anchors) = &v.anchors {
        let geom = clone_geom_mut(n)?;
        for (idx, edge) in anchors.edges() {
            let Some(edge) = edge else { continue };
            let rec = geom
                .children
                .get_mut(idx)
                .and_then(Item::as_node_mut)
                .filter(|r| r.children.len() > 1)
                .ok_or_else(|| layout_err(format!("prototype: geometry edge record {idx} missing")))?;
            rec.children[1] = Item::Node(
                crate::brace::parse_metadata(format_edge_target(&edge).as_bytes()).map_err(|e| layout_err(e.to_string()))?,
            );
        }
    }
    if let Some(meta) = n.len().checked_sub(2).and_then(|i| n.children.get_mut(i)).and_then(Item::as_node_mut) {
        if meta.len() > 5 {
            for (idx, flag) in [(3, v.modifies_data), (4, v.skip_on_input), (5, v.first_in_group)] {
                if let Some(b) = flag {
                    meta.children[idx] = bare(i64::from(b));
                }
            }
        }
    }
    let info = clone_info_mut(n)?;
    if let Some(ap) = patch_app_path(type_name) {
        let path: Vec<usize> = ap.iter().map(|&i| i as usize).collect();
        let app = nav_mut(info, &path)?;
        if app.len() < 7 {
            return Err(layout_err("prototype: appearance block too short"));
        }
        app.children[1] = bare(i64::from(v.enabled));
        app.children[2] = Item::Node(crate::brace::parse_metadata(format_color_node(v.back_color.as_ref()).as_bytes()).map_err(|e| layout_err(e.to_string()))?);
        app.children[3] = Item::Node(crate::brace::parse_metadata(format_color_node(v.text_color.as_ref()).as_bytes()).map_err(|e| layout_err(e.to_string()))?);
        app.children[4] = Item::Node(crate::brace::parse_metadata(format_font_node(v.font.as_ref()).as_bytes()).map_err(|e| layout_err(e.to_string()))?);
        app.children[5] = bare(i64::from(v.transparent_back));
        app.children[6] = Item::Node(crate::brace::parse_metadata(format_color_node(v.border_color.as_ref()).as_bytes()).map_err(|e| layout_err(e.to_string()))?);
        if app.len() > 12 {
            app.children[12] = Item::Node(localized_node(&v.tooltip));
        }
    }
    match type_name {
        "InputField" => {
            if let Some(vt) = &v.value_type {
                if let Some(pat) = format_attr_pattern(&vt.to_attr()) {
                    info.children[1] = Item::Node(
                        crate::brace::parse_metadata(pat.as_bytes()).map_err(|e| layout_err(e.to_string()))?,
                    );
                }
            }
            let q = nav_mut(info, &[2, 0])?;
            let sb = |s: &str| match s {
                "Use" => 1,
                "Auto" => 2,
                _ => 0,
            };
            q.children[2] = bare(sb(&v.horizontal_scroll_bar));
            q.children[3] = bare(sb(&v.vertical_scroll_bar));
            if let Some(b) = v.auto_cell_height {
                q.children[4] = bare(i64::from(b));
            }
            if let Some(b) = v.drop_list_button {
                q.children[6] = bare(i64::from(b));
            }
            if let Some(b) = v.choice_button {
                q.children[7] = bare(i64::from(b));
            }
            if let Some(b) = v.clear_button {
                q.children[8] = bare(i64::from(b));
            }
            if let Some(b) = v.spin_button {
                q.children[9] = bare(i64::from(b));
            }
            if let Some(b) = v.open_button {
                q.children[10] = bare(i64::from(b));
            }
            q.children[13] = bare(i64::from(v.read_only));
            q.children[27] = bare(i64::from(v.mark_negatives));
            q.children[35] = bare(i64::from(v.auto_mark_incomplete));
            if let Some(b) = v.auto_choice_incomplete {
                q.children[36] = bare(i64::from(b));
            }
            if info.len() > 9 {
                info.children[9] = bare(i64::from(v.choice_list_mode));
            }
            set_input_extras(info, v)?;
        }
        "LabelDecoration" => {
            let p = nav_mut(info, &[1])?;
            let align = match v.horizontal_align.as_str() {
                "Center" => 1,
                "Right" => 2,
                _ => 0,
            };
            p.children[3] = bare(align);
            p.children[5] = bare(i64::from(v.hyperlink));
        }
        "CheckBox" => {
            let p = nav_mut(info, &[1])?;
            p.children[2] = bare(i64::from(v.three_state));
            let sub = nav_mut(p, &[0])?;
            sub.children[3] = bare(if v.title_location == "Left" { 0 } else { 1 });
        }
        "Panel" => {
            if let Some(code) = v.show_tabs {
                let p = nav_mut(info, &[1])?;
                let mut idx = 2usize;
                for _ in 0..6 {
                    let sz: usize = p.children[idx]
                        .as_atom()
                        .and_then(|a| a.value.parse().ok())
                        .ok_or_else(|| layout_err("prototype: panel list size"))?;
                    idx += 1 + sz;
                }
                p.children[idx + 1] = bare(code);
            }
        }
        "Table" => {
            let t = nav_mut(info, &[2, 1])?;
            let mut flags: i64 = t.children[1]
                .as_atom()
                .and_then(|a| a.value.parse().ok())
                .unwrap_or(0);
            for p in TABLE_FLAGS {
                if let Some((bit, want)) = table_flag_bit(p).zip(table_flag_value(v, p)) {
                    let set = if p == Property::ReadOnly { !want } else { want };
                    flags = if set { flags | bit } else { flags & !bit };
                }
            }
            t.children[1] = bare(flags);
            let sb = |s: &str| match s {
                "Use" => 1,
                "Auto" => 2,
                _ => 0,
            };
            if !v.horizontal_scroll_bar.is_empty() {
                t.children[9] = bare(sb(&v.horizontal_scroll_bar));
            }
            if !v.vertical_scroll_bar.is_empty() {
                t.children[10] = bare(sb(&v.vertical_scroll_bar));
            }
            if let Some(s) = &v.initial_list_view {
                t.children[20] = bare(initial_list_view_code(s));
            }
            if let Some(b) = v.change_row_set {
                t.children[22] = bare(i64::from(b));
            }
            if let Some(s) = &v.output {
                t.children[24] = bare(output_code(s));
            }
            t.children[15] = bare(if v.selection_mode == "Multi" { 1 } else { 0 });
            if t.len() > 21 {
                let itv = match v.initial_tree_view.as_str() {
                    "ExpandTopLevel" => 1,
                    "ExpandAllLevels" => 2,
                    _ => 0,
                };
                t.children[21] = bare(itv);
            }
        }
        "RadioButton" => {
            if let Some(pat) = v.value_type.as_ref().and_then(|vt| format_attr_pattern(&vt.to_attr())) {
                info.children[1] =
                    Item::Node(crate::brace::parse_metadata(pat.as_bytes()).map_err(|e| layout_err(e.to_string()))?);
            }
        }
        "ChoiceField" => {
            if !v.choice_list.is_empty() {
                let p = nav_mut(info, &[1])?;
                if p.len() > 26 {
                    p.children[26] = Item::Node(
                        crate::brace::parse_metadata(format_choice_list_node(&v.choice_list).as_bytes())
                            .map_err(|e| layout_err(e.to_string()))?,
                    );
                }
            }
        }
        "CommandBar" => {
            let x = nav_mut(info, &[1])?;
            if let Some(af) = v.auto_fill {
                x.children[3] = bare(i64::from(af));
            }
            x.children[4] = bare(if v.horizontal_align == "Right" { 2 } else { 0 });
        }
        "UsualGroup" => {
            if let Some(bs) = v.border_style {
                let b = nav_mut(info, &[1, 3])?;
                b.children[3] = bare(bs);
            }
        }
        "PictureField" | "PictureDecoration" => {
            let p = nav_mut(info, &[1])?;
            p.children[3] = bare(i64::from(v.hyperlink));
        }
        _ => {}
    }
    set_clone_ext(info, type_name, v)?;
    Ok(())
}

/// Write events into a cloned prototype: exactly one event list must exist; its entries are built
/// from the donor event template with tag and handler replaced.
fn set_clone_events(n: &mut Node, events: &[(i64, String)], who: &str) -> Result<()> {
    if events.is_empty() {
        crate::donor::neutralize_events(n);
        return Ok(());
    }
    let paths = crate::donor::event_list_paths(n);
    if paths.len() != 1 {
        return Err(layout_err(format!(
            "{who}: events on a new control of this type are not supported ({} event lists in the prototype); add it without events",
            paths.len()
        )));
    }
    crate::donor::neutralize_events(n);
    let list = crate::donor::build_event_list(events)?;
    let mut cur = n;
    for &i in &paths[0] {
        cur = match cur.children[i].as_node_mut() {
            Some(x) => x,
            None => return Err(layout_err("prototype: event list path")),
        };
    }
    *cur = list;
    Ok(())
}

/// Byte range cutting item `k` of `n` from a counted list, including one separator.
fn removal_range(list: &SpanNode, k: usize, n: usize) -> Result<Span> {
    let items: Vec<Span> = (1..=n)
        .map(|i| match list.children.get(i) {
            Some(SpanItem::Node(x)) => Ok(x.span),
            Some(SpanItem::Atom(s)) => Ok(*s),
            None => Err(layout_err("structural edit: list span does not match its count")),
        })
        .collect::<Result<Vec<_>>>()?;
    if n == 1 {
        return Ok(items[0]);
    }
    if k == 0 {
        return Ok(Span { start: items[0].start, end: items[1].start });
    }
    Ok(Span { start: items[k - 1].end, end: items[k].end })
}

/// Edits of one counted list: removals by item index, insertions at the end.
#[derive(Default)]
struct ListEdit {
    removals: Vec<usize>,
    inserts: Vec<Vec<u8>>,
}

// ------------------------------------------------------------------ 4.x lossless property patchers

/// One replacement of the original stream, attributed to a control and the supported properties it serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedReplacement {
    pub control_id: i64,
    pub properties: Vec<Property>,
    /// Byte range in the ORIGINAL stream.
    pub range: Span,
    /// Bytes that replace `range`.
    pub bytes: Vec<u8>,
}

/// Result of a patch run: the new uncompressed stream plus every authorized replacement (sorted by
/// `range.start`, pairwise disjoint). Every byte of the original outside these ranges is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchedForm {
    pub bytes: Vec<u8>,
    pub replacements: Vec<AppliedReplacement>,
    /// Page geometry of every resized panel as the decoder derives it from `bytes` (verified). Requested-vs-decoded
    /// comparisons must expect exactly these page values, which are derived, not user-edited. Empty when no panel
    /// was resized.
    pub derived_pages: Vec<DerivedPage>,
    /// `(xml id, allocated stream id)` for every added control (empty without structural edits).
    pub allocated: Vec<(i64, i64)>,
}

impl PatchedForm {
    /// Ranges of the original stream that were allowed to change.
    pub fn authorized_ranges(&self) -> Vec<Span> {
        self.replacements.iter().map(|r| r.range).collect()
    }

    /// Check that `self.bytes` is `original` with exactly the recorded replacements applied.
    pub fn verify_preserved(&self, original: &[u8]) -> Result<()> {
        verify_preserved(original, &self.bytes, &self.replacements)
    }
}

/// Check byte equality of `original` and `patched` outside the authorized `replacements` (sorted, disjoint,
/// ranges relative to `original`) and that each authorized range holds exactly its recorded bytes.
pub fn verify_preserved(original: &[u8], patched: &[u8], replacements: &[AppliedReplacement]) -> Result<()> {
    let bad = |m: String| ExtractionError::new(GENERATED_FILE_MODIFIED, m).with("kind", "unauthorized-stream-change");
    let (mut o, mut p) = (0usize, 0usize);
    for r in replacements {
        let (start, end) = (r.range.start, r.range.end);
        if start < o || end < start || end > original.len() {
            return Err(bad(format!("authorized range {start}..{end} is out of order or out of bounds")));
        }
        let keep = start - o;
        if patched.get(p..p + keep) != Some(&original[o..start]) {
            return Err(bad(format!("stream bytes changed outside the authorized ranges (before offset {start})")));
        }
        p += keep;
        if patched.get(p..p + r.bytes.len()) != Some(r.bytes.as_slice()) {
            return Err(bad(format!("authorized range {start}..{end} does not hold the recorded replacement")));
        }
        p += r.bytes.len();
        o = end;
    }
    if patched.get(p..) != Some(&original[o..]) {
        return Err(bad("stream bytes changed outside the authorized ranges (tail)".to_string()));
    }
    Ok(())
}

/// A planned replacement before it is attributed to a control.
struct Piece {
    range: Span,
    bytes: Vec<u8>,
    properties: Vec<Property>,
}

/// An edit together with the binary control it applies to.
struct Site<'e, 'a> {
    source: &'e SpannedMetadata,
    edit: &'e ControlEdit,
    entry: &'e ControlEntry<'a>,
}

impl Site<'_, '_> {
    fn error(&self, code: &'static str, kind: &'static str, property: &str, message: String) -> ExtractionError {
        ExtractionError::new(
            code,
            format!("{} '{}' (id {}), {property}: {message}", self.edit.type_name, self.edit.name, self.edit.id),
        )
        .with("kind", kind)
        .with("control", self.edit.name.as_str())
        .with("id", self.edit.id)
        .with("property", property)
    }

    /// The original stream has a shape the patcher does not know: nothing is guessed.
    fn layout(&self, kind: &'static str, property: &str, message: String) -> ExtractionError {
        self.error(METADATA_LAYOUT_UNSUPPORTED, kind, property, message)
    }

    /// The requested value (or the baseline XML) is not acceptable.
    fn invalid(&self, kind: &'static str, property: &str, message: String) -> ExtractionError {
        self.error(GENERATED_FILE_MODIFIED, kind, property, message)
    }

    fn mismatch(&self, property: &str, xml: &str, stream: &str) -> ExtractionError {
        self.invalid(
            "baseline-mismatch",
            property,
            format!("the baseline XML says {xml} but the form stream holds {stream}; re-extract the form before editing"),
        )
    }
}

impl<'a> NodeRef<'a> {
    /// Child `i` when it is a node.
    pub fn child(self, i: usize) -> Option<NodeRef<'a>> {
        match (self.node.children.get(i)?, self.span.children.get(i)?) {
            (Item::Node(node), SpanItem::Node(span)) => Some(NodeRef { node, span }),
            _ => None,
        }
    }

    /// Child `i` when it is an atom, with its source span.
    pub fn atom(self, i: usize) -> Option<(&'a Atom, Span)> {
        match (self.node.children.get(i)?, self.span.children.get(i)?) {
            (Item::Atom(a), SpanItem::Atom(s)) => Some((a, *s)),
            _ => None,
        }
    }
}

fn strict_int(s: &str) -> Option<i64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// A bare integer atom `n[i]` and its span.
fn int_slot(site: &Site, n: NodeRef, i: usize, what: &str, property: &str) -> Result<(i64, Span)> {
    let found = n.atom(i).filter(|(a, _)| a.kind == AtomKind::Bare).and_then(|(a, s)| Some((strict_int(&a.value)?, s)));
    found.ok_or_else(|| site.layout("unexpected-shape", property, format!("{what} (geometry item {i}) is not a bare integer")))
}

fn non_negative_len(site: &Site, n: NodeRef, i: usize, property: &str) -> Result<usize> {
    let (v, _) = int_slot(site, n, i, "counted incoming list size", property)?;
    usize::try_from(v).ok().filter(|v| *v <= n.node.len()).ok_or_else(|| {
        site.layout("unexpected-shape", property, format!("counted incoming list size {v} (geometry item {i}) is out of range"))
    })
}

/// Integer slots of a control's geometry node (`left, top, right, bottom, visible, ..., page, z, tab`).
struct GeomSlots {
    left: (i64, Span),
    top: (i64, Span),
    right: (i64, Span),
    bottom: (i64, Span),
    visible: (i64, Span),
    page: i64,
    z_order: (i64, Span),
    tab_order: (i64, Span),
}

/// Strictly locate every slot of the geometry node. The tail is found exactly like `oof::geom_page`: after
/// the six node slots, six counted incoming lists (`size, size items`) precede `page, z, tab, flagA, flagB`,
/// which must end the node. Any other shape is rejected.
fn read_geom(site: &Site) -> Result<GeomSlots> {
    let g = site
        .entry
        .geom()
        .ok_or_else(|| site.layout("unexpected-shape", "geometry", "the geometry node is missing".to_string()))?;
    let p = "geometry";
    if g.node.len() < 18 {
        return Err(site.layout("unexpected-shape", p, format!("geometry node has {} items, expected at least 18", g.node.len())));
    }
    let slot = |i: usize, what: &str| int_slot(site, g, i, what, p);
    let (left, top, right, bottom) = (slot(1, "left")?, slot(2, "top")?, slot(3, "right")?, slot(4, "bottom")?);
    let visible = slot(5, "visible")?;
    for i in 6..12 {
        if g.child(i).is_none() {
            return Err(site.layout("unexpected-shape", p, format!("geometry item {i} is not a node")));
        }
    }
    let mut idx = 12usize;
    for _ in 0..6 {
        let n = non_negative_len(site, g, idx, p)?;
        idx = idx.checked_add(1 + n).filter(|i| *i <= g.node.len()).ok_or_else(|| {
            site.layout("unexpected-shape", p, "a counted incoming list runs past the end of the geometry node".to_string())
        })?;
    }
    if idx.checked_add(5) != Some(g.node.len()) {
        return Err(site.layout(
            "unexpected-shape",
            p,
            format!(
                "expected exactly 5 items (page, z-order, tab-order, 2 flags) after the incoming lists, found {}",
                g.node.len().saturating_sub(idx)
            ),
        ));
    }
    let page = slot(idx, "page")?;
    let z_order = slot(idx + 1, "z-order")?;
    let tab_order = slot(idx + 2, "tab-order")?;
    slot(idx + 3, "tail flag")?;
    slot(idx + 4, "tail flag")?;
    if geom_page(g.node) != (page.0, z_order.0, tab_order.0) {
        return Err(site.layout("unexpected-shape", p, "geometry tail disagrees with the decoder's reading".to_string()));
    }
    Ok(GeomSlots { left, top, right, bottom, visible, page: page.0, z_order, tab_order })
}

fn piece(range: Span, value: impl ToString, properties: &[Property]) -> Piece {
    Piece { range, bytes: value.to_string().into_bytes(), properties: properties.to_vec() }
}

const GEOMETRY: [Property; 4] = [Property::Left, Property::Top, Property::Width, Property::Height];

fn in_i32(site: &Site, property: Property, v: i64) -> Result<i64> {
    i32::try_from(v)
        .map(i64::from)
        .map_err(|_| site.invalid("invalid-geometry", property.xml_name(), format!("{v} is outside the 32-bit integer range")))
}

fn geometry_pieces(site: &Site, g: &GeomSlots) -> Result<Vec<Piece>> {
    let (edit, changed) = (site.edit, &site.edit.changed);
    let touched: Vec<Property> = GEOMETRY.iter().copied().filter(|p| changed.contains(p)).collect();
    if touched.is_empty() {
        return Ok(Vec::new());
    }
    let overflow = |what: &str| site.invalid("invalid-geometry", "geometry", format!("{what} overflows"));
    let old_width = g.right.0.checked_sub(g.left.0).ok_or_else(|| overflow("stream width"))?;
    let old_height = g.bottom.0.checked_sub(g.top.0).ok_or_else(|| overflow("stream height"))?;
    for (property, xml, stream) in [
        (Property::Left, edit.old.left, g.left.0),
        (Property::Top, edit.old.top, g.top.0),
        (Property::Width, edit.old.width, old_width),
        (Property::Height, edit.old.height, old_height),
    ] {
        if xml != stream {
            return Err(site.mismatch(property.xml_name(), &xml.to_string(), &stream.to_string()));
        }
    }
    let new = &edit.new;
    for (property, v) in [(Property::Width, new.width), (Property::Height, new.height)] {
        if changed.contains(&property) && v < 0 {
            let name = property.xml_name();
            return Err(site.invalid("invalid-geometry", name, format!("{name} must not be negative, got {v}")));
        }
    }
    let right = new.left.checked_add(new.width).ok_or_else(|| overflow("Left + Width"))?;
    let bottom = new.top.checked_add(new.height).ok_or_else(|| overflow("Top + Height"))?;
    let (left, top) = (in_i32(site, Property::Left, new.left)?, in_i32(site, Property::Top, new.top)?);
    let (right, bottom) = (in_i32(site, Property::Width, right)?, in_i32(site, Property::Height, bottom)?);
    let on = |ps: &[Property]| -> Vec<Property> { ps.iter().copied().filter(|p| touched.contains(p)).collect() };
    let mut out = Vec::new();
    for (slot, value, props) in [
        (g.left, left, on(&[Property::Left])),
        (g.top, top, on(&[Property::Top])),
        (g.right, right, on(&[Property::Left, Property::Width])),
        (g.bottom, bottom, on(&[Property::Top, Property::Height])),
    ] {
        if slot.0 != value {
            out.push(piece(slot.1, value, &props));
        }
    }
    if is_panel_resize(edit) {
        out.extend(panel_page_pieces(site)?);
    }
    Ok(out)
}

// ---- panel pages (derived page-boundary geometry)

/// Stored client-area edge of one page: the edge coordinate (patched) and its margin. The decoder reads the
/// page client size as `coord + margin`.
struct PageEdge {
    coord: (i64, Span),
    margin: i64,
}

/// A page whose geometry the decoder derives from its right (edge 3) and bottom (edge 4) boundary records.
struct PageEdges {
    index: usize,
    right: PageEdge,
    bottom: PageEdge,
}

/// Strict reading of a `Panel` info node, laid out exactly like `oof::panel_pages`: six counted lists, then
/// `base`: `showTabs` at `base+1`, the page table at `base+3`, the boundary-row count at `base+7` and the rows
/// (`{2,coord,_,_,edge,page,_,margin,_}`) from `base+8`.
struct PanelLayout {
    show_tabs: (i64, Span),
    /// Pages with both an edge-3 and an edge-4 row; every other page is derived from the panel size alone.
    edged: Vec<PageEdges>,
}

fn panel_int(site: &Site, n: NodeRef, i: usize, what: &str) -> Result<(i64, Span)> {
    let found = n.atom(i).filter(|(a, _)| a.kind == AtomKind::Bare).and_then(|(a, s)| Some((strict_int(&a.value)?, s)));
    found.ok_or_else(|| site.layout("unexpected-shape", "panel", format!("the panel {what} (record item {i}) is not a bare integer")))
}

fn panel_count(site: &Site, n: NodeRef, i: usize, what: &str, limit: usize) -> Result<usize> {
    let (v, _) = panel_int(site, n, i, what)?;
    usize::try_from(v)
        .ok()
        .filter(|v| *v <= limit)
        .ok_or_else(|| site.layout("unexpected-shape", "panel", format!("the panel {what} {v} (record item {i}) is out of range")))
}

fn panel_info<'a>(site: &Site<'_, 'a>) -> Result<NodeRef<'a>> {
    site.entry.info().ok_or_else(|| site.layout("unexpected-shape", "panel", "the panel info node is missing".to_string()))
}

fn read_panel(site: &Site) -> Result<PanelLayout> {
    let shape = |m: &str| site.layout("unexpected-shape", "panel", m.to_string());
    let info = panel_info(site)?;
    let p = info.child(1).filter(|p| p.node.len() >= 4).ok_or_else(|| shape("the panel record (info item 1) is missing"))?;
    let mut idx = 2usize;
    for _ in 0..6 {
        let n = panel_count(site, p, idx, "counted list size", p.node.len())?;
        idx = idx
            .checked_add(1 + n)
            .filter(|i| *i <= p.node.len())
            .ok_or_else(|| shape("a counted list runs past the end of the panel record"))?;
    }
    let base = idx;
    let show_tabs = panel_int(site, p, base + 1, "ShowTabs code")?;
    let table = p.child(base + 3).filter(|t| t.node.len() >= 2).ok_or_else(|| shape("the page table is missing"))?;
    let page_count = panel_count(site, table, 1, "page count", table.node.len())?;
    if table.node.len() != page_count + 2 || (0..page_count).any(|i| table.child(2 + i).is_none_or(|r| r.node.len() < 7)) {
        return Err(shape("the page table does not hold exactly the counted page records"));
    }
    let rows = panel_count(site, p, base + 7, "boundary row count", p.node.len())?;
    if base.checked_add(8 + rows).is_none_or(|end| end > p.node.len()) {
        return Err(shape("the page boundary rows run past the end of the panel record"));
    }
    // (page, edge) -> coordinate slot and margin, for the two edges the decoder uses.
    let mut found: HashMap<(usize, i64), PageEdge> = HashMap::new();
    for r in 0..rows {
        let row = p
            .child(base + 8 + r)
            .filter(|row| row.node.len() >= 8)
            .ok_or_else(|| shape("a page boundary row is not a node with at least 8 items"))?;
        let edge = panel_int(site, row, 4, "boundary edge")?.0;
        let coord = panel_int(site, row, 1, "boundary coordinate")?;
        let margin = panel_int(site, row, 7, "boundary margin")?.0;
        let page = panel_int(site, row, 5, "boundary page index")?.0;
        let Some(page) = usize::try_from(page).ok().filter(|pg| *pg < page_count) else { continue };
        if (edge == 3 || edge == 4) && found.insert((page, edge), PageEdge { coord, margin }).is_some() {
            return Err(shape(&format!("page {page} has more than one boundary row for edge {edge}; the page size would be ambiguous")));
        }
    }
    let mut edged = Vec::new();
    for index in 0..page_count {
        if let (Some(right), Some(bottom)) = (found.remove(&(index, 3)), found.remove(&(index, 4))) {
            edged.push(PageEdges { index, right, bottom });
        }
    }
    // The strict reading must agree with the decoder's.
    let decoded = panel_pages(info.node, site.edit.old.width, site.edit.old.height);
    if decoded.0 != show_tabs.0 || decoded.1.len() != page_count {
        return Err(shape("the panel layout disagrees with the decoder's reading"));
    }
    Ok(PanelLayout { show_tabs, edged })
}

/// Records the decoder needs to see after a panel resize. A page with edge records keeps its position and
/// changes its client size by exactly the panel's size change (the margins around the page area stay), which is
/// what the decoder derives from `coord + margin`; the other pages follow from the panel size alone.
fn panel_page_pieces(site: &Site) -> Result<Vec<Piece>> {
    let (old, new) = (&site.edit.old, &site.edit.new);
    let layout = read_panel(site)?;
    let titles = panel_pages(panel_info(site)?.node, old.width, old.height).1;
    let overflow = |what: &str| site.invalid("invalid-geometry", "geometry", format!("{what} overflows"));
    let mut out = Vec::new();
    for page in &layout.edged {
        let name = titles[page.index]["name"].as_str().unwrap_or_default();
        for (edge, property, old_extent, new_extent) in [
            (&page.right, Property::Width, old.width, new.width),
            (&page.bottom, Property::Height, old.height, new.height),
        ] {
            let delta = new_extent.checked_sub(old_extent).ok_or_else(|| overflow("the size change"))?;
            if delta == 0 {
                continue;
            }
            let area = edge.coord.0.checked_add(edge.margin).ok_or_else(|| overflow("the page area"))?;
            if area < 1 || area > old_extent {
                return Err(site.layout(
                    "unexpected-shape",
                    "panel",
                    format!(
                        "the client area {area} of page '{name}' is clamped by the panel size {old_extent}; its geometry cannot be derived exactly"
                    ),
                ));
            }
            let new_area = area.checked_add(delta).ok_or_else(|| overflow("the page area"))?;
            if new_area < 1 {
                let label = property.xml_name();
                return Err(site.invalid(
                    "invalid-geometry",
                    label,
                    format!("{label} {new_extent} leaves page '{name}' no client area; the minimum is {}", old_extent - area + 1),
                ));
            }
            let coord = in_i32(site, property, new_area - edge.margin)?;
            out.push(piece(edge.coord.1, coord, &[property]));
        }
    }
    Ok(out)
}

/// One page of a resized panel as the normal decoder derives it from the patched stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedPage {
    pub panel_id: i64,
    /// Position in the panel's page table.
    pub index: usize,
    pub name: String,
    pub left: i64,
    pub top: i64,
    pub width: i64,
    pub height: i64,
}

/// `(showTabs, pages)` the decoder must produce for a resized panel: pages with edge records keep
/// `Left`/`Top`/title/visibility and move `Width`/`Height` by the panel's size change; the others are derived
/// from the new panel size. Computed from the ORIGINAL stream.
fn expected_panel(site: &Site) -> Result<(i64, Vec<Value>)> {
    let (old, new) = (&site.edit.old, &site.edit.new);
    let layout = read_panel(site)?;
    let info = panel_info(site)?;
    let (show_tabs, before) = panel_pages(info.node, old.width, old.height);
    let (_, mut pages) = panel_pages(info.node, new.width, new.height);
    for page in &layout.edged {
        let mut kept = before[page.index].clone();
        kept["width"] = (before[page.index]["width"].as_i64().unwrap_or_default() + (new.width - old.width)).into();
        kept["height"] = (before[page.index]["height"].as_i64().unwrap_or_default() + (new.height - old.height)).into();
        pages[page.index] = kept;
    }
    Ok((show_tabs, pages))
}

fn derived_pages(panel_id: i64, pages: &[Value]) -> Vec<DerivedPage> {
    let int_of = |p: &Value, k: &str| p[k].as_i64().unwrap_or_default();
    pages
        .iter()
        .enumerate()
        .map(|(index, p)| DerivedPage {
            panel_id,
            index,
            name: p["name"].as_str().unwrap_or_default().to_string(),
            left: int_of(p, "left"),
            top: int_of(p, "top"),
            width: int_of(p, "width"),
            height: int_of(p, "height"),
        })
        .collect()
}

fn is_panel_resize(edit: &ControlEdit) -> bool {
    edit.type_name == "Panel" && (edit.changed.contains(&Property::Width) || edit.changed.contains(&Property::Height))
}

fn visible_pieces(site: &Site, g: &GeomSlots) -> Result<Vec<Piece>> {
    if !site.edit.changed.contains(&Property::Visible) {
        return Ok(Vec::new());
    }
    let current = g.visible.0 != 0;
    if current != site.edit.old.visible {
        return Err(site.mismatch("Visible", &site.edit.old.visible.to_string(), &current.to_string()));
    }
    Ok(vec![piece(g.visible.1, i32::from(site.edit.new.visible), &[Property::Visible])])
}

fn order_pieces(site: &Site, g: &GeomSlots) -> Result<Vec<Piece>> {
    let (edit, mut out) = (site.edit, Vec::new());
    let checked = |property: Property, v: i64| -> Result<i64> {
        if !(0..=i64::from(i32::MAX)).contains(&v) {
            let name = property.xml_name();
            return Err(site.invalid("invalid-order", name, format!("{name} must be between 0 and {}, got {v}", i32::MAX)));
        }
        Ok(v)
    };
    if edit.changed.contains(&Property::ZOrder) {
        if g.z_order.0 != edit.old.z_order {
            return Err(site.mismatch("ZOrder", &edit.old.z_order.to_string(), &g.z_order.0.to_string()));
        }
        out.push(piece(g.z_order.1, checked(Property::ZOrder, edit.new.z_order)?, &[Property::ZOrder]));
    }
    if edit.changed.contains(&Property::TabOrder) {
        // The projection shows TabOrder only when positive.
        if g.tab_order.0.max(0) != edit.old.tab_order {
            return Err(site.mismatch("TabOrder", &edit.old.tab_order.to_string(), &g.tab_order.0.to_string()));
        }
        out.push(piece(g.tab_order.1, checked(Property::TabOrder, edit.new.tab_order)?, &[Property::TabOrder]));
    }
    Ok(out)
}

// ---- captions

struct LocItem {
    lang: String,
    text: String,
    item: Span,
    text_span: Span,
}

/// `{1,N,{"lang","text"},...}` with exactly N items and nothing else.
fn strict_localized(n: NodeRef) -> Option<Vec<LocItem>> {
    let bare = |i: usize| n.atom(i).filter(|(a, _)| a.kind == AtomKind::Bare).map(|(a, _)| a.value.as_str());
    if bare(0)? != "1" {
        return None;
    }
    let count_text = bare(1)?;
    if count_text.is_empty() || !count_text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let count: usize = count_text.parse().ok()?;
    if n.node.len() != count.checked_add(2)? {
        return None;
    }
    let mut items = Vec::with_capacity(count);
    for k in 0..count {
        let item = n.child(2 + k)?;
        if item.node.len() != 2 {
            return None;
        }
        let ((l, _), (t, text_span)) = (item.atom(0)?, item.atom(1)?);
        if l.kind != AtomKind::Quoted || t.kind != AtomKind::Quoted {
            return None;
        }
        items.push(LocItem { lang: l.value.clone(), text: t.value.clone(), item: item.span.span, text_span });
    }
    Some(items)
}

struct CaptionSlot<'a> {
    node: NodeRef<'a>,
    items: Vec<LocItem>,
}

/// The localized node holding the control's caption, found exactly where the decoder reads it. `None`
/// means the decoder sees an empty caption and the stream has no node to hold one (fallback types only).
fn caption_slot<'a>(site: &Site<'_, 'a>) -> Result<Option<CaptionSlot<'a>>> {
    let unmapped = |m: String| site.layout("caption-unmappable", "Caption", m);
    let info = site.entry.info().ok_or_else(|| unmapped("the control info node is missing".to_string()))?;
    let (node, items) = if let Some(path) = caption_path(Some(site.entry.type_name)) {
        let mut cur = info;
        for &i in path {
            cur = cur
                .child(i as usize)
                .ok_or_else(|| unmapped(format!("the caption location info{path:?} does not exist in the stream")))?;
        }
        let items =
            strict_localized(cur).ok_or_else(|| unmapped(format!("the node at info{path:?} is not a plain localized string")))?;
        (cur, items)
    } else {
        // Types without a known location: the decoder takes the first non-empty localized string among the
        // properties. Only an unambiguous single candidate is patched.
        let props = info.child(1);
        let candidates: Vec<usize> = props
            .map(|p| {
                p.node
                    .children
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| localized(c).is_some_and(|l| !l.is_empty()))
                    .map(|(k, _)| k)
                    .collect()
            })
            .unwrap_or_default();
        let k = match candidates.as_slice() {
            [] => return Ok(None),
            [k] => *k,
            many => {
                return Err(unmapped(format!(
                    "{} non-empty localized strings (info[1] items {many:?}) could be the caption; the node cannot be chosen safely",
                    many.len()
                )))
            }
        };
        let cur = props
            .and_then(|p| p.child(k))
            .ok_or_else(|| unmapped("the caption candidate is not a node".to_string()))?;
        let items = strict_localized(cur)
            .ok_or_else(|| unmapped(format!("the caption candidate info[1][{k}] is not a plain localized string")))?;
        (cur, items)
    };
    Ok(Some(CaptionSlot { node, items }))
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn localized_pieces(
    site: &Site,
    node: NodeRef,
    items: &[LocItem],
    old: &[(String, String)],
    new: &[(String, String)],
    prop: Property,
) -> Result<Vec<Piece>> {
    let name = prop.xml_name();
    if new.iter().any(|(lang, _)| lang.is_empty()) {
        return Err(site.invalid("invalid-caption", name, format!("a {name} <Item> has an empty language")));
    }
    let stream: Vec<(String, String)> = items.iter().map(|i| (i.lang.clone(), i.text.clone())).collect();
    if stream != old {
        return Err(site.mismatch(name, &format!("{old:?}"), &format!("{stream:?}")));
    }
    let props = [prop];
    if items.len() == new.len() && items.iter().zip(new).all(|(i, (l, _))| i.lang == *l) {
        return Ok(items
            .iter()
            .zip(new)
            .filter(|(i, (_, t))| i.text != *t)
            .map(|(i, (_, t))| piece(i.text_span, quote(t), &props))
            .collect());
    }
    let own = site.source.slice(node.span.span).unwrap_or_default();
    let reference = if items.is_empty() { site.source.source() } else { own };
    let nl = if contains(reference, b"\r\n") {
        "\r\n"
    } else if contains(reference, b"\n") {
        "\n"
    } else {
        ""
    };
    let mut text = format!("{{1,{}", new.len());
    for (lang, t) in new {
        let kept = items
            .iter()
            .find(|i| i.lang == *lang && i.text == *t)
            .and_then(|i| site.source.slice(i.item))
            .and_then(|b| std::str::from_utf8(b).ok());
        text.push(',');
        text.push_str(nl);
        match kept {
            Some(original) => text.push_str(original),
            None => text.push_str(&format!("{{{},{}}}", quote(lang), quote(t))),
        }
    }
    if !new.is_empty() {
        text.push_str(nl);
    }
    text.push('}');
    Ok(vec![piece(node.span.span, text, &props)])
}

fn caption_pieces(site: &Site) -> Result<Vec<Piece>> {
    let (old, new) = (&site.edit.old.caption, &site.edit.new.caption);
    if new.iter().any(|(lang, _)| lang.is_empty()) {
        return Err(site.invalid("invalid-caption", "Caption", "a Caption <Item> has an empty language".to_string()));
    }
    let slot = caption_slot(site)?.ok_or_else(|| {
        site.layout(
            "caption-unmappable",
            "Caption",
            "the stream has no caption node for this control type, so a caption cannot be added or changed safely".to_string(),
        )
    })?;
    localized_pieces(site, slot.node, &slot.items, old, new, Property::Caption)
}

fn info_node_at<'a>(site: &Site<'_, 'a>, path: &[usize], prop: Property) -> Result<NodeRef<'a>> {
    let mut cur = site
        .entry
        .info()
        .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), "the control info node is missing".into()))?;
    for &i in path {
        cur = cur
            .child(i)
            .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), format!("info path {path:?} does not exist")))?;
    }
    Ok(cur)
}

fn info_int_slot(site: &Site, path: &[usize], idx: usize, prop: Property) -> Result<(i64, Span)> {
    let n = info_node_at(site, path, prop)?;
    int_slot(site, n, idx, prop.xml_name(), prop.xml_name())
}

fn meta_bool_piece(site: &Site, idx: usize, new: bool, prop: Property) -> Result<Piece> {
    let meta = site
        .entry
        .meta()
        .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), "control meta node is missing".into()))?;
    let (_, span) = int_slot(site, meta, idx, prop.xml_name(), prop.xml_name())?;
    Ok(piece(span, i32::from(new), &[prop]))
}

fn meta_flag(site: &Site, idx: usize, prop: Property) -> Result<bool> {
    let meta = site
        .entry
        .meta()
        .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), "control meta node is missing".into()))?;
    Ok(int_slot(site, meta, idx, prop.xml_name(), prop.xml_name())?.0 != 0)
}

/// Mirror piece in the string extension body (`info[3]`), when the extension exists.
fn ext_piece(site: &Site, idx: usize, value: impl ToString, prop: Property) -> Result<Option<Piece>> {
    let info = info_node_at(site, &[], prop)?;
    let Some(body) = string_ext_body(info) else { return Ok(None) };
    let span = body
        .span
        .children
        .get(idx)
        .map(SpanItem::span)
        .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), format!("string extension slot {idx} is missing")))?;
    Ok(Some(piece(span, value, &[prop])))
}

/// Value of the string extension integer slot `idx`, `None` when there is no extension.
fn ext_int(site: &Site, idx: usize, prop: Property) -> Result<Option<i64>> {
    let info = info_node_at(site, &[], prop)?;
    match string_ext_body(info) {
        None => Ok(None),
        Some(body) => Ok(Some(int_slot(site, body, idx, prop.xml_name(), prop.xml_name())?.0)),
    }
}

fn info_atom_span(site: &Site, path: &[usize], idx: usize, prop: Property) -> Result<(String, Span)> {
    let n = info_node_at(site, path, prop)?;
    n.atom(idx)
        .map(|(a, s)| (a.value.clone(), s))
        .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), format!("info slot {idx} is not an atom")))
}

const TABLE_FLAGS: [Property; 12] = [
    Property::UseAlternationRowColor,
    Property::EnableStartDrag,
    Property::Header,
    Property::Footer,
    Property::VerticalLines,
    Property::HorizontalLines,
    Property::ReadOnly,
    Property::AutoInsertNewRow,
    Property::ChangeSettingOfColumns,
    Property::ChangePositionOfColumns,
    Property::EnableDrag,
    Property::ChoiceMode,
];

/// Bit of the coalesced `Table` flag word `info[2][1][1]`; `ReadOnly` is stored inverted.
fn table_flag_bit(p: Property) -> Option<i64> {
    Some(match p {
        Property::Header => 1,
        Property::Footer => 2,
        Property::VerticalLines => 4,
        Property::HorizontalLines => 8,
        Property::UseAlternationRowColor => 1 << 4,
        Property::EnableStartDrag => 1 << 14,
        Property::EnableDrag => 1 << 20,
        Property::ChoiceMode => 1 << 9,
        Property::ReadOnly => 1 << 10,
        Property::AutoInsertNewRow => 1 << 15,
        Property::ChangeSettingOfColumns => 1 << 25,
        Property::ChangePositionOfColumns => 1 << 26,
        _ => return None,
    })
}

fn is_table_flag(p: Property) -> bool {
    table_flag_bit(p).is_some()
}

/// Wanted value of a flag-word property (`None` when the model does not set it).
fn table_flag_value(v: &ControlValues, p: Property) -> Option<bool> {
    match p {
        Property::Header => v.header,
        Property::Footer => v.footer,
        Property::VerticalLines => v.vertical_lines,
        Property::HorizontalLines => v.horizontal_lines,
        Property::UseAlternationRowColor => v.use_alternation_row_color,
        Property::EnableStartDrag => v.enable_start_drag,
        Property::EnableDrag => v.enable_drag,
        Property::ChoiceMode => v.choice_mode,
        Property::ReadOnly => v.table_read_only,
        Property::AutoInsertNewRow => v.auto_insert_new_row,
        Property::ChangeSettingOfColumns => v.change_setting_of_columns,
        Property::ChangePositionOfColumns => v.change_position_of_columns,
        _ => None,
    }
}

fn table_flag_read(flags: i64, p: Property) -> bool {
    let set = table_flag_bit(p).is_some_and(|b| flags & b != 0);
    if p == Property::ReadOnly { !set } else { set }
}

fn initial_list_view_code(s: &str) -> i64 {
    match s {
        "Begin" => 2,
        "End" => 1,
        _ => 4_294_967_295,
    }
}

fn output_code(s: &str) -> i64 {
    match s {
        "Enable" => 1,
        "Disable" => 2,
        _ => 0,
    }
}

/// All flag-word properties share one atom: emit a single piece (for the first changed one) covering all.
fn table_flag_piece(site: &Site, p: Property, out: &mut Vec<Piece>) -> Result<()> {
    let (old, new) = (&site.edit.old, &site.edit.new);
    if site.edit.changed.iter().copied().find(|&c| is_table_flag(c)) != Some(p) {
        return Ok(());
    }
    let touched: Vec<Property> = TABLE_FLAGS.iter().copied().filter(|c| site.edit.changed.contains(c)).collect();
    let (flags, span) = info_int_slot(site, &[2, 1], 1, p)?;
    let mut want = flags;
    for &t in &touched {
        let bit = table_flag_bit(t).unwrap_or(0);
        let on = table_flag_value(new, t)
            .ok_or_else(|| site.invalid("invalid-xml", t.xml_name(), format!("cannot remove {}", t.xml_name())))?;
        if t == Property::Footer {
            let cur = table_flag_read(flags, t);
            if Some(cur) != old.footer {
                return Err(site.mismatch("Footer", &format!("{:?}", old.footer), &cur.to_string()));
            }
        }
        let set = if t == Property::ReadOnly { !on } else { on };
        want = if set { want | bit } else { want & !bit };
    }
    out.push(piece(span, want, &touched));
    Ok(())
}

fn info_bool_piece(site: &Site, path: &[usize], idx: usize, old: Option<bool>, new: bool, prop: Property) -> Result<Piece> {
    let (cur, span) = info_int_slot(site, path, idx, prop)?;
    if let Some(expected) = old {
        if (cur != 0) != expected {
            return Err(site.mismatch(prop.xml_name(), &expected.to_string(), &(cur != 0).to_string()));
        }
    }
    Ok(piece(span, i32::from(new), &[prop]))
}

fn read_control_events(entry: &ControlEntry) -> Result<Vec<(i64, String)>> {
    let paths = crate::donor::event_list_paths(entry.control.node);
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    if paths.len() > 1 {
        return Err(layout_err(format!("control '{}' (id {}): multiple event lists", entry.name, entry.id)));
    }
    let mut cur = entry.control.node;
    for &i in &paths[0] {
        cur = cur.children.get(i).and_then(Item::as_node).ok_or_else(|| layout_err("event list path missing"))?;
    }
    let mut out = Vec::new();
    for item in cur.children.iter().skip(1) {
        let Item::Node(ev) = item else { return Err(layout_err("event item is not a node")) };
        let tag = int(&ev.children[0], "event tag").map_err(|e| layout_err(e.to_string()))?;
        let Some(Item::Node(body)) = ev.children.get(2) else { return Err(layout_err("event body missing")) };
        let handler = str_val(&body.children[1], "event handler").map_err(|e| layout_err(e.to_string()))?.to_string();
        out.push((tag, handler));
    }
    Ok(out)
}

fn panel_page_records<'a>(site: &Site<'_, 'a>) -> Result<Vec<NodeRef<'a>>> {
    let shape = |m: &str| site.layout("unexpected-shape", "Pages", m.to_string());
    let info = panel_info(site)?;
    let p = info.child(1).filter(|p| p.node.len() >= 4).ok_or_else(|| shape("the panel record (info item 1) is missing"))?;
    let mut idx = 2usize;
    for _ in 0..6 {
        let n = panel_count(site, p, idx, "counted list size", p.node.len())?;
        idx = idx
            .checked_add(1 + n)
            .filter(|i| *i <= p.node.len())
            .ok_or_else(|| shape("a counted list runs past the end of the panel record"))?;
    }
    let table = p.child(idx + 3).filter(|t| t.node.len() >= 2).ok_or_else(|| shape("the page table is missing"))?;
    let page_count = panel_count(site, table, 1, "page count", table.node.len())?;
    let mut out = Vec::with_capacity(page_count);
    for i in 0..page_count {
        let rec = table.child(2 + i).filter(|r| r.node.len() >= 7).ok_or_else(|| shape("page record missing or too short"))?;
        out.push(rec);
    }
    Ok(out)
}

fn read_panel_page_values(site: &Site) -> Result<Vec<PageValues>> {
    let recs = panel_page_records(site)?;
    let mut out = Vec::with_capacity(recs.len());
    for rec in recs {
        let name = rec
            .atom(6)
            .map(|(a, _)| a.value.clone())
            .ok_or_else(|| site.layout("unexpected-shape", "Pages", "page name atom missing".into()))?;
        let title_node = rec
            .child(1)
            .ok_or_else(|| site.layout("unexpected-shape", "Pages", "page title node missing".into()))?;
        let title = strict_localized(title_node)
            .ok_or_else(|| site.layout("unexpected-shape", "Pages", "page title not localized".into()))?
            .into_iter()
            .map(|i| (i.lang, i.text))
            .collect();
        let visible = int_slot(site, rec, 4, "Pages", "Visible")?.0 != 0;
        let enabled = int_slot(site, rec, 5, "Pages", "Enabled")?.0 != 0;
        out.push(PageValues { name, title, visible, enabled, title_picture: None });
    }
    Ok(out)
}

/// The pages without the write-only `TitlePicture`, as [`read_panel_page_values`] reports them.
fn pages_without_pictures(pages: &[PageValues]) -> Vec<PageValues> {
    pages.iter().cloned().map(|p| PageValues { title_picture: None, ..p }).collect()
}

fn table_column_props<'a>(site: &Site<'_, 'a>) -> Result<Vec<(NodeRef<'a>, NodeRef<'a>)>> {
    fn walk<'a>(site: &Site<'_, 'a>, nr: NodeRef<'a>, out: &mut Vec<(NodeRef<'a>, NodeRef<'a>)>) -> Result<()> {
        if !nr.node.is_empty() && crate::oof::is_column_guid(&nr.node.children[0]) {
            let col_guid = nr.atom(0).map(|(a, _)| a.value.as_str()).unwrap_or("");
            let wrapper = nr
                .child(1)
                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column wrapper missing".into()))?;
            let props = if col_guid == crate::oof::VL_COLUMN_GUID {
                wrapper
                    .child(1)
                    .ok_or_else(|| site.layout("unexpected-shape", "Columns", "VL column props missing".into()))?
            } else {
                wrapper
                    .child(1)
                    .and_then(|x| x.child(1))
                    .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column props missing".into()))?
            };
            if props.node.len() < 39 {
                return Err(site.layout("unexpected-shape", "Columns", "column props too short".into()));
            }
            out.push((wrapper, props));
            return Ok(());
        }
        for i in 0..nr.node.len() {
            if let Some(ch) = nr.child(i) {
                walk(site, ch, out)?;
            }
        }
        Ok(())
    }
    let info = site
        .entry
        .info()
        .ok_or_else(|| site.layout("unexpected-shape", "Columns", "table info missing".into()))?;
    let mut out = Vec::new();
    walk(site, info, &mut out)?;
    Ok(out)
}

fn read_column_editor_events(cp: NodeRef) -> Vec<(i64, String)> {
    use std::io::Read;
    let Some(bnode) = cp.child(39) else { return Vec::new() };
    let Some((b0, _)) = bnode.atom(0) else { return Vec::new() };
    if b0.kind != crate::brace::AtomKind::Base64 {
        return Vec::new();
    }
    let Ok(raw) = b0.decode_base64() else { return Vec::new() };
    if raw.len() <= 18 {
        return Vec::new();
    }
    let mut inflated = Vec::new();
    if flate2::read::DeflateDecoder::new(&raw[18..]).read_to_end(&mut inflated).is_err() || inflated.len() <= 8 {
        return Vec::new();
    }
    let Ok(tree) = crate::brace::parse_metadata(&inflated[8..]) else { return Vec::new() };
    let paths = crate::donor::event_list_paths(&tree);
    if paths.len() != 1 {
        return Vec::new();
    }
    let mut cur = &tree;
    for &i in &paths[0] {
        let Some(n) = cur.children.get(i).and_then(Item::as_node) else { return Vec::new() };
        cur = n;
    }
    let mut out = Vec::new();
    for item in cur.children.iter().skip(1) {
        let Item::Node(ev) = item else { continue };
        let Ok(tag) = int(&ev.children[0], "tag") else { continue };
        let Some(Item::Node(body)) = ev.children.get(2) else { continue };
        let Ok(h) = str_val(&body.children[1], "handler") else { continue };
        out.push((tag, h.to_string()));
    }
    out
}

fn patch_column_editor_events(site: &Site, cp: NodeRef, ed_type: &str, new_events: &[(i64, String)], prop: Property) -> Result<Piece> {
    use std::io::{Read, Write};
    let bnode = cp.child(39).ok_or_else(|| site.layout("unexpected-shape", "Columns", "column editor blob missing".into()))?;
    let (b0, span) = bnode.atom(0).ok_or_else(|| site.layout("unexpected-shape", "Columns", "column editor base64 missing".into()))?;
    let raw = b0.decode_base64().map_err(|_| site.layout("unexpected-shape", "Columns", "column editor base64 invalid".into()))?;
    if raw.len() <= 18 {
        return Err(site.layout("unexpected-shape", "Columns", "column editor raw too short".into()));
    }
    let mut inflated = Vec::new();
    flate2::read::DeflateDecoder::new(&raw[18..])
        .read_to_end(&mut inflated)
        .map_err(|_| site.layout("unexpected-shape", "Columns", "column editor deflate invalid".into()))?;
    if inflated.len() <= 8 {
        return Err(site.layout("unexpected-shape", "Columns", "column editor inflated too short".into()));
    }
    let inner_src = parse_spanned(&inflated[8..]).map_err(|e| site.layout("unexpected-shape", "Columns", e.to_string()))?;
    let path = crate::donor::control_event_list_path(&inner_src.root, ed_type, "column editor")?;
    let mut cur_span = &inner_src.spans;
    for &i in &path {
        cur_span = match cur_span.children.get(i) {
            Some(SpanItem::Node(n)) => n,
            _ => return Err(site.layout("unexpected-shape", "Columns", "column editor event span missing".into())),
        };
    }
    let list_node = crate::donor::build_event_list(new_events)?;
    let mut list_bytes = Vec::new();
    crate::donor::write_node(&list_node, &mut list_bytes);
    let patched_tree = inner_src.apply(&[Replacement { range: cur_span.span, bytes: list_bytes }])?;
    let mut payload = Vec::with_capacity(8 + patched_tree.len());
    payload.extend_from_slice(&inflated[..8]);
    payload.extend_from_slice(&patched_tree);
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&payload).map_err(|e| site.layout("unexpected-shape", "Columns", e.to_string()))?;
    let deflated = enc.finish().map_err(|e| site.layout("unexpected-shape", "Columns", e.to_string()))?;
    let mut full = Vec::with_capacity(18 + deflated.len());
    full.extend_from_slice(&raw[..18]);
    full.extend_from_slice(&deflated);
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&full);
    Ok(piece(span, format!("{{#base64:{b64}}}"), &[prop]))
}

/// `checked` / `picture_id` of the items (`info[1][26][2][6]`, items from child 2) wherever the model sets them.
fn choice_list_extras_match(info: &Node, items: &[ChoiceItemSpec]) -> bool {
    let Some(list) = info
        .node(1)
        .and_then(|p| p.node(26))
        .and_then(|c| c.node(2))
        .and_then(|c| c.node(6))
    else {
        return items.iter().all(|it| it.checked.is_none() && it.picture_id.is_none());
    };
    let atom = |it: &Node, i: usize| it.children.get(i)?.as_atom()?.value.trim().parse::<i64>().ok();
    items.iter().enumerate().all(|(n, want)| {
        let Some(rec) = list.node(2 + n) else { return false };
        want.picture_id.is_none_or(|v| atom(rec, 1) == Some(v)) && want.checked.is_none_or(|v| atom(rec, 2) == Some(i64::from(v)))
    })
}

fn read_choice_list_values(info: &Node) -> Vec<ChoiceItemSpec> {
    let mut props = serde_json::Map::new();
    let _ = crate::oof_props::decode_props("ChoiceField", info, &mut |_| {});
    props.extend(crate::oof_props::decode_props("ChoiceField", info, &mut |_| {}));
    let Some(Value::Array(arr)) = props.get("choiceList") else { return Vec::new() };
    let mut out = Vec::new();
    for item in arr {
        let val = item.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut pres = Vec::new();
        if let Some(Value::Array(pa)) = item.get("presentation") {
            for p in pa {
                if let Some(pair) = p.as_array() {
                    if pair.len() >= 2 {
                        let l = pair[0].as_str().unwrap_or("").to_string();
                        let t = pair[1].as_str().unwrap_or("").to_string();
                        pres.push((l, t));
                    }
                }
            }
        }
        out.push(ChoiceItemSpec { value: val, presentation: pres, checked: None, picture_id: None });
    }
    out
}

fn read_input_value_type(info: &Node) -> Option<ValueTypeSpec> {
    let pat_item = info.children.get(1)?;
    let vt = crate::oof_props::decode_pattern(pat_item).ok()??;
    let kind = vt.get("kind")?.as_str()?.to_string();
    let composite = vt.get("composite")?.as_bool()?;
    let fraction_digits = vt.get("fractionDigits").and_then(|v| v.as_i64());
    let digits = vt.get("digits").and_then(|v| v.as_i64());
    let length = vt.get("length").and_then(|v| v.as_i64());
    let flag = |key: &str| vt.get(key) == Some(&Value::Bool(true));
    let non_negative = flag("nonNegative");
    let fixed = flag("fixed");
    let lossy = flag("mixedComposite");
    let ref_guids = match (vt.get("refGuids").and_then(|v| v.as_array()), vt.get("refGuid").and_then(|v| v.as_str())) {
        (Some(arr), _) => arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        (None, Some(rg)) => rg.split(';').map(str::to_string).collect(),
        (None, None) => Vec::new(),
    };
    Some(ValueTypeSpec { kind, composite, fraction_digits, digits, length, non_negative, fixed, ref_guids, lossy })
}

fn read_cb_buttons(info: &Node) -> Vec<CommandBarButton> {
    let props = crate::oof_props::decode_props("CommandBar", info, &mut |_| {});
    let Some(Value::Array(arr)) = props.get("buttons") else { return Vec::new() };
    fn conv(arr: &[Value]) -> Vec<CommandBarButton> {
        let mut out = Vec::new();
        for b in arr {
            let name = b.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let id = b.get("id").and_then(|v| v.as_i64());
            let kind = b.get("kind").and_then(|v| v.as_str()).unwrap_or("Action").to_string();
            let checked = b.get("checked").and_then(|v| v.as_bool()).unwrap_or(false);
            let representation = b.get("representation").and_then(|v| v.as_str()).unwrap_or("Auto").to_string();
            let has_picture = b.get("hasPicture").and_then(|v| v.as_bool()).unwrap_or(false);
            let action_kind = b.get("actionKind").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let action = b.get("action").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let loc = |key: &str| -> Vec<(String, String)> {
                let mut res = Vec::new();
                if let Some(Value::Array(pa)) = b.get(key) {
                    for p in pa {
                        if let Some(pair) = p.as_array() {
                            if pair.len() >= 2 {
                                res.push((
                                    pair[0].as_str().unwrap_or("").to_string(),
                                    pair[1].as_str().unwrap_or("").to_string(),
                                ));
                            }
                        }
                    }
                }
                res
            };
            let sub = match b.get("buttons") {
                Some(Value::Array(sa)) => conv(sa),
                _ => Vec::new(),
            };
            out.push(CommandBarButton {
                name,
                id,
                kind,
                checked,
                representation,
                has_picture,
                action_kind,
                action,
                caption: loc("caption"),
                tooltip: loc("tooltip"),
                buttons: sub,
                picture: None,
                shortcut: None,
                description: Vec::new(),
                default_button: None,
                button_order: None,
            });
        }
        out
    }
    conv(arr)
}

/// The buttons without the write-only `Picture` / `Shortcut`, as [`read_cb_buttons`] reports them.
fn buttons_without_extras(btns: &[CommandBarButton]) -> Vec<CommandBarButton> {
    btns.iter()
        .cloned()
        .map(|b| CommandBarButton {
            picture: None,
            shortcut: None,
            description: Vec::new(),
            default_button: None,
            button_order: None,
            buttons: buttons_without_extras(&b.buttons),
            ..b
        })
        .collect()
}

/// `(action table, action count, root button group)` of a command bar.
fn cb_table<'a>(site: &Site<'_, 'a>, prop: Property) -> Result<(NodeRef<'a>, usize, NodeRef<'a>)> {
    let x = info_node_at(site, &[1], prop)?;
    let t = x.child(7).ok_or_else(|| site.layout("unexpected-shape", "Buttons", "command bar table missing".into()))?;
    let action_count: usize = t
        .atom(4)
        .and_then(|(a, _)| a.value.parse().ok())
        .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "action count missing".into()))?;
    let group_count: usize = t
        .atom(5 + action_count)
        .and_then(|(a, _)| a.value.parse().ok())
        .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "group count missing".into()))?;
    let root_g = t
        .child(6 + action_count + group_count.saturating_sub(1))
        .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "root button group missing".into()))?;
    Ok((t, action_count, root_g))
}

/// The `action[4][2]` record that holds the `Picture` (`[5]`) and `Shortcut` (`[6]`) of the action `guid`.
fn cb_action_meta<'a>(t: NodeRef<'a>, action_count: usize, guid: &str) -> Option<NodeRef<'a>> {
    (0..action_count)
        .filter_map(|ai| t.child(5 + ai))
        .find(|a| a.atom(1).map(|(a, _)| a.value.as_str()) == Some(guid))
        .and_then(|a| a.child(4))
        .and_then(|b| b.child(2))
}

/// Every root button that sets a `Picture` / `Shortcut` / `Description` / `DefaultButton` / `ButtonOrder` holds it in the patched stream.
fn cb_extras_match(site: &Site, new_btns: &[CommandBarButton]) -> Result<bool> {
    if new_btns.iter().all(|b| {
        b.picture.is_none()
            && b.shortcut.is_none()
            && b.description.is_empty()
            && b.default_button.is_none()
            && b.button_order.is_none()
    }) {
        return Ok(true);
    }
    let (t, action_count, root_g) = cb_table(site, Property::Buttons)?;
    for (i, nb) in new_btns.iter().enumerate() {
        let Some(bnode) = root_g.child(6 + 2 * i) else { return Ok(false) };
        if let Some(want) = nb.default_button {
            if int_slot(site, bnode, 13, "Buttons", "DefaultButton")?.0 != i64::from(want) {
                return Ok(false);
            }
        }
        if let Some(want) = nb.button_order {
            if int_slot(site, bnode, 14, "Buttons", "ButtonOrder")?.0 != want {
                return Ok(false);
            }
        }
        if nb.picture.is_none() && nb.shortcut.is_none() && nb.description.is_empty() {
            continue;
        }
        let Some(guid) = root_g.atom(5 + 2 * i).map(|(a, _)| a.value.clone()) else { return Ok(false) };
        let Some(meta) = cb_action_meta(t, action_count, &guid) else { return Ok(false) };
        if nb.picture.as_ref().is_some_and(|want| meta.node.children.get(5).and_then(read_picture_spec).as_ref() != Some(want)) {
            return Ok(false);
        }
        if nb.shortcut.as_ref().is_some_and(|want| meta.node.children.get(6).and_then(read_shortcut_spec).as_ref() != Some(want)) {
            return Ok(false);
        }
        if !nb.description.is_empty() {
            let got: Option<LocItems> =
                meta.child(4).and_then(strict_localized).map(|items| items.into_iter().map(|i| (i.lang, i.text)).collect());
            if got.as_ref() != Some(&nb.description) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn patch_cb_buttons(site: &Site, old_btns: &[CommandBarButton], new_btns: &[CommandBarButton], prop: Property) -> Result<Vec<Piece>> {
    if old_btns.len() != new_btns.len() {
        return Err(site.layout("unexpected-shape", "Buttons", "button count mismatch".into()));
    }
    let (t, action_count, root_g) = cb_table(site, prop)?;
    let mut out = Vec::new();
    for (i, (ob, nb)) in old_btns.iter().zip(new_btns.iter()).enumerate() {
        let guid = root_g
            .atom(5 + 2 * i)
            .map(|(a, _)| a.value.clone())
            .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button action guid missing".into()))?;
        let bnode = root_g
            .child(6 + 2 * i)
            .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button node missing".into()))?;
        if ob.name != nb.name {
            let (_, span) = bnode
                .atom(1)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button name missing".into()))?;
            out.push(piece(span, quote(&nb.name), &[prop]));
        }
        if ob.caption != nb.caption {
            let cnode = bnode
                .child(4)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button caption missing".into()))?;
            let items = strict_localized(cnode)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button caption malformed".into()))?;
            out.extend(localized_pieces(site, cnode, &items, &ob.caption, &nb.caption, prop)?);
        }
        if ob.representation != nb.representation {
            let (_, span) = bnode
                .atom(10)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button representation missing".into()))?;
            let rcode = match nb.representation.as_str() {
                "Text" => 1,
                "Picture" => 2,
                "PictureAndText" => 3,
                _ => 0,
            };
            out.push(piece(span, rcode, &[prop]));
        }
        if ob.checked != nb.checked {
            let (_, span) = bnode
                .atom(12)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button checked missing".into()))?;
            out.push(piece(span, i32::from(nb.checked), &[prop]));
        }
        if let Some(def) = nb.default_button.filter(|_| ob.default_button != nb.default_button) {
            let (_, span) = bnode
                .atom(13)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button DefaultButton slot missing".into()))?;
            out.push(piece(span, i32::from(def), &[prop]));
        }
        if let Some(ord) = nb.button_order.filter(|_| ob.button_order != nb.button_order) {
            let (_, span) = bnode
                .atom(14)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button ButtonOrder slot missing".into()))?;
            out.push(piece(span, ord, &[prop]));
        }
        let new_picture = nb.picture.as_ref().filter(|_| ob.picture != nb.picture);
        let new_shortcut = nb.shortcut.as_ref().filter(|_| ob.shortcut != nb.shortcut);
        let new_desc = (!nb.description.is_empty() && ob.description != nb.description).then_some(&nb.description);
        if new_picture.is_some() || new_shortcut.is_some() || new_desc.is_some() {
            let meta = cb_action_meta(t, action_count, &guid)
                .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button action record missing".into()))?;
            if let Some(desc) = new_desc {
                let dnode = meta
                    .child(4)
                    .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button description slot missing".into()))?;
                let items = strict_localized(dnode)
                    .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button description malformed".into()))?;
                let current: LocItems = items.iter().map(|i| (i.lang.clone(), i.text.clone())).collect();
                out.extend(localized_pieces(site, dnode, &items, &current, desc, prop)?);
            }
            for (idx, text) in [
                (5usize, new_picture.map(format_picture_node)),
                (6usize, new_shortcut.map(format_shortcut_node)),
            ] {
                if let Some(text) = text {
                    let span = meta
                        .child(idx)
                        .ok_or_else(|| site.layout("unexpected-shape", "Buttons", "button picture/shortcut slot missing".into()))?
                        .span
                        .span;
                    out.push(piece(span, text, &[prop]));
                }
            }
        }
        if ob.action != nb.action && nb.action_kind == "Handler" {
            for ai in 0..action_count {
                let Some(anode) = t.child(5 + ai) else { continue };
                if anode.atom(1).map(|(a, _)| a.value.as_str()) == Some(guid.as_str()) {
                    if let Some(body) = anode.child(4) {
                        if let Some((_, sp1)) = body.atom(1) {
                            out.push(piece(sp1, quote(&nb.action), &[prop]));
                        }
                        if let Some(meta) = body.child(2) {
                            if let Some((_, sp2)) = meta.atom(1) {
                                out.push(piece(sp2, quote(&nb.action), &[prop]));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

fn read_table_column_values(site: &Site) -> Result<Vec<ColumnValues>> {
    let cols = table_column_props(site)?;
    let mut out = Vec::with_capacity(cols.len());
    for (wrapper, cp) in cols {
        let id = cp.atom(5).and_then(|(a, _)| a.value.parse::<i64>().ok());
        let name = cp
            .atom(30)
            .map(|(a, _)| a.value.clone())
            .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column name atom missing".into()))?;
        let field_name = wrapper
            .atom(2)
            .map(|(a, _)| a.value.clone())
            .filter(|s| !s.is_empty());
        let loc_at = |idx: usize| -> Result<Vec<(String, String)>> {
            let n = cp
                .child(idx)
                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column localized node missing".into()))?;
            Ok(strict_localized(n)
                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column localized node malformed".into()))?
                .into_iter()
                .map(|i| (i.lang, i.text))
                .collect())
        };
        let header = loc_at(1)?;
        let footer_text = loc_at(2)?;
        let tooltip = loc_at(3)?;
        let width_item = cp
            .node
            .children
            .get(4)
            .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column width missing".into()))?;
        let width = crate::canonical::py_round(
            crate::oof_props::num(width_item)
                .map_err(|_| site.layout("unexpected-shape", "Columns", "column width is not a number".into()))?,
        );
        let visible = int_slot(site, cp, 25, "Columns", "Visible")?.0 != 0;
        let ro_flag = int_slot(site, cp, 26, "Columns", "ReadOnly")?.0 != 0;
        let ed_atom = cp
            .child(38)
            .and_then(|e| e.atom(1))
            .or_else(|| cp.atom(38))
            .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column editor guid missing".into()))?
            .0;
        let read_only = ro_flag || ed_atom.value == "00000000-0000-0000-0000-000000000000";
        let editor = if ed_atom.value == "00000000-0000-0000-0000-000000000000" {
            None
        } else {
            control_type(&ed_atom.value).map(str::to_string)
        };
        let events = if editor.is_some() { read_column_editor_events(cp) } else { Vec::new() };
        out.push(ColumnValues {
            id,
            name,
            field_name,
            header,
            footer_text,
            tooltip,
            width,
            visible,
            read_only,
            editor,
            events,
            ext: ColumnExt::default(),
        });
    }
    Ok(out)
}

/// `Location` shares `col_props[26]` with the column read-only flag, so a non-zero location decodes as `ReadOnly`.
fn location_reads_as_read_only(e: &ColumnExt) -> bool {
    matches!(e.location.as_deref(), Some("OnNextLine" | "InSameCell"))
}

/// `(bit, inverted)` of the column flag word `col_props[9]`, in the order of [`column_ext_flags`].
const COLUMN_FLAG_BITS: [(i64, bool); 11] = [
    (1, false),
    (1 << 3, false),
    (1 << 11, false),
    (1 << 12, false),
    (1 << 15, false),
    (1 << 18, true),
    (1 << 19, false),
    (1 << 20, false),
    (1 << 21, false),
    (1 << 22, false),
    (1 << 23, false),
];

fn column_ext_flags(e: &ColumnExt) -> [Option<bool>; 11] {
    [
        e.show_hierarchy,
        e.check_box_three_state,
        e.show_in_header,
        e.show_in_footer,
        e.mark_negatives,
        e.change_visible,
        e.auto_mark_incomplete,
        e.auto_cell_height,
        e.hyperlink,
        e.change_setting,
        e.change_position,
    ]
}

/// Column picture slots `col_props[10..=12]`.
fn column_ext_pictures(e: &ColumnExt) -> [(usize, &Option<PictureSpec>); 3] {
    [(10, &e.rows_pictures), (11, &e.header_picture), (12, &e.footer_picture)]
}

/// Slot (`col_props` index) and code of a `Location` / `SizeChange` / `EditMode` column value.
fn column_enum_code(tag: &str, v: &str) -> Option<i64> {
    let names: &[&str] = match tag {
        "Location" => &["NewColumn", "OnNextLine", "InSameCell"],
        "SizeChange" => &["Change", "DontChange"],
        "EditMode" => &["Enter", "Directly", "EnterOnInput"],
        _ => return None,
    };
    names.iter().position(|n| *n == v).map(|i| i as i64)
}

fn column_ext_enums(e: &ColumnExt) -> [(usize, &'static str, &Option<String>); 3] {
    [(26, "Location", &e.location), (33, "SizeChange", &e.size_change), (36, "EditMode", &e.edit_mode)]
}

fn column_ext_colors(e: &ColumnExt) -> [(usize, &Option<ColorSpec>); 6] {
    [
        (16, &e.field_back_color),
        (17, &e.field_text_color),
        (18, &e.header_back_color),
        (19, &e.header_text_color),
        (20, &e.footer_back_color),
        (21, &e.footer_text_color),
    ]
}

fn column_ext_fonts(e: &ColumnExt) -> [(usize, &Option<FontSpec>); 3] {
    [(22, &e.text_font), (23, &e.header_font), (24, &e.footer_font)]
}

fn column_ext_aligns(e: &ColumnExt) -> [(usize, &Option<String>); 3] {
    [(27, &e.header_horizontal_align), (28, &e.horizontal_align_in_column), (29, &e.footer_horizontal_align)]
}

/// Every write-only column property the model sets holds the requested value in the patched stream.
fn column_ext_matches(site: &Site, want: &[ColumnValues]) -> Result<bool> {
    let cols = table_column_props(site)?;
    if cols.len() != want.len() {
        return Ok(false);
    }
    for ((_, cp), w) in cols.into_iter().zip(want) {
        let e = &w.ext;
        let flags = column_ext_flags(e);
        if flags.iter().any(Option::is_some) {
            let cur = int_slot(site, cp, 9, "Columns", "Flags")?.0;
            for ((bit, inverted), f) in COLUMN_FLAG_BITS.iter().zip(flags) {
                if f.is_some_and(|b| (cur & bit != 0) != (b != *inverted)) {
                    return Ok(false);
                }
            }
        }
        if let Some(v) = e.check_data {
            if int_slot(site, cp, 7, "Columns", "CheckData")?.0 != v {
                return Ok(false);
            }
        }
        for (idx, pic) in column_ext_pictures(e) {
            if pic.as_ref().is_some_and(|p| cp.node.children.get(idx).and_then(read_picture_spec).as_ref() != Some(p)) {
                return Ok(false);
            }
        }
        for (idx, tag, v) in column_ext_enums(e) {
            if let Some(v) = v {
                if Some(int_slot(site, cp, idx, "Columns", tag)?.0) != column_enum_code(tag, v) {
                    return Ok(false);
                }
            }
        }
        for (idx, c) in column_ext_colors(e) {
            if c.as_ref().is_some_and(|c| cp.node.children.get(idx).and_then(read_color_spec).as_ref() != Some(c)) {
                return Ok(false);
            }
        }
        for (idx, f) in column_ext_fonts(e) {
            if f.as_ref().is_some_and(|f| cp.node.children.get(idx).and_then(read_font_spec).as_ref() != Some(f)) {
                return Ok(false);
            }
        }
        for (idx, a) in column_ext_aligns(e) {
            if let Some(a) = a {
                if int_slot(site, cp, idx, "Columns", "Align")?.0 != horizontal_align_code(a) {
                    return Ok(false);
                }
            }
        }
        if !e.format.is_empty() {
            let got: Option<LocItems> =
                cp.child(34).and_then(strict_localized).map(|items| items.into_iter().map(|i| (i.lang, i.text)).collect());
            if got.as_ref() != Some(&e.format) {
                return Ok(false);
            }
        }
        if let Some(h) = e.cell_height {
            if int_slot(site, cp, 37, "Columns", "CellHeight")?.0 != h {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn info_pieces(site: &Site) -> Result<Vec<Piece>> {
    let (old, new) = (&site.edit.old, &site.edit.new);
    let mut out = Vec::new();
    let app_vec: Option<Vec<usize>> = patch_app_path(site.entry.type_name).map(|p| p.iter().map(|&i| i as usize).collect());
    for &prop in &site.edit.changed {
        if let Some(slot) = site.entry.info().and_then(|i| ext_slot(site.entry.type_name, prop, i.node)) {
            out.extend(ext_pieces(site, prop, &slot)?);
            continue;
        }
        match prop {
            Property::DataPath
            | Property::Caption
            | Property::Left
            | Property::Top
            | Property::Width
            | Property::Height
            | Property::Visible
            | Property::ZOrder
            | Property::TabOrder
            | Property::Anchors => {}
            Property::Name => {
                let meta = site
                    .entry
                    .meta()
                    .ok_or_else(|| site.layout("unexpected-shape", "name", "control meta node is missing".into()))?;
                let (cur_atom, span) = meta
                    .atom(1)
                    .ok_or_else(|| site.layout("unexpected-shape", "name", "control meta[1] name atom is missing".into()))?;
                if cur_atom.value != old.name {
                    return Err(site.mismatch("name", &old.name, &cur_atom.value));
                }
                out.push(piece(span, quote(&new.name), &[prop]));
            }
            Property::Events => {
                let cur_events = read_control_events(site.entry)?;
                if cur_events != old.events {
                    return Err(site.mismatch("Events", &format!("{:?}", old.events), &format!("{cur_events:?}")));
                }
                let who = format!("{} '{}' (id {})", site.edit.type_name, site.edit.name, site.edit.id);
                let path = crate::donor::control_event_list_path(site.entry.control.node, site.entry.type_name, &who)?;
                let mut cur = site.entry.control;
                for &i in &path {
                    cur = cur
                        .child(i)
                        .ok_or_else(|| site.layout("unexpected-shape", "Events", format!("event list path {path:?} missing")))?;
                }
                let list_node = crate::donor::build_event_list(&new.events)?;
                let mut buf = Vec::new();
                crate::donor::write_node(&list_node, &mut buf);
                out.push(Piece { range: cur.span.span, bytes: buf, properties: vec![prop] });
            }
            Property::Enabled => {
                let ap = app_vec
                    .as_deref()
                    .ok_or_else(|| site.layout("unexpected-shape", "Enabled", "control has no appearance block".into()))?;
                out.push(info_bool_piece(site, ap, 1, Some(old.enabled), new.enabled, prop)?);
            }
            Property::TransparentBack => {
                let ap = app_vec
                    .as_deref()
                    .ok_or_else(|| site.layout("unexpected-shape", "TransparentBack", "control has no appearance block".into()))?;
                out.push(info_bool_piece(site, ap, 5, Some(old.transparent_back), new.transparent_back, prop)?);
            }
            Property::BackColor | Property::TextColor | Property::BorderColor => {
                let ap = app_vec
                    .as_deref()
                    .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), "control has no appearance block".into()))?;
                let (idx, want) = match prop {
                    Property::BackColor => (2usize, new.back_color.as_ref()),
                    Property::TextColor => (3usize, new.text_color.as_ref()),
                    Property::BorderColor => (6usize, new.border_color.as_ref()),
                    _ => unreachable!(),
                };
                let cnode = info_node_at(site, ap, prop)?
                    .child(idx)
                    .ok_or_else(|| site.layout("unexpected-shape", prop.xml_name(), "appearance color node missing".into()))?;
                out.push(piece(cnode.span.span, format_color_node(want), &[prop]));
            }
            Property::Font => {
                let ap = app_vec
                    .as_deref()
                    .ok_or_else(|| site.layout("unexpected-shape", "Font", "control has no appearance block".into()))?;
                let fnode = info_node_at(site, ap, prop)?
                    .child(4)
                    .ok_or_else(|| site.layout("unexpected-shape", "Font", "appearance font node missing".into()))?;
                out.push(piece(fnode.span.span, format_font_node(new.font.as_ref()), &[prop]));
            }
            Property::Tooltip => {
                let ap = app_vec
                    .as_deref()
                    .ok_or_else(|| site.layout("unexpected-shape", "Tooltip", "control has no appearance block".into()))?;
                let tip_node = info_node_at(site, ap, prop)?
                    .child(12)
                    .ok_or_else(|| site.layout("unexpected-shape", "Tooltip", "appearance block has no Tooltip node".into()))?;
                let items = strict_localized(tip_node)
                    .ok_or_else(|| site.layout("unexpected-shape", "Tooltip", "Tooltip node is not a plain localized string".into()))?;
                out.extend(localized_pieces(site, tip_node, &items, &old.tooltip, &new.tooltip, prop)?);
            }
            Property::ValueType => {
                let pat_node = info_node_at(site, &[], prop)?
                    .child(1)
                    .ok_or_else(|| site.layout("unexpected-shape", "ValueType", "input field pattern node missing".into()))?;
                let pat = match &new.value_type {
                    None => "{\"Pattern\"}".to_string(),
                    Some(vt) => format_attr_pattern(&vt.to_attr())
                        .ok_or_else(|| site.layout("unexpected-shape", "ValueType", "unsupported ValueType pattern".into()))?,
                };
                out.push(piece(pat_node.span.span, pat, &[prop]));
            }
            Property::HorizontalScrollBar | Property::VerticalScrollBar => {
                let is_table = site.entry.type_name == "Table";
                let (idx, want_s) = match (prop == Property::HorizontalScrollBar, is_table) {
                    (true, false) => (2usize, new.horizontal_scroll_bar.as_str()),
                    (false, false) => (3usize, new.vertical_scroll_bar.as_str()),
                    (true, true) => (9usize, new.horizontal_scroll_bar.as_str()),
                    (false, true) => (10usize, new.vertical_scroll_bar.as_str()),
                };
                let (_, span) = info_int_slot(site, if is_table { &[2, 1] } else { &[2, 0] }, idx, prop)?;
                let want = match want_s {
                    "Use" => 1,
                    "Auto" => 2,
                    _ => 0,
                };
                out.push(piece(span, want, &[prop]));
            }
            Property::AutoCellHeight => {
                let want = new.auto_cell_height.ok_or_else(|| site.invalid("invalid-xml", "AutoCellHeight", "cannot remove AutoCellHeight".into()))?;
                out.push(info_bool_piece(site, &[2, 0], 4, old.auto_cell_height, want, prop)?);
            }
            Property::DropListButton => {
                let want = new.drop_list_button.ok_or_else(|| site.invalid("invalid-xml", "DropListButton", "cannot remove DropListButton".into()))?;
                out.push(info_bool_piece(site, &[2, 0], 6, old.drop_list_button, want, prop)?);
            }
            Property::ChoiceButton => {
                let want = new.choice_button.unwrap_or(false);
                out.push(info_bool_piece(site, &[2, 0], 7, old.choice_button, want, prop)?);
            }
            Property::ClearButton => {
                let want = new.clear_button.unwrap_or(false);
                out.push(info_bool_piece(site, &[2, 0], 8, old.clear_button, want, prop)?);
            }
            Property::SpinButton => {
                let want = new.spin_button.ok_or_else(|| site.invalid("invalid-xml", "SpinButton", "cannot remove SpinButton".into()))?;
                out.push(info_bool_piece(site, &[2, 0], 9, old.spin_button, want, prop)?);
            }
            Property::OpenButton => {
                let want = new.open_button.unwrap_or(false);
                out.push(info_bool_piece(site, &[2, 0], 10, old.open_button, want, prop)?);
            }
            Property::ReadOnly if site.entry.type_name == "Table" => table_flag_piece(site, prop, &mut out)?,
            Property::ReadOnly => {
                out.push(info_bool_piece(site, &[2, 0], 13, Some(old.read_only), new.read_only, prop)?);
            }
            Property::MarkNegatives => {
                out.push(info_bool_piece(site, &[2, 0], 27, Some(old.mark_negatives), new.mark_negatives, prop)?);
            }
            Property::AutoMarkIncomplete => {
                out.push(info_bool_piece(site, &[2, 0], 35, Some(old.auto_mark_incomplete), new.auto_mark_incomplete, prop)?);
            }
            Property::AutoChoiceIncomplete => {
                let want = new.auto_choice_incomplete.ok_or_else(|| site.invalid("invalid-xml", "AutoChoiceIncomplete", "cannot remove AutoChoiceIncomplete".into()))?;
                out.push(info_bool_piece(site, &[2, 0], 36, old.auto_choice_incomplete, want, prop)?);
            }
            Property::ChoiceListMode => {
                out.push(info_bool_piece(site, &[], 9, Some(old.choice_list_mode), new.choice_list_mode, prop)?);
            }
            Property::HorizontalAlign => {
                if site.entry.type_name == "InputField" {
                    let (_, span) = info_int_slot(site, &[2, 0], 17, prop)?;
                    out.push(piece(span, horizontal_align_code(&new.horizontal_align), &[prop]));
                } else if site.entry.type_name == "CommandBar" {
                    let (cur, span) = info_int_slot(site, &[1], 4, prop)?;
                    let cur_s = if cur == 2 { "Right" } else { "" };
                    if cur_s != old.horizontal_align {
                        return Err(site.mismatch("HorizontalAlign", &old.horizontal_align, cur_s));
                    }
                    let want = if new.horizontal_align == "Right" { 2 } else { 0 };
                    out.push(piece(span, want, &[prop]));
                } else {
                    let (cur, span) = info_int_slot(site, &[1], 3, prop)?;
                    let cur_s = match cur {
                        1 => "Center",
                        2 => "Right",
                        _ => "",
                    };
                    if cur_s != old.horizontal_align {
                        return Err(site.mismatch("HorizontalAlign", &old.horizontal_align, cur_s));
                    }
                    let want = match new.horizontal_align.as_str() {
                        "Center" => 1,
                        "Right" => 2,
                        _ => 0,
                    };
                    out.push(piece(span, want, &[prop]));
                }
            }
            Property::ModifiesData => out.push(meta_bool_piece(site, 3, new.modifies_data.unwrap_or(false), prop)?),
            Property::SkipOnInput => out.push(meta_bool_piece(site, 4, new.skip_on_input.unwrap_or(false), prop)?),
            Property::FirstInGroup => out.push(meta_bool_piece(site, 5, new.first_in_group.unwrap_or(false), prop)?),
            Property::MultiLine => {
                let want = new.multi_line.unwrap_or(false);
                out.push(info_bool_piece(site, &[2, 0], 26, None, want, prop)?);
                out.extend(ext_piece(site, 3, i32::from(want), prop)?);
            }
            Property::PasswordMode => {
                let want = new.password_mode.unwrap_or(false);
                out.push(info_bool_piece(site, &[2, 0], 5, None, want, prop)?);
                out.extend(ext_piece(site, 5, i32::from(want), prop)?);
            }
            Property::ExtendedEdit => {
                let want = new.extended_edit.unwrap_or(false);
                out.push(info_bool_piece(site, &[2, 0], 24, None, want, prop)?);
                out.extend(ext_piece(site, 6, i32::from(want), prop)?);
            }
            Property::Mask => {
                let q = quote(new.mask.as_deref().unwrap_or(""));
                out.push(piece(info_atom_span(site, &[2, 0], 21, prop)?.1, &q, &[prop]));
                out.extend(ext_piece(site, 4, q, prop)?);
            }
            Property::ChooseType => {
                let (_, span) = info_int_slot(site, &[], 6, prop)?;
                out.push(piece(span, i32::from(new.choose_type.unwrap_or(false)), &[prop]));
            }
            Property::TextEdit => {
                let inv = i32::from(!new.text_edit.unwrap_or(true));
                out.push(piece(info_int_slot(site, &[], 5, prop)?.1, inv, &[prop]));
                out.push(piece(info_int_slot(site, &[2, 0], 12, prop)?.1, inv, &[prop]));
            }
            Property::VerticalAlign => {
                let a = new.vertical_align.as_deref().ok_or_else(|| site.invalid("invalid-xml", "VerticalAlign", "cannot remove VerticalAlign".into()))?;
                out.push(piece(info_int_slot(site, &[2, 0], 18, prop)?.1, vertical_align_code(a), &[prop]));
            }
            Property::TypeLink => {
                let tl = new.type_link.as_ref().ok_or_else(|| site.invalid("invalid-xml", "TypeLink", "cannot remove TypeLink".into()))?;
                let node = info_node_at(site, &[], prop)?
                    .child(8)
                    .ok_or_else(|| site.layout("unexpected-shape", "TypeLink", "info[8] type link node is missing".into()))?;
                out.push(piece(node.span.span, format_type_link(tl), &[prop]));
            }
            Property::Hyperlink => {
                let (idx, exp) = if matches!(site.entry.type_name, "PictureField" | "PictureDecoration") {
                    (3, None)
                } else {
                    (5, Some(old.hyperlink))
                };
                out.push(info_bool_piece(site, &[1], idx, exp, new.hyperlink, prop)?);
            }
            Property::ThreeState => {
                out.push(info_bool_piece(site, &[1], 2, Some(old.three_state), new.three_state, prop)?);
            }
            Property::TitleLocation => {
                let (cur, span) = info_int_slot(site, &[1, 0], 3, prop)?;
                let cur_s = if cur == 0 { "Left" } else { "" };
                if cur_s != old.title_location {
                    return Err(site.mismatch("TitleLocation", &old.title_location, cur_s));
                }
                let want = if new.title_location == "Left" { 0 } else { 1 };
                out.push(piece(span, want, &[prop]));
            }
            Property::ChoiceList => {
                let p = info_node_at(site, &[1], prop)?;
                let span = p
                    .span
                    .children
                    .get(26)
                    .map(SpanItem::span)
                    .ok_or_else(|| site.layout("unexpected-shape", "ChoiceList", "choice list slot missing".into()))?;
                out.push(piece(span, format_choice_list_node(&new.choice_list), &[prop]));
            }
            Property::ShowTabs => {
                let layout = read_panel(site)?;
                if Some(layout.show_tabs.0) != old.show_tabs {
                    return Err(site.mismatch("ShowTabs", &format!("{:?}", old.show_tabs), &layout.show_tabs.0.to_string()));
                }
                let want = new.show_tabs.unwrap_or(0);
                out.push(piece(layout.show_tabs.1, in_i32(site, prop, want)?, &[prop]));
            }
            Property::Pages => {
                let recs = panel_page_records(site)?;
                let cur_pages = read_panel_page_values(site)?;
                let old_pages = pages_without_pictures(&old.pages);
                if cur_pages != old_pages {
                    return Err(site.mismatch("Pages", &format!("{old_pages:?}"), &format!("{cur_pages:?}")));
                }
                if recs.len() != new.pages.len() {
                    return Err(site.layout("unexpected-shape", "Pages", "page count mismatch".into()));
                }
                for (i, rec) in recs.into_iter().enumerate() {
                    let (op, np) = (&old.pages[i], &new.pages[i]);
                    if op.title != np.title {
                        let title_node = rec
                            .child(1)
                            .ok_or_else(|| site.layout("unexpected-shape", "Pages", "page title node missing".into()))?;
                        let items = strict_localized(title_node)
                            .ok_or_else(|| site.layout("unexpected-shape", "Pages", "page title not localized".into()))?;
                        out.extend(localized_pieces(site, title_node, &items, &op.title, &np.title, prop)?);
                    }
                    if op.visible != np.visible {
                        let (_, span) = int_slot(site, rec, 4, "Pages", "Visible")?;
                        out.push(piece(span, i32::from(np.visible), &[prop]));
                    }
                    if op.enabled != np.enabled {
                        let (_, span) = int_slot(site, rec, 5, "Pages", "Enabled")?;
                        out.push(piece(span, i32::from(np.enabled), &[prop]));
                    }
                    if let Some(pic) = np.title_picture.as_ref().filter(|_| op.title_picture != np.title_picture) {
                        let span = rec
                            .child(2)
                            .and_then(|w| w.child(2))
                            .ok_or_else(|| site.layout("unexpected-shape", "Pages", "page title picture node missing".into()))?
                            .span
                            .span;
                        out.push(piece(span, format_picture_node(pic), &[prop]));
                    }
                }
            }
            Property::EnableStartDrag if site.entry.type_name == "Table" => table_flag_piece(site, prop, &mut out)?,
            Property::EnableDrag if site.entry.type_name == "Table" => table_flag_piece(site, prop, &mut out)?,
            Property::UseAlternationRowColor
            | Property::Header
            | Property::Footer
            | Property::VerticalLines
            | Property::HorizontalLines
            | Property::AutoInsertNewRow
            | Property::ChangeSettingOfColumns
            | Property::ChangePositionOfColumns
            | Property::ChoiceMode => table_flag_piece(site, prop, &mut out)?,
            Property::ChangeRowSet => {
                let want = new.change_row_set.ok_or_else(|| site.invalid("invalid-xml", "ChangeRowSet", "cannot remove ChangeRowSet".into()))?;
                out.push(info_bool_piece(site, &[2, 1], 22, old.change_row_set, want, prop)?);
            }
            Property::InitialListView => {
                let want = new.initial_list_view.as_deref().ok_or_else(|| site.invalid("invalid-xml", "InitialListView", "cannot remove InitialListView".into()))?;
                let (_, span) = info_int_slot(site, &[2, 1], 20, prop)?;
                out.push(piece(span, initial_list_view_code(want), &[prop]));
            }
            Property::Output => {
                let want = new.output.as_deref().ok_or_else(|| site.invalid("invalid-xml", "Output", "cannot remove Output".into()))?;
                let (_, span) = info_int_slot(site, &[2, 1], 24, prop)?;
                out.push(piece(span, output_code(want), &[prop]));
            }
            Property::SelectionMode => {
                let (cur, span) = info_int_slot(site, &[2, 1], 15, prop)?;
                let cur_s = if cur == 1 { "Multi" } else { "" };
                if cur_s != old.selection_mode {
                    return Err(site.mismatch("SelectionMode", &old.selection_mode, cur_s));
                }
                let want = if new.selection_mode == "Multi" { 1 } else { 0 };
                out.push(piece(span, want, &[prop]));
            }
            Property::InitialTreeView => {
                let (_, span) = info_int_slot(site, &[2, 1], 21, prop)?;
                let want = match new.initial_tree_view.as_str() {
                    "ExpandTopLevel" => 1,
                    "ExpandAllLevels" => 2,
                    _ => 0,
                };
                out.push(piece(span, want, &[prop]));
            }
            Property::Columns => {
                let cols = table_column_props(site)?;
                let cur_cols = read_table_column_values(site)?;
                let old_cols: Vec<ColumnValues> =
                    old.columns.iter().cloned().map(|c| ColumnValues { ext: ColumnExt::default(), ..c }).collect();
                if cur_cols != old_cols {
                    return Err(site.mismatch("Columns", &format!("{old_cols:?}"), &format!("{cur_cols:?}")));
                }
                if cols.len() != new.columns.len() {
                    return Err(site.layout("unexpected-shape", "Columns", "column count mismatch".into()));
                }
                for (i, (wrapper, cp)) in cols.into_iter().enumerate() {
                    let (oc, nc) = (&old.columns[i], &new.columns[i]);
                    if oc.name != nc.name {
                        let (_, span) = cp
                            .atom(30)
                            .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column name atom missing".into()))?;
                        out.push(piece(span, quote(&nc.name), &[prop]));
                    }
                    if oc.field_name != nc.field_name {
                        let (_, span) = wrapper
                            .atom(2)
                            .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column field name atom missing".into()))?;
                        out.push(piece(span, quote(nc.field_name.as_deref().unwrap_or("")), &[prop]));
                    }
                    for (idx, o_loc, n_loc) in [
                        (1usize, &oc.header, &nc.header),
                        (2usize, &oc.footer_text, &nc.footer_text),
                        (3usize, &oc.tooltip, &nc.tooltip),
                    ] {
                        if o_loc != n_loc {
                            let n = cp
                                .child(idx)
                                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column localized node missing".into()))?;
                            let items = strict_localized(n)
                                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column localized node malformed".into()))?;
                            out.extend(localized_pieces(site, n, &items, o_loc, n_loc, prop)?);
                        }
                    }
                    if oc.width != nc.width {
                        if nc.width <= 0 {
                            return Err(site.invalid("invalid-geometry", "Columns", "column Width must be positive".into()));
                        }
                        let (_, span) = cp
                            .atom(4)
                            .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column width atom missing".into()))?;
                        out.push(piece(span, in_i32(site, prop, nc.width)?, &[prop]));
                    }
                    if oc.visible != nc.visible {
                        let (_, span) = int_slot(site, cp, 25, "Columns", "Visible")?;
                        out.push(piece(span, i32::from(nc.visible), &[prop]));
                    }
                    // `Location` shares `col_props[26]` with the read-only flag and is written by the ext block.
                    if oc.read_only != nc.read_only && !(oc.ext != nc.ext && nc.ext.location.is_some()) {
                        let (_, span) = int_slot(site, cp, 26, "Columns", "ReadOnly")?;
                        out.push(piece(span, i32::from(nc.read_only), &[prop]));
                    }
                    if oc.editor != nc.editor {
                        if let Some(ed) = &nc.editor {
                            let (_, span) = cp
                                .child(38)
                                .and_then(|e| e.atom(1))
                                .or_else(|| cp.atom(38))
                                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column editor guid missing".into()))?;
                            let guid = CONTROL_GUIDS
                                .iter()
                                .find(|(_, n)| *n == ed.as_str())
                                .map(|(g, _)| *g)
                                .unwrap_or("381ed624-9217-4e63-85db-c4c3cb87daae");
                            out.push(piece(span, guid, &[prop]));
                        }
                    }
                    if oc.events != nc.events {
                        let ed_type = nc.editor.as_deref().unwrap_or("InputField");
                        out.push(patch_column_editor_events(site, cp, ed_type, &nc.events, prop)?);
                    }
                    if oc.ext != nc.ext {
                        let ne = &nc.ext;
                        let flags = column_ext_flags(ne);
                        if flags.iter().any(Option::is_some) {
                            let (mut cur, span) = int_slot(site, cp, 9, "Columns", "Flags")?;
                            for ((bit, inverted), f) in COLUMN_FLAG_BITS.iter().zip(flags) {
                                match f.map(|b| b != *inverted) {
                                    Some(true) => cur |= bit,
                                    Some(false) => cur &= !bit,
                                    None => {}
                                }
                            }
                            out.push(piece(span, cur, &[prop]));
                        }
                        if let Some(v) = ne.check_data {
                            let (_, span) = int_slot(site, cp, 7, "Columns", "CheckData")?;
                            out.push(piece(span, v, &[prop]));
                        }
                        for (idx, pic) in column_ext_pictures(ne) {
                            if let Some(pic) = pic {
                                let span = cp
                                    .child(idx)
                                    .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column picture node missing".into()))?
                                    .span
                                    .span;
                                out.push(piece(span, format_picture_node(pic), &[prop]));
                            }
                        }
                        for (idx, tag, v) in column_ext_enums(ne) {
                            if let Some(v) = v {
                                let code = column_enum_code(tag, v)
                                    .ok_or_else(|| site.invalid("invalid-xml", "Columns", format!("invalid {tag} '{v}'")))?;
                                if tag == "Location" && code == 0 && nc.read_only {
                                    return Err(site.invalid(
                                        "invalid-xml",
                                        "Columns",
                                        "Location NewColumn cannot be combined with a read-only column".into(),
                                    ));
                                }
                                let (_, span) = int_slot(site, cp, idx, "Columns", tag)?;
                                out.push(piece(span, code, &[prop]));
                            }
                        }
                        for (idx, c) in column_ext_colors(ne) {
                            if let Some(c) = c {
                                let span = cp
                                    .child(idx)
                                    .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column color node missing".into()))?
                                    .span
                                    .span;
                                out.push(piece(span, format_color_node(Some(c)), &[prop]));
                            }
                        }
                        for (idx, f) in column_ext_fonts(ne) {
                            if let Some(f) = f {
                                let span = cp
                                    .child(idx)
                                    .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column font node missing".into()))?
                                    .span
                                    .span;
                                out.push(piece(span, format_font_node(Some(f)), &[prop]));
                            }
                        }
                        for (idx, a) in column_ext_aligns(ne) {
                            if let Some(a) = a {
                                let (_, span) = int_slot(site, cp, idx, "Columns", "Align")?;
                                out.push(piece(span, horizontal_align_code(a), &[prop]));
                            }
                        }
                        if !ne.format.is_empty() {
                            let n = cp
                                .child(34)
                                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column format node missing".into()))?;
                            let items = strict_localized(n)
                                .ok_or_else(|| site.layout("unexpected-shape", "Columns", "column format node malformed".into()))?;
                            let current: LocItems = items.iter().map(|i| (i.lang.clone(), i.text.clone())).collect();
                            out.extend(localized_pieces(site, n, &items, &current, &ne.format, prop)?);
                        }
                        if let Some(h) = ne.cell_height {
                            let (_, span) = int_slot(site, cp, 37, "Columns", "CellHeight")?;
                            out.push(piece(span, h, &[prop]));
                        }
                    }
                }
            }
            Property::AutoFill => {
                let want = new.auto_fill.unwrap_or(false);
                out.push(info_bool_piece(site, &[1], 3, old.auto_fill, want, prop)?);
            }
            Property::Buttons => {
                out.extend(patch_cb_buttons(site, &old.buttons, &new.buttons, prop)?);
            }
            Property::ContextMenu | Property::ActionSource => {
                let name = prop.xml_name();
                // An absent element is the unset default (0xFFFFFFFF) the decoder leaves out of Form.xml.
                let want = if prop == Property::ContextMenu { new.context_menu } else { new.action_source }.unwrap_or(UNSET_CONTROL_REF);
                let meta = site
                    .entry
                    .meta()
                    .ok_or_else(|| site.layout("unexpected-shape", name, "control meta node is missing".into()))?;
                out.push(piece(int_slot(site, meta, 2, name, name)?.1, want, &[prop]));
            }
            Property::BorderStyle => {
                let (cur, span) = info_int_slot(site, &[1, 3], 3, prop)?;
                if Some(cur) != old.border_style {
                    return Err(site.mismatch("BorderStyle", &format!("{:?}", old.border_style), &cur.to_string()));
                }
                let want = new.border_style.unwrap_or(6);
                out.push(piece(span, in_i32(site, prop, want)?, &[prop]));
            }
            _ => {
                return Err(site.layout(
                    "unexpected-shape",
                    prop.xml_name(),
                    format!("{} is not supported on {}", prop.xml_name(), site.entry.type_name),
                ));
            }
        }
    }
    Ok(out)
}

fn is_geom_prop(p: Property) -> bool {
    matches!(
        p,
        Property::Left
            | Property::Top
            | Property::Width
            | Property::Height
            | Property::Visible
            | Property::ZOrder
            | Property::TabOrder
            | Property::Anchors
    )
}

fn anchors_pieces(site: &Site) -> Result<Vec<Piece>> {
    let mut out = Vec::new();
    if !site.edit.changed.contains(&Property::Anchors) {
        return Ok(out);
    }
    let Some(anchors) = &site.edit.new.anchors else { return Ok(out) };
    let g = site
        .entry
        .geom()
        .ok_or_else(|| site.layout("unexpected-shape", "geometry", "geometry node missing".into()))?;
    for (idx, edge) in anchors.edges() {
        let Some(edge) = edge else { continue };
        let target = g
            .child(idx)
            .and_then(|rec| rec.child(1))
            .ok_or_else(|| site.layout("unexpected-shape", "Anchors", format!("geometry edge record {idx} missing")))?;
        out.push(piece(target.span.span, format_edge_target(&edge), &[Property::Anchors]));
    }
    Ok(out)
}

// ---- orchestration

fn control_pieces(site: &Site) -> Result<Vec<Piece>> {
    let mut out = Vec::new();
    if site.edit.changed.contains(&Property::Caption) {
        out.extend(caption_pieces(site)?);
    }
    if site.edit.changed.iter().copied().any(is_geom_prop) {
        let g = read_geom(site)?;
        out.extend(geometry_pieces(site, &g)?);
        out.extend(visible_pieces(site, &g)?);
        out.extend(order_pieces(site, &g)?);
        out.extend(anchors_pieces(site)?);
    }
    out.extend(info_pieces(site)?);
    Ok(out)
}

/// After patching, read every changed property back from the patched stream by the same strict mapping.
fn read_back(site: &Site, original: &Site) -> Result<Vec<DerivedPage>> {
    let new = &site.edit.new;
    let original_page = original.entry.page;
    let wrong = |p: Property| {
        site.layout("verification-failed", p.xml_name(), "the patched stream does not hold the requested value".to_string())
    };
    if site.edit.changed.iter().copied().any(is_geom_prop) {
        let g = read_geom(site)?;
        if g.page != original_page {
            return Err(site.layout("verification-failed", "page", "the page slot changed".to_string()));
        }
        for p in &site.edit.changed {
            let ok = match p {
                Property::Left => g.left.0 == new.left,
                Property::Top => g.top.0 == new.top,
                Property::Width => g.right.0.checked_sub(g.left.0) == Some(new.width),
                Property::Height => g.bottom.0.checked_sub(g.top.0) == Some(new.height),
                Property::Visible => (g.visible.0 != 0) == new.visible,
                Property::ZOrder => g.z_order.0 == new.z_order,
                Property::TabOrder => g.tab_order.0 == new.tab_order,
                Property::Anchors => {
                    let geom_node = site
                        .entry
                        .geom()
                        .ok_or_else(|| layout_err("geometry node missing after patching"))?
                        .node;
                    new.anchors
                        .as_ref()
                        .is_some_and(|a| a.edges().into_iter().all(|(idx, want)| want.is_none() || read_anchor_edge(geom_node, idx) == want))
                }
                _ => true,
            };
            if !ok {
                return Err(wrong(*p));
            }
        }
    }
    if site.edit.changed.contains(&Property::Caption) {
        let items: Vec<(String, String)> = match caption_slot(site)? {
            Some(slot) => slot.items.into_iter().map(|i| (i.lang, i.text)).collect(),
            None => Vec::new(),
        };
        if items != new.caption {
            return Err(wrong(Property::Caption));
        }
    }
    let app_vec: Option<Vec<usize>> = patch_app_path(site.entry.type_name).map(|p| p.iter().map(|&i| i as usize).collect());
    for &p in &site.edit.changed {
        let ext = site.entry.info().and_then(|i| ext_slot(site.entry.type_name, p, i.node));
        let ok = match p {
            _ if ext.is_some() => {
                ext_matches(site, p, ext.as_ref().ok_or_else(|| wrong(p))?)?
                    && (p != Property::DefaultButton
                        || new.default_button != Some(true)
                        || read_default_button_id(site.source) == Some(site.edit.id))
            }
            Property::ContextMenu | Property::ActionSource => {
                let meta = site.entry.meta().ok_or_else(|| wrong(p))?;
                let want = if p == Property::ContextMenu { new.context_menu } else { new.action_source }.unwrap_or(UNSET_CONTROL_REF);
                int_slot(site, meta, 2, p.xml_name(), p.xml_name())?.0 == want
            }
            Property::Name => site.entry.name == new.name,
            Property::DataPath => read_control_data_path(site.source, site.edit.id) == new.data_path.as_ref().map(|(id, _)| *id),
            Property::Events => read_control_events(site.entry)? == new.events,
            Property::Enabled => (info_int_slot(site, app_vec.as_deref().unwrap(), 1, p)?.0 != 0) == new.enabled,
            Property::TransparentBack => (info_int_slot(site, app_vec.as_deref().unwrap(), 5, p)?.0 != 0) == new.transparent_back,
            Property::BackColor => {
                let n = info_node_at(site, app_vec.as_deref().unwrap(), p)?;
                read_color_spec(&n.node.children[2]) == new.back_color
            }
            Property::TextColor => {
                let n = info_node_at(site, app_vec.as_deref().unwrap(), p)?;
                read_color_spec(&n.node.children[3]) == new.text_color
            }
            Property::BorderColor => {
                let n = info_node_at(site, app_vec.as_deref().unwrap(), p)?;
                read_color_spec(&n.node.children[6]) == new.border_color
            }
            Property::Font => {
                let n = info_node_at(site, app_vec.as_deref().unwrap(), p)?;
                read_font_spec(&n.node.children[4]) == new.font
            }
            Property::Tooltip => {
                let tip_node = info_node_at(site, app_vec.as_deref().unwrap(), p)?.child(12).ok_or_else(|| wrong(p))?;
                let items: Vec<(String, String)> = strict_localized(tip_node)
                    .ok_or_else(|| wrong(p))?
                    .into_iter()
                    .map(|i| (i.lang, i.text))
                    .collect();
                items == new.tooltip
            }
            Property::ValueType => read_input_value_type(site.entry.info().unwrap().node) == new.value_type,
            Property::HorizontalScrollBar | Property::VerticalScrollBar if site.entry.type_name == "Table" => {
                let idx = if p == Property::HorizontalScrollBar { 9 } else { 10 };
                let want = if p == Property::HorizontalScrollBar { &new.horizontal_scroll_bar } else { &new.vertical_scroll_bar };
                let cur = match info_int_slot(site, &[2, 1], idx, p)?.0 {
                    1 => "Use",
                    2 => "Auto",
                    _ => "DontUse",
                };
                cur == want || (want.is_empty() && cur == "DontUse")
            }
            Property::EnableDrag if site.entry.type_name == "Table" => {
                Some(table_flag_read(info_int_slot(site, &[2, 1], 1, p)?.0, p)) == table_flag_value(new, p)
            }
            Property::ReadOnly if site.entry.type_name == "Table" => {
                Some(table_flag_read(info_int_slot(site, &[2, 1], 1, p)?.0, p)) == table_flag_value(new, p)
            }
            Property::UseAlternationRowColor
            | Property::Header
            | Property::EnableStartDrag
            | Property::VerticalLines
            | Property::HorizontalLines
            | Property::AutoInsertNewRow
            | Property::ChangeSettingOfColumns
            | Property::ChangePositionOfColumns
            | Property::ChoiceMode => {
                Some(table_flag_read(info_int_slot(site, &[2, 1], 1, p)?.0, p)) == table_flag_value(new, p)
            }
            Property::ChangeRowSet => Some(info_int_slot(site, &[2, 1], 22, p)?.0 != 0) == new.change_row_set,
            Property::InitialListView => {
                let cur = match info_int_slot(site, &[2, 1], 20, p)?.0 {
                    0 | 2 => "Begin",
                    1 => "End",
                    _ => "Auto",
                };
                new.initial_list_view.as_deref() == Some(cur)
            }
            Property::Output => {
                let cur = match info_int_slot(site, &[2, 1], 24, p)?.0 {
                    1 => "Enable",
                    2 => "Disable",
                    _ => "Auto",
                };
                new.output.as_deref() == Some(cur)
            }
            Property::HorizontalScrollBar => {
                let cur = match info_int_slot(site, &[2, 0], 2, p)?.0 {
                    1 => "Use",
                    2 => "Auto",
                    _ => "",
                };
                cur == new.horizontal_scroll_bar
            }
            Property::VerticalScrollBar => {
                let cur = match info_int_slot(site, &[2, 0], 3, p)?.0 {
                    1 => "Use",
                    2 => "Auto",
                    _ => "",
                };
                cur == new.vertical_scroll_bar
            }
            Property::AutoCellHeight => Some(info_int_slot(site, &[2, 0], 4, p)?.0 != 0) == new.auto_cell_height,
            Property::DropListButton => Some(info_int_slot(site, &[2, 0], 6, p)?.0 != 0) == new.drop_list_button,
            Property::ChoiceButton => Some(info_int_slot(site, &[2, 0], 7, p)?.0 != 0) == new.choice_button,
            Property::ClearButton => Some(info_int_slot(site, &[2, 0], 8, p)?.0 != 0) == new.clear_button,
            Property::SpinButton => Some(info_int_slot(site, &[2, 0], 9, p)?.0 != 0) == new.spin_button,
            Property::OpenButton => Some(info_int_slot(site, &[2, 0], 10, p)?.0 != 0) == new.open_button,
            Property::ReadOnly => (info_int_slot(site, &[2, 0], 13, p)?.0 != 0) == new.read_only,
            Property::MarkNegatives => (info_int_slot(site, &[2, 0], 27, p)?.0 != 0) == new.mark_negatives,
            Property::AutoMarkIncomplete => (info_int_slot(site, &[2, 0], 35, p)?.0 != 0) == new.auto_mark_incomplete,
            Property::AutoChoiceIncomplete => Some(info_int_slot(site, &[2, 0], 36, p)?.0 != 0) == new.auto_choice_incomplete,
            Property::ChoiceListMode => (info_int_slot(site, &[], 9, p)?.0 != 0) == new.choice_list_mode,
            Property::ModifiesData => Some(meta_flag(site, 3, p)?) == new.modifies_data,
            Property::SkipOnInput => Some(meta_flag(site, 4, p)?) == new.skip_on_input,
            Property::FirstInGroup => Some(meta_flag(site, 5, p)?) == new.first_in_group,
            Property::MultiLine => {
                let want = new.multi_line.unwrap_or(false);
                (info_int_slot(site, &[2, 0], 26, p)?.0 != 0) == want && ext_int(site, 3, p)?.is_none_or(|v| (v != 0) == want)
            }
            Property::PasswordMode => {
                let want = new.password_mode.unwrap_or(false);
                (info_int_slot(site, &[2, 0], 5, p)?.0 != 0) == want && ext_int(site, 5, p)?.is_none_or(|v| (v != 0) == want)
            }
            Property::ExtendedEdit => {
                let want = new.extended_edit.unwrap_or(false);
                (info_int_slot(site, &[2, 0], 24, p)?.0 != 0) == want && ext_int(site, 6, p)?.is_none_or(|v| (v != 0) == want)
            }
            Property::Mask => {
                let want = new.mask.as_deref().unwrap_or("");
                let ext_ok = match string_ext_body(info_node_at(site, &[], p)?) {
                    None => true,
                    Some(body) => body.atom(4).is_some_and(|(a, _)| a.kind == QUOTED && a.value == want),
                };
                let (cur, _) = info_atom_span(site, &[2, 0], 21, p)?;
                cur == want && ext_ok
            }
            Property::ChooseType => Some(info_int_slot(site, &[], 6, p)?.0 != 0) == new.choose_type,
            Property::TextEdit => {
                let want = i64::from(!new.text_edit.unwrap_or(true));
                info_int_slot(site, &[], 5, p)?.0 == want && info_int_slot(site, &[2, 0], 12, p)?.0 == want
            }
            Property::VerticalAlign => {
                Some(info_int_slot(site, &[2, 0], 18, p)?.0) == new.vertical_align.as_deref().map(vertical_align_code)
            }
            Property::TypeLink => {
                let expected = new
                    .type_link
                    .as_ref()
                    .and_then(|tl| crate::brace::parse_metadata(format_type_link(tl).as_bytes()).ok());
                expected.is_some() && info_node_at(site, &[], p)?.child(8).map(|n| n.node) == expected.as_ref()
            }
            Property::HorizontalAlign => {
                if site.entry.type_name == "InputField" {
                    info_int_slot(site, &[2, 0], 17, p)?.0 == horizontal_align_code(&new.horizontal_align)
                } else if site.entry.type_name == "CommandBar" {
                    let cur = if info_int_slot(site, &[1], 4, p)?.0 == 2 { "Right" } else { "" };
                    cur == new.horizontal_align
                } else {
                    let cur = match info_int_slot(site, &[1], 3, p)?.0 {
                        1 => "Center",
                        2 => "Right",
                        _ => "",
                    };
                    cur == new.horizontal_align
                }
            }
            Property::Hyperlink => {
                let idx = if matches!(site.entry.type_name, "PictureField" | "PictureDecoration") {
                    3
                } else {
                    5
                };
                (info_int_slot(site, &[1], idx, p)?.0 != 0) == new.hyperlink
            }
            Property::ThreeState => (info_int_slot(site, &[1], 2, p)?.0 != 0) == new.three_state,
            Property::TitleLocation => {
                let cur = if info_int_slot(site, &[1, 0], 3, p)?.0 == 0 { "Left" } else { "" };
                cur == new.title_location
            }
            Property::ChoiceList => {
                let info = site.entry.info().unwrap().node;
                let plain: Vec<ChoiceItemSpec> = new
                    .choice_list
                    .iter()
                    .map(|it| ChoiceItemSpec { checked: None, picture_id: None, ..it.clone() })
                    .collect();
                read_choice_list_values(info) == plain && choice_list_extras_match(info, &new.choice_list)
            }
            Property::ShowTabs => Some(read_panel(site)?.show_tabs.0) == new.show_tabs,
            Property::Pages => {
                read_panel_page_values(site)? == pages_without_pictures(&new.pages)
                    && panel_page_records(site)?.into_iter().zip(&new.pages).all(|(rec, np)| {
                        np.title_picture.as_ref().is_none_or(|want| {
                            rec.child(2).and_then(|w| w.node.children.get(2)).and_then(read_picture_spec).as_ref() == Some(want)
                        })
                    })
            }
            Property::Footer => Some(table_flag_read(info_int_slot(site, &[2, 1], 1, p)?.0, p)) == new.footer,
            Property::SelectionMode => {
                let cur = if info_int_slot(site, &[2, 1], 15, p)?.0 == 1 { "Multi" } else { "" };
                cur == new.selection_mode
            }
            Property::InitialTreeView => {
                let cur = match info_int_slot(site, &[2, 1], 21, p)?.0 {
                    1 => "ExpandTopLevel",
                    2 => "ExpandAllLevels",
                    _ => "",
                };
                cur == new.initial_tree_view
            }
            Property::Columns => {
                let plain: Vec<ColumnValues> =
                    new.columns.iter().cloned().map(|c| ColumnValues { read_only: c.read_only || location_reads_as_read_only(&c.ext), ext: ColumnExt::default(), ..c }).collect();
                read_table_column_values(site)? == plain && column_ext_matches(site, &new.columns)?
            }
            Property::AutoFill => Some(info_int_slot(site, &[1], 3, p)?.0 != 0) == new.auto_fill,
            Property::Buttons => {
                read_cb_buttons(site.entry.info().unwrap().node) == buttons_without_extras(&new.buttons)
                    && cb_extras_match(site, &new.buttons)?
            }
            Property::BorderStyle => Some(info_int_slot(site, &[1, 3], 3, p)?.0) == new.border_style,
            _ => true,
        };
        if !ok {
            return Err(wrong(p));
        }
    }
    if is_panel_resize(site.edit) {
        let mut expected = expected_panel(original)?;
        if site.edit.changed.contains(&Property::ShowTabs) {
            if let Some(st) = new.show_tabs {
                expected.0 = st;
            }
        }
        let actual = panel_pages(panel_info(site)?.node, new.width, new.height);
        if actual.0 != expected.0 || (!site.edit.changed.contains(&Property::ShowTabs) && actual != expected) {
            return Err(site.layout(
                "verification-failed",
                "panel",
                "the patched panel does not decode to the requested size with the derived page geometry".to_string(),
            ));
        }
        return Ok(derived_pages(site.edit.id, &actual.1));
    }
    if site.edit.type_name == "Panel" && site.edit.changed.contains(&Property::ShowTabs) {
        let actual = panel_pages(panel_info(site)?.node, new.width, new.height);
        return Ok(derived_pages(site.edit.id, &actual.1));
    }
    Ok(Vec::new())
}

type ZMember<'e, 'a> = (&'e ControlEntry<'a>, i64, i64);

/// The default button id held by the form header (`root[1][1][2]`).
fn read_default_button_id(source: &SpannedMetadata) -> Option<i64> {
    source.root.node(1)?.node(1)?.children.get(2)?.as_atom()?.value.trim().parse().ok()
}

fn read_control_data_path(source: &SpannedMetadata, control_id: i64) -> Option<i64> {
    let links = source.root.node(2)?.node(3)?;
    for item in links.children.iter().skip(1) {
        let Item::Node(row) = item else { continue };
        let cid: i64 = row.children.first()?.as_atom()?.value.parse().ok()?;
        if cid == control_id {
            let body = row.node(1)?;
            let aid_item = body.children.get(1)?;
            return crate::oof_props::int_or_first(aid_item, "attr id").ok();
        }
    }
    None
}

impl FormPatchPlan<'_> {
    /// Structural edits (additions/deletions/attributes/DataPath links): produce replacements on the original stream and the
    /// data `verify_structural` needs afterwards. Property pieces are produced separately by the caller.
    fn structural_pieces(&self, out: &mut Vec<AppliedReplacement>) -> Result<Vec<(i64, i64)>> {
        let mut allocated: Vec<(i64, i64)> = Vec::new();
        let has_dp_edits = self.diff.edits.iter().any(|e| e.changed.contains(&Property::DataPath));
        let default_buttons: Vec<i64> = self
            .diff
            .edits
            .iter()
            .filter(|e| e.changed.contains(&Property::DefaultButton) && e.new.default_button == Some(true))
            .map(|e| e.id)
            .collect();
        if self.diff.additions.is_empty()
            && self.diff.deletions.is_empty()
            && !self.diff.form_caption_changed
            && !self.diff.form_size_changed
            && !self.diff.form_events_changed
            && !self.diff.attributes_changed
            && !self.diff.form_props_changed
            && !has_dp_edits
            && default_buttons.is_empty()
        {
            return Ok(allocated);
        }

        // ---- form size (root[1][3..=4], root[1][11..=12], and root panel boundary rows)
        if self.diff.form_size_changed {
            let (Some(new_w), Some(new_h)) = self.diff.new_form_size else {
                return Err(layout_err("form stream: new form Width and Height must be present"));
            };
            let h1 = self.index.at_path(&[1])?;
            let (cur_w, span_w) = h1
                .atom(3)
                .and_then(|(a, s)| Some((strict_int(&a.value)?, s)))
                .ok_or_else(|| layout_err("form stream: root[1][3] width atom missing"))?;
            let (cur_h, span_h) = h1
                .atom(4)
                .and_then(|(a, s)| Some((strict_int(&a.value)?, s)))
                .ok_or_else(|| layout_err("form stream: root[1][4] height atom missing"))?;
            if self.diff.old_form_size != (Some(cur_w), Some(cur_h)) {
                return Err(ExtractionError::new(
                    GENERATED_FILE_MODIFIED,
                    format!("form size mismatch: XML has {:?}, stream has ({cur_w}, {cur_h})", self.diff.old_form_size),
                )
                .with("kind", "baseline-mismatch"));
            }
            if cur_w != new_w {
                out.push(AppliedReplacement { control_id: 0, properties: Vec::new(), range: span_w, bytes: new_w.to_string().into_bytes() });
                if let Some((a11, sp11)) = h1.atom(11) {
                    if strict_int(&a11.value) == Some(cur_w) {
                        out.push(AppliedReplacement { control_id: 0, properties: Vec::new(), range: sp11, bytes: new_w.to_string().into_bytes() });
                    }
                }
            }
            if cur_h != new_h {
                out.push(AppliedReplacement { control_id: 0, properties: Vec::new(), range: span_h, bytes: new_h.to_string().into_bytes() });
                if let Some((a12, sp12)) = h1.atom(12) {
                    if strict_int(&a12.value) == Some(cur_h) {
                        out.push(AppliedReplacement { control_id: 0, properties: Vec::new(), range: sp12, bytes: new_h.to_string().into_bytes() });
                    }
                }
            }
            let dw = new_w - cur_w;
            let dh = new_h - cur_h;
            if let Ok(p) = self.index.at_path(&[1, 2, 1, 1]) {
                if let Some(page_idx) = panel_page_list_idx(p.node) {
                    let rows_idx = page_idx + 4;
                    if let Some(rows) = p.atom(rows_idx).and_then(|(a, _)| a.value.parse::<usize>().ok()) {
                        for r in 0..rows {
                            let Some(row) = p.child(rows_idx + 1 + r) else { continue };
                            let Some(edge) = row.atom(4).and_then(|(a, _)| strict_int(&a.value)) else { continue };
                            let Some((coord, sp)) = row.atom(1).and_then(|(a, s)| Some((strict_int(&a.value)?, s))) else { continue };
                            let delta = match edge {
                                3 => dw,
                                4 => dh,
                                _ => 0,
                            };
                            if delta != 0 {
                                let updated = coord + delta;
                                if updated < 1 {
                                    return Err(ExtractionError::new(
                                        GENERATED_FILE_MODIFIED,
                                        "form resize would shrink a root page client area below 1".to_string(),
                                    )
                                    .with("kind", "invalid-geometry"));
                                }
                                out.push(AppliedReplacement { control_id: 0, properties: Vec::new(), range: sp, bytes: updated.to_string().into_bytes() });
                            }
                        }
                    }
                }
            }
        }

        // ---- default button (root[1][1][2])
        if let Some(&id) = default_buttons.first() {
            if default_buttons.len() > 1 {
                return Err(layout_err("form stream: only one button can be the default button"));
            }
            let (_, span) = self
                .index
                .at_path(&[1, 1])?
                .atom(2)
                .ok_or_else(|| layout_err("form stream: default button slot missing at root[1][1][2]"))?;
            out.push(AppliedReplacement {
                control_id: id,
                properties: vec![Property::DefaultButton],
                range: span,
                bytes: id.to_string().into_bytes(),
            });
        }

        // ---- form root properties
        if self.diff.form_props_changed {
            let fp = &self.diff.new_form_props;
            let root = self.index.at_path(&[])?;
            let h1 = self.index.at_path(&[1])?;
            for (idx, val) in [
                (5usize, fp.auto_title.map(i64::from)),
                (6, fp.window_state),
                (8, fp.size_change),
                (9, fp.connectable_window.map(i64::from)),
                (10, fp.allow_join_window.map(i64::from)),
                (14, fp.window_appearance_mode),
                (15, fp.window_location),
                (16, fp.change_window_appearance_mode),
                (17, fp.desktop_mode.map(i64::from)),
                (18, fp.allow_close.map(i64::from)),
                (19, fp.enter_key_behavior),
            ] {
                if let Some(v) = val {
                    let (_, span) = root
                        .atom(idx)
                        .ok_or_else(|| layout_err(format!("form stream: root[{idx}] atom missing")))?;
                    out.push(AppliedReplacement {
                        control_id: 0,
                        properties: Vec::new(),
                        range: span,
                        bytes: v.to_string().into_bytes(),
                    });
                }
            }
            for (idx, val) in [
                (5usize, fp.allow_normal_state.map(i64::from)),
                (6, fp.allow_docked_state.map(i64::from)),
                (7, fp.allow_autohide_state.map(i64::from)),
                (8, fp.allow_float_state.map(i64::from)),
                (9, fp.window_dock_location),
            ] {
                if let Some(v) = val {
                    let (_, span) = h1
                        .atom(idx)
                        .ok_or_else(|| layout_err(format!("form stream: root[1][{idx}] atom missing")))?;
                    out.push(AppliedReplacement {
                        control_id: 0,
                        properties: Vec::new(),
                        range: span,
                        bytes: v.to_string().into_bytes(),
                    });
                }
            }
            if let Some(style) = &fp.style_ref {
                let node = self.index.at_path(&[12]).map_err(|_| layout_err("form stream: root[12] style node missing"))?;
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: node.span.span,
                    bytes: format_style_node(style).into_bytes(),
                });
            }
            if let Some(id) = fp.default_control_id {
                let p = self.index.at_path(&[1, 2, 1, 1]).map_err(|_| layout_err("form stream: root panel parameters missing"))?;
                let (_, span) = default_control_slot(p.node)
                    .and_then(|i| p.atom(i))
                    .ok_or_else(|| layout_err("form stream: root panel default control slot missing"))?;
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: span,
                    bytes: id.to_string().into_bytes(),
                });
            }
            if let Some(pic) = &fp.caption_picture {
                let pnode = self
                    .index
                    .at_path(&[13, 2])
                    .map_err(|_| layout_err("form stream: root[13][2] caption picture missing"))?;
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: pnode.span.span,
                    bytes: format_picture_node(pic).into_bytes(),
                });
            }
        }
        // list span start -> (list NodeRef, original count, edits)
        let mut lists: BTreeMap<usize, (NodeRef, usize, ListEdit)> = BTreeMap::new();
        let list_entry = |nr: NodeRef| -> Result<(usize, usize)> {
            let Some((count_atom, _)) = nr.atom(0) else {
                return Err(layout_err("structural edit: list has no count atom"));
            };
            let count: usize = count_atom.value.parse().map_err(|_| layout_err("structural edit: bad count atom"))?;
            Ok((nr.span.span.start, count))
        };

        // ---- form caption (root[1][1][0])
        if self.diff.form_caption_changed {
            let cap_node = self
                .index
                .at_path(&[1, 1, 0])
                .ok()
                .ok_or_else(|| layout_err("form stream: form caption node missing at root[1][1][0]"))?;
            let mut buf = Vec::new();
            crate::donor::write_node(&localized_node(&self.diff.new_form_caption), &mut buf);
            out.push(AppliedReplacement {
                control_id: 0,
                properties: Vec::new(),
                range: cap_node.span.span,
                bytes: buf,
            });
        }

        // ---- form events (root[4], root[1][2][1], root[3])
        if self.diff.form_events_changed {
            let old_main: Vec<(i64, String)> = self.diff.old_form_events.iter().filter(|(t, _)| (70000..80000).contains(t)).cloned().collect();
            let new_main: Vec<(i64, String)> = self.diff.new_form_events.iter().filter(|(t, _)| (70000..80000).contains(t)).cloned().collect();
            if old_main != new_main {
                let ev_sec = self.index.at_path(&[4])?;
                let list_node = crate::donor::build_event_list(&new_main)?;
                let mut buf = Vec::new();
                crate::donor::write_node(&list_node, &mut buf);
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: ev_sec.span.span,
                    bytes: buf,
                });
            }
            let old_p: Vec<(i64, String)> = self.diff.old_form_events.iter().filter(|(t, _)| *t == 0).cloned().collect();
            let new_p: Vec<(i64, String)> = self.diff.new_form_events.iter().filter(|(t, _)| *t == 0).cloned().collect();
            if old_p != new_p {
                let pinfo = self.index.at_path(&[1, 2, 1])?;
                let paths = crate::donor::event_list_paths(pinfo.node);
                if paths.len() == 1 {
                    let mut cur = pinfo;
                    for &i in &paths[0] {
                        cur = cur.child(i).ok_or_else(|| layout_err("root panel event list missing"))?;
                    }
                    let list_node = crate::donor::build_event_list(&new_p)?;
                    let mut buf = Vec::new();
                    crate::donor::write_node(&list_node, &mut buf);
                    out.push(AppliedReplacement {
                        control_id: 0,
                        properties: Vec::new(),
                        range: cur.span.span,
                        bytes: buf,
                    });
                }
            }
            let old_ext: Vec<(i64, String)> = self.diff.old_form_events.iter().filter(|(t, _)| *t >= 80000).cloned().collect();
            let new_ext: Vec<(i64, String)> = self.diff.new_form_events.iter().filter(|(t, _)| *t >= 80000).cloned().collect();
            if old_ext != new_ext {
                let ext = self.index.at_path(&[3])?;
                let paths = crate::donor::event_list_paths(ext.node);
                if paths.len() == 1 {
                    let mut cur = ext;
                    for &i in &paths[0] {
                        cur = cur.child(i).ok_or_else(|| layout_err("extension event list missing"))?;
                    }
                    let list_node = crate::donor::build_event_list(&new_ext)?;
                    let mut buf = Vec::new();
                    crate::donor::write_node(&list_node, &mut buf);
                    out.push(AppliedReplacement {
                        control_id: 0,
                        properties: Vec::new(),
                        range: cur.span.span,
                        bytes: buf,
                    });
                }
            }
        }

        // ---- form attributes (root[2][1] and root[2][2])
        if self.diff.attributes_changed {
            let attr_sec = self.index.at_path(&[2])?;
            let (_, next_id_span) = attr_sec
                .atom(1)
                .ok_or_else(|| layout_err("form stream: next attribute id slot missing at root[2][1]"))?;
            let attr_list = attr_sec
                .child(2)
                .ok_or_else(|| layout_err("form stream: attributes list missing at root[2][2]"))?;
            let (_k, acount) = list_entry(attr_list)?;
            let mut orig_by_id: HashMap<i64, Span> = HashMap::new();
            let mut orig_pattern_by_id: HashMap<i64, Span> = HashMap::new();
            let mut kept_records: Vec<String> = Vec::new();
            let mut max_attr_id: i64 = 0;
            let old_ids: std::collections::HashSet<i64> = self.diff.old_attributes.iter().map(|a| a.id).collect();
            for i in 1..=acount {
                let rec = attr_list
                    .child(i)
                    .ok_or_else(|| layout_err("form stream: attribute record is not a node"))?;
                let id_item = rec
                    .node
                    .children
                    .first()
                    .ok_or_else(|| layout_err("form stream: attribute record id missing"))?;
                let aid = crate::oof_props::int_or_first(id_item, "attribute id").map_err(|e| layout_err(e.to_string()))?;
                orig_by_id.insert(aid, rec.span.span);
                if let Some(pat) = rec.child(5) {
                    orig_pattern_by_id.insert(aid, pat.span.span);
                }
                if !old_ids.contains(&aid) {
                    max_attr_id = max_attr_id.max(aid);
                    let raw_slice = std::str::from_utf8(&self.source.source()[rec.span.span.start..rec.span.span.end])
                        .map_err(|_| layout_err("form stream: non-utf8 attribute record"))?;
                    kept_records.push(raw_slice.to_string());
                }
            }
            for a in &self.diff.new_attributes {
                max_attr_id = max_attr_id.max(a.id);
                if self.diff.old_attributes.iter().any(|old| old == a) {
                    if let Some(sp) = orig_by_id.get(&a.id) {
                        let raw_slice = std::str::from_utf8(&self.source.source()[sp.start..sp.end])
                            .map_err(|_| layout_err("form stream: non-utf8 attribute record"))?;
                        kept_records.push(raw_slice.to_string());
                        continue;
                    }
                }
                // A renamed/re-flagged attribute keeps its original pattern bytes when its type is unchanged.
                let raw_pattern = match (self.diff.old_attributes.iter().find(|old| old.id == a.id), orig_pattern_by_id.get(&a.id)) {
                    (Some(old), Some(sp)) if old.same_value_type(a) => Some(
                        std::str::from_utf8(&self.source.source()[sp.start..sp.end])
                            .map_err(|_| layout_err("form stream: non-utf8 attribute pattern"))?,
                    ),
                    _ => None,
                };
                kept_records.push(format_attr_record(a, raw_pattern)?);
            }
            let mut list_text = format!("{{{}" , kept_records.len());
            for r in &kept_records {
                list_text.push(',');
                list_text.push_str(r);
            }
            list_text.push('}');
            out.push(AppliedReplacement {
                control_id: 0,
                properties: Vec::new(),
                range: next_id_span,
                bytes: (max_attr_id + 1).to_string().into_bytes(),
            });
            out.push(AppliedReplacement {
                control_id: 0,
                properties: Vec::new(),
                range: attr_list.span.span,
                bytes: list_text.into_bytes(),
            });
        }

        // ---- deletions
        for d in &self.diff.deletions {
            let entry = self
                .index
                .get(d.id)
                .ok_or_else(|| layout_err(format!("deleted control '{}' (id {}) not found in the stream", d.name, d.id)))?;
            let list = self.index.at_path(&entry.list_path)?;
            let (key, count) = list_entry(list)?;
            lists.entry(key).or_insert_with(|| (list, count, ListEdit::default())).2.removals.push(entry.list_index);
            // DataPath links of the deleted id
            if let Some(links) = self.index.links_list() {
                let (_k, lcount) = list_entry(links)?;
                let rows: Vec<usize> = links
                    .node
                    .children
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter(|(_, c)| {
                        matches!(c, Item::Node(row) if row.children.first().and_then(|i| i.as_atom()).and_then(|a| a.value.parse::<i64>().ok()) == Some(d.id))
                    })
                    .map(|(i, _)| i - 1)
                    .collect();
                if !rows.is_empty() {
                    let e = lists.entry(links.span.span.start).or_insert_with(|| (links, lcount, ListEdit::default()));
                    e.2.removals.extend(rows);
                }
            }
        }

        // ---- additions
        let mut max_id = crate::donor::max_control_id(self.source)?;
        for c in &self.diff.additions {
            if let Some(pid) = c.parent {
                let parent = self
                    .index
                    .get(pid)
                    .ok_or_else(|| layout_err(format!("added control '{}': parent id {pid} not found", c.name)))?;
                if parent.type_name == "CommandBar" {
                    return Err(layout_err(format!(
                        "added control '{}': adding under a CommandBar is not supported (command bar buttons are edited separately)",
                        c.name
                    )));
                }
            }
            let page = resolve_page(&self.index, c.parent, c.page.as_deref())?;
            max_id += 1;
            let mut clone = crate::donor::clone_prototype_raw(&c.type_name, max_id, &c.name)?;
            set_clone_geometry(&mut clone, &c.values, page)?;
            set_clone_caption(&mut clone, &c.type_name, &c.values.caption)?;
            set_clone_props(&mut clone, &c.type_name, &c.values)?;
            set_clone_events(&mut clone, &xml_events(c)?, &c.describe())?;
            let mut buf = Vec::new();
            crate::donor::write_node(&clone, &mut buf);
            let list = match c.parent {
                Some(pid) => self
                    .index
                    .get(pid)
                    .and_then(|e| e.children_list())
                    .ok_or_else(|| layout_err(format!("added control '{}': parent has no children list", c.name)))?,
                None => self.index.root_children_list()?,
            };
            let (key, count) = list_entry(list)?;
            lists.entry(key).or_insert_with(|| (list, count, ListEdit::default())).2.inserts.push(buf);
            allocated.push((c.id, max_id));
            if let Some((attr_id, _)) = &c.values.data_path {
                if let Some(links) = self.index.links_list() {
                    let (_k, lcount) = list_entry(links)?;
                    let e = lists.entry(links.span.span.start).or_insert_with(|| (links, lcount, ListEdit::default()));
                    e.2.inserts.push(format!("{{{max_id},{{1,{{{attr_id}}}}}}}").into_bytes());
                }
            }
        }

        // ---- existing control DataPath edits
        for ed in &self.diff.edits {
            if !ed.changed.contains(&Property::DataPath) {
                continue;
            }
            let Some(links) = self.index.links_list() else {
                return Err(layout_err("form stream: links list missing at root[2][3]"));
            };
            let (_k, lcount) = list_entry(links)?;
            let existing_idx = links.node.children.iter().enumerate().skip(1).find_map(|(i, c)| {
                let Item::Node(row) = c else { return None };
                let cid: i64 = row.children.first()?.as_atom()?.value.parse().ok()?;
                (cid == ed.id).then_some(i)
            });
            match (existing_idx, &ed.new.data_path) {
                (Some(i), Some((new_aid, _))) => {
                    let row = links
                        .child(i)
                        .and_then(|r| r.child(1))
                        .ok_or_else(|| layout_err("form stream: link row body missing"))?;
                    out.push(AppliedReplacement {
                        control_id: ed.id,
                        properties: vec![Property::DataPath],
                        range: row.span.span,
                        bytes: format!("{{1,{{{new_aid}}}}}").into_bytes(),
                    });
                }
                (Some(i), None) => {
                    let e = lists.entry(links.span.span.start).or_insert_with(|| (links, lcount, ListEdit::default()));
                    e.2.removals.push(i - 1);
                }
                (None, Some((new_aid, _))) => {
                    let e = lists.entry(links.span.span.start).or_insert_with(|| (links, lcount, ListEdit::default()));
                    e.2.inserts.push(format!("{{{0},{{1,{{{new_aid}}}}}}}", ed.id).into_bytes());
                }
                (None, None) => {}
            }
        }

        // ---- materialize replacements
        for (_key, (list, count, edit)) in lists {
            let control_id = 0;
            for &k in &edit.removals {
                let range = removal_range(list.span, k, count)?;
                out.push(AppliedReplacement { control_id, properties: Vec::new(), range, bytes: Vec::new() });
            }
            if !edit.inserts.is_empty() {
                let mut text = Vec::new();
                let existing_after = count - edit.removals.len();
                for (i, ins) in edit.inserts.iter().enumerate() {
                    if existing_after + i > 0 {
                        text.push(b',');
                    }
                    text.extend_from_slice(ins);
                }
                // Insert right after the last item COVERED BY THE COUNT: trailing nodes beyond the
                // count (tab records, sentinels) must stay after the inserted controls.
                let at = if count > 0 {
                    match list.span.children.get(count) {
                        Some(item) => item.span().end,
                        None => return Err(layout_err("structural edit: list span does not match its count")),
                    }
                } else {
                    list.span
                        .children
                        .first()
                        .map(|c| c.span().end)
                        .ok_or_else(|| layout_err("structural edit: count atom span missing"))?
                };
                out.push(AppliedReplacement { control_id, properties: Vec::new(), range: Span { start: at, end: at }, bytes: text });
            }
            let final_count = count - edit.removals.len() + edit.inserts.len();
            if final_count != count {
                let Some(SpanItem::Atom(count_span)) = list.span.children.first() else {
                    return Err(layout_err("structural edit: count atom span missing"));
                };
                out.push(AppliedReplacement {
                    control_id,
                    properties: Vec::new(),
                    range: *count_span,
                    bytes: final_count.to_string().into_bytes(),
                });
            }
        }

        // ---- max-id slot
        if !allocated.is_empty() {
            let head = self.index.at_path(&[1, 1])?;
            let Some((_a, span)) = head.atom(1) else {
                return Err(layout_err("form stream: max control id slot missing"));
            };
            out.push(AppliedReplacement { control_id: 0, properties: Vec::new(), range: span, bytes: max_id.to_string().into_bytes() });
        }
        Ok(allocated)
    }

    /// After structural patching: additions are present with the requested values, deletions are gone.
    fn verify_structural(&self, index: &ControlIndex, allocated: &[(i64, i64)]) -> Result<()> {
        // stored checks are recomputed by structural_pieces; keep the signature small by re-deriving
        let bad = |m: String| ExtractionError::new(GENERATED_FILE_MODIFIED, m).with("kind", "verification-failed");
        for d in &self.diff.deletions {
            if index.get(d.id).is_some() {
                return Err(bad(format!("deleted control '{}' (id {}) is still present", d.name, d.id)));
            }
        }
        let mut added = self.diff.additions.iter().zip(allocated);
        for (c, (_, new_id)) in &mut added {
            let Some(e) = index.get(*new_id) else {
                return Err(bad(format!("added control '{}' (id {new_id}) is missing after patching", c.name)));
            };
            if e.name != c.name || e.type_name != c.type_name {
                return Err(bad(format!("added control id {new_id}: expected {} '{}', found {} '{}'", c.type_name, c.name, e.type_name, e.name)));
            }
            let g = e.geom().ok_or_else(|| bad(format!("added control '{}' has no geometry", c.name)))?;
            let gi = |i: usize| g.atom(i).and_then(|(a, _)| a.value.parse::<i64>().ok());
            let (l, t, r, b, v) = (gi(1), gi(2), gi(3), gi(4), gi(5));
            let want = &c.values;
            if l != Some(want.left) || t != Some(want.top) || r != Some(want.left + want.width) || b != Some(want.top + want.height) || v != Some(i64::from(want.visible)) {
                return Err(bad(format!("added control '{}' (id {new_id}): geometry does not match the requested values", c.name)));
            }
        }
        Ok(())
    }

    /// ZOrder values must stay unique among the controls of one parent/page whenever they were unique in the
    /// original stream.
    fn check_z_order_collisions(&self) -> Result<()> {
        let edited: HashMap<i64, &ControlEdit> =
            self.diff.edits.iter().filter(|e| e.changed.contains(&Property::ZOrder)).map(|e| (e.id, e)).collect();
        if edited.is_empty() {
            return Ok(());
        }
        let mut groups: BTreeMap<(Option<i64>, i64), Vec<ZMember>> = BTreeMap::new();
        for e in &self.index.entries {
            let original = e.geom().map(|g| geom_page(g.node).1).unwrap_or(0);
            let result = edited.get(&e.id).map_or(original, |ed| ed.new.z_order);
            groups.entry((e.parent, e.page)).or_default().push((e, original, result));
        }
        for members in groups.values().filter(|m| m.iter().any(|(e, ..)| edited.contains_key(&e.id))) {
            let distinct = |f: fn(&ZMember) -> i64| {
                let mut v: Vec<i64> = members.iter().map(f).collect();
                v.sort_unstable();
                v.windows(2).all(|w| w[0] != w[1])
            };
            if !distinct(|m| m.1) || distinct(|m| m.2) {
                continue;
            }
            for (a, (first, _, z)) in members.iter().enumerate() {
                if let Some((second, _, _)) = members[a + 1..].iter().find(|m| m.2 == *z) {
                    let edited_one = if edited.contains_key(&first.id) { first } else { second };
                    return Err(ExtractionError::new(
                        GENERATED_FILE_MODIFIED,
                        format!(
                            "{} '{}' (id {}), ZOrder: value {z} collides with '{}' (id {}) under the same parent; ZOrder values must stay unique among siblings",
                            edited_one.type_name,
                            edited_one.name,
                            edited_one.id,
                            if std::ptr::eq(*edited_one, *first) { &second.name } else { &first.name },
                            if std::ptr::eq(*edited_one, *first) { second.id } else { first.id },
                        ),
                    )
                    .with("kind", "invalid-order")
                    .with("property", "ZOrder")
                    .with("id", edited_one.id));
                }
            }
        }
        Ok(())
    }

    /// Patch the original stream. An empty plan returns the exact original bytes. Otherwise only the source
    /// ranges of the changed atoms/nodes are replaced; the result is re-parsed and every requested value is
    /// read back, and the bytes outside the authorized ranges are compared with the original.
    pub fn apply(&self) -> Result<PatchedForm> {
        if self.is_empty() {
            return Ok(PatchedForm { bytes: self.source.emit(), replacements: Vec::new(), derived_pages: Vec::new(), allocated: Vec::new() });
        }
        self.check_z_order_collisions()?;
        let mut replacements: Vec<AppliedReplacement> = Vec::new();
        for (edit, entry) in self.targets() {
            let site = Site { source: self.source, edit, entry };
            for p in control_pieces(&site)? {
                replacements.push(AppliedReplacement { control_id: edit.id, properties: p.properties, range: p.range, bytes: p.bytes });
            }
        }
        let allocated = self.structural_pieces(&mut replacements)?;
        replacements.sort_by_key(|r| (r.range.start, r.range.end));
        let edits: Vec<Replacement> = replacements.iter().map(|r| Replacement { range: r.range, bytes: r.bytes.clone() }).collect();
        let bytes = self.source.apply(&edits)?;
        verify_preserved(self.source.source(), &bytes, &replacements)?;

        let reparsed = parse_spanned(&bytes)?;
        let index = ControlIndex::build(&reparsed)?;
        let structural = !self.diff.additions.is_empty() || !self.diff.deletions.is_empty();
        if !structural {
            let same_layout = index.entries.len() == self.index.entries.len()
                && index.entries.iter().zip(&self.index.entries).all(|(a, b)| (a.id, &a.path, a.page) == (b.id, &b.path, b.page));
            if !same_layout {
                return Err(ExtractionError::new(GENERATED_FILE_MODIFIED, "the patched stream has a different control layout")
                    .with("kind", "verification-failed"));
            }
        } else {
            self.verify_structural(&index, &allocated)?;
        }
        if self.diff.form_props_changed {
            self.verify_form_props(&index)?;
        }
        let mut derived_pages = Vec::new();
        if self.diff.form_size_changed {
            let (Some(new_w), Some(new_h)) = self.diff.new_form_size else {
                return Err(layout_err("form stream: new form size missing"));
            };
            let h1 = index.at_path(&[1])?;
            let got_w = h1.atom(3).and_then(|(a, _)| strict_int(&a.value));
            let got_h = h1.atom(4).and_then(|(a, _)| strict_int(&a.value));
            if (got_w, got_h) != (Some(new_w), Some(new_h)) {
                return Err(ExtractionError::new(GENERATED_FILE_MODIFIED, "patched form size did not read back")
                    .with("kind", "verification-failed"));
            }
            if let Ok(pinfo) = index.at_path(&[1, 2, 1]) {
                let actual = panel_pages(pinfo.node, new_w, new_h);
                derived_pages.extend(crate::form_patch::derived_pages(0, &actual.1));
            }
        }
        for (edit, entry) in self.targets() {
            let patched = index.get(edit.id).expect("layout compared above");
            let original = Site { source: self.source, edit, entry };
            derived_pages.extend(read_back(&Site { source: &reparsed, edit, entry: patched }, &original)?);
        }
        Ok(PatchedForm { bytes, replacements, derived_pages, allocated })
    }

    fn verify_form_props(&self, index: &ControlIndex) -> Result<()> {
        let bad = |m: String| ExtractionError::new(GENERATED_FILE_MODIFIED, m).with("kind", "verification-failed");
        let fp = &self.diff.new_form_props;
        let root = index.at_path(&[])?;
        let h1 = index.at_path(&[1])?;
        for (idx, val) in [
            (5usize, fp.auto_title.map(i64::from)),
            (6, fp.window_state),
            (8, fp.size_change),
            (9, fp.connectable_window.map(i64::from)),
            (10, fp.allow_join_window.map(i64::from)),
            (14, fp.window_appearance_mode),
            (15, fp.window_location),
            (16, fp.change_window_appearance_mode),
            (17, fp.desktop_mode.map(i64::from)),
            (18, fp.allow_close.map(i64::from)),
            (19, fp.enter_key_behavior),
        ] {
            if let Some(v) = val {
                let got = root.atom(idx).and_then(|(a, _)| strict_int(&a.value));
                if got != Some(v) {
                    return Err(bad(format!("form stream: root[{idx}] did not read back as {v}")));
                }
            }
        }
        for (idx, val) in [
            (5usize, fp.allow_normal_state.map(i64::from)),
            (6, fp.allow_docked_state.map(i64::from)),
            (7, fp.allow_autohide_state.map(i64::from)),
            (8, fp.allow_float_state.map(i64::from)),
            (9, fp.window_dock_location),
        ] {
            if let Some(v) = val {
                let got = h1.atom(idx).and_then(|(a, _)| strict_int(&a.value));
                if got != Some(v) {
                    return Err(bad(format!("form stream: root[1][{idx}] did not read back as {v}")));
                }
            }
        }
        if let Some(style) = &fp.style_ref {
            let want = crate::brace::parse_metadata(format_style_node(style).as_bytes()).ok();
            if want.is_none() || Some(index.at_path(&[12])?.node) != want.as_ref() {
                return Err(bad("form stream: root[12] style did not read back".to_string()));
            }
        }
        if let Some(id) = fp.default_control_id {
            let p = index.at_path(&[1, 2, 1, 1])?;
            let got = default_control_slot(p.node)
                .and_then(|i| p.node.children.get(i))
                .and_then(|a| a.as_atom())
                .and_then(|a| strict_int(&a.value));
            if got != Some(id) {
                return Err(bad(format!("form stream: root panel default control did not read back as {id}")));
            }
        }
        if let Some(pic) = &fp.caption_picture {
            let pnode = index.at_path(&[13])?;
            let got = pnode.node.children.get(2).and_then(read_picture_spec);
            if got.as_ref() != Some(pic) {
                return Err(bad("form stream: root[13][2] caption picture did not read back".to_string()));
            }
        }
        Ok(())
    }
}

/// Plan and apply in one step: parse both XML documents, diff them against the stream and patch it.
pub fn patch_form(source: &SpannedMetadata, baseline_xml: &[u8], edited_xml: &[u8]) -> Result<PatchedForm> {
    plan_form_patch(source, baseline_xml, edited_xml)?.apply()
}
