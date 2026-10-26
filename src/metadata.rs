//! EPF metadata inventory (port of the inventory half of `metadata.py`).

use serde_json::Value;
use std::collections::HashMap;

use crate::brace::{parse_metadata, Atom, AtomKind, Item, Node, BARE, QUOTED};
use crate::canonical::sha256_hex;
use crate::container::{inflate, looks_like_container, parse_container, Budget, Container, Limits};
use crate::errors::{
    ExtractionError, Result, MANAGED_FORM_UNSUPPORTED, METADATA_LAYOUT_UNSUPPORTED, METADATA_ORPHAN_ENTRY,
    MODULE_LOCATOR_MISSING, PAYLOAD_MISSING,
};

pub const ROOT_CLASS_ID: &str = "c3831ec8-d8d5-4f93-8a22-f9bfae07327f";
pub const FORMS_CLASS_ID: &str = "d5b0e5ed-256d-401c-9c36-f630cafd8a62";
pub const TEMPLATES_CLASS_ID: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
pub const SERVICE_ENTRIES: [&str; 4] = ["root", "version", "versions", "copyinfo"];
pub const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    pub locator: String,
    pub raw: Vec<u8>,
    pub sha256: String,
}

impl Payload {
    /// `sha256(raw)`: the stored digest when it is set (every production constructor fills it), else computed.
    pub fn digest(&self) -> String {
        if self.sha256.len() == 64 {
            self.sha256.clone()
        } else {
            sha256_hex(&self.raw)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormInfo {
    pub uuid: String,
    pub name: String,
    pub synonym: String,
    pub kind_code: i64,
    pub header_payload: Payload,
    pub layout_payload: Payload,
    pub module_payload: Payload,
    /// `OrdinaryForm` | `ManagedForm`
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInfo {
    pub uuid: String,
    pub name: String,
    pub synonym: String,
    pub type_code: i64,
    pub header_payload: Payload,
    pub content_payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessorInfo {
    pub object_uuid: String,
    pub name: String,
    pub synonym: String,
    pub default_form_uuid: String,
    pub root_metadata_entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessorInventory {
    pub processor: ProcessorInfo,
    pub object_module: Payload,
    pub forms: Vec<FormInfo>,
    pub templates: Vec<TemplateInfo>,
    /// `lowercase metadata uuid -> name` of the object, its attributes, tabular sections and
    /// their fields; used to resolve dotted `DataPath` steps when extracting ordinary forms.
    pub meta_lookup: crate::oof::MetaLookup,
    /// Scoped reverse table (name -> uuid) of the object's attributes and tabular sections.
    pub object_schema: crate::object_schema::ObjectSchema,
}

// ---------------------------------------------------------------- navigation

fn layout(message: &str) -> ExtractionError {
    ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, message)
}

fn at(path: &str, idx: usize) -> String {
    format!("{path}[{idx}]")
}

fn node_at<'a>(parent: &'a Node, idx: usize, path: &str) -> Result<&'a Node> {
    parent.node(idx).ok_or_else(|| layout("expected a list").with("path", at(path, idx)))
}

fn atom_at<'a>(parent: &'a Node, idx: usize, path: &str, kind: Option<AtomKind>) -> Result<&'a Atom> {
    let a = parent.atom(idx).ok_or_else(|| layout("expected an atom").with("path", at(path, idx)))?;
    if let Some(k) = kind {
        if a.kind != k {
            let name = match k {
                AtomKind::Bare => "bare",
                AtomKind::Quoted => "quoted",
                AtomKind::Base64 => "base64",
            };
            return Err(layout(&format!("expected a {name} atom")).with("path", at(path, idx)));
        }
    }
    Ok(a)
}

fn expect(parent: &Node, idx: usize, path: &str, value: &str) -> Result<()> {
    if atom_at(parent, idx, path, None)?.value != value {
        return Err(layout(&format!("expected '{value}'")).with("path", at(path, idx)));
    }
    Ok(())
}

fn int_at(parent: &Node, idx: usize, path: &str) -> Result<i64> {
    let v = &atom_at(parent, idx, path, Some(BARE))?.value;
    let digits = v.strip_prefix('-').unwrap_or(v);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(layout("expected an integer").with("path", at(path, idx)));
    }
    v.parse::<i64>().map_err(|_| layout("expected an integer").with("path", at(path, idx)))
}

