//! Metadata reference and type resolution for 1C ordinary forms.
//!
//! Maps between metadata reference TypeIds (GUIDs in binary `FormStream.bin`)
//! and human-readable names (e.g. `Справочник.Контрагенты` / `CatalogRef.Контрагенты`),
//! based on the `<xr:GeneratedType>` declarations in 1C configuration metadata XML files
//! (`Catalogs/*.xml`, `Documents/*.xml`, etc.).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Platform built-in type: `ValueTable`.
pub const VALUE_TABLE_GUID: &str = "acf6192e-81ca-46ef-93a6-5a6968b78663";
/// Platform built-in type: `ValueTree`.
pub const VALUE_TREE_GUID: &str = "e603c0f2-92fb-4d47-8f38-a44a381cf235";
/// Platform built-in type: `ValueList`.
pub const VALUE_LIST_GUID: &str = "4772b3b4-f4a3-49c0-a1a5-8cb5961511a3";
/// Platform built-in type: `AnyRef` (any reference).
pub const ANY_REF_GUID: &str = "e61ef7b8-f3e1-4f4b-8ac7-676e90524997";

/// Configuration subdirectories whose `*.xml` files carry `<xr:GeneratedType>` declarations.
const METADATA_SUBDIRS: [&str; 15] = [
    "Catalogs",
    "Documents",
    "Enums",
    "ChartsOfCharacteristicTypes",
    "ChartsOfAccounts",
    "ChartsOfCalculationTypes",
    "ExchangePlans",
    "BusinessProcesses",
    "Tasks",
    "DataProcessors",
    "Reports",
    "InformationRegisters",
    "AccumulationRegisters",
    "AccountingRegisters",
    "CalculationRegisters",
];

type Fingerprint = (usize, u64, Option<SystemTime>);

/// Bidirectional mapping between 1C metadata type GUIDs and readable names/aliases.
#[derive(Debug, Clone, Default)]
pub struct ConfigTypeLookup {
    guid_to_name: HashMap<String, String>,
    name_to_guid: HashMap<String, String>,
}

impl ConfigTypeLookup {
    /// Creates an empty lookup pre-populated with standard platform types.
    pub fn new() -> Self {
        let mut lookup = Self::default();
        lookup.register_builtins();
        lookup
    }

    fn register_builtins(&mut self) {
        let builtins = [
            (VALUE_TABLE_GUID, "ValueTable", &["ТаблицаЗначений", "ValueTable"][..]),
            (VALUE_TREE_GUID, "ValueTree", &["ДеревоЗначений", "ValueTree"][..]),
            (VALUE_LIST_GUID, "ValueList", &["СписокЗначений", "ValueList"][..]),
            (ANY_REF_GUID, "AnyRef", &["ЛюбаяСсылка", "AnyRef"][..]),
        ];
        for (guid, canonical, aliases) in builtins {
            self.guid_to_name.insert(guid.to_string(), canonical.to_string());
            for alias in aliases {
                self.name_to_guid.insert(alias.to_lowercase(), guid.to_string());
            }
        }
    }

    /// Automatically discover and load configuration type lookup starting from `start`.
    /// Walks up ancestor directories looking for `Configuration.xml` or `ConfigDumpInfo.xml`.
    pub fn find_and_load(start: &Path) -> Self {
        if let Some(root) = Self::find_config_root(start) {
            Self::from_config_dir(&root)
        } else {
            Self::new()
        }
    }

    /// Walk up from `start` to find the directory containing `Configuration.xml`.
    pub fn find_config_root(start: &Path) -> Option<PathBuf> {
        let mut curr = if start.is_file() {
            start.parent()?.to_path_buf()
        } else {
            start.to_path_buf()
        };
        loop {
            if curr.join("Configuration.xml").is_file() || curr.join("ConfigDumpInfo.xml").is_file() {
                return Some(curr);
            }
            if !curr.pop() {
                break;
            }
        }
        None
    }

