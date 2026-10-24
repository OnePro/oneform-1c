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

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use crate::brace::{parse_spanned, Atom, AtomKind, Item, Node, Replacement, Span, SpanItem, SpanNode, SpannedMetadata, BARE, QUOTED};
use crate::errors::{ExtractionError, Result, GENERATED_FILE_MODIFIED, METADATA_LAYOUT_UNSUPPORTED};
use crate::oof::{control_parts, control_type, geom_page, int, panel_pages, str_val, table, CONTROL_GUIDS};
use crate::oof_props::{app_path, caption_path, localized};

pub const UNKNOWN_CONTROL: &str = "UnknownControl";
/// Tag of the placeholder that stands for a nested control inside a [`FormXml::shell`].
pub const CONTROL_STUB: &str = "#control";
const SUPPORTED_TAGS: [&str; 30] = [
    "Caption",
    "Left",
    "Top",
    "Width",
    "Height",
    "Visible",
    "ZOrder",
    "TabOrder",
    "Enabled",
    "TransparentBack",
    "Tooltip",
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
    "ShowTabs",
    "Footer",
    "SelectionMode",
    "AutoFill",
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

/// Values of the supported property set, with the projection defaults applied
/// (`Visible` true, `TabOrder` 0, empty `Caption` when the element is absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlValues {
    /// `(language, text)` pairs in document order.
    pub caption: Vec<(String, String)>,
    pub left: i64,
    pub top: i64,
    pub width: i64,
    pub height: i64,
    pub visible: bool,
    pub z_order: i64,
    pub tab_order: i64,
    pub enabled: bool,
    pub transparent_back: bool,
    pub tooltip: Vec<(String, String)>,
    pub read_only: bool,
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
    pub show_tabs: Option<i64>,
    pub footer: Option<bool>,
    pub selection_mode: String,
    pub auto_fill: Option<bool>,
}

