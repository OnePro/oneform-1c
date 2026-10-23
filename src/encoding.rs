//! Strict text decoding, byte round-trip verification and newline accounting (port of `encoding.py`).

use serde_json::{json, Map, Value};

use crate::canonical::sha256_hex;
use crate::errors::{
    diagnostic, ExtractionError, Result, CONFIG_INVALID, ENCODING_AMBIGUOUS, ENCODING_CONFLICT, ENCODING_INVALID,
    ENCODING_UNSUPPORTED, TEXT_CONTAINS_REPLACEMENT_CHAR,
};

pub const OVERRIDE_CODECS: [&str; 6] = ["auto", "utf-8", "utf-8-sig", "utf-16-le", "utf-16-be", "cp1251"];
const CANDIDATE_ORDER: [&str; 4] = ["utf-8", "utf-16-le", "utf-16-be", "cp1251"];

#[derive(Debug, Clone)]
pub struct DecodedText {
    /// Concrete codec without BOM handling: utf-8 | utf-16-le | utf-16-be | cp1251.
    pub codec: String,
    /// Empty when the payload had no BOM.
    pub bom_hex: String,
    /// bom | override | ascii | unique-candidate | equivalent-candidates
    pub decision: String,
    /// LF-normalized text.
    pub text: String,
    /// UTF-8, no BOM, LF.
    pub canonical: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub newlines: Value,
    pub diagnostics: Vec<Value>,
}

impl DecodedText {
    pub fn encoding_record(&self) -> Value {
        json!({
            "codec": self.codec,
            "bomHex": self.bom_hex,
            "decision": self.decision,
            "byteRoundTrip": true,
        })
    }
}

fn invalid(key: &str, reason: &str) -> ExtractionError {
    ExtractionError::new(ENCODING_INVALID, reason).with("key", key)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Python-compatible `UnicodeDecodeError.reason` for a failed strict UTF-8 decode.
fn utf8_reason(raw: &[u8], e: &std::str::Utf8Error) -> &'static str {
    match e.error_len() {
        None => "unexpected end of data",
        Some(_) => {
            let b = raw[e.valid_up_to()];
            if (0x80..=0xC1).contains(&b) || b >= 0xF5 {
                "invalid start byte"
            } else {
                "invalid continuation byte"
            }
        }
    }
}

fn decode_utf16(raw: &[u8], big: bool) -> std::result::Result<String, &'static str> {
    if raw.len() % 2 != 0 {
        return Err("truncated data");
    }
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| if big { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
        .collect();
    let mut out = String::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        if (0xD800..0xDC00).contains(&u) {
            match units.get(i + 1) {
                None => return Err("unexpected end of data"),
                Some(&n) if (0xDC00..0xE000).contains(&n) => {
                    let cp = 0x10000 + (((u as u32) - 0xD800) << 10) + ((n as u32) - 0xDC00);
                    out.push(char::from_u32(cp).expect("valid pair"));
                    i += 2;
                }
                Some(_) => return Err("illegal UTF-16 surrogate"),
            }
        } else if (0xDC00..0xE000).contains(&u) {
            return Err("illegal encoding");
        } else {
            out.push(char::from_u32(u as u32).expect("bmp"));
            i += 1;
        }
    }
    Ok(out)
}

fn decode_raw(bytes: &[u8], codec: &str) -> std::result::Result<String, String> {
    let r = match codec {
        "utf-8" => std::str::from_utf8(bytes).map(str::to_string).map_err(|e| utf8_reason(bytes, &e)),
        "utf-16-le" => decode_utf16(bytes, false),
        "utf-16-be" => decode_utf16(bytes, true),
        "cp1251" => {
            if bytes.contains(&0x98) {
                Err("character maps to <undefined>")
            } else {
                Ok(encoding_rs::WINDOWS_1251.decode_without_bom_handling(bytes).0.into_owned())
            }
        }
        other => return Err(format!("unknown codec {other}")),
    };
    r.map_err(|reason| format!("not valid {codec}: {reason}"))
}

fn forbidden(c: char) -> bool {
    let u = c as u32;
    matches!(u, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f..=0x9f)
}

/// First forbidden control character as (byte offset, char). The forbidden set is ASCII except the C1 controls,
/// which UTF-8 encodes as `C2 80..C2 9F`, so a byte scan finds them all.
fn first_forbidden(text: &str) -> Option<(usize, char)> {
    let b = text.as_bytes();
    for (i, &x) in b.iter().enumerate() {
        match x {
            0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f => return Some((i, x as char)),
            0xc2 if matches!(b.get(i + 1), Some(0x80..=0x9f)) => return Some((i, char::from(b[i + 1]))),
            _ => {}
        }
    }
    None
}

