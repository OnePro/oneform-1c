//! Command line interface of `oneform`: one parallel entry point for EPF files and configuration dumps
//! (port of `oneform_cli.py`).
//!
//! `oneform [extract|pack|preview|check] [PATH]` walks `PATH` and handles every `.epf` (`src/` next to it) and every
//! configuration object with `Ext/Form.bin` (`<ObjDir>/src/`). `preview` renders `FormPreview.html` next to every
//! extracted ordinary `Form.xml`. After `extract`, `pack` and `preview` the offline viewer
//! `PATH/.oneform/index.html` is rebuilt.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::{Arg, ArgAction, Command};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::canonical::{dumps_json, dumps_with, sha256_hex, SORTED_SPACED};
use crate::cli_epf::{
    clean_path, describe, existing_source_sha, plan_lines, Outcome, EXIT_DIFFERS, EXIT_FAILED, EXIT_OK, EXIT_USAGE,
};
use crate::config::{effective_config, load_json_bytes};
use crate::container::Limits;
use crate::discovery::{is_epf, CONFIG_NAME};
use crate::errors::{ExtractionError, Result, CONFIG_INVALID, IO_ERROR, RECOVERY_REQUIRED};
use crate::formbin::{
    check_config_object, discover_config_objects, extract_config_object, find_object_module, pack_config_object,
    read_module_text, walk_dirs, ConfigObject, ConfigRunOpts, OBJECT_CLASS_TITLES,
};
use crate::manifest::MANIFEST_PATH;
use crate::pack::{publish_pack, stage_pack, PACK_UNCHANGED};
use crate::preview::{
    build_workspace_preview, is_ordinary_form_xml, render_form_preview_html, WorkspaceItem, WORKSPACE_DIR,
};
use crate::stage::{stage_processor, StageOpts};
use crate::transaction::{check_ownership, plan, processor_lock, publish, read_tree, recover, snapshot_epf};

pub const COMMANDS: [&str; 4] = ["extract", "pack", "preview", "check"];
pub const DEFAULT_WORKERS: usize = 3;
const SKIP_DIRS: [&str; 5] = [".git", ".venv", ".extract-work", WORKSPACE_DIR, "src"];
/// `json.dumps` of the Reference (default separators).
pub const DEFAULT_TOOLS_LOCK: &str = r#"{"schemaVersion": 1, "containerBackend": {"name": "epf_extract.adapters.container", "kind": "builtin-bounded-reader"}}"#;
const PREVIEW_HEAD: usize = 4096;
const WORKER_STACK: usize = 64 << 20;

const EPILOG: &str = "\
Without a command, PATH is extracted (oneform DIR == oneform extract DIR). Every .epf gets src/ next to it
(several EPF files in one directory: <dir>/<epf stem>/src); without extraction.json the original 1C names are
used for Forms/<name>/ and Templates/<name>/. Every configuration object with Forms/<Form>/Ext/Form.bin
(or CommonForms/<Form>/Ext/Form.bin) gets <ObjDir>/src/. pack writes edited FormModule.bsl / FormStream.bin /
ObjectModule.bsl back; preview renders FormPreview.html next to every ordinary Form.xml.
The viewer is PATH/.oneform/index.html. Exit codes: 0 ok, 1 failure, 2 usage/config error, 3 check found differences.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Epf,
    Config,
    Xml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub kind: Kind,
    /// POSIX path relative to the root: the EPF directory, the object directory or the `Form.xml` file.
    pub rel: String,
    pub epf: String,
    /// epf: directory whose `src` receives the output.
    pub proc_rel: String,
    /// epf: several EPF files share the directory.
    pub multi: bool,
    pub config: Option<ConfigObject>,
}

impl Target {
    fn new(kind: Kind, rel: String) -> Target {
        Target { kind, rel, epf: String::new(), proc_rel: String::new(), multi: false, config: None }
    }

