//! Consistency of the anchor graph of an ordinary form.
//!
//! Every control's geometry node holds six edge records (`geom[6..12]`, `{0,{2,target,side,offset},{2,altTarget,altSide,altOffset}}`)
//! followed by six counted incoming lists (`geom[12..]`). The list `k` of a target holds one `{0,src,edge}` entry for every
//! source control whose edge `edge` (primary or secondary part) is attached to side `k` of the target. Target `0` is the
//! container panel: its six lists live in the panel parameters node (`info[1]`, lists start at index 2).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::*;

/// Where a set of six incoming lists is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Holder {
    /// The geometry node of control `id`.
    Geom(i64),
    /// The panel parameters of the Panel control `id`; `0` is the form's root panel.
    Panel(i64),
}

pub(super) type Lists = [Vec<(i64, i64)>; 6];

/// Side `s` of a target is listed in list `s`; the secondary "right" side `5` shares the list of the right side, the secondary "bottom" side `4` the bottom list.
pub(super) fn list_of_side(side: i64) -> Option<usize> {
    match side {
        0..=3 => Some(side as usize),
        4 => Some(1),
        5 => Some(3),
        _ => None,
    }
}

/// The container id of a control for edges attached to target `0`: the closest ancestor that owns anchor lists.
pub(super) fn container_holder(idx: &ControlIndex, parent: Option<i64>) -> Holder {
    let mut p = parent;
    while let Some(id) = p {
        match idx.get(id) {
            Some(e) if e.type_name == "Panel" => return Holder::Panel(id),
            Some(e) => p = e.parent,
            None => break,
        }
    }
    Holder::Panel(0)
}

/// The node holding the six counted lists of `holder` and the index of the first list in it.
pub(super) fn holder_node<'a>(idx: &ControlIndex<'a>, h: Holder) -> Option<(NodeRef<'a>, usize)> {
    match h {
        Holder::Geom(id) => Some((idx.get(id)?.geom()?, 12)),
        Holder::Panel(0) => Some((idx.root_panel().ok()?.child(1)?.child(1)?, 2)),
        Holder::Panel(id) => Some((idx.get(id)?.info()?.child(1)?, 2)),
    }
}

/// Read the six counted lists starting at `start`; also returns the child index of every list's count atom.
pub(super) fn read_lists(n: &Node, start: usize) -> Option<(Lists, [usize; 6])> {
    let mut out: Lists = Default::default();
    let mut at = [0usize; 6];
    let mut i = start;
    for k in 0..6 {
        at[k] = i;
        let count: usize = n.atom(i)?.value.parse().ok()?;
        for j in 0..count {
            let e = n.node(i + 1 + j)?;
            if e.len() != 3 || e.atom(0)?.value != "0" {
                return None;
            }
            out[k].push((e.atom(1)?.value.parse().ok()?, e.atom(2)?.value.parse().ok()?));
        }
        i += 1 + count;
    }
    Some((out, at))
}

/// Attached `(edge index, part, edge)` records of a geometry node (all six records, both parts).
pub(super) fn attached_edges(geom: &Node) -> Vec<(usize, usize, AnchorEdge)> {
    let mut out = Vec::new();
    for e in 0..6 {
        for part in [1usize, 2] {
            if let Some(edge) = read_anchor_edge_part(geom, 6 + e, part) {
                if edge.side != 6 && edge.target != -1 {
                    out.push((e, part, edge));
                }
            }
        }
    }
    out
}

/// Check the anchor graph of a form stream: every attached edge (primary and secondary) of every control must be
/// listed exactly once in the matching incoming list of its target (target `0`: the container's panel lists), and
/// every edge must point to an existing control. Empty when the graph is consistent.
///
/// Real Designer output is lenient in the other direction: lists keep leftover entries of deleted controls and of
/// edges that were re-attached or copied, see [`anchor_graph_stale_entries`]; those are not violations.
pub fn anchor_graph_violations(source: &SpannedMetadata) -> Vec<String> {
    check(source, None).0
}

/// Incoming-list entries that no edge justifies although their source control exists (Designer leaves such leftovers
/// after re-attaching or copying controls; 1C tolerates them). Informational only.
pub fn anchor_graph_stale_entries(source: &SpannedMetadata) -> Vec<String> {
    check(source, None).1
}