/// Code point index of the byte offset `i` of `text`.
fn char_offset(text: &str, i: usize) -> usize {
    text.as_bytes()[..i].iter().filter(|&&x| x & 0xc0 != 0x80).count()
}

/// Strict decode + control check. Returns (text, failure reason).
fn decode_checked(raw: &[u8], bom: &[u8], codec: &str) -> std::result::Result<String, String> {
    let text = decode_raw(&raw[bom.len()..], codec)?;
    if let Some((i, c)) = first_forbidden(&text) {
        debug_assert!(forbidden(c));
        return Err(format!("forbidden control character U+{:04X} at offset {}", c as u32, char_offset(&text, i)));
    }
    Ok(text)
}

/// Newline record. `runs` is present only for `kind == "mixed"`.
pub fn analyze_newlines(text: &str) -> Value {
    // CR and LF are ASCII: scanning the UTF-8 bytes is the same as scanning the characters.
    let b = text.as_bytes();
    let (mut crlf, mut lf, mut cr) = (0u64, 0u64, 0u64);
    let mut runs: Vec<(&'static str, u64)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let kind = match b[i] {
            b'\r' if b.get(i + 1) == Some(&b'\n') => {
                i += 1;
                crlf += 1;
                "CRLF"
            }
            b'\n' => {
                lf += 1;
                "LF"
            }
            b'\r' => {
                cr += 1;
                "CR"
            }
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        match runs.last_mut() {
            Some((k, n)) if *k == kind => *n += 1,
            _ => runs.push((kind, 1)),
        }
    }
    let present: Vec<&str> = [("CRLF", crlf), ("LF", lf), ("CR", cr)].iter().filter(|(_, n)| *n > 0).map(|(k, _)| *k).collect();
    let kind = match present.len() {
        0 => "none",
        1 => present[0],
        _ => "mixed",
    };
    let mut rec = Map::new();
    rec.insert("kind".into(), kind.into());
    rec.insert("counts".into(), json!({"CRLF": crlf, "LF": lf, "CR": cr}));
    if kind == "mixed" {
        rec.insert("runs".into(), Value::Array(runs.into_iter().map(|(k, n)| json!([k, n])).collect()));
    }
    rec.insert("finalNewline".into(), Value::Bool(text.ends_with('\n') || text.ends_with('\r')));
    Value::Object(rec)
}

/// `text.replace("\r\n", "\n").replace("\r", "\n")` in one pass (no copy without CR).
pub fn normalize_newlines(text: String) -> String {
    if !text.as_bytes().contains(&b'\r') {
        return text;
    }
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' {
            out.push_str(&text[start..i]);
            out.push('\n');
            i += if b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
            start = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[start..]);
    out
}