    pub fn display(&self) -> &str {
        if self.kind == Kind::Epf {
            &self.proc_rel
        } else {
            &self.rel
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub command: String,
    pub dry_run: bool,
    pub backup: bool,
    pub replace_generated: bool,
    pub oof_path: Option<String>,
}

pub type Job = (Options, PathBuf, Target);

fn join(base: &str, name: &str) -> String {
    if base.is_empty() || base == "." {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

fn path_of(root: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() || rel == "." {
        return root.to_path_buf();
    }
    let mut p = root.to_path_buf();
    for part in rel.split('/') {
        p.push(part);
    }
    p
}

fn rel_posix(root: &Path, p: &Path) -> Option<String> {
    let rel = p.strip_prefix(root).ok()?;
    let s = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    Some(if s.is_empty() { ".".to_string() } else { s })
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

// --------------------------------------------------------------------------- discovery

pub fn discover_epf_targets(root: &Path) -> Vec<Target> {
    let mut targets = Vec::new();
    let skip = |d: &str| SKIP_DIRS.contains(&d);
    walk_dirs(root, &skip, &mut |base, files| {
        // `find_epfs(base)`: the walk already listed the directory (directories are never EPF candidates).
        let epfs: Vec<&str> = files.iter().filter(|e| is_epf(e)).map(|e| e.name.as_str()).collect();
        if epfs.is_empty() {
            return;
        }
        let rel = rel_posix(root, base).unwrap_or_else(|| ".".to_string());
        let multi = epfs.len() > 1;
        for name in epfs {
            let stem = &name[..name.len() - ".epf".len()];
            let proc_rel = if multi { join(&rel, stem) } else { rel.clone() };
            targets.push(Target {
                kind: Kind::Epf,
                rel: rel.clone(),
                epf: name.to_string(),
                proc_rel,
                multi,
                config: None,
            });
        }
    });
    targets
}

pub fn discover_xml_targets(root: &Path) -> Vec<Target> {
    let mut targets = Vec::new();
    let skip = |d: &str| SKIP_DIRS.contains(&d) && d != "src";
    walk_dirs(root, &skip, &mut |_base, files| {
        for f in files {
            if !f.name.to_lowercase().ends_with(".xml") || f.link {
                continue;
            }
            let p = &f.path;
            let mut head = Vec::with_capacity(PREVIEW_HEAD);
            let Ok(file) = fs::File::open(p) else { continue };
            if file.take(PREVIEW_HEAD as u64).read_to_end(&mut head).is_err()
                || !head.windows(b"ordinaryFormVersion".len()).any(|w| w == b"ordinaryFormVersion")
            {
                continue;
            }
            let Ok(data) = fs::read(p) else { continue };
            if !is_ordinary_form_xml(&data) {
                continue;
            }
            if let Some(rel) = rel_posix(root, p) {
                targets.push(Target::new(Kind::Xml, rel));
            }
        }
    });
    targets
}

pub fn discover_targets(root: &Path, command: &str) -> Vec<Target> {
    if command == "preview" {
        return discover_xml_targets(root);
    }
    let mut targets = discover_epf_targets(root);
    targets.extend(discover_config_objects(root).into_iter().map(|o| {
        let mut t = Target::new(Kind::Config, o.rel.clone());
        t.config = Some(o);
        t
    }));
    targets
}

// --------------------------------------------------------------------------- workers

fn zero_config() -> Value {
    json!({"schemaVersion": 1})
}

fn root_config(root: &Path) -> Result<Value> {
    let f = root.join(CONFIG_NAME);
    if f.is_file() {
        load_json_bytes(&fs::read(&f)?, CONFIG_NAME)
    } else {
        Ok(zero_config())
    }
}

/// Python truthiness of an optional JSON value.
fn falsy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => !b,
        Some(Value::Number(n)) => n.as_f64() == Some(0.0),
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
    }
}

fn outcome(path: &str, status: &'static str, summary: String, lines: Vec<String>) -> Outcome {
    let mut out = Outcome::new(path, status);
    out.summary = summary;
    out.lines = lines;
    out
}

fn counts(added: usize, updated: usize, deleted: usize) -> String {
    format!("+{added} ~{updated} -{deleted}")
}

fn process_epf(opts: &Options, root: &Path, t: &Target) -> Result<Outcome> {
    let root_cfg = root_config(root)?;
    let cfg_file = path_of(root, &t.rel).join(CONFIG_NAME);
    let mut proc_cfg = zero_config();
    if !t.multi && t.rel != "." && cfg_file.is_file() {
        proc_cfg = load_json_bytes(&fs::read(&cfg_file)?, &format!("{}/{CONFIG_NAME}", t.rel))?;
    }
    let lock_file = root.join("tools.lock.json");
    let tools_lock = if lock_file.is_file() { fs::read(&lock_file)? } else { DEFAULT_TOOLS_LOCK.as_bytes().to_vec() };
    let orig = falsy(proc_cfg.get("formAliases"));
    let epf_path = path_of(root, &t.rel).join(&t.epf);
    let src = path_of(root, &t.proc_rel).join("src");
    let name = t.display();
    let stage_opts = StageOpts { oof_path: opts.oof_path.as_deref(), use_original_names: orig, ..Default::default() };

    let _lock = processor_lock(root, &t.proc_rel)?;
    let snap = snapshot_epf(&epf_path)?;
    if opts.command == "pack" {
        if !src.join(MANIFEST_PATH).is_file() {
            return Ok(outcome(name, "SKIPPED", "no src; run extract first".into(), Vec::new()));
        }
        let plan_ = stage_pack(&snap.data, &t.epf, &read_tree(&src)?, &proc_cfg, &root_cfg, &tools_lock, &stage_opts)?;
        let summary = describe(&plan_.result.files, &plan_.result.status);
        if plan_.status == PACK_UNCHANGED {
            return Ok(outcome(name, "UNCHANGED", String::new(), Vec::new()));
        }
        let mut packed: Vec<String> = plan_.packed_paths.iter().map(|p| format!("  pack {p}")).collect();

        packed.extend(plan_.edit_notes.iter().cloned());
        let (added, updated, deleted) = plan(&plan_.existing, &plan_.result.files);
        if opts.dry_run {
            if !packed.is_empty() {
                return Ok(outcome(name, "WOULD_PACK", format!("{} file(s)", plan_.packed_paths.len()), packed));
            }
            return Ok(outcome(
                name,
                "WOULD_UPDATE",
                counts(added.len(), updated.len(), deleted.len()),
                plan_lines(&added, &updated, &deleted),
            ));
        }
        if let Some(parent) = src.parent() {
            fs::create_dir_all(parent)?;
        }
        publish_pack(root, &t.proc_rel, &plan_, &epf_path, &snap, opts.backup, false)?;
        if !packed.is_empty() {
            let mut lines = packed.clone();
            if opts.backup {
                lines.push(format!("  backup {}.bak", file_name(&epf_path)));
            }
            return Ok(outcome(name, "PACKED", format!("{} file(s), {summary}", plan_.packed_paths.len()), lines));
        }
        return Ok(outcome(name, "UPDATED", summary, Vec::new()));
    }

    let result = stage_processor(&snap.data, &t.epf, &proc_cfg, &root_cfg, &tools_lock, &stage_opts)?;
    let summary = describe(&result.files, &result.status);
    let existing = read_tree(&src)?;
    let (added, updated, deleted) = plan(&existing, &result.files);
    let changed = !(added.is_empty() && updated.is_empty() && deleted.is_empty());
    if opts.command == "check" {
        if !changed {
            return Ok(outcome(name, "OK", summary, Vec::new()));
        }
        let c = format!("(+{} ~{} -{})", added.len(), updated.len(), deleted.len());
        let stale = existing_source_sha(&existing).as_deref() != Some(snap.sha256.as_str());
        let (status, text) = if stale {
            ("STALE", format!("existing src is stale for current EPF {c}"))
        } else {
            ("DIFFERS", format!("src differs from the current extraction {c}"))
        };
        return Ok(outcome(name, status, text, plan_lines(&added, &updated, &deleted)));
    }
    if opts.dry_run {
        if !changed {
            return Ok(outcome(name, "UNCHANGED", String::new(), Vec::new()));
        }
        check_ownership(&existing, opts.replace_generated)?;
        return Ok(outcome(
            name,
            "WOULD_UPDATE",
            counts(added.len(), updated.len(), deleted.len()),
            plan_lines(&added, &updated, &deleted),
        ));
    }
    if let Some(parent) = src.parent() {
        fs::create_dir_all(parent)?;
    }
    let publ = publish(root, &t.proc_rel, &result.files, &epf_path, &snap, false, opts.replace_generated)?;
    Ok(outcome(name, publ.status, summary, Vec::new()))
}

fn process_config(opts: &Options, root: &Path, t: &Target) -> Result<Outcome> {
    let obj = t.config.as_ref().expect("config target has an object");
    let cfg = effective_config(&root_config(root)?, &zero_config(), &[])?;
    let limits = Limits::from_json(&Value::Object(cfg.limits));
    let run = ConfigRunOpts {
        dry_run: opts.dry_run,
        replace_generated: opts.replace_generated,
        backup: opts.backup,
        keep_work: false,
        oof_path: opts.oof_path.as_deref(),
        limits: Some(&limits),
        recover_leftovers: false,
    };
    match opts.command.as_str() {
        "check" => check_config_object(root, obj, &run),
        "pack" => {
            if !path_of(root, &obj.rel).join("src").join(MANIFEST_PATH).is_file() {
                return Ok(outcome(&obj.rel, "SKIPPED", "no src; run extract first".into(), Vec::new()));
            }
            pack_config_object(root, obj, &run)
        }
        _ => extract_config_object(root, obj, &run),
    }
}

fn form_module_bytes(form_dir: &Path) -> Vec<u8> {
    for name in ["FormModule.bsl", "Module.bsl"] {
        let p = form_dir.join(name);
        if p.is_file() {
            return fs::read(p).unwrap_or_default();
        }
    }
    Vec::new()
}

fn object_module_bytes(src_root: &Path) -> Vec<u8> {
    let inner = src_root.join("ObjectModule.bsl");
    if inner.is_file() {
        return fs::read(inner).unwrap_or_default();
    }
    match src_root.parent().and_then(find_object_module) {
        Some(outer) => fs::read(outer).map(|raw| read_module_text(&raw, "ObjectModule")).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Rewrite `target` through a `.tmp` sibling (`os.replace`).
fn replace_file(target: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_file_name(format!("{}.tmp", file_name(target)));
    fs::write(&tmp, data)?;
    fs::rename(&tmp, target)
}

/// Refresh `size`/`sha256` of the `FormPreview.html` entry of an existing `src/ExtractionManifest.json`.
fn update_manifest_preview(manifest_file: &Path, form_dir_name: &str, html: &[u8]) {
    let Ok(raw) = fs::read(manifest_file) else { return };
    let Ok(mut m) = serde_json::from_slice::<Value>(&raw) else { return };
    let rel_preview = format!("Forms/{form_dir_name}/FormPreview.html");
    let Some(files) = m.get_mut("files").and_then(Value::as_array_mut) else { return };
    for entry in files {
        if entry.is_object() && entry.get("path").and_then(Value::as_str) == Some(rel_preview.as_str()) {
            entry["size"] = Value::from(html.len());
            entry["sha256"] = Value::from(sha256_hex(html));
            if let Ok(bytes) = dumps_json(&m) {
                let _ = replace_file(manifest_file, &bytes);
            }
            return;
        }
    }
}

fn process_xml(opts: &Options, root: &Path, t: &Target) -> Result<Outcome> {
    let xml_path = path_of(root, &t.rel);
    let form_dir = xml_path.parent().unwrap_or(root).to_path_buf();
    let src_root = form_dir.parent().and_then(Path::parent).unwrap_or(root).to_path_buf();
    let html = render_form_preview_html(
        &fs::read(&xml_path)?,
        &form_module_bytes(&form_dir),
        &object_module_bytes(&src_root),
        &file_name(&form_dir),
    )?;
    let target = form_dir.join("FormPreview.html");
    if target.is_file() && fs::read(&target)? == html {
        return Ok(outcome(&t.rel, "UNCHANGED", String::new(), Vec::new()));
    }
    if opts.dry_run {
        return Ok(outcome(&t.rel, "WOULD_UPDATE", "FormPreview.html".into(), Vec::new()));
    }
    replace_file(&target, &html)?;
    let manifest_file = src_root.join(MANIFEST_PATH);
    if manifest_file.is_file() {
        update_manifest_preview(&manifest_file, &file_name(&form_dir), &html);
    }
    Ok(outcome(&t.rel, "UPDATED", "FormPreview.html".into(), Vec::new()))
}

fn failure(path: &str, e: ExtractionError) -> Outcome {
    let mut o = Outcome::new(path, "FAILED");
    o.code = Some(e.code);
    o.message = e.message;
    o.params = e.params;
    o
}

/// Process one target; never fails (a failure becomes a FAILED outcome).
pub fn run_target(job: &Job) -> Outcome {
    let (opts, root, t) = job;
    let start = Instant::now();
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match t.kind {
        Kind::Epf => process_epf(opts, root, t),
        Kind::Config => process_config(opts, root, t),
        Kind::Xml => process_xml(opts, root, t),
    }));
    let mut out = match run {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => failure(t.display(), e),
        Err(panic) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_default();
            let mut o = Outcome::new(t.display(), "FAILED");
            o.code = Some("INTERNAL_ERROR");
            o.message = format!("panic: {msg}");
            o
        }
    };
    out.duration = start.elapsed().as_secs_f64();
    out
}

/// Look for residual crash journals (`journal.json`) in `<root>/.extract-work/` once and return the set of
/// `processor` relative paths that need recovery. When `.extract-work/` has no journals (the 99.9% case), this
/// costs a single `read_dir` instead of `2 * N` lock + `read_dir` syscalls before parallel workers start.
fn processors_needing_recovery(root: &Path) -> BTreeSet<String> {
    let work = crate::transaction::work_dir(root);
    let Ok(rd) = fs::read_dir(&work) else { return BTreeSet::new() };
    let mut set = BTreeSet::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() || p.file_name().is_some_and(|n| n == "locks") {
            continue;
        }
        let j = p.join( crate::transaction::JOURNAL_NAME);
        if j.is_file() {
            match fs::read(&j).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                Some(v) => {
                    if let Some(proc_rel) = v.get("processor").and_then(Value::as_str) {
                        set.insert(proc_rel.to_string());
                    }
                }
                None => {
                    // Unreadable journal: mark all targets for recovery so the error is reported identically.
                    set.insert("*".to_string());
                }
            }
        }
    }
    set
}

