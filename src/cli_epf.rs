//! Command line interface of `epf-extract` / `epf-pack` (port of `cli.py`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Arg, ArgAction, Command};
use serde_json::{json, Map, Value};

use crate::canonical::{dumps_json, dumps_with, SORTED_SPACED};
use crate::config::{load_json_bytes, validate_codec, validate_override_key, validate_root_config};
use crate::discovery::{discover_all, find_epfs, resolve_selectors, CONFIG_NAME};
use crate::errors::{ExtractionError, Result, CONFIG_INVALID, IO_ERROR};
use crate::manifest::{tools_record, MANIFEST_PATH};
use crate::pack::{publish_pack, stage_pack, PACK_UNCHANGED};
use crate::props;
use crate::stage::{stage_processor, StageOpts, STATUS_PARTIAL};
use crate::transaction::{
    check_ownership, plan, processor_lock, publish, read_tree, recover, snapshot_epf, Snapshot, Tree,
};

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_DIFFERS: i32 = 3;

#[derive(Debug, Clone)]
pub struct Outcome {
    pub path: String,
    pub status: &'static str,
    pub code: Option<&'static str>,
    pub message: String,
    pub params: Map<String, Value>,
    pub summary: String,
    pub lines: Vec<String>,
    pub duration: f64,
}

