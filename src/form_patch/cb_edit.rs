//! Adding, removing and reordering the buttons of a `CommandBar` (`<Buttons>` of `Form.xml`).
//!
//! The panel record holds a table `X[7] = {5,T1,T2,1,nAct,Action×nAct,nGroups,Group×nGroups}`: actions and
//! buttons pair 1:1, the root group lists `(action ref, Button)` pairs and ends with a tail
//! `{pos,nRefs,(T1,id)×nRefs,{nSub,(T1,id,flag)×nSub}}` (`docs/design/command-bar-advisor-report.md`). When the set or
//! the order of the root buttons changes the whole table is rebuilt as ONE replacement: kept records are copied from
//! their source text (with the in-place property edits of [`patch_cb_button`] applied inside them), new records are
//! built from the shape of the panel's own siblings; `Standard:N` commands come from the templates of
//! [`crate::cb_standard`]. Creating sub-menus and pictures are not supported and are refused with the line of the
//! `<Button>`.

use std::collections::{HashMap, HashSet};

use regex::Regex;
use uuid::Uuid;

use super::*;
use crate::cb_standard::{StdTemplate, CODES, TEMPLATES};
use crate::oof_props::{NULL_GROUP_GUID, STANDARD_ACTION_GUID, STD_SUBMENU_ACTION_GUID, SUBMENU_ACTION_GUID};

const HANDLER_KIND: &str = crate::oof_props::EVENT_GUID;
const SUBMENU_KIND: &str = SUBMENU_ACTION_GUID;

/// A record to be written: an atom (verbatim text), a list, or a verbatim node copied from the source.
enum Tx {
    At(String),
    Nd(Vec<Tx>),
    Raw(String),
}

fn at(s: impl ToString) -> Tx {
    Tx::At(s.to_string())
}

/// 1C layout: a child node starts a new line, the closing brace follows a node child on a new line.
fn render(t: &Tx, nl: &str, out: &mut String) {
    match t {
        Tx::At(s) | Tx::Raw(s) => out.push_str(s),
        Tx::Nd(items) => {
            out.push('{');
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if !matches!(it, Tx::At(_)) {
                    out.push_str(nl);
                }
                render(it, nl, out);
            }
            if items.last().is_some_and(|l| !matches!(l, Tx::At(_))) {
                out.push_str(nl);
            }
            out.push('}');
        }
    }
}

fn loc(items: &[(String, String)]) -> Tx {
    let mut v = vec![at(1), at(items.len())];
    v.extend(items.iter().map(|(l, t)| Tx::Nd(vec![at(quote(l)), at(quote(t))])));
    Tx::Nd(v)
}

fn int_at(n: NodeRef, i: usize) -> Option<i64> {
    strict_int(&n.atom(i)?.0.value)
}

struct Act<'a> {
    guid: String,
    node: NodeRef<'a>,
    /// Sub-menu group `(guid, code)` of a sub-menu action; `None` for every other action (separators included).
    group: Option<(String, String)>,
    /// A platform sub-menu (`c93a51ed`): kept and moved, never created, changed or removed.
    platform: bool,
}

/// The decoded shape of a button table.
struct Model<'a> {
    t: NodeRef<'a>,
    t1: String,
    t2: i64,
    actions: Vec<Act<'a>>,
    groups: Vec<NodeRef<'a>>,
    root: usize,
    x_version: String,
    align: i64,
}

fn shape(site: &Site, why: impl Into<String>) -> ExtractionError {
    site.layout("unexpected-shape", "Buttons", why.into())
}

fn model<'a>(site: &Site<'_, 'a>, prop: Property) -> Result<Model<'a>> {
    let (t, n_act, root_g) = cb_table(site, prop)?;
    let x = info_node_at(site, &[1], prop)?;
    let t1 = t.atom(1).ok_or_else(|| shape(site, "table guid missing"))?.0.value.clone();
    let t2 = int_at(t, 2).ok_or_else(|| shape(site, "id counter missing"))?;
    let mut actions = Vec::with_capacity(n_act);
    for ai in 0..n_act {
        let node = t.child(5 + ai).ok_or_else(|| shape(site, "action record missing"))?;
        let guid = node.atom(1).ok_or_else(|| shape(site, "action guid missing"))?.0.value.clone();
        let kind = node.atom(3).map(|(a, _)| a.value.as_str()).unwrap_or_default();
        let mut group = None;
        let platform = kind == STD_SUBMENU_ACTION_GUID;
        if kind == SUBMENU_ACTION_GUID || kind == STD_SUBMENU_ACTION_GUID {
            let body = node.child(4).ok_or_else(|| shape(site, "action body missing"))?;
            let (g, c) = (body.atom(1), body.atom(2));
            if let (Some((g, _)), Some((c, _))) = (g, c) {
                if !(g.value.eq_ignore_ascii_case(NULL_GROUP_GUID) && c.value == "0") {
                    group = Some((g.value.clone(), c.value.clone()));
                }
            }
        }
        actions.push(Act { guid, node, group, platform });
    }
    let n_groups: usize = t
        .atom(5 + n_act)
        .and_then(|(a, _)| a.value.parse().ok())
        .ok_or_else(|| shape(site, "group count missing"))?;
    let groups: Vec<NodeRef<'a>> = (0..n_groups).filter_map(|gi| t.child(6 + n_act + gi)).collect();
    if groups.len() != n_groups {
        return Err(shape(site, "button group missing"));
    }
    let root = groups
        .iter()
        .position(|g| g.span.span == root_g.span.span)
        .ok_or_else(|| shape(site, "root button group missing"))?;
    let x_version = x.atom(1).map(|(a, _)| a.value.clone()).unwrap_or_default();
    let align = int_at(x, 4).unwrap_or(0);
    Ok(Model { t, t1, t2, actions, groups, root, x_version, align })
}

impl Model<'_> {
    /// The group with the key `(guid, code)`; at most one may exist.
    fn group_at(&self, site: &Site, key: &(String, String)) -> Result<Option<usize>> {
        let hits: Vec<usize> = (0..self.groups.len())
            .filter(|&gi| cb_group_key(self.groups[gi]) == Some((key.0.as_str(), key.1.as_str())))
            .collect();
        match hits.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            _ => Err(shape(site, format!("the sub-menu group ({}, {}) is listed twice", key.0, key.1))),
        }
    }
}

fn group_k(site: &Site, g: NodeRef) -> Result<usize> {
    let k = int_at(g, 4).and_then(|k| usize::try_from(k).ok()).ok_or_else(|| shape(site, "button count of a group missing"))?;
    if g.node.len() != 6 + 2 * k {
        return Err(shape(site, "button group has an unexpected number of items"));
    }
    Ok(k)
}