/// Resolve leftovers of a crashed run; done before (and without) parallel workers.
///
/// `recover` reads the journals of every run in `.extract-work`, so it must never race with a publication.
pub fn recover_target(opts: &Options, root: &Path, t: &Target) -> Result<Vec<String>> {
    if t.kind == Kind::Xml || !matches!(opts.command.as_str(), "extract" | "pack") || opts.dry_run {
        return Ok(Vec::new());
    }
    let rel = if t.kind == Kind::Epf { &t.proc_rel } else { &t.rel };
    let _lock = processor_lock(root, rel)?;
    Ok(recover(root, rel)?.into_iter().map(|a| format!("recovered: {a}")).collect())
}

// --------------------------------------------------------------------------- progress

enum Progress {
    Off,
    Bar(ProgressBar),
    /// Forced progress on a stream that is not a terminal: plain `\r` updates.
    Plain { desc: String, total: usize, done: AtomicUsize },
}

impl Progress {
    fn new(total: usize, desc: &str, show: Option<bool>) -> Progress {
        let tty = std::io::stderr().is_terminal();
        match show {
            Some(false) => Progress::Off,
            None if !tty => Progress::Off,
            _ if tty => {
                let bar = ProgressBar::with_draw_target(Some(total as u64), ProgressDrawTarget::stderr());
                if let Ok(style) = ProgressStyle::with_template("{msg}: {bar:30} {pos}/{len} obj [{elapsed}] {prefix}") {
                    bar.set_style(style);
                }
                bar.set_message(desc.to_string());
                Progress::Bar(bar)
            }
            _ => Progress::Plain { desc: desc.to_string(), total, done: AtomicUsize::new(0) },
        }
    }

