//! A `Table` bound to a ValueTable form attribute: the stream records of its columns, built from the value-table column
//! of the donor form (OpenSpec change `add-form-attributes-and-value-tables`).
//!
//! Layout of a table bound to a value table (`info = control[2]`), as the donor table `ТабличноеПоле1` and the real
//! forms have it:
//! - the link of the table in `root[2][3]` is `{1,{aid}}` (the form attribute itself);
//! - `info[1]` = `{"Pattern",{"#",acf6192e-...}}`, `info[3]` = [`ROW_SOURCE`]: both stay as the donor table has them;
//! - `info[2][1][23]` = `{N, column...}`; a column is `{737535a4-..., W}` with `W = {8, X, "<field>", "", "", 0}` and
//!   `X = {8, P, {-1}, {-1}, {-1}}`: the field of the value table is named inside the column record (`W[2]`), the
//!   data steps are empty.
//!
//! `P` (v19) has 46 slots, as the tabular column of [`crate::ts_table`], with `P[6]`, `P[7]`, `P[8]` = -1 (no field
//! ordinal) and `P[27]`, `P[29]` = 0 (as in the donor column and in the 170 version 19 value table columns of the real
//! forms). The editor is always an `InputField` with the editor blob of the value type; read-only is `P[26]` = 1, the
//! editor stays (6 real columns of version 19).

use crate::brace::{Atom, AtomKind, Item, Node};
use crate::form_patch::ValueTypeSpec;
use crate::oof::VT_COLUMN_GUID;
use crate::ts_table::{bare, editor_blob, encode_blob, format_1c, localized, parse, quoted, text_of, width_chars, Primitive};

/// Row source of the donor table (`info[3]`): a value table.
pub const ROW_SOURCE: &str = "{342cf854-134c-42bb-8af9-a2103d5d9723,{5,0,0,1}}";
/// `info[1]` of a table bound to a value table.
pub const VALUE_TABLE_PATTERN: &str = r##"{"Pattern",{"#",acf6192e-81ca-46ef-93a6-5a6968b78663}}"##;

/// Column `P` with an `InputField` editor (46 slots), from the first column of the donor table; the placeholders are
/// patched by [`column_record`].
const PROPS: &str = r#"{19,{1,0},{1,0},{1,0},100,0,-1,-1,-1,12590592,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},16,16,d2314b5d-8da4-4e0f-822b-45e7500eae09,{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{7,3,0,1,100},{7,3,0,1,100},1,0,0,4,0,"",{},15,0,{1,0},{"Pattern",{"S"}},0,1,381ed624-9217-4e63-85db-c4c3cb87daae,{0,0},0,0,0,0,0,100}"#;

/// The inner text of the `InputField` editor of a Boolean column, as the one real column of version 19 with a Boolean
/// pattern and an input field has it (its record versions differ from the ones of the other types).
const BOOLEAN_EDITOR: &str = r#"{9,{"Pattern",{"B"}},{{{13,1,{3,4,{0}},{3,4,{0}},{7,3,0,1,100},0,{3,3,{-22}},{3,4,{0}},{3,4,{0}},{3,3,{-7}},{3,3,{-21}},{3,1,{-18},0,0,0},{1,0},0,0},29,0,0,1,0,0,1,0,0,0,1,0,0,0,0,0,4,0,{"U"},{"U"},"",0,1,0,0,0,0,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},0,0,0,{0,0,0},{1,0},0,0,0,0,0,0,0,16777215}},{1,{9a7643d2-19e9-45e2-8893-280bc9195a97,{4,{"U"},{"U"},0,"",0,0}}},{0},0,1,0,{1,0},0}"#;

/// The simple value types of a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtType {
    String { length: Option<i64> },
    Number { digits: i64, fraction: i64, non_negative: bool },
    Date,
    DateTime,
    Boolean,
}

impl VtType {
    /// The type of `<ValueType>`; the error is the reason why a column cannot have it.
    pub fn from_spec(spec: &ValueTypeSpec) -> Result<VtType, String> {
        let kind = spec.kind.as_str();
        if spec.composite {
            return Err(format!("a composite value type ({kind}) is not supported (use String, Number, Date, DateTime or Boolean)"));
        }
        Ok(match kind {
            "String" if spec.fixed => return Err("strings of fixed length are not supported (a string with a length has a variable length)".to_string()),
            "String" => VtType::String { length: spec.length },
            "Number" => {
                let digits = spec.digits.unwrap_or(10);
                let fraction = spec.fraction_digits.unwrap_or(0);
                if digits > 38 || fraction > digits {
                    return Err(format!("Number needs digits of 1 to 38 and fractionDigits of at most digits (got {digits}, {fraction})"));
                }
                VtType::Number { digits, fraction, non_negative: spec.non_negative }
            }
            "Date" => VtType::Date,
            "DateTime" => VtType::DateTime,
            "Boolean" => VtType::Boolean,
            other => {
                return Err(format!("the value type {other} is not supported for a column (String, Number, Date, DateTime and Boolean are)"));
            }
        })
    }

