//! 1C Configuration XML dump support: ordinary forms stored as `Ext/Form.bin` (port of `formbin.py`).
//!
//! A configuration object (`Documents/X`, `Catalogs/Y`, `CommonForms/Z`, ...) with ordinary forms keeps each form in
//! `Forms/<FormName>/Ext/Form.bin` (an uncompressed 32-bit container with the entries `form` and `module`). The
//! projection `<ObjDir>/src/` follows the layout of an EPF `src/` with the same ownership, publication and pack
//! rules. The object module is only referenced (never copied into `src/`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::canonical::{dumps_json, dumps_with, sha256_hex, SORTED_SPACED};
use crate::cli_epf::{plan_lines, plural, Outcome};
use crate::config::load_json_bytes;
use crate::container::{parse_container, serialize_container, EntryOut, Limits};
use crate::discovery::{is_link, scan_dir, DirEnt};
use crate::encoding::{decode_payload, decode_payload_hashed};
use crate::errors::{
    diagnostic, ExtractionError, Result, FORM_PREVIEW_FAILED, FORM_PROJECTION_UNSUPPORTED, GENERATED_FILE_MODIFIED,
    IO_ERROR, OUTPUT_NOT_OWNED, PAYLOAD_MISSING, PUBLISH_VERIFY_FAILED, SOURCE_CHANGED,
};
use crate::form_basic::{build_form_definition, MODULE_NAME, STREAM_PATH};
use crate::form_patch::with_type_lookup;
use crate::manifest::{module_record, sort_diagnostics, GENERATOR_VERSION, MANIFEST_PATH};
use crate::metadata::{FormInfo, Payload};
use crate::metaref::ConfigTypeLookup;
use crate::object_schema::{with_object_schema, ObjectSchema};
use crate::naming::{is_uuid, resolve_1c_aliases, safe_join};
use crate::oof::{apply_projection, build_oof_with_types, MetaLookup, FORM_XML_NAME, META_KIND_KEY};
use crate::pack::{atomic_write, encode_text, plan_layout_patch, LayoutPatch, PACK_PACKED, PACK_SRC_ONLY, PACK_UNCHANGED, READ_ONLY_ROLES};
use crate::preview::{render_form_preview_html, FORM_PREVIEW_NAME};
use crate::stage::{ExtractionResult, STATUS_PARTIAL};
use crate::transaction::{
    check_ownership, manifest_hashes, plan, processor_lock, publish, read_tree, recover, snapshot_epf,
    verify_source_unchanged, Snapshot, Tree,
};

pub use crate::preview::OBJECT_CLASS_TITLES;

pub const OBJECT_MODULE_NAMES: [&str; 4] =
    ["ObjectModule.bsl", "RecordSetModule.bsl", "ValueManagerModule.bsl", "ManagerModule.bsl"];
pub const FORM_BIN_NAME: &str = "Form.bin";
pub const SKIP_DIRS: [&str; 5] = [".git", ".venv", ".extract-work", ".oneform", "src"];

/// Form name -> `Form.bin` bytes.
pub type Sources = BTreeMap<String, Vec<u8>>;

/// A configuration object (or common form) that owns at least one `Ext/Form.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigObject {
    /// POSIX path of the object directory relative to the workspace root.
    pub rel: String,
    pub object_class: String,
    /// Form directory names; a common form has exactly one (its own name).
    pub forms: Vec<String>,
    pub common: bool,
}

pub fn object_dir(root: &Path, obj: &ConfigObject) -> PathBuf {
    if obj.rel.is_empty() || obj.rel == "." {
        return root.to_path_buf();
    }
    let mut p = root.to_path_buf();
    for part in obj.rel.split('/') {
        p.push(part);
    }
    p
}

/// Path of a form's `Form.bin` relative to the object directory.
pub fn form_bin_rel(obj: &ConfigObject, name: &str) -> String {
    if obj.common {
        "Ext/Form.bin".to_string()
    } else {
        format!("Forms/{name}/Ext/Form.bin")
    }
}

pub fn src_dir(root: &Path, obj: &ConfigObject) -> PathBuf {
    object_dir(root, obj).join("src")
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `os.walk(root, followlinks=False)` with sorted, pruned subdirectories: `visit(dir, non_dir_entries)`.
pub fn walk_dirs(root: &Path, skip: &dyn Fn(&str) -> bool, visit: &mut dyn FnMut(&Path, &[DirEnt])) {
    let Ok(entries) = scan_dir(root) else { return };
    let (dirs, files): (Vec<DirEnt>, Vec<DirEnt>) = entries.into_iter().partition(|e| e.is_dir);
    visit(root, &files);
    for d in dirs {
        if skip(&d.name) || d.link {
            continue;
        }
        walk_dirs(&d.path, skip, visit);
    }
}

/// All objects under `root` that have `Forms/<name>/Ext/Form.bin` (or `CommonForms/<name>/Ext/Form.bin`).
pub fn discover_config_objects(root: &Path) -> Vec<ConfigObject> {
    let mut found: BTreeMap<String, (String, BTreeSet<String>, bool)> = BTreeMap::new();
    let skip = |d: &str| SKIP_DIRS.contains(&d);
    walk_dirs(root, &skip, &mut |base, files| {
        if name_of(base) != "Ext" || !files.iter().any(|f| f.name == FORM_BIN_NAME && !f.link) {
            return;
        }
        let Some(form_dir) = base.parent() else { return };
        let Some(parent) = form_dir.parent() else { return };
        let (obj_dir, name, common) = match name_of(parent).as_str() {
            "Forms" => match parent.parent() {
                Some(o) => (o.to_path_buf(), name_of(form_dir), false),
                None => return,
            },
            "CommonForms" => (form_dir.to_path_buf(), name_of(form_dir), true),
            _ => return,
        };
        let Ok(rel_path) = obj_dir.strip_prefix(root) else { return };
        let rel = rel_path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        let rel = if rel.is_empty() { ".".to_string() } else { rel };
        let cls = if common { "CommonForms".to_string() } else { obj_dir.parent().map(name_of).unwrap_or_default() };
        found.entry(rel).or_insert_with(|| (cls, BTreeSet::new(), common)).1.insert(name);
    });
    found
        .into_iter()
        .map(|(rel, (object_class, names, common))| ConfigObject {
            rel,
            object_class,
            forms: names.into_iter().collect(),
            common,
        })
        .collect()
}

// --------------------------------------------------------------------------- metadata XML

fn read_xml_text(path: &Path) -> Option<String> {
    let raw = fs::read(path).ok()?;
    let text = String::from_utf8(raw).ok()?;
    let text = text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text);
    Some(text)
}