    fn tick(&self, name: &str) {
        match self {
            Progress::Off => {}
            Progress::Bar(b) => {
                b.set_prefix(name.chars().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<String>());
                b.inc(1);
            }
            Progress::Plain { desc, total, done } => {
                let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                eprint!("\r{desc}: {n}/{total} obj");
            }
        }
    }

    fn finish(&self) {
        match self {
            Progress::Off => {}
            Progress::Bar(b) => b.finish(),
            Progress::Plain { .. } => eprintln!(),
        }
    }
}

/// Run all jobs (results keep the order of `jobs`); targets that failed with a transient I/O or recovery error are
/// retried serially once.
pub fn run_all(jobs: &[Job], workers: usize, desc: &str, show_progress: Option<bool>) -> Vec<Outcome> {
    let progress = Progress::new(jobs.len(), desc, show_progress);
    let run_one = |j: &Job| {
        let out = run_target(j);
        progress.tick(j.2.display());
        out
    };
    let serial = || jobs.iter().map(run_one).collect::<Vec<Outcome>>();
    let mut outcomes = if workers > 1 && jobs.len() > 1 {
        match rayon::ThreadPoolBuilder::new().num_threads(workers.min(jobs.len())).stack_size(WORKER_STACK).build() {
            Ok(pool) => pool.install(|| jobs.par_iter().map(run_one).collect::<Vec<Outcome>>()),
            Err(_) => serial(),
        }
    } else {
        serial()
    };
    progress.finish();
    for (i, out) in outcomes.iter_mut().enumerate() {
        if out.status == "FAILED" && matches!(out.code, Some(IO_ERROR | RECOVERY_REQUIRED)) {
            let (opts, root, t) = &jobs[i];
            let Ok(notes) = recover_target(opts, root, t) else { continue };
            let mut retried = run_target(&jobs[i]);
            retried.lines.splice(0..0, notes);
            *out = retried;
        }
    }
    outcomes
}

// --------------------------------------------------------------------------- viewer items

