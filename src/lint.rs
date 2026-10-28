//! `oneform lint`: layout checks of extracted ordinary forms (`src/Forms/<Form>/Form.xml`, versions 2.1 and 2.2).
//!
//! The checks read only `Form.xml` (the view of the agent) and never write. Every rule is an entry of [`RULES`]: the
//! id, the base severity, the id of the measured rule in `docs/design/design-rules.json` that grades it, and the
//! function that looks for violations. Thresholds come from the compact table `src/lint_table.csv`
//! (`scripts/design/export_lint_table.py`): a rule is a `warning` only when at least [`SHARE_MIN`] % of the forms of
//! the kind follow it and the two configurations (УТ and Бух.) do not disagree; otherwise it is reported as `info`
//! with its `share`. `error` rules (outside the container, duplicate name or id) do not depend on the table.
//!
//! Form kinds are derived like in `scripts/design/analyze_forms.py` (object family + form name + main attribute).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::cli_epf::{EXIT_DIFFERS, EXIT_FAILED, EXIT_OK, EXIT_USAGE};
use crate::cli_oneform::discover_xml_targets;

const TABLE: &str = include_str!("lint_table.csv");
/// Share of forms (percent) from which a measured rule is a warning rather than info.
pub const SHARE_MIN: u32 = 85;
/// Controls closer than this to the far edge of the container "reach" it (the analysis uses 12 px as well).
const REACH: f64 = 12.0;
/// Overlaps and shifts up to this size (px) are ignored.
const SLACK: f64 = 2.0;
/// Rows of the reading order: tops closer than this are one row.
const ROW_TOL: f64 = 6.0;
/// The tab order is reported when more than this many controls are out of place.
const TAB_TOLERANCE: usize = 2;
/// A label may stand this many px higher or lower than its field (the corpora keep 3-5 px in a few forms).
const LABEL_SLACK: f64 = 6.0;
/// The bottom panel may be this many px away from `form height - panel height`.
const BAR_SLACK: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Severity,
    pub line: usize,
    pub control: String,
    pub message: String,
    pub fix: String,
    pub kind: &'static str,
    pub share: Option<u32>,
}

// --------------------------------------------------------------------------- the table of measured rules

struct Row {
    value: Option<f64>,
    share: u32,
    divergent: bool,
    min: Option<f64>,
    max: Option<f64>,
}

fn table() -> &'static HashMap<String, Row> {
    static T: OnceLock<HashMap<String, Row>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = HashMap::new();
        for line in TABLE.lines().skip(1) {
            let c: Vec<&str> = line.split(',').collect();
            if c.len() < 8 {
                continue;
            }
            let num = |s: &str| s.parse::<f64>().ok();
            m.insert(
                format!("{}|{}", c[0], c[1]),
                Row {
                    value: num(c[2]),
                    share: c[3].parse().unwrap_or(0),
                    divergent: c[5] == "1",
                    min: num(c[6]),
                    max: num(c[7]),
                },
            );
        }
        m
    })
}

fn row(id: &str, kind: &str) -> Option<&'static Row> {
    table().get(&format!("{id}|{kind}"))
}

/// The row of a rule that the kind follows clearly: at least `SHARE_MIN` % and no disagreement between the corpora.
fn usual(id: &str, kind: &str) -> Option<&'static Row> {
    row(id, kind).filter(|r| r.share >= SHARE_MIN && !r.divergent)
}

// --------------------------------------------------------------------------- form model

// edge indices; the left edge is 0 (`edge >> 1` is the axis, `edge & !1` the near edge of the axis)
const RIGHT: usize = 1;
const TOP: usize = 2;
const BOTTOM: usize = 3;

struct Anchors {
    /// side of the target an edge is bound to: 0 Left, 1 Right, 2 Top, 3 Bottom, 4 None
    side: [Option<u8>; 4],
    target: [Option<String>; 4],
}

struct Ctl {
    tag: String,
    name: String,
    id: Option<String>,
    line: usize,
    l: Option<f64>,
    t: Option<f64>,
    w: Option<f64>,
    h: Option<f64>,
    tab: Option<f64>,
    z: Option<f64>,
    visible: bool,
    anchors: Option<Anchors>,
    caption: String,
    cont: usize,
}

impl Ctl {
    fn bounds(&self) -> Option<[f64; 4]> {
        Some([self.l?, self.t?, self.w?, self.h?])
    }
}

struct Cont {
    /// the `Page` control that is this container (None: the form)
    page: Option<usize>,
    /// the `Panel` that holds the page
    panel: Option<usize>,
    w: Option<f64>,
    h: Option<f64>,
    /// children of group frames: geometry is not checked
    skip: bool,
    els: Vec<usize>,
}

