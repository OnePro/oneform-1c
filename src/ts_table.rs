//! A `Table` bound to a tabular section (TS) of the object: the stream records of its columns, built from
//! templates captured from real forms (OpenSpec change `bind-tables-to-tabular-sections`, stages S1 and S4).
//!
//! Layout of a bound table (`info = control[2]`):
//! - `info[1]` = `{"Pattern",{"#",<TS TypeId>}}`, the value type of the table;
//! - `info[3]` = [`ROW_SOURCE`], the row source of a tabular section;
//! - `info[2][1][23]` = `{N, column...}`; a column is `{c2cf1953-..., {1, X, 0, 0}}` with
//!   `X = {8, P, DATA, {-1}, {-1}}`: `DATA` is `{0,<column uuid>}` or `{-2}` (`НомерСтроки`), `P` is the props record.
//!
//! `P` (v19) has 46 slots with an editor and 45 without: 1 header, 4 width, 5 id (local to the table), 6 ordinal of
//! the data field in the declaration order of the TS (`-2` for `НомерСтроки`), 9 flags, 25 visible, 26 read-only,
//! 30 name, 32 `ceil(width / 7)` (derived from 100 -> 15, 75 -> 11, 28 -> 4, ... over the real forms), 35 value
//! pattern, 38 editor guid, 39 editor blob `{{#base64:...},0}`, last slot the width again.
//! A column of a reference type has the same `P` as a string column; only `P[35]` (the pattern of the schema) and the
//! editor blob differ, see [`editor_text`].
//! Slots that are copied from the templates unexplained: 7, 8, 9 (bit 1), 10-24, 27-29, 31, 33, 34, 36, 37, 40-44.

use std::io::Write;

use base64::Engine;

use crate::brace::{parse_metadata, Atom, AtomKind, Item, Node};
use crate::object_schema::TabularSchema;
use crate::oof::COLUMN_GUID;

/// Row source of a table bound to a tabular section (`info[3]`).
pub const ROW_SOURCE: &str = "{51d1e122-c0f3-496f-901e-806df8206ba9,{2,1,{0,1}}}";

#[cfg(test)]
const INPUT_FIELD_GUID: &str = "381ed624-9217-4e63-85db-c4c3cb87daae";
#[cfg(test)]
const NO_EDITOR_GUID: &str = "00000000-0000-0000-0000-000000000000";
/// First 18 bytes of every column editor blob (before the deflate stream).
const BLOB_HEADER: [u8; 18] = [0x02, 0x01, 0x53, 0x4b, 0x6f, 0xf4, 0x88, 0x8d, 0xc1, 0x4e, 0xa0, 0xd5, 0xeb, 0xb6, 0xbd, 0xa0, 0xa7, 0x0d];

/// Column `P` with an `InputField` editor (46 slots), from the column `Серия` of `Корзина`; placeholders are
/// patched by [`column_record`].
const EDITABLE_PROPS: &str = r#"{19,{1,0},{1,0},{1,0},100,0,0,-1,-1,12590592,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},16,16,d2314b5d-8da4-4e0f-822b-45e7500eae09,{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{7,3,0,1,100},{7,3,0,1,100},1,0,1,4,1,"",{},15,0,{1,0},{"Pattern"},0,1,381ed624-9217-4e63-85db-c4c3cb87daae,{0,0},0,0,0,0,0,100}"#;

/// Column `P` of `НомерСтроки` without an editor (45 slots), from `Корзина`. A read-only column bound to a field of the
/// section has the same layout (no editor, no editor blob, the width again in the last slot), with the flags `P[9]`
/// of the editable column.
const ROW_NUMBER_PROPS: &str = r#"{19,{1,0},{1,0},{1,0},28,0,-2,-1,-1,12590594,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},16,16,d2314b5d-8da4-4e0f-822b-45e7500eae09,{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{7,3,0,1,100},{7,3,0,1,100},1,0,1,4,1,"",{},4,0,{1,0},{"Pattern"},0,1,00000000-0000-0000-0000-000000000000,0,0,0,0,0,28}"#;

/// Flags word `P[9]` of a column bound to a field of the section (the most frequent value in real forms).
const FIELD_FLAGS: i64 = 12590592;

/// What a column is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnData {
    /// `НомерСтроки`: read-only, no editor, data `{-2}`.
    RowNumber,
    /// A column of the tabular section: its uuid, 0-based position in the TS, the value pattern text and, for a
    /// reference type, the kind of the reference (`CatalogRef`, ...) as the object XML names it.
    Field { uuid: String, ordinal: usize, pattern: String, ref_kind: Option<String> },
}

/// Everything a column record is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: String,
    pub id: i64,
    pub header: Vec<(String, String)>,
    pub width: i64,
    pub visible: bool,
    pub read_only: bool,
    pub data: ColumnData,
}

/// Value type of a column that has an editor template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Primitive {
    String { length: i64 },
    Number { digits: i64, fraction: i64, non_negative: i64 },
    DateTime,
    Date,
    Ref { kind: RefKind },
}

/// Kinds of references with an editor template (verified against real forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Catalog,
    Document,
    Enum,
    ChartOfAccounts,
    ChartOfCharacteristicTypes,
}

impl RefKind {
    /// The kind from the prefix of the type name in the object XML (`cfg:CatalogRef.Name`).
    fn from_xml(kind: &str) -> Option<RefKind> {
        Some(match kind {
            "CatalogRef" => RefKind::Catalog,
            "DocumentRef" => RefKind::Document,
            "EnumRef" => RefKind::Enum,
            "ChartOfAccountsRef" => RefKind::ChartOfAccounts,
            "ChartOfCharacteristicTypesRef" => RefKind::ChartOfCharacteristicTypes,
            _ => return None,
        })
    }

