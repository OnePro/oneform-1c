//! `oneform validate`: is an ordinary form inside the scope that `pack` can edit safely?
//!
//! A form is `IN_SCOPE` iff it is an ordinary 8.2/8.3 form (stream root version 24..=27), has no control kinds
//! without a donor prototype (ActiveX, unknown GUIDs), has unique control names, round-trips byte for byte when it is
//! packed without edits and its projection (`src/`) is current. Every failed condition is a reason with a stable code
//! and a hint. The command works on the binary sources (`Ext/Form.bin`, `.epf`) in memory and writes nothing: no
//! `src/`, no lock files, no viewer.
//!
//! The unedited round trip is the real pack pipeline without an edit: the stream is projected to `Form.xml` (the same
//! staging `extract` and `check` use), the XML is parsed back, indexed against the stream, validated and diffed with
//! itself (`plan_form_patch`), the no-change plan is applied and the resulting bytes are compared with the original
//! stream; the form container is also rebuilt as `pack` does and its entries are compared.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::brace::{parse_spanned, Item, SpannedMetadata};
use crate::cli_epf::{clean_path, EXIT_FAILED, EXIT_OK, EXIT_USAGE};
use crate::cli_oneform::{discover_targets, falsy, path_of, root_config, zero_config, Kind, Target, DEFAULT_TOOLS_LOCK};
use crate::config::{effective_config, load_json_bytes};
use crate::container::{inflate, parse_container, serialize_container, EntryOut, Limits};
use crate::discovery::CONFIG_NAME;
use crate::donor;
use crate::errors::{ExtractionError, Result, FORM_XML_EXTRACTION_FAILED};
use crate::form_patch::{parse_form_xml, plan_form_patch, with_type_lookup, ControlIndex, UNKNOWN_CONTROL};
use crate::formbin::{
    form_bin_rel, object_dir, read_object_schema_with, src_dir, stage_config_object, ConfigObject, Sources,
};
use crate::manifest::MANIFEST_PATH;
use crate::metadata::discover;
use crate::metaref::ConfigTypeLookup;
use crate::object_schema::with_object_schema;
use crate::oof::CONTROL_GUIDS;
use crate::stage::{stage_processor, StageOpts};
use crate::transaction::{read_tree, Tree};

pub const FORMAT_80: &str = "format-8.0";
pub const METADATA_LAYOUT: &str = "metadata-layout";
pub const UNSUPPORTED_CONTROL: &str = "unsupported-control";
pub const DUPLICATE_NAME: &str = "duplicate-name";
pub const ROUND_TRIP_DIFFERS: &str = "round-trip-differs";
pub const STALE_PROJECTION: &str = "stale-projection";
pub const UNREADABLE: &str = "unreadable";

/// All reason codes, in the order of the summary.
pub const REASON_CODES: [&str; 7] =
    [FORMAT_80, METADATA_LAYOUT, UNSUPPORTED_CONTROL, DUPLICATE_NAME, ROUND_TRIP_DIFFERS, STALE_PROJECTION, UNREADABLE];

const CONFIGURATOR: &str = "edit this form in the Configurator";
/// Oldest stream root version of the 8.2/8.3 format the decoder and the patcher know.
const ROOT_VERSION_MIN: i64 = 24;
const ROOT_VERSION_MAX: i64 = 27;
const NAMES_SHOWN: usize = 3;
const MESSAGE_MAX: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub code: &'static str,
    pub hint: String,
}

fn reason(code: &'static str, hint: String) -> Reason {
    Reason { code, hint }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Object directory (configuration dump) or `.epf` file, relative to the validated root.
    pub object: String,
    pub form: String,
    pub reasons: Vec<Reason>,
}

impl Verdict {
    pub fn in_scope(&self) -> bool {
        // A stale projection is not a scope problem: `extract` refreshes it.
        self.reasons.iter().all(|r| r.code == STALE_PROJECTION)
    }
}

fn short(message: &str) -> String {
    let one_line = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= MESSAGE_MAX {
        one_line
    } else {
        let cut: String = one_line.chars().take(MESSAGE_MAX).collect();
        format!("{cut}...")
    }
}

// --------------------------------------------------------------------------- one stream

/// Control kinds that the donor can clone (the other kinds cannot be added, and a form that has them is not edited).
fn supported_kinds() -> &'static BTreeSet<&'static str> {
    static KINDS: OnceLock<BTreeSet<&'static str>> = OnceLock::new();
    KINDS.get_or_init(|| CONTROL_GUIDS.iter().map(|(_, n)| *n).filter(|n| donor::prototype(n).is_ok()).collect())
}