fn check(source: &SpannedMetadata, scope: Option<&[i64]>) -> (Vec<String>, Vec<String>) {
    let idx = match ControlIndex::build(source) {
        Ok(i) => i,
        Err(e) => return (vec![format!("cannot index the controls: {e}")], Vec::new()),
    };
    let (mut v, mut stale) = (Vec::new(), Vec::new());
    let name = |id: i64| idx.get(id).map(|e| format!("'{}' (id {id})", e.name)).unwrap_or_else(|| format!("(id {id})"));
    let what = |h: Holder| match h {
        Holder::Geom(id) => format!("control {}", name(id)),
        Holder::Panel(0) => "the root panel".to_string(),
        Holder::Panel(id) => format!("panel {}", name(id)),
    };
    // expected[holder][list] = (src, edge) entries demanded by the edges
    let mut expected: BTreeMap<Holder, Lists> = BTreeMap::new();
    for e in &idx.entries {
        if scope.is_some_and(|ids| !ids.contains(&e.id)) {
            continue;
        }
        let Some(g) = e.geom().filter(|g| g.node.len() >= 18) else {
            continue; // context menus and the like carry no anchors
        };
        for (edge_i, part, edge) in attached_edges(g.node) {
            let label = format!(
                "control {}: edge {edge_i}{} -> target {} side {}",
                name(e.id),
                if part == 2 { " (secondary)" } else { "" },
                edge.target,
                edge.side
            );
            let Some(k) = list_of_side(edge.side) else {
                v.push(format!("{label}: unknown side code"));
                continue;
            };
            let holder = if edge.target == 0 {
                container_holder(&idx, e.parent)
            } else if idx.get(edge.target).is_some() {
                Holder::Geom(edge.target)
            } else {
                v.push(format!("{label}: the target control does not exist"));
                continue;
            };
            let list = &mut expected.entry(holder).or_default()[k];
            if !list.contains(&(e.id, edge_i as i64)) {
                list.push((e.id, edge_i as i64));
            }
        }
    }
    let mut holders: Vec<Holder> = idx
        .entries
        .iter()
        .flat_map(|e| [Holder::Geom(e.id)].into_iter().chain((e.type_name == "Panel").then_some(Holder::Panel(e.id))))
        .collect();
    holders.push(Holder::Panel(0));
    for h in holders {
        let exp = expected.remove(&h).unwrap_or_default();
        let Some((node, start)) = holder_node(&idx, h) else {
            if exp.iter().any(|l| !l.is_empty()) {
                v.push(format!("{}: anchor lists not found but edges point to them", what(h)));
            }
            continue;
        };
        let Some((actual, _)) = read_lists(node.node, start) else {
            if matches!(h, Holder::Geom(id) if idx.get(id).and_then(|e| e.geom()).is_some_and(|g| g.node.len() < 18)) {
                continue;
            }
            v.push(format!("{}: the incoming lists are malformed", what(h)));
            continue;
        };
        for k in 0..6 {
            for item in &exp[k] {
                if !actual[k].contains(item) {
                    v.push(format!("{}: incoming list {k} lacks the entry from control {} edge {}", what(h), name(item.0), item.1));
                }
            }
            for item in &actual[k] {
                if idx.get(item.0).is_some() && !exp[k].contains(item) {
                    stale.push(format!("{}: incoming list {k} holds an entry from control {} edge {} that no edge justifies", what(h), name(item.0), item.1));
                }
            }
        }
    }
    (v, stale)
}

/// Like [`anchor_graph_violations`], restricted to the edges of the controls `ids` (used to verify added controls
/// while the anchors of other controls are edited separately).
pub(super) fn anchor_violations_of(source: &SpannedMetadata, ids: &[i64]) -> Vec<String> {
    check(source, Some(ids)).0
}

/// The anchor edges of a new control, `(primary, secondary)` for top, bottom, left and right (geometry records 6..=9).
pub(super) type NewEdges = [(AnchorEdge, AnchorEdge); 4];