fn open_doc(text: &str) -> Option<roxmltree::Document<'_>> {
    roxmltree::Document::parse_with_options(text, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .ok()
}

type XNode<'a, 'i> = roxmltree::Node<'a, 'i>;

fn child<'a, 'i>(n: XNode<'a, 'i>, name: &str) -> Option<XNode<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

/// ElementTree `.text`: the text before the first child element.
fn et_text<'a>(n: XNode<'a, '_>) -> &'a str {
    match n.first_child() {
        Some(c) if c.is_text() => c.text().unwrap_or(""),
        _ => "",
    }
}

/// `{uuid, name, synonym, formType}` of a metadata XML file; empty strings when missing or unparsable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub uuid: String,
    pub name: String,
    pub synonym: String,
    pub form_type: String,
}

/// `_read_meta`: `None` when the file is missing or unparsable (Python `{}`).
pub fn read_meta(path: &Path) -> Option<Meta> {
    let text = read_xml_text(path)?;
    meta_of_doc(&open_doc(&text)?)
}

fn meta_of_doc(doc: &roxmltree::Document<'_>) -> Option<Meta> {
    let root = doc.root_element();
    let obj = if root.tag_name().name() == "MetaDataObject" { root.children().find(|c| c.is_element())? } else { root };
    let mut meta = Meta { uuid: obj.attribute("uuid").unwrap_or("").to_string(), ..Default::default() };
    let Some(props) = child(obj, "Properties") else { return Some(meta) };
    meta.name = child(props, "Name").map(|n| et_text(n).trim().to_string()).unwrap_or_default();
    meta.form_type = child(props, "FormType").map(|n| et_text(n).trim().to_string()).unwrap_or_default();
    if let Some(syn) = child(props, "Synonym") {
        let mut items: BTreeMap<String, String> = BTreeMap::new();
        for item in syn.children().filter(|c| c.is_element()) {
            if let (Some(lang), Some(content)) = (child(item, "lang"), child(item, "content")) {
                items.insert(et_text(lang).trim().to_string(), et_text(content).to_string());
            }
        }
        meta.synonym = match items.get("ru").filter(|s| !s.is_empty()) {
            Some(ru) => ru.clone(),
            None => items.values().next().cloned().unwrap_or_default(),
        };
    }
    Some(meta)
}

/// `lowercase uuid -> Name` of every `Attribute`, `TabularSection`, `Dimension`, `Resource`, `AddressingAttribute`,
/// `AccountingFlag`, `ExtDimensionAccountingFlag` and journal `Column` (nested ones included) and of the object itself
/// when it is a constant or a register, plus `META_KIND_KEY -> <object kind>` (the element under `MetaDataObject`,
/// e.g. `Catalog`).
pub fn read_meta_lookup(path: &Path) -> MetaLookup {
    let Some(text) = read_xml_text(path) else { return HashMap::new() };
    open_doc(&text).map(|doc| lookup_of_doc(&doc)).unwrap_or_default()
}

/// [`ObjectSchema`] of the object XML at `path`; `None` when the file is missing or is not a metadata object.
pub fn read_object_schema(path: &Path) -> Option<ObjectSchema> {
    read_object_schema_with(path, None)
}

/// [`read_object_schema`] with the reference types of the columns of tabular sections resolved by `lookup`.
pub fn read_object_schema_with(path: &Path, lookup: Option<&ConfigTypeLookup>) -> Option<ObjectSchema> {
    ObjectSchema::from_object_xml_with(&read_xml_text(path)?, lookup)
}

/// `(_read_meta(path), read_meta_lookup(path))` from one parse of the file.
fn read_meta_and_lookup(path: &Path) -> (Meta, MetaLookup) {
    let Some(text) = read_xml_text(path) else { return Default::default() };
    match open_doc(&text) {
        Some(doc) => (meta_of_doc(&doc).unwrap_or_default(), lookup_of_doc(&doc)),
        None => Default::default(),
    }
}

fn lookup_of_doc(doc: &roxmltree::Document<'_>) -> MetaLookup {
    let mut lookup = HashMap::new();
    let root = doc.root_element();
    if root.tag_name().name() == "MetaDataObject" {
        if let Some(obj) = root.children().find(|c| c.is_element()) {
            lookup.insert(META_KIND_KEY.to_string(), obj.tag_name().name().to_string());
        }
    }
    for e in doc.descendants().filter(|n| n.is_element()) {
        let tag = e.tag_name().name();
        let is_member = matches!(
            tag,
            "Attribute"
                | "TabularSection"
                | "Dimension"
                | "Resource"
                | "AddressingAttribute"
                | "AccountingFlag"
                | "ExtDimensionAccountingFlag"
                | "Column"
        );
        let is_object = e.parent().is_some_and(|p| p.tag_name().name() == "MetaDataObject")
            && matches!(
                tag,
                "Constant" | "InformationRegister" | "AccumulationRegister" | "AccountingRegister" | "CalculationRegister"
            );
        if !is_member && !is_object {
            continue;
        }
        let Some(name) = child(e, "Properties").and_then(|p| child(p, "Name")) else { continue };
        let uid = e.attribute("uuid").unwrap_or("").trim().to_lowercase();
        let text = et_text(name).trim();
        if !text.is_empty() && is_uuid(&uid) {
            lookup.insert(uid, text.to_string());
        }
    }
    lookup
}

/// A valid lowercase uuid, else a name-based (uuid5 / `NAMESPACE_OID`) one derived from `fallback_path`.
pub fn uuid_or_fallback(value: &str, fallback_path: &str) -> String {
    let value = value.trim().to_lowercase();
    if is_uuid(&value) {
        value
    } else {
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, fallback_path.as_bytes()).hyphenated().to_string()
    }
}

pub fn find_object_module(obj_dir: &Path) -> Option<PathBuf> {
    OBJECT_MODULE_NAMES
        .iter()
        .map(|n| obj_dir.join("Ext").join(n))
        .find(|p| p.is_file() && !is_link(p))
}