fn root_version(source: &SpannedMetadata) -> Option<i64> {
    match source.root.children.first() {
        Some(Item::Atom(a)) => a.value.trim().parse().ok(),
        _ => None,
    }
}

fn round_trip(source: &SpannedMetadata, stream: &[u8], xml: &[u8]) -> std::result::Result<(), String> {
    let plan = plan_form_patch(source, xml, xml).map_err(|e| short(&e.message))?;
    if !plan.is_empty() {
        return Err("the unedited Form.xml yields edits against itself".to_string());
    }
    let patched = plan.apply().map_err(|e| short(&e.message))?;
    if patched.bytes != stream {
        return Err(format!("the repacked stream ({} bytes) differs from the original ({} bytes)", patched.bytes.len(), stream.len()));
    }
    Ok(())
}

/// Reasons of one form stream. `xml` is its `Form.xml` projection (`None` when the projection could not be built:
/// `why_no_xml` says why).
pub fn judge_stream(stream: &[u8], xml: Option<&[u8]>, why_no_xml: &str) -> Vec<Reason> {
    let source = match parse_spanned(stream) {
        Ok(s) => s,
        Err(e) => return vec![reason(METADATA_LAYOUT, format!("the stream does not parse: {}; {CONFIGURATOR}", short(&e.message)))],
    };
    match root_version(&source) {
        Some(v) if v < ROOT_VERSION_MIN => {
            return vec![reason(
                FORMAT_80,
                format!("the stream is in the platform 8.0 format (root version {v}); {CONFIGURATOR} (only the module is available)"),
            )]
        }
        Some(v) if v <= ROOT_VERSION_MAX => {}
        other => {
            let v = other.map_or("?".to_string(), |v| v.to_string());
            return vec![reason(METADATA_LAYOUT, format!("unknown stream root version {v}; {CONFIGURATOR}"))];
        }
    }
    let index = match ControlIndex::build(&source) {
        Ok(i) => i,
        Err(e) => return vec![reason(METADATA_LAYOUT, format!("{}; {CONFIGURATOR}", short(&e.message)))],
    };
    let mut out = Vec::new();

    let mut kinds: BTreeSet<String> = BTreeSet::new();
    for e in &index.entries {
        if e.type_name == UNKNOWN_CONTROL {
            kinds.insert(format!("unknown kind {}", e.guid));
        } else if !supported_kinds().contains(e.type_name) {
            kinds.insert(e.type_name.to_string());
        }
    }
    if !kinds.is_empty() {
        out.push(reason(
            UNSUPPORTED_CONTROL,
            format!("unsupported control kinds: {}; {CONFIGURATOR}", kinds.into_iter().collect::<Vec<_>>().join(", ")),
        ));
    }

    let mut count: HashMap<&str, usize> = HashMap::new();
    for e in index.entries.iter().filter(|e| !e.name.is_empty()) {
        *count.entry(e.name.as_str()).or_default() += 1;
    }
    let mut dups: Vec<&str> = count.iter().filter(|(_, n)| **n > 1).map(|(k, _)| *k).collect();
    dups.sort_unstable();
    if !dups.is_empty() {
        let shown = dups.iter().take(NAMES_SHOWN).map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ");
        let more = if dups.len() > NAMES_SHOWN { format!(" and {} more", dups.len() - NAMES_SHOWN) } else { String::new() };
        out.push(reason(DUPLICATE_NAME, format!("control names used more than once: {shown}{more}; give the controls unique names in the Configurator")));
    }

    let Some(xml) = xml else {
        out.push(reason(METADATA_LAYOUT, format!("Form.xml cannot be built from the stream ({}); {CONFIGURATOR}", short(why_no_xml))));
        return out;
    };
    // Duplicate names make the planner refuse the form by itself; do not report the same cause twice.
    if dups.is_empty() {
        if let Err(why) = round_trip(&source, stream, xml) {
            out.push(reason(ROUND_TRIP_DIFFERS, format!("{why}; {CONFIGURATOR}")));
        }
    }
    out
}

// --------------------------------------------------------------------------- staleness of src/

