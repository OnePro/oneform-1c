//! Metadata reference and type resolution for 1C ordinary forms.
//!
//! Maps between metadata reference TypeIds (GUIDs in binary `FormStream.bin`)
//! and human-readable names (e.g. `Справочник.Контрагенты` / `CatalogRef.Контрагенты`),
//! based on 1C configuration metadata XML files (`Catalogs/*.xml`, `Documents/*.xml`, etc.).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
            ("acf6192e-81ca-46ef-93a6-5a6968b78663", "ValueTable", &["ТаблицаЗначений", "ValueTable"][..]),
            ("e603c0f2-92fb-4d47-8f38-a44a381cf235", "ValueTree", &["ДеревоЗначений", "ValueTree"][..]),
            ("4772b3b4-f4a3-49c0-a1a5-8cb5961511a3", "ValueList", &["СписокЗначений", "ValueList"][..]),
            ("e61ef7b8-f3e1-4f4b-8ac7-676e90524997", "AnyRef", &["ЛюбаяСсылка", "AnyRef"][..]),
            ("47e6dd42-cd5c-4e3a-9bc4-14816ed53d38", "Справочник.Контрагенты", &["Справочник.Контрагенты", "СправочникСсылка.Контрагенты", "CatalogRef.Контрагенты", "Catalog.Контрагенты"][..]),
            ("44a01d39-c2b6-4751-85e6-06fdc2f23043", "Справочник.Номенклатура", &["Справочник.Номенклатура", "СправочникСсылка.Номенклатура", "CatalogRef.Номенклатура", "Catalog.Номенклатура"][..]),
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

    /// Scans a configuration root directory (all `*/*.xml` metadata files and `ConfigDumpInfo.xml`) and builds the lookup.
    pub fn from_config_dir(config_root: &Path) -> Self {
        let mut lookup = Self::new();
        let dump_info_path = config_root.join("ConfigDumpInfo.xml");
        if dump_info_path.is_file() {
            if let Ok(content) = fs::read_to_string(&dump_info_path) {
                lookup.parse_dump_info(&content);
            }
        }
        let Ok(entries) = fs::read_dir(config_root) else { return lookup };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                lookup.scan_metadata_subdir(&path);
            }
        }
        lookup
    }

    fn parse_dump_info(&mut self, xml: &str) {
        let mut pos = 0;
        while let Some(tag_start) = xml[pos..].find("<Metadata ") {
            let start = pos + tag_start;
            let tag_end = match xml[start..].find('>') {
                Some(i) => start + i,
                None => break,
            };
            let tag_str = &xml[start..tag_end];
            if let (Some(name), Some(id)) = (extract_attr(tag_str, "name"), extract_attr(tag_str, "id")) {
                let id_clean = id.trim().to_lowercase();
                if id_clean.len() == 36 && id_clean.contains('-') {
                    self.register_type(&id_clean, &name, None);
                }
            }
            pos = tag_end;
        }
    }

    fn scan_metadata_subdir(&mut self, subdir: &Path) {
        let Ok(files) = fs::read_dir(subdir) else { return };
        for f in files.flatten() {
            let path = f.path();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("xml")) {
                if let Ok(content) = fs::read_to_string(&path) {
                    self.parse_generated_types(&content);
                }
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

        // Register multiple aliases for flexible lookup
        self.name_to_guid.insert(name.to_lowercase(), guid.to_string());
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

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pat = format!("{attr}=\"");
    let at = tag.find(&pat)? + pat.len();
    let end = at + tag[at..].find('"')?;
    Some(tag[at..end].to_string())
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
}