struct Form {
    kind: &'static str,
    w: Option<f64>,
    h: Option<f64>,
    ctls: Vec<Ctl>,
    conts: Vec<Cont>,
    ids: HashMap<String, usize>,
    stub: bool,
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

fn child_num(n: roxmltree::Node, name: &str) -> Option<f64> {
    child(n, name)?.text()?.trim().parse::<f64>().ok()
}

fn caption_of(n: roxmltree::Node) -> String {
    let Some(c) = child(n, "Caption") else { return String::new() };
    let items: Vec<_> = c.children().filter(|i| i.is_element() && i.tag_name().name() == "Item").collect();
    let pick = items.iter().find(|i| i.attribute("language") == Some("ru")).or(items.first());
    match pick {
        Some(i) => i.text().unwrap_or("").trim().to_string(),
        None => c.text().unwrap_or("").trim().to_string(),
    }
}

fn side_code(v: &str) -> u8 {
    match v {
        "Left" => 0,
        "Right" => 1,
        "Top" => 2,
        "Bottom" => 3,
        _ => 4,
    }
}

fn parse_anchors(n: roxmltree::Node) -> Option<Anchors> {
    let a = child(n, "Anchors")?;
    let names = ["left", "right", "top", "bottom"];
    let mut out = Anchors { side: [None; 4], target: [None, None, None, None] };
    for (i, nm) in names.iter().enumerate() {
        out.side[i] = a.attribute(*nm).map(side_code);
        out.target[i] = a.attribute(format!("{nm}Target").as_str()).map(str::to_string);
    }
    Some(out)
}

fn classify(family: &str, fname: &str, main: &str) -> &'static str {
    match family {
        "Catalogs" => {
            if fname.starts_with("ФормаВыбора") {
                "catalog_choice"
            } else if fname == "ФормаСписка" || main.contains("Список") {
                "catalog_list"
            } else if fname == "ФормаГруппы" {
                "catalog_group"
            } else if fname == "ФормаЭлемента" || main.starts_with("СправочникОбъект") {
                "catalog_item"
            } else {
                "other"
            }
        }
        "Documents" => {
            if fname.starts_with("ФормаВыбора") {
                "document_choice"
            } else if fname == "ФормаСписка" || main.starts_with("ДокументСписок") {
                "document_list"
            } else if fname == "ФормаДокумента" || main.starts_with("ДокументОбъект") {
                "document"
            } else {
                "other"
            }
        }
        "DataProcessors" => "processor",
        "Reports" => "report",
        "CommonForms" => "common_form",
        "InformationRegisters" | "AccumulationRegisters" | "AccountingRegisters" => {
            if main.contains("Список") || fname == "ФормаСписка" {
                "register_list"
            } else {
                "other"
            }
        }
        "DocumentJournals" => "journal_list",
        _ => "other",
    }
}

struct Parser<'a, 'i> {
    doc: &'a roxmltree::Document<'i>,
    ctls: Vec<Ctl>,
    conts: Vec<Cont>,
}

impl Parser<'_, '_> {
    fn collect(&mut self, items: roxmltree::Node, cont: usize, panel: Option<usize>) {
        for x in items.children().filter(|c| c.is_element()) {
            let Some(name) = x.attribute("name") else { continue };
            let tag = x.tag_name().name().to_string();
            if tag == "Column" {
                continue;
            }
            let idx = self.ctls.len();
            self.ctls.push(Ctl {
                line: self.doc.text_pos_at(x.range().start).row as usize,
                name: name.to_string(),
                id: x.attribute("id").map(str::to_string),
                l: child_num(x, "Left"),
                t: child_num(x, "Top"),
                w: child_num(x, "Width"),
                h: child_num(x, "Height"),
                tab: child_num(x, "TabOrder"),
                z: child_num(x, "ZOrder"),
                visible: child(x, "Visible").and_then(|v| v.text()).map(str::trim) != Some("false"),
                anchors: parse_anchors(x),
                caption: caption_of(x),
                cont,
                tag: tag.clone(),
            });
            self.conts[cont].els.push(idx);
            let Some(sub) = child(x, "ChildItems") else { continue };
            match tag.as_str() {
                "Panel" => self.collect(sub, cont, Some(idx)),
                "Page" => {
                    let c = Cont { page: Some(idx), panel, w: self.ctls[idx].w, h: self.ctls[idx].h, skip: false, els: Vec::new() };
                    self.conts.push(c);
                    let n = self.conts.len() - 1;
                    self.collect(sub, n, None);
                }
                "UsualGroup" => {
                    self.conts.push(Cont { page: None, panel: None, w: None, h: None, skip: true, els: Vec::new() });
                    let n = self.conts.len() - 1;
                    self.collect(sub, n, None);
                }
                _ => {}
            }
        }
    }
}