    /// Process-wide cached lookup for `config_root` (keyed by canonical path).
    ///
    /// The cached entry is rebuilt when the scanned files change (file count, total size or newest mtime).
    pub fn for_config_root(config_root: &Path) -> Arc<ConfigTypeLookup> {
        type Cache = Mutex<HashMap<PathBuf, (Fingerprint, Arc<ConfigTypeLookup>)>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let key = fs::canonicalize(config_root).unwrap_or_else(|_| config_root.to_path_buf());
        let fingerprint = fingerprint_of(&key);
        let mut guard = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
        if let Some((fp, lookup)) = guard.get(&key) {
            if *fp == fingerprint {
                return Arc::clone(lookup);
            }
        }
        let lookup = Arc::new(Self::from_config_dir(&key));
        guard.insert(key, (fingerprint, Arc::clone(&lookup)));
        lookup
    }

    /// Scans the known metadata subdirectories of a configuration root (`Catalogs/*.xml`, `Documents/*.xml`, ...)
    /// and builds the lookup from their `<xr:GeneratedType>` declarations.
    pub fn from_config_dir(config_root: &Path) -> Self {
        let mut lookup = Self::new();
        for sub in METADATA_SUBDIRS {
            lookup.scan_metadata_subdir(&config_root.join(sub));
        }
        lookup
    }

    fn scan_metadata_subdir(&mut self, subdir: &Path) {
        for path in xml_files_in(subdir) {
            if let Ok(content) = fs::read_to_string(&path) {
                // `InternalInfo` is always at the top of a metadata file.
                let head = content.find("</InternalInfo>").map_or(content.as_str(), |i| &content[..i]);
                self.parse_generated_types(head);
            }
        }
    }

    /// Extracts `<xr:GeneratedType name="..." category="...">...<xr:TypeId>...</xr:TypeId>`
    fn parse_generated_types(&mut self, xml: &str) {
        let mut pos = 0;
        while let Some(gen_start) = xml[pos..].find("<xr:GeneratedType") {
            let start = pos + gen_start;
            let tag_end = match xml[start..].find('>') {
                Some(i) => start + i,
                None => break,
            };
            let tag_attrs = &xml[start..tag_end];
            let name = extract_attr(tag_attrs, "name");
            let category = extract_attr(tag_attrs, "category");

            // Look for </xr:GeneratedType>
            let gen_end = match xml[tag_end..].find("</xr:GeneratedType>") {
                Some(i) => tag_end + i,
                None => {
                    pos = tag_end;
                    continue;
                }
            };
            let inner = &xml[tag_end..gen_end];
            if let Some(type_id) = extract_inner_text(inner, "xr:TypeId") {
                let guid = type_id.trim().to_lowercase();
                if let Some(n) = name {
                    self.register_type(&guid, &n, category.as_deref());
                }
            }
            pos = gen_end + "</xr:GeneratedType>".len();
        }
    }

    fn register_type(&mut self, guid: &str, name: &str, category: Option<&str>) {
        let canonical = make_canonical_name(name, category);
        self.guid_to_name.insert(guid.to_string(), canonical.clone());
        self.name_to_guid.insert(name.to_lowercase(), guid.to_string());

        // Short aliases belong to the reference type only: `Object`/`List`/`Manager`/... types of the
        // same metadata object must never take them over.
        if category != Some("Ref") {
            return;
        }
        self.name_to_guid.insert(canonical.to_lowercase(), guid.to_string());
        if let Some((kind, obj)) = name.split_once('.') {
            if let Some(ru_kind) = ru_prefix(kind) {
                self.name_to_guid.insert(format!("{ru_kind}.{obj}").to_lowercase(), guid.to_string());
                self.name_to_guid.insert(format!("{ru_kind}Ссылка.{obj}").to_lowercase(), guid.to_string());
            }
            let bare_en = kind.strip_suffix("Ref").unwrap_or(kind);
            self.name_to_guid.insert(format!("{bare_en}.{obj}").to_lowercase(), guid.to_string());
        }
    }

