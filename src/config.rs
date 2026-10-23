//! Strict loading and merging of root / processor `extraction.json` (port of `config.py`).

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::canonical::{dumps_json, sha256_hex};
use crate::encoding::OVERRIDE_CODECS;
use crate::errors::{ExtractionError, Result, CONFIG_INVALID};
use crate::naming::{check_unique, is_uuid, validate_alias_value};

pub const FORM_BACKENDS: [&str; 3] = ["basic", "v8unpack-json", "oof"];
pub const RETAIN_MODES: [&str; 2] = ["on-incomplete", "always"];
pub const DEFAULT_LIMITS: [(&str, i64); 6] = [
    ("maxInputMiB", 256),
    ("maxExpandedMiB", 1024),
    ("maxEntryMiB", 64),
    ("maxEntries", 100000),
    ("maxDepth", 128),
    ("timeoutSeconds", 120),
];

fn bad(message: &str) -> ExtractionError {
    ExtractionError::new(CONFIG_INVALID, message)
}

const DUP_PREFIX: &str = "\u{0}duplicate-key:";

/// JSON value whose objects reject duplicate keys.
struct Strict(Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_bool<E>(self, v: bool) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Bool(v)))
            }
            fn visit_i64<E>(self, v: i64) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::from(v)))
            }
            fn visit_u64<E>(self, v: u64) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::from(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Strict, E> {
                Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E>(self, v: &str) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::String(v.to_string())))
            }
            fn visit_string<E>(self, v: String) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::String(v)))
            }
            fn visit_unit<E>(self) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(Strict(v)) = seq.next_element()? {
                    out.push(v);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Strict, A::Error> {
                let mut out = Map::new();
                while let Some(k) = map.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom(format!("{DUP_PREFIX}{k}")));
                    }
                    let Strict(v) = map.next_value()?;
                    out.insert(k, v);
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}

/// Parse UTF-8 JSON (one optional leading BOM); duplicate keys and NaN are rejected.
pub fn load_json_bytes(data: &[u8], what: &str) -> Result<Value> {
    let data = data.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(data);
    let text = std::str::from_utf8(data).map_err(|_| bad("not valid UTF-8").with("what", what))?;
    match serde_json::from_str::<Strict>(text) {
        Ok(Strict(v)) => Ok(v),
        Err(e) => {
            let msg = e.to_string();
            if let Some(rest) = msg.strip_prefix(DUP_PREFIX) {
                let key = rest.rsplit_once(" at line ").map_or(rest, |(k, _)| k);
                Err(bad("duplicate key").with("what", what).with("key", key))
            } else if msg.contains("recursion limit") {
                Err(bad("JSON nesting too deep").with("what", what))
            } else {
                Err(bad("invalid JSON").with("what", what).with("detail", msg))
            }
        }
    }
}

fn object<'a>(value: &'a Value, what: &str, allowed: &[&str], required: &[&str]) -> Result<&'a Map<String, Value>> {
    let Some(m) = value.as_object() else {
        return Err(bad("expected an object").with("what", what));
    };
    let mut unknown: Vec<String> = m.keys().filter(|k| !allowed.contains(&k.as_str())).cloned().collect();
    unknown.sort();
    if !unknown.is_empty() {
        return Err(bad("unknown keys").with("what", what).with("keys", unknown));
    }
    let mut missing: Vec<&str> = required.iter().copied().filter(|k| !m.contains_key(*k)).collect();
    missing.sort();
    if !missing.is_empty() {
        return Err(bad("missing keys").with("what", what).with("keys", missing));
    }
    Ok(m)
}

fn schema_version(value: &Value, what: &str) -> Result<()> {
    let ok = match value {
        Value::Number(n) => (n.is_i64() || n.is_u64()) && n.as_i64() == Some(1),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(bad("unsupported schemaVersion").with("what", what))
    }
}

