//! `oneform clone-form`: add a new ordinary form to a configuration object by copying an existing form of it.
//!
//! The stream of an ordinary form does not hold the form's own identifier, so a copy of `Ext/Form.bin` is a valid
//! new form; what makes it a form of the object is the metadata file `Forms/<Name>.xml` (new `uuid`, `Name`,
//! `Synonym`) and the `<Form>` entry in the object's `ChildObjects`.

use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::errors::ExtractionError;

type Result<T> = std::result::Result<T, ExtractionError>;

pub const FORM_CLONE_INVALID: &str = "FORM_CLONE_INVALID";

fn err(message: impl Into<String>) -> ExtractionError {
    ExtractionError::new(FORM_CLONE_INVALID, message.into())
}

/// A 1C identifier: letters (any alphabet), digits and `_`, not starting with a digit.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Replace the text of the first element `<tag>..</tag>` at or after `from`.
fn replace_element_text(xml: &str, from: usize, tag: &str, text: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml[from..].find(&open)? + from + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(format!("{}{}{}", &xml[..start], xml_escape(text), &xml[end..]))
}

/// The metadata file of the new form: the source's with a new `uuid`, `Name` and `Synonym`.
fn new_form_metadata(source: &str, from: &str, name: &str, synonym: &str, uuid: &Uuid) -> Result<String> {
    let marker = "<Form uuid=\"";
    let at = source.find(marker).ok_or_else(|| err("the source form's metadata file has no <Form uuid=\"..\"> element"))?;
    let id_start = at + marker.len();
    let id_end = source[id_start..].find('"').ok_or_else(|| err("the source form's uuid is not closed"))? + id_start;
    let mut xml = format!("{}{}{}", &source[..id_start], uuid, &source[id_end..]);
    let name_tag = format!("<Name>{from}</Name>");
    let name_at = xml.find(&name_tag).ok_or_else(|| err(format!("the source form's metadata file has no {name_tag}")))?;
    xml = format!("{}<Name>{}</Name>{}", &xml[..name_at], xml_escape(name), &xml[name_at + name_tag.len()..]);
    if let Some(syn) = xml.find("<Synonym>") {
        if let Some(replaced) = replace_element_text(&xml, syn, "v8:content", synonym) {
            xml = replaced;
        }
    }
    Ok(xml)
}

/// Position of the `</ChildObjects>` that closes the `<ChildObjects>` at `open` (tabular sections nest their own).
fn matching_close(xml: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = open;
    loop {
        let next_open = xml[at..].find("<ChildObjects>").map(|p| p + at);
        let next_close = xml[at..].find("</ChildObjects>").map(|p| p + at)?;
        match next_open {
            Some(o) if o < next_close => {
                depth += 1;
                at = o + "<ChildObjects>".len();
            }
            _ => {
                depth -= 1;
                if depth == 0 {
                    return Some(next_close);
                }
                at = next_close + "</ChildObjects>".len();
            }
        }
    }
}

/// Add `<Form>name</Form>` after the last `<Form>` entry of the object's `ChildObjects`, keeping its indentation.
fn register_in_object(object_xml: &str, name: &str) -> Result<String> {
    let entry = format!("<Form>{}</Form>", xml_escape(name));
    let Some(co_start) = object_xml.find("<ChildObjects>") else {
        // an object without children: `<ChildObjects/>` or no list at all
        if let Some(p) = object_xml.find("<ChildObjects/>") {
            let block = format!("<ChildObjects>{entry}</ChildObjects>");
            return Ok(format!("{}{block}{}", &object_xml[..p], &object_xml[p + "<ChildObjects/>".len()..]));
        }
        let end = object_xml.find("</MetaDataObject>").ok_or_else(|| err("the object's metadata file has no </MetaDataObject>"))?;
        let close = object_xml[..end].rfind("</").ok_or_else(|| err("the object element is not closed"))?;
        return Ok(format!("{}<ChildObjects>{entry}</ChildObjects>{}", &object_xml[..close], &object_xml[close..]));
    };
    let co_end = matching_close(object_xml, co_start).ok_or_else(|| err("<ChildObjects> is not closed"))?;
    let body = &object_xml[co_start..co_end];
    if body.contains(&entry) {
        return Err(err(format!("the object already lists a form named {name}")));
    }
    let Some(last) = body.rfind("<Form>").map(|p| p + co_start) else {
        return Ok(format!("{}{entry}{}", &object_xml[..co_end], &object_xml[co_end..]));
    };
    let end = object_xml[last..].find("</Form>").ok_or_else(|| err("a <Form> entry is not closed"))? + last + "</Form>".len();
    let line_start = object_xml[..last].rfind('\n').map_or(0, |p| p + 1);
    let indent = &object_xml[line_start..last];
    let sep = if line_start > 0 && indent.chars().all(char::is_whitespace) {
        let eol = if object_xml[..line_start].ends_with("\r\n") { "\r\n" } else { "\n" };
        format!("{eol}{indent}")
    } else {
        String::new()
    };
    Ok(format!("{}{sep}{entry}{}", &object_xml[..end], &object_xml[end..]))
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|e| err(format!("cannot read {}: {e}", path.display())))?;
    String::from_utf8(bytes).map_err(|_| err(format!("{} is not UTF-8", path.display())))
}

