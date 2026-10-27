//! Scoped reverse table of an object: attribute names -> uuid, standard attribute names -> code,
//! tabular sections and their columns.
//!
//! A flat `uuid -> name` lookup cannot be reversed (equal column names occur in different tabular
//! sections), so the schema is nested. The pack planner reads it through [`object_schema`]; it is
//! set per form with [`with_object_schema`] and is `None` when the object has no known schema.
//! Names are matched exactly (case-sensitive).

use std::cell::RefCell;
use std::sync::Arc;

use crate::brace::{Item, Node, BARE, QUOTED};
use crate::metadata::descriptor_of;
use crate::metaref::ConfigTypeLookup;
use crate::naming::is_uuid;
use crate::oof_props::{std_attr_code, std_attr_names};

/// Class of the group of attributes in the root metadata of an external processor.
const ATTRIBUTES_CLASS_ID: &str = "ec6bb5e5-b7a8-4d75-bec9-658107a699cf";
/// Class of the group of tabular sections.
const TABULAR_CLASS_ID: &str = "2bcef0d1-0981-11d6-b9b8-0050bae0a95d";
/// Class of the group of columns inside a tabular section.
const COLUMNS_CLASS_ID: &str = "5d24a9d1-098e-11d6-b9b8-0050bae0a95d";

/// Result of a lookup by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved<T> {
    One(T),
    /// The name occurs more than once in the scope.
    Ambiguous,
    Missing,
}

/// A tabular section with its columns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabularSchema {
    pub name: String,
    pub uuid: String,
    /// TypeId of the generated type `TabularSection` (the value type of a table bound to this section).
    pub type_id: Option<String>,
    /// `(name, uuid)` in declaration order.
    pub columns: Vec<(String, String)>,
    /// Value type of each column as the text of a stream pattern (`{"Pattern",{"S",10,1}}`), parallel to
    /// `columns`; `None` when it cannot be derived.
    pub column_patterns: Vec<Option<String>>,
    /// For a column whose value type is a single reference of the configuration: the kind of the reference as the
    /// object XML names it (`CatalogRef`, `EnumRef`, ...), parallel to `columns`; `None` for every other column and
    /// when the schema does not come from an object XML (the stream of a processor does not name the kind).
    pub column_ref_kinds: Vec<Option<String>>,
}

impl TabularSchema {
    pub fn resolve_column(&self, name: &str) -> Resolved<&str> {
        resolve(&self.columns, name)
    }

    /// 0-based position of the column `name` in the declaration order (`None`: missing or ambiguous).
    pub fn column_ordinal(&self, name: &str) -> Option<usize> {
        let mut found = self.columns.iter().enumerate().filter(|(_, (n, _))| n == name);
        match (found.next(), found.next()) {
            (Some((i, _)), None) => Some(i),
            _ => None,
        }
    }

    /// Stream pattern text of the value type of the column at `ordinal`.
    pub fn column_pattern(&self, ordinal: usize) -> Option<&str> {
        self.column_patterns.get(ordinal)?.as_deref()
    }

    /// Kind of the reference (`CatalogRef`, ...) that the column at `ordinal` has as its value type.
    pub fn column_ref_kind(&self, ordinal: usize) -> Option<&str> {
        self.column_ref_kinds.get(ordinal)?.as_deref()
    }
}

/// Names of the object's attributes (`attrs`), tabular sections and standard attributes of its `kind`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectSchema {
    /// XML element under `MetaDataObject` (`Catalog`, `Document`, ...); `None` for a processor container.
    pub kind: Option<String>,
    /// `(name, uuid)` in declaration order (registers: dimensions and resources included).
    pub attrs: Vec<(String, String)>,
    /// Kind of the value type of each attribute, parallel to `attrs`: `CatalogRef`, `DocumentRef`, `EnumRef`,
    /// `ChartOfAccountsRef`, any other `...Ref`, `String`, `Number`, `Date`, `Time`, `Boolean`, `Composite`; `None` when it
    /// cannot be derived (the type is missing or not understood).
    pub attr_kinds: Vec<Option<String>>,
    /// Stream pattern of each attribute, parallel to `attrs`: read from the metadata tree of a processor container;
    /// built from the `Type` of an object XML (a single primitive type, or a single reference type that the lookup
    /// knows; `None` otherwise). [`ObjectSchema::resolve_ref_kinds`] uses it to find the kind of a reference.
    pub(crate) attr_patterns: Vec<Option<String>>,
    /// `(name, stream pattern)` of the standard attributes whose type the object XML states: the `Код` /
    /// `Наименование` of a catalog, the `Номер` / `Дата` of a document.
    pub(crate) std_patterns: Vec<(String, String)>,
    pub tabular: Vec<TabularSchema>,
    /// Standard attributes of `kind` that this object does not have (a catalog without code, without
    /// description, not hierarchical, without owners); a link to them would be invalid.
    pub std_absent: Vec<String>,
}

fn resolve<'a>(list: &'a [(String, String)], name: &str) -> Resolved<&'a str> {
    let mut found = list.iter().filter(|(n, _)| n == name);
    match (found.next(), found.next()) {
        (None, _) => Resolved::Missing,
        (Some((_, uuid)), None) => Resolved::One(uuid.as_str()),
        (Some(_), Some(_)) => Resolved::Ambiguous,
    }
}