fn choice(value: &Value, choices: &[&str], what: &str) -> Result<String> {
    match value.as_str() {
        Some(s) if choices.contains(&s) => Ok(s.to_string()),
        _ => Err(bad("invalid value").with("what", what).with("allowed", choices.to_vec())),
    }
}

pub fn validate_codec(value: &Value, what: &str) -> Result<String> {
    choice(value, &OVERRIDE_CODECS, what)
}

/// Equivalent of `ObjectModule|Forms/<uuid>/FormModule|Templates/<uuid>/Template` (full match).
pub fn validate_override_key(key: &str) -> Result<()> {
    let ok = key == "ObjectModule"
        || key.strip_prefix("Forms/").and_then(|r| r.strip_suffix("/FormModule")).is_some_and(is_uuid)
        || key.strip_prefix("Templates/").and_then(|r| r.strip_suffix("/Template")).is_some_and(is_uuid);
    // The Python regex accepts any `[^/]+` for the middle part, then requires a UUID: identical.
    if ok {
        Ok(())
    } else {
        Err(bad("invalid encoding override key").with("key", key))
    }
}

/// Root config with builtin defaults merged in.
#[derive(Debug, Clone, PartialEq)]
pub struct RootConfig {
    pub processor_root: String,
    pub form_backend: String,
    pub encoding: String,
    pub retain_form_binary: String,
    pub limits: Map<String, Value>,
}

pub fn validate_root_config(d: &Value) -> Result<RootConfig> {
    let m = object(d, "root config", &["schemaVersion", "processorRoot", "defaults"], &["schemaVersion"])?;
    schema_version(&m["schemaVersion"], "root config")?;
    let root = match m.get("processorRoot") {
        None => "ExternalDataProcessors".to_string(),
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        Some(_) => return Err(bad("processorRoot must be a non-empty string")),
    };
    let empty = Value::Object(Map::new());
    let defaults = object(
        m.get("defaults").unwrap_or(&empty),
        "defaults",
        &["formBackend", "encoding", "retainFormBinary", "limits"],
        &[],
    )?;
    let limits = object(
        defaults.get("limits").unwrap_or(&empty),
        "limits",
        &DEFAULT_LIMITS.map(|(k, _)| k),
        &[],
    )?;
    let mut merged: Map<String, Value> = DEFAULT_LIMITS.iter().map(|(k, v)| ((*k).to_string(), Value::from(*v))).collect();
    for (k, v) in limits {
        let positive = matches!(v, Value::Number(n) if n.as_f64().is_some_and(|f| f > 0.0));
        if !positive {
            return Err(bad("limit must be a positive number").with("key", k.as_str()));
        }
        merged.insert(k.clone(), v.clone());
    }
    let dv = |k: &str, default: &'static str| defaults.get(k).cloned().unwrap_or_else(|| Value::from(default));
    Ok(RootConfig {
        processor_root: root,
        form_backend: choice(&dv("formBackend", "oof"), &FORM_BACKENDS, "formBackend")?,
        encoding: validate_codec(&dv("encoding", "auto"), "encoding")?,
        retain_form_binary: choice(&dv("retainFormBinary", "on-incomplete"), &RETAIN_MODES, "retainFormBinary")?,
        limits: merged,
    })
}

fn alias_map(value: Option<&Value>, what: &str) -> Result<BTreeMap<String, String>> {
    let empty = Value::Object(Map::new());
    let Some(m) = value.unwrap_or(&empty).as_object() else {
        return Err(bad("expected an object").with("what", what));
    };
    for (uuid, alias) in m {
        if !is_uuid(uuid) {
            return Err(bad("key is not a lowercase UUID").with("what", what).with("key", uuid.as_str()));
        }
        validate_alias_value(alias, what)?;
    }
    let pairs: Vec<(&str, &str)> = m.iter().map(|(k, v)| (k.as_str(), v.as_str().unwrap_or_default())).collect();
    check_unique(what, pairs)?;
    Ok(m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect())
}