/// The Designer's way to place a control: its top hangs below the nearest control above it and its left follows the
/// nearest control to its left (same container and page); with no such neighbour the edge stays unattached. A
/// control left floating in the middle of a row or column makes the form's layout fall back to its design size
/// in 1C, so the neighbours are what keeps the new control in the chain.
///
/// An existing control stands where the edited `Form.xml` puts it (`edited`), not where the stream has it: the same
/// edit may move the control and add one next to it.
fn neighbours(idx: &ControlIndex, edited: &FormXml, c: &ControlXml, page: i64, deleted: &[i64]) -> (Option<AnchorEdge>, Option<AnchorEdge>) {
    let v = &c.values;
    let (left, top, right, bottom) = (v.left, v.top, v.left + v.width, v.top + v.height);
    let mut above: Option<(i64, i64, i64)> = None; // (bottom, id, gap)
    let mut beside: Option<(i64, i64, i64)> = None;
    for e in &idx.entries {
        if e.parent != c.parent || e.page != page || deleted.contains(&e.id) {
            continue;
        }
        let Some(g) = e.geom() else { continue };
        let at = |i: usize| match g.node.children.get(i) {
            Some(Item::Atom(a)) => a.value.parse::<i64>().ok(),
            _ => None,
        };
        let (Some(l), Some(t), Some(r), Some(b), Some(visible)) = (at(1), at(2), at(3), at(4), at(5)) else { continue };
        let (l, t, r, b) = match edited.control(e.id) {
            Some(x) if x.type_name == e.type_name => (x.values.left, x.values.top, x.values.left + x.values.width, x.values.top + x.values.height),
            _ => (l, t, r, b),
        };
        if visible == 0 {
            continue;
        }
        if b <= top && l < right && r > left && above.is_none_or(|(ab, aid, _)| b > ab || (b == ab && e.id < aid)) {
            above = Some((b, e.id, top - b));
        }
        if r <= left && t < bottom && b > top && beside.is_none_or(|(br, bid, _)| r > br || (r == br && e.id < bid)) {
            beside = Some((r, e.id, left - r));
        }
    }
    (
        above.map(|(_, id, gap)| AnchorEdge { target: id, side: 1, offset: gap }),
        beside.map(|(_, id, gap)| AnchorEdge { target: id, side: 3, offset: gap }),
    )
}

/// Resolve the edges of a new control `new_id` (XML id `xml_id`): Designer defaults (top below the nearest control
/// above, left after the nearest control to the left, bottom/right to its own top/left plus `Height`/`Width`) for
/// every edge the request does not name; explicitly named edges are validated. The neighbours are found in the layout
/// of the edited form (`edited`).
pub(super) fn resolve_new_edges(
    idx: &ControlIndex,
    edited: &FormXml,
    c: &ControlXml,
    new_id: i64,
    page: i64,
    other_new_xml_ids: &[i64],
    deleted: &[i64],
) -> Result<NewEdges> {
    let v = &c.values;
    let (above, beside) = neighbours(idx, edited, c, page, deleted);
    let defaults = [
        above.unwrap_or(AnchorEdge::UNATTACHED),
        AnchorEdge { target: new_id, side: 0, offset: v.height },
        beside.unwrap_or(AnchorEdge::UNATTACHED),
        AnchorEdge { target: new_id, side: 2, offset: v.width },
    ];
    let spec = v.anchors.clone().unwrap_or_default();
    let requested = [spec.top, spec.bottom, spec.left, spec.right];
    let mut out = [(AnchorEdge::UNATTACHED, AnchorEdge::UNATTACHED); 4];
    for e in 0..4 {
        let names = ["top", "bottom", "left", "right"];
        let prim = requested[e].unwrap_or(defaults[e]);
        let validated = [requested[e].is_some(), spec.alt[e].is_some()];
        for (part, edge) in [(false, Some(prim)), (true, spec.alt[e])] {
            let Some(mut edge) = edge else { continue };
            if edge.side != 6 && validated[usize::from(part)] {
                if edge.target == c.id {
                    edge.target = new_id;
                } else if other_new_xml_ids.contains(&edge.target) {
                    return Err(anchor_err(c, format!("<Anchors> {}{}: the target {} is another new control of the same edit; anchor to an existing control or to the form (0)", names[e], if part { "Alt" } else { "" }, edge.target)));
                } else if edge.target != 0 && (idx.get(edge.target).is_none() || deleted.contains(&edge.target)) {
                    return Err(anchor_err(c, format!("<Anchors> {}{}: the target control {} does not exist", names[e], if part { "Alt" } else { "" }, edge.target)));
                }
            }
            if part {
                out[e].1 = edge;
            } else {
                out[e].0 = edge;
            }
        }
    }
    Ok(out)
}