impl ObjectSchema {
    /// Fill the kind of the reference (`CatalogRef`, ...) of the columns whose value type is a single reference that
    /// the schema does not name (the container of a processor); `lookup` is built only when there is such a column.
    pub fn resolve_ref_kinds(&mut self, lookup: impl FnOnce() -> Option<Arc<ConfigTypeLookup>>) {
        let wanted = |t: &TabularSchema, i: usize| t.column_ref_kinds[i].is_none() && t.column_pattern(i).and_then(crate::ts_table::single_ref_guid).is_some();
        let attr_ref = |p: &Option<String>| p.as_deref().and_then(crate::ts_table::single_ref_guid);
        let attrs_wanted = self.attr_patterns.iter().zip(&self.attr_kinds).any(|(p, k)| k.as_deref() == Some("Ref") && attr_ref(p).is_some());
        if !attrs_wanted && !self.tabular.iter().any(|t| t.column_ref_kinds.len() == t.columns.len() && (0..t.columns.len()).any(|i| wanted(t, i))) {
            return;
        }
        let Some(lookup) = lookup() else { return };
        for (pattern, kind) in self.attr_patterns.iter().zip(&mut self.attr_kinds) {
            if kind.as_deref() != Some("Ref") {
                continue;
            }
            *kind = attr_ref(pattern)
                .and_then(|guid| lookup.resolve_guid(&guid))
                .and_then(crate::ts_table::ref_kind_of_canonical)
                .map(str::to_string);
        }
        for t in &mut self.tabular {
            if t.column_ref_kinds.len() != t.columns.len() {
                continue;
            }
            for i in 0..t.columns.len() {
                if !wanted(t, i) {
                    continue;
                }
                let guid = t.column_pattern(i).and_then(crate::ts_table::single_ref_guid).unwrap_or_default();
                t.column_ref_kinds[i] = lookup.resolve_guid(&guid).and_then(crate::ts_table::ref_kind_of_canonical).map(str::to_string);
            }
        }
    }

