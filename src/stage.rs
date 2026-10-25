//! Pure in-memory staging of the `src/` tree for one processor (port of `stage.py`).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::canonical::{dumps_json, sha256_hex};
use crate::config::effective_config;
use crate::container::{parse_container, Limits};
use crate::encoding::decode_payload;
use crate::errors::{
    diagnostic, ExtractionError, Result, CONFIG_INVALID, FORM_PREVIEW_FAILED, FORM_PROJECTION_UNSUPPORTED,
    MANAGED_FORM_SKIPPED,
};
use crate::form_basic::{build_form_definition, MODULE_NAME, STREAM_PATH};
use crate::manifest::{build_manifest, module_record, sort_diagnostics, tools_record, ManifestInput, MANIFEST_PATH};
use crate::metadata::{discover, Payload};
use crate::naming::{resolve_1c_aliases, resolve_aliases, safe_join};
use crate::oof::{apply_projection, build_oof, FORM_XML_NAME};
use crate::preview::{render_form_preview_html, FORM_PREVIEW_NAME};
use crate::report::render_report;
use crate::templates::build_template;
use crate::transaction::Tree;

pub const STATUS_COMPLETE: &str = "complete";
pub const STATUS_PARTIAL: &str = "partial-reference";

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub files: Tree,
    pub status: String,
    pub diagnostics: Vec<Value>,
}

/// Optional settings for [`stage_processor`].
#[derive(Debug, Clone, Default)]
pub struct StageOpts<'a> {
    pub cli_overrides: &'a [(&'a str, &'a str)],
    pub form_backend: Option<&'a str>,
    pub oof_path: Option<&'a str>,
    pub allow_managed: bool,
    pub use_original_names: bool,
}

