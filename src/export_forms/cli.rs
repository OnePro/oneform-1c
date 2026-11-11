//! `oneform export-forms <DUMP> --out <DIR> [--check] [-j N] [--json]`: the command line side of the export.
//!
//! The command reads the dump and writes into `--out` only. It never looks at `.oneform`, history, diagnostics,
//! `EPF_OOF_PATH`, `ONEFORM_CARRIERS` or `.oneform.json`, and it does not touch the process-wide projection switches.

use std::path::Path;

use serde_json::{json, Value};

use super::{apply_export, check_export, check_out_location, plan_export, DifferenceKind};
use crate::canonical::{dumps_with, JsonOpts};
use crate::cli_epf::{EXIT_DIFFERS, EXIT_FAILED, EXIT_OK, EXIT_USAGE};

const PRETTY_SORTED: JsonOpts = JsonOpts { indent: Some(2), sort_keys: true, ensure_ascii: false, compact: false };

fn usage_error(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_USAGE
}

fn kind_label(kind: DifferenceKind) -> (&'static str, &'static str) {
    match kind {
        DifferenceKind::Added => ("ADDED", "added"),
        DifferenceKind::Changed => ("CHANGED", "changed"),
        DifferenceKind::Removed => ("REMOVED", "removed"),
    }
}

fn print_json(v: &Value) {
    println!("{}", dumps_with(v, &PRETTY_SORTED).unwrap_or_default());
}

/// Runs the command; the result is the process exit code: 0 done, 1 a form could not be read or an I/O error, 2 usage or
/// unsafe paths, 3 `--check` found differences.
pub fn run_cli(dump: &str, out: &str, check: bool, json_output: bool, workers: usize) -> i32 {
    let dump_path = Path::new(dump);
    let out_path = Path::new(out);
    if !dump_path.is_dir() {
        return usage_error(&format!("{dump} is not a directory"));
    }
    if let Err(refusal) = check_out_location(dump_path, out_path) {
        return usage_error(&refusal.message);
    }
    let plan = plan_export(dump_path, workers);
    if plan.form_count == 0 {
        return usage_error(&format!("no ordinary forms under {dump}"));
    }
    let version = env!("CARGO_PKG_VERSION");
    let report = &plan.report;
    let mut written: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();
    let mut unchanged = 0usize;
    let mut differences = Vec::new();
    if check {
        differences = check_export(out_path, &plan, version);
    } else {
        match apply_export(out_path, &plan, version) {
            Ok(done) => {
                written = done.written;
                removed = done.removed;
                unchanged = done.unchanged;
            }
            Err(e) => {
                eprintln!("oneform: error: {e}");
                return EXIT_FAILED;
            }
        }
        written.sort();
        removed.sort();
    }
    // Failures win over differences: a run that could not read a form is a failure first, whatever --check found.
    let code = if !report.failures.is_empty() {
        EXIT_FAILED
    } else if check && !differences.is_empty() {
        EXIT_DIFFERS
    } else {
        EXIT_OK
    };
    if json_output {
        let mut doc = json!({
            "dump": dump,
            "out": out,
            "forms": plan.form_count,
            "written": written,
            "removed": removed,
            "unchanged": unchanged,
            "withoutProjection": report.without_projection.iter().map(|w| json!({"path": w.path, "reason": w.reason})).collect::<Vec<_>>(),
            "failures": report.failures.iter().map(|f| json!({"path": f.path, "code": f.code, "message": f.message})).collect::<Vec<_>>(),
            "check": check,
            "exitCode": code,
        });
        if check {
            doc["differences"] = Value::Array(differences.iter().map(|d| json!({"path": d.path, "kind": kind_label(d.kind).1})).collect());
        }
        print_json(&doc);
        return code;
    }
    for f in &report.failures {
        eprintln!("oneform: failed: {}: {}: {}", f.path, f.code, f.message);
    }
    for d in &differences {
        println!("{} {}", kind_label(d.kind).0, d.path);
    }
    for w in &report.without_projection {
        println!("without Form.xml: {} ({})", w.path, w.reason);
    }
    let (forms, none, failed) = (plan.form_count, report.without_projection.len(), report.failures.len());
    if check {
        println!("export-forms --check: {forms} forms, {} differences, {none} without Form.xml, {failed} failed", differences.len());
    } else {
        println!(
            "export-forms: {forms} forms, {} written, {} removed, {unchanged} unchanged, {none} without Form.xml, {failed} failed",
            written.len(),
            removed.len()
        );
    }
    code
}
