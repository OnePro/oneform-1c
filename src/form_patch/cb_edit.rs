//! Adding, removing and reordering the buttons of a `CommandBar` (`<Buttons>` of `Form.xml`).
//!
//! The panel record holds a table `X[7] = {5,T1,T2,1,nAct,Action×nAct,nGroups,Group×nGroups}`: actions and
//! buttons pair 1:1, the root group lists `(action ref, Button)` pairs and ends with a tail
//! `{pos,nRefs,(T1,id)×nRefs,{nSub,(T1,id,flag)×nSub}}` (`docs/design/command-bar-advisor-report.md`). When the set or
//! the order of the root buttons changes the whole table is rebuilt as ONE replacement: kept records are copied from
//! their source text (with the in-place property edits of [`patch_cb_button`] applied inside them), new records are
//! built from the shape of the panel's own siblings. Creating sub-menus, `Standard:N` commands and pictures are not
//! supported and are refused with the line of the `<Button>`.

use std::collections::{HashMap, HashSet};

use regex::Regex;
use uuid::Uuid;

use super::*;
use crate::oof_props::{NULL_GROUP_GUID, STD_SUBMENU_ACTION_GUID, SUBMENU_ACTION_GUID};

const HANDLER_KIND: &str = crate::oof_props::EVENT_GUID;
const SEPARATOR_KIND: &str = SUBMENU_ACTION_GUID;

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
        if kind == SUBMENU_ACTION_GUID || kind == STD_SUBMENU_ACTION_GUID {
            let body = node.child(4).ok_or_else(|| shape(site, "action body missing"))?;
            let (g, c) = (body.atom(1), body.atom(2));
            if let (Some((g, _)), Some((c, _))) = (g, c) {
                if !(g.value.eq_ignore_ascii_case(NULL_GROUP_GUID) && c.value == "0") {
                    group = Some((g.value.clone(), c.value.clone()));
                }
            }
        }
        actions.push(Act { guid, node, group });
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