/// The invariants of a button table; `stage` is the error kind (`unexpected-shape` before, `verification-failed` after
/// the rebuild).
fn check_table(site: &Site, m: &Model, stage: &'static str) -> Result<()> {
    let bad = |why: String| site.layout(stage, "Buttons", why);
    if m.t.node.len() != 6 + m.actions.len() + m.groups.len() {
        return Err(bad("the button table has an unexpected number of items".into()));
    }
    let mut guids: HashSet<&str> = HashSet::new();
    for a in &m.actions {
        if !guids.insert(a.guid.as_str()) {
            return Err(bad(format!("the action {} is listed twice", a.guid)));
        }
    }
    let act_versions: HashSet<&str> = m.actions.iter().filter_map(|a| a.node.atom(0).map(|(v, _)| v.value.as_str())).collect();
    if act_versions.len() > 1 {
        return Err(bad("the actions of the panel have different record versions".into()));
    }
    let mut referenced: HashSet<String> = HashSet::new();
    let mut ids: HashSet<i64> = HashSet::new();
    let mut shapes: HashSet<(String, usize)> = HashSet::new();
    let mut max_id = 0;
    for (gi, g) in m.groups.iter().enumerate() {
        let k = group_k(site, *g).map_err(|e| bad(e.message))?;
        for i in 0..k {
            let guid = g.atom(5 + 2 * i).ok_or_else(|| bad("action reference missing".into()))?.0.value.clone();
            if !guids.contains(guid.as_str()) {
                return Err(bad(format!("a button refers to the unknown action {guid}")));
            }
            if !referenced.insert(guid.clone()) {
                return Err(bad(format!("the action {guid} is used by two buttons")));
            }
            let b = g.child(6 + 2 * i).ok_or_else(|| bad("button record missing".into()))?;
            if b.atom(6).map(|(a, _)| a.value.as_str()) != Some(m.t1.as_str()) {
                return Err(bad("a button does not carry the table guid".into()));
            }
            let id = int_at(b, 7).ok_or_else(|| bad("button id missing".into()))?;
            if !ids.insert(id) {
                return Err(bad(format!("the button id {id} is used twice")));
            }
            max_id = max_id.max(id);
            shapes.insert((b.atom(0).map(|(a, _)| a.value.clone()).unwrap_or_default(), b.node.len()));
        }
        if let Some(code) = int_at(*g, 2).filter(|_| g.atom(1).map(|(a, _)| a.value.as_str()) == Some(m.t1.as_str())) {
            max_id = max_id.max(code);
        }
        if gi == m.root {
            check_tail(site, *g, k, stage)?;
        }
    }
    if shapes.len() > 1 {
        return Err(bad("the buttons of the panel have different record versions".into()));
    }
    if referenced.len() != m.actions.len() {
        return Err(bad("an action is used by no button".into()));
    }
    if m.t2 < max_id {
        return Err(bad(format!("the id counter {} is below the largest id {max_id}", m.t2)));
    }
    Ok(())
}

/// The tail of a group: `{pos,nRefs,(T1,id)×nRefs,{nSub,(T1,id,flag)×nSub}}`; the entries of the sub-menu list are
/// in button order.
fn check_tail(site: &Site, root: NodeRef, k: usize, stage: &'static str) -> Result<()> {
    let bad = |why: &str| site.layout(stage, "Buttons", format!("tail of the button group: {why}"));
    let tail = root.child(5 + 2 * k).ok_or_else(|| bad("missing"))?;
    let (pos, refs) = (int_at(tail, 0).ok_or_else(|| bad("position missing"))?, int_at(tail, 1).ok_or_else(|| bad("reference count missing"))?);
    let refs = usize::try_from(refs).map_err(|_| bad("negative reference count"))?;
    if tail.node.len() != 3 + 2 * refs {
        return Err(bad("unexpected number of items"));
    }
    if pos < -1 || pos > k as i64 {
        return Err(bad("position is outside -1..=k"));
    }
    let sub = tail.child(2 + 2 * refs).ok_or_else(|| bad("sub-menu list missing"))?;
    let n = int_at(sub, 0).and_then(|n| usize::try_from(n).ok()).ok_or_else(|| bad("sub-menu count missing"))?;
    if sub.node.len() != 1 + 3 * n {
        return Err(bad("sub-menu list has an unexpected number of items"));
    }
    let ids: HashSet<i64> = (0..k).filter_map(|i| root.child(6 + 2 * i).and_then(|b| int_at(b, 7))).collect();
    for r in 0..refs {
        if !int_at(tail, 3 + 2 * r).is_some_and(|id| ids.contains(&id)) {
            return Err(bad("a reference names a button that is not in the group"));
        }
    }
    let order: Vec<i64> = (0..k).filter_map(|i| root.child(6 + 2 * i).and_then(|b| int_at(b, 7))).collect();
    let mut last = None;
    for s in 0..n {
        let at = int_at(sub, 2 + 3 * s).and_then(|id| order.iter().position(|o| *o == id));
        let Some(at) = at else {
            return Err(bad("a sub-menu entry names a button that is not in the group"));
        };
        if last.is_some_and(|l| l >= at) {
            return Err(bad("the sub-menu entries are not in button order"));
        }
        last = Some(at);
    }
    Ok(())
}

fn collect(btns: &[CommandBarButton], ids: &mut HashSet<i64>, names: &mut Vec<String>) {
    for b in btns {
        ids.extend(b.id);
        names.push(b.name.clone());
        collect(&b.buttons, ids, names);
    }
}

/// True when the set, the order or the nesting of the buttons differs at any level: a missing id marks a new button.
pub(super) fn is_structural(old: &[CommandBarButton], new: &[CommandBarButton]) -> bool {
    old.len() != new.len() || old.iter().zip(new).any(|(o, n)| n.id.is_none() || n.id != o.id || is_structural(&o.buttons, &n.buttons))
}

/// The requested buttons as the patched stream decodes them: new buttons get the ids `first_id`, `first_id + 1`, ...
/// in document order, depth first (a new sub-menu takes its button id, then its group code, then the ids of its buttons).
pub(super) fn resolve(old: &[CommandBarButton], new: &[CommandBarButton], first_id: i64) -> Vec<CommandBarButton> {
    fn go(list: &[CommandBarButton], known: &HashSet<i64>, next: &mut i64) -> Vec<CommandBarButton> {
        list.iter()
            .map(|b| {
                let mut b = b.clone();
                if b.id.is_none_or(|id| !known.contains(&id)) {
                    b.id = Some(*next);
                    *next += 1;
                    if b.kind == "Submenu" {
                        *next += 1;
                        if b.action_kind.is_empty() {
                            b.action_kind = "Submenu".to_string();
                        }
                    } else if b.kind == "Action" {
                        if let Ok(Some(code)) = standard_code(&b) {
                            b.action_kind = "Standard".to_string();
                            if b.caption.iter().all(|(_, t)| t.is_empty()) {
                                b.caption = std_caption(code).map(|c| vec![("ru".to_string(), c.to_string())]).unwrap_or_default();
                            }
                        } else if b.action_kind.is_empty() {
                            b.action_kind = "Handler".to_string();
                        }
                    }
                }
                b.buttons = go(&b.buttons, known, next);
                b
            })
            .collect()
    }
    let mut known = HashSet::new();
    collect(old, &mut known, &mut Vec::new());
    go(new, &known, &mut first_id.clone())
}

