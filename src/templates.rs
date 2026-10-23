//! Template content extraction (port of `templates.py`).

use serde_json::{Map, Value};

use crate::brace::{parse_metadata, Atom, Item, BARE, BASE64, QUOTED};
use crate::canonical::sha256_hex;
use crate::encoding::{decode_payload, DecodedText};
use crate::errors::{diagnostic, Result, TEMPLATE_KIND_UNKNOWN, TEMPLATE_LAYOUT_UNSUPPORTED};
use crate::metadata::TemplateInfo;

pub const HTML_TYPE_CODE: i64 = 3;

#[derive(Debug, Clone)]
pub struct TemplateOutput {
    /// `Template.html` | `Template.bin`
    pub content_name: String,
    pub content: Vec<u8>,
    pub definition: Value,
    pub decoded: Option<DecodedText>,
    pub decoded_sha256: Option<String>,
    pub diagnostics: Vec<Value>,
}

impl TemplateOutput {
    pub fn content_known(&self) -> bool {
        self.decoded.is_some()
    }
}

/// Strictly parse `{5,1,"<lang>",{#base64:...},0}`; None when the shape differs.
fn parse_html_wrapper(raw: &[u8]) -> Option<(String, Vec<u8>)> {
    let node = parse_metadata(raw).ok()?;
    let c = &node.children;
    if c.len() != 5
        || c[0] != Item::Atom(Atom::new(BARE, "5"))
        || c[1] != Item::Atom(Atom::new(BARE, "1"))
        || c[4] != Item::Atom(Atom::new(BARE, "0"))
    {
        return None;
    }
    let lang = match &c[2] {
        Item::Atom(a) if a.kind == QUOTED => a.value.clone(),
        _ => return None,
    };
    match &c[3] {
        Item::Atom(a) if a.kind == BASE64 => Some((lang, a.decode_base64().ok()?)),
        _ => None,
    }
}