    pub fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }

    /// uuid of the attribute `name` of the object itself (not of its tabular sections).
    pub fn resolve_attr(&self, name: &str) -> Resolved<&str> {
        resolve(&self.attrs, name)
    }

    /// Kind of the value type of the attribute `name` of the object itself (see [`ObjectSchema::attr_kinds`]); `None`
    /// when the name is missing or ambiguous or the kind is not known.
    pub fn attr_kind(&self, name: &str) -> Option<&str> {
        let mut found = self.attrs.iter().enumerate().filter(|(_, (n, _))| n == name);
        match (found.next(), found.next()) {
            (Some((i, _)), None) => self.attr_kinds.get(i)?.as_deref(),
            _ => None,
        }
    }

    /// Stream pattern of the value type of the attribute `name` of the object itself (`{"Pattern",...}`); `None` when
    /// the name is missing or ambiguous or the type is not a single one that is known.
    pub fn attr_pattern(&self, name: &str) -> Option<&str> {
        let mut found = self.attrs.iter().enumerate().filter(|(_, (n, _))| n == name);
        match (found.next(), found.next()) {
            (Some((i, _)), None) => self.attr_patterns.get(i)?.as_deref(),
            _ => None,
        }
    }

    /// Stream pattern of the value type of the standard attribute `name`, when the object XML states it.
    pub fn std_pattern(&self, name: &str) -> Option<&str> {
        self.std_patterns.iter().find(|(n, _)| n == name).map(|(_, p)| p.as_str())
    }

    /// The tabular section `name`.
    pub fn resolve_tabular(&self, name: &str) -> Resolved<&TabularSchema> {
        let mut found = self.tabular.iter().filter(|t| t.name == name);
        match (found.next(), found.next()) {
            (None, _) => Resolved::Missing,
            (Some(t), None) => Resolved::One(t),
            (Some(_), Some(_)) => Resolved::Ambiguous,
        }
    }

    /// Code `-N` of the standard attribute `name` of this object's kind.
    pub fn std_code(&self, name: &str) -> Option<i64> {
        if self.std_absent.iter().any(|n| n == name) {
            return None;
        }
        std_attr_code(self.kind(), name)
    }

    /// Every name that can follow the root in a `DataPath` (attributes, tabular sections, standard
    /// attributes); a hint for error messages.
    pub fn root_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.attrs.iter().map(|(n, _)| n.clone()).collect();
        names.extend(self.tabular.iter().map(|t| t.name.clone()));
        names.extend(std_attr_names(self.kind()).into_iter().filter(|n| !self.std_absent.iter().any(|a| a == n)).map(str::to_string));
        names
    }

    /// From the XML of a configuration object (`<Obj>.xml`): `ChildObjects/{Attribute,Dimension,Resource}`
    /// and `ChildObjects/TabularSection` with its own `ChildObjects/Attribute`. `None` when the text
    /// is not a `MetaDataObject` document.
    pub fn from_object_xml(text: &str) -> Option<ObjectSchema> {
        Self::from_object_xml_with(text, None)
    }

    /// [`ObjectSchema::from_object_xml`] that resolves the reference types of the columns of tabular sections
    /// (`cfg:CatalogRef.Name`) through `lookup`; without it only the primitive types get a pattern.
    pub fn from_object_xml_with(text: &str, lookup: Option<&ConfigTypeLookup>) -> Option<ObjectSchema> {
        let doc = roxmltree::Document::parse_with_options(
            text.strip_prefix('\u{feff}').unwrap_or(text),
            roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
        )
        .ok()?;
        let root = doc.root_element();
        if root.tag_name().name() != "MetaDataObject" {
            return None;
        }
        let obj = root.children().find(|c| c.is_element())?;
        let mut schema = ObjectSchema { kind: Some(obj.tag_name().name().to_string()), ..Default::default() };
        if obj.tag_name().name() == "Catalog" {
            let prop = |name: &str| {
                xchild(obj, "Properties").and_then(|p| xchild(p, name)).and_then(|e| e.text()).map(str::trim).map(str::to_string)
            };
            let zero = |name: &str| prop(name).is_some_and(|v| v == "0");
            if zero("CodeLength") {
                schema.std_absent.push("Код".into());
            }
            if zero("DescriptionLength") {
                schema.std_absent.push("Наименование".into());
            }
            if prop("Hierarchical").is_some_and(|v| v != "true") {
                schema.std_absent.push("Родитель".into());
            }
            let (len, variable) = (prop("CodeLength").and_then(|v| v.parse::<i64>().ok()), prop("CodeAllowedLength").is_none_or(|v| v != "Fixed"));
            if let (Some(len @ 1..), true) = (len, prop("CodeType").is_some_and(|v| v == "String")) {
                schema.std_patterns.push(("Код".into(), string_pattern(len, variable)));
            }
            if let Some(len @ 1..) = prop("DescriptionLength").and_then(|v| v.parse::<i64>().ok()) {
                schema.std_patterns.push(("Наименование".into(), string_pattern(len, true)));
            }
            let owners = xchild(obj, "Properties").and_then(|p| xchild(p, "Owners"));
            if owners.is_some_and(|o| !o.children().any(|c| c.is_element())) {
                schema.std_absent.push("Владелец".into());
            }
        }
        if obj.tag_name().name() == "Document" {
            let prop = |name: &str| xchild(obj, "Properties").and_then(|p| xchild(p, name)).and_then(|e| e.text()).map(str::trim);
            if let (Some(len @ 1..), Some("String")) = (prop("NumberLength").and_then(|v| v.parse::<i64>().ok()), prop("NumberType")) {
                schema.std_patterns.push(("Номер".into(), string_pattern(len, prop("NumberAllowedLength") != Some("Fixed"))));
            }
            schema.std_patterns.push(("Дата".into(), r#"{"Pattern",{"D"}}"#.into()));
        }
        let Some(children) = xchild(obj, "ChildObjects") else { return Some(schema) };
        for e in children.children().filter(|c| c.is_element()) {
            let Some((name, uuid)) = xml_entry(e) else { continue };
            match e.tag_name().name() {
                "Attribute" | "Dimension" | "Resource" => {
                    schema.attrs.push((name, uuid));
                    let ty = xchild(e, "Properties").and_then(|p| xchild(p, "Type"));
                    schema.attr_kinds.push(ty.and_then(xml_type_kind));
                    schema.attr_patterns.push(ty.and_then(|t| xml_type_pattern(t, lookup)).map(|(pattern, _)| pattern));
                }
                "TabularSection" => {
                    let mut columns = Vec::new();
                    let mut column_patterns = Vec::new();
                    let mut column_ref_kinds = Vec::new();
                    let attrs = xchild(e, "ChildObjects")
                        .into_iter()
                        .flat_map(|c| c.children().filter(|c| c.is_element() && c.tag_name().name() == "Attribute"));
                    for attr in attrs {
                        let Some(entry) = xml_entry(attr) else { continue };
                        columns.push(entry);
                        let (pattern, ref_kind) =
                            xchild(attr, "Properties").and_then(|p| xchild(p, "Type")).and_then(|t| xml_type_pattern(t, lookup)).unzip();
                        column_patterns.push(pattern);
                        column_ref_kinds.push(ref_kind.flatten());
                    }
                    let type_id = xchild(e, "InternalInfo").and_then(tabular_type_id);
                    schema.tabular.push(TabularSchema { name, uuid, type_id, columns, column_patterns, column_ref_kinds });
                }
                _ => {}
            }
        }
        Some(schema)
    }

    /// From the root metadata tree of an external processor container (kind `None`).
    ///
    /// The properties node of the root class holds groups `{<class>,<count>,<item>...}`; an item is
    /// `{<head>,...}` whose head contains the descriptor `{3,{1,0,<uuid>},"<Name>",...}` of the
    /// attribute / tabular section / column. Attributes are the group of `ATTRIBUTES_CLASS_ID`; a
    /// tabular section item carries its own group of `COLUMNS_CLASS_ID`. A tree of an unexpected
    /// shape yields an empty schema.
    pub fn from_meta_tree(meta: &Node) -> ObjectSchema {
        let mut schema = ObjectSchema::default();
        let Some(props) = meta.node(3).and_then(|cls| cls.node(1)) else { return schema };
        for group in props.children.iter().skip(2).filter_map(as_node) {
            match group_class(group) {
                Some(ATTRIBUTES_CLASS_ID) => {
                    for item in group_items(group) {
                        let Some(entry) = item_entry(item) else { continue };
                        let pattern = item.node(0).and_then(item_pattern);
                        schema.attrs.push(entry);
                        schema.attr_kinds.push(pattern.as_deref().and_then(pattern_kind));
                        schema.attr_patterns.push(pattern);
                    }
                }
                Some(TABULAR_CLASS_ID) => {
                    for item in group_items(group) {
                        let Some((name, uuid)) = item_entry(item) else { continue };
                        let mut columns = Vec::new();
                        let mut column_patterns = Vec::new();
                        let mut column_ref_kinds = Vec::new();
                        let col_items = item
                            .children
                            .iter()
                            .filter_map(as_node)
                            .filter(|n| group_class(n) == Some(COLUMNS_CLASS_ID))
                            .flat_map(group_items);
                        for col in col_items {
                            let Some(entry) = item_entry(col) else { continue };
                            columns.push(entry);
                            column_patterns.push(col.node(0).and_then(item_pattern));
                            column_ref_kinds.push(None);
                        }
                        // head `{0,{11,<TabularSection TypeId>,<ValueId>,<row TypeId>,<row ValueId>,...}}`
                        let type_id = item
                            .node(0)
                            .and_then(|h| h.node(1))
                            .filter(|n| matches!(n.children.first(), Some(Item::Atom(a)) if a.value == "11"))
                            .and_then(|n| match n.children.get(1) {
                                Some(Item::Atom(a)) if is_uuid(&a.value) => Some(a.value.to_ascii_lowercase()),
                                _ => None,
                            });
                        schema.tabular.push(TabularSchema { name, uuid, type_id, columns, column_patterns, column_ref_kinds });
                    }
                }
                _ => {}
            }
        }
        schema
    }
}

fn as_node(item: &Item) -> Option<&Node> {
    match item {
        Item::Node(n) => Some(n),
        Item::Atom(_) => None,
    }
}

/// Class uuid of a group `{<class>,<count>,...}`.
fn group_class(group: &Node) -> Option<&str> {
    match group.children.first() {
        Some(Item::Atom(a)) if a.kind == BARE && is_uuid(&a.value) => Some(a.value.as_str()),
        _ => None,
    }
}

fn group_items(group: &Node) -> impl Iterator<Item = &Node> {
    group.children.iter().skip(2).filter_map(as_node)
}

/// `(name, uuid)` of the first descriptor in the head (`item[0]`) of a group item.
fn item_entry(item: &Node) -> Option<(String, String)> {
    let (uuid, name) = first_descriptor(item.node(0)?)?;
    Some((name.to_string(), uuid.to_string()))
}

/// Text of the first `{"Pattern",...}` node below `head` (the value type of an attribute or a column).
fn item_pattern(head: &Node) -> Option<String> {
    fn find(n: &Node) -> Option<&Node> {
        if matches!(n.children.first(), Some(Item::Atom(a)) if a.kind == QUOTED && a.value == "Pattern") {
            return Some(n);
        }
        n.children.iter().filter_map(as_node).find_map(find)
    }
    let mut out = Vec::new();
    crate::donor::write_node(find(head)?, &mut out);
    String::from_utf8(out).ok()
}

fn first_descriptor(node: &Node) -> Option<(&str, &str)> {
    descriptor_of(node).or_else(|| node.children.iter().filter_map(as_node).find_map(first_descriptor))
}

/// `xr:TypeId` of the `xr:GeneratedType` with `category="TabularSection"` in `InternalInfo`.
fn tabular_type_id(info: roxmltree::Node<'_, '_>) -> Option<String> {
    let generated = info
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "GeneratedType" && c.attribute("category") == Some("TabularSection"))?;
    let id = xchild(generated, "TypeId")?.text()?.trim().to_lowercase();
    is_uuid(&id).then_some(id)
}