/// The resolved buttons of a structural `Buttons` edit; `None` for an edit that keeps the set and the order.
pub(super) fn resolved_for(original: &Site) -> Result<Option<Vec<CommandBarButton>>> {
    let edit = original.edit;
    if !edit.changed.contains(&Property::Buttons) || !is_structural(&edit.old.buttons, &edit.new.buttons) {
        return Ok(None);
    }
    let m = model(original, Property::Buttons)?;
    Ok(Some(resolve(&edit.old.buttons, &edit.new.buttons, m.t2 + 1)))
}

fn refuse(site: &Site, b: &CommandBarButton, why: &str) -> ExtractionError {
    site.invalid("unsupported-button", "Buttons", format!("line {}: button '{}': {why}", b.line.0, b.name))
}

/// The code of a `Standard:N` button: `Ok(None)` for a handler, `Err` for a malformed standard command.
fn standard_code(b: &CommandBarButton) -> std::result::Result<Option<u32>, &'static str> {
    match (b.action.strip_prefix("Standard:"), b.action_kind.as_str()) {
        (Some(n), "" | "Standard") => n.parse().map(Some).map_err(|_| "the action must be Standard:N with a number N"),
        (Some(_), _) => Err("action=\"Standard:N\" needs actionKind=\"Standard\" or none"),
        (None, "Standard") => Err("a standard command needs action=\"Standard:N\""),
        (None, _) => Ok(None),
    }
}

/// The template of `code` for a form class: the class row, else the row of any form.
fn std_template(code: u32, class: &str) -> Option<&'static StdTemplate> {
    TEMPLATES
        .iter()
        .find(|t| t.code == code && t.class == class)
        .or_else(|| TEMPLATES.iter().find(|t| t.code == code && t.class.is_empty()))
}

/// The default caption of a new standard command: the most frequent Russian text of its buttons.
fn std_caption(code: u32) -> Option<&'static str> {
    CODES.iter().find(|c| c.code == code).map(|c| c.caption).filter(|c| !c.is_empty())
}