pub fn build_template(info: &TemplateInfo, alias: &str, override_codec: &str) -> Result<TemplateOutput> {
    let component = format!("Templates/{}", info.uuid);
    let payload = &info.content_payload;
    let mut diags: Vec<Value> = Vec::new();
    let mut decoded: Option<DecodedText> = None;
    let mut decoded_sha: Option<String> = None;
    let mut language: Option<String> = None;
    let mut type_name = "Unknown";
    let mut content_name = "Template.bin";
    let mut content = payload.raw.clone();

    if info.type_code != HTML_TYPE_CODE {
        diags.push(diagnostic(
            TEMPLATE_KIND_UNKNOWN,
            "warning",
            &component,
            &[("templateTypeCode", info.type_code.into())],
        ));
    } else {
        match parse_html_wrapper(&payload.raw) {
            None => diags.push(diagnostic(
                TEMPLATE_LAYOUT_UNSUPPORTED,
                "warning",
                &component,
                &[("templateTypeCode", info.type_code.into())],
            )),
            Some((lang, html)) => {
                language = Some(lang);
                // encoding failures are fatal (never downgraded to .bin)
                let d = decode_payload(&html, &format!("{component}/Template"), override_codec)?;
                decoded_sha = Some(sha256_hex(&html));
                diags.extend(d.diagnostics.iter().cloned());
                type_name = "HTMLDocument";
                content_name = "Template.html";
                content = d.canonical.clone();
                decoded = Some(d);
            }
        }
    }

    let mut def = Map::new();
    def.insert("schemaVersion".into(), 1.into());
    def.insert("kind".into(), "TemplateReference".into());
    def.insert("templateId".into(), info.uuid.as_str().into());
    def.insert("originalName".into(), info.name.as_str().into());
    def.insert("synonym".into(), info.synonym.as_str().into());
    def.insert("alias".into(), alias.into());
    def.insert("templateType".into(), type_name.into());
    def.insert("templateTypeCode".into(), info.type_code.into());
    if let Some(l) = language {
        def.insert("languageCode".into(), l.into());
    }
    def.insert("content".into(), content_name.into());
    def.insert("contentKnown".into(), decoded.is_some().into());
    def.insert("status".into(), (if decoded.is_some() { "extracted" } else { "reference-only" }).into());
    def.insert("sourceSha256".into(), payload.sha256.as_str().into());
    Ok(TemplateOutput {
        content_name: content_name.to_string(),
        content,
        definition: Value::Object(def),
        decoded,
        decoded_sha256: decoded_sha,
        diagnostics: diags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::reconstruct;
    use crate::errors::ENCODING_AMBIGUOUS;
    use crate::metadata::Payload;
    use base64::Engine;
    use serde_json::json;

    const UID: &str = "68b792c0-6443-4ad3-86cf-adadc7d25135";
    const BOM: &[u8] = b"\xef\xbb\xbf";

    fn info(content: &[u8], type_code: i64) -> TemplateInfo {
        let p = Payload { locator: "container:0/entry:x".into(), raw: content.to_vec(), sha256: sha256_hex(content) };
        TemplateInfo {
            uuid: UID.into(),
            name: "Имя".into(),
            synonym: "Син".into(),
            type_code,
            header_payload: p.clone(),
            content_payload: p,
        }
    }

    fn wrap(html: &[u8], sep: &[u8]) -> Vec<u8> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(html).into_bytes();
        let chunks: Vec<&[u8]> = b64.chunks(8).collect();
        let mut out = BOM.to_vec();
        out.extend_from_slice(b"{5,1,\"ru\",{#base64:");
        out.extend_from_slice(&chunks.join(sep));
        out.extend_from_slice(b"},0}");
        out
    }

    #[test]
    fn html_template() {
        // port: tests/test_templates.py::test_html_template
        let mut html = BOM.to_vec();
        html.extend_from_slice("<!DOCTYPE html>\r\n<p>Привет</p>".as_bytes());
        let out = build_template(&info(&wrap(&html, b"\r\r\n"), 3), "PhotoTemplate", "auto").unwrap();
        assert_eq!(out.content_name, "Template.html");
        assert_eq!(out.content, "<!DOCTYPE html>\n<p>Привет</p>".as_bytes());
        let d = out.definition.as_object().unwrap();
        let keys: Vec<&str> = d.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "schemaVersion", "kind", "templateId", "originalName", "synonym", "alias", "templateType",
                "templateTypeCode", "languageCode", "content", "contentKnown", "status", "sourceSha256"
            ]
        );
        assert_eq!(d["templateType"], "HTMLDocument");
        assert_eq!(d["templateTypeCode"], 3);
        assert_eq!(d["languageCode"], "ru");
        assert_eq!(d["contentKnown"], true);
        assert!(out.content_known());
        let dec = out.decoded.as_ref().unwrap();
        let rec = json!({"encoding": dec.encoding_record(), "newlines": dec.newlines});
        assert_eq!(reconstruct(&out.content, &rec).unwrap(), html);
        assert_eq!(out.decoded_sha256.unwrap(), sha256_hex(&html));
    }

    #[test]
    fn unknown_type_code_is_binary() {
        // port: tests/test_templates.py::test_unknown_type_code_is_binary
        let raw = b"\x00\x01binary";
        let out = build_template(&info(raw, 7), "T", "auto").unwrap();
        assert_eq!(out.content_name, "Template.bin");
        assert_eq!(out.content, raw);
        assert_eq!(out.definition["templateType"], "Unknown");
        assert_eq!(out.definition["contentKnown"], false);
        assert!(out.definition.get("languageCode").is_none());
        let codes: Vec<&str> = out.diagnostics.iter().map(|x| x["code"].as_str().unwrap()).collect();
        assert_eq!(codes, [TEMPLATE_KIND_UNKNOWN]);
    }

    #[test]
    fn unexpected_shape_is_binary() {
        // port: tests/test_templates.py::test_unexpected_shape_is_binary
        let cases: Vec<Vec<u8>> = vec![
            [BOM, b"{5,1,\"ru\",{#base64:aGk=},1}"].concat(),
            [BOM, b"{5,1,\"ru\",{#base64:aGk=}}"].concat(),
            [BOM, b"{5,2,\"ru\",{#base64:aGk=},0}"].concat(),
            [BOM, b"{5,1,ru,{#base64:aGk=},0}"].concat(),
            [BOM, b"{4,1,\"ru\",{#base64:aGk=},0}"].concat(),
            [BOM, b"{5,1,\"ru\",aGk=,0}"].concat(),
            [BOM, b"{5,1,\"ru\",{#base64:aGk},0}"].concat(), // bad padding
            [BOM, b"{5,1,\"ru\",{#base64:a*k=},0}"].concat(),
            [BOM, b"{5,1,\"ru\",{#base64:aG\xc2\xa0k=},0}"].concat(), // NBSP is not strippable whitespace
            [BOM, b"not metadata"].concat(),
            b"\xff\xfe".to_vec(),
        ];
        for raw in cases {
            let out = build_template(&info(&raw, 3), "T", "auto").unwrap();
            assert_eq!(out.content_name, "Template.bin", "{raw:?}");
            assert_eq!(out.content, raw);
            let codes: Vec<&str> = out.diagnostics.iter().map(|x| x["code"].as_str().unwrap()).collect();
            assert_eq!(codes, [TEMPLATE_LAYOUT_UNSUPPORTED], "{raw:?}");
            assert_eq!(out.definition["status"], "reference-only");
        }
    }

    #[test]
    fn encoding_failure_is_fatal() {
        // port: tests/test_templates.py::test_encoding_failure_is_fatal
        let html = encoding_rs::WINDOWS_1251.encode("Привет").0.into_owned(); // even length -> ambiguous
        let e = build_template(&info(&wrap(&html, b"\r\n"), 3), "T", "auto").unwrap_err();
        assert_eq!(e.code, ENCODING_AMBIGUOUS);
        assert_eq!(e.params["key"], format!("Templates/{UID}/Template"));
        let out = build_template(&info(&wrap(&html, b"\r\n"), 3), "T", "cp1251").unwrap();
        assert_eq!(out.content, "Привет".as_bytes());
        let bad = [BOM, b"\xff\xfe"].concat();
        assert!(build_template(&info(&wrap(&bad, b"\r\n"), 3), "T", "auto").is_err());
    }
}
