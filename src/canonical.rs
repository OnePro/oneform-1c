//! Canonical, byte-stable serialization helpers (port of `canonical.py` plus the Python-compatible
//! float/round primitives the other writers need).

use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::errors::{ExtractionError, Result, IO_ERROR};

pub fn sha256_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 15) as usize] as char);
    }
    s
}

/// Python `round(x)` for floats (half to even).
pub fn py_round(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// Python `repr(float)`: shortest round-trip digits, `1e-05`, `1e+16`, `33.0`, `inf`/`nan` are rejected by callers.
pub fn py_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf".into() } else { "inf".into() };
    }
    let sci = format!("{:e}", x.abs());
    let (mant, exp) = sci.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent number");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x == 0.0 {
        return format!("{sign}0.0");
    }
    let n = digits.len() as i32;
    if !(-4..16).contains(&exp) {
        let mut s = String::from(sign);
        s.push_str(&digits[..1]);
        if n > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        s.push('e');
        s.push(if exp < 0 { '-' } else { '+' });
        s.push_str(&format!("{:02}", exp.abs()));
        return s;
    }
    if exp >= 0 {
        let int_len = exp + 1;
        if n <= int_len {
            format!("{sign}{digits}{}.0", "0".repeat((int_len - n) as usize))
        } else {
            format!("{sign}{}.{}", &digits[..int_len as usize], &digits[int_len as usize..])
        }
    } else {
        format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
    }
}

/// Options of the Python `json.dumps` flavours used by the Reference.
#[derive(Clone, Copy)]
pub struct JsonOpts {
    pub indent: Option<usize>,
    pub sort_keys: bool,
    pub ensure_ascii: bool,
    pub compact: bool,
}

/// `json.dumps(obj, ensure_ascii=False, indent=2)` + `\n` (insertion order).
pub const PRETTY: JsonOpts = JsonOpts { indent: Some(2), sort_keys: false, ensure_ascii: false, compact: false };
/// `json.dumps(obj, ensure_ascii=False, sort_keys=True, separators=(",", ":"))`.
pub const COMPACT_SORTED: JsonOpts = JsonOpts { indent: None, sort_keys: true, ensure_ascii: false, compact: true };
/// `json.dumps(obj, indent=2, sort_keys=True)` (ASCII escaped, as `journal.json`).
pub const ASCII_SORTED_PRETTY: JsonOpts = JsonOpts { indent: Some(2), sort_keys: true, ensure_ascii: true, compact: false };
/// `json.dumps(obj, ensure_ascii=False, sort_keys=True, default=str)` (default separators `, ` / `: `).
pub const SORTED_SPACED: JsonOpts = JsonOpts { indent: None, sort_keys: true, ensure_ascii: false, compact: false };

/// Python `json` string escaping (`ensure_ascii` escapes everything outside `' '..='~'`, DEL included).
fn escape_str(s: &str, ensure_ascii: bool, out: &mut String) {
    out.push('"');
    let b = s.as_bytes();
    // Copy runs of characters that need no escaping in one go; every escaped character starts at a char boundary.
    let plain = |x: u8| x >= 0x20 && x != b'"' && x != b'\\' && (!ensure_ascii || x < 0x7f);
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        if plain(b[i]) {
            i += 1;
            continue;
        }
        out.push_str(&s[start..i]);
        let c = s[i..].chars().next().expect("char boundary");
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x10000 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => {
                let v = c as u32 - 0x10000;
                let _ = write!(out, "\\u{:04x}\\u{:04x}", 0xd800 + (v >> 10), 0xdc00 + (v & 0x3ff));
            }
        }
        i += c.len_utf8();
        start = i;
    }
    out.push_str(&s[start..]);
    out.push('"');
}