    /// `{<guid of the editor parameters>,{<parameters>}}` of the editor: the part of the editor record that depends
    /// on the kind of the reference (the default parameters of real editors, no choice parameters).
    fn parameters(self) -> &'static str {
        match self {
            RefKind::Catalog => "8585207b-9ca7-425b-9385-e8fd67c4e148,{5,0,2,00000000-0000-0000-0000-000000000000,{1,0}}",
            RefKind::Document => "11fe1b25-97c5-4d25-ab3f-8c9a5655122e,{3,00000000-0000-0000-0000-000000000000}",
            RefKind::Enum => "30ee7b7c-8dac-4e20-b58e-e7f7742ae8d8,{1,1,00000000-0000-0000-0000-000000000000}",
            RefKind::ChartOfAccounts => "16ab4ef5-818b-4f8a-a958-51ed44e07bda,{1,0,00000000-0000-0000-0000-000000000000,{1,0}}",
            RefKind::ChartOfCharacteristicTypes => "5f656c99-ee3b-4bb7-92a1-e6f5a93c2c9d,{0,0,2,00000000-0000-0000-0000-000000000000,{1,0}}",
        }
    }
}

/// Editor parameters of the primitive types: `{"U"}` strings (`9a7643d2`), the same for numbers and dates.
const PRIMITIVE_PARAMETERS: &str = r#"9a7643d2-19e9-45e2-8893-280bc9195a97,{4,{"U"},{"U"},0,"",0,0}"#;

/// Why a value pattern has no editor template (the text is shown to the user).
fn classify(pattern: &str, ref_kind: Option<&str>) -> Result<Primitive, String> {
    let tree = parse_metadata(pattern.as_bytes()).map_err(|e| format!("value type '{pattern}' is not a pattern: {e}"))?;
    let branches: Vec<&Node> = tree.children.iter().skip(1).filter_map(Item::as_node).collect();
    let [branch] = branches.as_slice() else {
        return Err(format!("value type {pattern} is composite; editors of composite columns are not supported yet"));
    };
    let atom = |i: usize| branch.atom(i).map(|a| a.value.as_str());
    let int = |i: usize| atom(i).and_then(|v| v.parse::<i64>().ok());
    match (atom(0), branch.len()) {
        (Some("S"), 3) if int(1).is_some_and(|l| l > 0) && int(2) == Some(1) => Ok(Primitive::String { length: int(1).unwrap_or(0) }),
        (Some("S"), 1) => Err("value type String of unlimited length is not supported for a column yet".to_string()),
        (Some("S"), _) => Err(format!("value type {pattern} (fixed length string) is not supported for a column yet")),
        (Some("N"), 4) => match (int(1), int(2), int(3)) {
            (Some(digits), Some(fraction), Some(non_negative)) if digits > 0 && fraction >= 0 && matches!(non_negative, 0 | 1) => {
                Ok(Primitive::Number { digits, fraction, non_negative })
            }
            _ => Err(format!("value type {pattern} is not understood")),
        },
        (Some("D"), 1) => Ok(Primitive::DateTime),
        (Some("D"), 2) if atom(1) == Some("D") => Ok(Primitive::Date),
        (Some("D"), 2) => Err("value type Time is not supported for a column yet".to_string()),
        (Some("B"), _) => Err(
            "value type Boolean is not supported for a column yet (the Configurator binds it as check box data, a different record shape)"
                .to_string(),
        ),
        (Some("#"), 2) if atom(1).is_some_and(crate::naming::is_uuid) => match ref_kind {
            None => Err(format!("value type {pattern} is a reference and the object metadata does not tell its kind")),
            Some(kind) => RefKind::from_xml(kind).map(|kind| Primitive::Ref { kind }).ok_or_else(|| {
                format!(
                    "value type {pattern} is a reference of the kind {kind}; only CatalogRef, DocumentRef, EnumRef, ChartOfAccountsRef and ChartOfCharacteristicTypesRef are supported for a column"
                )
            }),
        },
        (Some("#"), _) => Err(format!("value type {pattern} is a reference that is not understood")),
        _ => Err(format!("value type {pattern} is not supported for a column")),
    }
}

/// Whether a column with this value pattern can be built (the reason otherwise).
pub fn check_pattern(pattern: &str, ref_kind: Option<&str>) -> Result<(), String> {
    classify(pattern, ref_kind).map(|_| ())
}

/// Template of the inner text of the editor blob: an `InputField` info record (v9) with its own pattern.
fn editor_text(pattern: &str, prim: Primitive) -> String {
    let (middle, a, b, parameters) = match prim {
        Primitive::String { length } => (format!("0,0,1,0,0,0,0,0,0,1,0,0,{length},0,0,4,0"), 0, 0, PRIMITIVE_PARAMETERS),
        Primitive::Number { digits, fraction, non_negative } => {
            (format!("0,0,0,0,0,0,0,0,0,1,0,0,{digits},{fraction},{non_negative},4,0"), 0, 0, PRIMITIVE_PARAMETERS)
        }
        Primitive::DateTime => ("0,0,0,0,0,1,0,0,0,1,0,0,0,0,0,4,0".to_string(), 2, 2, PRIMITIVE_PARAMETERS),
        Primitive::Date => ("0,0,0,0,0,1,0,0,0,1,0,0,0,0,0,4,0".to_string(), 0, 0, PRIMITIVE_PARAMETERS),
        // the choice and open buttons (slots 7 and 8 of the run) are on
        Primitive::Ref { kind } => ("0,0,1,0,0,1,1,0,0,1,0,0,0,0,0,4,0".to_string(), 0, 0, kind.parameters()),
    };
    format!(
        r#"{{9,{pattern},{{{{{{16,1,{{3,4,{{0}}}},{{3,4,{{0}}}},{{7,3,0,1,100}},0,{{3,3,{{-22}}}},{{3,4,{{0}}}},{{3,4,{{0}}}},{{3,3,{{-7}}}},{{3,3,{{-21}}}},{{3,1,{{-18}},0,0,0}},{{1,0}},0,0,100,0,0}},30,{middle},{{"U"}},{{"U"}},"",{a},1,0,0,0,0,{{4,0,{{0}},"",-1,-1,1,0,""}},{{4,0,{{0}},"",-1,-1,1,0,""}},{b},0,0,{{0,0,0}},{{1,0}},0,0,0,0,0,0,0,16777215,2}}}},{{1,{{{parameters}}}}},{{0}},0,1,0,{{1,0}},0}}"#
    )
}