fn anchor_err(c: &ControlXml, message: String) -> ExtractionError {
    ExtractionError::new(GENERATED_FILE_MODIFIED, format!("{} '{}' (id {}): {message}", c.type_name, c.name, c.id))
        .with("kind", "invalid-xml")
        .with("property", "Anchors")
        .with("id", c.id)
}

/// Write the resolved edges into a clone's geometry node and the entries of its own lists (self references).
/// `external` collects the entries that belong to other holders: `(holder, list) -> (source, edge)`.
pub(super) fn apply_new_edges(
    geom: &mut Node,
    new_id: i64,
    edges: &NewEdges,
    container: Holder,
    external: &mut BTreeMap<(Holder, usize), Vec<(i64, i64)>>,
) -> Result<()> {
    let mut own: Lists = Default::default();
    for (e, (prim, alt)) in edges.iter().enumerate() {
        let rec = geom
            .children
            .get_mut(6 + e)
            .and_then(Item::as_node_mut)
            .filter(|r| r.children.len() > 2)
            .ok_or_else(|| layout_err(format!("prototype: geometry edge record {} missing", 6 + e)))?;
        for (part, edge) in [(1, prim), (2, alt)] {
            rec.children[part] = Item::Node(
                crate::brace::parse_metadata(format_edge_target(edge).as_bytes()).map_err(|er| layout_err(er.to_string()))?,
            );
            if edge.side == 6 || edge.target == -1 {
                continue;
            }
            let k = list_of_side(edge.side).ok_or_else(|| layout_err(format!("anchor side {} cannot be listed", edge.side)))?;
            let entry = (new_id, e as i64);
            let bucket = if edge.target == new_id {
                &mut own[k]
            } else {
                let holder = if edge.target == 0 { container } else { Holder::Geom(edge.target) };
                external.entry((holder, k)).or_default()
            };
            if !bucket.contains(&entry) {
                bucket.push(entry);
            }
        }
    }
    // Replace the six emptied `0` counts by the filled lists.
    let empty = (12..18).all(|i| geom.children.get(i).and_then(|c| c.as_atom()).is_some_and(|a| a.value == "0"));
    if !empty {
        return Err(layout_err("prototype: incoming lists of the clone are not empty"));
    }
    let mut flat = Vec::new();
    for list in own {
        flat.push(bare(list.len() as i64));
        for (src, edge) in list {
            flat.push(Item::Node(
                crate::brace::parse_metadata(format!("{{0,{src},{edge}}}").as_bytes()).map_err(|er| layout_err(er.to_string()))?,
            ));
        }
    }
    geom.children.splice(12..18, flat);
    Ok(())
}