fn supported_codes() -> String {
    CODES.iter().map(|c| c.code.to_string()).collect::<Vec<_>>().join(", ")
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// What a new button may hold; everything else is refused.
fn validate_new(site: &Site, b: &CommandBarButton) -> Result<()> {
    if !is_identifier(&b.name) {
        return Err(refuse(site, b, "the name must be an identifier (letters, digits, '_'; no leading digit)"));
    }
    if b.kind != "Submenu" && !b.buttons.is_empty() {
        return Err(refuse(site, b, "only a sub-menu holds buttons"));
    }
    if b.picture.is_some()
        || b.shortcut.is_some()
        || !b.description.is_empty()
        || b.default_button.is_some()
        || b.button_order.is_some()
        || b.has_picture
    {
        return Err(refuse(
            site,
            b,
            "a new button can set only Caption, Tooltip, checked and representation; picture, shortcut, description, DefaultButton and ButtonOrder are not supported yet",
        ));
    }
    let lang_ok = |items: &[(String, String)]| items.iter().all(|(l, _)| !l.is_empty());
    if !lang_ok(&b.caption) || !lang_ok(&b.tooltip) {
        return Err(refuse(site, b, "a localized <Item> has an empty language"));
    }
    match b.kind.as_str() {
        "Submenu" => {
            if !b.action.is_empty() || !(b.action_kind.is_empty() || b.action_kind == "Submenu") {
                return Err(refuse(
                    site,
                    b,
                    "only a user sub-menu (no action) can be created; platform sub-menus (StdSubmenu) are kept or moved, not created",
                ));
            }
            if !b.tooltip.is_empty() {
                return Err(refuse(site, b, "a sub-menu holds no tooltip"));
            }
            if b.caption.iter().all(|(_, t)| t.is_empty()) {
                return Err(refuse(site, b, "a new sub-menu needs a <Caption>"));
            }
        }
        "Separator" => {
            if !b.action.is_empty() || !b.action_kind.is_empty() || !b.caption.is_empty() || !b.tooltip.is_empty() {
                return Err(refuse(site, b, "a new separator holds no action, caption or tooltip"));
            }
        }
        "Action" => {
            if let Some(code) = standard_code(b).map_err(|why| refuse(site, b, why))? {
                if std_template(code, site.edit.form_class).is_none() {
                    return Err(refuse(
                        site,
                        b,
                        &format!("no template for Standard:{code}; supported codes: {}", supported_codes()),
                    ));
                }
                if !b.tooltip.is_empty() {
                    return Err(refuse(site, b, "a new standard command takes its tooltip from the platform; remove <Tooltip>"));
                }
                return Ok(());
            }
            if !b.action_kind.is_empty() && b.action_kind != "Handler" {
                return Err(refuse(
                    site,
                    b,
                    "only handler buttons (actionKind=\"Handler\"), Standard:N commands, separators and user sub-menus can be created",
                ));
            }
            if b.action.is_empty() {
                return Err(refuse(site, b, "a new button needs action=\"ProcedureName\""));
            }
            if !is_identifier(&b.action) {
                return Err(refuse(site, b, "the action must be a procedure name (letters, digits, '_'; no leading digit)"));
            }
            if b.caption.iter().all(|(_, t)| t.is_empty()) {
                return Err(refuse(site, b, "a new button needs a <Caption>"));
            }
        }
        other => return Err(refuse(site, b, &format!("kind '{other}' cannot be created"))),
    }
    Ok(())
}

/// Record versions of the panel: copied from its own records, the donor formats for an empty panel.
struct Shape {
    x9: bool,
    act_version: String,
    btn_version: String,
    btn_len: usize,
    trailer: String,
}

fn panel_shape(m: &Model) -> Shape {
    let x9 = m.x_version != "8";
    let first_act = m.actions.first();
    let act_version = first_act
        .and_then(|a| a.node.atom(0))
        .map(|(a, _)| a.value.clone())
        .unwrap_or_else(|| if x9 { "7" } else { "6" }.to_string());
    let first_btn = m.groups.iter().find_map(|g| if group_k_unchecked(*g) > 0 { g.child(6) } else { None });
    let btn_version = first_btn.and_then(|b| b.atom(0)).map(|(a, _)| a.value.clone()).unwrap_or_else(|| "8".to_string());
    let btn_len = first_btn.map_or(16, |b| b.node.len());
    // the last atom of a version-7 action is not decoded; follow the panel, else the majority of the corpus
    let mut counts: Vec<(String, usize)> = Vec::new();
    for a in &m.actions {
        if let Some((v, _)) = a.node.atom(a.node.node.len().saturating_sub(1)) {
            match counts.iter_mut().find(|(t, _)| *t == v.value) {
                Some(c) => c.1 += 1,
                None => counts.push((v.value.clone(), 1)),
            }
        }
    }
    let trailer = counts
        .iter()
        .rev()
        .max_by_key(|c| c.1)
        .map(|c| c.0.clone())
        .unwrap_or_else(|| if m.align == 2 { "1" } else { "2" }.to_string());
    Shape { x9, act_version, btn_version, btn_len, trailer }
}

fn group_k_unchecked(g: NodeRef) -> usize {
    int_at(g, 4).and_then(|k| usize::try_from(k).ok()).unwrap_or(0)
}

fn picture_node(x9: bool) -> Tx {
    if x9 {
        Tx::Nd(vec![at(4), at(0), Tx::Nd(vec![at(0)]), at("\"\""), at(-1), at(-1), at(1), at(0), at("\"\"")])
    } else {
        Tx::Nd(vec![at(3), at(0), Tx::Nd(vec![at(0)]), at("\"\""), at(-1), at(-1), at(1), at(0)])
    }
}

/// `group` is `(T1, code)` of the new group of a user sub-menu.
fn action_record(sh: &Shape, guid: &str, b: &CommandBarButton, std: Option<(u32, &StdTemplate)>, group: Option<(&str, i64)>) -> Tx {
    let mut v = vec![at(&sh.act_version), at(guid), at(1)];
    if let Some((code, t)) = std {
        let [src, ty, kind, grp, gcode, flag] = t.body;
        v.push(at(STANDARD_ACTION_GUID));
        v.push(Tx::Nd(vec![
            at(6),
            at(src),
            at(ty),
            at(code),
            Tx::Nd(vec![at(1), at(kind), at(grp), at(gcode), at(flag)]),
            at(0),
            at(1),
        ]));
    } else if let Some((t1, code)) = group {
        v.push(at(SUBMENU_KIND));
        v.push(Tx::Nd(vec![at(1), at(t1), at(code)]));
    } else if b.kind == "Separator" {
        v.push(at(SUBMENU_KIND));
        v.push(Tx::Nd(vec![at(1), at(NULL_GROUP_GUID), at(0)]));
    } else {
        let meta = Tx::Nd(vec![
            at(1),
            at(quote(&b.action)),
            loc(&b.caption),
            loc(&b.tooltip),
            loc(&[]),
            picture_node(sh.x9),
            Tx::Nd(vec![at(0), at(0), at(0)]),
        ]);
        v.push(at(HANDLER_KIND));
        v.push(Tx::Nd(vec![at(3), at(quote(&b.action)), meta]));
    }
    v.push(at(0));
    if sh.act_version == "7" {
        v.push(at(&sh.trailer));
    }
    Tx::Nd(v)
}

fn button_record(sh: &Shape, t1: &str, id: i64, b: &CommandBarButton, std: Option<(u32, &StdTemplate)>) -> Tx {
    let repr = match b.representation.as_str() {
        "Text" => 1,
        "Picture" => 2,
        "PictureAndText" => 3,
        _ => 0,
    };
    let separator = b.kind == "Separator";
    // b2, b3, b5, b8, b9, b11: the winning values of the template of a standard command
    let [b2, b3, b5, b8, b9, b11] = match std {
        Some((_, t)) => t.button,
        None if b.kind == "Submenu" => ["0", "1", "0", "1e2", "1", "1"],
        None => ["0", "1", if separator { "0" } else { "1" }, "1e2", if separator { "2" } else { "0" }, "1"],
    };
    let caption = match std {
        Some((code, _)) if b.caption.iter().all(|(_, t)| t.is_empty()) => {
            std_caption(code).map(|c| vec![("ru".to_string(), c.to_string())]).unwrap_or_default()
        }
        _ => b.caption.clone(),
    };
    let mut v = vec![
        at(&sh.btn_version),
        at(quote(&b.name)),
        at(b2),
        at(b3),
        loc(&caption),
        at(b5),
        at(t1),
        at(id),
        at(b8),
        at(b9),
        at(repr),
        at(b11),
        at(i32::from(b.checked)),
        at(0),
        at(0),
        at(0),
    ];
    v.truncate(sh.btn_len);
    Tx::Nd(v)
}

/// Source text of `span` with the in-place `pieces` that lie inside it applied.
fn emit(site: &Site, span: Span, pieces: &[Piece], used: &mut [bool]) -> Result<String> {
    let src = site.source.slice(span).ok_or_else(|| shape(site, "record is outside the stream"))?;
    let mut inside: Vec<usize> =
        (0..pieces.len()).filter(|&i| pieces[i].range.start >= span.start && pieces[i].range.end <= span.end).collect();
    inside.sort_by_key(|&i| (pieces[i].range.start, pieces[i].range.end));
    let mut out = Vec::with_capacity(src.len());
    let mut at = span.start;
    for i in inside {
        let p = &pieces[i];
        if p.range.start < at {
            return Err(shape(site, "two edits of one button overlap"));
        }
        out.extend_from_slice(&src[at - span.start..p.range.start - span.start]);
        out.extend_from_slice(&p.bytes);
        at = p.range.end;
        used[i] = true;
    }
    out.extend_from_slice(&src[at - span.start..]);
    String::from_utf8(out).map_err(|_| shape(site, "record is not valid UTF-8"))
}

fn raw_atom(site: &Site, n: NodeRef, i: usize) -> Result<String> {
    let (_, span) = n.atom(i).ok_or_else(|| shape(site, "atom missing"))?;
    Ok(String::from_utf8_lossy(site.source.slice(span).unwrap_or_default()).into_owned())
}

/// A button of the stream, bound to its place in the button tree of the baseline `Form.xml`.
struct Bound<'a, 'o> {
    id: i64,
    group: usize,
    index: usize,
    guid: String,
    node: NodeRef<'a>,
    act: usize,
    /// The existing group of a sub-menu button.
    sub: Option<usize>,
    /// Another button shows the same group first, so its buttons are not bound again here.
    dup: bool,
    /// A platform sub-menu, or one whose group several buttons show: its content cannot change.
    frozen: bool,
    old: &'o CommandBarButton,
}

fn bind<'a, 'o>(
    site: &Site,
    m: &Model<'a>,
    gi: usize,
    old: &'o [CommandBarButton],
    depth: usize,
    reached: &mut HashSet<usize>,
    out: &mut Vec<Bound<'a, 'o>>,
) -> Result<()> {
    reached.insert(gi);
    let g = m.groups[gi];
    let k = group_k(site, g)?;
    if old.len() != k {
        return Err(site.mismatch("Buttons", &format!("{} buttons in a group", old.len()), &format!("{k} buttons")));
    }
    for (i, ob) in old.iter().enumerate() {
        let node = g.child(6 + 2 * i).ok_or_else(|| shape(site, "button node missing"))?;
        let guid = g.atom(5 + 2 * i).ok_or_else(|| shape(site, "action reference missing"))?.0.value.clone();
        let id = int_at(node, 7).ok_or_else(|| shape(site, "button id missing"))?;
        if ob.id != Some(id) {
            return Err(site.mismatch("Buttons", &format!("button '{}' id {:?}", ob.name, ob.id), &format!("id {id}")));
        }
        let act = m.actions.iter().position(|a| a.guid == guid).ok_or_else(|| shape(site, "unknown action"))?;
        let sub = match &m.actions[act].group {
            Some(key) if ob.kind == "Submenu" => m.group_at(site, key)?,
            _ => None,
        };
        let dup = sub.is_some_and(|g| reached.contains(&g));
        out.push(Bound { id, group: gi, index: i, guid, node, act, sub, dup, frozen: false, old: ob });
        if !ob.buttons.is_empty() {
            let Some(sub_g) = sub else { return Err(shape(site, "sub-menu button group missing")) };
            if depth >= CB_MAX_DEPTH {
                return Err(shape(site, "sub-menus are nested deeper than three levels"));
            }
            if !dup {
                bind(site, m, sub_g, &ob.buttons, depth + 1, reached, out)?;
            }
        }
    }
    Ok(())
}

/// What the new tree does with one button: keep the stream record (moved or not) or create one.
enum Plan<'r> {
    Keep { bound: usize, kids: Vec<Plan<'r>> },
    New { rb: &'r CommandBarButton, kids: Vec<Plan<'r>> },
}

#[allow(clippy::too_many_arguments)]
fn classify<'r>(
    site: &Site,
    bound: &[Bound],
    by_id: &HashMap<i64, usize>,
    list: &'r [CommandBarButton],
    depth: usize,
    seen: &mut HashSet<i64>,
    fresh: &mut Vec<&'r CommandBarButton>,
) -> Result<Vec<Plan<'r>>> {
    let mut out = Vec::with_capacity(list.len());
    for nb in list {
        let known = nb.id.and_then(|id| by_id.get(&id).copied());
        if let Some(bi) = known {
            let id = nb.id.unwrap_or_default();
            if !seen.insert(id) {
                return Err(refuse(site, nb, &format!("the id {id} is used twice")));
            }
            let bd = &bound[bi];
            if bd.old.kind != nb.kind {
                return Err(refuse(site, nb, "the kind of an existing button cannot change"));
            }
            if nb.kind != "Submenu" && !nb.buttons.is_empty() {
                return Err(refuse(site, nb, "only a sub-menu holds buttons"));
            }
            let mut kids = Vec::new();
            if nb.kind == "Submenu" {
                if bd.frozen {
                    let was: Vec<Option<i64>> = bd.old.buttons.iter().map(|b| b.id).collect();
                    let now: Vec<Option<i64>> = nb.buttons.iter().map(|b| b.id).collect();
                    if was != now {
                        return Err(refuse(site, nb, "the buttons of a platform sub-menu, or of one that several buttons show, cannot be added, removed or moved"));
                    }
                } else if !nb.buttons.is_empty() && bd.sub.is_none() {
                    return Err(refuse(site, nb, "this sub-menu has no button group"));
                }
                if !nb.buttons.is_empty() {
                    if depth >= CB_MAX_DEPTH {
                        return Err(refuse(site, nb, "sub-menus are nested deeper than three levels"));
                    }
                    if !bd.dup {
                        kids = classify(site, bound, by_id, &nb.buttons, depth + 1, seen, fresh)?;
                    }
                }
            }
            out.push(Plan::Keep { bound: bi, kids });
        } else {
            validate_new(site, nb)?;
            fresh.push(nb);
            let mut kids = Vec::new();
            if !nb.buttons.is_empty() {
                if depth >= CB_MAX_DEPTH {
                    return Err(refuse(site, nb, "sub-menus are nested deeper than three levels"));
                }
                kids = classify(site, bound, by_id, &nb.buttons, depth + 1, seen, fresh)?;
            }
            out.push(Plan::New { rb: nb, kids });
        }
    }
    Ok(out)
}