/// `Err(message)` (with the line) when the XML cannot be read.
fn parse_form(text: &str, family: &str, dir_name: &str) -> std::result::Result<Form, (usize, String)> {
    let doc = roxmltree::Document::parse_with_options(text, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .map_err(|e| (e.pos().row as usize, format!("Form.xml is not well-formed XML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "Form" {
        return Err((1, format!("the root element is <{}>, not <Form>", root.tag_name().name())));
    }
    let fname = root.attribute("name").unwrap_or(dir_name);
    let main_id = child(root, "MainAttribute").and_then(|m| m.attribute("attributeId"));
    let main = child(root, "Attributes")
        .and_then(|a| a.children().find(|c| c.is_element() && c.attribute("id") == main_id && main_id.is_some()))
        .and_then(|a| a.attribute("name"))
        .unwrap_or("");
    let mut p = Parser { doc: &doc, ctls: Vec::new(), conts: vec![Cont { page: None, panel: None, w: None, h: None, skip: false, els: Vec::new() }] };
    p.conts[0].w = child_num(root, "Width");
    p.conts[0].h = child_num(root, "Height");
    if let Some(ci) = child(root, "ChildItems") {
        p.collect(ci, 0, None);
    }
    let Parser { ctls, conts, .. } = p;
    let mut ids = HashMap::new();
    for (i, c) in ctls.iter().enumerate() {
        if let Some(id) = &c.id {
            ids.entry(id.clone()).or_insert(i);
        }
    }
    let stub = !ctls
        .iter()
        .any(|c| !matches!(c.tag.as_str(), "CommandBar" | "LabelDecoration" | "PictureDecoration" | "UsualGroup" | "Page"));
    Ok(Form { kind: classify(family, fname, main), w: conts[0].w, h: conts[0].h, ctls, conts, ids, stub })
}

// --------------------------------------------------------------------------- anchors

impl Form {
    /// Whether the edge follows the far side (right / bottom) of the container; port of `edge_ref` of the analysis.
    fn edge_hi(&self, i: usize, edge: usize, depth: u32) -> bool {
        let axis = edge >> 1;
        let lo_edge = axis << 1;
        if depth > 8 {
            return false;
        }
        let ctl = &self.ctls[i];
        let Some(v) = ctl.anchors.as_ref().and_then(|a| a.side[edge]) else {
            return if edge == lo_edge { false } else { self.edge_hi(i, lo_edge, depth + 1) };
        };
        let fallback = v as usize != lo_edge;
        let Some(target) = ctl.anchors.as_ref().and_then(|a| a.target[edge].as_deref()) else { return fallback };
        let ti = if ctl.id.as_deref() == Some(target) { Some(i) } else { self.ids.get(target).copied() };
        match ti {
            Some(ti) if v < 4 && (v as usize >> 1) == axis => self.edge_hi(ti, v as usize, depth + 1),
            _ => fallback,
        }
    }

    fn hi(&self, i: usize, edge: usize) -> bool {
        self.edge_hi(i, edge, 0)
    }

    fn bar(&self, name: &str) -> Option<usize> {
        self.conts[0].els.iter().copied().find(|&i| self.ctls[i].tag == "CommandBar" && self.ctls[i].name == name)
    }

    /// Whether the container follows the size of the window along an axis (the form always does; a page does when
    /// its panel stretches).
    fn container_follows(&self, cont: usize, edge: usize) -> bool {
        match self.conts[cont].panel {
            Some(p) => self.hi(p, edge),
            None => true,
        }
    }

    fn place(&self, cont: usize) -> String {
        match self.conts[cont].page {
            Some(p) => format!("page '{}'", self.ctls[p].name),
            None => "the form".to_string(),
        }
    }
}

// --------------------------------------------------------------------------- reporting

struct Rule {
    id: &'static str,
    severity: Severity,
    /// the measured rule (`docs/design/design-rules.json`) that grades the finding by the share of forms
    table: Option<&'static str>,
}

fn num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

impl Form {
    fn report(&self, id: &str, table: Option<&str>, ctl: Option<usize>, message: String, fix: String, out: &mut Vec<Finding>) {
        let Some(rule) = RULES.iter().find(|r| r.id == id) else { return };
        let (severity, share) = match table.or(rule.table) {
            None => (rule.severity, None),
            Some(id) => match row(id, self.kind) {
                Some(r) if r.share >= SHARE_MIN && !r.divergent => (rule.severity, Some(r.share)),
                Some(r) => (Severity::Info, Some(r.share)),
                None => (Severity::Info, None),
            },
        };
        let mut message = message;
        if severity == Severity::Info {
            match share {
                Some(s) => message.push_str(&format!(" [share={s} %, kind={}]", self.kind)),
                None => message.push_str(&format!(" [kind={}]", self.kind)),
            }
        }
        let (line, control) = match ctl {
            Some(i) => (self.ctls[i].line, self.ctls[i].name.clone()),
            None => (1, "(form)".to_string()),
        };
        out.push(Finding { rule: rule.id, severity, line, control, message, fix, kind: self.kind, share });
    }

    fn visible_bounds(&self, i: usize) -> Option<[f64; 4]> {
        let c = &self.ctls[i];
        if c.visible {
            c.bounds()
        } else {
            None
        }
    }
}

// --------------------------------------------------------------------------- error rules

/// The hint for a control that reaches past the container along one axis: where to move it, or how large the container
/// must be; a size below 1 is never suggested.
fn overflow_fix(pos_name: &str, size_name: &str, pos: f64, size: f64, container: f64) -> String {
    let enlarge = format!("enlarge the container to {size_name} {}", num(pos + size));
    if size <= container {
        format!("move the control to {pos_name} {} or {enlarge}", num(container - size))
    } else if container >= 1.0 {
        format!("reduce {size_name} to {} (at {pos_name} 0) or {enlarge}", num(container))
    } else {
        enlarge
    }
}

fn r_outside(f: &Form, out: &mut Vec<Finding>) {
    for (ci, cont) in f.conts.iter().enumerate() {
        let (Some(cw), Some(ch)) = (cont.w, cont.h) else { continue };
        if cont.skip {
            continue;
        }
        for &i in &cont.els {
            if f.ctls[i].tag == "Page" {
                continue;
            }
            let Some([l, t, w, h]) = f.visible_bounds(i) else { continue };
            let place = f.place(ci);
            if l < 0.0 || t < 0.0 {
                let fix = format!("set Left and Top to 0 or more (now Left {}, Top {})", num(l), num(t));
                f.report("outside-container", None, Some(i), format!("negative coordinates in {place}"), fix, out);
            }
            if l + w > cw {
                let fix = overflow_fix("Left", "Width", l, w, cw);
                f.report("outside-container", None, Some(i), format!("right edge {} is beyond the width {} of {place}", num(l + w), num(cw)), fix, out);
            }
            if t + h > ch {
                let fix = overflow_fix("Top", "Height", t, h, ch);
                f.report("outside-container", None, Some(i), format!("bottom edge {} is beyond the height {} of {place}", num(t + h), num(ch)), fix, out);
            }
        }
    }
}

fn r_duplicates(f: &Form, out: &mut Vec<Finding>) {
    let mut names: HashMap<&str, usize> = HashMap::new();
    let mut ids: HashMap<&str, usize> = HashMap::new();
    // page names live apart from control names (the same page name is used by several panels)
    for (i, c) in f.ctls.iter().enumerate().filter(|(_, c)| c.tag != "Page") {
        if let Some(&first) = names.get(c.name.as_str()) {
            let message = format!("name '{}' is already used by the control at line {}", c.name, f.ctls[first].line);
            f.report("duplicate-name", None, Some(i), message, "give the control a unique name".to_string(), out);
        } else {
            names.insert(c.name.as_str(), i);
        }
        let Some(id) = c.id.as_deref() else { continue };
        if let Some(&first) = ids.get(id) {
            let message = format!("id {id} is already used by '{}' at line {}", f.ctls[first].name, f.ctls[first].line);
            f.report("duplicate-id", None, Some(i), message, "give the control an id that is free in the form".to_string(), out);
        } else {
            ids.insert(id, i);
        }
    }
}

// --------------------------------------------------------------------------- warnings

/// Controls whose overlap matters: the simple ones (fields, labels with text, buttons, tables, ...).
fn solid(f: &Form, i: usize) -> Option<[f64; 4]> {
    let c = &f.ctls[i];
    if matches!(c.tag.as_str(), "Panel" | "Page" | "UsualGroup" | "CommandBar" | "Splitter" | "PictureDecoration") {
        return None;
    }
    if c.tag == "LabelDecoration" && c.caption.is_empty() {
        return None;
    }
    f.visible_bounds(i).filter(|b| b[2] > 0.0 && b[3] > 0.0)
}

/// Alternative controls laid over each other (shown one at a time by the module) or one inside the other.
fn stacked(a: &[f64; 4], b: &[f64; 4]) -> bool {
    let inside = |x: &[f64; 4], y: &[f64; 4]| {
        x[0] >= y[0] - SLACK && x[1] >= y[1] - SLACK && x[0] + x[2] <= y[0] + y[2] + SLACK && x[1] + x[3] <= y[1] + y[3] + SLACK
    };
    inside(a, b) || inside(b, a)
}

fn r_overlap(f: &Form, out: &mut Vec<Finding>) {
    for cont in f.conts.iter().filter(|c| !c.skip) {
        let cand: Vec<(usize, [f64; 4])> = cont.els.iter().filter_map(|&i| solid(f, i).map(|b| (i, b))).collect();
        for (n, &(b, bb)) in cand.iter().enumerate() {
            for &(a, ab) in &cand[..n] {
                let dx = (ab[0] + ab[2]).min(bb[0] + bb[2]) - ab[0].max(bb[0]);
                let dy = (ab[1] + ab[3]).min(bb[1] + bb[3]) - ab[1].max(bb[1]);
                if dx > SLACK && dy > SLACK && !stacked(&ab, &bb) {
                    let message = format!("overlaps '{}' (line {}) by {} x {} px", f.ctls[a].name, f.ctls[a].line, num(dx), num(dy));
                    let fix = "move or resize one of the two controls so that they do not intersect".to_string();
                    f.report("overlap", None, Some(b), message, fix, out);
                }
            }
        }
    }
}

fn r_no_stretch_right(f: &Form, out: &mut Vec<Finding>) {
    for (ci, cont) in f.conts.iter().enumerate().filter(|(_, c)| !c.skip) {
        let Some(cw) = cont.w else { continue };
        if !f.container_follows(ci, RIGHT) {
            continue;
        }
        let stretches = |j: usize| {
            matches!(f.ctls[j].tag.as_str(), "InputField" | "ChoiceField" | "Table") && f.ctls[j].visible && f.hi(j, RIGHT)
        };
        for &i in &cont.els {
            let id = match f.ctls[i].tag.as_str() {
                "InputField" | "ChoiceField" => "field-no-stretch",
                "Table" => "table-no-stretch",
                _ => continue,
            };
            let Some([l, _, w, _]) = f.visible_bounds(i) else { continue };
            let gap = cw - (l + w);
            if !(0.0..=REACH).contains(&gap) || f.hi(i, RIGHT) {
                continue;
            }
            // a field is the odd one out only when other fields or tables of the container do stretch; a fixed dialog is not a defect
            if id == "field-no-stretch" && !cont.els.iter().any(|&j| j != i && stretches(j)) {
                continue;
            }
            let message = format!(
                "{} ends {} px from the right edge of {} but its right side is not anchored, so it does not follow the window width",
                f.ctls[i].tag,
                num(gap),
                f.place(ci)
            );
            let fix = format!("<Anchors right=\"Right\" rightOffset=\"-{}\"/> (the other edges stay as they are)", num(gap));
            f.report(id, None, Some(i), message, fix, out);
        }
    }
}

fn r_no_stretch_bottom(f: &Form, out: &mut Vec<Finding>) {
    let bar_top = f.bar("ОсновныеДействияФормы").and_then(|b| f.visible_bounds(b)).map(|b| b[1]);
    for (ci, cont) in f.conts.iter().enumerate().filter(|(_, c)| !c.skip) {
        let Some(ch) = cont.h else { continue };
        if !f.container_follows(ci, BOTTOM) {
            continue;
        }
        let limit = match (ci, bar_top) {
            (0, Some(t)) => t,
            _ => ch,
        };
        for &i in cont.els.iter().filter(|&&i| f.ctls[i].tag == "Table") {
            let Some([_, t, _, h]) = f.visible_bounds(i) else { continue };
            let gap = limit - (t + h);
            if !(0.0..=REACH).contains(&gap) || f.hi(i, BOTTOM) {
                continue;
            }
            let what = if ci == 0 && bar_top.is_some() { "the bottom panel" } else { "the bottom edge" };
            let message = format!("Table ends {} px above {what} but its bottom side is not anchored, so it does not stretch with the window", num(gap));
            let fix = format!("<Anchors bottom=\"Bottom\" bottomOffset=\"{}\"/> (offset from the bottom of the container)", num(t + h - ch));
            f.report("table-no-stretch-bottom", None, Some(i), message, fix, out);
        }
    }
}

fn r_bottom_bar(f: &Form, out: &mut Vec<Finding>) {
    let (Some(b), Some(fw), Some(fh)) = (f.bar("ОсновныеДействияФормы"), f.w, f.h) else { return };
    let Some([l, t, w, h]) = f.ctls[b].bounds() else { return };
    let want = fh - h;
    if (t - want).abs() > BAR_SLACK {
        let message = format!("the bottom panel is at Top {} but the form height {} puts it at {}", num(t), num(fh), num(want));
        f.report("bottom-bar-position", None, Some(b), message, format!("set Top to {} (form Height - panel Height)", num(want)), out);
    }
    if l != 0.0 || w != fw {
        let message = format!("the bottom panel spans Left {} Width {}, the form is {} wide", num(l), num(w), num(fw));
        f.report("bottom-bar-width", None, Some(b), message, format!("set Left to 0 and Width to {}", num(fw)), out);
    }
    if !f.hi(b, TOP) || !f.hi(b, RIGHT) {
        let id = f.ctls[b].id.clone().unwrap_or_default();
        let message = "the bottom panel is not anchored to the bottom and right edges, so it stays behind when the window is resized".to_string();
        let fix = format!("<Anchors top=\"Bottom\" topTarget=\"{id}\" topOffset=\"-{}\" bottom=\"Bottom\" left=\"Left\" right=\"Right\"/>", num(h));
        f.report("bottom-bar-anchor", None, Some(b), message, fix, out);
    }
}

fn r_covers_bar(f: &Form, out: &mut Vec<Finding>) {
    let Some(b) = f.bar("ОсновныеДействияФормы") else { return };
    let Some([bl, bt, bw, _]) = f.visible_bounds(b) else { return };
    for &i in &f.conts[0].els {
        if matches!(f.ctls[i].tag.as_str(), "CommandBar" | "Page") {
            continue;
        }
        let Some([l, t, w, h]) = f.visible_bounds(i) else { continue };
        let across = (l + w).min(bl + bw) - l.max(bl);
        if t < bt && t + h > bt + SLACK && across > SLACK {
            let message = format!("{} reaches {} and goes under the bottom panel (Top {})", f.ctls[i].tag, num(t + h), num(bt));
            f.report("covers-bottom-bar", None, Some(i), message, format!("reduce Height to {} or move the panel down", num(bt - t)), out);
        }
    }
}

fn r_top_bar(f: &Form, out: &mut Vec<Finding>) {
    let (Some(b), Some(fw)) = (f.bar("ДействияФормы"), f.w) else { return };
    let Some([l, t, w, _]) = f.ctls[b].bounds() else { return };
    if t != 0.0 {
        f.report("top-bar-position", None, Some(b), format!("the top panel is at Top {}", num(t)), "set Top to 0".to_string(), out);
    }
    if l != 0.0 || w != fw {
        let message = format!("the top panel spans Left {} Width {}, the form is {} wide", num(l), num(w), num(fw));
        f.report("top-bar-width", None, Some(b), message, format!("set Left to 0 and Width to {}", num(fw)), out);
    }
}

/// Which elements are outside a longest increasing subsequence of `seq`.
fn out_of_place(seq: &[usize]) -> Vec<bool> {
    let n = seq.len();
    let mut len = vec![1usize; n];
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        for j in 0..i {
            if seq[j] < seq[i] && len[j] + 1 > len[i] {
                len[i] = len[j] + 1;
                prev[i] = j;
            }
        }
    }
    let mut keep = vec![false; n];
    let mut cur = (0..n).max_by_key(|&i| (len[i], std::cmp::Reverse(i))).unwrap_or(usize::MAX);
    while cur != usize::MAX {
        keep[cur] = true;
        cur = prev[cur];
    }
    keep.iter().map(|k| !k).collect()
}

fn r_tab_order(f: &Form, out: &mut Vec<Finding>) {
    for (ci, cont) in f.conts.iter().enumerate().filter(|(_, c)| !c.skip) {
        let els: Vec<usize> = cont
            .els
            .iter()
            .copied()
            .filter(|&i| {
                let c = &f.ctls[i];
                c.tag != "Page" && c.visible && c.tab.is_some() && c.t.is_some() && c.l.is_some()
            })
            .collect();
        let m = els.len();
        if m < 4 {
            continue;
        }
        let t = |k: usize| f.ctls[els[k]].t.unwrap_or(0.0);
        let l = |k: usize| f.ctls[els[k]].l.unwrap_or(0.0);
        let tab = |k: usize| f.ctls[els[k]].tab.unwrap_or(0.0);
        let z = |k: usize| f.ctls[els[k]].z.unwrap_or(0.0);
        let mut by_top: Vec<usize> = (0..m).collect();
        by_top.sort_by(|&a, &b| t(a).total_cmp(&t(b)).then(l(a).total_cmp(&l(b))));
        let mut row_of = vec![0usize; m];
        let (mut row, mut start) = (0usize, t(by_top[0]));
        for &k in &by_top {
            if t(k) - start > ROW_TOL {
                row += 1;
                start = t(k);
            }
            row_of[k] = row;
        }
        let mut reading: Vec<usize> = (0..m).collect();
        reading.sort_by(|&a, &b| row_of[a].cmp(&row_of[b]).then(l(a).total_cmp(&l(b))).then(tab(a).total_cmp(&tab(b))));
        let mut pos = vec![0usize; m];
        for (p, &k) in reading.iter().enumerate() {
            pos[k] = p;
        }
        let mut by_tab: Vec<usize> = (0..m).collect();
        by_tab.sort_by(|&a, &b| tab(a).total_cmp(&tab(b)).then(z(a).total_cmp(&z(b))).then(a.cmp(&b)));
        let seq: Vec<usize> = by_tab.iter().map(|&k| pos[k]).collect();
        let bad = out_of_place(&seq);
        let count = bad.iter().filter(|b| **b).count();
        if count <= TAB_TOLERANCE {
            continue;
        }
        let first = by_tab[bad.iter().position(|b| *b).unwrap_or(0)];
        let table = if ci == 0 { "tab.form.reading_order_exact" } else { "tab.page.reading_order_exact" };
        let message = format!(
            "TabOrder does not follow the reading order (top to bottom, left to right) in {}: {count} of {m} controls are out of place, the first is TabOrder {}",
            f.place(ci),
            num(tab(first))
        );
        let fix = "renumber TabOrder 1..N in reading order, keeping a label and its field next to each other".to_string();
        f.report("tab-order", Some(table), Some(els[first]), message, fix, out);
    }
}

fn label_of(f: &Form, field: usize) -> Option<usize> {
    let want = format!("Надпись{}", f.ctls[field].name);
    f.conts[f.ctls[field].cont]
        .els
        .iter()
        .copied()
        .find(|&i| f.ctls[i].tag == "LabelDecoration" && f.ctls[i].name == want && f.visible_bounds(i).is_some())
}

fn is_field(c: &Ctl) -> bool {
    matches!(c.tag.as_str(), "InputField" | "ChoiceField")
}

fn r_label_row(f: &Form, out: &mut Vec<Finding>) {
    for (fi, fc) in f.ctls.iter().enumerate().filter(|(_, c)| is_field(c)) {
        let (Some([fl, ft, _, fh]), Some(li)) = (f.visible_bounds(fi), label_of(f, fi)) else { continue };
        let Some([ll, lt, lw, lh]) = f.visible_bounds(li) else { continue };
        let beside = ll + lw <= fl + SLACK && lt < ft + fh && ft < lt + lh;
        if beside && (lt - ft).abs() > LABEL_SLACK {
            let message = format!("the label stands at Top {} but its field '{}' at Top {}", num(lt), fc.name, num(ft));
            f.report("label-row", None, Some(li), message, format!("set Top to {}", num(ft)), out);
        }
    }
}

// --------------------------------------------------------------------------- info: deviations from the usual values

/// The measured rule of an entry of [`RULES`].
fn rule_table(id: &str) -> &'static str {
    RULES.iter().find(|r| r.id == id).and_then(|r| r.table).unwrap_or("")
}

fn r_margins(f: &Form, out: &mut Vec<Finding>) {
    let (Some(fw), Some(fh)) = (f.w, f.h) else { return };
    let body: Vec<(usize, [f64; 4])> = f.conts[0]
        .els
        .iter()
        .filter_map(|&i| {
            let c = &f.ctls[i];
            if c.tag == "CommandBar" || c.tag == "Page" {
                return None;
            }
            c.bounds().filter(|b| b[2] > 0.0 && b[3] > 0.0).map(|b| (i, b))
        })
        .collect();
    if f.stub || body.is_empty() {
        return;
    }
    let top_edge = f.bar("ДействияФормы").and_then(|b| f.ctls[b].bounds()).map(|b| b[1] + b[3]).unwrap_or(0.0);
    let bottom_top = f.bar("ОсновныеДействияФормы").and_then(|b| f.ctls[b].bounds()).map(|b| b[1]).unwrap_or(fh);
    // (control, value) with the smallest / largest value of `key`
    let extreme = |items: &[(usize, [f64; 4])], key: &dyn Fn(&[f64; 4]) -> f64, max: bool| -> Option<(usize, f64)> {
        let mut it = items.iter().map(|(i, b)| (*i, key(b)));
        let first = it.next()?;
        Some(it.fold(first, |best, x| if (max && x.1 > best.1) || (!max && x.1 < best.1) { x } else { best }))
    };
    let mut check = |rule: &str, what: &str, found: Option<(usize, f64)>, fix: &str| {
        let (Some((ctl, value)), Some(want)) = (found, usual(rule_table(rule), f.kind).and_then(|u| u.value)) else { return };
        if value >= 0.0 && value != want {
            f.report(rule, None, Some(ctl), format!("{what} is {} px, usual {}", num(value), num(want)), fix.to_string(), out);
        }
    };
    check("margin-left", "left margin", extreme(&body, &|b| b[0], false), "shift the leftmost controls to the usual margin");
    check(
        "margin-right",
        "right margin",
        extreme(&body, &|b| b[0] + b[2], true).map(|(i, v)| (i, fw - v)),
        "resize or move the rightmost controls to the usual margin",
    );
    check(
        "margin-top",
        "gap under the top panel",
        extreme(&body, &|b| b[1], false).map(|(i, v)| (i, v - top_edge)),
        "move the first row to the usual gap",
    );
    let above: Vec<(usize, [f64; 4])> = body.iter().filter(|(_, b)| b[1] < bottom_top).cloned().collect();
    check(
        "margin-bottom",
        "gap above the bottom panel",
        extreme(&above, &|b| b[1] + b[3], true).map(|(i, v)| (i, bottom_top - v)),
        "keep the usual gap between the last row and the bottom panel",
    );
}

fn r_bar_height(f: &Form, out: &mut Vec<Finding>) {
    for (name, rule) in [("ДействияФормы", "top-bar-height"), ("ОсновныеДействияФормы", "bottom-bar-height")] {
        let (Some(b), Some(u)) = (f.bar(name), usual(rule_table(rule), f.kind).and_then(|u| u.value)) else { continue };
        if let Some(h) = f.ctls[b].h.filter(|h| *h != u) {
            f.report(rule, None, Some(b), format!("panel '{name}' is {} px high, usual {}", num(h), num(u)), format!("set Height to {}", num(u)), out);
        }
    }
}

fn r_field_heights(f: &Form, out: &mut Vec<Finding>) {
    let field_h = usual(rule_table("field-height"), f.kind).and_then(|u| u.value);
    let label_h = usual(rule_table("label-height"), f.kind).and_then(|u| u.value);
    for (i, c) in f.ctls.iter().enumerate().filter(|(_, c)| is_field(c)) {
        if let (Some(u), Some(h)) = (field_h, c.h) {
            if h != u {
                f.report("field-height", None, Some(i), format!("field height is {} px, usual {}", num(h), num(u)), format!("set Height to {}", num(u)), out);
            }
        }
        if let (Some(u), Some(li)) = (label_h, label_of(f, i)) {
            if let Some(h) = f.ctls[li].h.filter(|h| *h != u) {
                f.report("label-height", None, Some(li), format!("label height is {} px, usual {}", num(h), num(u)), format!("set Height to {}", num(u)), out);
            }
        }
    }
}

fn r_form_size(f: &Form, out: &mut Vec<Finding>) {
    if f.stub {
        return;
    }
    for (rule, what, v) in [("form-width", "width", f.w), ("form-height", "height", f.h)] {
        let (Some(row), Some(v)) = (row(rule_table(rule), f.kind), v) else { continue };
        let (Some(lo), Some(hi)) = (row.min, row.max) else { continue };
        if v < lo || v > hi {
            let message = format!("form {what} {} is outside {}..{} seen in all measured forms of the kind", num(v), num(lo), num(hi));
            f.report(rule, None, None, message, format!("check that the form needs this {what}; most forms of the kind are inside the range"), out);
        }
    }
}

// --------------------------------------------------------------------------- the rule list

/// Every finding id with its base severity and the measured rule that grades it. A measured warning is demoted to
/// `info` (with its share) when the table says fewer than `SHARE_MIN` % of the forms of the kind follow it, or the
/// corpora disagree; the info rules are reported only for kinds whose usual value is that clear.
static RULES: [Rule; 26] = [
    Rule { id: "invalid-xml", severity: Severity::Error, table: None },
    Rule { id: "outside-container", severity: Severity::Error, table: None },
    Rule { id: "duplicate-name", severity: Severity::Error, table: None },
    Rule { id: "duplicate-id", severity: Severity::Error, table: None },
    Rule { id: "overlap", severity: Severity::Warning, table: None },
    Rule { id: "covers-bottom-bar", severity: Severity::Warning, table: None },
    Rule { id: "field-no-stretch", severity: Severity::Warning, table: Some("anchors.input_stretch_when_at_right_margin") },
    Rule { id: "table-no-stretch", severity: Severity::Warning, table: Some("anchors.table_stretch_both") },
    Rule { id: "table-no-stretch-bottom", severity: Severity::Warning, table: Some("anchors.table_stretch_both") },
    Rule { id: "bottom-bar-position", severity: Severity::Warning, table: Some("bar.bottom.top_eq_h_minus_height") },
    Rule { id: "bottom-bar-width", severity: Severity::Warning, table: Some("bar.bottom.left0_full_width") },
    Rule { id: "bottom-bar-anchor", severity: Severity::Warning, table: Some("bar.bottom.anchor_class") },
    Rule { id: "top-bar-position", severity: Severity::Warning, table: Some("bar.top.top0") },
    Rule { id: "top-bar-width", severity: Severity::Warning, table: Some("bar.top.left0_full_width") },
    Rule { id: "tab-order", severity: Severity::Info, table: Some("tab.form.reading_order_exact") },
    Rule { id: "label-row", severity: Severity::Warning, table: Some("pair.top_delta") },
    Rule { id: "margin-left", severity: Severity::Info, table: Some("margin.left") },
    Rule { id: "margin-right", severity: Severity::Info, table: Some("margin.right") },
    Rule { id: "margin-top", severity: Severity::Info, table: Some("margin.top_gap") },
    Rule { id: "margin-bottom", severity: Severity::Info, table: Some("margin.bottom_gap") },
    Rule { id: "top-bar-height", severity: Severity::Info, table: Some("bar.top.height") },
    Rule { id: "bottom-bar-height", severity: Severity::Info, table: Some("bar.bottom.height") },
    Rule { id: "field-height", severity: Severity::Info, table: Some("field.height") },
    Rule { id: "label-height", severity: Severity::Info, table: Some("pair.label_height") },
    Rule { id: "form-width", severity: Severity::Info, table: Some("form.width") },
    Rule { id: "form-height", severity: Severity::Info, table: Some("form.height") },
];

/// The checks; each one reports findings of the rules above.
static CHECKS: [fn(&Form, &mut Vec<Finding>); 14] = [
    r_outside,
    r_duplicates,
    r_overlap,
    r_no_stretch_right,
    r_no_stretch_bottom,
    r_bottom_bar,
    r_covers_bar,
    r_top_bar,
    r_tab_order,
    r_label_row,
    r_margins,
    r_bar_height,
    r_field_heights,
    r_form_size,
];

// --------------------------------------------------------------------------- one form, a tree of forms

/// Findings of one `Form.xml` text. `family` is the object family directory (`Catalogs`, `Documents`, ...).
pub fn lint_text(text: &str, family: &str, dir_name: &str) -> (&'static str, Vec<Finding>) {
    let text = text.trim_start_matches('\u{feff}');
    match parse_form(text, family, dir_name) {
        Ok(form) => {
            let mut out = Vec::new();
            for check in CHECKS {
                check(&form, &mut out);
            }
            out.sort_by(|a, b| a.line.cmp(&b.line).then(a.severity.cmp(&b.severity)).then(a.rule.cmp(b.rule)));
            (form.kind, out)
        }
        Err((line, message)) => {
            let finding = Finding {
                rule: "invalid-xml",
                severity: Severity::Error,
                line: line.max(1),
                control: "(form)".to_string(),
                message,
                fix: "run oneform extract again or repair the XML".to_string(),
                kind: "other",
                share: None,
            };
            ("other", vec![finding])
        }
    }
}

struct FormReport {
    /// `<object>/<form>`
    label: String,
    /// path of Form.xml relative to the linted root
    file: String,
    kind: &'static str,
    findings: Vec<Finding>,
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The object family of the directory that holds `src`: its parent directory for configuration objects; for the dump of
/// an external report/processor the root tag of `<object>.xml` next to the object directory decides.
fn family_of(obj_root: &Path) -> String {
    let parent_name = obj_root.parent().map(name_of).unwrap_or_default();
    if let Some(parent) = obj_root.parent() {
        let sibling = parent.join(format!("{}.xml", name_of(obj_root)));
        if let Ok(bytes) = fs::read(&sibling) {
            let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).into_owned();
            if head.contains("<ExternalReport") {
                return "Reports".to_string();
            }
            if head.contains("<ExternalDataProcessor") {
                return "DataProcessors".to_string();
            }
        }
    }
    let has_epf = fs::read_dir(obj_root)
        .map(|rd| rd.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("epf"))))
        .unwrap_or(false);
    if has_epf {
        "DataProcessors".to_string()
    } else {
        parent_name
    }
}

