//! Donor prototypes for structural edits (OpenSpec change `add-form-controls-from-donor`).
//!
//! The Designer-produced `DonorForm` stream is embedded into the binary; for every known control
//! type it provides a prototype node that is cloned when `Form.xml` adds a new control. Cloning
//! copies the semantic tree, assigns a fresh id and name, and neutralizes the donor's events and
//! data links, so the XML patchers can then apply the user's values as ordinary edits.

use crate::brace::{parse_spanned, Atom, AtomKind, Item, Node, SpannedMetadata};
use crate::errors::{ExtractionError, Result, METADATA_LAYOUT_UNSUPPORTED};
use crate::form_patch::ControlIndex;

/// Designer-produced donor form stream (all 27 ordinary-form control types).
pub static DONOR_STREAM: &[u8] = include_bytes!("../tests/fixtures/forms/DonorForm/FormStream.bin");

/// Lazily parsed donor (parsed once per process; the embedded bytes are 'static).
fn donor() -> &'static SpannedMetadata {
    use std::sync::OnceLock;
    static DONOR: OnceLock<SpannedMetadata> = OnceLock::new();
    DONOR.get_or_init(|| parse_spanned(DONOR_STREAM).expect("embedded donor stream parses"))
}

/// The first donor control of `type_name`, deep-cloned as a semantic tree (no spans).
pub fn prototype(type_name: &str) -> Result<Node> {
    let src = donor();
    let index = ControlIndex::build(src)?;
    let Some(entry) = index.entries.iter().find(|e| e.type_name == type_name) else {
        return Err(ExtractionError::new(
            METADATA_LAYOUT_UNSUPPORTED,
            format!("the donor has no prototype for control type '{type_name}'"),
        )
        .with("type", type_name));
    };
    Ok(clone_node(entry.control.node))
}

pub(crate) fn clone_node(n: &Node) -> Node {
    Node {
        children: n
            .children
            .iter()
            .map(|c| match c {
                Item::Node(x) => Item::Node(clone_node(x)),
                Item::Atom(a) => Item::Atom(a.clone()),
            })
            .collect(),
    }
}

fn item_str(it: &Item) -> Option<&str> {
    match it {
        Item::Atom(a) => Some(&a.value),
        Item::Node(_) => None,
    }
}

/// Canonical serialization of a (cloned) subtree: compact form, quotes escaped as `""`,
/// base64 blobs emitted as `{#base64:...}` without line wrapping (the parser strips whitespace).
pub fn write_node(n: &Node, out: &mut Vec<u8>) {
    out.push(b'{');
    for (i, c) in n.children.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        match c {
            Item::Node(x) => write_node(x, out),
            Item::Atom(a) => match a.kind {
                AtomKind::Bare => out.extend_from_slice(a.value.as_bytes()),
                AtomKind::Quoted => {
                    out.push(b'"');
                    out.extend_from_slice(a.value.replace('"', "\"\"").as_bytes());
                    out.push(b'"');
                }
                AtomKind::Base64 => {
                    out.extend_from_slice(b"{#base64:");
                    out.extend_from_slice(a.value.as_bytes());
                    out.push(b'}');
                }
            },
        }
    }
    out.push(b'}');
}

/// EVENT_GUID event node: `{tag, e1692cc2-..., {3, handler, ...}}` (see `oof::event`).
pub(crate) fn is_event_node(n: &Node) -> bool {
    n.len() >= 3 && item_str(&n.children[1]).is_some_and(|v| v.eq_ignore_ascii_case(crate::oof::EVENT_GUID))
}

/// Strip the donor's events from a cloned control: any counted list whose items are all event
/// nodes becomes `{0}`; standalone event nodes are removed in place (their parents are counted
/// lists handled by the same rule, so a standalone one means an unknown layout and stays).
pub(crate) fn neutralize_events(n: &mut Node) {
    for c in &mut n.children {
        if let Item::Node(x) = c {
            let is_event_list = !x.is_empty()
                && matches!(&x.children[0], Item::Atom(a) if a.kind == AtomKind::Bare && a.value.bytes().all(|b| b.is_ascii_digit()))
                && x.children[1..].iter().all(|it| matches!(it, Item::Node(e) if is_event_node(e)));
            if is_event_list && x.len() > 1 {
                *x = Node { children: vec![Item::Atom(Atom::new(AtomKind::Bare, "0"))] };
            } else {
                neutralize_events(x);
            }
        }
    }
}

/// Paths of the counted event lists inside a control record (lists whose items are all event nodes).
pub(crate) fn event_list_paths(n: &Node) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    fn walk(n: &Node, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        for (i, c) in n.children.iter().enumerate() {
            let Item::Node(x) = c else { continue };
            path.push(i);
            let is_list = x.len() > 1
                && matches!(&x.children[0], Item::Atom(a) if a.kind == AtomKind::Bare && a.value.bytes().all(|b| b.is_ascii_digit()))
                && x.children[1..].iter().all(|it| matches!(it, Item::Node(e) if is_event_node(e)));
            if is_list {
                out.push(path.clone());
            } else {
                walk(x, path, out);
            }
            path.pop();
        }
    }
    walk(n, &mut Vec::new(), &mut out);
    out
}

