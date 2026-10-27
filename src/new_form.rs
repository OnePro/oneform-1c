//! `oneform new-form`: a blank ordinary form of a given kind in a configuration object or in the dump of an
//! external data processor / report.
//!
//! The form is made of an embedded template (`src/templates/new_form/*.form.bin`, mined from real forms by
//! `scripts/mine_blank_forms.py`): the standard command panels, the main attribute, no events, an empty module.
//! The main attribute of an ordinary form holds the `TypeId` of the owner's generated type, so the template carries a
//! placeholder GUID that is replaced by the `TypeId` of the target object (`CatalogObject.X`, `CatalogList.X`,
//! `DocumentObject.X`, `ExternalDataProcessorObject.X`, `ExternalReportObject.X`). The command writes the metadata file
//! `Forms/<Name>.xml` and `Forms/<Name>/Ext/Form.bin`, lists the form in the object's `ChildObjects`, optionally makes
//! it the default form, and extracts the object so that `src/Forms/<Name>/Form.xml` exists. If anything fails,
//! nothing stays changed.

use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::cli_epf::clean_path;
use crate::clone_form::{object_xml_path, read_text, register_in_object, valid_name, xml_escape};
use crate::errors::ExtractionError;
use crate::formbin::{extract_config_object, ConfigObject, ConfigRunOpts, FORM_BIN_NAME};

type Result<T> = std::result::Result<T, ExtractionError>;

pub const NEW_FORM_INVALID: &str = "NEW_FORM_INVALID";

fn err(message: impl Into<String>) -> ExtractionError {
    ExtractionError::new(NEW_FORM_INVALID, message.into())
}

const CATALOG_ITEM: &[u8] = include_bytes!("templates/new_form/catalog-item.form.bin");
const CATALOG_LIST: &[u8] = include_bytes!("templates/new_form/catalog-list.form.bin");
const DOCUMENT: &[u8] = include_bytes!("templates/new_form/document.form.bin");
const EXTERNAL: &[u8] = include_bytes!("templates/new_form/external.form.bin");
const FORM_META: &str = include_str!("templates/new_form/form_meta.xml");

/// The GUID the templates carry instead of the `TypeId` of their owner (see `scripts/mine_blank_forms.py`).
const PLACEHOLDER_CATALOG_ITEM: &str = "7f5a1c00-0000-4000-8000-000000000001";
const PLACEHOLDER_CATALOG_LIST: &str = "7f5a1c00-0000-4000-8000-000000000002";
const PLACEHOLDER_DOCUMENT: &str = "7f5a1c00-0000-4000-8000-000000000003";
const PLACEHOLDER_EXTERNAL: &str = "7f5a1c00-0000-4000-8000-000000000004";