/// Kind of the single type of the `Type` element of an attribute (see [`ObjectSchema::attr_kinds`]); several
/// types: `Composite`.
fn xml_type_kind(ty: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut types = ty.children().filter(|c| c.is_element() && c.tag_name().name() == "Type");
    let (one, rest) = (types.next()?, types.next());
    if rest.is_some() {
        return Some("Composite".to_string());
    }
    let name = one.text()?.trim();
    Some(match name {
        "xs:boolean" => "Boolean".to_string(),
        "xs:string" => "String".to_string(),
        "xs:decimal" => "Number".to_string(),
        "xs:dateTime" => match xchild(ty, "DateQualifiers").and_then(|q| xchild(q, "DateFractions")).and_then(|f| f.text()).map(str::trim) {
            Some("Time") => "Time".to_string(),
            _ => "Date".to_string(),
        },
        other => {
            let (kind, _) = other.strip_prefix("cfg:")?.split_once('.')?;
            kind.ends_with("Ref").then(|| kind.to_string())?
        }
    })
}

/// [`xml_type_kind`] for a stream pattern: a reference is `Ref` until [`ObjectSchema::resolve_ref_kinds`] names it.
fn pattern_kind(pattern: &str) -> Option<String> {
    let tree = crate::brace::parse_metadata(pattern.as_bytes()).ok()?;
    let branches: Vec<&Node> = tree.children.iter().skip(1).filter_map(as_node).collect();
    let [b] = branches.as_slice() else { return (branches.len() > 1).then(|| "Composite".to_string()) };
    let atom = |i: usize| b.atom(i).map(|a| a.value.as_str());
    Some(match (atom(0)?, atom(1)) {
        ("S", _) => "String",
        ("N", _) => "Number",
        ("B", _) => "Boolean",
        ("D", Some("T")) => "Time",
        ("D", _) => "Date",
        ("#", _) => "Ref",
        _ => return None,
    }
    .to_string())
}