/// Canonical UTF-8/LF bytes of a module; undecodable modules degrade to a lossy view.
pub fn read_module_text(raw: &[u8], key: &str) -> Vec<u8> {
    match decode_payload(raw, key, "auto") {
        Ok(d) => d.canonical,
        Err(_) => {
            let text = String::from_utf8_lossy(raw);
            let text = text.trim_start_matches('\u{feff}');
            text.replace("\r\n", "\n").replace('\r', "\n").into_bytes()
        }
    }
}

// --------------------------------------------------------------------------- staging

fn bin_path(root: &Path, obj: &ConfigObject, name: &str) -> PathBuf {
    let mut p = object_dir(root, obj);
    for part in form_bin_rel(obj, name).split('/') {
        p.push(part);
    }
    p
}

fn read_sources(root: &Path, obj: &ConfigObject) -> Result<Sources> {
    obj.forms.iter().map(|n| Ok((n.clone(), fs::read(bin_path(root, obj, n))?))).collect()
}

fn meta_of(path: &Path) -> Meta {
    read_meta(path).unwrap_or_default()
}

fn quote(s: &str) -> Result<String> {
    dumps_with(&Value::String(s.to_string()), &SORTED_SPACED)
}

/// Build the complete `<ObjDir>/src/` tree in memory (`sources`: form name -> `Form.bin` bytes).
///
/// `meta_lookup` (metadata uuid -> name) defaults to the attributes of `<ObjDir>.xml`.
pub fn stage_config_object(
    root: &Path,
    obj: &ConfigObject,
    sources: Option<&Sources>,
    oof_path: Option<&str>,
    limits: Option<&Limits>,
    meta_lookup: Option<&MetaLookup>,
) -> Result<ExtractionResult> {
    stage_config_object_hashed(root, obj, sources, None, oof_path, limits, meta_lookup)
}