pub fn is_uuid(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

fn uuid_at(parent: &Node, idx: usize, path: &str) -> Result<String> {
    let v = &atom_at(parent, idx, path, Some(BARE))?.value;
    if !is_uuid(v) {
        return Err(layout("expected a lowercase UUID").with("path", at(path, idx)));
    }
    Ok(v.clone())
}

/// `{3,{1,0,<uuid>},"<Name>",{1,"ru","<Synonym>"}|{0},...}` -> (uuid, name, synonym).
/// Recursively collect `uuid -> name` from every `{3,{1,0,<uuid>},"<Name>",{syn}}` descriptor
/// in the object metadata tree (the object itself, its attributes, tabular sections and their
/// fields). This is the `meta_lookup` source for EPF/ERF containers, which have no sibling
/// `<Object>.xml` (config objects use `formbin::read_meta_lookup` instead).
pub fn metadata_lookup_tree(node: &Node, out: &mut HashMap<String, String>) {
    if let Some((uid, name)) = descriptor_of(node) {
        out.entry(uid.to_string()).or_insert_with(|| name.to_string());
        return;
    }
    for ch in &node.children {
        if let Item::Node(ch) = ch {
            metadata_lookup_tree(ch, out);
        }
    }
}

/// `(uuid, name)` when `node` itself is a `{3,{1,0,<uuid>},"<Name>",{syn}...}` descriptor.
pub fn descriptor_of(node: &Node) -> Option<(&str, &str)> {
    if node.len() < 4 {
        return None;
    }
    let Item::Atom(tag) = &node.children[0] else { return None };
    let (Item::Node(ident), Item::Atom(name)) = (&node.children[1], &node.children[2]) else { return None };
    if tag.kind != BARE || tag.value != "3" || name.kind != QUOTED || ident.len() < 3 {
        return None;
    }
    match (&ident.children[0], &ident.children[1], &ident.children[2]) {
        (Item::Atom(a), Item::Atom(b), Item::Atom(u))
            if a.kind == BARE && a.value == "1" && b.kind == BARE && b.value == "0" && u.kind == BARE && is_uuid(&u.value) =>
        {
            Some((u.value.as_str(), name.value.as_str()))
        }
        _ => None,
    }
}

fn object_descriptor(node: &Node, path: &str) -> Result<(String, String, String)> {
    expect(node, 0, path, "3")?;
    let ident = node_at(node, 1, path)?;
    let ip = format!("{path}[1]");
    expect(ident, 0, &ip, "1")?;
    expect(ident, 1, &ip, "0")?;
    let uid = uuid_at(ident, 2, &ip)?;
    let name = atom_at(node, 2, path, Some(QUOTED))?.value.clone();
    let syn = node_at(node, 3, path)?;
    let sp = format!("{path}[3]");
    let syn_count = atom_at(syn, 0, &sp, Some(BARE))?.value.as_str();
    if syn_count != "0" && syn_count != "1" {
        return Err(layout("expected '1'").with("path", at(&sp, 0)));
    }
    let synonym = if syn_count == "1" && syn.len() >= 3 {
        atom_at(syn, 2, &sp, Some(QUOTED))?.value.clone()
    } else {
        String::new()
    };
    Ok((uid, name, synonym))
}

// ---------------------------------------------------------------- inventory

pub fn payload(locator: String, raw: Vec<u8>) -> Payload {
    let sha256 = sha256_hex(&raw);
    Payload { locator, raw, sha256 }
}

struct Reader<'a> {
    c: &'a Container,
    limits: &'a Limits,
    budget: Budget,
}