fn write_value(v: &Value, o: &JsonOpts, level: usize, out: &mut String) -> Result<()> {
    let (item_sep, key_sep) = if o.compact { (",", ":") } else if o.indent.is_some() { (",", ": ") } else { (", ", ": ") };
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                let f = n.as_f64().unwrap_or(f64::NAN);
                if !f.is_finite() {
                    return Err(ExtractionError::new(IO_ERROR, "non-finite float in JSON"));
                }
                out.push_str(&py_float_repr(f));
            }
        }
        Value::String(s) => escape_str(s, o.ensure_ascii, out),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(item_sep);
                }
                if let Some(w) = o.indent {
                    out.push('\n');
                    out.push_str(&" ".repeat(w * (level + 1)));
                }
                write_value(item, o, level + 1, out)?;
            }
            if let Some(w) = o.indent {
                out.push('\n');
                out.push_str(&" ".repeat(w * level));
            }
            out.push(']');
        }
        Value::Object(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            let mut keys: Vec<&String> = m.keys().collect();
            if o.sort_keys {
                // UTF-8 byte order == code point order (Python `sorted` on `str`).
                keys.sort();
            }
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push_str(item_sep);
                }
                if let Some(w) = o.indent {
                    out.push('\n');
                    out.push_str(&" ".repeat(w * (level + 1)));
                }
                escape_str(k, o.ensure_ascii, out);
                out.push_str(key_sep);
                write_value(&m[*k], o, level + 1, out)?;
            }
            if let Some(w) = o.indent {
                out.push('\n');
                out.push_str(&" ".repeat(w * level));
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Serialize with the given Python-compatible options (no trailing newline).
pub fn dumps_with(v: &Value, o: &JsonOpts) -> Result<String> {
    let mut s = String::new();
    write_value(v, o, 0, &mut s)?;
    Ok(s)
}

/// UTF-8 JSON, indent 2, insertion-ordered keys, exactly one trailing LF (`canonical.dumps_json`).
pub fn dumps_json(v: &Value) -> Result<Vec<u8>> {
    let mut s = dumps_with(v, &PRETTY)?;
    s.push('\n');
    Ok(s.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn float_repr_matches_python() {
        let cases: &[(f64, &str)] = &[
            (0.1, "0.1"),
            (33.0, "33.0"),
            (1e-5, "1e-05"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (0.0001, "0.0001"),
            (1.5e20, "1.5e+20"),
            (33.00000000000001, "33.00000000000001"),
            (-2.5, "-2.5"),
            (0.0, "0.0"),
            (123456789.125, "123456789.125"),
            (1.2345e-7, "1.2345e-07"),
            (-0.0, "-0.0"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (0.00012, "0.00012"),
            (0.000012, "1.2e-05"),
            (9999999999999998.0, "9999999999999998.0"),
            (12345678901234567.0, "1.2345678901234568e+16"),
            (100.0, "100.0"),
            (-1e-7, "-1e-07"),
            (0.30000000000000004, "0.30000000000000004"),
        ];
        for (x, want) in cases {
            assert_eq!(py_float_repr(*x), *want, "{x}");
        }
    }

    #[test]
    fn round_is_half_even() {
        assert_eq!(py_round(0.5), 0);
        assert_eq!(py_round(1.5), 2);
        assert_eq!(py_round(2.5), 2);
        assert_eq!(py_round(-2.5), -2);
        assert_eq!(py_round(2.6), 3);
    }

    #[test]
    fn floats_corpus() {
        // Python reference values: tests/golden/primitives/floats.json
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/primitives/floats.json");
        let v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        let rows = v.as_array().or_else(|| v.get("samples").and_then(Value::as_array)).expect("rows");
        assert!(rows.len() > 100);
        for r in rows {
            let s = r.get("input").or_else(|| r.get("s")).and_then(Value::as_str).expect("input");
            let f: f64 = s.parse().unwrap();
            if let Some(w) = r.get("repr").and_then(Value::as_str) {
                assert_eq!(py_float_repr(f), w, "repr {s}");
            }
            if let Some(w) = r.get("round").and_then(Value::as_str).filter(|_| f.abs() < 1e15) {
                assert_eq!(py_round(f).to_string(), w, "round {s}");
            }
        }
    }

    #[test]
    fn pretty_json_layout() {
        let v = json!({"b": [1, 2, {"x": "я\"\n"}], "a": {}, "c": [], "d": 1.0});
        let got = String::from_utf8(dumps_json(&v).unwrap()).unwrap();
        let want = "{\n  \"b\": [\n    1,\n    2,\n    {\n      \"x\": \"я\\\"\\n\"\n    }\n  ],\n  \"a\": {},\n  \"c\": [],\n  \"d\": 1.0\n}\n";
        assert_eq!(got, want);
    }

    #[test]
    fn sorted_compact_and_ascii() {
        let v = json!({"b": "я", "a": [1, 2]});
        assert_eq!(dumps_with(&v, &COMPACT_SORTED).unwrap(), "{\"a\":[1,2],\"b\":\"я\"}");
        assert_eq!(dumps_with(&v, &SORTED_SPACED).unwrap(), "{\"a\": [1, 2], \"b\": \"я\"}");
        assert!(dumps_with(&v, &ASCII_SORTED_PRETTY).unwrap().contains("\\u044f"));
        // Python: json.dumps("\x7f\x80я\U0001F600") / ensure_ascii=False keeps DEL and C1 raw
        let s = json!("\u{7f}\u{80}я\u{1F600}\u{1}\"\\/");
        assert_eq!(
            dumps_with(&s, &ASCII_SORTED_PRETTY).unwrap(),
            "\"\\u007f\\u0080\\u044f\\ud83d\\ude00\\u0001\\\"\\\\/\""
        );
        assert_eq!(dumps_with(&s, &PRETTY).unwrap(), "\"\u{7f}\u{80}я\u{1F600}\\u0001\\\"\\\\/\"");
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}