/// Text in the layout the platform writes: a nested record starts on a new line and a record that ends with
/// a nested record closes on a new line. Nothing parses it differently, it only keeps the blob like the real ones.
fn format_1c(n: &Node, out: &mut String) {
    out.push('{');
    for (i, c) in n.children.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match c {
            Item::Node(x) => {
                out.push_str("\r\n");
                format_1c(x, out);
            }
            Item::Atom(a) => match a.kind {
                AtomKind::Bare => out.push_str(&a.value),
                AtomKind::Quoted => {
                    out.push('"');
                    out.push_str(&a.value.replace('"', "\"\""));
                    out.push('"');
                }
                AtomKind::Base64 => {
                    out.push_str("{#base64:");
                    out.push_str(&a.value);
                    out.push('}');
                }
            },
        }
    }
    if matches!(n.children.last(), Some(Item::Node(_))) {
        out.push_str("\r\n");
    }
    out.push('}');
}

/// `base64(header + deflate(u64 LE length of the text + text))` of the editor of a column with `pattern`.
fn editor_blob(pattern: &str, prim: Primitive) -> Result<String, String> {
    let tree = parse_metadata(editor_text(pattern, prim).as_bytes()).map_err(|e| format!("editor template: {e}"))?;
    let mut text = String::new();
    format_1c(&tree, &mut text);
    Ok(encode_blob(&text))
}

fn encode_blob(text: &str) -> String {
    let mut inner = vec![0xef, 0xbb, 0xbf];
    inner.extend_from_slice(text.as_bytes());
    let mut payload = (inner.len() as u64).to_le_bytes().to_vec();
    payload.extend_from_slice(&inner);
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&payload).expect("writing to a Vec");
    let deflated = enc.finish().expect("finishing a Vec");
    let mut full = BLOB_HEADER.to_vec();
    full.extend_from_slice(&deflated);
    base64::engine::general_purpose::STANDARD.encode(full)
}

/// The text of an editor blob (`header + deflate(u64 length + BOM + text)`); `None` when it is malformed or its
/// length prefix is wrong.
pub fn decode_blob(b64: &str) -> Option<String> {
    use std::io::Read;
    let raw = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    if raw.len() <= 18 || raw[..18] != BLOB_HEADER {
        return None;
    }
    let mut inflated = Vec::new();
    flate2::read::DeflateDecoder::new(&raw[18..]).read_to_end(&mut inflated).ok()?;
    let len = u64::from_le_bytes(inflated.get(..8)?.try_into().ok()?);
    if len != (inflated.len() - 8) as u64 {
        return None;
    }
    String::from_utf8(inflated[8..].to_vec()).ok().map(|s| s.trim_start_matches('\u{feff}').to_string())
}

fn bare(v: impl ToString) -> Item {
    Item::Atom(Atom::new(AtomKind::Bare, v.to_string()))
}

fn quoted(v: &str) -> Item {
    Item::Atom(Atom::new(AtomKind::Quoted, v))
}

fn parse(text: &str) -> Result<Node, String> {
    parse_metadata(text.as_bytes()).map_err(|e| format!("template {text}: {e}"))
}

/// `{1,N,{"lang","text"}...}` (`{1,0}` without items).
fn localized(items: &[(String, String)]) -> Node {
    let mut n = Node { children: vec![bare(1), bare(items.len())] };
    for (lang, text) in items {
        n.children.push(Item::Node(Node { children: vec![quoted(lang), quoted(text)] }));
    }
    n
}

/// `ceil(width / 7)`: the slot 32 of the real forms.
fn width_chars(width: i64) -> i64 {
    (width + 6) / 7
}

/// The record of one column.
pub fn column_record(spec: &ColumnSpec) -> Result<Node, String> {
    let (mut props, data, ordinal) = match &spec.data {
        ColumnData::RowNumber => (parse(ROW_NUMBER_PROPS)?, "{-2}".to_string(), -2),
        ColumnData::Field { uuid, ordinal, .. } if spec.read_only => {
            let mut props = parse(ROW_NUMBER_PROPS)?;
            props.children[9] = bare(FIELD_FLAGS);
            (props, format!("{{0,{uuid}}}"), *ordinal as i64)
        }
        ColumnData::Field { uuid, ordinal, .. } => (parse(EDITABLE_PROPS)?, format!("{{0,{uuid}}}"), *ordinal as i64),
    };
    let last = props.len() - 1;
    props.children[1] = Item::Node(localized(&spec.header));
    props.children[4] = bare(spec.width);
    props.children[5] = bare(spec.id);
    props.children[6] = bare(ordinal);
    props.children[25] = bare(i32::from(spec.visible));
    props.children[30] = quoted(&spec.name);
    props.children[32] = bare(width_chars(spec.width));
    props.children[last] = bare(spec.width);
    if let ColumnData::Field { pattern, ref_kind, .. } = &spec.data {
        let prim = classify(pattern, ref_kind.as_deref())?;
        props.children[35] = Item::Node(parse(pattern)?);
        if !spec.read_only {
            let blob = editor_blob(pattern, prim)?;
            props.children[39] = Item::Node(Node { children: vec![Item::Atom(Atom::new(AtomKind::Base64, blob)), bare(0)] });
        }
    } else if !spec.read_only {
        return Err("the column НомерСтроки is read-only".to_string());
    }
    let x = Node { children: vec![bare(8), Item::Node(props), Item::Node(parse(&data)?), Item::Node(parse("{-1}")?), Item::Node(parse("{-1}")?)] };
    let w = Node { children: vec![bare(1), Item::Node(x), bare(0), bare(0)] };
    Ok(Node { children: vec![Item::Atom(Atom::new(AtomKind::Bare, COLUMN_GUID)), Item::Node(w)] })
}

