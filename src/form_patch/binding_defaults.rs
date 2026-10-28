//! Selection buttons of an added `InputField` bound to an attribute of the object.
//!
//! The donor prototype of an `InputField` has no buttons. The Configurator gives a field bound to a reference
//! attribute the choice and open buttons, a date field the choice button (the calendar); a number, string or
//! boolean field has none (as the donor). A button the XML does not mention takes that default; a mentioned one always wins.
//!
//! The same field has its own `<ValueType>` in the Configurator: the type of the bound data (string with its length,
//! number with digits, date, boolean, reference). The donor prototype carries a `ValueList` there; a `<ValueType>` the
//! XML does not state is taken from the schema (a single known type; a composite or unknown one stays as the donor has it).

use super::{FormAttribute, FormXml, ValueTypeSpec};
use crate::brace::{parse_metadata, Item};
use crate::object_schema::{ObjectSchema, Resolved};

/// `(ChoiceButton, OpenButton)` of a field bound to a value of `kind` (see [`ObjectSchema::attr_kinds`]);
/// `None`: the kind has no default (the buttons stay as the donor has them).
fn buttons_of(kind: &str) -> Option<(bool, bool)> {
    match kind {
        "CatalogRef" | "DocumentRef" => Some((true, true)),
        "EnumRef" | "ChartOfAccountsRef" | "Date" => Some((true, false)),
        _ => None,
    }
}

/// Kind of the value `Root.Name` stands for: an attribute of the object or the standard attribute `Дата`.
fn value_kind<'a>(schema: &'a ObjectSchema, name: &str) -> Option<&'a str> {
    match schema.resolve_attr(name) {
        Resolved::One(_) => schema.attr_kind(name),
        Resolved::Missing => (name == "Дата" && schema.std_code(name).is_some()).then_some("Date"),
        Resolved::Ambiguous => None,
    }
}

/// The own value type of a field bound to `Root.name`: the pattern of the attribute, or of the standard attribute.
fn own_value_type(schema: &ObjectSchema, name: &str) -> Option<ValueTypeSpec> {
    let pattern = match schema.resolve_attr(name) {
        Resolved::One(_) => schema.attr_pattern(name),
        Resolved::Missing => schema.std_pattern(name),
        Resolved::Ambiguous => None,
    }?;
    let spec = super::value_type_of_pattern(&Item::Node(parse_metadata(pattern.as_bytes()).ok()?))?;
    (!spec.composite && !spec.lossy).then_some(spec)
}

/// The value type of a field bound to the plain attribute `a` of the form: the attribute's own simple type (string with
/// its length, number with digits, date, boolean, reference). A composite attribute or a table keeps the donor's type.
fn form_attribute_type(a: &FormAttribute) -> Option<ValueTypeSpec> {
    let known = matches!(a.kind.as_str(), "String" | "Number" | "Date" | "DateTime" | "Boolean") || a.kind.ends_with("Ref");
    (known && !a.composite && !a.main).then(|| ValueTypeSpec {
        kind: a.kind.clone(),
        composite: false,
        fraction_digits: a.fraction_digits,
        digits: a.digits,
        length: a.length,
        non_negative: a.non_negative,
        // the pattern of the field repeats the one of the attribute, `{"S",length,1}` included
        fixed: a.fixed,
        ref_guids: a.ref_guids.clone(),
        lossy: false,
    })
}

/// Applies the defaults to the `InputField`s of `edited` that are not in `baseline` and are bound to an attribute of
/// the object or of the form, for the buttons and the `<ValueType>` their XML does not mention. A path that cannot be
/// resolved is left to the planner, which refuses it.
///
/// A field of a known simple type is not a list or a spin field: the donor's drop-down and spin buttons (it carries a
/// `ValueList` type) are off unless the XML says otherwise.
pub(super) fn apply(edited: &mut FormXml, baseline: &FormXml) {
    let schema = crate::object_schema::object_schema();
    let attributes = edited.attributes.clone();
    for c in &mut edited.controls {
        if c.type_name != "InputField" || baseline.control(c.id).is_some() {
            continue;
        }
        let Some((attr_id, path)) = c.values.data_path.clone() else { continue };
        let parts: Vec<&str> = path.split('.').collect();
        match (parts.as_slice(), &schema) {
            ([_, name], Some(schema)) => {
                if let Some((choice, open)) = value_kind(schema, name).and_then(buttons_of) {
                    let [has_choice, has_open] = c.explicit_buttons;
                    if !has_choice {
                        c.values.choice_button = Some(choice);
                    }
                    if !has_open {
                        c.values.open_button = Some(open);
                    }
                }
                if !c.explicit_value_type {
                    if let Some(spec) = own_value_type(schema, name) {
                        c.values.value_type = Some(spec);
                    }
                }
            }
            ([name], _) if !c.explicit_value_type => {
                let attribute = attributes.iter().find(|a| a.id == attr_id && a.name == *name);
                if let Some(spec) = attribute.and_then(form_attribute_type) {
                    c.values.value_type = Some(spec);
                }
            }
            _ => {}
        }
        if c.values.value_type.as_ref().is_some_and(|vt| !vt.composite && !vt.kind.is_empty() && vt.kind != "ValueList") {
            c.values.drop_list_button.get_or_insert(false);
            c.values.spin_button.get_or_insert(false);
        }
    }
}

/// Added fields bound to an attribute of the object whose own `<ValueType>` was not stated and could not be taken from
/// the schema (no object schema, or no single known type): they keep the donor's `ValueList` type. `diff` is the diff
/// of the plan, whose additions are the fields after [`apply`].
pub fn unresolved_type_warnings(diff: &super::FormDiff) -> Vec<String> {
    let mut out = Vec::new();
    for c in &diff.additions {
        let Some((_, path)) = &c.values.data_path else { continue };
        if c.type_name != "InputField" || c.explicit_value_type || c.values.value_type.is_some() || path.split('.').count() != 2 {
            continue;
        }
        out.push(format!(
            "field '{}' is bound to '{path}' but its value type could not be resolved; it keeps the template type (ValueList) — give <ValueType kind=... refGuid=...> explicitly or run from a directory next to the configuration export",
            c.name
        ));
    }
    out
}