impl Reader<'_> {
    fn stream(&mut self, name: &str, missing_code: &'static str) -> Result<Vec<u8>> {
        let Some(e) = self.c.get(name) else {
            return Err(ExtractionError::new(missing_code, "entry is missing").with("entry", name));
        };
        inflate(&e.raw, self.limits, Some(&mut self.budget))
    }

    fn nested(&mut self, name: &str, missing_code: &'static str, allowed: Option<&[&str]>) -> Result<Container> {
        let data = self.stream(name, missing_code)?;
        if !looks_like_container(&data) {
            return Err(layout("entry is not a nested container").with("entry", name));
        }
        let b = std::mem::replace(&mut self.budget, Budget::new(self.limits));
        let inner = parse_container(&data, self.limits, self.c.depth + 1, Some(b))?;
        self.budget = inner.budget.clone();
        if let Some(allowed) = allowed {
            let extra: Vec<Value> = inner.names().into_iter().filter(|n| !allowed.contains(&n.as_str())).map(Value::from).collect();
            if !extra.is_empty() {
                return Err(ExtractionError::new(METADATA_ORPHAN_ENTRY, "unexpected entries in nested container")
                    .with("entry", name)
                    .with("entries", Value::Array(extra)));
            }
        }
        Ok(inner)
    }

    fn nested_payload(&self, parent: &str, inner: &Container, name: &str, code: &'static str) -> Result<Payload> {
        let Some(e) = inner.get(name) else {
            return Err(ExtractionError::new(code, "entry is missing").with("entry", format!("{parent}/{name}")));
        };
        Ok(payload(format!("container:{}/entry:{parent}/entry:{name}", self.c.depth), e.raw.clone()))
    }
}