    /// Lookup readable name for a type GUID (e.g. `"47e6dd42-..."` -> `"Справочник.Контрагенты"`).
    pub fn resolve_guid(&self, guid: &str) -> Option<&str> {
        self.guid_to_name.get(&guid.trim().to_lowercase()).map(String::as_str)
    }

    /// Lookup type GUID for a name or alias (e.g. `"Справочник.Контрагенты"` -> `"47e6dd42-..."`).
    pub fn resolve_name(&self, name: &str) -> Option<&str> {
        self.name_to_guid.get(&name.trim().to_lowercase()).map(String::as_str)
    }

    /// Iterate over all registered GUID -> canonical name mappings.
    pub fn all_guid_names(&self) -> impl Iterator<Item = (&str, &str)> {
        self.guid_to_name.iter().map(|(g, n)| (g.as_str(), n.as_str()))
    }
}

/// Value of `attr="..."` in a tag; `attr` must be preceded by whitespace (so `id=` never matches `uuid=`).
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pat = format!("{attr}=\"");
    tag.match_indices(&pat).find_map(|(i, _)| {
        if !tag[..i].ends_with(char::is_whitespace) {
            return None;
        }
        let at = i + pat.len();
        let end = at + tag[at..].find('"')?;
        Some(tag[at..end].to_string())
    })
}

fn xml_files_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(files) = fs::read_dir(dir) else { return Vec::new() };
    files
        .flatten()
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("xml")))
        .collect()
}

/// Cheap change detector for the scanned files: `(file count, total size, newest mtime)`.
fn fingerprint_of(config_root: &Path) -> Fingerprint {
    let mut count = 0;
    let mut size = 0;
    let mut newest = None;
    for sub in METADATA_SUBDIRS {
        for path in xml_files_in(&config_root.join(sub)) {
            let Ok(meta) = fs::metadata(&path) else { continue };
            count += 1;
            size += meta.len();
            newest = newest.max(meta.modified().ok());
        }
    }
    (count, size, newest)
}

fn extract_inner_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let at = xml.find(&open)? + open.len();
    let end = at + xml[at..].find(&close)?;
    Some(xml[at..end].to_string())
}

fn ru_prefix(kind: &str) -> Option<&'static str> {
    match kind {
        "CatalogRef" | "Catalog" => Some("Справочник"),
        "DocumentRef" | "Document" => Some("Документ"),
        "EnumRef" | "Enum" => Some("Перечисление"),
        "ChartOfCharacteristicTypesRef" | "ChartOfCharacteristicTypes" => Some("ПланВидовХарактеристик"),
        "ChartOfAccountsRef" | "ChartOfAccounts" => Some("ПланСчетов"),
        "ChartOfCalculationTypesRef" | "ChartOfCalculationTypes" => Some("ПланВидовРасчета"),
        "ExchangePlanRef" | "ExchangePlan" => Some("ПланОбмена"),
        "BusinessProcessRef" | "BusinessProcess" => Some("БизнесПроцесс"),
        "TaskRef" | "Task" => Some("Задача"),
        _ => None,
    }
}