impl Outcome {
    pub fn new(path: impl Into<String>, status: &'static str) -> Outcome {
        Outcome {
            path: path.into(),
            status,
            code: None,
            message: String::new(),
            params: Map::new(),
            summary: String::new(),
            lines: Vec::new(),
            duration: 0.0,
        }
    }

    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

pub fn describe(files: &Tree, status: &str) -> String {
    let manifest: Value = serde_json::from_slice(&files[MANIFEST_PATH]).unwrap_or(Value::Null);
    let warnings = manifest["diagnostics"]
        .as_array()
        .map(|a| a.iter().filter(|d| d["severity"] == "warning").count())
        .unwrap_or(0);
    let forms = manifest["forms"].as_array().map(Vec::len).unwrap_or(0);
    let templates = manifest["templates"].as_array().map(Vec::len).unwrap_or(0);
    let reference = if status == STATUS_PARTIAL { "reference partial" } else { "reference complete" };
    format!(
        "code complete, {}, {}, {reference} ({})",
        plural(forms, "form"),
        plural(templates, "template"),
        plural(warnings, "warning"),
    )
}

pub fn existing_source_sha(existing: &Tree) -> Option<String> {
    let raw = existing.get(MANIFEST_PATH)?;
    let manifest = load_json_bytes(raw, MANIFEST_PATH).ok()?;
    manifest.get("source")?.get("sha256")?.as_str().map(str::to_string)
}

pub fn plan_lines(added: &[String], updated: &[String], deleted: &[String]) -> Vec<String> {
    added
        .iter()
        .map(|p| format!("  add {p}"))
        .chain(updated.iter().map(|p| format!("  update {p}")))
        .chain(deleted.iter().map(|p| format!("  delete {p}")))
        .collect()
}

pub struct EpfArgs {
    pub root: String,
    pub processor: Vec<String>,
    pub all: bool,
    pub check: bool,
    pub dry_run: bool,
    pub pack: bool,
    pub backup: bool,
    pub no_backup: bool,
    pub encoding: Vec<String>,
    pub form_backend: Option<String>,
    pub form_xml: bool,
    pub oof_path: Option<String>,
    pub keep_work: bool,
    pub replace_generated: bool,
    pub report_json: Option<String>,
}

const EPILOG: &str = "\
Outputs under <processor>/src are REFERENCE-ONLY projections for review and diffs.
Nothing is written back to an EPF unless you ask for it with --pack (epf-pack): then
edited ObjectModule.bsl, FormModule.bsl, FormStream.bin, Template.html and Template.bin
files are written back into the EPF and src/ is regenerated from the packed EPF.
Definition files, Form.xml (--form-backend oof) and the manifest are never packed. One-off --encoding overrides are not saved; move accepted
values to the processor extraction.json. Exit codes: 0 ok, 1 extraction/publication
failure, 2 CLI/config error, 3 --check found differences.";

fn build_command(prog: &'static str) -> Command {
    Command::new(prog)
        .version(env!("CARGO_PKG_VERSION"))
        .after_help(EPILOG)
        .arg(Arg::new("root").long("root").default_value(".").value_name("PATH"))
        .arg(Arg::new("processor").long("processor").action(ArgAction::Append).value_name("PATH"))
        .arg(Arg::new("all").long("all").action(ArgAction::SetTrue))
        .arg(Arg::new("check").long("check").action(ArgAction::SetTrue))
        .arg(Arg::new("dry_run").long("dry-run").action(ArgAction::SetTrue))
        .arg(Arg::new("pack").long("pack").action(ArgAction::SetTrue))
        .arg(Arg::new("backup").long("backup").action(ArgAction::SetTrue))
        .arg(Arg::new("no_backup").long("no-backup").action(ArgAction::SetTrue))
        .arg(Arg::new("encoding").long("encoding").action(ArgAction::Append).value_name("KEY=CODEC"))
        .arg(
            Arg::new("form_backend")
                .long("form-backend")
                .value_parser(["basic", "v8unpack-json", "oof"]),
        )
        .arg(Arg::new("form_xml").long("form-xml").action(ArgAction::SetTrue))
        .arg(Arg::new("oof_path").long("oof-path").value_name("PATH"))
        .arg(Arg::new("keep_work").long("keep-work").action(ArgAction::SetTrue))
        .arg(Arg::new("replace_generated").long("replace-generated").action(ArgAction::SetTrue))
        .arg(Arg::new("report_json").long("report-json").value_name("PATH"))
}

fn parse_args(argv: &[String], prog: &'static str) -> std::result::Result<EpfArgs, i32> {
    let mut full = vec![prog.to_string()];
    full.extend_from_slice(argv);
    let m = match build_command(prog).try_get_matches_from(full) {
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
    let processor: Vec<String> = m.get_many::<String>("processor").map(|v| v.cloned().collect()).unwrap_or_default();
    let all = m.get_flag("all");
    if (processor.is_empty() && !all) || (!processor.is_empty() && all) {
        eprintln!("{prog}: error: one of the arguments --processor --all is required");
        return Err(EXIT_USAGE);
    }
    let check = m.get_flag("check");
    let dry_run = m.get_flag("dry_run");
    if check && dry_run {
        eprintln!("{prog}: error: argument --dry-run: not allowed with argument --check");
        return Err(EXIT_USAGE);
    }
    let backup = m.get_flag("backup");
    let no_backup = m.get_flag("no_backup");
    if backup && no_backup {
        eprintln!("{prog}: error: argument --no-backup: not allowed with argument --backup");
        return Err(EXIT_USAGE);
    }
    let pack = m.get_flag("pack");
    let replace_generated = m.get_flag("replace_generated");
    if pack && (check || replace_generated) {
        eprintln!("{prog}: error: --pack cannot be combined with --check or --replace-generated");
        return Err(EXIT_USAGE);
    }
    if (backup || no_backup) && !pack {
        eprintln!("{prog}: error: --backup/--no-backup require --pack");
        return Err(EXIT_USAGE);
    }
    Ok(EpfArgs {
        root: m.get_one::<String>("root").cloned().unwrap_or_else(|| ".".into()),
        processor,
        all,
        check,
        dry_run,
        pack,
        backup,
        no_backup,
        encoding: m.get_many::<String>("encoding").map(|v| v.cloned().collect()).unwrap_or_default(),
        form_backend: m.get_one::<String>("form_backend").cloned(),
        form_xml: m.get_flag("form_xml"),
        oof_path: m.get_one::<String>("oof_path").cloned(),
        keep_work: m.get_flag("keep_work"),
        replace_generated,
        report_json: m.get_one::<String>("report_json").cloned(),
    })
}

fn process_one(
    args: &EpfArgs,
    root: &Path,
    proc_rel: &str,
    root_cfg: &Value,
    tools_lock: &[u8],
    overrides: &[(String, String)],
) -> Result<Outcome> {
    let mut pdir = root.to_path_buf();
    for part in proc_rel.split('/') {
        pdir.push(part);
    }
    let cfg_path = pdir.join(CONFIG_NAME);
    if !cfg_path.is_file() {
        return Err(ExtractionError::new(CONFIG_INVALID, "processor extraction.json is required").with("processor", proc_rel));
    }
    let proc_cfg = load_json_bytes(&fs::read(&cfg_path)?, &format!("{proc_rel}/{CONFIG_NAME}"))?;
    let epfs = find_epfs(&pdir)?;
    if epfs.len() != 1 {
        let found: Vec<Value> = epfs.into_iter().map(Value::from).collect();
        return Err(ExtractionError::new(CONFIG_INVALID, "exactly one EPF is required")
            .with("processor", proc_rel)
            .with("found", Value::Array(found)));
    }
    let epf_name = &epfs[0];
    let epf_path = pdir.join(epf_name);
    let src = pdir.join("src");

    let _lock = processor_lock(root, proc_rel)?;
    let mut notes: Vec<String> = if !(args.check || args.dry_run) {
        recover(root, proc_rel)?.into_iter().map(|a| format!("recovered: {a}")).collect()
    } else {
        Vec::new()
    };
    let snap = snapshot_epf(&epf_path)?;
    let ov_refs: Vec<(&str, &str)> = overrides.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let opts = StageOpts {
        cli_overrides: &ov_refs,
        form_backend: args.form_backend.as_deref(),
        oof_path: args.oof_path.as_deref(),
        allow_managed: false,
        use_original_names: false,
    };
    if args.pack {
        return pack_one(args, root, proc_rel, &proc_cfg, root_cfg, tools_lock, &opts, epf_name, &epf_path, &src, &snap, notes);
    }
    let result = stage_processor(&snap.data, epf_name, &proc_cfg, root_cfg, tools_lock, &opts)?;
    let summary = describe(&result.files, &result.status);
    if args.check {
        let existing = read_tree(&src)?;
        let (added, updated, deleted) = plan(&existing, &result.files);
        if added.is_empty() && updated.is_empty() && deleted.is_empty() {
            let mut out = Outcome::new(proc_rel, "OK");
            out.summary = summary;
            out.lines = notes;
            return Ok(out);
        }
        let counts = format!("(+{} ~{} -{})", added.len(), updated.len(), deleted.len());
        let (status, sum) = if existing_source_sha(&existing).as_deref() != Some(snap.sha256.as_str()) {
            ("STALE", format!("existing src is stale for current EPF {counts}"))
        } else {
            ("DIFFERS", format!("src differs from the current extraction {counts}"))
        };
        let mut out = Outcome::new(proc_rel, status);
        out.summary = sum;
        notes.extend(plan_lines(&added, &updated, &deleted));
        out.lines = notes;
        return Ok(out);
    }
    if args.dry_run {
        let existing = read_tree(&src)?;
        let (added, updated, deleted) = plan(&existing, &result.files);
        if added.is_empty() && updated.is_empty() && deleted.is_empty() {
            let mut out = Outcome::new(proc_rel, "UNCHANGED");
            out.lines = notes;
            return Ok(out);
        }
        check_ownership(&existing, args.replace_generated)?;
        let mut out = Outcome::new(proc_rel, "WOULD_UPDATE");
        out.summary = format!("+{} ~{} -{}", added.len(), updated.len(), deleted.len());
        notes.extend(plan_lines(&added, &updated, &deleted));
        out.lines = notes;
        return Ok(out);
    }
    let pub_res = publish(root, proc_rel, &result.files, &epf_path, &snap, args.keep_work, args.replace_generated)?;
    if let Some(ref rd) = pub_res.run_dir {
        notes.push(format!("work kept: {}", rd.display()));
    }
    let mut out = Outcome::new(proc_rel, pub_res.status);
    out.summary = summary;
    out.lines = notes;
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn pack_one(
    args: &EpfArgs,
    root: &Path,
    proc_rel: &str,
    proc_cfg: &Value,
    root_cfg: &Value,
    tools_lock: &[u8],
    opts: &StageOpts<'_>,
    epf_name: &str,
    epf_path: &Path,
    src: &Path,
    snap: &Snapshot,
    mut notes: Vec<String>,
) -> Result<Outcome> {
    let plan_ = stage_pack(&snap.data, epf_name, &read_tree(src)?, proc_cfg, root_cfg, tools_lock, opts)?;
    let summary = describe(&plan_.result.files, &plan_.result.status);
    let packed: Vec<String> = plan_.packed_paths.iter().map(|p| format!("  pack {p}")).collect();
    if plan_.status == PACK_UNCHANGED {
        let mut out = Outcome::new(proc_rel, "UNCHANGED");
        out.lines = notes;
        return Ok(out);
    }
    let (added, updated, deleted) = plan(&plan_.existing, &plan_.result.files);
    if args.dry_run {
        if !packed.is_empty() {
            let mut out = Outcome::new(proc_rel, "WOULD_PACK");
            out.summary = plural(packed.len(), "file");
            notes.extend(packed);
            out.lines = notes;
            return Ok(out);
        }
        let mut out = Outcome::new(proc_rel, "WOULD_UPDATE");
        out.summary = format!("+{} ~{} -{}", added.len(), updated.len(), deleted.len());
        notes.extend(plan_lines(&added, &updated, &deleted));
        out.lines = notes;
        return Ok(out);
    }
    let pub_res = publish_pack(root, proc_rel, &plan_, epf_path, snap, args.backup, args.keep_work)?;
    if let Some(ref rd) = pub_res.run_dir {
        notes.push(format!("work kept: {}", rd.display()));
    }
    if !packed.is_empty() {
        let mut out = Outcome::new(proc_rel, "PACKED");
        out.summary = format!("{}, {summary}", plural(packed.len(), "file"));
        notes.extend(packed);
        if args.backup {
            notes.push(format!("  backup {epf_name}.bak"));
        }
        out.lines = notes;
        return Ok(out);
    }
    let mut out = Outcome::new(proc_rel, "UPDATED");
    out.summary = summary;
    out.lines = notes;
    Ok(out)
}

fn safe_process(
    args: &EpfArgs,
    root: &Path,
    proc_rel: &str,
    root_cfg: &Value,
    tools_lock: &[u8],
    overrides: &[(String, String)],
) -> Outcome {
    let start = Instant::now();
    let mut out = match process_one(args, root, proc_rel, root_cfg, tools_lock, overrides) {
        Ok(o) => o,
        Err(e) => {
            let mut o = Outcome::new(proc_rel, "FAILED");
            o.code = Some(e.code);
            o.message = e.message;
            o.params = e.params;
            o
        }
    };
    out.duration = start.elapsed().as_secs_f64();
    out
}

pub fn print_outcome(o: &Outcome, dry_run: bool) {
    let name = o.name();
    if o.status == "FAILED" {
        println!("{name}: FAILED, {}, existing src unchanged", o.code.unwrap_or(""));
        println!("  {}", o.message);
        if !o.params.is_empty() {
            println!("  {}", dumps_with(&Value::Object(o.params.clone()), &SORTED_SPACED).unwrap_or_default());
        }
        return;
    }
    match o.status {
        "UNCHANGED" => println!("{name}: {}", if dry_run { "NO CHANGES" } else { "UNCHANGED" }),
        "UPDATED" => println!("{name}: UPDATED, {}", o.summary),
        "PACKED" => println!("{name}: PACKED, {}", o.summary),
        "WOULD_PACK" => println!("{name}: WOULD PACK ({})", o.summary),
        "OK" => println!("{name}: OK, {}", o.summary),
        "DIFFERS" | "STALE" => println!("{name}: {}, {}", o.status, o.summary),
        "WOULD_UPDATE" => println!("{name}: WOULD UPDATE ({})", o.summary),
        _ => {}
    }
    for line in &o.lines {
        println!("{line}");
    }
}

fn exit_code(outcomes: &[Outcome], args: &EpfArgs) -> i32 {
    let failed: Vec<&Outcome> = outcomes.iter().filter(|o| o.status == "FAILED").collect();
    if failed.iter().any(|o| o.code == Some(CONFIG_INVALID)) {
        return EXIT_USAGE;
    }
    if !failed.is_empty() {
        return EXIT_FAILED;
    }
    if args.check && outcomes.iter().any(|o| matches!(o.status, "DIFFERS" | "STALE")) {
        return EXIT_DIFFERS;
    }
    EXIT_OK
}

fn batch_line(outcomes: &[Outcome], args: &EpfArgs, code: i32) -> String {
    let n = |statuses: &[&str]| outcomes.iter().filter(|o| statuses.contains(&o.status)).count();
    let body = if args.check {
        format!("{} ok, {} differ, {} failed", n(&["OK"]), n(&["DIFFERS", "STALE"]), n(&["FAILED"]))
    } else if args.pack && args.dry_run {
        format!(
            "{} would pack, {} would update, {} unchanged, {} failed",
            n(&["WOULD_PACK"]),
            n(&["WOULD_UPDATE"]),
            n(&["UNCHANGED"]),
            n(&["FAILED"])
        )
    } else if args.pack {
        format!(
            "{} packed, {} updated, {} unchanged, {} failed",
            n(&["PACKED"]),
            n(&["UPDATED"]),
            n(&["UNCHANGED"]),
            n(&["FAILED"])
        )
    } else if args.dry_run {
        format!("{} would update, {} unchanged, {} failed", n(&["WOULD_UPDATE"]), n(&["UNCHANGED"]), n(&["FAILED"]))
    } else {
        format!("{} updated, {} unchanged, {} failed", n(&["UPDATED"]), n(&["UNCHANGED"]), n(&["FAILED"]))
    };
    format!("Batch: {body}; exit {code}")
}

fn fail_setup(message: &str) -> i32 {
    eprintln!("epf-extract: error: {message}");
    EXIT_USAGE
}

fn parse_overrides(items: &[String]) -> Result<Vec<(String, String)>> {
    let mut result: Vec<(String, String)> = Vec::new();
    for item in items {
        let Some((key, codec)) = item.split_once('=') else {
            return Err(ExtractionError::new(CONFIG_INVALID, "--encoding expects KEY=CODEC").with("value", item.as_str()));
        };
        validate_override_key(key)?;
        let c = validate_codec(&Value::from(codec), &format!("--encoding {key}"))?;
        if let Some(slot) = result.iter_mut().find(|(k, _)| k == key) {
            slot.1 = c;
        } else {
            result.push((key.to_string(), c));
        }
    }
    Ok(result)
}

pub fn clean_path(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p)
    };
    if let Ok(c) = abs.canonicalize() {
        let s = c.to_string_lossy();
        return PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s));
    }
    let mut out = PathBuf::new();
    for comp in abs.components() {
        use std::path::Component::*;
        match comp {
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn write_report(path: &Path, mode: &str, code: i32, outcomes: &[Outcome], overrides: &[(String, String)]) -> Result<()> {
    let mut ov = Map::new();
    for (k, v) in overrides {
        ov.insert(k.clone(), Value::from(v.as_str()));
    }
    let procs: Vec<Value> = outcomes
        .iter()
        .map(|o| {
            let dur = (o.duration * 1000.0).round() / 1000.0;
            json!({
                "path": o.path,
                "status": o.status,
                "code": o.code,
                "message": o.message,
                "summary": o.summary,
                "durationSeconds": dur,
            })
        })
        .collect();
    let report = json!({
        "schemaVersion": 1,
        "mode": mode,
        "exitCode": code,
        "cliEncodingOverrides": Value::Object(ov),
        "processors": procs,
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, dumps_json(&report)?)?;
    Ok(())
}

pub fn main_with_prog(argv: &[String], prog: &'static str) -> i32 {
    if argv.first().map(String::as_str) == Some("props") {
        let (code, out, err) = props::run(&argv[1..]);
        print!("{out}");
        eprint!("{err}");
        return code;
    }
    let mut args = match parse_args(argv, prog) {
        Ok(a) => a,
        Err(code) => return code,
    };
    if args.form_xml {
        if !matches!(args.form_backend.as_deref(), None | Some("oof")) {
            return fail_setup(&format!("--form-xml conflicts with --form-backend {}", args.form_backend.unwrap_or_default()));
        }
        args.form_backend = Some("oof".into());
    }
    if args.form_backend.as_deref() == Some("v8unpack-json") {
        return fail_setup("form backend 'v8unpack-json' is not implemented; use 'basic' or 'oof'");
    }
    if args.oof_path.is_none() {
        args.oof_path = std::env::var("EPF_OOF_PATH").ok().filter(|s| !s.is_empty());
    }

    let root = clean_path(Path::new(&args.root));
    type Setup = (Value, Vec<u8>, Vec<(String, String)>, Vec<String>, Option<PathBuf>);
    let setup = (|| -> Result<Setup> {
        let root_file = root.join(CONFIG_NAME);
        if !root_file.is_file() {
            return Err(ExtractionError::new(
                "SETUP",
                format!("{} not found; run from the repository root or pass --root", root_file.display()),
            ));
        }
        let root_cfg = load_json_bytes(&fs::read(&root_file)?, CONFIG_NAME)?;
        let processor_root = validate_root_config(&root_cfg)?.processor_root;
        let lock_file = root.join("tools.lock.json");
        if !lock_file.is_file() {
            return Err(ExtractionError::new("SETUP", format!("{} not found", lock_file.display())));
        }
        let tools_lock = fs::read(&lock_file)?;
        tools_record(&tools_lock, "basic")?;
        let overrides = parse_overrides(&args.encoding)?;
        let processors = if args.all {
            discover_all(&root, &processor_root)?
        } else {
            resolve_selectors(&root, &processor_root, &args.processor)?
        };
        let mut report_path = None;
        if let Some(ref rj) = args.report_json {
            let rp = clean_path(Path::new(rj));
            let mut pr = root.clone();
            for part in processor_root.split('/') {
                pr.push(part);
            }
            if let Ok(rel) = rp.strip_prefix(&pr) {
                if rel.components().any(|c| c.as_os_str() == "src") {
                    return Err(ExtractionError::new("SETUP", "--report-json must not point inside a src/ directory"));
                }
            }
            report_path = Some(rp);
        }
        Ok((root_cfg, tools_lock, overrides, processors, report_path))
    })();

    let (root_cfg, tools_lock, overrides, processors, report_path) = match setup {
        Ok(v) => v,
        Err(e) if e.code == "SETUP" => return fail_setup(&e.message),
        Err(e) if e.code == IO_ERROR => return fail_setup(&e.message),
        Err(e) => {
            let details = if e.params.is_empty() {
                String::new()
            } else {
                format!(" {}", dumps_with(&Value::Object(e.params), &SORTED_SPACED).unwrap_or_default())
            };
            return fail_setup(&format!("{}: {}{details}", e.code, e.message));
        }
    };

    let mode = if args.check {
        "check"
    } else if args.pack && args.dry_run {
        "pack-dry-run"
    } else if args.pack {
        "pack"
    } else if args.dry_run {
        "dry-run"
    } else {
        "extract"
    };
    let mut outcomes = Vec::new();
    for proc_rel in &processors {
        let out = safe_process(&args, &root, proc_rel, &root_cfg, &tools_lock, &overrides);
        print_outcome(&out, args.dry_run);
        outcomes.push(out);
    }
    let code = exit_code(&outcomes, &args);
    if !overrides.is_empty() {
        let mut sorted: Vec<(&str, &str)> = overrides.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        sorted.sort_by_key(|(k, _)| *k);
        let listed = sorted.into_iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ");
        println!(
            "Note: one-off CLI overrides were used ({listed}); they are not saved. \
             Move accepted values to the processor extraction.json (encodingOverrides) \
             so the next run and --check reproduce this result."
        );
    }
    println!("{}", batch_line(&outcomes, &args, code));
    if let Some(ref rp) = report_path {
        if let Err(e) = write_report(rp, mode, code, &outcomes, &overrides) {
            eprintln!("epf-extract: error: cannot write report: {e}");
            return EXIT_USAGE;
        }
    }
    code
}

pub fn pack_main(argv: &[String]) -> i32 {
    let mut full = vec!["--pack".to_string()];
    full.extend_from_slice(argv);
    main_with_prog(&full, "epf-pack")
}