/// `posixpath.normpath`.
pub fn normpath(p: &str) -> String {
    let abs = p.starts_with('/');
    let mut stack: Vec<&str> = Vec::new();
    for comp in p.split('/') {
        match comp {
            "" | "." => {}
            ".." => {
                if stack.last().is_some_and(|l| *l != "..") {
                    stack.pop();
                } else if !abs {
                    stack.push("..");
                }
            }
            c => stack.push(c),
        }
    }
    let body = stack.join("/");
    match (abs, body.is_empty()) {
        (true, _) => format!("/{body}"),
        (false, true) => ".".to_string(),
        (false, false) => body,
    }
}

/// `posixpath.join(a, b)`.
fn posix_join(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

fn str_or<'a>(v: &'a Value, default: &'a str) -> &'a str {
    v.as_str().filter(|s| !s.is_empty()).unwrap_or(default)
}

fn manifest_items(root: &Path, t: &Target) -> Vec<WorkspaceItem> {
    let (src_rel, cls, rel) = match (&t.kind, &t.config) {
        (Kind::Epf, _) => (join(&t.proc_rel, "src"), "ExternalDataProcessors".to_string(), t.proc_rel.clone()),
        (_, Some(c)) => (join(&t.rel, "src"), c.object_class.clone(), t.rel.clone()),
        _ => return Vec::new(),
    };
    let Ok(raw) = fs::read(path_of(root, &src_rel).join(MANIFEST_PATH)) else { return Vec::new() };
    let Ok(manifest) = serde_json::from_slice::<Value>(&raw) else { return Vec::new() };
    let (Some(source), Some(forms)) = (manifest.get("source"), manifest.get("forms").and_then(Value::as_array)) else {
        return Vec::new();
    };
    let om = manifest.get("objectModule").cloned().unwrap_or(Value::Null);
    let om_path_v = om.get("path").and_then(Value::as_str).filter(|p| !p.is_empty());
    let om_path = match (om.get("state").and_then(Value::as_str), om_path_v) {
        (Some("referenced"), Some(p)) => normpath(&posix_join(&src_rel, p)),
        (Some("extracted"), Some(p)) => posix_join(&src_rel, p),
        _ => String::new(),
    };
    let mut warnings: BTreeMap<String, i64> = BTreeMap::new();
    for d in manifest.get("diagnostics").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default() {
        if d.get("severity").and_then(Value::as_str) == Some("warning") {
            *warnings.entry(d.get("component").and_then(Value::as_str).unwrap_or("").to_string()).or_insert(0) += 1;
        }
    }
    let mut items = Vec::new();
    for f in forms {
        let (Some(id), Some(alias), Some(module_path)) = (
            f.get("id").and_then(Value::as_str),
            f.get("alias").and_then(Value::as_str),
            f.get("module").and_then(|m| m.get("path")).and_then(Value::as_str),
        ) else {
            return Vec::new();
        };
        let layout = f.get("layout").cloned().unwrap_or(Value::Null);
        let layout_path = |k: &str| match layout.get(k).and_then(Value::as_str).filter(|p| !p.is_empty()) {
            Some(p) => posix_join(&src_rel, p),
            None => String::new(),
        };
        items.push(WorkspaceItem {
            class_name: cls.clone(),
            rel_path: rel.clone(),
            object_id: str_or(&source["processorId"], "").to_string(),
            object_name: str_or(&source["processorName"], "").to_string(),
            object_synonym: str_or(&source["objectSynonym"], "").to_string(),
            form_id: Some(id.to_string()),
            form_name: str_or(&f["originalName"], alias).to_string(),
            alias: Some(alias.to_string()),
            synonym: str_or(&f["synonym"], "").to_string(),
            warning_count: warnings.get(&format!("Forms/{id}")).copied().unwrap_or(0),
            xml_path: layout_path("xml"),
            module_path: posix_join(&src_rel, module_path),
            preview_path: layout_path("preview"),
            object_module_path: om_path.clone(),
        });
    }
    items
}

fn xml_item(root: &Path, t: &Target) -> Option<WorkspaceItem> {
    let xml_path = path_of(root, &t.rel);
    let form_dir = xml_path.parent()?.to_path_buf();
    let forms_dir = form_dir.parent()?.to_path_buf();
    let src_root = if file_name(&forms_dir) == "Forms" { forms_dir.parent()?.to_path_buf() } else { form_dir.clone() };
    let obj_root = if file_name(&src_root) == "src" { src_root.parent()?.to_path_buf() } else { src_root.clone() };
    let obj_rel = match rel_posix(root, &obj_root)? {
        r if r == "." => String::new(),
        r => r,
    };
    let mut parts: Vec<&str> = obj_rel.split('/').collect();
    parts.pop();
    let class_name = if obj_rel.is_empty() {
        String::new()
    } else {
        parts
            .iter()
            .rev()
            .find(|p| OBJECT_CLASS_TITLES.iter().any(|(c, _)| c == *p))
            .map(|p| (*p).to_string())
            .unwrap_or_default()
    };
    let text = String::from_utf8(fs::read(&xml_path).ok()?).ok()?;
    let doc = roxmltree::Document::parse_with_options(
        text.trim_start_matches('\u{feff}'),
        roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() },
    )
    .ok()?;
    let attrs = doc.root_element();
    let module = [form_dir.join("FormModule.bsl"), form_dir.join("Module.bsl")].into_iter().find(|p| p.is_file());
    let om = [Some(src_root.join("ObjectModule.bsl")), find_object_module(&obj_root)]
        .into_iter()
        .flatten()
        .find(|p| p.is_file());
    let rel = |p: Option<PathBuf>| -> Option<String> {
        match p {
            Some(p) => rel_posix(root, &p),
            None => Some(String::new()),
        }
    };
    let preview = form_dir.join("FormPreview.html");
    let alias = file_name(&form_dir);
    Some(WorkspaceItem {
        class_name,
        rel_path: obj_rel,
        object_name: {
            let n = file_name(&obj_root);
            if n.is_empty() {
                file_name(root)
            } else {
                n
            }
        },
        form_id: Some(attrs.attribute("id").unwrap_or("").to_string()),
        form_name: match attrs.attribute("name") {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => alias.clone(),
        },
        alias: Some(alias),
        xml_path: t.rel.clone(),
        module_path: rel(module)?,
        preview_path: if preview.is_file() { rel_posix(root, &preview)? } else { String::new() },
        object_module_path: rel(om)?,
        ..Default::default()
    })
}