/// The tail of the root group: `{pos,nRefs,(T1,id)×nRefs,{nSub,(T1,id,flag)×nSub}}`.
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
    for s in 0..n {
        if !int_at(sub, 2 + 3 * s).is_some_and(|id| ids.contains(&id)) {
            return Err(bad("a sub-menu entry names a button that is not in the group"));
        }
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

/// True when the set or the order of the root buttons differs: a missing id marks a new button.
pub(super) fn is_structural(old: &[CommandBarButton], new: &[CommandBarButton]) -> bool {
    old.len() != new.len() || old.iter().zip(new).any(|(o, n)| n.id.is_none() || n.id != o.id)
}

/// The requested root buttons as the patched stream decodes them: new buttons get the ids `first_id`, `first_id + 1`, ...
/// in document order.
pub(super) fn resolve(old: &[CommandBarButton], new: &[CommandBarButton], first_id: i64) -> Vec<CommandBarButton> {
    let mut known = HashSet::new();
    collect(old, &mut known, &mut Vec::new());
    let mut next = first_id;
    new.iter()
        .map(|b| {
            let mut b = b.clone();
            if b.id.is_none_or(|id| !known.contains(&id)) {
                b.id = Some(next);
                next += 1;
                if b.kind == "Action" && b.action_kind.is_empty() {
                    b.action_kind = "Handler".to_string();
                }
            }
            b
        })
        .collect()
}

/// The resolved root buttons of a structural `Buttons` edit; `None` for an edit that keeps the set and the order.
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

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// What a new button may hold; everything else is refused.
fn validate_new(site: &Site, b: &CommandBarButton) -> Result<()> {
    if !is_identifier(&b.name) {
        return Err(refuse(site, b, "the name must be an identifier (letters, digits, '_'; no leading digit)"));
    }
    if !b.buttons.is_empty() || b.kind == "Submenu" {
        return Err(refuse(site, b, "a sub-menu cannot be created yet; add handler buttons and separators"));
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
        "Separator" => {
            if !b.action.is_empty() || !b.action_kind.is_empty() || !b.caption.is_empty() || !b.tooltip.is_empty() {
                return Err(refuse(site, b, "a new separator holds no action, caption or tooltip"));
            }
        }
        "Action" => {
            if b.action.starts_with("Standard:") || b.action_kind == "Standard" {
                return Err(refuse(site, b, "standard commands (Standard:N) cannot be added yet"));
            }
            if !b.action_kind.is_empty() && b.action_kind != "Handler" {
                return Err(refuse(site, b, "only handler buttons can be created (actionKind=\"Handler\")"));
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

fn action_record(sh: &Shape, guid: &str, b: &CommandBarButton) -> Tx {
    let mut v = vec![at(&sh.act_version), at(guid), at(1)];
    if b.kind == "Separator" {
        v.push(at(SEPARATOR_KIND));
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

fn button_record(sh: &Shape, t1: &str, id: i64, b: &CommandBarButton) -> Tx {
    let repr = match b.representation.as_str() {
        "Text" => 1,
        "Picture" => 2,
        "PictureAndText" => 3,
        _ => 0,
    };
    let separator = b.kind == "Separator";
    let mut v = vec![
        at(&sh.btn_version),
        at(quote(&b.name)),
        at(0),
        at(1),
        loc(&b.caption),
        at(i32::from(!separator)),
        at(t1),
        at(id),
        at("1e2"),
        at(if separator { 2 } else { 0 }),
        at(repr),
        at(1),
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

/// Rebuilds the whole button table for a changed set / order of the root buttons.
pub(super) fn patch_structural(
    site: &Site,
    old_btns: &[CommandBarButton],
    new_btns: &[CommandBarButton],
    prop: Property,
) -> Result<Vec<Piece>> {
    let m = model(site, prop)?;
    check_table(site, &m, "unexpected-shape")?;
    let root = m.groups[m.root];
    let k = group_k(site, root)?;
    if old_btns.len() != k {
        return Err(site.mismatch("Buttons", &format!("{} root buttons", old_btns.len()), &format!("{k} root buttons")));
    }
    // the stream's root buttons, in order
    let mut stream_ids = Vec::with_capacity(k);
    let mut guids = Vec::with_capacity(k);
    let mut bnodes = Vec::with_capacity(k);
    for i in 0..k {
        let b = root.child(6 + 2 * i).ok_or_else(|| shape(site, "button node missing"))?;
        stream_ids.push(int_at(b, 7).ok_or_else(|| shape(site, "button id missing"))?);
        guids.push(root.atom(5 + 2 * i).ok_or_else(|| shape(site, "action reference missing"))?.0.value.clone());
        bnodes.push(b);
    }
    for (ob, id) in old_btns.iter().zip(&stream_ids) {
        if ob.id != Some(*id) {
            return Err(site.mismatch("Buttons", &format!("button '{}' id {:?}", ob.name, ob.id), &format!("id {id}")));
        }
    }
    let mut every_id = HashSet::new();
    let mut every_name = Vec::new();
    collect(old_btns, &mut every_id, &mut every_name);
    let root_index: HashMap<i64, usize> = stream_ids.iter().copied().zip(0..).collect();

    // classify the requested buttons
    let mut kept: Vec<Option<usize>> = Vec::with_capacity(new_btns.len());
    let mut seen_old = vec![false; k];
    let mut seen_ids: HashSet<i64> = HashSet::new();
    for nb in new_btns {
        match nb.id {
            Some(id) if every_id.contains(&id) => {
                let Some(&i) = root_index.get(&id) else {
                    return Err(refuse(site, nb, "a button cannot move between sub-menu levels yet"));
                };
                if !seen_ids.insert(id) {
                    return Err(refuse(site, nb, &format!("the id {id} is used twice")));
                }
                let ob = &old_btns[i];
                if ob.kind != nb.kind {
                    return Err(refuse(site, nb, "the kind of an existing button cannot change"));
                }
                seen_old[i] = true;
                kept.push(Some(i));
            }
            Some(id) if !seen_ids.insert(id) => return Err(refuse(site, nb, &format!("the id {id} is used twice"))),
            _ => {
                validate_new(site, nb)?;
                kept.push(None);
            }
        }
    }
    // removals
    let action_of = |guid: &str| m.actions.iter().find(|a| a.guid == guid);
    for i in (0..k).filter(|&i| !seen_old[i]) {
        let ob = &old_btns[i];
        if ob.kind == "Submenu" || !ob.buttons.is_empty() || action_of(&guids[i]).is_none_or(|a| a.group.is_some()) {
            return Err(site.invalid(
                "unsupported-button",
                "Buttons",
                format!("button '{}': removing a sub-menu is not supported yet", ob.name),
            ));
        }
    }
    // names of new buttons must be free
    let mut taken: HashSet<String> = every_name.into_iter().collect();
    for i in (0..k).filter(|&i| !seen_old[i]) {
        taken.remove(&old_btns[i].name);
    }
    for (nb, keep) in new_btns.iter().zip(&kept) {
        if keep.is_none() && !taken.insert(nb.name.clone()) {
            return Err(refuse(site, nb, "a button with this name exists in the panel"));
        }
    }

    // in-place edits of kept buttons (applied inside the copied records)
    let mut pieces: Vec<Piece> = Vec::new();
    for (nb, keep) in new_btns.iter().zip(&kept) {
        if let Some(i) = *keep {
            patch_cb_button(site, (m.t, m.actions.len()), (&guids[i], bnodes[i]), (&old_btns[i], nb), prop, 1, &mut pieces)?;
        }
    }
    let mut unique: Vec<Piece> = Vec::with_capacity(pieces.len());
    for p in pieces {
        if !unique.iter().any(|u| u.range == p.range && u.bytes == p.bytes) {
            unique.push(p);
        }
    }
    let pieces = unique;
    let mut used = vec![false; pieces.len()];

    // tail
    let tail = root.child(5 + 2 * k).ok_or_else(|| shape(site, "tail missing"))?;
    let n_refs = int_at(tail, 1).and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
    let sub = tail.child(2 + 2 * n_refs).ok_or_else(|| shape(site, "sub-menu list missing"))?;
    let n_sub = int_at(sub, 0).and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
    let removed_ids: HashSet<i64> = (0..k).filter(|&i| !seen_old[i]).map(|i| stream_ids[i]).collect();
    let k_new = new_btns.len();
    let mut new_index: Vec<Option<usize>> = vec![None; k];
    for (j, keep) in kept.iter().enumerate() {
        if let Some(i) = keep {
            new_index[*i] = Some(j);
        }
    }
    let pos = int_at(tail, 0).ok_or_else(|| shape(site, "tail position missing"))?;
    let new_pos = if pos == -1 {
        -1
    } else if pos == k as i64 {
        k_new as i64
    } else {
        let from = usize::try_from(pos).map_err(|_| shape(site, "tail position is negative"))?;
        new_index[from..].iter().flatten().next().map_or(k_new, |j| *j) as i64
    };
    let mut tail_items = vec![at(new_pos)];
    let mut refs: Vec<Tx> = Vec::new();
    for r in 0..n_refs {
        if !int_at(tail, 3 + 2 * r).is_some_and(|id| removed_ids.contains(&id)) {
            refs.push(at(raw_atom(site, tail, 2 + 2 * r)?));
            refs.push(at(raw_atom(site, tail, 3 + 2 * r)?));
        }
    }
    tail_items.push(at(refs.len() / 2));
    tail_items.extend(refs);
    let mut subs: HashMap<i64, [String; 3]> = HashMap::new();
    for s in 0..n_sub {
        let id = int_at(sub, 2 + 3 * s).ok_or_else(|| shape(site, "sub-menu entry id missing"))?;
        subs.insert(id, [raw_atom(site, sub, 1 + 3 * s)?, raw_atom(site, sub, 2 + 3 * s)?, raw_atom(site, sub, 3 + 3 * s)?]);
    }
    let mut sub_items = vec![at(0)];
    let mut n_listed = 0;
    for keep in &kept {
        if let Some(entry) = keep.and_then(|i| subs.get(&stream_ids[i])) {
            sub_items.extend(entry.iter().map(at));
            n_listed += 1;
        }
    }
    sub_items[0] = at(n_listed);
    tail_items.push(Tx::Nd(sub_items));

    // the new records
    let shp = panel_shape(&m);
    let source_lc = String::from_utf8_lossy(site.source.source()).to_lowercase();
    let ns = Uuid::parse_str(&m.t1).unwrap_or(Uuid::NAMESPACE_OID);
    let mut next_id = m.t2;
    let mut pairs: Vec<Tx> = Vec::new();
    let mut new_actions: Vec<Tx> = Vec::new();
    let mut fresh: HashSet<String> = HashSet::new();
    for (nb, keep) in new_btns.iter().zip(&kept) {
        match keep {
            Some(i) => {
                let span = bnodes[*i].span.span;
                pairs.push(at(&guids[*i]));
                pairs.push(Tx::Raw(emit(site, span, &pieces, &mut used)?));
            }
            None => {
                next_id += 1;
                let mut n = 0;
                let guid = loop {
                    let g = Uuid::new_v5(&ns, format!("{}|{}|{}|{n}", m.t1, nb.name, nb.action).as_bytes()).to_string();
                    if !source_lc.contains(&g) && !fresh.contains(&g) {
                        break g;
                    }
                    n += 1;
                };
                fresh.insert(guid.clone());
                new_actions.push(action_record(&shp, &guid, nb));
                pairs.push(at(&guid));
                pairs.push(button_record(&shp, &m.t1, next_id, nb));
            }
        }
    }
    let mut root_items = vec![
        at(raw_atom(site, root, 0)?),
        at(raw_atom(site, root, 1)?),
        at(raw_atom(site, root, 2)?),
        at(raw_atom(site, root, 3)?),
        at(k_new),
    ];
    root_items.extend(pairs);
    root_items.push(Tx::Nd(tail_items));

    let removed_guids: HashSet<&str> = (0..k).filter(|&i| !seen_old[i]).map(|i| guids[i].as_str()).collect();
    let mut table = vec![
        at(raw_atom(site, m.t, 0)?),
        at(raw_atom(site, m.t, 1)?),
        at(next_id),
        at(raw_atom(site, m.t, 3)?),
        at(0),
    ];
    let mut n_actions = 0;
    for a in m.actions.iter().filter(|a| !removed_guids.contains(a.guid.as_str())) {
        table.push(Tx::Raw(emit(site, a.node.span.span, &pieces, &mut used)?));
        n_actions += 1;
    }
    n_actions += new_actions.len();
    table.extend(new_actions);
    table[4] = at(n_actions);
    table.push(at(m.groups.len()));
    for (gi, g) in m.groups.iter().enumerate() {
        if gi == m.root {
            table.push(Tx::Nd(std::mem::take(&mut root_items)));
        } else {
            table.push(Tx::Raw(emit(site, g.span.span, &pieces, &mut used)?));
        }
    }
    if used.iter().any(|u| !u) {
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

/// After patching: the table holds the invariants and the root group lists exactly `resolved`, in order, with the
/// id counter advanced by the number of new buttons.
pub(super) fn verify(site: &Site, original: &Site, resolved: &[CommandBarButton]) -> Result<()> {
    let before = model(original, Property::Buttons)?;
    let m = model(site, Property::Buttons)?;
    check_table(site, &m, "verification-failed")?;
    let bad = |why: String| site.layout("verification-failed", "Buttons", why);
    let root = m.groups[m.root];
    let k = group_k(site, root)?;
    let ids: Vec<Option<i64>> = (0..k).map(|i| root.child(6 + 2 * i).and_then(|b| int_at(b, 7))).collect();
    if ids != resolved.iter().map(|b| b.id).collect::<Vec<_>>() {
        return Err(bad("the root group does not list the requested buttons in order".into()));
    }
    let n_new = resolved.iter().filter(|b| b.id.is_some_and(|id| id > before.t2)).count() as i64;
    if m.t2 != before.t2 + n_new {
        return Err(bad(format!("the id counter is {} instead of {}", m.t2, before.t2 + n_new)));
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