/// [`stage_config_object`] with the known `sha256` of (some of) the `sources` (form name -> hex digest).
fn stage_config_object_hashed(
    root: &Path,
    obj: &ConfigObject,
    sources: Option<&Sources>,
    source_shas: Option<&BTreeMap<String, String>>,
    oof_path: Option<&str>,
    limits: Option<&Limits>,
    meta_lookup: Option<&MetaLookup>,
) -> Result<ExtractionResult> {
    let default_limits = Limits::default();
    let limits = limits.unwrap_or(&default_limits);
    let base = object_dir(root, obj);
    let owned_sources;
    let sources = match sources {
        Some(s) => s,
        None => {
            owned_sources = read_sources(root, obj)?;
            &owned_sources
        }
    };
    let meta_xml = base.parent().map(|p| p.join(format!("{}.xml", name_of(&base))));
    let (ometa, owned_lookup) = match (meta_lookup, meta_xml.as_deref()) {
        (_, None) => Default::default(),
        (Some(_), Some(x)) => (meta_of(x), MetaLookup::new()),
        (None, Some(x)) => read_meta_and_lookup(x),
    };
    let meta_lookup = meta_lookup.unwrap_or(&owned_lookup);
    let type_lookup = ConfigTypeLookup::for_config_root(&ConfigTypeLookup::find_config_root(root).unwrap_or_else(|| root.to_path_buf()));
    let obj_uuid = uuid_or_fallback(&ometa.uuid, &obj.rel);
    let obj_name = if ometa.name.is_empty() { name_of(&base) } else { ometa.name.clone() };
    let obj_synonym = ometa.synonym.clone();

    let om_path = if obj.common { None } else { find_object_module(&base) };
    let (om_text, object_module) = match &om_path {
        Some(p) => {
            let om_raw = fs::read(p)?;
            (
                read_module_text(&om_raw, "ObjectModule"),
                json!({"state": "referenced", "path": format!("../Ext/{}", name_of(p)), "sourceSha256": sha256_hex(&om_raw)}),
            )
        }
        None => (Vec::new(), json!({"state": "absent", "path": null})),
    };

    struct Parsed<'s> {
        bin_rel: String,
        raw: &'s [u8],
        raw_sha: String,
        info: FormInfo,
        fmeta: Meta,
    }
    let mut parsed: Vec<Parsed> = Vec::new();
    for name in &obj.forms {
        let bin_rel = form_bin_rel(obj, name);
        let raw = sources
            .get(name)
            .ok_or_else(|| ExtractionError::new(PAYLOAD_MISSING, "Form.bin source is missing").with("path", bin_rel.as_str()))?;
        let container = parse_container(raw, limits, 0, None)?;
        let form_entry = container.get("form").ok_or_else(|| {
            ExtractionError::new(PAYLOAD_MISSING, "Form.bin has no 'form' entry").with("path", bin_rel.as_str())
        })?;
        let mod_raw = container.get("module").map(|e| e.raw.clone()).unwrap_or_default();
        let fmeta = if obj.common { ometa.clone() } else { meta_of(&base.join("Forms").join(format!("{name}.xml"))) };
        let form_uuid = uuid_or_fallback(&fmeta.uuid, &format!("{}/{bin_rel}", obj.rel));
        let form_name = if fmeta.name.is_empty() { name.clone() } else { fmeta.name.clone() };
        let form_raw = form_entry.raw.clone();
        let payload = |suffix: &str, data: Vec<u8>| Payload {
            locator: format!("{bin_rel}#{suffix}"),
            sha256: sha256_hex(&data),
            raw: data,
        };
        let info = FormInfo {
            uuid: form_uuid,
            name: form_name,
            synonym: fmeta.synonym.clone(),
            kind_code: 0,
            header_payload: payload("header", Vec::new()),
            layout_payload: payload("form", form_raw),
            module_payload: payload("module", mod_raw),
            kind: "OrdinaryForm".to_string(),
        };
        let raw_sha = source_shas.and_then(|s| s.get(name)).cloned().unwrap_or_else(|| sha256_hex(raw));
        parsed.push(Parsed { bin_rel, raw, raw_sha, info, fmeta });
    }
    let alias_items: Vec<(String, String)> = parsed.iter().map(|p| (p.info.uuid.clone(), p.info.name.clone())).collect();
    let alias_of: HashMap<String, String> =
        resolve_1c_aliases("Forms", "Form", &alias_items, &BTreeMap::new())?.into_iter().collect();

    let mut files = Tree::new();
    let mut roles: BTreeMap<String, String> = BTreeMap::new();
    // path -> sha256 of the file bytes, when already computed elsewhere
    let mut known_sha: HashMap<String, String> = HashMap::new();
    let mut diags: Vec<Value> = Vec::new();
    fn put(files: &mut Tree, roles: &mut BTreeMap<String, String>, path: &str, data: Vec<u8>, role: &str) {
        files.insert(path.to_string(), data);
        roles.insert(path.to_string(), role.to_string());
    }

    let mut form_records: Vec<Value> = Vec::new();
    let mut source_forms = Map::new();
    for Parsed { bin_rel, raw, raw_sha, info, fmeta } in &parsed {
        let alias = alias_of[&info.uuid].as_str();
        let key = format!("Forms/{}/FormModule", info.uuid);
        let decoded = decode_payload_hashed(&info.module_payload.raw, &key, "auto", Some(&info.module_payload.sha256))?;
        let mod_path = safe_join(&["Forms", alias, MODULE_NAME])?;
        put(&mut files, &mut roles, &mod_path, decoded.canonical.clone(), "form-module");
        known_sha.insert(mod_path.clone(), decoded.output_sha256.clone());
        diags.extend(decoded.diagnostics.clone());
        let module = module_record(&mod_path, &info.module_payload.locator, &decoded);

        let (mut definition, stream) = build_form_definition(info, alias, false);
        let def_path = safe_join(&["Forms", alias, "FormDefinition.json"])?;
        let mut stream_parts = vec!["Forms", alias];
        stream_parts.extend(STREAM_PATH.split('/'));
        let stream_path = safe_join(&stream_parts)?;
        let mut layout_status = "unsupported";
        let mut xml_path: Option<String> = None;
        let mut preview_path: Option<String> = None;
        let oof = build_oof_with_types(info, alias, oof_path, Some(meta_lookup), Some(type_lookup.as_ref()));
        diags.extend(oof.diagnostics.clone());
        let component = format!("Forms/{}", info.uuid);
        if let Some(xml) = &oof.xml {
            if let Value::Object(ref mut d) = definition {
                apply_projection(d, &oof);
            }
            let xp = safe_join(&["Forms", alias, FORM_XML_NAME])?;
            put(&mut files, &mut roles, &xp, xml.clone(), "form-xml");
            xml_path = Some(xp);
            layout_status = "partial";
            match render_form_preview_html(xml, &decoded.canonical, &om_text, alias) {
                Ok(html) => {
                    let pp = safe_join(&["Forms", alias, FORM_PREVIEW_NAME])?;
                    put(&mut files, &mut roles, &pp, html, "form-preview");
                    preview_path = Some(pp);
                    definition["projection"]["formPreview"] = Value::from(FORM_PREVIEW_NAME);
                }
                Err(e) => diags.push(diagnostic(
                    FORM_PREVIEW_FAILED,
                    "warning",
                    &component,
                    &[("alias", alias.into()), ("reason", e.to_string().into())],
                )),
            }
        } else {
            diags.push(diagnostic(
                FORM_PROJECTION_UNSUPPORTED,
                "warning",
                &component,
                &[("backend", "oof".into()), ("alias", alias.into())],
            ));
        }
        put(&mut files, &mut roles, &def_path, dumps_json(&definition)?, "form-definition");
        let stream_len = stream.len();
        put(&mut files, &mut roles, &stream_path, stream, "form-binary");
        known_sha.insert(stream_path.clone(), info.layout_payload.sha256.clone());
        let mut layout = Map::new();
        layout.insert("backend".into(), (if xml_path.is_some() { "oof" } else { "basic" }).into());
        layout.insert("status".into(), layout_status.into());
        layout.insert(
            "binary".into(),
            json!({
                "path": stream_path,
                "representation": "raw-stream",
                "sourceLocator": info.layout_payload.locator,
                "sourceSha256": info.layout_payload.sha256,
                "size": stream_len,
            }),
        );
        if let Some(xp) = xml_path {
            layout.insert("xml".into(), xp.into());
        }
        if let Some(pp) = preview_path {
            layout.insert("preview".into(), pp.into());
        }
        let form_type = if fmeta.form_type.is_empty() { "Ordinary" } else { fmeta.form_type.as_str() };
        form_records.push(json!({
            "id": info.uuid,
            "originalName": info.name,
            "synonym": info.synonym,
            "alias": alias,
            "kind": "OrdinaryForm",
            "formType": form_type,
            "isDefault": false,
            "definition": def_path,
            "module": module,
            "layout": Value::Object(layout),
        }));
        source_forms.insert(alias.to_string(), json!({"binPath": bin_rel, "size": raw.len(), "sha256": raw_sha}));
    }

    let diags = sort_diagnostics(diags);
    let warnings = diags.iter().filter(|d| d["severity"] == "warning").count();
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut report: Vec<String> = vec![
        "# Extraction report".into(),
        String::new(),
        format!("- Status: `{STATUS_PARTIAL}`"),
        format!("- Object: {} (`{obj_uuid}`), class `{}`", quote(&obj_name)?, obj.object_class),
        format!("- Object module: {}", s(&object_module["state"])),
        format!("- Forms: {}", form_records.len()),
        format!("- Warnings: {warnings}"),
        String::new(),
        "## Forms".into(),
        String::new(),
    ];
    form_records.sort_by_key(|r| s(&r["id"]));
    for r in &form_records {
        report.push(format!(
            "- `{}` {} (`{}`): module {}, layout {} ({})",
            s(&r["alias"]),
            quote(&s(&r["originalName"]))?,
            s(&r["id"]),
            s(&r["module"]["state"]),
            s(&r["layout"]["status"]),
            s(&r["layout"]["backend"]),
        ));
    }
    report.extend([
        String::new(),
        "## Notes".into(),
        String::new(),
        "- `src/` is a derived projection of `Ext/Form.bin`. Edit `FormModule.bsl` or `Reference/FormStream.bin` \
         and run `oneform pack` to write them back; `Form.xml`, `FormPreview.html` and `FormDefinition.json` \
         are regenerated."
            .into(),
        String::new(),
    ]);
    put(&mut files, &mut roles, "ExtractionReport.md", report.join("\n").into_bytes(), "report");
    let file_records: Vec<Value> = files
        .iter()
        .map(|(p, d)| {
            let sha = known_sha.remove(p).unwrap_or_else(|| sha256_hex(d));
            json!({"path": p, "role": roles[p], "size": d.len(), "sha256": sha})
        })
        .collect();
    let manifest = json!({
        "schemaVersion": 1,
        "generator": {"name": "oneform", "version": GENERATOR_VERSION, "outputSchemaVersion": 1},
        "source": {
            "kind": "config-object",
            "objectClass": obj.object_class,
            "processorId": obj_uuid,
            "processorName": obj_name,
            "objectSynonym": obj_synonym,
            "forms": Value::Object(source_forms),
        },
        "status": STATUS_PARTIAL,
        "codeComplete": true,
        "objectModule": object_module,
        "forms": form_records,
        "files": file_records,
        "diagnostics": diags,
    });
    put(&mut files, &mut roles, MANIFEST_PATH, dumps_json(&manifest)?, "manifest");
    Ok(ExtractionResult { files, status: STATUS_PARTIAL.to_string(), diagnostics: diags })
}

