//! Deterministic `ExtractionReport.md` (port of `report.py`).

use serde_json::Value;

use crate::canonical::{dumps_with, SORTED_SPACED};
use crate::errors::Result;

fn q(value: &str) -> Result<String> {
    dumps_with(&Value::String(value.to_string()), &SORTED_SPACED)
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn sorted_by_id(items: &[Value]) -> Vec<&Value> {
    let mut v: Vec<&Value> = items.iter().collect();
    v.sort_by(|a, b| text(&a["id"]).cmp(&text(&b["id"])));
    v
}

#[allow(clippy::too_many_arguments)]
pub fn render_report(
    epf_name: &str,
    epf_sha256: &str,
    processor: &Value,
    status: &str,
    object_module_state: &str,
    forms: &[Value],
    templates: &[Value],
    diagnostics: &[Value],
) -> Result<Vec<u8>> {
    let mut warnings: Vec<&Value> = diagnostics.iter().filter(|d| d["severity"] == "warning").collect();
    let mut lines: Vec<String> = vec![
        "# Extraction report".into(),
        String::new(),
        format!("- Status: `{status}`"),
        "- Code complete: yes".into(),
        format!("- Source: {} (sha256 `{epf_sha256}`)", q(epf_name)?),
        format!("- Processor: {} (`{}`)", q(&text(&processor["name"]))?, text(&processor["uuid"])),
        String::new(),
        "## Counts".into(),
        String::new(),
        format!("- Object module: {object_module_state}"),
        format!("- Forms: {}", forms.len()),
        format!("- Templates: {}", templates.len()),
        format!("- Warnings: {}", warnings.len()),
        String::new(),
        "## Forms".into(),
        String::new(),
    ];
    for f in sorted_by_id(forms) {
        lines.push(format!(
            "- `{}` {} (`{}`): module {}, layout {} ({})",
            text(&f["alias"]),
            q(&text(&f["originalName"]))?,
            text(&f["id"]),
            text(&f["module"]["state"]),
            text(&f["layout"]["status"]),
            text(&f["layout"]["backend"]),
        ));
    }
    lines.extend([String::new(), "## Templates".into(), String::new()]);
    for t in sorted_by_id(templates) {
        lines.push(format!(
            "- `{}` {} (`{}`): {}, {}",
            text(&t["alias"]),
            q(&text(&t["originalName"]))?,
            text(&t["id"]),
            text(&t["templateType"]),
            text(&t["status"]),
        ));
    }
    lines.extend([String::new(), "## Warnings".into(), String::new()]);
    if warnings.is_empty() {
        lines.push("- none".into());
    }
    warnings.sort_by_key(|d| (text(&d["component"]), text(&d["code"])));
    for d in warnings {
        lines.push(format!(
            "- `{}` in `{}`: {}",
            text(&d["code"]),
            text(&d["component"]),
            dumps_with(&d["params"], &SORTED_SPACED)?
        ));
    }
    lines.extend([
        String::new(),
        "## Notes".into(),
        String::new(),
        "- Form layouts are reference-only: the basic backend does not decode controls, attributes or events. \
         The raw form stream is kept as binary evidence."
            .into(),
        "- The EPF is the single source of truth. `src/` is a derived projection for review; \
         change the processor, save the EPF and extract again."
            .into(),
        String::new(),
    ]);
    Ok(lines.join("\n").into_bytes())
}