fn object_xml_path(object_dir: &Path) -> Result<PathBuf> {
    let name = object_dir.file_name().and_then(|n| n.to_str()).ok_or_else(|| err("the object directory has no name"))?;
    let parent = object_dir.parent().ok_or_else(|| err("the object directory has no parent"))?;
    Ok(parent.join(format!("{name}.xml")))
}

/// Create form `name` in `object_dir` (e.g. `.../Catalogs/Контрагенты`) as a copy of its form `from`.
/// Returns the files created or changed, relative to the object directory's parent.
pub fn clone_form(object_dir: &Path, from: &str, name: &str, synonym: Option<&str>) -> Result<Vec<String>> {
    if !valid_name(name) {
        return Err(err(format!("{name:?} is not a valid form name (letters, digits and _, not starting with a digit)")));
    }
    let object_xml_file = object_xml_path(object_dir)?;
    let forms = object_dir.join("Forms");
    let src_meta = forms.join(format!("{from}.xml"));
    let src_bin = forms.join(from).join("Ext").join("Form.bin");
    if !src_meta.is_file() || !src_bin.is_file() {
        return Err(err(format!(
            "form {from} not found in {} (expected Forms/{from}.xml and Forms/{from}/Ext/Form.bin)",
            object_dir.display()
        )));
    }
    let dst_meta = forms.join(format!("{name}.xml"));
    let dst_dir = forms.join(name);
    if dst_meta.exists() || dst_dir.exists() {
        return Err(err(format!("form {name} already exists in {}", object_dir.display())));
    }
    let object_xml = read_text(&object_xml_file)?;
    let object_uuid: String = object_xml.find(" uuid=\"").map(|p| object_xml[p + 7..].chars().take(36).collect()).unwrap_or_default();
    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("oneform:clone-form:{object_uuid}:{name}").as_bytes());
    let meta = new_form_metadata(&read_text(&src_meta)?, from, name, synonym.unwrap_or(name), &uuid)?;
    let registered = register_in_object(&object_xml, name)?;
    // the new files first, the object's list last: a failure leaves nothing half-registered
    fs::create_dir_all(dst_dir.join("Ext")).map_err(|e| err(e.to_string()))?;
    fs::write(&dst_meta, meta.as_bytes()).map_err(|e| err(e.to_string()))?;
    fs::copy(&src_bin, dst_dir.join("Ext").join("Form.bin")).map_err(|e| err(e.to_string()))?;
    fs::write(&object_xml_file, registered.as_bytes()).map_err(|e| err(e.to_string()))?;
    let obj = object_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    Ok(vec![
        format!("{obj}/Forms/{name}.xml"),
        format!("{obj}/Forms/{name}/Ext/Form.bin"),
        format!("{obj}.xml (ChildObjects: +{name})"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked() {
        assert!(valid_name("НоваяФорма1") && valid_name("_x") && !valid_name("1x") && !valid_name("a b") && !valid_name(""));
    }

    #[test]
    fn registration_keeps_indentation_and_rejects_duplicates() {
        let xml = "<A>\n\t\t<ChildObjects>\n\t\t\t<Form>Один</Form>\n\t\t\t<Form>Два</Form>\n\t\t\t<Template>Т</Template>\n\t\t</ChildObjects>\n</A>";
        let out = register_in_object(xml, "Три").unwrap();
        assert!(out.contains("<Form>Два</Form>\n\t\t\t<Form>Три</Form>\n\t\t\t<Template>"), "{out}");
        assert!(register_in_object(&out, "Три").is_err());
        let nested = "<A><ChildObjects><Form>Один</Form><TabularSection><ChildObjects><Attribute/></ChildObjects></TabularSection></ChildObjects></A>";
        assert_eq!(
            register_in_object(nested, "Два").unwrap(),
            "<A><ChildObjects><Form>Один</Form><Form>Два</Form><TabularSection><ChildObjects><Attribute/></ChildObjects></TabularSection></ChildObjects></A>"
        );
        assert!(register_in_object("<M><A><Properties/></A></M>", "Один").unwrap_err().message.contains("MetaDataObject"));
        assert_eq!(
            register_in_object("<MetaDataObject><A><Properties/></A></MetaDataObject>", "Один").unwrap(),
            "<MetaDataObject><A><Properties/><ChildObjects><Form>Один</Form></ChildObjects></A></MetaDataObject>"
        );
        assert_eq!(
            register_in_object("<MetaDataObject><A><ChildObjects/></A></MetaDataObject>", "Один").unwrap(),
            "<MetaDataObject><A><ChildObjects><Form>Один</Form></ChildObjects></A></MetaDataObject>"
        );
        let flat = "<A><ChildObjects><Form>Один</Form></ChildObjects></A>";
        assert_eq!(
            register_in_object(flat, "Два").unwrap(),
            "<A><ChildObjects><Form>Один</Form><Form>Два</Form></ChildObjects></A>"
        );
    }

    #[test]
    fn metadata_gets_a_new_uuid_name_and_synonym() {
        let src = "<M><Form uuid=\"11111111-1111-1111-1111-111111111111\"><Properties><Name>Старая</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Старая форма</v8:content></v8:item></Synonym></Properties></Form></M>";
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"x");
        let out = new_form_metadata(src, "Старая", "Новая", "Новая форма", &id).unwrap();
        assert!(out.contains(&format!("uuid=\"{id}\"")) && out.contains("<Name>Новая</Name>") && out.contains("<v8:content>Новая форма</v8:content>"));
        assert!(!out.contains("1111"));
    }
}
