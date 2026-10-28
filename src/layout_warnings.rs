//! Advisory findings of a planned form edit that never block `pack`: references in the code to controls and pages
//! that the edit removes or renames, and added or moved controls that stick out of their container.
//!
//! The scan of the module is textual and simple: it skips `//` comments and string literals (a string is read only for
//! the `ЭлементыФормы["Имя"]` form), so it can miss a dynamic reference and cannot see through variables.

use std::collections::HashSet;

use regex::Regex;

use crate::form_patch::{CommandBarButton, ControlXml, FormDiff, FormXml, Property};

/// References listed per control before `+N more`.
const MAX_LINES: usize = 20;

/// Event handler suffixes of ordinary form controls: `<Control><Suffix>` is the usual name of a handler.
const HANDLER_SUFFIXES: [&str; 24] = [
    "ПриИзменении",
    "НачалоВыбора",
    "Выбор",
    "ОкончаниеВводаТекста",
    "Очистка",
    "Открытие",
    "Регулирование",
    "Нажатие",
    "ПриАктивизации",
    "ПриАктивизацииСтроки",
    "ПриАктивизацииКолонки",
    "ПриВыводеСтроки",
    "ПередНачаломДобавления",
    "ПередНачаломИзменения",
    "ПередУдалением",
    "ПриНачалеРедактирования",
    "ПриОкончанииРедактирования",
    "ПередОкончаниемРедактирования",
    "ОбработкаВыбора",
    "Перетаскивание",
    "НачалоПеретаскивания",
    "ПроверкаПеретаскивания",
    "ПриПолученииДанных",
    "ПриВыбореСтраницы",
];

/// The name of a control or page that does not exist any more under that name.
struct Gone {
    name: String,
    /// `removed control`, `removed page`, `renamed control`, `renamed page`
    what: String,
    /// New name of a renamed one.
    now: Option<String>,
    /// Exact handler procedures of a removed control.
    handlers: Vec<String>,
}

fn kind_of(c: &ControlXml) -> &'static str {
    if c.type_name == "Page" {
        "page"
    } else {
        "control"
    }
}

fn gone_names(diff: &FormDiff, requested: &FormXml) -> Vec<Gone> {
    let alive: HashSet<String> = requested.controls.iter().map(|c| c.name.to_lowercase()).collect();
    let mut out = Vec::new();
    for c in &diff.deletions {
        if !alive.contains(&c.name.to_lowercase()) {
            out.push(Gone {
                name: c.name.clone(),
                what: format!("removed {}", kind_of(c)),
                now: None,
                handlers: c.values.events.iter().map(|(_, p)| p.clone()).filter(|p| !p.is_empty()).collect(),
            });
        }
    }
    for e in &diff.edits {
        if e.changed.contains(&Property::Name) && e.old.name != e.new.name && !alive.contains(&e.old.name.to_lowercase()) {
            let what = if e.type_name == "Page" { "renamed page" } else { "renamed control" };
            out.push(Gone { name: e.old.name.clone(), what: what.to_string(), now: Some(e.new.name.clone()), handlers: Vec::new() });
        }
    }
    out
}

/// One line of code in two views: with the string literals kept (comment cut off), and with their contents blanked.
/// `in_string` carries an open multi-line literal (its continuation lines start with `|`) to the next line.
fn code_views(line: &str, in_string: &mut bool) -> (String, String) {
    let mut kept = String::new();
    let mut blank = String::new();
    let mut chars: Vec<char> = line.chars().collect();
    if *in_string {
        if let Some(at) = chars.iter().position(|c| !c.is_whitespace()).filter(|&at| chars[at] == '|') {
            chars.drain(..=at);
        } else {
            *in_string = false;
        }
    }
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if *in_string {
            if c == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    kept.push_str("\"\"");
                    blank.push_str("  ");
                    i += 2;
                    continue;
                }
                *in_string = false;
                kept.push('"');
                blank.push('"');
            } else {
                kept.push(c);
                blank.push(' ');
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            break;
        } else {
            if c == '"' {
                *in_string = true;
            }
            kept.push(c);
            blank.push(c);
        }
        i += 1;
    }
    (kept, blank)
}

const IDENT: &str = r"\p{L}\p{N}_";