/// Replacements for the incoming lists of the surviving holders: the entries `adds` are appended (a list that
/// already holds an entry keeps it once) and, when controls are removed, every entry whose source is one of the
/// `removed` ids is dropped from every list that holds one (also leftovers that no edge justified). One list is
/// edited by one set of pieces: a list that only grows gets the count rewritten and the entries inserted after the
/// last existing entry (new ids are the largest, so ascending `(source, edge)` order is kept); a list that loses
/// entries is rewritten as a whole from the raw bytes of the kept entries followed by the added ones. Holders of
/// removed controls disappear with their control and are not touched.
pub(super) fn list_replacements(
    idx: &ControlIndex,
    adds: &BTreeMap<(Holder, usize), Vec<(i64, i64)>>,
    removed: &BTreeSet<i64>,
) -> Result<Vec<AppliedReplacement>> {
    let gone = |h: Holder| matches!(h, Holder::Geom(id) | Holder::Panel(id) if removed.contains(&id));
    let mut holders: BTreeSet<Holder> = adds.keys().map(|(h, _)| *h).collect();
    if !removed.is_empty() {
        for e in &idx.entries {
            holders.insert(Holder::Geom(e.id));
            if e.type_name == "Panel" {
                holders.insert(Holder::Panel(e.id));
            }
        }
        holders.insert(Holder::Panel(0));
    }
    let mut out = Vec::new();
    for h in holders.into_iter().filter(|h| !gone(*h)) {
        let wanted = adds.keys().any(|(a, _)| *a == h);
        let Some((node, start)) = holder_node(idx, h) else {
            if wanted {
                return Err(layout_err(format!("anchor target {h:?}: the incoming lists were not found in the stream")));
            }
            continue;
        };
        let Some((lists, at)) = read_lists(node.node, start) else {
            if wanted {
                return Err(layout_err(format!("anchor target {h:?}: the incoming lists are malformed")));
            }
            continue;
        };
        for k in 0..6 {
            let count = lists[k].len();
            let kept: Vec<usize> = (0..count).filter(|&j| !removed.contains(&lists[k][j].0)).collect();
            let mut entries: Vec<(i64, i64)> = adds.get(&(h, k)).cloned().unwrap_or_default();
            entries.sort_unstable();
            entries.dedup();
            entries.retain(|e| !kept.iter().any(|&j| lists[k][j] == *e));
            if kept.len() == count && entries.is_empty() {
                continue;
            }
            let Some(SpanItem::Atom(count_span)) = node.span.children.get(at[k]) else {
                return Err(layout_err("anchor target: count atom span missing"));
            };
            let items: Vec<String> = entries.iter().map(|(s, e)| format!("{{0,{s},{e}}}")).collect();
            if kept.len() < count {
                let last = node
                    .span
                    .children
                    .get(at[k] + count)
                    .ok_or_else(|| layout_err("anchor target: list span does not match its count"))?
                    .span();
                let mut bytes = (kept.len() + items.len()).to_string().into_bytes();
                for &j in &kept {
                    let raw = node
                        .span
                        .children
                        .get(at[k] + 1 + j)
                        .ok_or_else(|| layout_err("anchor list: entry span missing"))?
                        .span();
                    bytes.push(b',');
                    bytes.extend_from_slice(&idx.source().source()[raw.start..raw.end]);
                }
                for it in &items {
                    bytes.push(b',');
                    bytes.extend_from_slice(it.as_bytes());
                }
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: Span { start: count_span.start, end: last.end },
                    bytes,
                });
            } else if count == 0 {
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: *count_span,
                    bytes: format!("{},{}", items.len(), items.join(",")).into_bytes(),
                });
            } else {
                let last = node
                    .span
                    .children
                    .get(at[k] + count)
                    .ok_or_else(|| layout_err("anchor target: list span does not match its count"))?
                    .span();
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: *count_span,
                    bytes: (count + items.len()).to_string().into_bytes(),
                });
                out.push(AppliedReplacement {
                    control_id: 0,
                    properties: Vec::new(),
                    range: Span { start: last.end, end: last.end },
                    bytes: format!(",{}", items.join(",")).into_bytes(),
                });
            }
        }
    }
    Ok(out)
}

