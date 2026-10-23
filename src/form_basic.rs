//! Basic ordinary-form reference: metadata-level definition + exact layout stream
//! (port of `adapters/ordinary_basic.py`).

use serde_json::{json, Map, Value};

use crate::metadata::FormInfo;

pub const STREAM_PATH: &str = "Reference/FormStream.bin";
pub const MODULE_NAME: &str = "FormModule.bsl";

/// Return (FormDefinition, bytes of `Reference/FormStream.bin`).
pub fn build_form_definition(info: &FormInfo, alias: &str, is_default: bool) -> (Value, Vec<u8>) {
    let stream = info.layout_payload.raw.clone();
    let mut d = Map::new();
    d.insert("schemaVersion".into(), 1.into());
    d.insert("kind".into(), "OrdinaryFormReference".into());
    d.insert("formId".into(), info.uuid.as_str().into());
    d.insert("originalName".into(), info.name.as_str().into());
    d.insert("synonym".into(), info.synonym.as_str().into());
    d.insert("alias".into(), alias.into());
    d.insert("isDefault".into(), is_default.into());
    d.insert("module".into(), MODULE_NAME.into());
    d.insert(
        "projection".into(),
        json!({
            "backend": "basic",
            "status": "unsupported",
            "referenceOnly": true,
            "controlsKnown": false,
            "unknownSections": ["Attributes", "Controls", "Events"],
        }),
    );
    d.insert("attributes".into(), Value::Null);
    d.insert("events".into(), Value::Null);
    d.insert("controls".into(), Value::Null);
    d.insert(
        "binaryEvidence".into(),
        json!({
            "path": STREAM_PATH,
            "representation": "raw-stream",
            "size": stream.len(),
            "sha256": info.layout_payload.digest(),
        }),
    );
    d.insert("diagnostics".into(), Value::Array(vec![]));
    (Value::Object(d), stream)
}