/// `{N, column...}`.
pub fn column_list(specs: &[ColumnSpec]) -> Result<Node, String> {
    let mut list = Node { children: vec![bare(specs.len())] };
    for s in specs {
        list.children.push(Item::Node(column_record(s)?));
    }
    Ok(list)
}

/// `{"Pattern",{"#",<TS TypeId>}}`.
pub fn tabular_pattern(type_id: &str) -> String {
    format!(r##"{{"Pattern",{{"#",{type_id}}}}}"##)
}

/// Binds the table record `info` (a clone of a donor table) to the tabular section with `type_id`: value type, row
/// source and the columns.
pub fn bind_info(info: &mut Node, type_id: &str, specs: &[ColumnSpec]) -> Result<(), String> {
    let list = column_list(specs)?;
    let value_type = parse(&tabular_pattern(type_id))?;
    let row_source = parse(ROW_SOURCE)?;
    if info.children.len() < 4 {
        return Err("the table record is too short".to_string());
    }
    let slot = info.children[2]
        .as_node_mut()
        .and_then(|n| n.children.get_mut(1))
        .and_then(Item::as_node_mut)
        .and_then(|t| t.children.get_mut(23))
        .ok_or("the table record has no column list at [2][1][23]")?;
    *slot = Item::Node(list);
    info.children[1] = Item::Node(value_type);
    info.children[3] = Item::Node(row_source);
    Ok(())
}

/// Name of the column that holds the row number of a tabular section.
pub const ROW_NUMBER: &str = "НомерСтроки";

/// One `<Column>` of an added table as the parser saw it.
#[derive(Debug, Clone, Default)]
pub struct ColumnRequest {
    pub line: u32,
    pub name: String,
    pub id: Option<i64>,
    pub header: Vec<(String, String)>,
    pub width: i64,
    pub visible: bool,
    pub read_only: bool,
    pub editor: Option<String>,
    /// `attributeId` attribute and text of the `<DataPath>` of the column.
    pub attribute_id: Option<String>,
    pub data_path: Option<String>,
    /// `(line, what)` of everything else the column carries that cannot be written.
    pub unsupported: Vec<(u32, String)>,
}

/// The column records of an added table bound to the tabular section `ts` of the main attribute `root` (whose id
/// is `attr_id`). The errors are the refusals, each with its line.
pub fn plan_columns(root: &str, attr_id: i64, ts: &TabularSchema, requests: &[ColumnRequest]) -> Result<Vec<ColumnSpec>, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    let mut specs = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for r in requests {
        let at = format!("line {}: column '{}'", r.line, r.name);
        let before = problems.len();
        for (line, what) in &r.unsupported {
            problems.push(format!("line {line}: column '{}': {what} is not supported on a column of an added table", r.name));
        }
        match r.id {
            None => problems.push(format!("{at}: the id attribute is missing")),
            Some(id) if id < 0 => problems.push(format!("{at}: the id must not be negative")),
            Some(id) if !ids.insert(id) => problems.push(format!("{at}: the id {id} is used by another column")),
            _ => {}
        }
        if r.width <= 0 {
            problems.push(format!("{at}: Width must be positive"));
        }
        if let Some(editor) = r.editor.as_deref().filter(|e| *e != "InputField") {
            problems.push(format!("{at}: the editor {editor} is not supported (only InputField)"));
        }
        let Some(path) = r.data_path.as_deref() else {
            problems.push(format!("{at}: <DataPath> is required: {root}.{}.<column of the section> or {root}.{}.{ROW_NUMBER}", ts.name, ts.name));
            continue;
        };
        if r.attribute_id.as_deref().map(str::trim) != Some(attr_id.to_string().as_str()) {
            problems.push(format!("{at}: DataPath '{path}': attributeId must be {attr_id} (the main attribute)"));
            continue;
        }
        let parts: Vec<&str> = path.split('.').collect();
        let [path_root, section, field] = parts.as_slice() else {
            problems.push(format!("{at}: DataPath '{path}' must be {root}.{}.<column>", ts.name));
            continue;
        };
        if *path_root != root {
            problems.push(format!("{at}: DataPath '{path}': the path starts with '{path_root}', the table's path with '{root}'"));
            continue;
        }
        if *section != ts.name {
            problems.push(format!(
                "{at}: DataPath '{path}': the column belongs to the tabular section '{section}', the table is bound to '{}'",
                ts.name
            ));
            continue;
        }
        let data = if *field == ROW_NUMBER {
            if r.editor.is_some() || !r.read_only {
                problems.push(format!("{at}: {ROW_NUMBER} is read-only and has no editor (use <ReadOnly>true</ReadOnly>, without <Editor>)"));
                continue;
            }
            ColumnData::RowNumber
        } else {
            if r.read_only && r.editor.is_some() {
                problems.push(format!("{at}: a read-only column has no editor: remove <Editor> or use <ReadOnly>false</ReadOnly>"));
                continue;
            }
            let Some(ordinal) = ts.column_ordinal(field) else {
                let known: Vec<&str> = ts.columns.iter().map(|(n, _)| n.as_str()).collect();
                let why = if known.iter().filter(|n| **n == *field).count() > 1 { "occurs twice" } else { "does not exist" };
                problems.push(format!(
                    "{at}: DataPath '{path}': the column '{field}' {why} in the tabular section '{}' (columns: {}, {ROW_NUMBER})",
                    ts.name,
                    known.join(", ")
                ));
                continue;
            };
            let Some(pattern) = ts.column_pattern(ordinal) else {
                problems.push(format!(
                    "{at}: DataPath '{path}': the value type of the column '{field}' is not known (a composite type or a reference type that the configuration does not resolve)"
                ));
                continue;
            };
            let ref_kind = ts.column_ref_kind(ordinal);
            if let Err(why) = check_pattern(pattern, ref_kind) {
                problems.push(format!("{at}: DataPath '{path}': {why}"));
                continue;
            }
            ColumnData::Field {
                uuid: ts.columns[ordinal].1.to_ascii_lowercase(),
                ordinal,
                pattern: pattern.to_string(),
                ref_kind: ref_kind.map(str::to_string),
            }
        };
        if problems.len() > before {
            continue;
        }
        specs.push(ColumnSpec {
            name: r.name.clone(),
            id: r.id.unwrap_or(0),
            header: r.header.clone(),
            width: r.width,
            visible: r.visible,
            read_only: r.read_only,
            data,
        });
    }
    if problems.is_empty() {
        Ok(specs)
    } else {
        Err(problems)
    }
}