fn lint_file(root: &Path, rel: &str) -> Option<FormReport> {
    let path: PathBuf = rel.split('/').fold(root.to_path_buf(), |p, s| p.join(s));
    let form_dir = path.parent()?;
    let forms_dir = form_dir.parent()?;
    let src_root = if name_of(forms_dir) == "Forms" { forms_dir.parent()? } else { form_dir };
    let obj_root = if name_of(src_root) == "src" { src_root.parent()? } else { src_root };
    let object = match obj_root.strip_prefix(root) {
        Ok(r) if !r.as_os_str().is_empty() => r.to_string_lossy().replace('\\', "/"),
        _ => format!("{}/{}", obj_root.parent().map(name_of).unwrap_or_default(), name_of(obj_root)),
    };
    let dir_name = name_of(form_dir);
    let bytes = fs::read(&path).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let (kind, findings) = lint_text(&text, &family_of(obj_root), &dir_name);
    Some(FormReport { label: format!("{object}/{dir_name}"), file: rel.to_string(), kind, findings })
}

fn finding_line(label: &str, f: &Finding) -> String {
    let fix = if f.fix.is_empty() { String::new() } else { format!(" (fix: {})", f.fix) };
    format!("{label}: {} {} line {}: {}: {}{fix}", f.severity.as_str(), f.rule, f.line, f.control, f.message)
}