/// Build the qualified inventory (32-bit container, ordinary forms) or fail closed.
pub fn discover(
    container: &Container,
    limits: &Limits,
    budget: Option<Budget>,
    allow_managed: bool,
) -> Result<ProcessorInventory> {
    let mut r = Reader { c: container, limits, budget: budget.unwrap_or_else(|| container.budget.clone()) };
    let d = container.depth;

    // root -> root metadata entry name
    let root = parse_metadata(&r.stream("root", PAYLOAD_MISSING)?)?;
    expect(&root, 0, "root", "2")?;
    let root_meta_name = uuid_at(&root, 1, "root")?;
    if root.len() != 3 || atom_at(&root, 2, "root", None)?.value.as_str() != "" {
        return Err(layout("unexpected root entry shape").with("path", "root"));
    }

    // root metadata
    let meta = parse_metadata(&r.stream(&root_meta_name, PAYLOAD_MISSING)?)?;
    let mut p = root_meta_name.clone();
    expect(&meta, 0, &p, "1")?;
    let rf = node_at(&meta, 1, &p)?;
    if rf.len() != 1 || atom_at(rf, 0, &format!("{p}[1]"), None)?.value != root_meta_name {
        return Err(layout("root metadata self-reference mismatch").with("path", format!("{p}[1]")));
    }
    expect(&meta, 2, &p, "1")?;
    let cls = node_at(&meta, 3, &p)?;
    p.push_str("[3]");
    expect(cls, 0, &p, ROOT_CLASS_ID)?;
    let props = node_at(cls, 1, &p)?;
    expect(props, 0, &format!("{p}[1]"), "1")?;
    let head = node_at(props, 1, &format!("{p}[1]"))?;
    let hp = format!("{p}[1][1]");
    expect(head, 0, &hp, "4")?;
    let desc_holder = node_at(head, 3, &hp)?;
    expect(desc_holder, 0, &format!("{hp}[3]"), "0")?;
    let (obj_uuid, obj_name, obj_syn) =
        object_descriptor(node_at(desc_holder, 1, &format!("{hp}[3]"))?, &format!("{hp}[3][1]"))?;
    let default_form = uuid_at(head, 4, &hp)?;

    let mut forms_uuids: Option<Vec<String>> = None;
    let mut templates_uuids: Option<Vec<String>> = None;
    for idx in 2..props.len() {
        let Item::Node(child) = &props.children[idx] else { continue };
        let Some(Item::Atom(first)) = child.children.first() else { continue };
        if first.kind != BARE {
            continue;
        }
        if first.value == FORMS_CLASS_ID || first.value == TEMPLATES_CLASS_ID {
            let gp = format!("{p}[1][{idx}]");
            let count = int_at(child, 1, &gp)?;
            if count < 0 || count != child.len() as i64 - 2 {
                return Err(layout("group count mismatch").with("path", gp));
            }
            let mut ids = Vec::new();
            for k in 2..child.len() {
                ids.push(uuid_at(child, k, &gp)?);
            }
            let mut uniq = ids.clone();
            uniq.sort();
            uniq.dedup();
            if uniq.len() != ids.len() {
                return Err(layout("duplicate uuid in group").with("path", gp));
            }
            if first.value == FORMS_CLASS_ID {
                if forms_uuids.is_some() {
                    return Err(layout("duplicate Forms group").with("path", gp));
                }
                forms_uuids = Some(ids);
            } else {
                if templates_uuids.is_some() {
                    return Err(layout("duplicate Templates group").with("path", gp));
                }
                templates_uuids = Some(ids);
            }
        }
    }
    let forms_uuids = forms_uuids.ok_or_else(|| layout("Forms group is missing").with("path", p.clone()))?;
    let templates_uuids = templates_uuids.ok_or_else(|| layout("Templates group is missing").with("path", p.clone()))?;
    if default_form != ZERO_UUID && !forms_uuids.contains(&default_form) {
        return Err(layout("default form is not listed in Forms").with("uuid", default_form));
    }

    let is_omitted_empty = |entry_name: &str| -> bool {
        if container.get(entry_name).is_some() {
            return false;
        }
        let Some(ve) = container.get("versions") else { return false };
        let Ok(vraw) = inflate(&ve.raw, limits, None) else { return false };
        let Ok(vnode) = parse_metadata(&vraw) else { return false };
        if vnode.len() <= 4 {
            return false;
        }
        let mut any_listed = false;
        for c in vnode.children.iter().skip(3) {
            if let Item::Atom(a) = c {
                if a.kind == QUOTED {
                    any_listed = true;
                    if a.value == entry_name {
                        return false;
                    }
                }
            }
        }
        any_listed
    };

    // object module
    let obj_entry = format!("{obj_uuid}.0");
    let object_module = if is_omitted_empty(&obj_entry) {
        payload(format!("container:{d}/entry:{obj_entry}/entry:text"), Vec::new())
    } else {
        let inner = r.nested(&obj_entry, MODULE_LOCATOR_MISSING, Some(&["text", "info"]))?;
        r.nested_payload(&obj_entry, &inner, "text", MODULE_LOCATOR_MISSING)?
    };

    // forms
    let mut forms: Vec<FormInfo> = Vec::new();
    for fu in &forms_uuids {
        let hdr = r.stream(fu, PAYLOAD_MISSING)?;
        let t = parse_metadata(&hdr)?;
        let hp = fu.clone();
        expect(&t, 0, &hp, "1")?;
        let a = node_at(&t, 1, &hp)?;
        expect(a, 0, &format!("{hp}[1]"), "1")?;
        let b = node_at(a, 1, &format!("{hp}[1]"))?;
        let kind_code = int_at(b, 0, &format!("{hp}[1][1]"))?;
        let c = node_at(b, 1, &format!("{hp}[1][1]"))?;
        expect(c, 0, &format!("{hp}[1][1][1]"), "13")?;
        let (uid, name, syn) =
            object_descriptor(node_at(c, 1, &format!("{hp}[1][1][1]"))?, &format!("{hp}[1][1][1][1]"))?;
        if &uid != fu {
            return Err(layout("form header uuid differs from listed uuid").with("listed", fu.clone()).with("header", uid));
        }
        let entry = format!("{fu}.0");
        if kind_code != 0 {
            if !allow_managed {
                return Err(ExtractionError::new(MANAGED_FORM_UNSUPPORTED, "only ordinary forms are supported")
                    .with("uuid", fu.clone())
                    .with("kind_code", kind_code));
            }
            let mut module_payload = payload(format!("container:{d}/entry:{entry}"), Vec::new());
            if container.get(&entry).is_some() {
                let data = r.stream(&entry, PAYLOAD_MISSING)?;
                if looks_like_container(&data) {
                    let b = std::mem::replace(&mut r.budget, Budget::new(limits));
                    let inner = parse_container(&data, limits, d + 1, Some(b))?;
                    r.budget = inner.budget.clone();
                    if let Some(m) = inner.get("module") {
                        module_payload = payload(format!("container:{d}/entry:{entry}/entry:module"), m.raw.clone());
                    }
                }
            }
            forms.push(FormInfo {
                uuid: fu.clone(),
                name,
                synonym: syn,
                kind_code,
                header_payload: payload(format!("container:{d}/entry:{fu}"), hdr),
                layout_payload: payload(format!("container:{d}/entry:{entry}"), Vec::new()),
                module_payload,
                kind: "ManagedForm".into(),
            });
            continue;
        }
        let fin = r.nested(&entry, MODULE_LOCATOR_MISSING, Some(&["form", "module"]))?;
        let layout_p = r.nested_payload(&entry, &fin, "form", PAYLOAD_MISSING)?;
        let module = r.nested_payload(&entry, &fin, "module", MODULE_LOCATOR_MISSING)?;
        forms.push(FormInfo {
            uuid: fu.clone(),
            name,
            synonym: syn,
            kind_code,
            header_payload: payload(format!("container:{d}/entry:{fu}"), hdr),
            layout_payload: layout_p,
            module_payload: module,
            kind: "OrdinaryForm".into(),
        });
    }

    // templates
    let mut templates: Vec<TemplateInfo> = Vec::new();
    for tu in &templates_uuids {
        let hdr = r.stream(tu, PAYLOAD_MISSING)?;
        let t = parse_metadata(&hdr)?;
        let hp = tu.clone();
        expect(&t, 0, &hp, "1")?;
        let a = node_at(&t, 1, &hp)?;
        expect(a, 0, &format!("{hp}[1]"), "2")?;
        let type_code = int_at(a, 1, &format!("{hp}[1]"))?;
        let (uid, name, syn) = object_descriptor(node_at(a, 2, &format!("{hp}[1]"))?, &format!("{hp}[1][2]"))?;
        if &uid != tu {
            return Err(layout("template header uuid differs from listed uuid").with("listed", tu.clone()).with("header", uid));
        }
        let tmpl_entry = format!("{tu}.0");
        let content = if is_omitted_empty(&tmpl_entry) {
            Vec::new()
        } else {
            r.stream(&tmpl_entry, PAYLOAD_MISSING)?
        };
        templates.push(TemplateInfo {
            uuid: tu.clone(),
            name,
            synonym: syn,
            type_code,
            header_payload: payload(format!("container:{d}/entry:{tu}"), hdr),
            content_payload: payload(format!("container:{d}/entry:{tu}.0"), content),
        });
    }

    // orphan check: every top-level entry must be accounted for
    let mut accounted: Vec<String> = SERVICE_ENTRIES.iter().map(|s| s.to_string()).collect();
    accounted.push(root_meta_name.clone());
    accounted.push(obj_entry.clone());
    for fu in &forms_uuids {
        accounted.push(fu.clone());
        accounted.push(format!("{fu}.0"));
    }
    for tu in &templates_uuids {
        accounted.push(tu.clone());
        accounted.push(format!("{tu}.0"));
    }
    let orphans: Vec<Value> = container.names().into_iter().filter(|n| !accounted.contains(n)).map(Value::from).collect();
    if !orphans.is_empty() {
        return Err(ExtractionError::new(METADATA_ORPHAN_ENTRY, "unaccounted top-level entries")
            .with("entries", Value::Array(orphans)));
    }

    let mut meta_lookup = crate::oof::MetaLookup::new();
    metadata_lookup_tree(&meta, &mut meta_lookup);
    let object_schema = crate::object_schema::ObjectSchema::from_meta_tree(&meta);

    Ok(ProcessorInventory {
        processor: ProcessorInfo {
            object_uuid: obj_uuid,
            name: obj_name,
            synonym: obj_syn,
            default_form_uuid: default_form,
            root_metadata_entry: root_meta_name,
        },
        object_module,
        forms,
        templates,
        meta_lookup,
        object_schema,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_shape() {
        assert!(is_uuid("710723d8-83e7-48ae-9996-7a45571ed0df"));
        assert!(!is_uuid("710723D8-83e7-48ae-9996-7a45571ed0df"));
        assert!(!is_uuid("710723d8-83e7-48ae-9996"));
    }

    #[test]
    fn donor_epf_inventory() {
        let epf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/donor.epf");
        if !epf.is_file() {
            return;
        }
        let data = std::fs::read(epf).unwrap();
        let limits = Limits::default();
        let c = parse_container(&data, &limits, 0, None).unwrap();
        let inv = discover(&c, &limits, None, false).unwrap();
        assert_eq!(inv.forms.len(), 1);
        assert_eq!(inv.templates.len(), 4);
        assert_eq!(inv.processor.name, "ВнешняяОбработка1");
    }
}