impl ControlValues {
    fn empty() -> ControlValues {
        ControlValues {
            caption: Vec::new(),
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            visible: true,
            z_order: 0,
            tab_order: 0,
            enabled: true,
            transparent_back: false,
            tooltip: Vec::new(),
            read_only: false,
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
            show_tabs: None,
            footer: None,
            selection_mode: String::new(),
            auto_fill: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Property {
    Caption,
    Left,
    Top,
    Width,
    Height,
    Visible,
    ZOrder,
    TabOrder,
    Enabled,
    TransparentBack,
    Tooltip,
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
    ShowTabs,
    Footer,
    SelectionMode,
    AutoFill,
}

impl Property {
    pub fn xml_name(self) -> &'static str {
        match self {
            Property::Caption => "Caption",
            Property::Left => "Left",
            Property::Top => "Top",
            Property::Width => "Width",
            Property::Height => "Height",
            Property::Visible => "Visible",
            Property::ZOrder => "ZOrder",
            Property::TabOrder => "TabOrder",
            Property::Enabled => "Enabled",
            Property::TransparentBack => "TransparentBack",
            Property::Tooltip => "Tooltip",
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
            Property::ShowTabs => "ShowTabs",
            Property::Footer => "Footer",
            Property::SelectionMode => "SelectionMode",
            Property::AutoFill => "AutoFill",
        }
    }
}

impl ControlValues {
    /// Supported properties whose value differs from `other`.
    pub fn changed_from(&self, other: &ControlValues) -> Vec<Property> {
        [
            (self.caption == other.caption, Property::Caption),
            (self.left == other.left, Property::Left),
            (self.top == other.top, Property::Top),
            (self.width == other.width, Property::Width),
            (self.height == other.height, Property::Height),
            (self.visible == other.visible, Property::Visible),
            (self.z_order == other.z_order, Property::ZOrder),
            (self.tab_order == other.tab_order, Property::TabOrder),
            (self.enabled == other.enabled, Property::Enabled),
            (self.transparent_back == other.transparent_back, Property::TransparentBack),
            (self.tooltip == other.tooltip, Property::Tooltip),
            (self.read_only == other.read_only, Property::ReadOnly),
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
            (self.show_tabs == other.show_tabs, Property::ShowTabs),
            (self.footer == other.footer, Property::Footer),
            (self.selection_mode == other.selection_mode, Property::SelectionMode),
            (self.auto_fill == other.auto_fill, Property::AutoFill),
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

/// Comparable semantic model of one `Form.xml`.
#[derive(Debug, Clone)]
pub struct FormXml {
    /// `baseline` / `edited`, used in messages.
    pub label: String,
    pub name: String,
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
        reorder(&mut out.shell, &z);
        for c in &mut out.controls {
            reorder(&mut c.rest, &z);
            // Write-only InputField flags (verified in binary by read_back, not emitted by render_form_xml
            // so existing golden Form.xml files stay byte-identical) are cleared before XML comparison.
            c.values.drop_list_button = None;
            c.values.spin_button = None;
            c.values.auto_choice_incomplete = None;
            c.values.auto_cell_height = None;
        }
        out
    }
}

fn tag_supported_for(type_name: &str, tag: &str) -> bool {
    match tag {
        "Caption" | "Left" | "Top" | "Width" | "Height" | "Visible" | "ZOrder" | "TabOrder" => true,
        "Enabled" | "TransparentBack" | "Tooltip" => app_path(type_name).is_some(),
        "ReadOnly" | "ChoiceButton" | "ClearButton" | "OpenButton" | "DropListButton" | "SpinButton"
        | "AutoMarkIncomplete" | "AutoChoiceIncomplete" | "MarkNegatives" | "AutoCellHeight" | "ChoiceListMode" => {
            type_name == "InputField"
        }
        "HorizontalAlign" => matches!(type_name, "LabelDecoration" | "CommandBar"),
        "Hyperlink" => type_name == "LabelDecoration",
        "ThreeState" | "TitleLocation" => type_name == "CheckBox",
        "ShowTabs" => type_name == "Panel",
        "Footer" | "SelectionMode" => type_name == "Table",
        "AutoFill" => type_name == "CommandBar",
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
        let mut rest = XmlElement { children: Vec::new(), ..inner.clone() };
        let mut found: HashMap<&str, &XmlElement> = HashMap::new();
        for c in &inner.children {
            if let Some(tag) = SUPPORTED_TAGS.iter().find(|t| **t == c.tag && tag_supported_for(&type_name, t)) {
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
        let is_inp = type_name == "InputField";
        let horizontal_align = match type_name.as_str() {
            "LabelDecoration" => enum_of("HorizontalAlign", &["Left", "Center", "Right"], "Left")?,
            "CommandBar" => enum_of("HorizontalAlign", &["Left", "Right"], "Left")?,
            _ => String::new(),
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
        let values = ControlValues {
            caption: loc_of("Caption")?,
            left: int_of("Left", true)?,
            top: int_of("Top", true)?,
            width: int_of("Width", true)?,
            height: int_of("Height", true)?,
            visible: bool_of("Visible", true)?,
            z_order: int_of("ZOrder", true)?,
            tab_order: int_of("TabOrder", false)?,
            enabled: bool_of("Enabled", true)?,
            transparent_back: bool_of("TransparentBack", false)?,
            tooltip: loc_of("Tooltip")?,
            read_only: bool_of("ReadOnly", false)?,
            choice_button: if is_inp { Some(opt_bool_of("ChoiceButton")?.unwrap_or(false)) } else { None },
            clear_button: if is_inp { Some(opt_bool_of("ClearButton")?.unwrap_or(false)) } else { None },
            open_button: if is_inp { Some(opt_bool_of("OpenButton")?.unwrap_or(false)) } else { None },
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
            title_location: enum_of("TitleLocation", &["Left", "Right"], "Right")?,
            show_tabs,
            footer: if type_name == "Table" { Some(opt_bool_of("Footer")?.unwrap_or(false)) } else { None },
            selection_mode: enum_of("SelectionMode", &["Single", "Multi"], "Single")?,
            auto_fill: if type_name == "CommandBar" { Some(opt_bool_of("AutoFill")?.unwrap_or(true)) } else { None },
        };
        Ok((values, rest))
    }
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
    let root = convert(doc.root_element(), &doc);
    let mut p = Parser { label, form: root.attrs.get("name").cloned().unwrap_or_default(), controls: Vec::new() };
    if root.tag != "Form" {
        return Err(p.bad(root.line, format!("root element must be <Form>, found <{}>", root.tag)));
    }
    let shell = p.walk(&root, None, None)?;
    Ok(FormXml { label: label.to_string(), name: p.form, controls: p.controls, shell })
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
        for c in &xml.controls {
            if let Some(prev) = seen.insert(c.id, c) {
                problems.push(problem(
                    "duplicate-control",
                    format!("line {}: duplicate control id {} ({} and {}); controls cannot be matched uniquely", c.line, c.id, prev.describe(), c.describe()),
                ));
                continue;
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
            if c.name != e.name {
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
}

impl FormDiff {
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.additions.is_empty() && self.deletions.is_empty()
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
    let by = e.attrs.get(attr).or_else(|| e.attrs.get("language"));
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
    for c in &edited.controls {
        if let Some(prev) = seen.insert(c.id, c) {
            problems.push(problem(
                "duplicate-control",
                format!("line {}: duplicate control id {} ({} and {}); controls cannot be matched uniquely", c.line, c.id, prev.describe(), c.describe()),
            ));
            continue;
        }
        let Some(b) = base_ids.get(&c.id) else {
            if structural {
                additions.push(c.clone());
            } else {
                problems.push(problem("added-control", format!("line {}: {} is not in the baseline (adding controls is not supported)", c.line, c.describe())));
            }
            continue;
        };
        if b.name != c.name {
            problems.push(problem("renamed-control", format!("line {}: control id {} renamed '{}' -> '{}' (renaming is not supported)", c.line, c.id, b.name, c.name)));
        }
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
        let changed = c.values.changed_from(&b.values);
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
    if problems.is_empty() {
        Ok(FormDiff { edits, additions, deletions })
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
    let mut out = Vec::new();
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

/// Apply all non-geometry, non-caption `ControlValues` properties to a donor-cloned control node.
fn set_clone_props(n: &mut Node, type_name: &str, v: &ControlValues) -> Result<()> {
    let info = clone_info_mut(n)?;
    if let Some(ap) = app_path(type_name) {
        let path: Vec<usize> = ap.iter().map(|&i| i as usize).collect();
        let app = nav_mut(info, &path)?;
        app.children[1] = bare(i64::from(v.enabled));
        app.children[5] = bare(i64::from(v.transparent_back));
        if app.len() > 12 {
            app.children[12] = Item::Node(localized_node(&v.tooltip));
        }
    }
    match type_name {
        "InputField" => {
            let q = nav_mut(info, &[2, 0])?;
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
            if let Some(footer) = v.footer {
                let cur: i64 = t.children[1]
                    .as_atom()
                    .and_then(|a| a.value.parse().ok())
                    .unwrap_or(0);
                t.children[1] = bare(if footer { cur | 2 } else { cur & !2 });
            }
            t.children[15] = bare(if v.selection_mode == "Multi" { 1 } else { 0 });
        }
        "CommandBar" => {
            let x = nav_mut(info, &[1])?;
            if let Some(af) = v.auto_fill {
                x.children[3] = bare(i64::from(af));
            }
            x.children[4] = bare(if v.horizontal_align == "Right" { 2 } else { 0 });
        }
        _ => {}
    }
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
    let template = crate::donor::event_template()?;
    let template_handler = match template.children.get(2) {
        Some(Item::Node(body)) => body.children.get(1).and_then(|h| h.as_atom()).map(|a| a.value.clone()).unwrap_or_default(),
        _ => String::new(),
    };
    let mut list = Node { children: vec![Item::Atom(Atom::new(BARE, events.len().to_string()))] };
    for (tag, handler) in events {
        let mut ev = crate::donor::clone_node(&template);
        ev.children[0] = bare(*tag);
        fn replace_handler(n: &mut Node, from: &str, to: &str) {
            for c in &mut n.children {
                match c {
                    Item::Atom(a) if a.kind == QUOTED && a.value == from => a.value = to.to_string(),
                    Item::Node(x) => replace_handler(x, from, to),
                    _ => {}
                }
            }
        }
        replace_handler(&mut ev, &template_handler, handler);
        list.children.push(Item::Node(ev));
    }
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

fn info_bool_piece(site: &Site, path: &[usize], idx: usize, old: Option<bool>, new: bool, prop: Property) -> Result<Piece> {
    let (cur, span) = info_int_slot(site, path, idx, prop)?;
    if let Some(expected) = old {
        if (cur != 0) != expected {
            return Err(site.mismatch(prop.xml_name(), &expected.to_string(), &(cur != 0).to_string()));
        }
    }
    Ok(piece(span, i32::from(new), &[prop]))
}

fn info_pieces(site: &Site) -> Result<Vec<Piece>> {
    let (old, new) = (&site.edit.old, &site.edit.new);
    let mut out = Vec::new();
    let app_vec: Option<Vec<usize>> = app_path(site.entry.type_name).map(|p| p.iter().map(|&i| i as usize).collect());
    for &prop in &site.edit.changed {
        match prop {
            Property::Caption
            | Property::Left
            | Property::Top
            | Property::Width
            | Property::Height
            | Property::Visible
            | Property::ZOrder
            | Property::TabOrder => {}
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
                if site.entry.type_name == "CommandBar" {
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
            Property::Hyperlink => {
                out.push(info_bool_piece(site, &[1], 5, Some(old.hyperlink), new.hyperlink, prop)?);
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
            Property::ShowTabs => {
                let layout = read_panel(site)?;
                if Some(layout.show_tabs.0) != old.show_tabs {
                    return Err(site.mismatch("ShowTabs", &format!("{:?}", old.show_tabs), &layout.show_tabs.0.to_string()));
                }
                let want = new.show_tabs.unwrap_or(0);
                out.push(piece(layout.show_tabs.1, in_i32(site, prop, want)?, &[prop]));
            }
            Property::Footer => {
                let (flags, span) = info_int_slot(site, &[2, 1], 1, prop)?;
                let cur = (flags & 2) != 0;
                if Some(cur) != old.footer {
                    return Err(site.mismatch("Footer", &format!("{:?}", old.footer), &cur.to_string()));
                }
                let want = if new.footer.unwrap_or(false) { flags | 2 } else { flags & !2 };
                out.push(piece(span, want, &[prop]));
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
            Property::AutoFill => {
                let want = new.auto_fill.unwrap_or(false);
                out.push(info_bool_piece(site, &[1], 3, old.auto_fill, want, prop)?);
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
    )
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
    let app_vec: Option<Vec<usize>> = app_path(site.entry.type_name).map(|p| p.iter().map(|&i| i as usize).collect());
    for &p in &site.edit.changed {
        let ok = match p {
            Property::Enabled => (info_int_slot(site, app_vec.as_deref().unwrap(), 1, p)?.0 != 0) == new.enabled,
            Property::TransparentBack => (info_int_slot(site, app_vec.as_deref().unwrap(), 5, p)?.0 != 0) == new.transparent_back,
            Property::Tooltip => {
                let tip_node = info_node_at(site, app_vec.as_deref().unwrap(), p)?.child(12).ok_or_else(|| wrong(p))?;
                let items: Vec<(String, String)> = strict_localized(tip_node)
                    .ok_or_else(|| wrong(p))?
                    .into_iter()
                    .map(|i| (i.lang, i.text))
                    .collect();
                items == new.tooltip
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
            Property::HorizontalAlign => {
                if site.entry.type_name == "CommandBar" {
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
            Property::Hyperlink => (info_int_slot(site, &[1], 5, p)?.0 != 0) == new.hyperlink,
            Property::ThreeState => (info_int_slot(site, &[1], 2, p)?.0 != 0) == new.three_state,
            Property::TitleLocation => {
                let cur = if info_int_slot(site, &[1, 0], 3, p)?.0 == 0 { "Left" } else { "" };
                cur == new.title_location
            }
            Property::ShowTabs => Some(read_panel(site)?.show_tabs.0) == new.show_tabs,
            Property::Footer => Some((info_int_slot(site, &[2, 1], 1, p)?.0 & 2) != 0) == new.footer,
            Property::SelectionMode => {
                let cur = if info_int_slot(site, &[2, 1], 15, p)?.0 == 1 { "Multi" } else { "" };
                cur == new.selection_mode
            }
            Property::AutoFill => Some(info_int_slot(site, &[1], 3, p)?.0 != 0) == new.auto_fill,
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

impl FormPatchPlan<'_> {
    /// Structural edits (additions/deletions): produce replacements on the original stream and the
    /// data `verify_structural` needs afterwards. Property pieces are produced separately by the caller.
    fn structural_pieces(&self, out: &mut Vec<AppliedReplacement>) -> Result<Vec<(i64, i64)>> {
        let mut allocated: Vec<(i64, i64)> = Vec::new();
        if self.diff.additions.is_empty() && self.diff.deletions.is_empty() {
            return Ok(allocated);
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
        let mut derived_pages = Vec::new();
        for (edit, entry) in self.targets() {
            let patched = index.get(edit.id).expect("layout compared above");
            let original = Site { source: self.source, edit, entry };
            derived_pages.extend(read_back(&Site { source: &reparsed, edit, entry: patched }, &original)?);
        }
        Ok(PatchedForm { bytes, replacements, derived_pages, allocated })
    }
}

/// Plan and apply in one step: parse both XML documents, diff them against the stream and patch it.
pub fn patch_form(source: &SpannedMetadata, baseline_xml: &[u8], edited_xml: &[u8]) -> Result<PatchedForm> {
    plan_form_patch(source, baseline_xml, edited_xml)?.apply()
}