pub const KINDS: [&str; 5] = ["processor", "report", "catalog-item", "catalog-list", "document"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Processor,
    Report,
    CatalogItem,
    CatalogList,
    Document,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        Ok(match s {
            "processor" => Kind::Processor,
            "report" => Kind::Report,
            "catalog-item" => Kind::CatalogItem,
            "catalog-list" => Kind::CatalogList,
            "document" => Kind::Document,
            _ => return Err(err(format!("unknown form kind {s:?}; expected one of {}", KINDS.join(", ")))),
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Kind::Processor => "processor",
            Kind::Report => "report",
            Kind::CatalogItem => "catalog-item",
            Kind::CatalogList => "catalog-list",
            Kind::Document => "document",
        }
    }

    /// The element under `MetaDataObject` of the object the form belongs to.
    fn root_element(self) -> &'static str {
        match self {
            Kind::Processor => "ExternalDataProcessor",
            Kind::Report => "ExternalReport",
            Kind::CatalogItem | Kind::CatalogList => "Catalog",
            Kind::Document => "Document",
        }
    }

    fn external(self) -> bool {
        matches!(self, Kind::Processor | Kind::Report)
    }

    /// Name of the generated type (before the object name) whose `TypeId` the main attribute holds.
    fn type_name(self) -> &'static str {
        match self {
            Kind::Processor => "ExternalDataProcessorObject",
            Kind::Report => "ExternalReportObject",
            Kind::CatalogItem => "CatalogObject",
            Kind::CatalogList => "CatalogList",
            Kind::Document => "DocumentObject",
        }
    }

    fn template(self) -> (&'static [u8], &'static str) {
        match self {
            Kind::CatalogItem => (CATALOG_ITEM, PLACEHOLDER_CATALOG_ITEM),
            Kind::CatalogList => (CATALOG_LIST, PLACEHOLDER_CATALOG_LIST),
            Kind::Document => (DOCUMENT, PLACEHOLDER_DOCUMENT),
            Kind::Processor | Kind::Report => (EXTERNAL, PLACEHOLDER_EXTERNAL),
        }
    }

    /// The property of the object that names its default form for this kind.
    fn default_property(self) -> &'static str {
        match self {
            Kind::Processor | Kind::Report => "DefaultForm",
            Kind::CatalogItem | Kind::Document => "DefaultObjectForm",
            Kind::CatalogList => "DefaultListForm",
        }
    }

    /// The directory of the object class a configuration object lies in (none for external objects).
    fn class_dir(self) -> Option<&'static str> {
        match self {
            Kind::CatalogItem | Kind::CatalogList => Some("Catalogs"),
            Kind::Document => Some("Documents"),
            Kind::Processor | Kind::Report => None,
        }
    }
}

/// Name of the first element under `<MetaDataObject>`.
fn root_element_name(xml: &str) -> Option<&str> {
    let at = xml.find("<MetaDataObject")?;
    let rest = &xml[at..];
    let open_end = rest.find('>')?;
    let after = rest[open_end + 1..].trim_start();
    let tag = after.strip_prefix('<')?;
    let end = tag.find(|c: char| c.is_whitespace() || c == '>' || c == '/')?;
    Some(&tag[..end])
}

/// Text of the first `<tag>..</tag>` at or after `from`.
fn element_text<'a>(xml: &'a str, from: usize, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml[from..].find(&open)? + from + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))? + start;
    Some(&xml[start..end])
}

/// `TypeId` of the generated type `<type_name>.<object>`.
fn generated_type_id(xml: &str, type_name: &str, object: &str) -> Option<String> {
    let marker = format!("GeneratedType name=\"{type_name}.{object}\"");
    let at = xml.find(&marker)?;
    Some(element_text(xml, at, "xr:TypeId")?.trim().to_string()).filter(|id| id.len() == 36)
}

/// Replace the value of the object's property `tag` (`<tag/>` or `<tag>..</tag>`, before `ChildObjects`).
fn set_property(xml: &str, tag: &str, value: &str) -> Option<String> {
    let limit = xml.find("<ChildObjects").unwrap_or(xml.len());
    let new = format!("<{tag}>{}</{tag}>", xml_escape(value));
    let empty = format!("<{tag}/>");
    if let Some(p) = xml[..limit].find(&empty) {
        return Some(format!("{}{new}{}", &xml[..p], &xml[p + empty.len()..]));
    }
    let open = format!("<{tag}>");
    let start = xml[..limit].find(&open)?;
    let close = format!("</{tag}>");
    let end = xml[start..].find(&close)? + start + close.len();
    Some(format!("{}{new}{}", &xml[..start], &xml[end..]))
}

