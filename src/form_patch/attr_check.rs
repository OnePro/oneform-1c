//! Structural check of the attribute section of a form stream (`root[2]`), independent of the form decoder.
//!
//! The platform rejects a whole form («Ошибка формата потока») when the attribute section is malformed, while
//! our own decoder may still read it. The check looks at the raw brace tree only: the stream version, the record
//! shape that the version dictates, counts against list lengths, unique ids below the next-id counter, and the
//! links (`root[2][3]`: control id -> data path) that must point to an existing attribute.

use std::collections::HashSet;

use crate::brace::{parse_spanned, Item, Node, QUOTED};

fn int_atom(i: Option<&Item>) -> Option<i64> {
    match i {
        Some(Item::Atom(a)) if a.kind != QUOTED => a.value.trim().parse().ok(),
        _ => None,
    }
}

/// `{N}`: a node with one integer atom.
fn id_node(i: Option<&Item>) -> Option<i64> {
    match i {
        Some(Item::Node(n)) if n.len() == 1 => int_atom(n.get(0)),
        _ => None,
    }
}

/// Number of items of the attribute record of a stream of this version (`None`: the version is not known).
pub fn attribute_record_len(version: i64) -> Option<usize> {
    match version {
        19..=26 => Some(5),
        27 => Some(6),
        _ => None,
    }
}

/// Problems of the attribute section of an already parsed stream; empty when it is consistent.
pub fn attribute_section_issues(root: &Node) -> Vec<String> {
    let mut out = Vec::new();
    let Some(version) = int_atom(root.get(0)) else {
        return vec!["the stream has no version at root[0]".to_string()];
    };
    let Some(rec_len) = attribute_record_len(version) else {
        return vec![format!("unknown stream version {version}")];
    };
    let Some(sec) = root.node(2) else {
        return vec!["root[2] (the attribute section) is not a list".to_string()];
    };
    if id_node(sec.get(0)).is_none() {
        out.push("root[2][0]: the main attribute id is not {N}".to_string());
    }
    let next_id = int_atom(sec.get(1));
    if next_id.is_none() {
        out.push("root[2][1]: the next attribute id is not an integer".to_string());
    }
    let Some(list) = sec.node(2) else {
        out.push("root[2][2]: the attribute list is missing".to_string());
        return out;
    };
    match int_atom(list.get(0)) {
        Some(n) if n >= 0 && n as usize == list.len() - 1 => {}
        other => out.push(format!("root[2][2]: the count {other:?} differs from the {} records", list.len() - 1)),
    }
    let mut ids: HashSet<i64> = HashSet::new();
    let mut max_id: Option<i64> = None;
    for (n, item) in list.children.iter().enumerate().skip(1) {
        let Some(rec) = item.as_node() else {
            out.push(format!("attribute #{n}: the record is not a list"));
            continue;
        };
        if rec.len() != rec_len {
            out.push(format!("attribute #{n}: {} items in a record of a version-{version} stream (expected {rec_len})", rec.len()));
            continue;
        }
        let Some(id) = id_node(rec.get(0)) else {
            out.push(format!("attribute #{n}: the id is not {{N}}"));
            continue;
        };
        if id < 0 || !ids.insert(id) {
            out.push(format!("attribute #{n}: the id {id} is negative or repeated"));
        }
        max_id = Some(max_id.map_or(id, |m| m.max(id)));
        let name_at = rec_len - 2;
        for flag in 1..name_at {
            if int_atom(rec.get(flag)).is_none_or(|v| v < 0) {
                out.push(format!("attribute id {id}: item {flag} is not a non-negative integer"));
            }
        }
        if !matches!(rec.get(name_at), Some(Item::Atom(a)) if a.kind == QUOTED && !a.value.is_empty()) {
            out.push(format!("attribute id {id}: the name is not a quoted string"));
        }
        let pattern_ok = matches!(rec.node(rec_len - 1).and_then(|p| p.atom(0)), Some(a) if a.kind == QUOTED && a.value == "Pattern");
        if !pattern_ok {
            out.push(format!("attribute id {id}: the last item is not a {{\"Pattern\",...}} list"));
        }
    }
    if let (Some(next), Some(max)) = (next_id, max_id) {
        if next <= max {
            out.push(format!("root[2][1]: the next attribute id {next} is not above the greatest attribute id {max}"));
        }
    }
    if let Some(links) = sec.node(3) {
        match int_atom(links.get(0)) {
            Some(n) if n >= 0 && n as usize == links.len() - 1 => {}
            other => out.push(format!("root[2][3]: the count {other:?} differs from the {} links", links.len() - 1)),
        }
        let mut owners: HashSet<i64> = HashSet::new();
        for (n, item) in links.children.iter().enumerate().skip(1) {
            let Some(row) = item.as_node() else {
                out.push(format!("link #{n}: the row is not a list"));
                continue;
            };
            let Some(owner) = int_atom(row.get(0)) else {
                out.push(format!("link #{n}: the control id is not an integer"));
                continue;
            };
            if !owners.insert(owner) {
                out.push(format!("link #{n}: the control {owner} has two links"));
            }
            if let Some(target) = row.node(1).and_then(|b| id_node(b.get(1))) {
                // a negative id is a standard attribute of the form (the list `-2`, the tree `-1`...), not a listed one
                if target >= 0 && !ids.contains(&target) {
                    out.push(format!("link of the control {owner}: the attribute {target} does not exist"));
                }
            }
        }
    }
    out
}

/// Problems of the attribute section of a stream; a stream that cannot be parsed is one problem.
pub fn stream_attribute_issues(stream: &[u8]) -> Vec<String> {
    match parse_spanned(stream) {
        Ok(s) => attribute_section_issues(&s.root),
        Err(e) => vec![format!("the stream cannot be parsed: {}", e.message)],
    }
}