fn terminators(newlines: &Value) -> Vec<&'static str> {
    let kind = newlines["kind"].as_str().unwrap_or("none");
    match kind {
        "none" => vec![],
        "mixed" => newlines["runs"]
            .as_array()
            .map(|runs| {
                runs.iter()
                    .flat_map(|r| {
                        let k = r[0].as_str().unwrap_or("LF");
                        let n = r[1].as_u64().unwrap_or(0) as usize;
                        std::iter::repeat_n(term_str(k), n)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        k => {
            let n = newlines["counts"][k].as_u64().unwrap_or(0) as usize;
            vec![term_str(k); n]
        }
    }
}

fn term_str(k: &str) -> &'static str {
    match k {
        "CRLF" => "\r\n",
        "LF" => "\n",
        _ => "\r",
    }
}

fn encode_text(text: &str, codec: &str) -> std::result::Result<Vec<u8>, String> {
    match codec {
        "utf-8" => Ok(text.as_bytes().to_vec()),
        "utf-16-le" => Ok(text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()),
        "utf-16-be" => Ok(text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()),
        "cp1251" => {
            let (bytes, _, unmappable) = encoding_rs::WINDOWS_1251.encode(text);
            if unmappable {
                Err("character cannot be encoded in cp1251".into())
            } else {
                Ok(bytes.into_owned())
            }
        }
        other => Err(format!("unknown codec {other}")),
    }
}

/// Restore the exact original payload bytes from canonical bytes + a module record
/// (`encoding` {codec, bomHex} and `newlines` as stored in the manifest).
pub fn reconstruct(canonical: &[u8], record: &Value) -> std::result::Result<Vec<u8>, String> {
    let enc = &record["encoding"];
    let nl = &record["newlines"];
    let text = std::str::from_utf8(canonical).map_err(|e| e.to_string())?;
    let parts: Vec<&str> = text.split('\n').collect();
    let terms = terminators(nl);
    if terms.len() != parts.len() - 1 {
        return Err("newline record does not match canonical text".into());
    }
    let mut out = String::with_capacity(text.len() + terms.len());
    out.push_str(parts[0]);
    for (t, p) in terms.iter().zip(&parts[1..]) {
        out.push_str(t);
        out.push_str(p);
    }
    let bom_hex = enc["bomHex"].as_str().unwrap_or("");
    let mut bytes: Vec<u8> = (0..bom_hex.len() / 2)
        .map(|i| u8::from_str_radix(&bom_hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string()))
        .collect::<std::result::Result<_, _>>()?;
    bytes.extend(encode_text(&out, enc["codec"].as_str().unwrap_or("utf-8"))?);
    Ok(bytes)
}

pub fn decode_payload(raw: &[u8], key: &str, override_codec: &str) -> Result<DecodedText> {
    decode_payload_hashed(raw, key, override_codec, None)
}

/// [`decode_payload`] with the already known `sha256(raw)` (saves hashing the payload twice).
pub fn decode_payload_hashed(raw: &[u8], key: &str, override_codec: &str, raw_sha256: Option<&str>) -> Result<DecodedText> {
    if !OVERRIDE_CODECS.contains(&override_codec) {
        return Err(ExtractionError::new(CONFIG_INVALID, "unknown codec name").with("key", key).with("codec", override_codec));
    }
    if raw.starts_with(&[0xff, 0xfe, 0x00, 0x00]) || raw.starts_with(&[0x00, 0x00, 0xfe, 0xff]) {
        return Err(ExtractionError::new(ENCODING_UNSUPPORTED, "UTF-32 is not supported").with("key", key));
    }
    let mut bom: &[u8] = b"";
    let mut bom_codec: Option<&str> = None;
    for (sig, codec) in [(&[0xef, 0xbb, 0xbf][..], "utf-8"), (&[0xff, 0xfe][..], "utf-16-le"), (&[0xfe, 0xff][..], "utf-16-be")] {
        if raw.starts_with(sig) {
            bom = sig;
            bom_codec = Some(codec);
            break;
        }
    }

    let mut decision = "override";
    let codec: String;
    if override_codec == "auto" {
        if let Some(bc) = bom_codec {
            codec = bc.to_string();
            decision = "bom";
        } else {
            let (c, d) = auto(raw, key)?;
            return finish(raw, b"", &c, d, key, raw_sha256);
        }
    } else {
        codec = if override_codec == "utf-8-sig" { "utf-8".to_string() } else { override_codec.to_string() };
        if override_codec == "utf-8-sig" && bom_codec != Some("utf-8") {
            return Err(ExtractionError::new(ENCODING_CONFLICT, "utf-8-sig requires a UTF-8 BOM").with("key", key));
        }
        if let Some(bc) = bom_codec {
            if bc != codec {
                return Err(ExtractionError::new(ENCODING_CONFLICT, "BOM contradicts the encoding override")
                    .with("key", key)
                    .with("bom", bc)
                    .with("override", override_codec));
            }
        }
    }
    finish(raw, bom, &codec, decision, key, raw_sha256)
}

fn auto(raw: &[u8], key: &str) -> Result<(String, &'static str)> {
    if raw.is_ascii() && !raw.contains(&0) {
        return Ok(("utf-8".into(), "ascii"));
    }
    let mut texts: Vec<(&str, String)> = Vec::new();
    let mut reasons = Map::new();
    for codec in CANDIDATE_ORDER {
        if codec.starts_with("utf-16") && raw.len() % 2 != 0 {
            reasons.insert(codec.into(), "odd length".into());
            continue;
        }
        match decode_checked(raw, b"", codec) {
            Ok(t) => texts.push((codec, t)),
            Err(why) => {
                reasons.insert(codec.into(), why.into());
            }
        }
    }
    if texts.is_empty() {
        return Err(invalid(key, "no encoding candidate is valid").with("reasons", Value::Object(reasons)));
    }
    let first = &texts[0].1;
    if texts.iter().any(|(_, t)| t != first) {
        let names: Vec<Value> = texts.iter().map(|(c, _)| Value::from(*c)).collect();
        return Err(ExtractionError::new(
            ENCODING_AMBIGUOUS,
            "several encodings decode the payload differently; set an encoding override",
        )
        .with("key", key)
        .with("candidates", Value::Array(names)));
    }
    let decision = if texts.len() == 1 { "unique-candidate" } else { "equivalent-candidates" };
    Ok((texts[0].0.to_string(), decision))
}

fn finish(raw: &[u8], bom: &[u8], codec: &str, decision: &str, key: &str, raw_sha256: Option<&str>) -> Result<DecodedText> {
    let text = decode_checked(raw, bom, codec).map_err(|why| invalid(key, &why).with("codec", codec))?;
    let mut diags = Vec::new();
    if text.contains('\u{fffd}') {
        let count = text.matches('\u{fffd}').count() as u64;
        let first = text.chars().position(|c| c == '\u{fffd}').unwrap_or(0) as u64;
        diags.push(diagnostic(
            TEXT_CONTAINS_REPLACEMENT_CHAR,
            "warning",
            key,
            &[("count", count.into()), ("firstOffset", first.into())],
        ));
    }
    let newlines = analyze_newlines(&text);
    let norm = normalize_newlines(text);
    let canonical = norm.clone().into_bytes();
    Ok(DecodedText {
        codec: codec.to_string(),
        bom_hex: hex(bom),
        decision: decision.to_string(),
        text: norm,
        source_sha256: raw_sha256.map_or_else(|| sha256_hex(raw), str::to_string),
        output_sha256: sha256_hex(&canonical),
        canonical,
        newlines,
        diagnostics: diags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_bom() {
        let d = decode_payload(b"a\r\nb\n", "k", "auto").unwrap();
        assert_eq!((d.codec.as_str(), d.decision.as_str(), d.text.as_str()), ("utf-8", "ascii", "a\nb\n"));
        assert_eq!(d.newlines["kind"], "mixed");
        let d = decode_payload(b"\xef\xbb\xbf\xd1\x8f", "k", "auto").unwrap();
        assert_eq!((d.bom_hex.as_str(), d.decision.as_str(), d.text.as_str()), ("efbbbf", "bom", "я"));
    }

    #[test]
    fn cp1251_and_ambiguous() {
        let d = decode_payload(&[0xff], "k", "auto").unwrap();
        assert_eq!((d.codec.as_str(), d.text.as_str()), ("cp1251", "я"));
        let e = decode_payload(&[0x04, 0x00, 0x00], "k", "auto").unwrap_err();
        assert_eq!(e.code, ENCODING_INVALID);
        assert_eq!(decode_payload(b"x", "k", "klingon").unwrap_err().code, CONFIG_INVALID);
        assert_eq!(decode_payload(&[0xff, 0xfe, 0, 0], "k", "auto").unwrap_err().code, ENCODING_UNSUPPORTED);
        assert_eq!(decode_payload(b"abc", "k", "utf-8-sig").unwrap_err().code, ENCODING_CONFLICT);
    }

    #[test]
    fn forbidden_offsets_and_newline_runs() {
        // Python: _FORBIDDEN.search(text).start() is a code point offset; C1 controls are forbidden too.
        let e = decode_payload("\u{feff}яжa\u{1}".as_bytes(), "k", "auto").unwrap_err();
        assert_eq!(e.message, "forbidden control character U+0001 at offset 3");
        let e = decode_payload("\u{feff}я\u{85}".as_bytes(), "k", "auto").unwrap_err();
        assert_eq!(e.message, "forbidden control character U+0085 at offset 1");
        let e = decode_payload("\u{feff}\u{7f}".as_bytes(), "k", "auto").unwrap_err();
        assert_eq!(e.message, "forbidden control character U+007F at offset 0");
        assert!(decode_payload("\u{feff}\u{a0}\t\r\n".as_bytes(), "k", "auto").is_ok());
        let nl = analyze_newlines("a\r\n\r\nb\nc\r\rd\r\n");
        assert_eq!(nl["runs"], json!([["CRLF", 2], ["LF", 1], ["CR", 2], ["CRLF", 1]]));
        assert_eq!(nl["counts"], json!({"CRLF": 3, "LF": 1, "CR": 2}));
        assert_eq!(normalize_newlines("a\r\n\r\nb\nc\r\rd\r\nж".into()), "a\n\nb\nc\n\nd\nж");
        assert_eq!(analyze_newlines("x")["kind"], "none");
    }

    #[test]
    fn reconstruct_roundtrip() {
        for raw in [&b"a\r\nb\rc\nd"[..], b"\xef\xbb\xbfx\r\ny\r\n", &[0xcf, 0xf0, 0xe8, 0x0d, 0x0a]] {
            let d = decode_payload(raw, "k", "auto").unwrap();
            let rec = json!({"encoding": d.encoding_record(), "newlines": d.newlines});
            assert_eq!(reconstruct(&d.canonical, &rec).unwrap(), raw);
        }
    }
}