/// The tail of an existing group, read leniently.
struct TailInfo {
    pos: i64,
    refs: Vec<(i64, [String; 2])>,
    subs: Vec<(i64, [String; 3])>,
}

fn read_tail(site: &Site, g: NodeRef) -> Result<TailInfo> {
    let k = group_k(site, g)?;
    let tail = g.child(5 + 2 * k).ok_or_else(|| shape(site, "tail missing"))?;
    let pos = int_at(tail, 0).ok_or_else(|| shape(site, "tail position missing"))?;
    let n_refs = int_at(tail, 1).and_then(|n| usize::try_from(n).ok()).ok_or_else(|| shape(site, "tail reference count missing"))?;
    let mut refs = Vec::with_capacity(n_refs);
    for r in 0..n_refs {
        let id = int_at(tail, 3 + 2 * r).ok_or_else(|| shape(site, "tail reference id missing"))?;
        refs.push((id, [raw_atom(site, tail, 2 + 2 * r)?, raw_atom(site, tail, 3 + 2 * r)?]));
    }
    let sub = tail.child(2 + 2 * n_refs).ok_or_else(|| shape(site, "sub-menu list missing"))?;
    let n_sub = int_at(sub, 0).and_then(|n| usize::try_from(n).ok()).ok_or_else(|| shape(site, "sub-menu count missing"))?;
    let mut subs = Vec::with_capacity(n_sub);
    for s in 0..n_sub {
        let id = int_at(sub, 2 + 3 * s).ok_or_else(|| shape(site, "sub-menu entry id missing"))?;
        subs.push((id, [raw_atom(site, sub, 1 + 3 * s)?, raw_atom(site, sub, 2 + 3 * s)?, raw_atom(site, sub, 3 + 3 * s)?]));
    }
    Ok(TailInfo { pos, refs, subs })
}