fn make_canonical_name(name: &str, category: Option<&str>) -> String {
    if category == Some("Ref") {
        if let Some((kind, obj)) = name.split_once('.') {
            if let Some(ru) = ru_prefix(kind) {
                return format!("{ru}.{obj}");
            }
        }
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_generated_types_sample() {
        let sample = r#"
            <Catalog uuid="b6412cac-a3e5-4c01-b215-6903507b9229">
                <InternalInfo>
                    <xr:GeneratedType name="CatalogObject.Контрагенты" category="Object">
                        <xr:TypeId>f04b5444-61be-425a-b941-9a6144713f21</xr:TypeId>
                        <xr:ValueId>3cf280a0-a7f2-4df3-88ee-6ad4c879cfbf</xr:ValueId>
                    </xr:GeneratedType>
                    <xr:GeneratedType name="CatalogRef.Контрагенты" category="Ref">
                        <xr:TypeId>47e6dd42-cd5c-4e3a-9bc4-14816ed53d38</xr:TypeId>
                        <xr:ValueId>54fde65c-b39d-431b-a3f2-0c008773c8bd</xr:ValueId>
                    </xr:GeneratedType>
                </InternalInfo>
            </Catalog>
        "#;
        let mut lookup = ConfigTypeLookup::new();
        lookup.parse_generated_types(sample);

        // GUID -> Canonical Name
        assert_eq!(lookup.resolve_guid("47e6dd42-cd5c-4e3a-9bc4-14816ed53d38"), Some("Справочник.Контрагенты"));

        // Name aliases -> GUID
        assert_eq!(lookup.resolve_name("Справочник.Контрагенты"), Some("47e6dd42-cd5c-4e3a-9bc4-14816ed53d38"));
        assert_eq!(lookup.resolve_name("СправочникСсылка.Контрагенты"), Some("47e6dd42-cd5c-4e3a-9bc4-14816ed53d38"));
        assert_eq!(lookup.resolve_name("CatalogRef.Контрагенты"), Some("47e6dd42-cd5c-4e3a-9bc4-14816ed53d38"));
        assert_eq!(lookup.resolve_name("Catalog.Контрагенты"), Some("47e6dd42-cd5c-4e3a-9bc4-14816ed53d38"));

        // Builtins still work
        assert_eq!(lookup.resolve_name("ТаблицаЗначений"), Some("acf6192e-81ca-46ef-93a6-5a6968b78663"));
        assert_eq!(lookup.resolve_guid("acf6192e-81ca-46ef-93a6-5a6968b78663"), Some("ValueTable"));
    }

    #[test]
    fn non_ref_types_never_take_over_ref_aliases() {
        let sample = r#"<xr:GeneratedType name="CatalogRef.Т" category="Ref"><xr:TypeId>11111111-1111-1111-1111-111111111111</xr:TypeId></xr:GeneratedType>
            <xr:GeneratedType name="CatalogObject.Т" category="Object"><xr:TypeId>22222222-2222-2222-2222-222222222222</xr:TypeId></xr:GeneratedType>"#;
        let mut lookup = ConfigTypeLookup::new();
        lookup.parse_generated_types(sample);
        assert_eq!(lookup.resolve_name("Справочник.Т"), Some("11111111-1111-1111-1111-111111111111"));
        assert_eq!(lookup.resolve_name("Catalog.Т"), Some("11111111-1111-1111-1111-111111111111"));
        assert_eq!(lookup.resolve_name("CatalogObject.Т"), Some("22222222-2222-2222-2222-222222222222"));
    }

    #[test]
    fn extract_attr_requires_leading_whitespace() {
        assert_eq!(extract_attr(r#"<Metadata uuid="a" id="b""#, "id").as_deref(), Some("b"));
        assert_eq!(extract_attr(r#"<Metadata uuid="a""#, "id"), None);
    }

    #[test]
    fn for_config_root_rescans_when_files_change() {
        let dir = tempfile::tempdir().unwrap();
        let cat = dir.path().join("Catalogs");
        fs::create_dir_all(&cat).unwrap();
        let xml = |guid: &str| {
            format!(r#"<Catalog><InternalInfo><xr:GeneratedType name="CatalogRef.А" category="Ref"><xr:TypeId>{guid}</xr:TypeId></xr:GeneratedType></InternalInfo></Catalog>"#)
        };
        let g1 = "33333333-3333-3333-3333-333333333333";
        let g2 = "44444444-4444-4444-4444-444444444444";
        fs::write(cat.join("А.xml"), xml(g1)).unwrap();
        let first = ConfigTypeLookup::for_config_root(dir.path());
        assert_eq!(first.resolve_name("Справочник.А"), Some(g1));
        assert!(Arc::ptr_eq(&first, &ConfigTypeLookup::for_config_root(dir.path())));
        fs::write(cat.join("Б.xml"), xml(g2).replace("CatalogRef.А", "CatalogRef.Б")).unwrap();
        let second = ConfigTypeLookup::for_config_root(dir.path());
        assert_eq!(second.resolve_name("Справочник.Б"), Some(g2));
    }
}
