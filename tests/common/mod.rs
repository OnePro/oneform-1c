//! Helpers shared by the ordinary-form integration tests.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;

use oneform::metadata::{FormInfo, Payload};

/// `formbin.read_meta_lookup`: `lowercase uuid -> Name` of every `Attribute` and `TabularSection`
/// (nested ones included) and `@kind -> <object kind>`; `{}` when the file is missing or unparsable.
pub fn read_meta_lookup(path: &Path) -> HashMap<String, String> {
    let mut lookup = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return lookup };
    let Ok(doc) = roxmltree::Document::parse(&text) else { return lookup };
    if let Some(obj) = doc.root_element().children().find(|c| c.is_element()) {
        lookup.insert("@kind".to_string(), obj.tag_name().name().to_string());
    }
    for e in doc.descendants().filter(|n| n.is_element()) {
        if !matches!(e.tag_name().name(), "Attribute" | "TabularSection") {
            continue;
        }
        let Some(props) = child(e, "Properties") else { continue };
        let Some(name) = child(props, "Name") else { continue };
        let uid = e.attribute("uuid").unwrap_or("").trim().to_lowercase();
        let text = name.first_child().filter(|c| c.is_text()).and_then(|c| c.text()).unwrap_or("").trim();
        if !text.is_empty() && is_uuid(&uid) {
            lookup.insert(uid, text.to_string());
        }
    }
    lookup
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

fn is_uuid(s: &str) -> bool {
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

pub fn form_info(uuid: &str, name: &str, layout: &[u8]) -> FormInfo {
    let payload = |locator: &str, raw: &[u8]| Payload { locator: locator.into(), raw: raw.to_vec(), sha256: String::new() };
    FormInfo {
        uuid: uuid.into(),
        name: name.into(),
        synonym: String::new(),
        kind_code: 0,
        header_payload: payload("h", b""),
        layout_payload: payload("f", layout),
        module_payload: payload("m", b""),
        kind: "OrdinaryForm".into(),
    }
}