/// One button of a group to be written.
struct Row {
    id: i64,
    guid: String,
    rec: Tx,
    /// Index in the same group of the stream; `None` for a new or a moved button.
    old_idx: Option<usize>,
    new_sub: bool,
}

/// Sub-menu entries and platform references of every group: they follow their button when it moves.
struct Lists {
    refs: HashMap<i64, [String; 2]>,
    subs: HashMap<i64, [String; 3]>,
}

/// `{pos,nRefs,(T1,id)×nRefs,{nSub,(T1,id,flag)×nSub}}` of a rewritten group: the position stays anchored to the
/// button it sat on (a removed button hands it to the next survivor), references keep their order, the sub-menu list
/// holds the sub-menu buttons that were listed (and the new ones) in button order.
fn make_tail(site: &Site, old: Option<(&TailInfo, usize)>, rows: &[Row], t1: &str, lists: &Lists) -> Result<Tx> {
    let k_new = rows.len();
    let pos = match old {
        None => -1,
        Some((t, _)) if t.pos == -1 => -1,
        Some((t, k_old)) if t.pos == k_old as i64 => k_new as i64,
        Some((t, k_old)) => {
            let from = usize::try_from(t.pos)
                .ok()
                .filter(|f| *f < k_old)
                .ok_or_else(|| shape(site, "tail position is outside the group"))?;
            // the first surviving button at or after the anchor, in the old order
            let anchor = rows.iter().enumerate().filter_map(|(j, r)| r.old_idx.filter(|i| *i >= from).map(|i| (i, j))).min_by_key(|(i, _)| *i);
            anchor.map_or(k_new, |(_, j)| j) as i64
        }
    };
    let mut refs: Vec<&[String; 2]> = Vec::new();
    let mut listed: HashSet<i64> = HashSet::new();
    if let Some((t, _)) = old {
        for (id, raw) in &t.refs {
            if rows.iter().any(|r| r.id == *id) && listed.insert(*id) {
                refs.push(raw);
            }
        }
    }
    for r in rows {
        if let Some(raw) = lists.refs.get(&r.id) {
            if listed.insert(r.id) {
                refs.push(raw);
            }
        }
    }
    let mut items = vec![at(pos), at(refs.len())];
    for raw in refs {
        items.extend(raw.iter().map(at));
    }
    let mut subs: Vec<Tx> = Vec::new();
    let mut n_sub = 0;
    for r in rows {
        if let Some(entry) = lists.subs.get(&r.id) {
            subs.extend(entry.iter().map(at));
            n_sub += 1;
        } else if r.new_sub {
            subs.extend([at(t1), at(r.id), at(0)]);
            n_sub += 1;
        }
    }
    let mut sub_list = vec![at(n_sub)];
    sub_list.extend(subs);
    items.push(Tx::Nd(sub_list));
    Ok(Tx::Nd(items))
}

#[derive(Clone, Copy)]
enum Target {
    Existing(usize),
    New(i64),
}

/// The new records and the pieces shared by the passes of [`patch_structural`].
struct Build<'c, 's, 'a, 'o> {
    site: &'c Site<'s, 'a>,
    m: &'c Model<'a>,
    bound: &'c [Bound<'a, 'o>],
    pieces: &'c [Piece],
    used: Vec<bool>,
    shp: Shape,
    new_guid: HashMap<i64, String>,
    new_actions: Vec<Tx>,
    new_groups: Vec<Tx>,
    rebuilt: HashMap<usize, Tx>,
    lists: Lists,
}

impl Build<'_, '_, '_, '_> {
    fn std_of(&self, b: &CommandBarButton) -> Option<(u32, &'static StdTemplate)> {
        standard_code(b).ok().flatten().and_then(|c| std_template(c, self.site.edit.form_class).map(|t| (c, t)))
    }

    /// Creates the action records of the new buttons, depth first (the order of the allocated ids).
    fn register(&mut self, plans: &[Plan]) {
        let source_lc = String::from_utf8_lossy(self.site.source.source()).to_lowercase();
        let ns = Uuid::parse_str(&self.m.t1).unwrap_or(Uuid::NAMESPACE_OID);
        self.register_in(plans, &source_lc, ns);
    }

    fn register_in(&mut self, plans: &[Plan], source_lc: &str, ns: Uuid) {
        for p in plans {
            match p {
                Plan::Keep { kids, .. } => self.register_in(kids, source_lc, ns),
                Plan::New { rb, kids } => {
                    let id = rb.id.unwrap_or_default();
                    let mut n = 0;
                    let guid = loop {
                        let g = Uuid::new_v5(&ns, format!("{}|{}|{}|{n}", self.m.t1, rb.name, rb.action).as_bytes()).to_string();
                        if !source_lc.contains(&g) && !self.new_guid.values().any(|v| *v == g) {
                            break g;
                        }
                        n += 1;
                    };
                    let group = (rb.kind == "Submenu").then(|| (self.m.t1.as_str(), id + 1));
                    let record = action_record(&self.shp, &guid, rb, self.std_of(rb), group);
                    self.new_actions.push(record);
                    self.new_guid.insert(id, guid);
                    self.register_in(kids, source_lc, ns);
                }
            }
        }
    }

    fn rows(&mut self, plans: &[Plan], group: Option<usize>) -> Result<Vec<Row>> {
        let mut rows = Vec::with_capacity(plans.len());
        for p in plans {
            match p {
                Plan::Keep { bound, .. } => {
                    let b = &self.bound[*bound];
                    let rec = Tx::Raw(emit(self.site, b.node.span.span, self.pieces, &mut self.used)?);
                    let old_idx = (Some(b.group) == group).then_some(b.index);
                    rows.push(Row { id: b.id, guid: b.guid.clone(), rec, old_idx, new_sub: false });
                }
                Plan::New { rb, .. } => {
                    let id = rb.id.unwrap_or_default();
                    let guid = self.new_guid.get(&id).cloned().unwrap_or_default();
                    let rec = button_record(&self.shp, &self.m.t1, id, rb, self.std_of(rb));
                    rows.push(Row { id, guid, rec, old_idx: None, new_sub: rb.kind == "Submenu" });
                }
            }
        }
        Ok(rows)
    }

    fn group_record(header: [String; 4], rows: Vec<Row>, tail: Tx) -> Tx {
        let mut items: Vec<Tx> = header.iter().map(at).collect();
        items.push(at(rows.len()));
        for r in rows {
            items.push(at(r.guid));
            items.push(r.rec);
        }
        items.push(tail);
        Tx::Nd(items)
    }

