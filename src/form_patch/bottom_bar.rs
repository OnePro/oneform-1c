//! The bottom command bar of an ordinary form, as the Configurator creates it.
//!
//! An added `CommandBar` that sits at the bottom of the form (or of a page of the form itself) gets the border
//! `ЧертаСверху` (code 7; the donor record carries 4, the top bar) and, for what its XML omits, `HorizontalAlign`
//! `Right`, `AutoFill` `false` and the anchors to the bottom and both sides. Explicit values are kept; a placement that
//! differs from the Designer's is reported.

use super::{bare, nav_mut, AnchorEdge, AnchorsSpec, ControlXml, FormXml, Result, XmlElement};
use crate::brace::Node;

/// Name of the bottom bar in the Configurator.
const DESIGNER_NAME: &str = "ОсновныеДействияФормы";
/// Height of the bar in the Configurator.
const DESIGNER_HEIGHT: i64 = 25;
/// How far the bottom edge of a bar may be from the bottom of its container and still make it the bottom bar.
const TOLERANCE: i64 = 10;
/// `ЧертаСверху`.
const BORDER_TOP_LINE: i64 = 7;

/// Writes the border `ЧертаСверху` into the info node of a cloned bar.
pub(super) fn write_border(info: &mut Node) -> Result<()> {
    nav_mut(info, &[1, 0, 11])?.children[3] = bare(BORDER_TOP_LINE);
    Ok(())
}

/// `<Width>` / `<Height>` of the `Page` named `name` that is a direct child of the form.
fn page_size(shell: &XmlElement, name: &str) -> (Option<i64>, Option<i64>) {
    let page = shell
        .children
        .iter()
        .filter(|e| e.tag == "ChildItems")
        .flat_map(|e| &e.children)
        .find(|e| e.tag == "Page" && e.attrs.get("name").map(String::as_str) == Some(name));
    let num = |tag: &str| page.and_then(|p| p.children.iter().find(|e| e.tag == tag)).and_then(|e| e.text.trim().parse().ok());
    (num("Width"), num("Height"))
}

/// `(width, height)` of the form, or of the page of the form that holds the control; `None` for a control of a `Panel`
/// (its parent is the panel) or of a nested container.
fn container(form: &FormXml, c: &ControlXml) -> Option<(Option<i64>, Option<i64>)> {
    if c.parent.is_some() {
        return None;
    }
    Some(match &c.page {
        None => (form.form_width, form.form_height),
        Some(name) => page_size(&form.shell, name),
    })
}

fn designer_anchors(id: i64, height: i64) -> AnchorsSpec {
    let edge = |side, target, offset| Some(AnchorEdge { target, side, offset });
    AnchorsSpec { top: edge(1, id, -height), bottom: edge(1, 0, 0), left: edge(2, 0, 0), right: edge(3, 0, 0), alt: [None; 4] }
}

fn anchors_xml(a: &AnchorsSpec) -> String {
    let side = |s: i64| match s {
        0 => "Top",
        1 => "Bottom",
        2 => "Left",
        _ => "Right",
    };
    let mut out = String::from("<Anchors");
    for (name, edge) in [("top", a.top), ("bottom", a.bottom), ("left", a.left), ("right", a.right)] {
        if let Some(e) = edge {
            out.push_str(&format!(" {name}=\"{}\"", side(e.side)));
            if e.target != 0 {
                out.push_str(&format!(" {name}Target=\"{}\"", e.target));
            }
            if e.offset != 0 {
                out.push_str(&format!(" {name}Offset=\"{}\"", e.offset));
            }
        }
    }
    out.push_str("/>");
    out
}

/// Marks the added bottom bars of `edited`, completes what their XML omits and returns the note and the deviation
/// warning of each bar (complete output lines).
pub(super) fn apply(edited: &mut FormXml, baseline: &FormXml) -> Vec<String> {
    let mut lines = Vec::new();
    let form = edited.clone();
    for c in &mut edited.controls {
        if c.type_name != "CommandBar" || baseline.control(c.id).is_some() {
            continue;
        }
        let Some((width, height)) = container(&form, c) else { continue };
        let v = &c.values;
        let at_bottom = height.is_some_and(|h| (v.top + v.height - h).abs() <= TOLERANCE);
        if c.name != DESIGNER_NAME && !at_bottom {
            continue;
        }
        c.bottom_bar = true;
        let mut set = Vec::new();
        if !c.explicit_bar[0] {
            c.values.horizontal_align = "Right".to_string();
            set.push("HorizontalAlign Right".to_string());
        }
        if !c.explicit_bar[1] {
            c.values.auto_fill = Some(false);
            set.push("AutoFill false".to_string());
        }
        let wanted = designer_anchors(c.id, c.values.height);
        if !c.explicit_bar[2] {
            c.values.anchors = Some(wanted.clone());
            set.push(format!("anchors {}", anchors_xml(&wanted)));
        }
        set.push("border ЧертаСверху".to_string());
        lines.push(format!("  note {{path}}: bottom command bar '{}' gets the Designer defaults: {}", c.name, set.join(", ")));

        let v = &c.values;
        let mut off = Vec::new();
        if v.left != 0 {
            off.push(format!("Left {} -> 0", v.left));
        }
        if let Some(w) = width.filter(|w| *w != v.width) {
            off.push(format!("Width {} -> {w}", v.width));
        }
        if v.height != DESIGNER_HEIGHT {
            off.push(format!("Height {} -> {DESIGNER_HEIGHT}", v.height));
        }
        if let Some(top) = height.map(|h| h - v.height).filter(|t| *t != v.top) {
            off.push(format!("Top {} -> {top}", v.top));
        }
        if c.explicit_bar[2] && v.anchors.as_ref().is_some_and(|a| *a != wanted) {
            off.push(format!("anchors -> {}", anchors_xml(&wanted)));
        }
        if c.explicit_bar[0] && v.horizontal_align != "Right" {
            let was = if v.horizontal_align.is_empty() { "Left" } else { v.horizontal_align.as_str() };
            off.push(format!("HorizontalAlign {was} -> Right"));
        }
        if !off.is_empty() {
            lines.push(format!(
                "  warning {{path}}: bottom command bar '{}' is not placed like the Designer's: {}",
                c.name,
                off.join(", ")
            ));
        }
    }
    lines
}