pub fn describe(result: &ExtractionResult) -> String {
    let manifest: Value = serde_json::from_slice(&result.files[MANIFEST_PATH]).unwrap_or(Value::Null);
    let n = manifest["forms"].as_array().map_or(0, Vec::len);
    format!(
        "{}, reference partial ({})",
        plural(n, "form"),
        plural(result.diagnostics.len(), "warning")
    )
}

// --------------------------------------------------------------------------- source snapshots

fn snapshots(root: &Path, obj: &ConfigObject) -> Result<BTreeMap<String, Snapshot>> {
    obj.forms.iter().map(|n| Ok((n.clone(), snapshot_epf(&bin_path(root, obj, n))?))).collect()
}

fn verify(root: &Path, obj: &ConfigObject, snaps: &BTreeMap<String, Snapshot>) -> Result<()> {
    for (n, snap) in snaps {
        verify_source_unchanged(&bin_path(root, obj, n), snap)?;
    }
    Ok(())
}

fn sources_of(snaps: &BTreeMap<String, Snapshot>) -> Sources {
    snaps.iter().map(|(n, s)| (n.clone(), s.data.clone())).collect()
}

fn shas_of(snaps: &BTreeMap<String, Snapshot>) -> BTreeMap<String, String> {
    snaps.iter().map(|(n, s)| (n.clone(), s.sha256.clone())).collect()
}

fn stage_snapshots(root: &Path, obj: &ConfigObject, snaps: &BTreeMap<String, Snapshot>, opts: &ConfigRunOpts<'_>) -> Result<ExtractionResult> {
    stage_config_object_hashed(root, obj, Some(&sources_of(snaps)), Some(&shas_of(snaps)), opts.oof_path, opts.limits, None)
}

fn old_source_forms(existing: &Tree) -> Option<Value> {
    let manifest = load_json_bytes(existing.get(MANIFEST_PATH)?, MANIFEST_PATH).ok()?;
    manifest.get("source")?.get("forms").cloned()
}

/// Options shared by [`extract_config_object`], [`check_config_object`] and [`pack_config_object`].
#[derive(Debug, Clone)]
pub struct ConfigRunOpts<'a> {
    pub dry_run: bool,
    pub replace_generated: bool,
    pub backup: bool,
    pub keep_work: bool,
    pub oof_path: Option<&'a str>,
    pub limits: Option<&'a Limits>,
    pub recover_leftovers: bool,
}

impl Default for ConfigRunOpts<'_> {
    fn default() -> Self {
        ConfigRunOpts {
            dry_run: false,
            replace_generated: false,
            backup: false,
            keep_work: false,
            oof_path: None,
            limits: None,
            recover_leftovers: true,
        }
    }
}

fn recovered_notes(root: &Path, obj: &ConfigObject, opts: &ConfigRunOpts<'_>) -> Result<Vec<String>> {
    if opts.recover_leftovers && !opts.dry_run {
        Ok(recover(root, &obj.rel)?.into_iter().map(|a| format!("recovered: {a}")).collect())
    } else {
        Ok(Vec::new())
    }
}

fn outcome(path: &str, status: &'static str, summary: String, lines: Vec<String>) -> Outcome {
    let mut out = Outcome::new(path, status);
    out.summary = summary;
    out.lines = lines;
    out
}

// --------------------------------------------------------------------------- extract / check

/// Publish `<ObjDir>/src/` atomically (the object's `Form.bin` files are only read).
pub fn extract_config_object(root: &Path, obj: &ConfigObject, opts: &ConfigRunOpts<'_>) -> Result<Outcome> {
    let _lock = processor_lock(root, &obj.rel)?;
    let mut notes = recovered_notes(root, obj, opts)?;
    let snaps = snapshots(root, obj)?;
    let result = stage_snapshots(root, obj, &snaps, opts)?;
    let summary = describe(&result);
    let existing = read_tree(&src_dir(root, obj))?;
    if opts.dry_run {
        let (added, updated, deleted) = plan(&existing, &result.files);
        if added.is_empty() && updated.is_empty() && deleted.is_empty() {
            return Ok(outcome(&obj.rel, "UNCHANGED", String::new(), notes));
        }
        check_ownership(&existing, opts.replace_generated)?;
        notes.extend(plan_lines(&added, &updated, &deleted));
        return Ok(outcome(
            &obj.rel,
            "WOULD_UPDATE",
            format!("+{} ~{} -{}", added.len(), updated.len(), deleted.len()),
            notes,
        ));
    }
    verify(root, obj, &snaps)?;
    let first = &obj.forms[0];
    let publ = publish(
        root,
        &obj.rel,
        &result.files,
        &bin_path(root, obj, first),
        &snaps[first],
        opts.keep_work,
        opts.replace_generated,
    )?;
    if let Some(rd) = &publ.run_dir {
        notes.push(format!("work kept: {}", rd.display()));
    }
    Ok(outcome(&obj.rel, publ.status, summary, notes))
}

