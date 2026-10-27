//! Selection buttons of an added `InputField` bound to an attribute of the object.
//!
//! The donor prototype of an `InputField` has no buttons. The Configurator gives a field bound to a reference
//! attribute the choice and open buttons, a date field the choice button (the calendar); a number, string or
//! boolean field has none (as the donor). A button the XML does not mention takes that default; a mentioned one always wins.

use super::FormXml;
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

/// Applies the defaults to the `InputField`s of `edited` that are not in `baseline` and are bound to an attribute of
/// the object, for the buttons their XML does not mention. A path that cannot be resolved is left to the planner,
/// which refuses it.
pub(super) fn apply(edited: &mut FormXml, baseline: &FormXml) {
    let Some(schema) = crate::object_schema::object_schema() else { return };
    for c in &mut edited.controls {
        if c.type_name != "InputField" || baseline.control(c.id).is_some() {
            continue;
        }
        let Some((_, path)) = &c.values.data_path else { continue };
        let parts: Vec<&str> = path.split('.').collect();
        let [_, name] = parts.as_slice() else { continue };
        let Some((choice, open)) = value_kind(&schema, name).and_then(buttons_of) else { continue };
        let [has_choice, has_open] = c.explicit_buttons;
        if !has_choice {
            c.values.choice_button = Some(choice);
        }
        if !has_open {
            c.values.open_button = Some(open);
        }
    }
}
