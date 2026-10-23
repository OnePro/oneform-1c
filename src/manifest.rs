//! `ExtractionManifest.json` structure (port of `manifest.py`). No timestamps, hosts or absolute paths.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::canonical::sha256_hex;
use crate::config::load_json_bytes;
use crate::encoding::DecodedText;
use crate::errors::{ExtractionError, Result, CONFIG_INVALID};

pub const MANIFEST_PATH: &str = "ExtractionManifest.json";
pub const OUTPUT_SCHEMA_VERSION: i64 = 1;
/// `epf_extract.__version__` of the Reference.
pub const GENERATOR_VERSION: &str = "0.1.0";

pub fn module_record(path: &str, locator: &str, decoded: &DecodedText) -> Value {
    json!({
        "state": if decoded.canonical.is_empty() { "empty" } else { "extracted" },
        "path": path,
        "sourceLocator": locator,
        "sourceSha256": decoded.source_sha256,
        "outputSha256": decoded.output_sha256,
        "encoding": decoded.encoding_record(),
        "newlines": decoded.newlines,
    })
}

pub fn tools_record(lock_bytes: &[u8], form_backend: &str) -> Result<Value> {
    let lock = load_json_bytes(lock_bytes, "tools.lock.json")?;
    let backend = lock.get("containerBackend").filter(|b| b.is_object());
    let field = |k: &str| backend.and_then(|b| b.get(k)).and_then(Value::as_str);
    let (Some(name), Some(kind)) = (field("name"), field("kind")) else {
        return Err(ExtractionError::new(CONFIG_INVALID, "tools.lock.json has no valid containerBackend"));
    };
    Ok(json!({
        "lockSha256": sha256_hex(lock_bytes),
        "containerBackend": {"name": name, "kind": kind},
        "formBackend": form_backend,
    }))
}

pub struct ManifestInput<'a> {
    pub epf_name: &'a str,
    pub epf_bytes: &'a [u8],
    /// Object with `uuid` and `name`.
    pub processor: &'a Value,
    pub config_sha256: &'a str,
    pub tools: Value,
    pub status: &'a str,
    pub object_module: Value,
    pub forms: Vec<Value>,
    pub templates: Vec<Value>,
    pub files: &'a BTreeMap<String, Vec<u8>>,
    pub roles: &'a BTreeMap<String, String>,
    pub diagnostics: Vec<Value>,
}

fn sort_by_id(mut records: Vec<Value>) -> Vec<Value> {
    records.sort_by(|a, b| a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or("")));
    records
}

pub fn build_manifest(m: ManifestInput<'_>) -> Result<Value> {
    let mut files = Vec::new();
    for (p, data) in m.files {
        if p == MANIFEST_PATH {
            continue;
        }
        let role = m.roles.get(p).ok_or_else(|| {
            ExtractionError::new(CONFIG_INVALID, "file has no role").with("path", p.as_str())
        })?;
        files.push(json!({"path": p, "role": role, "size": data.len(), "sha256": sha256_hex(data)}));
    }
    let mut out = Map::new();
    out.insert("schemaVersion".into(), OUTPUT_SCHEMA_VERSION.into());
    out.insert(
        "generator".into(),
        json!({
            "name": "epf-source-extractor",
            "version": GENERATOR_VERSION,
            "outputSchemaVersion": OUTPUT_SCHEMA_VERSION,
        }),
    );
    out.insert(
        "source".into(),
        json!({
            "epf": m.epf_name,
            "size": m.epf_bytes.len(),
            "sha256": sha256_hex(m.epf_bytes),
            "processorId": m.processor["uuid"],
            "processorName": m.processor["name"],
        }),
    );
    out.insert("configuration".into(), json!({"effectiveConfigSha256": m.config_sha256}));
    out.insert("tools".into(), m.tools);
    out.insert("status".into(), m.status.into());
    out.insert("codeComplete".into(), true.into());
    out.insert("objectModule".into(), m.object_module);
    out.insert("forms".into(), Value::Array(sort_by_id(m.forms)));
    out.insert("templates".into(), Value::Array(sort_by_id(m.templates)));
    out.insert("files".into(), Value::Array(files));
    out.insert("diagnostics".into(), Value::Array(sort_diagnostics(m.diagnostics)));
    Ok(Value::Object(out))
}

pub fn sort_diagnostics(mut diagnostics: Vec<Value>) -> Vec<Value> {
    let key = |d: &Value| (d["component"].as_str().unwrap_or("").to_string(), d["code"].as_str().unwrap_or("").to_string());
    diagnostics.sort_by_key(key);
    diagnostics
}