pub fn collect_items(root: &Path, targets: &[Target]) -> Vec<WorkspaceItem> {
    let mut items = Vec::new();
    for t in targets {
        if t.kind == Kind::Xml {
            items.extend(xml_item(root, t));
        } else {
            items.extend(manifest_items(root, t));
        }
    }
    items
}

// --------------------------------------------------------------------------- CLI

#[derive(Debug, Clone)]
pub struct Cli {
    pub first: Option<String>,
    pub second: Option<String>,
    pub workers: i64,
    pub dry_run: bool,
    pub backup: bool,
    pub no_backup: bool,
    pub replace_generated: bool,
    pub oof_path: Option<String>,
    pub progress: bool,
    pub no_progress: bool,
}

fn build_command() -> Command {
    Command::new("oneform")
        .version(env!("CARGO_PKG_VERSION"))
        .override_usage("oneform [OPTIONS] [extract|pack|preview|check] [PATH]")
        .about("Extract, pack, check and preview 1C ordinary forms of EPF files and configuration dumps.")
        .after_help(EPILOG)
        .arg(Arg::new("first").value_name("[extract|pack|preview|check]").help("command (default: extract)"))
        .arg(Arg::new("second").value_name("PATH").help("directory to process (default: .)"))
        .arg(
            Arg::new("workers")
                .short('j')
                .long("workers")
                .value_name("N")
                .value_parser(clap::value_parser!(i64))
                .default_value("3")
                .help("parallel workers (default: 3)"),
        )
        .arg(Arg::new("dry_run").long("dry-run").action(ArgAction::SetTrue).help("print the plan; write nothing"))
        .arg(
            Arg::new("backup")
                .long("backup")
                .action(ArgAction::SetTrue)
                .conflicts_with("no_backup")
                .help("pack: keep the previous EPF / Form.bin as .bak"),
        )
        .arg(Arg::new("no_backup").long("no-backup").action(ArgAction::SetTrue).help("pack: do not write .bak (default)"))
        .arg(
            Arg::new("replace_generated")
                .long("replace-generated")
                .action(ArgAction::SetTrue)
                .help("extract: replace manually modified files that the old manifest owns"),
        )
        .arg(Arg::new("oof_path").long("oof-path").value_name("PATH").help("optional external 'oof' binary (default: EPF_OOF_PATH)"))
        .arg(
            Arg::new("progress")
                .long("progress")
                .action(ArgAction::SetTrue)
                .conflicts_with("no_progress")
                .help("force progress bar even when stderr is not a TTY"),
        )
        .arg(Arg::new("no_progress").long("no-progress").action(ArgAction::SetTrue).help("disable progress bar"))
}

pub fn parse_cli(argv: &[String]) -> std::result::Result<Cli, i32> {
    let mut full = vec!["oneform".to_string()];
    full.extend_from_slice(argv);
    let m = match build_command().try_get_matches_from(full) {
        Ok(m) => m,
        Err(e) => {
            use clap::error::ErrorKind;
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                print!("{e}");
                return Err(EXIT_OK);
            }
            eprint!("{e}");
            return Err(EXIT_USAGE);
        }
    };
    Ok(Cli {
        first: m.get_one::<String>("first").cloned(),
        second: m.get_one::<String>("second").cloned(),
        workers: m.get_one::<i64>("workers").copied().unwrap_or(DEFAULT_WORKERS as i64),
        dry_run: m.get_flag("dry_run"),
        backup: m.get_flag("backup"),
        no_backup: m.get_flag("no_backup"),
        replace_generated: m.get_flag("replace_generated"),
        oof_path: m.get_one::<String>("oof_path").cloned(),
        progress: m.get_flag("progress"),
        no_progress: m.get_flag("no_progress"),
    })
}

/// Python `repr()` of a `str`.
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `(command, path)` from the two optional positionals; `Err(message)` for an unknown command.
pub fn resolve_command(first: Option<&str>, second: Option<&str>) -> std::result::Result<(String, String), String> {
    match (first, second) {
        (Some(f), s) if COMMANDS.contains(&f) => Ok((f.to_string(), s.unwrap_or(".").to_string())),
        (None, _) => Ok(("extract".to_string(), ".".to_string())),
        (Some(f), Some(_)) => {
            Err(format!("unknown command {}; expected one of {}", py_repr(f), COMMANDS.join(", ")))
        }
        (Some(f), None) => Ok(("extract".to_string(), f.to_string())),
    }
}