/// Validated processor config.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessorConfig {
    pub epf: Option<String>,
    pub form_aliases: BTreeMap<String, String>,
    pub template_aliases: BTreeMap<String, String>,
    /// Insertion ordered.
    pub encoding_overrides: Vec<(String, String)>,
}

pub fn validate_processor_config(d: &Value) -> Result<ProcessorConfig> {
    let m = object(
        d,
        "processor config",
        &["schemaVersion", "epf", "formAliases", "templateAliases", "encodingOverrides"],
        &["schemaVersion"],
    )?;
    schema_version(&m["schemaVersion"], "processor config")?;
    let epf = match m.get("epf") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if !s.is_empty() && !s.contains('/') && !s.contains('\\') => Some(s.clone()),
        Some(_) => return Err(bad("epf must be a file name")),
    };
    let empty = Value::Object(Map::new());
    let Some(overrides) = m.get("encodingOverrides").unwrap_or(&empty).as_object() else {
        return Err(bad("encodingOverrides must be an object"));
    };
    let mut ov = Vec::new();
    for (k, v) in overrides {
        validate_override_key(k)?;
        ov.push((k.clone(), validate_codec(v, &format!("encodingOverrides[{k}]"))?));
    }
    Ok(ProcessorConfig {
        epf,
        form_aliases: alias_map(m.get("formAliases"), "Forms")?,
        template_aliases: alias_map(m.get("templateAliases"), "Templates")?,
        encoding_overrides: ov,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveConfig {
    pub epf: Option<String>,
    pub form_backend: String,
    pub encoding: String,
    pub retain_form_binary: String,
    pub limits: Map<String, Value>,
    pub form_aliases: BTreeMap<String, String>,
    pub template_aliases: BTreeMap<String, String>,
    /// Insertion ordered (processor first, then CLI).
    pub encoding_overrides: Vec<(String, String)>,
}

impl EffectiveConfig {
    /// Scoped override wins over the general `encoding`.
    pub fn codec_for(&self, key: &str) -> &str {
        self.encoding_overrides.iter().find(|(k, _)| k == key).map_or(self.encoding.as_str(), |(_, v)| v.as_str())
    }

    /// Settings that influence output bytes; no paths, no resource limits.
    pub fn semantic(&self) -> Value {
        fn srt<'a>(it: impl Iterator<Item = (&'a String, &'a String)>) -> Value {
            let sorted: BTreeMap<&String, &String> = it.collect();
            Value::Object(sorted.into_iter().map(|(k, v)| (k.clone(), Value::from(v.as_str()))).collect())
        }
        let mut m = Map::new();
        m.insert("formBackend".into(), self.form_backend.as_str().into());
        m.insert("encoding".into(), self.encoding.as_str().into());
        m.insert("retainFormBinary".into(), self.retain_form_binary.as_str().into());
        m.insert("formAliases".into(), srt(self.form_aliases.iter()));
        m.insert("templateAliases".into(), srt(self.template_aliases.iter()));
        m.insert("encodingOverrides".into(), srt(self.encoding_overrides.iter().map(|(k, v)| (k, v))));
        Value::Object(m)
    }

    pub fn sha256(&self) -> String {
        sha256_hex(&dumps_json(&self.semantic()).expect("semantic config contains only strings"))
    }
}

/// Merge: CLI > processor > root defaults > builtin. Inputs are parsed JSON objects.
pub fn effective_config(
    root_config: &Value,
    processor_config: &Value,
    cli_overrides: &[(&str, &str)],
) -> Result<EffectiveConfig> {
    let root = validate_root_config(root_config)?;
    let proc = validate_processor_config(processor_config)?;
    let mut overrides = proc.encoding_overrides;
    for (k, v) in cli_overrides {
        validate_override_key(k)?;
        let codec = validate_codec(&Value::from(*v), &format!("override[{k}]"))?;
        match overrides.iter_mut().find(|(ek, _)| ek == k) {
            Some(slot) => slot.1 = codec,
            None => overrides.push(((*k).to_string(), codec)),
        }
    }
    Ok(EffectiveConfig {
        epf: proc.epf,
        form_backend: root.form_backend,
        encoding: root.encoding,
        retain_form_binary: root.retain_form_binary,
        limits: root.limits,
        form_aliases: proc.form_aliases,
        template_aliases: proc.template_aliases,
        encoding_overrides: overrides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{ALIAS_COLLISION, ALIAS_INVALID};
    use serde_json::json;

    const U1: &str = "47200e71-ce18-488c-9d4a-169dd6066884";
    const U2: &str = "637a2a20-fab2-465f-addf-9fb74e76c15c";

    fn root() -> Value {
        json!({"schemaVersion": 1, "processorRoot": "ExternalDataProcessors", "defaults": {"encoding": "auto"}})
    }

    fn proc() -> Value {
        json!({
            "schemaVersion": 1,
            "epf": "a.epf",
            "formAliases": {U1: "MainForm"},
            "templateAliases": {U2: "Tpl"},
            "encodingOverrides": {"ObjectModule": "cp1251", format!("Forms/{U1}/FormModule"): "utf-8"},
        })
    }

    fn code_of(r: &Value, p: &Value, cli: &[(&str, &str)]) -> &'static str {
        effective_config(r, p, cli).unwrap_err().code
    }

    #[test]
    fn load_json_bytes_bom_and_errors() {
        // port: tests/test_config.py::test_load_json_bytes_bom_and_errors
        assert_eq!(load_json_bytes(b"\xef\xbb\xbf{\"a\": 1}", "config").unwrap(), json!({"a": 1}));
        let cases: [&[u8]; 6] =
            [b"\xef\xbb\xbf\xef\xbb\xbf{}", b"\xff\xfe{}", b"{", b"{\"a\":1,\"a\":2}", b"NaN", b"[Infinity]"];
        for raw in cases {
            assert_eq!(load_json_bytes(raw, "config").unwrap_err().code, CONFIG_INVALID, "{raw:?}");
        }
        let e = load_json_bytes(b"{\"a\":1,\"a\":2}", "config").unwrap_err();
        assert_eq!(e.message, "duplicate key");
        assert_eq!(e.params["key"], "a");
    }

    #[test]
    fn valid_and_merge() {
        // port: tests/test_config.py::test_valid_and_merge
        let cfg = effective_config(&root(), &proc(), &[("ObjectModule", "utf-8")]).unwrap();
        assert_eq!(cfg.encoding_overrides.iter().find(|(k, _)| k == "ObjectModule").unwrap().1, "utf-8");
        assert_eq!(cfg.codec_for("ObjectModule"), "utf-8");
        assert_eq!(cfg.codec_for(&format!("Templates/{U2}/Template")), "auto");
        assert_eq!(cfg.limits["maxDepth"], 128);
        assert_eq!(cfg.form_backend, "oof");
        assert_eq!(cfg.epf.as_deref(), Some("a.epf"));
    }

    #[test]
    fn general_encoding_default_and_scoped_wins() {
        // port: tests/test_config.py::test_general_encoding_default_and_scoped_wins
        let mut r = root();
        r["defaults"]["encoding"] = json!("cp1251");
        let cfg = effective_config(&r, &proc(), &[]).unwrap();
        assert_eq!(cfg.codec_for("ObjectModule"), "cp1251");
        assert_eq!(cfg.codec_for(&format!("Forms/{U1}/FormModule")), "utf-8");
    }

    #[test]
    fn hash_stable_and_semantic() {
        // port: tests/test_config.py::test_hash_stable_and_semantic
        let a = effective_config(&root(), &proc(), &[]).unwrap();
        let mut reordered = proc();
        reordered["formAliases"] = json!({U1: "MainForm"});
        reordered["epf"] = json!("renamed.epf");
        assert_eq!(a.sha256(), effective_config(&root(), &reordered, &[]).unwrap().sha256());
        assert_eq!(a.sha256().len(), 64);
        let mut other = proc();
        other["formAliases"] = json!({U1: "Other"});
        assert_ne!(effective_config(&root(), &other, &[]).unwrap().sha256(), a.sha256());
        assert_ne!(effective_config(&root(), &proc(), &[("ObjectModule", "utf-8")]).unwrap().sha256(), a.sha256());
        let s = serde_json::to_string(&a.semantic()).unwrap();
        assert!(!s.contains('\\') && !s.contains("ExternalDataProcessors"));
    }

    #[test]
    fn strictness() {
        // port: tests/test_config.py::test_strictness
        type Mutate = Box<dyn Fn(&mut Value, &mut Value)>;
        let cases: Vec<Mutate> = vec![
            Box::new(|r, _| r["extra"] = json!(1)),
            Box::new(|r, _| r["schemaVersion"] = json!(2)),
            Box::new(|r, _| r["schemaVersion"] = json!(true)),
            Box::new(|r, _| r["defaults"]["encoding"] = json!("latin-1")),
            Box::new(|r, _| r["defaults"]["formBackend"] = json!("auto")),
            Box::new(|r, _| r["defaults"]["bogus"] = json!(1)),
            Box::new(|r, _| r["defaults"]["limits"] = json!({"maxDepth": 0})),
            Box::new(|r, _| r["defaults"]["limits"] = json!({"nope": 1})),
            Box::new(|_, p| p["extra"] = json!(1)),
            Box::new(|_, p| p["schemaVersion"] = json!(2)),
            Box::new(|_, p| {
                p.as_object_mut().unwrap().remove("schemaVersion");
            }),
            Box::new(|_, p| p["epf"] = json!("dir/a.epf")),
            Box::new(|_, p| p["encodingOverrides"]["Forms/not-a-uuid/FormModule"] = json!("utf-8")),
            Box::new(|_, p| p["encodingOverrides"]["Other"] = json!("utf-8")),
            Box::new(|_, p| p["encodingOverrides"][format!("Forms/{U1}/Template")] = json!("utf-8")),
            Box::new(|_, p| p["encodingOverrides"][format!("Templates/{U2}/Template")] = json!("utf8")),
            Box::new(|_, p| p["formAliases"]["BAD"] = json!("X")),
        ];
        for (i, mutate) in cases.iter().enumerate() {
            let (mut r, mut p) = (root(), proc());
            mutate(&mut r, &mut p);
            assert_eq!(code_of(&r, &p, &[]), CONFIG_INVALID, "case {i}");
        }
    }

    #[test]
    fn alias_collision_in_config() {
        // port: tests/test_config.py::test_alias_collision_in_config
        let mut p = proc();
        p["formAliases"][U2] = json!("mainform");
        assert_eq!(code_of(&root(), &p, &[]), ALIAS_COLLISION);
    }

    #[test]
    fn invalid_alias_value() {
        // port: tests/test_config.py::test_invalid_alias_value
        let mut p = proc();
        p["formAliases"][U2] = json!("1x");
        assert_eq!(code_of(&root(), &p, &[]), ALIAS_INVALID);
    }

    #[test]
    fn bad_cli_override() {
        // port: tests/test_config.py::test_bad_cli_override
        assert_eq!(code_of(&root(), &proc(), &[("Nope", "utf-8")]), CONFIG_INVALID);
        assert_eq!(code_of(&root(), &proc(), &[("ObjectModule", "bogus")]), CONFIG_INVALID);
    }

    #[test]
    fn processor_defaults() {
        // port: tests/test_config.py::test_processor_defaults
        assert!(validate_processor_config(&json!({"schemaVersion": 1})).unwrap().form_aliases.is_empty());
    }
}