/// What the stream says about one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnRead {
    pub name: String,
    pub id: i64,
    pub ordinal: i64,
    /// Text of the data step `X[2]` (`{0,<uuid>}`, `{-2}`, `{-1}`).
    pub data: String,
    pub slots: usize,
    /// The whole column record as text.
    pub record: String,
}

fn text_of(n: &Node) -> String {
    let mut out = Vec::new();
    crate::donor::write_node(n, &mut out);
    String::from_utf8_lossy(&out).into_owned()
}

/// `(value type, row source, columns)` of a table record; `None` when the record does not have the layout.
pub fn read_table(info: &Node) -> Option<(String, String, Vec<ColumnRead>)> {
    let list = info.node(2)?.node(1)?.node(23)?;
    let count: usize = list.atom(0)?.value.parse().ok()?;
    let mut columns = Vec::new();
    for i in 1..=count {
        let col = list.node(i)?;
        let x = col.node(1)?.node(1)?;
        let props = x.node(1)?;
        columns.push(ColumnRead {
            name: props.atom(30)?.value.clone(),
            id: props.atom(5)?.value.parse().ok()?,
            ordinal: props.atom(6)?.value.parse().ok()?,
            data: text_of(x.node(2)?),
            slots: props.len(),
            record: text_of(col),
        });
    }
    if list.len() != count + 1 {
        return None;
    }
    Some((text_of(info.node(1)?), text_of(info.node(3)?), columns))
}