/// Compare `<ObjDir>/src/` with an in-memory staging of the current `Form.bin` files.
pub fn check_config_object(root: &Path, obj: &ConfigObject, opts: &ConfigRunOpts<'_>) -> Result<Outcome> {
    let _lock = processor_lock(root, &obj.rel)?;
    let snaps = snapshots(root, obj)?;
    let result = stage_snapshots(root, obj, &snaps, opts)?;
    let summary = describe(&result);
    let existing = read_tree(&src_dir(root, obj))?;
    let (added, updated, deleted) = plan(&existing, &result.files);
    if added.is_empty() && updated.is_empty() && deleted.is_empty() {
        return Ok(outcome(&obj.rel, "OK", summary, Vec::new()));
    }
    let counts = format!("(+{} ~{} -{})", added.len(), updated.len(), deleted.len());
    let current: Value = serde_json::from_slice(&result.files[MANIFEST_PATH]).unwrap_or(Value::Null);
    let stale = old_source_forms(&existing).as_ref() != current["source"].get("forms");
    let (status, text) = if stale {
        ("STALE", format!("existing src is stale for current Form.bin {counts}"))
    } else {
        ("DIFFERS", format!("src differs from the current extraction {counts}"))
    };
    Ok(outcome(&obj.rel, status, text, plan_lines(&added, &updated, &deleted)))
}

// --------------------------------------------------------------------------- pack

#[derive(Debug, Clone)]
pub struct ConfigPackPlan {
    /// `PACK_UNCHANGED` | `PACK_PACKED` | `PACK_SRC_ONLY`
    pub status: &'static str,
    pub packed_paths: Vec<String>,
    /// One line per detected `Form.xml` edit (control, properties), for the pack report.
    pub edit_notes: Vec<String>,
    /// Form name -> new `Form.bin` (only changed forms).
    pub new_sources: Sources,
    /// Staging of the (new) `Form.bin` files = the new src tree.
    pub result: ExtractionResult,
    pub existing: Tree,
}

/// Replace/append container entries; every other entry keeps its stored bytes and header.
fn rebuild_bin(raw: &[u8], edits: &[(&str, Vec<u8>)], limits: &Limits) -> Result<Vec<u8>> {
    let container = parse_container(raw, limits, 0, None)?;
    let mut entries: Vec<EntryOut<'_>> = container
        .entries
        .iter()
        .map(|e| {
            let data = edits.iter().find(|(n, _)| *n == e.name).map_or(e.raw.as_slice(), |(_, d)| d.as_slice());
            (e.name.as_str(), data, Some(e.header.as_slice()))
        })
        .collect();
    for (n, data) in edits {
        if !container.contains(n) {
            entries.push((*n, data.as_slice(), None));
        }
    }
    serialize_container(&entries, 0x200)
}

fn paths_value(paths: impl IntoIterator<Item = String>) -> Value {
    Value::Array(paths.into_iter().map(Value::from).collect())
}

fn existing_file<'a>(existing: &'a Tree, path: &str) -> Result<&'a Vec<u8>> {
    existing
        .get(path)
        .ok_or_else(|| ExtractionError::new(PAYLOAD_MISSING, "expected src file is missing").with("paths", paths_value([path.to_string()])))
}