/// Pattern of a string of `len` characters (`{"S",len,1}` is a variable length, `{"S",len,0}` a fixed one).
fn string_pattern(len: i64, variable: bool) -> String {
    format!(r#"{{"Pattern",{{"S",{len},{}}}}}"#, i32::from(variable))
}

/// Stream pattern text of the `Type` element of an attribute (`v8:Type` with its qualifiers): a single
/// primitive type or a single reference type resolved by `lookup`; `None` for everything else (composite
/// types, types the lookup does not know). The second item is the kind of the reference (`CatalogRef`, ...) of a
/// reference type.
fn xml_type_pattern(ty: roxmltree::Node<'_, '_>, lookup: Option<&ConfigTypeLookup>) -> Option<(String, Option<String>)> {
    let mut types = ty.children().filter(|c| c.is_element() && c.tag_name().name() == "Type");
    let (one, rest) = (types.next()?, types.next());
    if rest.is_some() {
        return None;
    }
    let name = one.text()?.trim();
    let text = |q: &str, tag: &str| -> Option<&str> { xchild(xchild(ty, q)?, tag)?.text().map(str::trim) };
    let int = |q: &str, tag: &str| -> Option<i64> { text(q, tag)?.parse().ok() };
    let mut ref_kind = None;
    let branch = match name {
        "xs:boolean" => r#"{"B"}"#.to_string(),
        "xs:string" => {
            let len = int("StringQualifiers", "Length").unwrap_or(0);
            match (len, text("StringQualifiers", "AllowedLength")) {
                (0, _) => r#"{"S"}"#.to_string(),
                (l, Some("Fixed")) => format!(r#"{{"S",{l},0}}"#),
                (l, _) => format!(r#"{{"S",{l},1}}"#),
            }
        }
        "xs:decimal" => {
            let digits = int("NumberQualifiers", "Digits").unwrap_or(10);
            let fraction = int("NumberQualifiers", "FractionDigits").unwrap_or(0);
            let non_negative = text("NumberQualifiers", "AllowedSign") == Some("Nonnegative");
            format!(r#"{{"N",{digits},{fraction},{}}}"#, i32::from(non_negative))
        }
        "xs:dateTime" => match text("DateQualifiers", "DateFractions") {
            Some("Date") => r#"{"D","D"}"#.to_string(),
            Some("Time") => r#"{"D","T"}"#.to_string(),
            _ => r#"{"D"}"#.to_string(),
        },
        other => {
            let reference = other.strip_prefix("cfg:")?;
            ref_kind = reference.split_once('.').map(|(kind, _)| kind.to_string());
            format!(r##"{{"#",{}}}"##, lookup?.resolve_name(reference)?)
        }
    };
    Some((format!(r#"{{"Pattern",{branch}}}"#), ref_kind))
}

fn xchild<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

/// `(Properties/Name, uuid attribute)` of a metadata element; `None` without a name or a valid uuid.
fn xml_entry(e: roxmltree::Node<'_, '_>) -> Option<(String, String)> {
    let name = xchild(xchild(e, "Properties")?, "Name")?.text()?.trim();
    let uuid = e.attribute("uuid")?.trim().to_lowercase();
    (!name.is_empty() && is_uuid(&uuid)).then(|| (name.to_string(), uuid))
}

// ---------------------------------------------------------------------------- scope

thread_local! {
    // The initializer already is `const`; clippy still reports this static.
    #[allow(clippy::missing_const_for_thread_local)]
    static ACTIVE_SCHEMA: RefCell<Vec<Option<Arc<ObjectSchema>>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` with `schema` as the active [`object_schema`] (`None`: no schema known for this form).
pub fn with_object_schema<R>(schema: Option<ObjectSchema>, f: impl FnOnce() -> R) -> R {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            ACTIVE_SCHEMA.with(|s| s.borrow_mut().pop());
        }
    }
    ACTIVE_SCHEMA.with(|s| s.borrow_mut().push(schema.map(Arc::new)));
    let _pop = Pop;
    f()
}

/// The schema of the form being planned; `None` outside [`with_object_schema`] or when none is known.
pub fn object_schema() -> Option<Arc<ObjectSchema>> {
    ACTIVE_SCHEMA.with(|s| s.borrow().last().cloned().flatten())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brace::parse_metadata;

    const CATALOG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.17">
  <Catalog uuid="aaaaaaaa-0000-0000-0000-000000000001">
    <Properties><Name>Контрагенты</Name></Properties>
    <ChildObjects>
      <Attribute uuid="11111111-0000-0000-0000-000000000001"><Properties><Name>ИНН</Name></Properties></Attribute>
      <Attribute uuid="11111111-0000-0000-0000-000000000002"><Properties><Name>КПП</Name></Properties></Attribute>
      <Attribute uuid="11111111-0000-0000-0000-000000000003"><Properties><Name>КПП</Name></Properties></Attribute>
      <TabularSection uuid="22222222-0000-0000-0000-000000000001">
        <Properties><Name>Контакты</Name></Properties>
        <ChildObjects>
          <Attribute uuid="33333333-0000-0000-0000-000000000001"><Properties><Name>Значение</Name></Properties></Attribute>
        </ChildObjects>
      </TabularSection>
      <TabularSection uuid="22222222-0000-0000-0000-000000000002">
        <Properties><Name>Счета</Name></Properties>
        <ChildObjects>
          <Attribute uuid="33333333-0000-0000-0000-000000000002"><Properties><Name>Значение</Name></Properties></Attribute>
        </ChildObjects>
      </TabularSection>
      <Form>ФормаЭлемента</Form>
    </ChildObjects>
  </Catalog>
</MetaDataObject>"#;

    #[test]
    fn standard_attributes_a_catalog_does_not_have_are_not_offered() {
        let xml = r#"<MetaDataObject><Catalog uuid="aaaaaaaa-0000-0000-0000-000000000001"><Properties>
            <Name>Синхр</Name><Hierarchical>false</Hierarchical><Owners/><CodeLength>7</CodeLength><DescriptionLength>0</DescriptionLength>
            </Properties><ChildObjects/></Catalog></MetaDataObject>"#;
        let s = ObjectSchema::from_object_xml(xml).unwrap();
        assert_eq!(s.std_code("Код"), Some(-2));
        assert_eq!(s.std_code("Наименование"), None);
        assert_eq!(s.std_code("Родитель"), None);
        assert_eq!(s.std_code("Владелец"), None);
        assert!(s.root_names().contains(&"Код".to_string()));
        assert!(!s.root_names().contains(&"Наименование".to_string()));
        let owned = xml.replace("<Owners/>", "<Owners><xr:Item>Catalog.X</xr:Item></Owners>").replace("<Hierarchical>false", "<Hierarchical>true");
        let s = ObjectSchema::from_object_xml(&owned.replace("<MetaDataObject>", "<MetaDataObject xmlns:xr=\"urn:x\">")).unwrap();
        assert_eq!(s.std_code("Родитель"), Some(-4));
        assert_eq!(s.std_code("Владелец"), Some(-5));
    }

    #[test]
    fn reference_columns_get_their_pattern_and_kind_from_the_lookup() {
        let dir = tempfile::tempdir().unwrap();
        for (sub, file, name, category, type_id) in [
            ("Catalogs", "Номенклатура.xml", "CatalogRef.Номенклатура", "Ref", "47e6dd42-cd5c-4e3a-9bc4-14816ed53d38"),
            ("Catalogs", "Номенклатура2.xml", "CatalogObject.Номенклатура", "Object", "f04b5444-61be-425a-b941-9a6144713f21"),
            ("Enums", "Виды.xml", "EnumRef.Виды", "Ref", "52c49a35-3374-4995-9803-5fc37bb7d7cf"),
        ] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            let xml = format!(
                r#"<Obj xmlns:xr="urn:x"><InternalInfo><xr:GeneratedType name="{name}" category="{category}"><xr:TypeId>{type_id}</xr:TypeId></xr:GeneratedType></InternalInfo></Obj>"#
            );
            std::fs::write(dir.path().join(sub).join(file), xml).unwrap();
        }
        let lookup = ConfigTypeLookup::from_config_dir(dir.path());
        let column = |n: u8, ty: &str| {
            format!(
                r#"<Attribute uuid="33333333-0000-0000-0000-00000000000{n}"><Properties><Name>К{n}</Name><Type><v8:Type>{ty}</v8:Type></Type></Properties></Attribute>"#
            )
        };
        let columns = [
            column(1, "cfg:CatalogRef.Номенклатура"),
            column(2, "cfg:EnumRef.Виды"),
            column(3, "cfg:CatalogRef.Нет"),
            column(4, "xs:boolean"),
            column(5, "cfg:CatalogObject.Номенклатура"),
        ]
        .concat();
        let xml = format!(
            r#"<MetaDataObject xmlns:v8="urn:v8"><Document uuid="aaaaaaaa-0000-0000-0000-000000000001"><ChildObjects><TabularSection uuid="22222222-0000-0000-0000-000000000001"><Properties><Name>Т</Name></Properties><ChildObjects>{columns}</ChildObjects></TabularSection></ChildObjects></Document></MetaDataObject>"#
        );
        let s = ObjectSchema::from_object_xml_with(&xml, Some(&lookup)).unwrap();
        let t = &s.tabular[0];
        let got: Vec<(Option<&str>, Option<&str>)> = (0..5).map(|i| (t.column_pattern(i), t.column_ref_kind(i))).collect();
        assert_eq!(
            got,
            [
                (Some(r##"{"Pattern",{"#",47e6dd42-cd5c-4e3a-9bc4-14816ed53d38}}"##), Some("CatalogRef")),
                (Some(r##"{"Pattern",{"#",52c49a35-3374-4995-9803-5fc37bb7d7cf}}"##), Some("EnumRef")),
                (None, None),
                (Some(r#"{"Pattern",{"B"}}"#), None),
                (Some(r##"{"Pattern",{"#",f04b5444-61be-425a-b941-9a6144713f21}}"##), Some("CatalogObject")),
            ]
        );
        // without a lookup a reference has neither a pattern nor a kind
        let s = ObjectSchema::from_object_xml(&xml).unwrap();
        assert_eq!((s.tabular[0].column_pattern(0), s.tabular[0].column_ref_kind(0)), (None, None));
    }

    #[test]
    fn attribute_kinds_come_from_the_type_of_the_object_xml_and_the_stream_pattern() {
        let attr = |n: u8, ty: &str| {
            format!(r#"<Attribute uuid="11111111-0000-0000-0000-00000000000{n}"><Properties><Name>А{n}</Name><Type>{ty}</Type></Properties></Attribute>"#)
        };
        let attrs = [
            attr(1, "<v8:Type>cfg:CatalogRef.Номенклатура</v8:Type>"),
            attr(2, "<v8:Type>xs:dateTime</v8:Type><v8:DateQualifiers><v8:DateFractions>Time</v8:DateFractions></v8:DateQualifiers>"),
            attr(3, "<v8:Type>xs:decimal</v8:Type>"),
            attr(4, "<v8:Type>cfg:EnumRef.Виды</v8:Type><v8:Type>xs:string</v8:Type>"),
            attr(5, "<v8:Type>cfg:CatalogObject.Номенклатура</v8:Type>"),
            r#"<Attribute uuid="11111111-0000-0000-0000-000000000006"><Properties><Name>А6</Name></Properties></Attribute>"#.to_string(),
        ]
        .concat();
        let xml = format!(
            r#"<MetaDataObject xmlns:v8="urn:v8"><Document uuid="aaaaaaaa-0000-0000-0000-000000000001"><ChildObjects>{attrs}</ChildObjects></Document></MetaDataObject>"#
        );
        let s = ObjectSchema::from_object_xml(&xml).unwrap();
        let kinds: Vec<Option<&str>> = (1..=6).map(|n| s.attr_kind(&format!("А{n}"))).collect();
        assert_eq!(kinds, [Some("CatalogRef"), Some("Time"), Some("Number"), Some("Composite"), None, None]);
        assert_eq!(s.attr_kind("Нет"), None);
        assert_eq!(pattern_kind(r#"{"Pattern",{"S",10,1}}"#).as_deref(), Some("String"));
        assert_eq!(pattern_kind(r#"{"Pattern",{"D","D"}}"#).as_deref(), Some("Date"));
        assert_eq!(pattern_kind(r#"{"Pattern",{"D","T"}}"#).as_deref(), Some("Time"));
        assert_eq!(pattern_kind(r#"{"Pattern",{"B"},{"S"}}"#).as_deref(), Some("Composite"));
        assert_eq!(pattern_kind(r##"{"Pattern",{"#",47e6dd42-cd5c-4e3a-9bc4-14816ed53d38}}"##).as_deref(), Some("Ref"));
    }

    #[test]
    fn object_xml_nested_and_ambiguous() {
        let s = ObjectSchema::from_object_xml(CATALOG).unwrap();
        assert_eq!(s.kind(), Some("Catalog"));
        assert_eq!(s.resolve_attr("ИНН"), Resolved::One("11111111-0000-0000-0000-000000000001"));
        assert_eq!(s.resolve_attr("КПП"), Resolved::Ambiguous);
        assert_eq!(s.resolve_attr("инн"), Resolved::Missing);
        assert_eq!(s.resolve_attr("Значение"), Resolved::Missing, "columns are not attributes of the object");
        let Resolved::One(a) = s.resolve_tabular("Контакты") else { panic!() };
        let Resolved::One(b) = s.resolve_tabular("Счета") else { panic!() };
        assert_eq!(a.resolve_column("Значение"), Resolved::One("33333333-0000-0000-0000-000000000001"));
        assert_eq!(b.resolve_column("Значение"), Resolved::One("33333333-0000-0000-0000-000000000002"));
        assert_eq!(b.resolve_column("Нет"), Resolved::Missing);
        assert_eq!(s.resolve_tabular("Нет"), Resolved::Missing);
    }

    #[test]
    fn object_xml_standard_codes_and_root_names() {
        let s = ObjectSchema::from_object_xml(CATALOG).unwrap();
        assert_eq!(s.std_code("Код"), Some(-2));
        assert_eq!(s.std_code("Наименование"), Some(-3));
        assert_eq!(s.std_code("ИНН"), None);
        let names = s.root_names();
        for n in ["ИНН", "Контакты", "Код", "Наименование"] {
            assert!(names.iter().any(|x| x == n), "{n}");
        }
        assert_eq!(ObjectSchema::default().std_code("Код"), None, "no kind, no standard attributes");
    }

    #[test]
    fn object_xml_register_and_non_object() {
        let xml = r#"<MetaDataObject><InformationRegister uuid="aaaaaaaa-0000-0000-0000-000000000001"><ChildObjects>
          <Dimension uuid="44444444-0000-0000-0000-000000000001"><Properties><Name>Контрагент</Name></Properties></Dimension>
          <Resource uuid="44444444-0000-0000-0000-000000000002"><Properties><Name>Сумма</Name></Properties></Resource>
        </ChildObjects></InformationRegister></MetaDataObject>"#;
        let s = ObjectSchema::from_object_xml(xml).unwrap();
        assert_eq!(s.kind(), Some("InformationRegister"));
        assert_eq!(s.resolve_attr("Контрагент"), Resolved::One("44444444-0000-0000-0000-000000000001"));
        assert_eq!(s.resolve_attr("Сумма"), Resolved::One("44444444-0000-0000-0000-000000000002"));
        assert_eq!(s.std_code("Период"), Some(-2));
        assert!(ObjectSchema::from_object_xml("<Other/>").is_none());
        assert!(ObjectSchema::from_object_xml("not xml").is_none());
    }

    fn desc(uuid: &str, name: &str) -> String {
        format!("{{3,{{1,0,{uuid}}},\"{name}\",{{0}}}}")
    }

    /// `{{0,{23,{27,{2,<descriptor>,{Pattern}}}}},0}`: an attribute or a column.
    fn attr_item(uuid: &str, name: &str) -> String {
        ["{{0,{23,{27,{2,", &desc(uuid, name), ",{Pattern}}}}},0}"].concat()
    }

    /// A tabular section with one column `Колонка`.
    fn tab_item(uuid: &str, name: &str, col_uuid: &str) -> String {
        [
            "{{0,{11,", uuid, ",", uuid, ",", uuid, ",", uuid, ",{0,", &desc(uuid, name), "}}},1,{",
            COLUMNS_CLASS_ID, ",1,", &attr_item(col_uuid, "Колонка"), "}}",
        ]
        .concat()
    }

    fn id(n: u8) -> String {
        format!("0000000{n}-0000-0000-0000-000000000000")
    }

    /// Root metadata tree of a processor: attributes `Реквизит`, `Дубль`, `Дубль`; tabular sections
    /// `А` and `Б`, both with the column `Колонка`.
    fn processor_tree() -> Node {
        let groups = [
            ["{", TABULAR_CLASS_ID, ",2,", &tab_item(&id(1), "А", &id(2)), ",", &tab_item(&id(3), "Б", &id(4)), "}"].concat(),
            "{3daea016-69b7-4ed4-9453-127911372fe6,0}".to_string(),
            [
                "{", ATTRIBUTES_CLASS_ID, ",3,", &attr_item(&id(5), "Реквизит"), ",", &attr_item(&id(6), "Дубль"), ",",
                &attr_item(&id(7), "Дубль"), "}",
            ]
            .concat(),
        ];
        let text = ["{1,{", &id(9), "},1,{c3831ec8-d8d5-4f93-8a22-f9bfae07327f,{1,{4,x},4,", &groups.join(","), "}}}"].concat();
        parse_metadata(text.as_bytes()).unwrap()
    }

    #[test]
    fn processor_tree_nested() {
        let s = ObjectSchema::from_meta_tree(&processor_tree());
        assert_eq!(s.kind(), None);
        assert_eq!(s.resolve_attr("Реквизит"), Resolved::One(id(5).as_str()));
        assert_eq!(s.resolve_attr("Дубль"), Resolved::Ambiguous);
        assert_eq!(s.resolve_attr("Колонка"), Resolved::Missing);
        let Resolved::One(a) = s.resolve_tabular("А") else { panic!("{s:?}") };
        let Resolved::One(b) = s.resolve_tabular("Б") else { panic!("{s:?}") };
        assert_eq!(a.resolve_column("Колонка"), Resolved::One(id(2).as_str()));
        assert_eq!(b.resolve_column("Колонка"), Resolved::One(id(4).as_str()));
        assert_eq!(a.uuid, id(1));
        assert_eq!(s.std_code("Код"), None);
    }

    #[test]
    fn processor_tree_of_unexpected_shape_is_empty() {
        let tree = parse_metadata(b"{1,{x},1}").unwrap();
        assert_eq!(ObjectSchema::from_meta_tree(&tree), ObjectSchema::default());
    }

    fn inventory_of(path: &std::path::Path) -> Option<crate::metadata::ProcessorInventory> {
        use crate::container::{parse_container, Limits};
        let data = std::fs::read(path).ok()?;
        let limits = Limits::default();
        let c = parse_container(&data, &limits, 0, None).unwrap();
        Some(crate::metadata::discover(&c, &limits, None, false).unwrap())
    }

    #[test]
    fn processor_container_fixture() {
        let epf = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/donor.epf");
        let inv = inventory_of(&epf).expect("fixture");
        // the donor has neither attributes nor tabular sections: an empty, kind-less schema
        assert_eq!(inv.object_schema, ObjectSchema::default());
    }

    #[test]
    fn processor_with_attributes_if_available() {
        let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else { return };
        let epf = std::path::Path::new(&home).join("Documents/1с/export_db/TestProcesser/original/РабочееМестоМенеджера.epf");
        let Some(inv) = inventory_of(&epf) else { return };
        let s = &inv.object_schema;
        let Resolved::One(uuid) = s.resolve_attr("Контрагент") else { panic!("{s:?}") };
        assert_eq!(inv.meta_lookup.get(uuid).map(String::as_str), Some("Контрагент"));
        let Resolved::One(t) = s.resolve_tabular("Корзина") else { panic!("{s:?}") };
        let Resolved::One(col) = t.resolve_column("Номенклатура") else { panic!("{t:?}") };
        assert_eq!(inv.meta_lookup.get(col).map(String::as_str), Some("Номенклатура"));
    }

    #[test]
    fn scope_is_none_safe_and_nested() {
        assert!(object_schema().is_none());
        let s = ObjectSchema::from_object_xml(CATALOG).unwrap();
        with_object_schema(Some(s), || {
            assert_eq!(object_schema().unwrap().kind(), Some("Catalog"));
            with_object_schema(None, || assert!(object_schema().is_none()));
            assert!(object_schema().is_some());
        });
        assert!(object_schema().is_none());
    }
}