/// `oneform lint [PATH] [--json] [--strict]`: the exit code is 1 when an error is found, 3 with `--strict` when
/// there are warnings, otherwise 0.
pub fn run_cli(root: &Path, json_output: bool, strict: bool) -> i32 {
    if !root.is_dir() {
        eprintln!("oneform: error: {} is not a directory", root.display());
        return EXIT_USAGE;
    }
    let mut rels: Vec<String> = discover_xml_targets(root).into_iter().map(|t| t.rel).collect();
    rels.sort();
    let reports: Vec<FormReport> = rels.iter().filter_map(|r| lint_file(root, r)).collect();
    if reports.is_empty() {
        eprintln!("oneform: error: nothing to process under {}", root.display());
        return EXIT_USAGE;
    }
    let count = |s: Severity| reports.iter().flat_map(|r| &r.findings).filter(|f| f.severity == s).count();
    let (errors, warnings, infos) = (count(Severity::Error), count(Severity::Warning), count(Severity::Info));
    let code = if errors > 0 {
        EXIT_FAILED
    } else if strict && warnings > 0 {
        EXIT_DIFFERS
    } else {
        EXIT_OK
    };
    if json_output {
        let items: Vec<Value> = reports
            .iter()
            .flat_map(|r| {
                r.findings.iter().map(move |f| {
                    json!({
                        "form": r.label,
                        "file": r.file,
                        "kind": r.kind,
                        "severity": f.severity.as_str(),
                        "rule": f.rule,
                        "line": f.line,
                        "control": f.control,
                        "message": f.message,
                        "fix": f.fix,
                        "share": f.share,
                    })
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&Value::Array(items)).unwrap_or_default());
        return code;
    }
    for r in &reports {
        for f in &r.findings {
            println!("{}", finding_line(&r.label, f));
        }
    }
    let with = reports.iter().filter(|r| !r.findings.is_empty()).count();
    println!(
        "Lint: {} forms, {} with findings; {errors} errors, {warnings} warnings, {infos} info; exit {code}",
        reports.len(),
        with
    );
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_follow_the_analysis() {
        assert_eq!(classify("Catalogs", "ФормаЭлемента", "СправочникОбъект"), "catalog_item");
        assert_eq!(classify("Catalogs", "Другая", "СправочникСписок"), "catalog_list");
        assert_eq!(classify("Catalogs", "ФормаВыбораГруппы", "СправочникСписок"), "catalog_choice");
        assert_eq!(classify("Documents", "Х", "ДокументОбъект"), "document");
        assert_eq!(classify("Documents", "ФормаСписка", "Х"), "document_list");
        assert_eq!(classify("DataProcessors", "Форма", ""), "processor");
        assert_eq!(classify("Reports", "Форма", ""), "report");
        assert_eq!(classify("Enums", "Форма", ""), "other");
    }

    #[test]
    fn the_table_is_embedded_and_graded() {
        let r = row("margin.left", "catalog_item").unwrap();
        assert_eq!((r.value, r.share, r.divergent), (Some(8.0), 98, false));
        assert!(usual("margin.left", "catalog_item").is_some());
        // the corpora disagree on the spacing of rows, and the table has no such rule: nothing is imposed
        assert!(usual("margin.bottom_gap", "catalog_item").is_none());
        assert!(table().len() > 150);
    }

    #[test]
    fn longest_increasing_subsequence_marks_the_stragglers() {
        assert_eq!(out_of_place(&[0, 1, 2, 3]), vec![false; 4]);
        assert_eq!(out_of_place(&[3, 0, 1, 2]), vec![true, false, false, false]);
        assert_eq!(out_of_place(&[3, 2, 1, 0]).iter().filter(|b| **b).count(), 3);
    }
}