    /// Writes the group of `plans`: an existing group only when its buttons changed, a new group always; then the groups
    /// of the sub-menus below.
    fn groups(&mut self, plans: &[Plan], target: Target) -> Result<()> {
        match target {
            Target::Existing(gi) => {
                let g = self.m.groups[gi];
                let k = group_k(self.site, g)?;
                let old_ids: Vec<Option<i64>> = (0..k).map(|i| g.child(6 + 2 * i).and_then(|b| int_at(b, 7))).collect();
                let new_ids: Vec<Option<i64>> = plans
                    .iter()
                    .map(|p| match p {
                        Plan::Keep { bound, .. } => Some(self.bound[*bound].id),
                        Plan::New { rb, .. } => rb.id,
                    })
                    .collect();
                if old_ids != new_ids {
                    let tail = read_tail(self.site, g)?;
                    let rows = self.rows(plans, Some(gi))?;
                    let t1 = self.m.t1.clone();
                    let new_tail = make_tail(self.site, Some((&tail, k)), &rows, &t1, &self.lists)?;
                    let header = [0, 1, 2, 3].map(|i| raw_atom(self.site, g, i).unwrap_or_default());
                    self.rebuilt.insert(gi, Self::group_record(header, rows, new_tail));
                }
            }
            Target::New(code) => {
                let rows = self.rows(plans, None)?;
                let t1 = self.m.t1.clone();
                let tail = make_tail(self.site, None, &rows, &t1, &self.lists)?;
                let header = [raw_version(self.m), t1, code.to_string(), "0".to_string()];
                self.new_groups.push(Self::group_record(header, rows, tail));
            }
        }
        self.below(plans)
    }