/// The metadata file of the new form: the same text as the Configurator writes (BOM, the object's line endings).
fn form_metadata(object_xml: &str, name: &str, synonym: &str, uuid: &Uuid, external: bool) -> String {
    let version = object_xml
        .find("<MetaDataObject")
        .and_then(|at| {
            let tag_end = object_xml[at..].find('>')? + at;
            let v = object_xml[at..tag_end].rfind("version=\"")? + at + "version=\"".len();
            object_xml[v..tag_end].split('"').next().map(str::to_string)
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "2.20".to_string());
    let extended = if external { "\t\t\t<ExtendedPresentation/>\n" } else { "" };
    let text = FORM_META
        .replace("{{VERSION}}", &version)
        .replace("{{UUID}}", &uuid.to_string())
        .replace("{{NAME}}", &xml_escape(name))
        .replace("{{SYNONYM}}", &xml_escape(synonym))
        .replace("{{EXTENDED}}\n", extended);
    let text = if object_xml.contains("\r\n") { text.replace('\n', "\r\n") } else { text };
    format!("\u{feff}{text}")
}

/// The `Form.bin` of the new form: the template with the owner's `TypeId` put in.
fn form_bin(kind: Kind, type_id: &str) -> Result<Vec<u8>> {
    let (template, placeholder) = kind.template();
    let (from, to) = (placeholder.as_bytes(), type_id.as_bytes());
    if from.len() != to.len() {
        return Err(err(format!("{type_id:?} is not a GUID")));
    }
    let mut out = template.to_vec();
    let mut found = 0;
    let mut i = 0;
    while i + from.len() <= out.len() {
        if &out[i..i + from.len()] == from {
            out[i..i + from.len()].copy_from_slice(to);
            found += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    if found == 0 {
        return Err(err("the form template has no place for the owner's TypeId (broken build)"));
    }
    Ok(out)
}

/// Undo of a failed run: the new files and directories are removed, the object's file is restored.
struct Rollback {
    object_file: PathBuf,
    object_text: String,
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    object_written: bool,
}

impl Rollback {
    fn undo(&self) {
        if self.object_written {
            let _ = fs::write(&self.object_file, self.object_text.as_bytes());
        }
        for f in &self.files {
            let _ = fs::remove_file(f);
        }
        for d in &self.dirs {
            let _ = fs::remove_dir_all(d);
        }
    }
}

/// What the command did, for the report of the CLI.
pub struct NewForm {
    pub files: Vec<String>,
    pub extract: String,
}

/// Create the blank form `name` of `kind` in `object_dir` (e.g. `.../Catalogs/Контрагенты` or `.../src/МояОбработка`).
pub fn new_form(object_dir: &Path, kind: Kind, name: &str, synonym: Option<&str>, make_default: bool) -> Result<NewForm> {
    new_form_with(object_dir, kind, name, synonym, make_default, false)
}

/// [`new_form`]; `replace_generated` lets the extract of the object replace `src` files made by another oneform version.
pub fn new_form_with(
    object_dir: &Path,
    kind: Kind,
    name: &str,
    synonym: Option<&str>,
    make_default: bool,
    replace_generated: bool,
) -> Result<NewForm> {
    if !valid_name(name) {
        return Err(err(format!("{name:?} is not a valid form name (letters, digits and _, not starting with a digit)")));
    }
    let ext = object_dir.extension().and_then(|e| e.to_str()).map(str::to_lowercase);
    if object_dir.is_file() || matches!(ext.as_deref(), Some("epf" | "erf")) {
        return Err(err(format!(
            "{} is a binary file; new-form works on the dump of an external processor or report: run epf-dump / erf-dump first, then give the object directory of the dump",
            object_dir.display()
        )));
    }
    if !object_dir.is_dir() {
        return Err(err(format!("{} is not a directory", object_dir.display())));
    }
    let object_dir = &clean_path(object_dir);
    let obj_name = object_dir.file_name().and_then(|n| n.to_str()).ok_or_else(|| err("the object directory has no name"))?.to_string();
    let object_xml_file = object_xml_path(object_dir)?;
    if !object_xml_file.is_file() {
        return Err(err(format!(
            "{} not found: {} is not an object directory of a dump (the object's metadata file must lie next to it)",
            object_xml_file.display(),
            object_dir.display()
        )));
    }
    let object_text = read_text(&object_xml_file)?;
    let found = root_element_name(&object_text).unwrap_or("?");
    if found != kind.root_element() {
        return Err(err(format!(
            "--kind {} needs an object of type {}, but {} is {found}",
            kind.as_str(),
            kind.root_element(),
            object_xml_file.display()
        )));
    }
    let props = object_text.find("<Properties>").unwrap_or(0);
    if element_text(&object_text, props, "Name") != Some(obj_name.as_str()) {
        return Err(err(format!("the <Name> of {} is not {obj_name}", object_xml_file.display())));
    }
    let type_id = generated_type_id(&object_text, kind.type_name(), &obj_name).ok_or_else(|| {
        err(format!(
            "{} has no <xr:GeneratedType name=\"{}.{obj_name}\"> with a TypeId (an object of a dump of the platform has it)",
            object_xml_file.display(),
            kind.type_name()
        ))
    })?;
    let forms = object_dir.join("Forms");
    let meta_file = forms.join(format!("{name}.xml"));
    let form_dir = forms.join(name);
    if meta_file.exists() || form_dir.exists() {
        return Err(err(format!("form {name} already exists in {}", object_dir.display())));
    }
    let mut new_text = register_in_object(&object_text, name)?;
    if make_default {
        let value = format!("{}.{obj_name}.Form.{name}", kind.root_element());
        new_text = set_property(&new_text, kind.default_property(), &value)
            .ok_or_else(|| err(format!("the object has no <{}> property to make {name} its default form", kind.default_property())))?;
    }
    let object_uuid: String = object_text.find(" uuid=\"").map(|p| object_text[p + 7..].chars().take(36).collect()).unwrap_or_default();
    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("oneform:new-form:{object_uuid}:{name}").as_bytes());
    let meta = form_metadata(&object_text, name, synonym.unwrap_or(name), &uuid, kind.external());
    let bin = form_bin(kind, &type_id)?;

    // the new files first, the object's list last
    let mut undo = Rollback {
        object_file: object_xml_file.clone(),
        object_text: object_text.clone(),
        files: vec![meta_file.clone()],
        dirs: vec![if forms.exists() { form_dir.clone() } else { forms.clone() }],
        object_written: false,
    };
    let written = (|| -> std::io::Result<()> {
        fs::create_dir_all(form_dir.join("Ext"))?;
        fs::write(&meta_file, meta.as_bytes())?;
        fs::write(form_dir.join("Ext").join(FORM_BIN_NAME), &bin)?;
        undo.object_written = true;
        fs::write(&object_xml_file, new_text.as_bytes())
    })();
    if let Err(e) = written {
        undo.undo();
        return Err(err(e.to_string()));
    }
    let extract = match extract_new_form(object_dir, kind, &obj_name, replace_generated) {
        Ok(s) => s,
        Err(e) => {
            undo.undo();
            return Err(err(format!("extract of the new form failed, nothing was changed: {}", e.message)));
        }
    };
    let mut changed = format!("{obj_name}.xml (ChildObjects: +{name}");
    if make_default {
        changed.push_str(&format!(", {}", kind.default_property()));
    }
    changed.push(')');
    Ok(NewForm {
        files: vec![
            format!("{obj_name}/Forms/{name}.xml"),
            format!("{obj_name}/Forms/{name}/Ext/Form.bin"),
            changed,
            format!("{obj_name}/src/Forms/{name}/Form.xml"),
        ],
        extract,
    })
}

/// The normal extract of the object (the root is the same as when `extract` runs on the whole dump).
fn extract_new_form(object_dir: &Path, kind: Kind, obj_name: &str, replace_generated: bool) -> Result<String> {
    let parent = object_dir.parent().ok_or_else(|| err("the object directory has no parent"))?;
    let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let (root, rel, class) = match (kind.class_dir(), parent.parent()) {
        (Some(class), Some(root)) if parent_name == class => (root.to_path_buf(), format!("{class}/{obj_name}"), class.to_string()),
        _ => (parent.to_path_buf(), obj_name.to_string(), parent_name.to_string()),
    };
    let mut forms: Vec<String> = fs::read_dir(object_dir.join("Forms"))
        .map_err(|e| err(e.to_string()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("Ext").join(FORM_BIN_NAME).is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    forms.sort();
    let obj = ConfigObject { rel, object_class: class, forms, common: false };
    let run = ConfigRunOpts { replace_generated, ..ConfigRunOpts::default() };
    extract_config_object(&root, &obj, &run).map(|o| format!("{} {}", o.path, o.status)).map_err(|e| err(e.message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_element_and_type_id_are_read() {
        let xml = "<?xml version=\"1.0\"?>\n<MetaDataObject xmlns=\"x\" version=\"2.17\">\n\t<Catalog uuid=\"a\"><InternalInfo><xr:GeneratedType name=\"CatalogObject.Т\" category=\"Object\"><xr:TypeId>5a4cef5b-3d14-40c1-9724-8a9a4d6560ba</xr:TypeId></xr:GeneratedType></InternalInfo></Catalog></MetaDataObject>";
        assert_eq!(root_element_name(xml), Some("Catalog"));
        assert_eq!(generated_type_id(xml, "CatalogObject", "Т").as_deref(), Some("5a4cef5b-3d14-40c1-9724-8a9a4d6560ba"));
        assert_eq!(generated_type_id(xml, "CatalogList", "Т"), None);
    }

    #[test]
    fn default_form_property_is_replaced_empty_or_filled() {
        let a = "<P><DefaultObjectForm/><DefaultListForm>Catalog.Т.Form.Старая</DefaultListForm></P><ChildObjects/>";
        assert_eq!(set_property(a, "DefaultObjectForm", "Catalog.Т.Form.Н").unwrap(), "<P><DefaultObjectForm>Catalog.Т.Form.Н</DefaultObjectForm><DefaultListForm>Catalog.Т.Form.Старая</DefaultListForm></P><ChildObjects/>");
        assert!(set_property(a, "DefaultListForm", "Catalog.Т.Form.Н").unwrap().contains("<DefaultListForm>Catalog.Т.Form.Н</DefaultListForm>"));
        assert!(set_property(a, "DefaultForm", "x").is_none());
        // a property after ChildObjects is not the object's own
        assert!(set_property("<P/><ChildObjects/><DefaultForm/>", "DefaultForm", "x").is_none());
    }

    #[test]
    fn metadata_follows_the_object_style() {
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"x");
        let m = form_metadata("<MetaDataObject version=\"2.13\">\r\n", "Ф&1", "Форма <1>", &id, false);
        assert!(m.starts_with('\u{feff}') && m.contains("version=\"2.13\"") && m.contains("\r\n") && !m.contains("ExtendedPresentation"), "{m}");
        assert!(m.contains("<Name>Ф&amp;1</Name>") && m.contains("<v8:content>Форма &lt;1&gt;</v8:content>") && m.contains(&id.to_string()));
        let e = form_metadata("<MetaDataObject>", "Ф", "Ф", &id, true);
        assert!(e.contains("version=\"2.20\"") && e.contains("<ExtendedPresentation/>") && !e.contains("\r\n") && !e.contains("{{"), "{e}");
    }

    #[test]
    fn templates_take_the_owner_type_id() {
        for kind in [Kind::CatalogItem, Kind::CatalogList, Kind::Document, Kind::Processor, Kind::Report] {
            let id = "11111111-2222-4333-8444-555555555555";
            let bin = form_bin(kind, id).unwrap();
            let (template, placeholder) = kind.template();
            assert_eq!(bin.len(), template.len());
            assert!(!bin.windows(36).any(|w| w == placeholder.as_bytes()));
            assert!(bin.windows(36).any(|w| w == id.as_bytes()));
        }
        assert!(form_bin(Kind::Document, "short").is_err());
    }
}