/// Pure in-memory: pack edited `existing` src files into the `Form.bin` files of `obj`.
pub fn stage_config_pack(
    root: &Path,
    obj: &ConfigObject,
    sources: &Sources,
    existing: &Tree,
    oof_path: Option<&str>,
    limits: Option<&Limits>,
) -> Result<ConfigPackPlan> {
    let default_limits = Limits::default();
    let limits = limits.unwrap_or(&default_limits);
    if existing.is_empty() {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src is missing or empty; run extract first"));
    }
    let owned = manifest_hashes(existing)?;
    let unknown: Vec<String> =
        existing.keys().filter(|p| !owned.contains_key(*p) && p.as_str() != MANIFEST_PATH).cloned().collect();
    if !unknown.is_empty() {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src contains files not listed in the manifest")
            .with("paths", paths_value(unknown)));
    }
    let missing: Vec<String> = owned.keys().filter(|p| !existing.contains_key(*p)).cloned().collect();
    if !missing.is_empty() {
        return Err(ExtractionError::new(PAYLOAD_MISSING, "expected src file is missing").with("paths", paths_value(missing)));
    }
    let old = load_json_bytes(&existing[MANIFEST_PATH], MANIFEST_PATH)?;
    let old_source = old.get("source").filter(|s| s.is_object()).cloned().unwrap_or_else(|| json!({}));

    let current = stage_config_object(root, obj, Some(sources), oof_path, Some(limits), None)?;
    let cur: Value = serde_json::from_slice(&current.files[MANIFEST_PATH]).unwrap_or(Value::Null);
    let cur_id = &cur["source"]["processorId"];
    if old_source.get("kind").and_then(Value::as_str) != Some("config-object") || old_source.get("processorId") != Some(cur_id) {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src manifest belongs to a different object")
            .with("expected", cur_id.clone())
            .with("found", old_source.get("processorId").cloned().unwrap_or(Value::Null)));
    }
    if old_source.get("forms") != Some(&cur["source"]["forms"]) {
        return Err(ExtractionError::new(SOURCE_CHANGED, "Form.bin changed since src was extracted; run extract before pack")
            .with("object", obj.rel.as_str()));
    }
    let mut readonly: Vec<String> = cur["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter_map(|f| {
                    let (role, path) = (f["role"].as_str()?, f["path"].as_str()?);
                    (READ_ONLY_ROLES.contains(&role) && existing.get(path) != current.files.get(path))
                        .then(|| path.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    readonly.sort();
    if !readonly.is_empty() {
        return Err(ExtractionError::new(
            GENERATED_FILE_MODIFIED,
            "definition files are read-only projections and cannot be packed; if you did not edit them, they come from another oneform version: run `oneform extract` again",
        )
        .with("paths", paths_value(readonly)));
    }
    let name_of_bin: HashMap<String, &String> = obj.forms.iter().map(|n| (form_bin_rel(obj, n), n)).collect();
    // form name -> [(entry, data)] in insertion order
    let mut edits: BTreeMap<&str, Vec<(&str, Vec<u8>)>> = BTreeMap::new();
    let mut expected: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut packed: Vec<String> = Vec::new();
    let mut layouts: Vec<LayoutPatch> = Vec::new();
    let type_lookup = ConfigTypeLookup::for_config_root(&ConfigTypeLookup::find_config_root(root).unwrap_or_else(|| root.to_path_buf()));
    let schema = if obj.common {
        None
    } else {
        let base = object_dir(root, obj);
        base.parent().and_then(|p| read_object_schema_with(&p.join(format!("{}.xml", name_of(&base))), Some(&*type_lookup)))
    };
    for rec in cur["forms"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let alias = rec["alias"].as_str().unwrap_or("");
        let bin = cur["source"]["forms"][alias]["binPath"].as_str().unwrap_or("");
        let Some(name) = name_of_bin.get(bin) else { continue };
        let mod_path = rec["module"]["path"].as_str().unwrap_or("");
        let ex_mod = existing_file(existing, mod_path)?;
        if Some(ex_mod) != current.files.get(mod_path) {
            if let Some((payload, canonical)) = encode_text(mod_path, ex_mod, &rec["module"])? {
                crate::bsl_order::check_module_edit(mod_path, current.files.get(mod_path).map(Vec::as_slice), &canonical)?;
                edits.entry(name.as_str()).or_default().push(("module", payload));
                expected.insert(mod_path.to_string(), canonical);
                packed.push(mod_path.to_string());
            }
        }
        let stream_path = rec["layout"]["binary"]["path"].as_str().unwrap_or("");
        let ex_stream = existing_file(existing, stream_path)?;
        let planned = with_object_schema(schema.clone(), || {
            with_type_lookup(Arc::clone(&type_lookup), || plan_layout_patch(&rec["layout"], existing, &current.files))
        })?;
        if let Some(mut lp) = planned {
            if !obj.common {
                if let Some(path) = find_object_module(&object_dir(root, obj)) {
                    if let Ok(raw) = fs::read(&path) {
                        lp.add_module_warnings(&name_of(&path), &read_module_text(&raw, "ObjectModule"));
                    }
                }
            }
            edits.entry(name.as_str()).or_default().push(("form", lp.patched.bytes.clone()));
            expected.insert(stream_path.to_string(), lp.patched.bytes.clone());
            packed.push(lp.xml_path.clone());
            layouts.push(lp);
        } else if Some(ex_stream) != current.files.get(stream_path) {
            edits.entry(name.as_str()).or_default().push(("form", ex_stream.clone()));
            expected.insert(stream_path.to_string(), ex_stream.clone());
            packed.push(stream_path.to_string());
        }
    }

    if packed.is_empty() {
        let status = if existing == &current.files { PACK_UNCHANGED } else { PACK_SRC_ONLY };
        return Ok(ConfigPackPlan {
            status,
            packed_paths: Vec::new(),
            edit_notes: Vec::new(),
            new_sources: Sources::new(),
            result: current,
            existing: existing.clone(),
        });
    }

    let mut new_sources = Sources::new();
    for (n, e) in &edits {
        new_sources.insert((*n).to_string(), rebuild_bin(&sources[*n], e, limits)?);
    }
    let mut merged = sources.clone();
    merged.extend(new_sources.iter().map(|(k, v)| (k.clone(), v.clone())));
    let new = stage_config_object(root, obj, Some(&merged), oof_path, Some(limits), None)?;
    let wrong: Vec<String> =
        expected.iter().filter(|(p, data)| new.files.get(*p) != Some(*data)).map(|(p, _)| p.clone()).collect();
    if !wrong.is_empty() {
        return Err(ExtractionError::new(PUBLISH_VERIFY_FAILED, "packed Form.bin does not reproduce the edited files")
            .with("paths", paths_value(wrong)));
    }
    for lp in &layouts {
        lp.verify_staged(&new.files)?;
    }
    packed.sort();
    let edit_notes = layouts.iter().flat_map(|lp| lp.summary()).collect();
    Ok(ConfigPackPlan { status: PACK_PACKED, packed_paths: packed, edit_notes, new_sources, result: new, existing: existing.clone() })
}

/// Write edited `FormModule.bsl` / `FormStream.bin` back into `Ext/Form.bin` and refresh `src/`.
pub fn pack_config_object(root: &Path, obj: &ConfigObject, opts: &ConfigRunOpts<'_>) -> Result<Outcome> {
    let _lock = processor_lock(root, &obj.rel)?;
    let mut notes = recovered_notes(root, obj, opts)?;
    let snaps = snapshots(root, obj)?;
    let plan_ = stage_config_pack(
        root,
        obj,
        &sources_of(&snaps),
        &read_tree(&src_dir(root, obj))?,
        opts.oof_path,
        opts.limits,
    )?;
    let summary = describe(&plan_.result);
    if plan_.status == PACK_UNCHANGED {
        return Ok(outcome(&obj.rel, "UNCHANGED", String::new(), notes));
    }
    let mut packed: Vec<String> = plan_.packed_paths.iter().map(|p| format!("  pack {p}")).collect();
    packed.extend(plan_.edit_notes.iter().cloned());
    if opts.dry_run {
        if !packed.is_empty() {
            notes.extend(packed.iter().cloned());
            return Ok(outcome(&obj.rel, "WOULD_PACK", format!("{} file(s)", plan_.packed_paths.len()), notes));
        }
        let (added, updated, deleted) = plan(&plan_.existing, &plan_.result.files);
        notes.extend(plan_lines(&added, &updated, &deleted));
        return Ok(outcome(
            &obj.rel,
            "WOULD_UPDATE",
            format!("+{} ~{} -{}", added.len(), updated.len(), deleted.len()),
            notes,
        ));
    }
    verify(root, obj, &snaps)?;
    let first = &obj.forms[0];
    let mut written: Vec<&String> = Vec::new();
    let published = (|| -> Result<crate::transaction::PublishResult> {
        for (name, data) in &plan_.new_sources {
            let path = bin_path(root, obj, name);
            if opts.backup {
                atomic_write(&path.with_file_name(format!("{}.bak", name_of(&path))), &snaps[name].data)?;
            }
            atomic_write(&path, data)?;
            written.push(name);
        }
        let first_data = plan_.new_sources.get(first).unwrap_or(&snaps[first].data);
        let first_snap = Snapshot { data: first_data.clone(), size: first_data.len(), sha256: sha256_hex(first_data) };
        publish(root, &obj.rel, &plan_.result.files, &bin_path(root, obj, first), &first_snap, opts.keep_work, true)
    })();
    let publ = match published {
        Ok(p) => p,
        Err(e) => {
            for name in written {
                if let Err(re) = atomic_write(&bin_path(root, obj, name), &snaps[name].data) {
                    return Err(ExtractionError::new(IO_ERROR, "src publication failed and Form.bin could not be restored")
                        .with("error", re.message));
                }
            }
            return Err(e);
        }
    };
    if let Some(rd) = &publ.run_dir {
        notes.push(format!("work kept: {}", rd.display()));
    }
    if !packed.is_empty() {
        let text = format!("{} file(s), {summary}", plan_.packed_paths.len());
        notes.extend(packed);
        if opts.backup {
            notes.push("  backup Form.bin.bak".to_string());
        }
        return Ok(outcome(&obj.rel, "PACKED", text, notes));
    }
    Ok(outcome(&obj.rel, "UPDATED", summary, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const U1: &str = "aaaaaaaa-0000-0000-0000-000000000001";

    fn write(path: &Path, data: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    #[test]
    fn read_meta_lookup_collects_nested_attributes() {
        // port: tests/test_oof.py::test_read_meta_lookup_collects_nested_attributes
        let dir = tempfile::tempdir().unwrap();
        let xml = dir.path().join("Doc.xml");
        write(
            &xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\"><Document uuid=\"d\"><ChildObjects>\
             <Attribute uuid=\"AAAAAAAA-0000-0000-0000-000000000001\"><Properties><Name>Организация</Name></Properties></Attribute>\
             <TabularSection uuid=\"aaaaaaaa-0000-0000-0000-000000000002\"><Properties><Name>Товары</Name></Properties>\
             <ChildObjects><Attribute uuid=\"aaaaaaaa-0000-0000-0000-000000000003\"><Properties><Name>Цена</Name></Properties>\
             </Attribute></ChildObjects></TabularSection></ChildObjects></Document></MetaDataObject>",
        );
        let want: MetaLookup = [
            ("aaaaaaaa-0000-0000-0000-000000000001", "Организация"),
            ("aaaaaaaa-0000-0000-0000-000000000002", "Товары"),
            ("aaaaaaaa-0000-0000-0000-000000000003", "Цена"),
            (META_KIND_KEY, "Document"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        assert_eq!(read_meta_lookup(&xml), want);
        assert!(read_meta_lookup(&dir.path().join("missing.xml")).is_empty());
    }

    #[test]
    fn read_meta_reads_properties_and_synonym() {
        let dir = tempfile::tempdir().unwrap();
        let xml = dir.path().join("Form.xml");
        write(
            &xml,
            &format!(
                "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">\
                 <Form uuid=\"{U1}\"><Properties><Name> Форма </Name><Synonym>\
                 <v8:item><v8:lang>en</v8:lang><v8:content>Form</v8:content></v8:item>\
                 <v8:item><v8:lang>ru</v8:lang><v8:content>Форма РУ</v8:content></v8:item></Synonym>\
                 <FormType>Ordinary</FormType></Properties></Form></MetaDataObject>"
            ),
        );
        let m = read_meta(&xml).unwrap();
        assert_eq!(
            m,
            Meta { uuid: U1.into(), name: "Форма".into(), synonym: "Форма РУ".into(), form_type: "Ordinary".into() }
        );
        assert!(read_meta(&dir.path().join("none.xml")).is_none());
        write(&xml, "<broken");
        assert!(read_meta(&xml).is_none());
    }

    #[test]
    fn uuid_fallback_is_uuid5_oid() {
        assert_eq!(uuid_or_fallback(&format!(" {} ", U1.to_uppercase()), "x"), U1);
        // python: str(uuid.uuid5(uuid.NAMESPACE_OID, "Catalogs/A/Forms/B/Ext/Form.bin"))
        let a = uuid_or_fallback("", "Catalogs/A/Forms/B/Ext/Form.bin");
        assert!(is_uuid(&a));
        assert_eq!(a, uuid_or_fallback("not-a-uuid", "Catalogs/A/Forms/B/Ext/Form.bin"));
        assert_ne!(a, uuid_or_fallback("", "Catalogs/A/Forms/C/Ext/Form.bin"));
    }

    #[test]
    fn discover_objects_common_and_root_itself() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let bin = |p: &str| write(&root.join(p), "x");
        bin("Documents/Doc/Forms/B/Ext/Form.bin");
        bin("Documents/Doc/Forms/A/Ext/Form.bin");
        bin("Documents/Doc/src/Forms/Z/Ext/Form.bin");
        bin("CommonForms/Common/Ext/Form.bin");
        bin("Catalogs/Cat/Forms/F/Ext/Other.bin");
        let found: Vec<ConfigObject> = discover_config_objects(root);
        let rels: Vec<&str> = found.iter().map(|o| o.rel.as_str()).collect();
        assert_eq!(rels, ["CommonForms/Common", "Documents/Doc"]);
        assert_eq!(found[1].forms, ["A", "B"]);
        assert_eq!(found[1].object_class, "Documents");
        assert!(found[0].common && found[0].object_class == "CommonForms");
        assert_eq!(form_bin_rel(&found[0], "Common"), "Ext/Form.bin");

        let doc = root.join("Documents").join("Doc");
        let own = discover_config_objects(&doc);
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].rel, ".");
        assert_eq!(object_dir(&doc, &own[0]), doc);
    }

    #[test]
    fn rebuild_bin_replaces_and_appends_entries() {
        let raw = serialize_container(&[("form", b"F0", None), ("module", b"M0", None)], 0x200).unwrap();
        let out = rebuild_bin(&raw, &[("module", b"M1".to_vec())], &Limits::default()).unwrap();
        let c = parse_container(&out, &Limits::default(), 0, None).unwrap();
        assert_eq!(c.get("form").unwrap().raw, b"F0");
        assert_eq!(c.get("module").unwrap().raw, b"M1");
        let only_form = serialize_container(&[("form", b"F0", None)], 0x200).unwrap();
        let out = rebuild_bin(&only_form, &[("module", b"M2".to_vec())], &Limits::default()).unwrap();
        let c = parse_container(&out, &Limits::default(), 0, None).unwrap();
        assert_eq!(c.names(), ["form", "module"]);
        assert_eq!(c.get("module").unwrap().raw, b"M2");
    }
}