/// `(dot form, bracket form)` of the references to the element `name`.
fn reference_regexes(name: &str) -> Option<(Regex, Regex)> {
    let n = regex::escape(name);
    let dot = format!(
        r"(?i)(?:(?:^|[^{IDENT}])элементыформы|(?:^|[^{IDENT}.])элементы|(?:^|[^{IDENT}])(?:страницы|pages))\s*\.\s*{n}(?:[^{IDENT}]|$)"
    );
    let bracket = format!(r#"(?i)(?:(?:^|[^{IDENT}])(?:элементыформы|страницы|pages)|(?:^|[^{IDENT}.])элементы)\s*\[\s*"{n}"\s*\]"#);
    Some((Regex::new(&dot).ok()?, Regex::new(&bracket).ok()?))
}

/// Procedures that a remaining control or the form still calls (lower case).
fn handlers_used_elsewhere(requested: &FormXml) -> HashSet<String> {
    fn buttons(b: &[CommandBarButton], out: &mut HashSet<String>) {
        for x in b {
            out.insert(x.action.to_lowercase());
            buttons(&x.buttons, out);
        }
    }
    let mut out = HashSet::new();
    for (_, p) in &requested.form_events {
        out.insert(p.to_lowercase());
    }
    for c in &requested.controls {
        for (_, p) in &c.values.events {
            out.insert(p.to_lowercase());
        }
        buttons(&c.values.buttons, &mut out);
    }
    out
}

/// Warnings (text without prefix) for the module `label` (`text` is its content): every reference to a removed or
/// renamed control and page, and, when `orphans` is set, every handler procedure of a removed control that is still
/// declared there.
pub fn module_warnings(diff: &FormDiff, requested: &FormXml, label: &str, text: &str, orphans: bool) -> Vec<String> {
    let gone = gone_names(diff, requested);
    if gone.is_empty() {
        return Vec::new();
    }
    let used = handlers_used_elsewhere(requested);
    let decl = Regex::new(&format!(r"(?i)^\s*(?:процедура|функция|procedure|function)\s+([{IDENT}]+)\s*\(")).unwrap();
    // (line number, trimmed line, view with strings, view without string contents)
    let mut lines: Vec<(usize, String, String, String)> = Vec::new();
    let mut in_string = false;
    for (i, raw) in text.lines().enumerate() {
        let (kept, blank) = code_views(raw, &mut in_string);
        lines.push((i + 1, raw.trim().to_string(), kept, blank));
    }
    let mut out = Vec::new();
    for g in &gone {
        let Some((dot, bracket)) = reference_regexes(&g.name) else { continue };
        let hits: Vec<&(usize, String, String, String)> =
            lines.iter().filter(|(_, _, kept, blank)| dot.is_match(blank) || bracket.is_match(kept)).collect();
        let now = g.now.as_ref().map(|n| format!(" (now '{n}')")).unwrap_or_default();
        for (line, raw, _, _) in hits.iter().take(MAX_LINES) {
            out.push(format!("{} '{}'{now} is still referenced at {label}:{line}: {raw}", g.what, g.name));
        }
        if hits.len() > MAX_LINES {
            out.push(format!("{} '{}'{now} is referenced at {label} in +{} more lines", g.what, g.name, hits.len() - MAX_LINES));
        }
        if !orphans || g.now.is_some() {
            continue;
        }
        let name_lc = g.name.to_lowercase();
        for (line, _, _, blank) in &lines {
            let Some(p) = decl.captures(blank).map(|c| c[1].to_string()) else { continue };
            let p_lc = p.to_lowercase();
            let exact = g.handlers.iter().any(|h| h.to_lowercase() == p_lc);
            let by_name =
                p_lc.strip_prefix(&name_lc).is_some_and(|rest| HANDLER_SUFFIXES.iter().any(|s| s.to_lowercase() == rest));
            if (exact || by_name) && !used.contains(&p_lc) {
                out.push(format!("procedure {p} belongs to removed control '{}' ({label}:{line}): remove it, it is an orphan", g.name));
            }
        }
    }
    out
}

/// Added controls and controls whose geometry the edit changed that the layout lint finds beyond their form or page.
pub fn container_warnings(diff: &FormDiff, edited_xml: &str) -> Vec<String> {
    let geometry = [Property::Left, Property::Top, Property::Width, Property::Height];
    let mut names: HashSet<&str> = diff.additions.iter().map(|c| c.name.as_str()).collect();
    for e in &diff.edits {
        if e.changed.iter().any(|p| geometry.contains(p)) {
            names.insert(e.new.name.as_str());
        }
    }
    if names.is_empty() {
        return Vec::new();
    }
    let (_, findings) = crate::lint::lint_text(edited_xml, "", "");
    let mut out = Vec::new();
    for f in findings {
        if f.rule == "outside-container" && names.contains(f.control.as_str()) {
            out.push(format!("control '{}' extends beyond its container ({}): {}", f.control, f.message, f.fix));
        }
    }
    out
}