/// Build the complete `src/` tree (relative POSIX path -> bytes); no filesystem access.
pub fn stage_processor(
    epf_bytes: &[u8],
    epf_name: &str,
    processor_config: &Value,
    root_config: &Value,
    tools_lock_bytes: &[u8],
    opts: &StageOpts<'_>,
) -> Result<ExtractionResult> {
    let mut cfg = effective_config(root_config, processor_config, opts.cli_overrides)?;
    if epf_name.is_empty() || epf_name.contains('/') || epf_name.contains('\\') {
        return Err(ExtractionError::new(CONFIG_INVALID, "epf name must be a plain file name"));
    }
    if let Some(ref c_epf) = cfg.epf {
        if c_epf != epf_name {
            return Err(ExtractionError::new(CONFIG_INVALID, "configured epf differs from the given EPF")
                .with("epf", c_epf.clone()));
        }
    }
    if let Some(b) = opts.form_backend {
        cfg.form_backend = b.to_string();
    }
    if !matches!(cfg.form_backend.as_str(), "basic" | "oof") {
        return Err(ExtractionError::new(CONFIG_INVALID, "only the basic and oof form backends are implemented"));
    }
    let tools = tools_record(tools_lock_bytes, &cfg.form_backend)?;

    let limits = Limits::from_json(&Value::Object(cfg.limits.clone()));
    let c = parse_container(epf_bytes, &limits, 0, None)?;
    let inv = discover(&c, &limits, None, opts.allow_managed)?;
    let proc = json!({"uuid": inv.processor.object_uuid, "name": inv.processor.name});

    let mut valid_keys: BTreeSet<String> = ["ObjectModule".to_string()].into_iter().collect();
    for f in &inv.forms {
        valid_keys.insert(format!("Forms/{}/FormModule", f.uuid));
    }
    for t in &inv.templates {
        valid_keys.insert(format!("Templates/{}/Template", t.uuid));
    }
    let mut override_keys: Vec<&String> = cfg.encoding_overrides.iter().map(|(k, _)| k).collect();
    override_keys.sort();
    for key in override_keys {
        if !valid_keys.contains(key) {
            return Err(ExtractionError::new(CONFIG_INVALID, "encoding override key is not in the EPF").with("key", key.clone()));
        }
    }

    let (form_alias, tmpl_alias): (BTreeMap<String, String>, BTreeMap<String, String>) = if opts.use_original_names {
        let items_f: Vec<(String, String)> = inv.forms.iter().map(|f| (f.uuid.clone(), f.name.clone())).collect();
        let items_t: Vec<(String, String)> = inv.templates.iter().map(|t| (t.uuid.clone(), t.name.clone())).collect();
        (
            resolve_1c_aliases("Forms", "Form", &items_f, &cfg.form_aliases)?.into_iter().collect(),
            resolve_1c_aliases("Templates", "Template", &items_t, &cfg.template_aliases)?.into_iter().collect(),
        )
    } else {
        let uuids_f: Vec<String> = inv.forms.iter().map(|f| f.uuid.clone()).collect();
        let uuids_t: Vec<String> = inv.templates.iter().map(|t| t.uuid.clone()).collect();
        (
            resolve_aliases("Forms", "Form", &uuids_f, &cfg.form_aliases)?.into_iter().collect(),
            resolve_aliases("Templates", "Template", &uuids_t, &cfg.template_aliases)?.into_iter().collect(),
        )
    };

    let mut files: Tree = Tree::new();
    let mut roles: BTreeMap<String, String> = BTreeMap::new();
    let mut diags: Vec<Value> = Vec::new();

    fn put(files: &mut Tree, roles: &mut BTreeMap<String, String>, path: String, data: Vec<u8>, role: &str) {
        files.insert(path.clone(), data);
        roles.insert(path, role.to_string());
    }

    let module = |parts: &[&str], role: &str, payload: &Payload, key: &str, files: &mut Tree, roles: &mut BTreeMap<String, String>, diags: &mut Vec<Value>| -> Result<Value> {
        let path = safe_join(parts)?;
        let decoded = decode_payload(&payload.raw, key, cfg.codec_for(key))?;
        put(files, roles, path.clone(), decoded.canonical.clone(), role);
        diags.extend(decoded.diagnostics.clone());
        Ok(module_record(&path, &payload.locator, &decoded))
    };

    let object_module = module(&["ObjectModule.bsl"], "object-module", &inv.object_module, "ObjectModule", &mut files, &mut roles, &mut diags)?;

    let mut form_records: Vec<Value> = Vec::new();
    for f in &inv.forms {
        let alias = &form_alias[&f.uuid];
        let mod_rec = module(&["Forms", alias, MODULE_NAME], "form-module", &f.module_payload, &format!("Forms/{}/FormModule", f.uuid), &mut files, &mut roles, &mut diags)?;
        let (mut definition, stream) = build_form_definition(f, alias, f.uuid == inv.processor.default_form_uuid);
        let def_path = safe_join(&["Forms", alias, "FormDefinition.json"])?;
        let mut stream_parts = vec!["Forms", alias];
        stream_parts.extend(STREAM_PATH.split('/'));
        let stream_path = safe_join(&stream_parts)?;
        let mut layout_backend = "basic";
        let mut layout_status = "unsupported";
        let mut xml_path: Option<String> = None;
        let mut preview_path: Option<String> = None;
        if f.kind == "ManagedForm" {
            diags.push(diagnostic(MANAGED_FORM_SKIPPED, "warning", &format!("Forms/{}", f.uuid), &[("alias", alias.as_str().into())]));
        }
        let oof = if cfg.form_backend == "oof" && f.kind != "ManagedForm" {
            Some(build_oof(f, alias, opts.oof_path, Some(&inv.meta_lookup)))
        } else {
            None
        };
        if let Some(ref o) = oof {
            diags.extend(o.diagnostics.clone());
        }
        if let Some(o) = oof.as_ref().filter(|o| o.xml.is_some()) {
            let xml = o.xml.as_ref().expect("checked");
            if let Value::Object(ref mut d) = definition {
                apply_projection(d, o);
            }
            let xp = safe_join(&["Forms", alias, FORM_XML_NAME])?;
            put(&mut files, &mut roles, xp.clone(), xml.clone(), "form-xml");
            xml_path = Some(xp);
            layout_backend = "oof";
            layout_status = "partial";
            let mod_bytes = files.get(mod_rec["path"].as_str().unwrap_or("")).cloned().unwrap_or_default();
            let obj_bytes = files.get("ObjectModule.bsl").cloned().unwrap_or_default();
            match render_form_preview_html(xml, &mod_bytes, &obj_bytes, alias) {
                Ok(preview_bytes) => {
                    let pp = safe_join(&["Forms", alias, FORM_PREVIEW_NAME])?;
                    put(&mut files, &mut roles, pp.clone(), preview_bytes, "form-preview");
                    preview_path = Some(pp);
                    definition["projection"]["formPreview"] = Value::from(FORM_PREVIEW_NAME);
                }
                Err(e) => {
                    diags.push(diagnostic(
                        FORM_PREVIEW_FAILED,
                        "warning",
                        &format!("Forms/{}", f.uuid),
                        &[("alias", alias.as_str().into()), ("reason", e.to_string().into())],
                    ));
                }
            }
        } else {
            diags.push(diagnostic(
                FORM_PROJECTION_UNSUPPORTED,
                "warning",
                &format!("Forms/{}", f.uuid),
                &[("backend", "basic".into()), ("alias", alias.as_str().into())],
            ));
        }
        put(&mut files, &mut roles, def_path.clone(), dumps_json(&definition)?, "form-definition");
        let stream_len = stream.len();
        put(&mut files, &mut roles, stream_path.clone(), stream, "form-binary");
        let mut layout = Map::new();
        layout.insert("backend".into(), layout_backend.into());
        layout.insert("status".into(), layout_status.into());
        layout.insert(
            "binary".into(),
            json!({
                "path": stream_path,
                "representation": "raw-stream",
                "sourceLocator": f.layout_payload.locator,
                "sourceSha256": f.layout_payload.sha256,
                "size": stream_len,
            }),
        );
        if let Some(xp) = xml_path {
            layout.insert("xml".into(), xp.into());
        }
        if let Some(pp) = preview_path {
            layout.insert("preview".into(), pp.into());
        }
        form_records.push(json!({
            "id": f.uuid,
            "originalName": f.name,
            "synonym": f.synonym,
            "alias": alias,
            "kind": f.kind,
            "isDefault": definition["isDefault"],
            "definition": def_path,
            "module": mod_rec,
            "layout": Value::Object(layout),
        }));
    }

    let mut template_records: Vec<Value> = Vec::new();
    let mut partial = !form_records.is_empty();
    for t in &inv.templates {
        let alias = &tmpl_alias[&t.uuid];
        let out = build_template(t, alias, cfg.codec_for(&format!("Templates/{}/Template", t.uuid)))?;
        let content_path = safe_join(&["Templates", alias, &out.content_name])?;
        let def_path = safe_join(&["Templates", alias, "TemplateDefinition.json"])?;
        let content_sha = sha256_hex(&out.content);
        let known = out.content_known();
        put(&mut files, &mut roles, content_path.clone(), out.content, "template-content");
        put(&mut files, &mut roles, def_path.clone(), dumps_json(&out.definition)?, "template-definition");
        diags.extend(out.diagnostics);
        let mut content = Map::new();
        content.insert("path".into(), content_path.into());
        content.insert("sourceLocator".into(), t.content_payload.locator.clone().into());
        content.insert("sourceSha256".into(), t.content_payload.sha256.clone().into());
        content.insert("outputSha256".into(), content_sha.into());
        content.insert("contentKnown".into(), known.into());
        if let Some(decoded) = out.decoded {
            content.insert("transport".into(), "metadata-base64".into());
            content.insert("decodedSha256".into(), out.decoded_sha256.into());
            content.insert("encoding".into(), decoded.encoding_record());
            content.insert("newlines".into(), decoded.newlines);
        } else {
            partial = true;
        }
        template_records.push(json!({
            "id": t.uuid,
            "originalName": t.name,
            "synonym": t.synonym,
            "alias": alias,
            "templateType": out.definition["templateType"],
            "templateTypeCode": t.type_code,
            "status": out.definition["status"],
            "definition": def_path,
            "content": Value::Object(content),
        }));
    }

    let status = if partial { STATUS_PARTIAL } else { STATUS_COMPLETE };
    let diags = sort_diagnostics(diags);
    let report = render_report(
        epf_name,
        &sha256_hex(epf_bytes),
        &proc,
        status,
        object_module["state"].as_str().unwrap_or(""),
        &form_records,
        &template_records,
        &diags,
    )?;
    put(&mut files, &mut roles, "ExtractionReport.md".into(), report, "report");
    let cfg_sha = cfg.sha256();
    let manifest = build_manifest(ManifestInput {
        epf_name,
        epf_bytes,
        processor: &proc,
        config_sha256: &cfg_sha,
        tools,
        status,
        object_module,
        forms: form_records,
        templates: template_records,
        files: &files,
        roles: &roles,
        diagnostics: diags.clone(),
    })?;
    put(&mut files, &mut roles, MANIFEST_PATH.into(), dumps_json(&manifest)?, "manifest");
    Ok(ExtractionResult { files, status: status.to_string(), diagnostics: diags })
}