    /// `{"Pattern",...}` of the type, as the stream holds it.
    pub fn pattern(&self) -> String {
        match self {
            VtType::String { length: None } => r#"{"Pattern",{"S"}}"#.to_string(),
            VtType::String { length: Some(l) } => format!(r#"{{"Pattern",{{"S",{l},1}}}}"#),
            VtType::Number { digits, fraction, non_negative } => {
                format!(r#"{{"Pattern",{{"N",{digits},{fraction},{}}}}}"#, i32::from(*non_negative))
            }
            VtType::Date => r#"{"Pattern",{"D","D"}}"#.to_string(),
            VtType::DateTime => r#"{"Pattern",{"D"}}"#.to_string(),
            VtType::Boolean => r#"{"Pattern",{"B"}}"#.to_string(),
        }
    }

    fn editor(&self) -> Result<String, String> {
        let pattern = self.pattern();
        let prim = match self {
            VtType::String { length } => Primitive::String { length: length.unwrap_or(0) },
            VtType::Number { digits, fraction, non_negative } => {
                Primitive::Number { digits: *digits, fraction: *fraction, non_negative: i64::from(*non_negative) }
            }
            VtType::Date => Primitive::Date,
            VtType::DateTime => Primitive::DateTime,
            VtType::Boolean => {
                let tree = parse(BOOLEAN_EDITOR)?;
                let mut text = String::new();
                format_1c(&tree, &mut text);
                return Ok(encode_blob(&text));
            }
        };
        editor_blob(&pattern, prim)
    }
}

/// Everything a column record is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtColumn {
    /// Name of the column control (`P[30]`).
    pub name: String,
    pub id: i64,
    /// Name of the field of the value table (`W[2]`).
    pub field: String,
    pub header: Vec<(String, String)>,
    pub width: i64,
    pub visible: bool,
    pub read_only: bool,
    pub value: VtType,
}

/// The record of one column.
pub fn column_record(c: &VtColumn) -> Result<Node, String> {
    let mut props = parse(PROPS)?;
    let last = props.len() - 1;
    props.children[1] = Item::Node(localized(&c.header));
    props.children[4] = bare(c.width);
    props.children[5] = bare(c.id);
    props.children[25] = bare(i32::from(c.visible));
    props.children[26] = bare(i32::from(c.read_only));
    props.children[30] = quoted(&c.name);
    props.children[32] = bare(width_chars(c.width));
    props.children[35] = Item::Node(parse(&c.value.pattern())?);
    props.children[39] = Item::Node(Node { children: vec![Item::Atom(Atom::new(AtomKind::Base64, c.value.editor()?)), bare(0)] });
    props.children[last] = bare(c.width);
    let steps = || Ok::<_, String>(Item::Node(parse("{-1}")?));
    let x = Node { children: vec![bare(8), Item::Node(props), steps()?, steps()?, steps()?] };
    let w = Node { children: vec![bare(8), Item::Node(x), quoted(&c.field), quoted(""), quoted(""), bare(0)] };
    Ok(Node { children: vec![Item::Atom(Atom::new(AtomKind::Bare, VT_COLUMN_GUID)), Item::Node(w)] })
}

/// `{N, column...}`.
pub fn column_list(columns: &[VtColumn]) -> Result<Node, String> {
    let mut list = Node { children: vec![bare(columns.len())] };
    for c in columns {
        list.children.push(Item::Node(column_record(c)?));
    }
    Ok(list)
}

/// Puts the columns into the table record `info` (a clone of the donor table, which is bound to a value table).
pub fn bind_info(info: &mut Node, columns: &[VtColumn]) -> Result<(), String> {
    let list = column_list(columns)?;
    let slot = info
        .children
        .get_mut(2)
        .and_then(Item::as_node_mut)
        .and_then(|n| n.children.get_mut(1))
        .and_then(Item::as_node_mut)
        .and_then(|t| t.children.get_mut(23))
        .ok_or("the table record has no column list at [2][1][23]")?;
    *slot = Item::Node(list);
    Ok(())
}

/// Compares the table record `info` as read back from a stream with what [`bind_info`] was asked to write: the value
/// type and the row source of the donor table, the number of columns and every column record as a whole.
pub fn check_table(info: &Node, columns: &[VtColumn]) -> Result<(), String> {
    let list = info
        .node(2)
        .and_then(|n| n.node(1))
        .and_then(|n| n.node(23))
        .ok_or("the table record does not have the layout of a table")?;
    let count: usize = list.atom(0).and_then(|a| a.value.parse().ok()).ok_or("the column list has no count")?;
    if count != columns.len() || list.len() != count + 1 {
        return Err(format!("the table has {count} columns, expected {}", columns.len()));
    }
    let value_type = info.node(1).map(text_of).unwrap_or_default();
    if value_type != VALUE_TABLE_PATTERN {
        return Err(format!("the value type of the table is {value_type}, expected {VALUE_TABLE_PATTERN}"));
    }
    let row_source = info.node(3).map(text_of).unwrap_or_default();
    if row_source != ROW_SOURCE {
        return Err(format!("the row source of the table is {row_source}, expected {ROW_SOURCE}"));
    }
    for (i, want) in columns.iter().enumerate() {
        let got = list.node(i + 1).ok_or("a column record is missing")?;
        if text_of(got) != text_of(&column_record(want)?) {
            return Err(format!("column '{}': the stored record differs from the one that was built", want.name));
        }
    }
    Ok(())
}

/// One `<Column>` of an added table as the parser saw it.
#[derive(Debug, Clone, Default)]
pub struct VtColumnRequest {
    pub line: u32,
    pub name: String,
    pub id: Option<i64>,
    pub header: Vec<(String, String)>,
    pub width: i64,
    pub visible: bool,
    pub read_only: bool,
    pub editor: Option<String>,
    /// `attributeId` attribute, text and `steps` of the `<DataPath>` of the column.
    pub attribute_id: Option<String>,
    pub data_path: Option<String>,
    pub steps: Option<String>,
    /// `<ValueType>` of the column: its line and the type (`None`: not a plain type).
    pub value_type: Option<(u32, Option<ValueTypeSpec>)>,
    /// `(line, what)` of everything else the column carries that cannot be written.
    pub unsupported: Vec<(u32, String)>,
}

/// The column records of an added table bound to the value table attribute `attr_name` (id `attr_id`). The errors are
/// the refusals, each with its line.
pub fn plan_columns(attr_name: &str, attr_id: i64, requests: &[VtColumnRequest]) -> Result<Vec<VtColumn>, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    let mut specs = Vec::new();
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    let mut fields = std::collections::HashSet::new();
    for r in requests {
        let at = format!("line {}: column '{}'", r.line, r.name);
        let before = problems.len();
        for (line, what) in &r.unsupported {
            problems.push(format!("line {line}: column '{}': {what} is not supported on a column of an added table", r.name));
        }
        if !crate::naming::is_identifier(&r.name) {
            problems.push(format!("{at}: the name must be an identifier (letters, digits and _, not starting with a digit)"));
        } else if !names.insert(r.name.clone()) {
            problems.push(format!("{at}: the name is used by another column"));
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
        if r.steps.is_some() {
            problems.push(format!("{at}: steps of <DataPath> are not supported on a column of a value table"));
        }
        let field = match r.data_path.as_deref() {
            None => r.name.clone(),
            Some(path) => {
                if r.attribute_id.as_deref().map(str::trim) != Some(attr_id.to_string().as_str()) {
                    problems.push(format!("{at}: DataPath '{path}': attributeId must be {attr_id} (the attribute {attr_name})"));
                }
                match path.split_once('.') {
                    Some((root, field)) if root == attr_name && crate::naming::is_identifier(field) => field.to_string(),
                    _ => {
                        problems.push(format!("{at}: DataPath '{path}' must be {attr_name}.<field of the value table>"));
                        r.name.clone()
                    }
                }
            }
        };
        if !fields.insert(field.clone()) {
            problems.push(format!("{at}: the field '{field}' is used by another column"));
        }
        let value = match &r.value_type {
            None => Ok(VtType::String { length: None }),
            Some((line, None)) => Err(format!(
                "line {line}: column '{}': this <ValueType> is not supported (use kind String with length, Number, Date, DateTime or Boolean; composite is true or false when given; no other attributes)",
                r.name
            )),
            Some((line, Some(spec))) => VtType::from_spec(spec).map_err(|why| format!("line {line}: column '{}': {why}", r.name)),
        };
        let value = match value {
            Ok(v) => Some(v),
            Err(why) => {
                problems.push(why);
                None
            }
        };
        if let (Some(value), true) = (value, problems.len() == before) {
            specs.push(VtColumn {
                name: r.name.clone(),
                id: r.id.unwrap_or(0),
                field,
                header: r.header.clone(),
                width: r.width,
                visible: r.visible,
                read_only: r.read_only,
                value,
            });
        }
    }
    if problems.is_empty() {
        Ok(specs)
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, id: i64, value: VtType) -> VtColumn {
        VtColumn { name: name.into(), id, field: name.into(), header: vec![("ru".into(), name.into())], width: 100, visible: true, read_only: false, value }
    }

    #[test]
    fn a_column_has_the_layout_of_the_donor_column() {
        let c = column_record(&column("Колонка1", 0, VtType::String { length: None })).unwrap();
        let w = c.node(1).unwrap();
        assert_eq!(c.atom(0).unwrap().value, VT_COLUMN_GUID);
        assert_eq!((w.len(), w.atom(2).unwrap().value.as_str(), w.atom(3).unwrap().value.as_str()), (6, "Колонка1", ""));
        let p = w.node(1).unwrap().node(1).unwrap();
        assert_eq!((p.len(), p.atom(30).unwrap().value.as_str(), p.atom(38).unwrap().value.as_str()), (46, "Колонка1", "381ed624-9217-4e63-85db-c4c3cb87daae"));
        assert_eq!(text_of(p.node(35).unwrap()), r#"{"Pattern",{"S"}}"#);
        // the donor column: the same record up to the editor blob (the donor writes 100 as 1e2)
        let donor = crate::donor::donor();
        let index = crate::form_patch::ControlIndex::build(donor).unwrap();
        let table = index.entries.iter().find(|e| e.type_name == "Table").unwrap();
        let cols = table.control.node.node(2).unwrap().node(2).unwrap().node(1).unwrap().node(23).unwrap();
        let before_blob = |t: String| t.split("{{#base64:").next().unwrap().to_string();
        let donor_col = before_blob(text_of(cols.node(1).unwrap()));
        let ours = before_blob(text_of(&c));
        assert_eq!(ours.replace(",100,0,-1", ",1e2,0,-1"), donor_col);
    }

    #[test]
    fn the_types_have_their_patterns_and_editors() {
        let ty = |kind: &str| ValueTypeSpec { kind: kind.into(), ..ValueTypeSpec::default() };
        assert_eq!(VtType::from_spec(&ty("Boolean")).unwrap().pattern(), r#"{"Pattern",{"B"}}"#);
        assert_eq!(VtType::from_spec(&ty("Date")).unwrap().pattern(), r#"{"Pattern",{"D","D"}}"#);
        assert_eq!(VtType::from_spec(&ty("DateTime")).unwrap().pattern(), r#"{"Pattern",{"D"}}"#);
        let number = ValueTypeSpec { kind: "Number".into(), digits: Some(15), fraction_digits: Some(2), ..ValueTypeSpec::default() };
        assert_eq!(VtType::from_spec(&number).unwrap().pattern(), r#"{"Pattern",{"N",15,2,0}}"#);
        let string = ValueTypeSpec { kind: "String".into(), length: Some(50), ..ValueTypeSpec::default() };
        assert_eq!(VtType::from_spec(&string).unwrap().pattern(), r#"{"Pattern",{"S",50,1}}"#);
        for kind in ["Time", "Ref", "ValueTable", "ValueList"] {
            assert!(VtType::from_spec(&ty(kind)).is_err(), "{kind}");
        }
        for value in [VtType::String { length: None }, VtType::String { length: Some(50) }, VtType::Date, VtType::DateTime, VtType::Boolean] {
            let blob = value.editor().unwrap();
            assert_eq!(crate::ts_table::blob_pattern(&blob).as_deref(), Some(value.pattern().as_str()));
        }
    }

    #[test]
    fn the_planner_names_every_refusal() {
        let req = |name: &str, id: i64| VtColumnRequest { line: 3, name: name.into(), id: Some(id), width: 100, visible: true, ..VtColumnRequest::default() };
        let mut bad_type = req("В", 2);
        bad_type.value_type = Some((4, Some(ValueTypeSpec { kind: "Ref".into(), ..ValueTypeSpec::default() })));
        let mut bad_path = req("Г", 3);
        bad_path.data_path = Some("Другая.Г".into());
        bad_path.attribute_id = Some("5".into());
        let mut bad_editor = req("Д", 4);
        bad_editor.editor = Some("CheckBox".into());
        let err = plan_columns("Таблица", 5, &[req("А", 0), req("А", 0), bad_type, bad_path, bad_editor]).unwrap_err().join("\n");
        for part in ["the name is used by another column", "the id 0 is used", "not supported for a column", "must be Таблица.<field", "only InputField"] {
            assert!(err.contains(part), "{part}: {err}");
        }
        let ok = plan_columns("Таблица", 5, &[req("А", 0), req("Б", 1)]).unwrap();
        assert_eq!((ok[0].field.as_str(), ok[1].id), ("А", 1));
    }
}