/// The controls that go away with `deleted`: the deleted ids and everything below a deleted control.
pub(super) fn removed_closure(idx: &ControlIndex, deleted: &[i64]) -> BTreeSet<i64> {
    let mut out: BTreeSet<i64> = deleted.iter().copied().collect();
    loop {
        let before = out.len();
        for e in &idx.entries {
            if e.parent.is_some_and(|p| out.contains(&p)) {
                out.insert(e.id);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// Where side `side_list` (a list index `0..4`: top, bottom, left, right) of the removed control `id` sits in terms of
/// a surviving target: follows the control's own primary edge of that side, through other removed controls and
/// through the control itself (`{2,self,0,height}` fixed sizes), adding up the offsets. Returns the resulting
/// edge and the control that owns the final edge (to compare its container). `None` when a link in the chain is
/// unattached, dangling or loops.
fn resolve_removed_side(
    idx: &ControlIndex,
    removed: &BTreeSet<i64>,
    id: i64,
    side_list: usize,
    acc: i64,
    seen: &mut Vec<(i64, usize)>,
) -> Option<(AnchorEdge, i64)> {
    if side_list > 3 || seen.contains(&(id, side_list)) {
        return None;
    }
    seen.push((id, side_list));
    let edge = read_anchor_edge_part(idx.get(id)?.geom()?.node, 6 + side_list, 1)?;
    if edge.side == 6 || edge.target == -1 {
        return None;
    }
    let acc = acc + edge.offset;
    if edge.target == 0 || (!removed.contains(&edge.target) && idx.get(edge.target).is_some()) {
        return Some((AnchorEdge { target: edge.target, side: edge.side, offset: acc }, id));
    }
    resolve_removed_side(idx, removed, edge.target, list_of_side(edge.side)?, acc, seen)
}

/// Re-anchor the surviving controls that are attached to a removed control, so that they keep their absolute position.
///
/// For every attached edge `e` (primary or secondary part) of a surviving control `D` whose target is a removed
/// control `R` (side `S`, offset `o`): R's own primary edge of the same side says where that side of `R` sits,
/// `{2,T,S',o'}`, so `D`'s edge becomes `{2,T,S',o'+o}` (the position `T.S' + o' + o` equals `R.S + o`). An edge of
/// `R` that is attached to `R` itself (a fixed size, `R.right = R.left + width`) or to another removed control is
/// followed until a surviving control or the container is reached, adding up the offsets. If the chain ends
/// unattached, dangling, in a loop, in the container of another panel or in `D` itself, `D` falls back to the
/// container at its current absolute coordinate: the top/bottom edges (`e` = 0, 1) become `{2,0,0,D.top|D.bottom}`,
/// the left/right edges (`e` = 2, 3) become `{2,0,2,D.left|D.right}`. The new incoming-list entry `{0,D,e}` is
/// added to `adds`. Edges named by the user in the same edit (`user`, keyed by `(id, edge, part)`) are not touched;
/// they must not point to a removed control.
pub(super) fn reanchor_dependants(
    idx: &ControlIndex,
    removed: &BTreeSet<i64>,
    user: &HashMap<(i64, usize, usize), AnchorEdge>,
    adds: &mut BTreeMap<(Holder, usize), Vec<(i64, i64)>>,
) -> Result<Vec<AppliedReplacement>> {
    let mut out = Vec::new();
    for d in idx.entries.iter().filter(|d| !removed.contains(&d.id)) {
        let Some(g) = d.geom().filter(|g| g.node.len() >= 18) else { continue };
        let rect: Vec<i64> = (1..=4).map(|i| g.atom(i).and_then(|(a, _)| a.value.parse().ok()).unwrap_or(0)).collect();
        let container = container_holder(idx, d.parent);
        for (e, part, edge) in attached_edges(g.node) {
            if !removed.contains(&edge.target) {
                continue;
            }
            let name = || format!("control '{}' (id {}): edge {e}", d.name, d.id);
            if let Some(u) = user.get(&(d.id, e, part)) {
                if removed.contains(&u.target) {
                    return Err(ExtractionError::new(
                        GENERATED_FILE_MODIFIED,
                        format!("{}: <Anchors> points to the control {} that is removed by the same edit", name(), u.target),
                    )
                    .with("kind", "invalid-xml")
                    .with("property", "Anchors")
                    .with("id", d.id));
                }
                continue;
            }
            let resolved = list_of_side(edge.side)
                .and_then(|k| resolve_removed_side(idx, removed, edge.target, k, edge.offset, &mut Vec::new()))
                .filter(|(n, owner)| {
                    n.target != d.id
                        && (n.target != 0 || idx.get(*owner).is_some_and(|o| container_holder(idx, o.parent) == container))
                })
                .map(|(n, _)| n);
            let new = match resolved {
                Some(n) => n,
                None => {
                    let (side, at) = match e {
                        0 => (0, rect[1]),
                        1 => (0, rect[3]),
                        2 => (2, rect[0]),
                        3 => (2, rect[2]),
                        _ => {
                            return Err(ExtractionError::new(
                                GENERATED_FILE_MODIFIED,
                                format!("{} is attached to a removed control and cannot be re-anchored", name()),
                            )
                            .with("kind", "unsupported-anchor")
                            .with("id", d.id))
                        }
                    };
                    AnchorEdge { target: 0, side, offset: at }
                }
            };
            let holder = if new.target == 0 { container } else { Holder::Geom(new.target) };
            let k = list_of_side(new.side).ok_or_else(|| layout_err(format!("{}: anchor side {} cannot be listed", name(), new.side)))?;
            let list = adds.entry((holder, k)).or_default();
            if !list.contains(&(d.id, e as i64)) {
                list.push((d.id, e as i64));
            }
            let target = g
                .child(6 + e)
                .and_then(|rec| rec.child(part))
                .ok_or_else(|| layout_err(format!("{}: edge record missing", name())))?;
            out.push(AppliedReplacement {
                control_id: 0,
                properties: Vec::new(),
                range: target.span.span,
                bytes: format_edge_target(&new).into_bytes(),
            });
        }
    }
    Ok(out)
}