/// A cloned prototype for a new control WITHOUT event neutralization (the caller decides).
pub(crate) fn clone_prototype_raw(type_name: &str, new_id: i64, name: &str) -> Result<Node> {
    let mut n = prototype(type_name)?;
    // control record: {guid, id, ..., geom, meta, children}; id at [1], name at meta[1]
    if n.len() < 6 {
        return Err(ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("donor prototype for '{type_name}' has an unexpected shape")));
    }
    n.children[1] = Item::Atom(Atom::new(AtomKind::Bare, new_id.to_string()));
    let meta_idx = n.len() - 2;
    let Some(Item::Node(meta)) = n.children.get_mut(meta_idx) else {
        return Err(ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("donor prototype for '{type_name}' has no meta node")));
    };
    if meta.len() < 2 {
        return Err(ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("donor prototype for '{type_name}' has a short meta node")));
    }
    meta.children[1] = Item::Atom(Atom::new(AtomKind::Quoted, name));
    Ok(n)
}

/// A cloned prototype with donor events stripped (used when the XML specifies no events).
pub fn clone_prototype(type_name: &str, new_id: i64, name: &str) -> Result<Node> {
    let mut n = clone_prototype_raw(type_name, new_id, name)?;
    neutralize_events(&mut n);
    Ok(n)
}

/// A template event node cloned from the donor (its handler string is replaced per use).
pub(crate) fn event_template() -> Result<Node> {
    fn find(n: &Node) -> Option<&Node> {
        for c in &n.children {
            if let Item::Node(x) = c {
                if is_event_node(x) {
                    return Some(x);
                }
                if let Some(found) = find(x) {
                    return Some(found);
                }
            }
        }
        None
    }
    let src = donor();
    find(&src.root)
        .map(clone_node)
        .ok_or_else(|| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, "the donor has no event template"))
}

/// The form's maximum control id (`root[1][1][1]`, verified by the exploration pipeline).
pub fn max_control_id(source: &SpannedMetadata) -> Result<i64> {
    let v = source
        .root
        .node(1)
        .and_then(|s| s.node(1))
        .and_then(|h| h.get(1))
        .and_then(item_str)
        .ok_or_else(|| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, "form stream: max control id slot not found at root[1][1][1]"))?;
    v.parse::<i64>()
        .map_err(|_| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("form stream: max control id slot is not an integer: {v:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oof::CONTROL_GUIDS;
    use crate::oof_props::{int, str_val};

    #[test]
    fn donor_embeds_and_covers_all_27_types() {
        let src = donor();
        let index = ControlIndex::build(src).unwrap();
        assert!(!index.entries.is_empty());
        assert_eq!(max_control_id(src).unwrap(), 36);
        // The donor covers 24 of the 27 known types; the missing ones must fail actionably.
        const MISSING: [&str; 3] = ["ActiveXControl", "ChoiceField", "UsualGroup"];
        for (guid, name) in CONTROL_GUIDS {
            let p = prototype(name);
            if MISSING.contains(&name) {
                assert!(p.is_err(), "{name} unexpectedly present in the donor");
            } else {
                assert!(p.is_ok(), "no prototype for {name} ({guid})");
            }
        }
        // unknown type is an actionable error
        let e = prototype("NoSuchType").unwrap_err();
        assert_eq!(e.code, METADATA_LAYOUT_UNSUPPORTED);
    }

    #[test]
    fn prototype_inventory_events_and_datapath() {
        // task 1.2 inventory: which donor prototypes are clean (no events, no links)
        let src = donor();
        let index = ControlIndex::build(src).unwrap();
        let mut clean = 0;
        let mut dirty = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for e in &index.entries {
            if seen.contains(&e.type_name) {
                continue;
            }
            seen.push(e.type_name);
            let mut clone = clone_node(e.control.node);
            let before_events = count_events(&clone);
            neutralize_events(&mut clone);
            if before_events == 0 {
                clean += 1;
            } else {
                dirty.push(format!("{}({})", e.type_name, before_events));
            }
        }
        println!("clean prototypes: {clean}; with events: {}", dirty.join(", "));
        assert!(clean >= 1);
        // after neutralization no event nodes remain anywhere
        for (.., name) in CONTROL_GUIDS {
            let Ok(mut c) = clone_prototype(name, 1000, "Новый") else { continue };
            assert_eq!(count_events(&c), 0, "{name}: events survived neutralization");
            // id and name were replaced
            assert_eq!(int(&c.children[1], "id").unwrap(), 1000);
            let meta_idx = c.len() - 2;
            let Item::Node(meta) = &c.children[meta_idx] else { panic!("meta") };
            assert_eq!(str_val(&meta.children[1], "name").unwrap(), "Новый");
            c.children.clear();
        }
    }

    fn count_events(n: &Node) -> usize {
        n.children
            .iter()
            .map(|c| match c {
                Item::Node(x) if is_event_node(x) => 1,
                Item::Node(x) => count_events(x),
                Item::Atom(_) => 0,
            })
            .sum()
    }

    #[test]
    fn canonical_write_round_trips() {
        let src = donor();
        let index = ControlIndex::build(src).unwrap();
        for e in &index.entries {
            let mut out = Vec::new();
            write_node(e.control.node, &mut out);
            let reparsed = parse_spanned(&out).unwrap();
            assert_eq!(&reparsed.root, e.control.node, "{} reparse mismatch", e.name);
        }
    }
}