fn print_outcome(o: &Outcome, dry_run: bool) {
    if o.status == "FAILED" {
        println!("{}: FAILED, {}", o.path, o.code.unwrap_or(""));
        println!("  {}", o.message);
        if !o.params.is_empty() {
            println!("  {}", dumps_with(&Value::Object(o.params.clone()), &SORTED_SPACED).unwrap_or_default());
        }
        return;
    }
    let label = match o.status {
        "WOULD_UPDATE" => "WOULD UPDATE",
        "WOULD_PACK" => "WOULD PACK",
        "UNCHANGED" if dry_run => "NO CHANGES",
        other => other,
    };
    if o.summary.is_empty() {
        println!("{}: {label}", o.path);
    } else {
        println!("{}: {label}, {}", o.path, o.summary);
    }
    for line in &o.lines {
        println!("{line}");
    }
}

fn exit_code(outcomes: &[Outcome], command: &str) -> i32 {
    let failed: Vec<&Outcome> = outcomes.iter().filter(|o| o.status == "FAILED").collect();
    if failed.iter().any(|o| o.code == Some(CONFIG_INVALID)) {
        return EXIT_USAGE;
    }
    if !failed.is_empty() {
        return EXIT_FAILED;
    }
    if command == "check" && outcomes.iter().any(|o| matches!(o.status, "DIFFERS" | "STALE")) {
        return EXIT_DIFFERS;
    }
    EXIT_OK
}

fn fail(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_USAGE
}

