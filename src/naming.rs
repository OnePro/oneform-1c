//! Alias validation, default aliases and safe relative path construction (port of `naming.py`).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::errors::{
    ExtractionError, Result, ALIAS_COLLISION, ALIAS_INVALID, ALIAS_UNKNOWN_UUID, PATH_UNSAFE,
};

/// `[A-Za-z][A-Za-z0-9_-]{0,63}` (full match).
pub fn is_alias(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_alphabetic()
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'-')
}

/// `[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}` (full match).
pub fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

/// A 1C identifier: letters (any script), digits and `_`, not starting with a digit.
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

pub fn is_reserved(name: &str) -> bool {
    let u = name.to_uppercase();
    if matches!(u.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    (u.starts_with("COM") || u.starts_with("LPT")) && matches!(&u[3..], "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
}

fn py_str(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn validate_alias(alias: &str, kind: &str) -> Result<String> {
    if !is_alias(alias) {
        return Err(ExtractionError::new(ALIAS_INVALID, "alias must match [A-Za-z][A-Za-z0-9_-]{0,63}")
            .with("kind", kind)
            .with("alias", alias));
    }
    if is_reserved(alias.split('.').next().unwrap_or("")) {
        return Err(ExtractionError::new(ALIAS_INVALID, "alias is a Windows reserved name")
            .with("kind", kind)
            .with("alias", alias));
    }
    Ok(alias.to_string())
}

/// `validate_alias` for an arbitrary JSON value (non-strings are invalid).
pub fn validate_alias_value(alias: &Value, kind: &str) -> Result<String> {
    match alias {
        Value::String(s) => validate_alias(s, kind),
        other => Err(ExtractionError::new(ALIAS_INVALID, "alias must match [A-Za-z][A-Za-z0-9_-]{0,63}")
            .with("kind", kind)
            .with("alias", py_str(other))),
    }
}

pub fn default_alias(prefix: &str, uuid: &str) -> String {
    format!("{prefix}_{}", uuid.replace('-', "_"))
}

/// Approximation of Python `str.casefold` (full folding for the common special cases).
pub fn casefold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ß' | 'ẞ' => out.push_str("ss"),
            'ſ' => out.push('s'),
            'ς' => out.push('σ'),
            c => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// `aliases`: (uuid, alias) pairs; case-insensitive uniqueness within `kind`.
pub fn check_unique<'a>(kind: &str, aliases: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<()> {
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for (uuid, alias) in aliases {
        let folded = casefold(alias);
        if let Some(prev) = seen.get(&folded) {
            if *prev != uuid {
                let mut uuids = vec![(*prev).to_string(), uuid.to_string()];
                uuids.sort();
                return Err(ExtractionError::new(ALIAS_COLLISION, "alias is used more than once")
                    .with("kind", kind)
                    .with("alias", alias)
                    .with("uuids", uuids));
            }
        }
        seen.insert(folded, uuid);
    }
    Ok(())
}

fn check_known(kind: &str, known: &BTreeSet<&str>, configured: &BTreeMap<String, String>) -> Result<()> {
    for uuid in configured.keys() {
        if !known.contains(uuid.as_str()) {
            return Err(ExtractionError::new(ALIAS_UNKNOWN_UUID, "alias refers to a uuid that is not in the EPF")
                .with("kind", kind)
                .with("uuid", uuid.as_str()));
        }
    }
    Ok(())
}

/// Map every uuid of the EPF to its alias (configured or default); result in `uuids` order.
pub fn resolve_aliases(
    kind: &str,
    prefix: &str,
    uuids: &[String],
    configured: &BTreeMap<String, String>,
) -> Result<Vec<(String, String)>> {
    let known: BTreeSet<&str> = uuids.iter().map(String::as_str).collect();
    check_known(kind, &known, configured)?;
    let mut result = Vec::new();
    for u in uuids {
        let alias = match configured.get(u) {
            Some(a) => a.clone(),
            None => default_alias(prefix, u),
        };
        result.push((u.clone(), validate_alias(&alias, kind)?));
    }
    check_unique(kind, result.iter().map(|(u, a)| (u.as_str(), a.as_str())))?;
    Ok(result)
}

const FORBIDDEN: &str = "/\\:*?\"<>|";

pub fn sanitize_1c_name(name: &str, fallback: &str) -> String {
    let cleaned: String =
        name.chars().filter(|c| !FORBIDDEN.contains(*c) && (*c as u32) >= 0x20 && *c as u32 != 0x7f).collect();
    let cleaned = cleaned.trim_matches(' ').trim_end_matches(['.', ' ']);
    if cleaned.is_empty()
        || cleaned == "."
        || cleaned == ".."
        || is_reserved(cleaned.split('.').next().unwrap_or(""))
    {
        return fallback.to_string();
    }
    cleaned.to_string()
}

/// Map `(uuid, original 1C name)` pairs to aliases: configured, else the sanitized original
/// name; result in `items` order.
pub fn resolve_1c_aliases(
    kind: &str,
    prefix: &str,
    items: &[(String, String)],
    configured: &BTreeMap<String, String>,
) -> Result<Vec<(String, String)>> {
    let known: BTreeSet<&str> = items.iter().map(|(u, _)| u.as_str()).collect();
    check_known(kind, &known, configured)?;
    let mut result: BTreeMap<String, String> = BTreeMap::new();
    for (u, _) in items {
        if let Some(a) = configured.get(u) {
            result.insert(u.clone(), validate_alias(a, kind)?);
        }
    }
    let mut taken: BTreeSet<String> = result.values().map(|a| casefold(a)).collect();
    for (uuid, name) in items {
        if configured.contains_key(uuid) {
            continue;
        }
        let base = sanitize_1c_name(name, &default_alias(prefix, uuid));
        let mut alias = base.clone();
        let mut n = 1;
        while taken.contains(&casefold(&alias)) {
            n += 1;
            alias = format!("{base}_{n}");
        }
        taken.insert(casefold(&alias));
        result.insert(uuid.clone(), alias);
    }
    let ordered: Vec<(String, String)> = items.iter().map(|(u, _)| (u.clone(), result[u].clone())).collect();
    check_unique(kind, ordered.iter().map(|(u, a)| (u.as_str(), a.as_str())))?;
    Ok(ordered)
}

/// Join relative path parts into a POSIX path, rejecting anything that could escape.
pub fn safe_join<S: AsRef<str>>(parts: &[S]) -> Result<String> {
    if parts.is_empty() {
        return Err(ExtractionError::new(PATH_UNSAFE, "empty path"));
    }
    for part in parts {
        let part = part.as_ref();
        let bad = part.is_empty()
            || part == "."
            || part == ".."
            || part.chars().any(|c| FORBIDDEN.contains(c) || (c as u32) < 0x20 || c as u32 == 0x7f)
            || part != part.trim_end_matches(['.', ' '])
            || part != part.trim_start_matches(' ');
        if bad {
            return Err(ExtractionError::new(PATH_UNSAFE, "unsafe path component").with("part", part));
        }
    }
    Ok(parts.iter().map(AsRef::as_ref).collect::<Vec<&str>>().join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const U1: &str = "47200e71-ce18-488c-9d4a-169dd6066884";
    const U2: &str = "637a2a20-fab2-465f-addf-9fb74e76c15c";

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
    }

    fn uuids() -> Vec<String> {
        vec![U1.to_string(), U2.to_string()]
    }

    #[test]
    fn valid_alias() {
        // port: tests/test_naming.py::test_valid_alias
        for a in ["MainForm", "A", "a_b-c9", &"X".repeat(64)] {
            assert_eq!(validate_alias(a, "Forms").unwrap(), a);
        }
    }

    #[test]
    fn invalid_alias() {
        // port: tests/test_naming.py::test_invalid_alias
        let long = "X".repeat(65);
        for a in [
            "", "1Form", "_x", "Форма", "a b", "a.b", "a/b", "..", long.as_str(), "CON", "nul", "Com1", "lpt9", "aux",
            "PRN",
        ] {
            assert_eq!(validate_alias(a, "Forms").unwrap_err().code, ALIAS_INVALID, "{a:?}");
        }
        for v in [json!(null), json!(5)] {
            assert_eq!(validate_alias_value(&v, "Forms").unwrap_err().code, ALIAS_INVALID);
        }
    }

    #[test]
    fn com0_is_not_reserved() {
        // port: tests/test_naming.py::test_com0_is_not_reserved
        assert_eq!(validate_alias("COM0", "Forms").unwrap(), "COM0");
    }

    #[test]
    fn default_alias_and_resolve() {
        // port: tests/test_naming.py::test_default_alias
        assert_eq!(default_alias("Form", U1), "Form_47200e71_ce18_488c_9d4a_169dd6066884");
        let r = resolve_aliases("Forms", "Form", &uuids(), &map(&[(U1, "MainForm")])).unwrap();
        assert_eq!(
            r,
            vec![
                (U1.to_string(), "MainForm".to_string()),
                (U2.to_string(), "Form_637a2a20_fab2_465f_addf_9fb74e76c15c".to_string())
            ]
        );
    }

    #[test]
    fn collision_case_insensitive() {
        // port: tests/test_naming.py::test_collision_case_insensitive
        let e = resolve_aliases("Forms", "Form", &uuids(), &map(&[(U1, "Main"), (U2, "main")])).unwrap_err();
        assert_eq!(e.code, ALIAS_COLLISION);
    }

    #[test]
    fn default_collides_with_explicit() {
        // port: tests/test_naming.py::test_default_collides_with_explicit
        let explicit = default_alias("Form", U2).to_lowercase();
        let e = resolve_aliases("Forms", "Form", &uuids(), &map(&[(U1, &explicit)])).unwrap_err();
        assert_eq!(e.code, ALIAS_COLLISION);
    }

    #[test]
    fn unknown_uuid() {
        // port: tests/test_naming.py::test_unknown_uuid
        let other = "11111111-1111-1111-1111-111111111111";
        let e = resolve_aliases("Forms", "Form", &[U1.to_string()], &map(&[(other, "X")])).unwrap_err();
        assert_eq!(e.code, ALIAS_UNKNOWN_UUID);
    }

    #[test]
    fn safe_join_cases() {
        // port: tests/test_naming.py::test_safe_join
        assert_eq!(safe_join(&["Forms", "Main", "FormModule.bsl"]).unwrap(), "Forms/Main/FormModule.bsl");
        let bad: [&[&str]; 11] = [
            &["..", "x"], &["/abs"], &["a/b"], &["a\\b"], &["C:"], &["."], &[""], &["x."], &["x "], &[], &["a\0"],
        ];
        for b in bad {
            assert_eq!(safe_join(b).unwrap_err().code, PATH_UNSAFE, "{b:?}");
        }
    }

    #[test]
    fn resolve_1c_aliases_names_and_dedup() {
        // extra (no Python test): sanitized original names with numeric dedup
        let items = vec![(U1.to_string(), "Main".to_string()), (U2.to_string(), "main".to_string())];
        let r = resolve_1c_aliases("Forms", "Form", &items, &BTreeMap::new()).unwrap();
        assert_eq!(r[0].1, "Main");
        assert_eq!(r[1].1, "main_2");
        let items = vec![(U1.to_string(), "CON".to_string())];
        let r = resolve_1c_aliases("Forms", "Form", &items, &BTreeMap::new()).unwrap();
        assert_eq!(r[0].1, default_alias("Form", U1));
    }
}