    /// The groups of the sub-menus among `plans`. The group of a platform sub-menu is copied as it is.
    fn below(&mut self, plans: &[Plan]) -> Result<()> {
        for p in plans {
            match p {
                Plan::Keep { bound, kids } => {
                    let b = &self.bound[*bound];
                    match b.sub {
                        Some(sub) if !b.frozen => self.groups(kids, Target::Existing(sub))?,
                        _ => self.below(kids)?,
                    }
                }
                Plan::New { rb, kids } => {
                    if rb.kind == "Submenu" {
                        self.groups(kids, Target::New(rb.id.unwrap_or_default() + 1))?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// The record version of a group (`5`), copied from the first group of the panel.
fn raw_version(m: &Model) -> String {
    m.groups.first().and_then(|g| g.atom(0)).map(|(a, _)| a.value.clone()).unwrap_or_else(|| "5".to_string())
}

/// The in-place edits of the kept buttons (the sub-menu content is handled by the recursion over `plans`).
fn kept_edits(
    site: &Site,
    m: &Model,
    bound: &[Bound],
    plans: &[Plan],
    resolved_of: &HashMap<i64, &CommandBarButton>,
    prop: Property,
    out: &mut Vec<Piece>,
) -> Result<()> {
    for p in plans {
        if let Plan::Keep { bound: bi, kids } = p {
            let b = &bound[*bi];
            if let Some(nb) = resolved_of.get(&b.id) {
                let ob = CommandBarButton { buttons: Vec::new(), ..b.old.clone() };
                let nb = CommandBarButton { buttons: Vec::new(), ..(*nb).clone() };
                patch_cb_button(site, (m.t, m.actions.len()), (&b.guid, b.node), (&ob, &nb), prop, 1, out)?;
            }
            kept_edits(site, m, bound, kids, resolved_of, prop, out)?;
        }
    }
    Ok(())
}

fn index_by_id<'r>(list: &'r [CommandBarButton], out: &mut HashMap<i64, &'r CommandBarButton>) {
    for b in list {
        if let Some(id) = b.id {
            out.insert(id, b);
        }
        index_by_id(&b.buttons, out);
    }
}

/// Rebuilds the whole button table for a changed set, order or nesting of the buttons.
pub(super) fn patch_structural(
    site: &Site,
    old_btns: &[CommandBarButton],
    new_btns: &[CommandBarButton],
    prop: Property,
) -> Result<Vec<Piece>> {
    let m = model(site, prop)?;
    check_table(site, &m, "unexpected-shape")?;
    let resolved = resolve(old_btns, new_btns, m.t2 + 1);

    // the stream's buttons, bound to the baseline tree
    let mut bound: Vec<Bound> = Vec::new();
    bind(site, &m, m.root, old_btns, 1, &mut HashSet::new(), &mut bound)?;
    let mut users: HashMap<usize, usize> = HashMap::new();
    for g in bound.iter().filter_map(|b| b.sub) {
        *users.entry(g).or_default() += 1;
    }
    for b in &mut bound {
        b.frozen = m.actions[b.act].platform || b.sub.is_some_and(|g| users[&g] > 1);
    }
    let by_id: HashMap<i64, usize> = bound.iter().enumerate().map(|(i, b)| (b.id, i)).collect();

    // what each requested button is
    let mut seen_ids: HashSet<i64> = HashSet::new();
    let mut fresh: Vec<&CommandBarButton> = Vec::new();
    let plans = classify(site, &bound, &by_id, &resolved, 1, &mut seen_ids, &mut fresh)?;

    // removals: a platform sub-menu stays; a user sub-menu goes with its group and, below it, everything not moved away
    let removed: Vec<usize> = (0..bound.len()).filter(|&i| !seen_ids.contains(&bound[i].id)).collect();
    for &i in &removed {
        if bound[i].frozen {
            return Err(site.invalid(
                "unsupported-button",
                "Buttons",
                format!("button '{}': removing a platform sub-menu, or one that several buttons show, is not supported", bound[i].old.name),
            ));
        }
    }
    let dropped_groups: HashSet<usize> = removed.iter().filter_map(|&i| bound[i].sub).collect();
    let removed_guids: HashSet<&str> = removed.iter().map(|&i| bound[i].guid.as_str()).collect();

    // names of new buttons must be free
    let mut taken: HashSet<&str> = (0..bound.len()).filter(|i| !removed.contains(i)).map(|i| bound[i].old.name.as_str()).collect();
    for nb in &fresh {
        if !taken.insert(nb.name.as_str()) {
            return Err(refuse(site, nb, "a button with this name exists in the panel"));
        }
    }

    // in-place edits of kept buttons (applied inside the copied records)
    let mut resolved_of: HashMap<i64, &CommandBarButton> = HashMap::new();
    index_by_id(&resolved, &mut resolved_of);
    let mut pieces: Vec<Piece> = Vec::new();
    kept_edits(site, &m, &bound, &plans, &resolved_of, prop, &mut pieces)?;
    let mut unique: Vec<Piece> = Vec::with_capacity(pieces.len());
    for p in pieces {
        if !unique.iter().any(|u| u.range == p.range && u.bytes == p.bytes) {
            unique.push(p);
        }
    }
    let pieces = unique;

    // the lists that follow their buttons
    let mut lists = Lists { refs: HashMap::new(), subs: HashMap::new() };
    for g in &m.groups {
        if let Ok(t) = read_tail(site, *g) {
            lists.refs.extend(t.refs);
            lists.subs.extend(t.subs);
        }
    }

    let n_alloc: i64 = fresh.iter().map(|b| if b.kind == "Submenu" { 2 } else { 1 }).sum();
    let next_id = m.t2 + n_alloc;
    let mut bx = Build {
        site,
        m: &m,
        bound: &bound,
        used: vec![false; pieces.len()],
        pieces: &pieces,
        shp: panel_shape(&m),
        new_guid: HashMap::new(),
        new_actions: Vec::new(),
        new_groups: Vec::new(),
        rebuilt: HashMap::new(),
        lists,
    };
    bx.register(&plans);
    bx.groups(&plans, Target::Existing(m.root))?;

    // the table
    let mut table = vec![
        at(raw_atom(site, m.t, 0)?),
        at(raw_atom(site, m.t, 1)?),
        at(next_id),
        at(raw_atom(site, m.t, 3)?),
        at(0),
    ];
    let mut n_actions = 0;
    for a in m.actions.iter().filter(|a| !removed_guids.contains(a.guid.as_str())) {
        table.push(Tx::Raw(emit(site, a.node.span.span, &pieces, &mut bx.used)?));
        n_actions += 1;
    }
    n_actions += bx.new_actions.len();
    table.append(&mut bx.new_actions);
    table[4] = at(n_actions);
    let mut group_records: Vec<Tx> = Vec::new();
    for (gi, g) in m.groups.iter().enumerate() {
        if dropped_groups.contains(&gi) {
            continue;
        }
        match bx.rebuilt.remove(&gi) {
            Some(rec) => group_records.push(rec),
            None => group_records.push(Tx::Raw(emit(site, g.span.span, &pieces, &mut bx.used)?)),
        }
    }
    group_records.append(&mut bx.new_groups);
    table.push(at(group_records.len()));
    table.extend(group_records);
    if bx.used.iter().any(|u| !u) {
        return Err(shape(site, "an edit of a kept button lies outside the copied records"));
    }
    let src = site.source.source();
    let nl = if contains(src, b"\r\n") {
        "\r\n"
    } else if contains(src, b"\n") {
        "\n"
    } else {
        ""
    };
    let mut text = String::new();
    render(&Tx::Nd(table), nl, &mut text);
    Ok(vec![piece(m.t.span.span, text, &[prop])])
}

/// After patching: the table holds the invariants, the tails of the groups are sound, the id counter advanced by the
/// ids and group codes that were allocated and the root group lists the requested buttons, in order. The whole tree
/// is compared by the caller against what the reader decodes.
pub(super) fn verify(site: &Site, original: &Site, resolved: &[CommandBarButton]) -> Result<()> {
    let before = model(original, Property::Buttons)?;
    let m = model(site, Property::Buttons)?;
    check_table(site, &m, "verification-failed")?;
    let bad = |why: String| site.layout("verification-failed", "Buttons", why);
    let key_of = |g: NodeRef| cb_group_key(g).map(|(a, b)| (a.to_string(), b.to_string()));
    let old_keys: HashSet<(String, String)> = before.groups.iter().filter_map(|g| key_of(*g)).collect();
    let sound_before: HashSet<(String, String)> = before
        .groups
        .iter()
        .filter(|g| group_k(original, **g).and_then(|k| check_tail(original, **g, k, "unexpected-shape")).is_ok())
        .filter_map(|g| key_of(*g))
        .collect();
    for g in &m.groups {
        let Some(key) = key_of(*g) else { continue };
        if sound_before.contains(&key) || !old_keys.contains(&key) {
            check_tail(site, *g, group_k(site, *g)?, "verification-failed")?;
        }
    }
    let root = m.groups[m.root];
    let k = group_k(site, root)?;
    let ids: Vec<Option<i64>> = (0..k).map(|i| root.child(6 + 2 * i).and_then(|b| int_at(b, 7))).collect();
    if ids != resolved.iter().map(|b| b.id).collect::<Vec<_>>() {
        return Err(bad("the root group does not list the requested buttons in order".into()));
    }
    fn count(list: &[CommandBarButton], after: i64, new: &mut i64, subs: &mut Vec<(i64, usize)>) {
        for b in list {
            if b.id.is_some_and(|id| id > after) {
                *new += 1;
                if b.kind == "Submenu" {
                    *new += 1;
                    subs.push((b.id.unwrap_or_default() + 1, b.buttons.len()));
                }
            }
            count(&b.buttons, after, new, subs);
        }
    }
    let (mut n_new, mut new_subs) = (0, Vec::new());
    count(resolved, before.t2, &mut n_new, &mut new_subs);
    if m.t2 != before.t2 + n_new {
        return Err(bad(format!("the id counter is {} instead of {}", m.t2, before.t2 + n_new)));
    }
    for (code, n) in new_subs {
        let key = (m.t1.clone(), code.to_string());
        let Some(gi) = m.group_at(site, &key)? else {
            return Err(bad(format!("the group of the new sub-menu {code} is missing")));
        };
        if group_k(site, m.groups[gi])? != n {
            return Err(bad(format!("the group of the new sub-menu {code} does not hold {n} buttons")));
        }
    }
    Ok(())
}

/// Handler buttons whose procedure is new or changed by the edit: `(button, procedure)`.
pub fn new_handlers(edit: &ControlEdit) -> Vec<(String, String)> {
    fn known(btns: &[CommandBarButton], out: &mut HashMap<i64, String>) {
        for b in btns {
            if let Some(id) = b.id {
                out.insert(id, b.action.clone());
            }
            known(&b.buttons, out);
        }
    }
    fn walk(btns: &[CommandBarButton], old: &HashMap<i64, String>, out: &mut Vec<(String, String)>) {
        for b in btns {
            let handler = b.kind == "Action" && !b.action.is_empty() && !b.action.starts_with("Standard:");
            if handler && b.id.and_then(|id| old.get(&id)) != Some(&b.action) {
                out.push((b.name.clone(), b.action.clone()));
            }
            walk(&b.buttons, old, out);
        }
    }
    let mut old = HashMap::new();
    known(&edit.old.buttons, &mut old);
    let mut out = Vec::new();
    if edit.changed.contains(&Property::Buttons) {
        walk(&edit.new.buttons, &old, &mut out);
    }
    out
}

/// True when `module` declares `Процедура <name>(` or `Функция <name>(` (any case, English keywords too).
pub fn module_declares(module: &str, name: &str) -> bool {
    Regex::new(&format!(r"(?i)(процедура|функция|procedure|function)\s+{}\s*\(", regex::escape(name)))
        .is_ok_and(|re| re.is_match(module))
}