pub fn main(argv: &[String]) -> i32 {
    let args = match parse_cli(argv) {
        Ok(a) => a,
        Err(code) => return code,
    };
    let (command, path) = match resolve_command(args.first.as_deref(), args.second.as_deref()) {
        Ok(v) => v,
        Err(m) => return fail(&m),
    };
    if args.workers < 1 {
        return fail("--workers must be at least 1");
    }
    if (args.backup || args.no_backup) && command != "pack" {
        return fail("--backup/--no-backup require the pack command");
    }
    if args.replace_generated && matches!(command.as_str(), "pack" | "check" | "preview") {
        return fail("--replace-generated is only valid for extract");
    }

    let root = clean_path(Path::new(&path));
    if !root.is_dir() {
        return fail(&format!("{} is not a directory", root.display()));
    }
    let opts = Options {
        command: command.clone(),
        dry_run: args.dry_run,
        backup: args.backup,
        replace_generated: args.replace_generated,
        oof_path: args.oof_path.clone().or_else(|| std::env::var("EPF_OOF_PATH").ok()).filter(|s| !s.is_empty()),
    };
    let targets = discover_targets(&root, &command);
    if targets.is_empty() {
        return fail(&format!("nothing to process under {}", root.display()));
    }
    let mut jobs: Vec<Job> = Vec::new();
    let mut recovered: Vec<Vec<String>> = Vec::new();
    let mut outcomes: Vec<Outcome> = Vec::new();
    let dirty = if matches!(command.as_str(), "extract" | "pack") && !args.dry_run {
        processors_needing_recovery(&root)
    } else {
        BTreeSet::new()
    };
    for t in &targets {
        let rel = if t.kind == Kind::Epf { &t.proc_rel } else { &t.rel };
        let needs = !dirty.is_empty() && (dirty.contains("*") || dirty.contains(rel));
        let res = if needs { recover_target(&opts, &root, t) } else { Ok(Vec::new()) };
        match res {
            Ok(notes) => {
                recovered.push(notes);
                jobs.push((opts.clone(), root.clone(), t.clone()));
            }
            Err(e) => outcomes.push(failure(t.display(), e)),
        }
    }
    let show = if args.progress {
        Some(true)
    } else if args.no_progress {
        Some(false)
    } else {
        None
    };
    let mut ran = run_all(&jobs, args.workers as usize, &format!("oneform {command}"), show);
    for (out, notes) in ran.iter_mut().zip(recovered) {
        out.lines.splice(0..0, notes);
    }
    outcomes.extend(ran);
    for o in &outcomes {
        print_outcome(o, args.dry_run);
    }
    let mut code = exit_code(&outcomes, &command);

    if command != "check" && !args.dry_run {
        let items = collect_items(&root, &targets);
        if !items.is_empty() {
            match build_workspace_preview(&root, &items) {
                Ok(out) => {
                    let n_objects =
                        items.iter().map(|i| (i.class_name.as_str(), i.rel_path.as_str())).collect::<BTreeSet<_>>().len();
                    println!("Viewer: {} ({n_objects} objects, {} forms)", out.join("index.html").display(), items.len());
                }
                Err(e) if e.code == IO_ERROR => {
                    eprintln!("oneform: viewer: {}", e.message);
                    code = code.max(EXIT_FAILED);
                }
                Err(e) => {
                    eprintln!("oneform: viewer: {}: {}", e.code, e.message);
                    code = code.max(EXIT_FAILED);
                }
            }
        }
    }

    let n = |statuses: &[&str]| outcomes.iter().filter(|o| statuses.contains(&o.status)).count();
    println!(
        "Batch: {} targets, {} updated, {} ok/unchanged, {} differ, {} planned, {} skipped, {} failed; exit {code}",
        outcomes.len(),
        n(&["UPDATED", "PACKED"]),
        n(&["OK", "UNCHANGED"]),
        n(&["DIFFERS", "STALE"]),
        n(&["WOULD_UPDATE", "WOULD_PACK"]),
        n(&["SKIPPED"]),
        n(&["FAILED"]),
    );
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    fn write(path: &Path, data: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    #[test]
    fn positional_resolution() {
        assert_eq!(resolve_command(None, None), Ok(("extract".into(), ".".into())));
        assert_eq!(resolve_command(Some("check"), None), Ok(("check".into(), ".".into())));
        assert_eq!(resolve_command(Some("pack"), Some("d")), Ok(("pack".into(), "d".into())));
        assert_eq!(resolve_command(Some("dir"), None), Ok(("extract".into(), "dir".into())));
        assert_eq!(
            resolve_command(Some("bogus"), Some(".")),
            Err("unknown command 'bogus'; expected one of extract, pack, preview, check".to_string())
        );
        assert_eq!(py_repr("it's"), "\"it's\"");
    }

    #[test]
    fn normpath_matches_posixpath() {
        assert_eq!(normpath("src/../Ext/ObjectModule.bsl"), "Ext/ObjectModule.bsl");
        assert_eq!(normpath("Documents/D/src/../Ext/X.bsl"), "Documents/D/Ext/X.bsl");
        assert_eq!(normpath("../a"), "../a");
        assert_eq!(normpath("a/.."), ".");
        assert_eq!(posix_join("a/src", "../Ext/M.bsl"), "a/src/../Ext/M.bsl");
    }

    #[test]
    fn default_tools_lock_is_valid_json() {
        let v: Value = serde_json::from_str(DEFAULT_TOOLS_LOCK).unwrap();
        assert_eq!(v["containerBackend"]["kind"], "builtin-bounded-reader");
        assert_eq!(DEFAULT_WORKERS, 3);
    }

    #[test]
    fn parse_cli_options() {
        let c = parse_cli(&s(&["extract", "ws", "-j", "2", "--dry-run", "--no-progress"])).unwrap();
        assert_eq!((c.first.as_deref(), c.second.as_deref(), c.workers), (Some("extract"), Some("ws"), 2));
        assert!(c.dry_run && c.no_progress && !c.progress);
        assert_eq!(parse_cli(&s(&[])).unwrap().workers, 3);
        assert_eq!(parse_cli(&s(&["--backup", "--no-backup"])).unwrap_err(), EXIT_USAGE);
        assert_eq!(parse_cli(&s(&["--progress", "--no-progress"])).unwrap_err(), EXIT_USAGE);
    }

    #[test]
    fn usage_errors() {
        // port: tests/test_oneform.py::test_usage_errors
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_string_lossy().into_owned();
        assert_eq!(main(&s(&["bogus", &d])), 2);
        assert_eq!(main(&s(&["extract", &format!("{d}/missing")])), 2);
        assert_eq!(main(&s(&["extract", &d])), 2);
        assert_eq!(main(&s(&["extract", &d, "--backup"])), 2);
        assert_eq!(main(&s(&["check", &d, "--replace-generated"])), 2);
        assert_eq!(main(&s(&["extract", &d, "-j", "0"])), 2);
    }

    #[test]
    fn discover_targets_epf_config_and_xml() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("Zero/Proc.epf"), b"x");
        write(&root.join("Dir/A.epf"), b"x");
        write(&root.join("Dir/B.EPF"), b"x");
        write(&root.join("Dir/B.epf.bak"), b"x");
        write(&root.join("Documents/D/Forms/F/Ext/Form.bin"), b"x");
        write(&root.join("src/Skipped.epf"), b"x");
        write(
            &root.join("Documents/D/src/Forms/F/Form.xml"),
            b"<Form ordinaryFormVersion=\"2.1\" name=\"F\"><Width>1</Width></Form>",
        );
        write(&root.join("Other.xml"), b"<Form name=\"Managed\"/>");

        let epf = discover_epf_targets(root);
        let shown: Vec<(&str, &str, bool)> = epf.iter().map(|t| (t.proc_rel.as_str(), t.epf.as_str(), t.multi)).collect();
        assert_eq!(shown, [("Dir/A", "A.epf", true), ("Dir/B", "B.EPF", true), ("Zero", "Proc.epf", false)]);
        let all = discover_targets(root, "extract");
        assert_eq!(all.len(), 4);
        assert_eq!((all[3].kind, all[3].rel.as_str()), (Kind::Config, "Documents/D"));
        let xml = discover_targets(root, "preview");
        assert_eq!(xml.iter().map(|t| t.rel.as_str()).collect::<Vec<_>>(), ["Documents/D/src/Forms/F/Form.xml"]);
        let items = collect_items(root, &xml);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].class_name.as_str(), items[0].rel_path.as_str()), ("Documents", "Documents/D"));
        assert_eq!(items[0].form_name, "F");
    }

    #[test]
    fn preview_updates_html_and_manifest_entry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let form = root.join("Documents/D/src/Forms/F");
        write(&form.join("Form.xml"), b"<Form ordinaryFormVersion=\"2.1\" name=\"F\"><Width>1</Width></Form>");
        write(&form.join("FormPreview.html"), b"old");
        let manifest = json!({"schemaVersion": 1, "files": [
            {"path": "Forms/F/FormPreview.html", "role": "form-preview", "size": 3, "sha256": "x"}]});
        write(&root.join("Documents/D/src").join(MANIFEST_PATH), &dumps_json(&manifest).unwrap());
        let opts = Options { command: "preview".into(), dry_run: false, backup: false, replace_generated: false, oof_path: None };
        let t = Target::new(Kind::Xml, "Documents/D/src/Forms/F/Form.xml".into());
        let out = run_target(&(opts.clone(), root.to_path_buf(), t.clone()));
        assert_eq!((out.status, out.summary.as_str()), ("UPDATED", "FormPreview.html"));
        let html = fs::read(form.join("FormPreview.html")).unwrap();
        assert_ne!(html, b"old");
        let m: Value = serde_json::from_slice(&fs::read(root.join("Documents/D/src").join(MANIFEST_PATH)).unwrap()).unwrap();
        assert_eq!(m["files"][0]["size"], html.len());
        assert_eq!(m["files"][0]["sha256"], sha256_hex(&html));
        assert_eq!(run_target(&(opts, root.to_path_buf(), t)).status, "UNCHANGED");
    }
}