/// Compares the table record `info` as read back from a stream with what [`bind_info`] was asked to write: value
/// type, row source, column count and, for every column, name, id, ordinal, data step and the whole record.
pub fn check_table(info: &Node, type_id: &str, specs: &[ColumnSpec]) -> Result<(), String> {
    let (value_type, row_source, columns) = read_table(info).ok_or("the table record does not have the layout of a bound table")?;
    let want = text_of(&parse(&tabular_pattern(type_id))?);
    if value_type != want {
        return Err(format!("the value type of the table is {value_type}, expected {want}"));
    }
    if row_source != ROW_SOURCE {
        return Err(format!("the row source of the table is {row_source}, expected {ROW_SOURCE}"));
    }
    if columns.len() != specs.len() {
        return Err(format!("the table has {} columns, expected {}", columns.len(), specs.len()));
    }
    for (got, spec) in columns.iter().zip(specs) {
        let (ordinal, data) = match &spec.data {
            ColumnData::RowNumber => (-2, "{-2}".to_string()),
            ColumnData::Field { uuid, ordinal, .. } => (*ordinal as i64, format!("{{0,{uuid}}}")),
        };
        if (got.name.as_str(), got.id, got.ordinal, got.data.as_str()) != (spec.name.as_str(), spec.id, ordinal, data.as_str()) {
            return Err(format!(
                "column '{}': stored as name '{}', id {}, ordinal {}, data {}; expected id {}, ordinal {ordinal}, data {data}",
                spec.name, got.name, got.id, got.ordinal, got.data, spec.id
            ));
        }
        if got.record != text_of(&column_record(spec)?) {
            return Err(format!("column '{}': the stored record differs from the one that was built", spec.name));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(name: &str, id: i64, data: ColumnData, read_only: bool) -> ColumnSpec {
        ColumnSpec { name: name.into(), id, header: vec![("ru".into(), name.into())], width: 100, visible: true, read_only, data }
    }

    fn field(ordinal: usize, pattern: &str) -> ColumnData {
        ColumnData::Field { uuid: format!("33333333-0000-0000-0000-00000000000{ordinal}"), ordinal, pattern: pattern.into(), ref_kind: None }
    }

    /// The editor blob of a column of `Корзина` with the editor pattern `{"N",10,3,1}`, as the platform wrote it.
    const REAL_BLOB: &str = "AgFTS2/0iI3BTqDV67a9oKcNfVA7DsIwDEWwVeolMjuSnbRNcgnEwgEKdGVA3aqcjIEjcQUSuwRUPkkk5/nZec/ZrFe87tfbFKCuJrXrx3G4nBWDrQJCsECxrmLOyKEOKCMLTQ74ZJfYpVZMpYQY04UrbA7amF89n1g63DJh6JVhN5p8VkmbswQoMKlL0iJ8bppr8pRJml2qvYolKlWqUOi5CpnTlE/mlTT9YcsLX1y+Oeqcc4ZaKH/E44XedY09GU1hCLppB6O9D1Ybj4djoND2wYn+cgKUGfhL5xdZlMSMWIgP";

    /// Editor blobs of columns with a reference type in real forms (v19, no events, default parameters): the kind,
    /// the TypeId in the pattern and the blob.
    const REAL_REF_BLOBS: [(RefKind, &str, &str); 5] = [
        (RefKind::Catalog, "c7fe2ab6-14f2-4f47-a9ca-61ee6ce8b430", "AgFTS2/0iI3BTqDV67a9oKcNjVFBbgMhDKx6jJRP0KstYTCY/KKXPGB3yx57qHKLeFkOfVK/UAO7mzSNqtqWzYwNtkV6fmrydfk8H2C/O5vX4XTKH++mgRcDk8zZDWNE4tkhzyw4HKYBI+Ucp5xG9rbsd6XWd6MIVJEHrmHL3mMBD1ZLydqih3/c+I19DSj3hKMr02ZBSrWHamMJbIfau5Pedrx4ukFduY9ojqZs0ZgfRUovVbblkKrVvOmX/shuLzwY8qoURcRRALfu17ZLIQVnZUT9GEF2QU8+BcxpfosycSZOtS7oEw7sIvjArbL21y7dlrn7uttw5Rs="),
        (RefKind::Document, "65d48f7f-b7d6-4ef4-999c-005178776986", "AgFTS2/0iI3BTqDV67a9oKcNjVBLbkMhDKy6jJRL0F2FJZuf4Rbd5ADvx7KLKruIk2XRI/UKBfxekqZRVRvZzNiYgdfnp25f589T0vvdSb0Nx+Py8a46eFE6+NnFzBlGngO4JTtIKU2A6Ikjc0gxlP2utH5ZFDQ1ZLVrCbfqPWZtNdZWQix1848Tv7FtCfieMHRluhag2O6o3lnSKLDeLaRFwWukGyTuRKI6qHLJSv1oqvTahb0G1FarKzn0R/Uy4YHIq1NgZkNem+19/XVEeaHReEg8eXBz3Q2jzRCnNPjgPRmzyGfgavAgbNZGr9O7ABJhIqd8AwIBU0tv9IiNwU6g1eu2vaCnDWNmgID3u/cDAA=="),
        (RefKind::Enum, "52c49a35-3374-4995-9803-5fc37bb7d7cf", "AgFTS2/0iI3BTqDV67a9oKcNjVBLbgMhDK2yjJRL0C2WbD4x3CKbHGCGYZZdVNlFnKyLHqlXKOCZpE2iKDbC854f5g1689bj5+v7HPVue1aH4XTKnx+qg3elvUkuDtaDtezAxeghBrTg52R5HHniNJfdtjS9LNprashq1wqu3VvM2mqsUkIs9eOFE/fYtgJ8Sxi6Mt0LUGh31OwsaRRY7xbSouBlpz9I0olFdVTlUpX6J6r0osLeA2qr9ZUcetK9THhg8pq0Z2ZDXpv1/+SlMWceOUGYhgQuG4TRhwyZZ2ZnhhymIOI6cwl4sK3Rhi/zuwUSa2Ko/AI="),
        (RefKind::ChartOfAccounts, "2ab79427-7bc9-43d3-aec3-7a54ed35c9ee", "AgFTS2/0iI3BTqDV67a9oKcNjU9LbgMhDI26jJRL0C2WMJgx3KKbHgBmyLKLKLuIk3XRI/UK5TeTNo2i2BbmPRv8PL3smn1/fl28POwv4i2cz+n0IRp4FVKHyJ40A8fZA5nFQEizAQ6W0mLs7FPKh32u/T1wkliRkVSTWqu3mKWRqrSiUrlcnnjxH5uagG8JjVemaQF0dUbxxqJUHZbZnTSq43HiL9SdukTxLvKWhfjTVOjRpVoNsEati/7oQXX74Y7Iq+PEzBqt1Ot+bTucQqR0tODQRaCjCxC8dWAxLURJcVzC+FGqYXDnWG2dXmb0GKr7spu0/AMCAVNLb/SIjcFOoNXrtr2gpw1jZoCA97v3AwA="),
        (RefKind::ChartOfCharacteristicTypes, "a0658439-32bd-4b4a-a2cc-88c4c8570eaf", "AgFTS2/0iI3BTqDV67a9oKcNjVFLTgMxDEUsK/USYWtLdv65BRsOkEkzSxaouyonY8GRuAL5TKZQKoRtxfOenfhZ4x8fun2+f1wCHA8X8RzP5/z2Kjp4EhDJGq9VQCWXE+pFR4wyJfQ+6eSNoxzXcjyU1j+CLXBDCnRLNKu32IECqq1MVOrHP278xqoldLeE5CvTtSD7NqN6ZxlowDp7kIoG3k7+hobrIVG8iLJnIX40VXrrol5DbtHqYlz6o7q/cEfk1dk65yQbkHO/vp1ZrbEpBMxZLfUfLQ6DjIzZriYGlWQKp/m2BNoM7xzT5vw6ZcSme6y7iytf"),
    ];

    #[test]
    fn the_blob_format_is_the_platform_one() {
        // decode_blob checks the header and the length prefix; our own blob must decode the same way
        let ours = editor_blob(r#"{"Pattern",{"N",10,3,1}}"#, Primitive::Number { digits: 10, fraction: 3, non_negative: 1 }).unwrap();
        let text = decode_blob(&ours).expect("our blob decodes with a valid length prefix");
        assert!(text.starts_with("{9,\r\n{\"Pattern\",\r\n{\"N\",10,3,1}\r\n},\r\n{\r\n{\r\n{16,1,\r\n{3,4,\r\n{0}\r\n}"), "{text}");
        assert!(text.ends_with("{1,0},0}"), "{text}");
        // the text is the one of the real blob (the compressed bytes may differ with the deflate implementation)
        assert_eq!(decode_blob(REAL_BLOB).expect("the real blob has a valid header and length prefix"), text);
    }

    #[test]
    fn the_reference_editors_are_the_platform_ones() {
        for (kind, type_id, real) in REAL_REF_BLOBS {
            let pattern = format!(r##"{{"Pattern",{{"#",{type_id}}}}}"##);
            let ours = decode_blob(&editor_blob(&pattern, Primitive::Ref { kind }).unwrap()).expect("our blob has a valid length prefix");
            let real = decode_blob(real).expect("the real blob has a valid header and length prefix");
            // the real forms differ in the style colors of the first record only
            let flat = |t: &str| t.replace(['\r', '\n'], "");
            assert_eq!(flat(&ours).replace("{3,3,{-22}}", "{3,4,{0}}"), flat(&real), "{kind:?}");
            assert!(flat(&ours).contains(&format!("{{1,{{{}}}}}", kind.parameters())), "{kind:?}");
        }
    }

    #[test]
    fn a_reference_column_has_the_pattern_of_the_schema_and_the_editor_of_its_kind() {
        let type_id = "47e6dd42-cd5c-4e3a-9bc4-14816ed53d38";
        let pattern = format!(r##"{{"Pattern",{{"#",{type_id}}}}}"##);
        let data = ColumnData::Field {
            uuid: "33333333-0000-0000-0000-000000000003".into(),
            ordinal: 3,
            pattern: pattern.clone(),
            ref_kind: Some("CatalogRef".into()),
        };
        let c = column_record(&spec("Контрагент", 4, data.clone(), false)).unwrap();
        let p = c.node(1).unwrap().node(1).unwrap().node(1).unwrap();
        assert_eq!((p.len(), p.atom(38).unwrap().value.as_str(), p.atom(6).unwrap().value.as_str()), (46, INPUT_FIELD_GUID, "3"));
        assert_eq!(text_of(p.node(35).unwrap()), pattern);
        let blob = p.node(39).unwrap().atom(0).unwrap();
        assert_eq!(blob.kind, AtomKind::Base64);
        let text = decode_blob(&blob.value).unwrap().replace(['\r', '\n'], "");
        assert!(text.starts_with(&format!(r##"{{9,{pattern},"##)), "{text}");
        assert!(text.contains("{1,{8585207b-9ca7-425b-9385-e8fd67c4e148,{5,0,2,"), "{text}");
        // a read-only reference column has no editor, as any other read-only column
        let ro = column_record(&spec("Контрагент", 4, data, true)).unwrap();
        let p = ro.node(1).unwrap().node(1).unwrap().node(1).unwrap();
        assert_eq!((p.len(), p.atom(38).unwrap().value.as_str()), (45, NO_EDITOR_GUID));
        assert_eq!(text_of(p.node(35).unwrap()), pattern);
    }

    #[test]
    fn references_are_classified_by_their_kind() {
        let one = r##"{"Pattern",{"#",44a01d39-c2b6-4751-85e6-06fdc2f23043}}"##;
        for (kind, want) in [
            ("CatalogRef", RefKind::Catalog),
            ("DocumentRef", RefKind::Document),
            ("EnumRef", RefKind::Enum),
            ("ChartOfAccountsRef", RefKind::ChartOfAccounts),
            ("ChartOfCharacteristicTypesRef", RefKind::ChartOfCharacteristicTypes),
        ] {
            assert_eq!(classify(one, Some(kind)).unwrap(), Primitive::Ref { kind: want });
        }
        for (kind, why) in [
            (None, "does not tell its kind"),
            (Some("ExchangePlanRef"), "kind ExchangePlanRef"),
            (Some("BusinessProcessRef"), "kind BusinessProcessRef"),
            (Some("CatalogObject"), "kind CatalogObject"),
        ] {
            let e = check_pattern(one, kind).unwrap_err();
            assert!(e.contains(why), "{kind:?}: {e}");
        }
        let composite = r##"{"Pattern",{"#",44a01d39-c2b6-4751-85e6-06fdc2f23043},{"#",54a01d39-c2b6-4751-85e6-06fdc2f23043}}"##;
        assert!(check_pattern(composite, Some("CatalogRef")).unwrap_err().contains("composite"));
        assert!(check_pattern(r##"{"Pattern",{"#"}}"##, Some("CatalogRef")).unwrap_err().contains("not understood"));
        // a non-reference type ignores the kind
        check_pattern(r#"{"Pattern",{"S",10,1}}"#, Some("CatalogRef")).unwrap();
    }

    #[test]
    fn every_template_parses_and_has_the_slot_count() {
        for (t, n) in [(EDITABLE_PROPS, 46), (ROW_NUMBER_PROPS, 45)] {
            assert_eq!(parse(t).unwrap().len(), n);
        }
        assert_eq!(width_chars(100), 15);
        assert_eq!(width_chars(75), 11);
        assert_eq!(width_chars(28), 4);
        assert_eq!(width_chars(200), 29);
        assert_eq!(width_chars(120), 18);
    }

    #[test]
    fn column_records_have_the_documented_shape() {
        let rn = column_record(&spec("Номер", 0, ColumnData::RowNumber, true)).unwrap();
        let f = column_record(&spec("Цена", 7, field(2, r#"{"Pattern",{"N",15,2,0}}"#), false)).unwrap();
        let ro = column_record(&spec("Итог", 8, field(1, r#"{"Pattern",{"S",10,1}}"#), true)).unwrap();
        let info = |c: &Node| {
            let x = c.node(1).unwrap().node(1).unwrap().clone();
            let p = x.node(1).unwrap().clone();
            (x, p)
        };
        let (x, p) = info(&rn);
        assert_eq!((p.len(), text_of(x.node(2).unwrap()), p.atom(6).unwrap().value.as_str()), (45, "{-2}".to_string(), "-2"));
        assert_eq!(p.atom(38).unwrap().value, NO_EDITOR_GUID);
        let (x, p) = info(&f);
        assert_eq!(p.len(), 46);
        assert_eq!(text_of(x.node(2).unwrap()), "{0,33333333-0000-0000-0000-000000000002}");
        assert_eq!((p.atom(5).unwrap().value.as_str(), p.atom(6).unwrap().value.as_str(), p.atom(26).unwrap().value.as_str()), ("7", "2", "0"));
        assert_eq!(text_of(p.node(35).unwrap()), r#"{"Pattern",{"N",15,2,0}}"#);
        assert_eq!(p.atom(38).unwrap().value, INPUT_FIELD_GUID);
        assert_eq!((p.atom(32).unwrap().value.as_str(), p.atom(45).unwrap().value.as_str()), ("15", "100"));
        let blob = p.node(39).unwrap().atom(0).unwrap();
        assert_eq!(blob.kind, AtomKind::Base64);
        let text = decode_blob(&blob.value).unwrap();
        assert!(text.replace(['\r', '\n'], "").contains(r#"{"Pattern",{"N",15,2,0}}"#), "the editor carries its own pattern: {text}");
        // a read-only field column has no editor: the layout of НомерСтроки with the field data and the flags of a field
        let (x, p) = info(&ro);
        assert_eq!(p.len(), 45);
        assert_eq!(text_of(x.node(2).unwrap()), "{0,33333333-0000-0000-0000-000000000001}");
        assert_eq!((p.atom(5).unwrap().value.as_str(), p.atom(6).unwrap().value.as_str(), p.atom(26).unwrap().value.as_str()), ("8", "1", "0"));
        assert_eq!((p.atom(9).unwrap().value.as_str(), p.atom(38).unwrap().value.as_str()), ("12590592", NO_EDITOR_GUID));
        assert_eq!(text_of(p.node(35).unwrap()), r#"{"Pattern",{"S",10,1}}"#);
        assert_eq!((p.atom(32).unwrap().value.as_str(), p.atom(44).unwrap().value.as_str()), ("15", "100"));
    }

    #[test]
    fn unsupported_value_types_have_a_reason() {
        for (pat, why) in [
            (r#"{"Pattern",{"B"}}"#, "Boolean"),
            (r#"{"Pattern",{"S"}}"#, "unlimited"),
            (r#"{"Pattern",{"S",10,0}}"#, "fixed length"),
            (r#"{"Pattern",{"D","T"}}"#, "Time"),
            (r##"{"Pattern",{"#",44a01d39-c2b6-4751-85e6-06fdc2f23043}}"##, "does not tell its kind"),
            (r#"{"Pattern",{"S",10,1},{"N",1,0,0}}"#, "composite"),
        ] {
            let e = check_pattern(pat, None).unwrap_err();
            assert!(e.contains(why), "{pat}: {e}");
        }
        for pat in [r#"{"Pattern",{"S",10,1}}"#, r#"{"Pattern",{"N",15,2,1}}"#, r#"{"Pattern",{"D"}}"#, r#"{"Pattern",{"D","D"}}"#] {
            check_pattern(pat, None).unwrap();
        }
    }

    #[test]
    fn a_table_record_is_bound_and_read_back() {
        let donor = crate::donor::prototype("Table").unwrap();
        let mut info = donor.node(2).unwrap().clone();
        let specs = vec![
            spec("Номер", 0, ColumnData::RowNumber, true),
            spec("Цена", 5, field(2, r#"{"Pattern",{"N",15,2,0}}"#), false),
            spec("Дата", 3, field(0, r#"{"Pattern",{"D","D"}}"#), true),
            spec(
                "Вид",
                4,
                ColumnData::Field {
                    uuid: "33333333-0000-0000-0000-000000000004".into(),
                    ordinal: 3,
                    pattern: r##"{"Pattern",{"#",44a01d39-c2b6-4751-85e6-06fdc2f23043}}"##.into(),
                    ref_kind: Some("EnumRef".into()),
                },
                false,
            ),
        ];
        bind_info(&mut info, "32dc563e-0b51-4621-9936-5bfc9fd58ef3", &specs).unwrap();
        check_table(&info, "32dc563e-0b51-4621-9936-5bfc9fd58ef3", &specs).unwrap();
        assert!(check_table(&info, "32dc563e-0b51-4621-9936-5bfc9fd58ef4", &specs).unwrap_err().contains("value type"));
        let mut other = specs.clone();
        other[1].id = 9;
        assert!(check_table(&info, "32dc563e-0b51-4621-9936-5bfc9fd58ef3", &other).unwrap_err().contains("column 'Цена'"));
        assert!(check_table(&info, "32dc563e-0b51-4621-9936-5bfc9fd58ef3", &specs[..2]).unwrap_err().contains("4 columns"));
        let (vt, source, cols) = read_table(&info).unwrap();
        assert_eq!(vt, r##"{"Pattern",{"#",32dc563e-0b51-4621-9936-5bfc9fd58ef3}}"##);
        assert_eq!(source, ROW_SOURCE);
        let got: Vec<(&str, i64, i64, usize)> = cols.iter().map(|c| (c.name.as_str(), c.id, c.ordinal, c.slots)).collect();
        assert_eq!(got, [("Номер", 0, -2, 45), ("Цена", 5, 2, 46), ("Дата", 3, 0, 45), ("Вид", 4, 3, 46)]);
        for (c, s) in cols.iter().zip(&specs) {
            assert_eq!(c.record, text_of(&column_record(s).unwrap()));
        }
        // the decoder reads the same names and ids back
        let props = info.node(2).unwrap().node(1).unwrap().node(23).unwrap().node(2).unwrap().node(1).unwrap().node(1).unwrap().node(1).unwrap();
        let decoded = crate::oof_props::decode_column_props(props).unwrap();
        assert_eq!(decoded["id"], 5);
        assert_eq!(decoded["width"], 100);
        assert_eq!(decoded["valueType"]["kind"], "Number");
    }
}