/// Why the projection `existing` (the `src/` tree) of the form `alias` is out of date, if it is. Pending edits of
/// `Form.xml` or `FormModule.bsl` are not staleness; a missing `src/` is not either (`extract` creates it).
fn stale_reason(existing: &Tree, current: &Value, rec: &Value, alias: &str, config: bool) -> Option<String> {
    let old: Value = serde_json::from_slice(existing.get(MANIFEST_PATH)?).ok()?;
    let moved = if config {
        old["source"]["forms"].get(alias) != current["source"]["forms"].get(alias)
    } else {
        old["source"].get("sha256") != current["source"].get("sha256")
    };
    if moved {
        let what = if config { "Form.bin" } else { "the EPF" };
        return Some(format!("src/ was extracted from another version of {what}"));
    }
    let xml_path = rec["layout"]["xml"].as_str()?;
    let Some(bytes) = existing.get(xml_path) else {
        return Some(format!("src/{xml_path} is missing"));
    };
    match parse_form_xml(bytes, "src") {
        Err(e) if e.params.get("kind").and_then(Value::as_str) == Some("stale-form-xml") => {
            Some(format!("src/{xml_path} was extracted by an older oneform"))
        }
        _ => None,
    }
}

// --------------------------------------------------------------------------- objects

fn manifest_of(files: &Tree) -> Result<Value> {
    load_json_bytes(files.get(MANIFEST_PATH).map(Vec::as_slice).unwrap_or_default(), MANIFEST_PATH)
}

/// `FORM_XML_EXTRACTION_FAILED` reason of the form `uuid`, if the projection failed.
fn projection_failure(diags: &[Value], uuid: &str) -> String {
    let component = format!("Forms/{uuid}");
    diags
        .iter()
        .find(|d| d["code"] == FORM_XML_EXTRACTION_FAILED && d["component"] == component.as_str())
        .and_then(|d| d["params"]["reason"].as_str())
        .unwrap_or("the form decoder refused the stream")
        .to_string()
}

/// Whether the container survives the rebuild that `pack` does (parse, serialize, parse): same entries, same names,
/// headers and payloads. The padding of the stored file is not compared: `pack` never keeps it.
fn container_exact(raw: &[u8], depth: i64, limits: &Limits) -> Result<bool> {
    let c = parse_container(raw, limits, depth, None)?;
    let entries: Vec<EntryOut<'_>> = c.entries.iter().map(|e| (e.name.as_str(), e.raw.as_slice(), Some(e.header.as_slice()))).collect();
    let back = parse_container(&serialize_container(&entries, 0x200)?, limits, depth, None)?;
    let same = |a: &crate::container::Entry, b: &crate::container::Entry| a.name == b.name && a.raw == b.raw && a.header == b.header;
    Ok(c.entries.len() == back.entries.len() && c.entries.iter().zip(&back.entries).all(|(a, b)| same(a, b)))
}

fn unreadable(object: &str, form: &str, e: &ExtractionError) -> Verdict {
    Verdict {
        object: object.to_string(),
        form: form.to_string(),
        reasons: vec![reason(UNREADABLE, format!("{}: {}", e.code, short(&e.message)))],
    }
}

/// Verdicts of the forms in a staged tree. `exact` maps a form id to the result of the container check.
fn judge_staged(
    object: &str,
    files: &Tree,
    diags: &[Value],
    existing: &Tree,
    config: bool,
    exact: &HashMap<String, Result<bool>>,
    plan_scope: &dyn Fn(&mut dyn FnMut()),
) -> Result<Vec<Verdict>> {
    let manifest = manifest_of(files)?;
    let mut out = Vec::new();
    for rec in manifest["forms"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let id = rec["id"].as_str().unwrap_or("");
        let alias = rec["alias"].as_str().unwrap_or("");
        let name = rec["originalName"].as_str().filter(|n| !n.is_empty()).unwrap_or(alias);
        let stream = rec["layout"]["binary"]["path"].as_str().and_then(|p| files.get(p));
        let xml = rec["layout"]["xml"].as_str().and_then(|p| files.get(p));
        let Some(stream) = stream else {
            let e = ExtractionError::new("PAYLOAD_MISSING", "the form stream is not in the projection");
            out.push(unreadable(object, name, &e));
            continue;
        };
        let why = projection_failure(diags, id);
        let mut reasons = Vec::new();
        plan_scope(&mut || reasons = judge_stream(stream, xml.map(Vec::as_slice), &why));
        match exact.get(id) {
            Some(Ok(false)) => reasons.push(reason(
                ROUND_TRIP_DIFFERS,
                format!("the form container does not survive the rebuild of pack (entries differ after serialize and parse); {CONFIGURATOR}"),
            )),
            Some(Err(e)) => reasons.push(reason(UNREADABLE, format!("{}: {}", e.code, short(&e.message)))),
            _ => {}
        }
        if let Some(why) = stale_reason(existing, &manifest, rec, alias, config) {
            reasons.push(reason(STALE_PROJECTION, format!("{why}; run `oneform extract` again")));
        }
        out.push(Verdict { object: object.to_string(), form: name.to_string(), reasons });
    }
    Ok(out)
}

