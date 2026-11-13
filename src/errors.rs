//! Stable error codes and the single error type (port of `errors.py`).

use serde_json::{Map, Value};

// Error codes (the string value is part of the output contract).
pub const CONTAINER_LAYOUT_UNSUPPORTED: &str = "CONTAINER_LAYOUT_UNSUPPORTED";
pub const CONTAINER_CORRUPT: &str = "CONTAINER_CORRUPT";
pub const CONTAINER_CYCLE: &str = "CONTAINER_CYCLE";
pub const CONTAINER_LIMIT_EXCEEDED: &str = "CONTAINER_LIMIT_EXCEEDED";
pub const CONTAINER_DUPLICATE_ENTRY: &str = "CONTAINER_DUPLICATE_ENTRY";
pub const CONTAINER_BAD_ENTRY_NAME: &str = "CONTAINER_BAD_ENTRY_NAME";
/// Bytes after the end of the container that are not a valid 64-bit container (`container_scan`).
pub const CONTAINER_TRAILING_DATA: &str = "CONTAINER_TRAILING_DATA";
/// A record cannot be used for the requested comparison, for example it does not inflate (`container_scan`).
pub const CONTAINER_ENTRY_INVALID: &str = "CONTAINER_ENTRY_INVALID";
pub const METADATA_PARSE_ERROR: &str = "METADATA_PARSE_ERROR";
pub const METADATA_LAYOUT_UNSUPPORTED: &str = "METADATA_LAYOUT_UNSUPPORTED";
pub const METADATA_ORPHAN_ENTRY: &str = "METADATA_ORPHAN_ENTRY";
pub const MODULE_LOCATOR_MISSING: &str = "MODULE_LOCATOR_MISSING";
pub const PAYLOAD_MISSING: &str = "PAYLOAD_MISSING";
pub const MANAGED_FORM_UNSUPPORTED: &str = "MANAGED_FORM_UNSUPPORTED";
pub const ENCODING_UNSUPPORTED: &str = "ENCODING_UNSUPPORTED";
pub const ENCODING_CONFLICT: &str = "ENCODING_CONFLICT";
pub const ENCODING_AMBIGUOUS: &str = "ENCODING_AMBIGUOUS";
pub const ENCODING_INVALID: &str = "ENCODING_INVALID";
pub const ALIAS_INVALID: &str = "ALIAS_INVALID";
pub const ALIAS_COLLISION: &str = "ALIAS_COLLISION";
pub const ALIAS_UNKNOWN_UUID: &str = "ALIAS_UNKNOWN_UUID";
pub const PATH_UNSAFE: &str = "PATH_UNSAFE";
pub const CONFIG_INVALID: &str = "CONFIG_INVALID";
pub const TEMPLATE_KIND_UNKNOWN: &str = "TEMPLATE_KIND_UNKNOWN";
pub const TEMPLATE_LAYOUT_UNSUPPORTED: &str = "TEMPLATE_LAYOUT_UNSUPPORTED";
pub const TEXT_CONTAINS_REPLACEMENT_CHAR: &str = "TEXT_CONTAINS_REPLACEMENT_CHAR";
pub const FORM_PROJECTION_UNSUPPORTED: &str = "FORM_PROJECTION_UNSUPPORTED";
pub const FORM_PROJECTION_INCOMPLETE: &str = "FORM_PROJECTION_INCOMPLETE";
pub const FORM_XML_EXTRACTION_FAILED: &str = "FORM_XML_EXTRACTION_FAILED";
pub const FORM_OOF_EXTERNAL_FAILED: &str = "FORM_OOF_EXTERNAL_FAILED";
pub const FORM_CONTROL_UNKNOWN: &str = "FORM_CONTROL_UNKNOWN";
pub const FORM_PROPERTY_UNREADABLE: &str = "FORM_PROPERTY_UNREADABLE";
pub const MANAGED_FORM_SKIPPED: &str = "MANAGED_FORM_SKIPPED";
pub const FORM_PREVIEW_FAILED: &str = "FORM_PREVIEW_FAILED";
pub const PROCESSOR_LOCKED: &str = "PROCESSOR_LOCKED";
pub const SOURCE_CHANGED: &str = "SOURCE_CHANGED";
pub const OUTPUT_NOT_OWNED: &str = "OUTPUT_NOT_OWNED";
pub const OUTPUT_SCHEMA_UNKNOWN: &str = "OUTPUT_SCHEMA_UNKNOWN";
pub const GENERATED_FILE_MODIFIED: &str = "GENERATED_FILE_MODIFIED";
/// A requested edit of `Form.xml` or of the form module that the tool refuses (invalid XML, duplicate names, an
/// unsupported change); the generated files themselves were not tampered with.
pub const FORM_EDIT_REJECTED: &str = "FORM_EDIT_REJECTED";
/// A plan that writes the form layout (the form stream, `Form.bin`, templates) was requested without `--experimental`;
/// nothing was written. Exit code 2, like a usage error.
pub const EXPERIMENTAL_REQUIRED: &str = "EXPERIMENTAL_REQUIRED";
pub const RECOVERY_REQUIRED: &str = "RECOVERY_REQUIRED";
pub const PUBLISH_VERIFY_FAILED: &str = "PUBLISH_VERIFY_FAILED";
pub const IO_ERROR: &str = "IO_ERROR";

/// Error with a stable machine-readable `code` and structured `params` (insertion ordered).
#[derive(Debug, Clone)]
pub struct ExtractionError {
    pub code: &'static str,
    pub message: String,
    pub params: Map<String, Value>,
}

pub type Result<T> = std::result::Result<T, ExtractionError>;

impl ExtractionError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        ExtractionError { code, message: message.into(), params: Map::new() }
    }

    /// Add a parameter (builder style).
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }
}

impl std::fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ExtractionError {}

/// Structured diagnostic record (params sorted by key for stable output).
pub fn diagnostic(code: &str, severity: &str, component: &str, params: &[(&str, Value)]) -> Value {
    let mut sorted: Vec<&(&str, Value)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut p = Map::new();
    for (k, v) in sorted {
        p.insert((*k).to_string(), v.clone());
    }
    let mut m = Map::new();
    m.insert("code".into(), code.into());
    m.insert("severity".into(), severity.into());
    m.insert("component".into(), component.into());
    m.insert("params".into(), Value::Object(p));
    Value::Object(m)
}

impl From<std::io::Error> for ExtractionError {
    fn from(e: std::io::Error) -> Self {
        ExtractionError::new(IO_ERROR, e.to_string())
    }
}