fn validate_config(root: &Path, obj: &ConfigObject, limits: &Limits, oof_path: Option<&str>) -> Result<Vec<Verdict>> {
    let mut sources = Sources::new();
    for n in &obj.forms {
        let path = object_dir(root, obj).join(form_bin_rel(obj, n));
        sources.insert(n.clone(), fs::read(&path)?);
    }
    let staged = stage_config_object(root, obj, Some(&sources), oof_path, Some(limits), None)?;
    let manifest = manifest_of(&staged.files)?;
    let mut exact: HashMap<String, Result<bool>> = HashMap::new();
    for (alias, info) in manifest["source"]["forms"].as_object().into_iter().flatten() {
        let bin = info["binPath"].as_str().unwrap_or("");
        let id = manifest["forms"].as_array().into_iter().flatten().find(|r| r["alias"] == alias.as_str());
        let name = obj.forms.iter().find(|n| form_bin_rel(obj, n) == bin);
        if let (Some(id), Some(name)) = (id.and_then(|r| r["id"].as_str()), name) {
            exact.insert(id.to_string(), container_exact(&sources[name], 0, limits));
        }
    }
    let config_root = ConfigTypeLookup::find_config_root(root).unwrap_or_else(|| root.to_path_buf());
    let type_lookup = ConfigTypeLookup::for_config_root(&config_root);
    let schema = if obj.common {
        None
    } else {
        let base = object_dir(root, obj);
        let xml = base.parent().zip(base.file_name()).map(|(p, n)| p.join(format!("{}.xml", n.to_string_lossy())));
        xml.and_then(|x| read_object_schema_with(&x, Some(&*type_lookup)))
    };
    let existing = read_tree(&src_dir(root, obj)).unwrap_or_default();
    let scope = |f: &mut dyn FnMut()| with_object_schema(schema.clone(), || with_type_lookup(type_lookup.clone(), f));
    judge_staged(&obj.rel, &staged.files, &staged.diagnostics, &existing, true, &exact, &scope)
}

fn validate_epf(root: &Path, t: &Target, oof_path: Option<&str>) -> Result<Vec<Verdict>> {
    let root_cfg = root_config(root)?;
    let cfg_file = path_of(root, &t.rel).join(CONFIG_NAME);
    let mut proc_cfg = zero_config();
    if !t.multi && t.rel != "." && cfg_file.is_file() {
        proc_cfg = load_json_bytes(&fs::read(&cfg_file)?, &format!("{}/{CONFIG_NAME}", t.rel))?;
    }
    let lock_file = root.join("tools.lock.json");
    let tools_lock = if lock_file.is_file() { fs::read(&lock_file)? } else { DEFAULT_TOOLS_LOCK.as_bytes().to_vec() };
    let epf_path = path_of(root, &t.rel).join(&t.epf);
    let bytes = fs::read(&epf_path)?;
    let orig = falsy(proc_cfg.get("formAliases"));
    let opts = StageOpts { oof_path, use_original_names: orig, epf_dir: epf_path.parent(), ..Default::default() };
    let staged = stage_processor(&bytes, &t.epf, &proc_cfg, &root_cfg, &tools_lock, &opts)?;
    let cfg = effective_config(&root_cfg, &proc_cfg, &[])?;
    let limits = Limits::from_json(&Value::Object(cfg.limits));
    let outer = parse_container(&bytes, &limits, 0, None)?;
    let inv = discover(&outer, &limits, None, false)?;
    let mut exact: HashMap<String, Result<bool>> = HashMap::new();
    for f in &inv.forms {
        if let Some(e) = outer.get(&format!("{}.0", f.uuid)) {
            exact.insert(f.uuid.clone(), inflate(&e.raw, &limits, None).and_then(|inner| container_exact(&inner, 1, &limits)));
        }
    }
    let src = path_of(root, &t.proc_rel).join("src");
    let existing = read_tree(&src).unwrap_or_default();
    let schema = inv.object_schema.clone();
    let scope = |f: &mut dyn FnMut()| with_object_schema(Some(schema.clone()), f);
    let object = if t.rel == "." { t.epf.clone() } else { format!("{}/{}", t.rel, t.epf) };
    judge_staged(&object, &staged.files, &staged.diagnostics, &existing, false, &exact, &scope)
}

fn validate_target(root: &Path, t: &Target, limits: &Limits, oof_path: Option<&str>) -> Vec<Verdict> {
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match (&t.kind, &t.config) {
        (Kind::Config, Some(obj)) => validate_config(root, obj, limits, oof_path),
        _ => validate_epf(root, t, oof_path),
    }));
    let object = match (&t.kind, &t.config) {
        (Kind::Config, Some(obj)) => obj.rel.clone(),
        _ if t.rel == "." => t.epf.clone(),
        _ => format!("{}/{}", t.rel, t.epf),
    };
    match run {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => vec![unreadable(&object, "*", &e)],
        Err(_) => vec![unreadable(&object, "*", &ExtractionError::new("INTERNAL_ERROR", "panic while validating"))],
    }
}

// --------------------------------------------------------------------------- output

pub fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

fn verdict_line(v: &Verdict) -> String {
    if v.in_scope() {
        return match v.reasons.first() {
            None => format!("{}/{}: IN_SCOPE", v.object, v.form),
            Some(r) => format!("{}/{}: IN_SCOPE (run extract: {} \u{2014} {})", v.object, v.form, r.code, r.hint),
        };
    }
    let codes = v.reasons.iter().map(|r| r.code).collect::<Vec<_>>().join(",");
    let hints = v.reasons.iter().map(|r| r.hint.as_str()).collect::<Vec<_>>().join("; ");
    format!("{}/{}: OUT_OF_SCOPE {codes} \u{2014} {hints}", v.object, v.form)
}

fn verdict_json(v: &Verdict) -> Value {
    json!({
        "object": v.object,
        "form": v.form,
        "in_scope": v.in_scope(),
        "reasons": v.reasons.iter().map(|r| json!({"code": r.code, "hint": r.hint})).collect::<Vec<_>>(),
    })
}

/// Forms per reason code (a form with two reasons counts under both).
pub fn breakdown(verdicts: &[Verdict]) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for r in verdicts.iter().flat_map(|v| &v.reasons) {
        *m.entry(r.code).or_default() += 1;
    }
    m
}

/// Verdicts of every ordinary form under `root`, sorted by object and form. `workers` threads, 64 MiB stacks.
pub fn validate_root(root: &Path, workers: usize, oof_path: Option<&str>) -> Result<Vec<Verdict>> {
    use rayon::prelude::*;
    let cfg = effective_config(&root_config(root)?, &zero_config(), &[])?;
    let limits = Limits::from_json(&Value::Object(cfg.limits));
    let targets = discover_targets(root, "validate");
    let run = |t: &Target| validate_target(root, t, &limits, oof_path);
    let per_target: Vec<Vec<Verdict>> = if workers > 1 && targets.len() > 1 {
        match rayon::ThreadPoolBuilder::new().num_threads(workers.min(targets.len())).stack_size(64 << 20).build() {
            Ok(pool) => pool.install(|| targets.par_iter().map(run).collect()),
            Err(_) => targets.iter().map(run).collect(),
        }
    } else {
        targets.iter().map(run).collect()
    };
    let mut all: Vec<Verdict> = per_target.into_iter().flatten().collect();
    all.sort_by(|a, b| (&a.object, &a.form).cmp(&(&b.object, &b.form)));
    Ok(all)
}

/// `oneform validate [PATH] [--json]`: exit 0 when every form is in scope, 1 when any is out, 2 for usage errors.
pub fn run_cli(path: &Path, json_output: bool, workers: usize, oof_path: Option<&str>) -> i32 {
    let root = clean_path(path);
    if !root.is_dir() {
        eprintln!("oneform: error: {} is not a directory", root.display());
        return EXIT_USAGE;
    }
    let verdicts = match validate_root(&root, workers, oof_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("oneform: error: {e}");
            return if e.code == crate::errors::CONFIG_INVALID { EXIT_USAGE } else { EXIT_FAILED };
        }
    };
    if verdicts.is_empty() {
        eprintln!("oneform: error: nothing to process under {}", root.display());
        return EXIT_USAGE;
    }
    let in_scope = verdicts.iter().filter(|v| v.in_scope()).count();
    let code = if in_scope == verdicts.len() { EXIT_OK } else { EXIT_FAILED };
    if json_output {
        let items: Vec<Value> = verdicts.iter().map(verdict_json).collect();
        println!("{}", serde_json::to_string_pretty(&Value::Array(items)).unwrap_or_default());
        return code;
    }
    for v in &verdicts {
        println!("{}", verdict_line(v));
    }
    println!("{} forms, {in_scope} in scope ({:.1} %)", verdicts.len(), percent(in_scope, verdicts.len()));
    let by = breakdown(&verdicts);
    for c in REASON_CODES {
        if let Some(n) = by.get(c) {
            println!("  {c}: {n}");
        }
    }
    code
}
