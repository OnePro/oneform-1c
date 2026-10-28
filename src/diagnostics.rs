//! Diagnostics log and `oneform diagnose`.
//!
//! `extract`, `pack`, `clone-form`, `new-form` and `history restore` that end with a refusal, a failure or warnings append
//! one JSON line (metadata only: no file contents) to `<project dir>/.oneform/diagnostics.jsonl`; the project dir is the
//! object directory of a configuration form or the directory of the `.epf`. The log keeps the last 500 lines
//! (`ONEFORM_DIAG_CAP`). Writing it never changes the result of the command: a failure to write is silent.
//! `oneform diagnose [PATH] [--json]` reads the log, the last history snapshot and the `validate` verdicts (read-only).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::cli_epf::{clean_path, Outcome, EXIT_OK, EXIT_USAGE};
use crate::cli_oneform::{discover_targets, path_of};
use crate::history;
use crate::validate;

pub const DIAG_NAME: &str = "diagnostics.jsonl";
pub const CAP_ENV: &str = "ONEFORM_DIAG_CAP";
pub const DEFAULT_CAP: usize = 500;
/// Entries shown by `diagnose` and copied into a feedback report.
pub const RECENT: usize = 10;
const WARNINGS_KEPT: usize = 5;
const MESSAGE_MAX: usize = 300;
const GUIDE: &str = "docs/agent-guide.md";

/// `<project dir>/.oneform/diagnostics.jsonl`.
pub fn diag_path(project: &Path) -> PathBuf {
    project.join(".oneform").join(DIAG_NAME)
}

static ARGV: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// The command line of this run (recorded in the log lines, with absolute paths made relative).
pub fn set_argv(argv: &[String]) {
    let _ = ARGV.set(argv.to_vec());
}

fn argv() -> Vec<String> {
    ARGV.get().cloned().unwrap_or_default()
}

fn cap() -> usize {
    std::env::var(CAP_ENV).ok().and_then(|v| v.trim().parse::<usize>().ok()).filter(|n| *n > 0).unwrap_or(DEFAULT_CAP)
}

/// What one log line says.
#[derive(Debug, Clone, Default)]
pub struct Record {
    pub command: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub code: Option<String>,
    pub kind: Option<String>,
    pub message: String,
    pub warnings: Vec<String>,
    pub target: Option<String>,
    pub form: Option<String>,
    pub dry_run: bool,
}

fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.chars().count() > MESSAGE_MAX {
        line.chars().take(MESSAGE_MAX).collect::<String>() + "..."
    } else {
        line.to_string()
    }
}

/// An absolute path becomes relative to the working directory (or its last component): no absolute paths in the log.
fn relative_arg(arg: &str) -> String {
    let p = Path::new(arg);
    if !p.is_absolute() {
        return arg.to_string();
    }
    let rel = std::env::current_dir().ok().and_then(|cwd| p.strip_prefix(&cwd).ok().map(Path::to_path_buf));
    match rel {
        Some(r) if r.as_os_str().is_empty() => ".".to_string(),
        Some(r) => r.to_string_lossy().replace('\\', "/"),
        None => format!(".../{}", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
    }
}

/// `stream version N` of a message of `unsupported-stream-version`.
fn stream_version(message: &str) -> Option<i64> {
    let rest = message.split("stream version ").nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn to_json(r: &Record) -> Value {
    let mut warnings: Vec<String> = r.warnings.iter().map(|w| first_line(w)).collect();
    let count = warnings.len();
    warnings.truncate(WARNINGS_KEPT);
    json!({
        "time": history::time_text(history::local_secs()),
        "command": r.command,
        "args": r.args.iter().map(|a| relative_arg(a)).collect::<Vec<_>>(),
        "exit_code": r.exit_code,
        "code": r.code,
        "kind": r.kind,
        "message": first_line(&r.message),
        "warning_count": count,
        "warnings": warnings,
        "target": r.target,
        "form": r.form,
        "stream_version": stream_version(&r.message),
        "oneform_version": env!("CARGO_PKG_VERSION"),
        "dry_run": r.dry_run,
    })
}

/// Append one line to the log of `project`, then drop the oldest lines beyond the cap. Never fails the caller.
pub fn append(project: &Path, record: &Record) {
    // the project directory is an absolute path: it is written as `.`
    let dir = project.to_string_lossy().into_owned();
    let hide = |t: &str| t.replace(&dir, ".").replace(&dir.replace(std::path::MAIN_SEPARATOR, "/"), ".");
    let mut r = record.clone();
    r.message = hide(&r.message);
    r.warnings = r.warnings.iter().map(|w| hide(w)).collect();
    let _ = try_append(project, &r);
}

fn try_append(project: &Path, record: &Record) -> std::io::Result<()> {
    if !project.is_dir() {
        return Ok(());
    }
    let path = diag_path(project);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let old = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = old.lines().filter(|l| !l.trim().is_empty()).collect();
    let line = serde_json::to_string(&to_json(record)).map_err(std::io::Error::other)?;
    lines.push(&line);
    let keep = cap();
    let from = lines.len().saturating_sub(keep);
    let mut text = lines[from..].join("\n");
    text.push('\n');
    fs::write(&path, text)
}

/// `Forms/<Form>/Form.xml` -> `<Form>`.
fn form_of_path(path: Option<&str>) -> Option<String> {
    let mut parts = path?.split('/');
    (parts.next() == Some("Forms")).then(|| parts.next().map(str::to_string)).flatten()
}

/// A record of a processed target that ended with a failure or warnings (`None` for a clean outcome). A dry run keeps
/// only refusals.
pub fn from_outcome(command: &str, dry_run: bool, exit_code: i32, o: &Outcome) -> Option<Record> {
    let warnings: Vec<String> = o.lines.iter().map(|l| l.trim().to_string()).filter(|l| l.starts_with("warning")).collect();
    let failed = o.status == "FAILED";
    if !failed && (warnings.is_empty() || dry_run) {
        return None;
    }
    Some(Record {
        command: command.to_string(),
        args: argv(),
        exit_code,
        code: o.code.map(str::to_string),
        kind: o.params.get("kind").and_then(Value::as_str).map(str::to_string),
        message: if failed { o.message.clone() } else { String::new() },
        warnings,
        target: Some(o.path.clone()),
        form: o.params.get("form").and_then(Value::as_str).map(str::to_string).or_else(|| form_of_path(o.params.get("path").and_then(Value::as_str))),
        dry_run,
    })
}

/// A record of a refused `clone-form` / `new-form` / `history restore`.
pub fn from_error(command: &str, dry_run: bool, exit_code: i32, code: Option<&str>, kind: Option<&str>, message: &str) -> Record {
    Record {
        command: command.to_string(),
        args: argv(),
        exit_code,
        code: code.map(str::to_string),
        kind: kind.map(str::to_string),
        message: message.to_string(),
        dry_run,
        ..Default::default()
    }
}

/// The last `n` log lines of `project`, oldest first (unreadable lines are skipped).
pub fn recent(project: &Path, n: usize) -> Vec<Value> {
    let text = fs::read_to_string(diag_path(project)).unwrap_or_default();
    let all: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect();
    let from = all.len().saturating_sub(n);
    all[from..].to_vec()
}

/// Project directories below `root`: object directories, directories of `.epf` files, and every directory that already
/// has a history, a log or a feedback folder (including `root` itself).
pub fn project_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for t in discover_targets(root, "extract") {
        dirs.insert(path_of(root, &t.rel));
    }
    for (dir, _) in history::find_histories(root) {
        dirs.insert(dir);
    }
    if diag_path(root).is_file() || root.join(".oneform/feedback").is_dir() {
        dirs.insert(root.to_path_buf());
    }
    dirs.into_iter().collect()
}

// --------------------------------------------------------------------------- hints

/// Error code or kind -> the section of `docs/agent-guide.md` that explains it. The first matching key wins.
const HINTS: [(&str, &str); 14] = [
    ("unsupported-stream-version", "Форма старой версии потока (25 / 26): что сказать пользователю"),
    ("unsupported-button", "Каталог стандартных команд (`Standard:N`)"),
    ("unsupported-panel", "Типичные отказы и что делать"),
    ("bsl-procedure-after-body", "Типичные отказы и что делать"),
    ("FORM_EDIT_REJECTED", "Типичные отказы и что делать"),
    ("PUBLISH_VERIFY_FAILED", "Типичные отказы и что делать"),
    ("GENERATED_FILE_MODIFIED", "Изменить форму из выгрузки конфигурации (справочник, документ, обработка)"),
    ("SOURCE_CHANGED", "Изменить форму из выгрузки конфигурации (справочник, документ, обработка)"),
    ("OUTPUT_NOT_OWNED", "Изменить форму из выгрузки конфигурации (справочник, документ, обработка)"),
    ("MANAGED_FORM_UNSUPPORTED", "Область применимости: `oneform validate`"),
    ("METADATA_LAYOUT_UNSUPPORTED", "Область применимости: `oneform validate`"),
    ("CONTAINER_CORRUPT", "Область применимости: `oneform validate`"),
    ("IO_ERROR", "Типичные отказы и что делать"),
    ("INTERNAL_ERROR", "Что гарантируется и что проверять в 1С"),
];
const GENERIC_HINT: &str = "Типичные отказы и что делать";

/// The section of the guide for an error `code` / `kind` (unknown: the generic table of refusals).
pub fn hint_section(code: &str, kind: &str) -> &'static str {
    HINTS
        .iter()
        .find(|(k, _)| (!kind.is_empty() && *k == kind) || (!code.is_empty() && *k == code))
        .map_or(GENERIC_HINT, |(_, s)| s)
}

// --------------------------------------------------------------------------- diagnose

pub(crate) fn entry_line(e: &Value) -> String {
    let s = |k: &str| e[k].as_str().unwrap_or("");
    let what = match (s("code"), s("kind")) {
        ("", _) => format!("{} warning(s)", e["warning_count"].as_u64().unwrap_or(0)),
        (c, "") => c.to_string(),
        (c, k) => format!("{c}/{k}"),
    };
    let dry = if e["dry_run"].as_bool() == Some(true) { " (dry-run)" } else { "" };
    let msg = if s("message").is_empty() { s("warnings").to_string() } else { s("message").to_string() };
    let msg = if msg.is_empty() { e["warnings"][0].as_str().unwrap_or("").to_string() } else { msg };
    format!("  {}  {} exit {}{dry}  {what}: {msg}", s("time"), s("command"), e["exit_code"])
}

/// Commands of the "the form did not open in 1C" checklist.
pub fn checklist(id: &str, path: &str) -> Vec<String> {
    let p = if path == "." { String::new() } else { format!("{path} ") };
    vec![
        format!("oneform history diff {p}{id}"),
        format!("oneform history restore {p}{id}"),
        format!("oneform feedback add {p}--history {id} --1c-error \"<текст ошибки 1С>\""),
    ]
}

/// `oneform diagnose [PATH] [--json]`: read-only.
pub fn run_cli(path: &str, json_output: bool) -> i32 {
    let root = clean_path(Path::new(path));
    if !root.is_dir() {
        eprintln!("oneform: error: {} is not a directory", root.display());
        return EXIT_USAGE;
    }
    let projects = project_dirs(&root);
    let mut entries: Vec<(String, Value)> = Vec::new();
    for p in &projects {
        let label = history::rel_of(&root, p);
        entries.extend(recent(p, RECENT).into_iter().map(|e| (label.clone(), e)));
    }
    entries.sort_by(|a, b| a.1["time"].as_str().cmp(&b.1["time"].as_str()));
    let from = entries.len().saturating_sub(RECENT);
    let entries = &entries[from..];

    let last = history::latest_under(&root);
    let id = last.as_ref().map_or("<id>".to_string(), |(_, i)| i.id.clone());
    let verdicts = validate::validate_root(&root, 1, None);

    let mut hints: Vec<(String, String, &'static str)> = Vec::new();
    for (_, e) in entries {
        let (code, kind) = (e["code"].as_str().unwrap_or(""), e["kind"].as_str().unwrap_or(""));
        if code.is_empty() && kind.is_empty() {
            continue;
        }
        let item = (code.to_string(), kind.to_string(), hint_section(code, kind));
        if !hints.contains(&item) {
            hints.push(item);
        }
    }
    let commands = checklist(&id, path);

    if json_output {
        let validate_json = match &verdicts {
            Ok(v) => v.iter().map(validate::verdict_json).collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };
        let out = json!({
            "oneform_version": env!("CARGO_PKG_VERSION"),
            "root": root.to_string_lossy(),
            "diagnostics": entries.iter().map(|(p, e)| {
                let mut e = e.clone();
                e["project"] = Value::from(p.as_str());
                e
            }).collect::<Vec<_>>(),
            "last_snapshot": last.as_ref().map(|(dir, i)| json!({
                "id": i.id,
                "object_dir": history::rel_of(&root, dir),
                "form": i.manifest["form"],
                "time": i.manifest["time"],
                "summary": i.manifest["summary"],
            })),
            "validate": validate_json,
            "validate_error": verdicts.as_ref().err().map(|e| e.to_string()),
            "hints": hints.iter().map(|(c, k, s)| json!({"code": c, "kind": k, "doc": GUIDE, "section": s})).collect::<Vec<_>>(),
            "checklist": {"title": "Форма не открылась в 1С", "commands": commands},
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return EXIT_OK;
    }

    println!("oneform diagnose {} (oneform {})", root.display(), env!("CARGO_PKG_VERSION"));
    println!("Последние записи диагностики ({}):", entries.len());
    if entries.is_empty() {
        println!("  нет (отказов и предупреждений не записано)");
    }
    for (_, e) in entries {
        println!("{}", entry_line(e));
    }
    match &last {
        Some((dir, i)) => {
            let summary: Vec<&str> = i.manifest["summary"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            println!(
                "Последний снимок истории: {} (форма {}, {}), {}",
                i.id,
                i.manifest["form"].as_str().unwrap_or(""),
                history::rel_of(&root, dir),
                summary.join("; ")
            );
        }
        None => println!("Последний снимок истории: нет (pack ещё не менял форму)"),
    }
    println!("Проверка форм (validate):");
    match &verdicts {
        Ok(v) if v.is_empty() => println!("  форм нет"),
        Ok(v) => v.iter().for_each(|x| println!("  {}", validate::verdict_line(x))),
        Err(e) => println!("  validate не выполнен: {e}"),
    }
    if !hints.is_empty() {
        println!("Что почитать ({GUIDE}):");
        for (c, k, s) in &hints {
            let name = if k.is_empty() { c.clone() } else if c.is_empty() { k.clone() } else { format!("{c}/{k}") };
            println!("  {name}: раздел «{s}»");
        }
    }
    println!("Форма не открылась в 1С:");
    for (i, c) in commands.iter().enumerate() {
        let what = ["что изменил последний pack", "вернуть прежнюю форму", "сохранить отчёт для разработчика oneform с текстом ошибки 1С"][i];
        println!("  {}. {c}   # {what}", i + 1);
    }
    EXIT_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_versions_and_hints() {
        assert_eq!(first_line("\n  a b\nsecond"), "a b");
        assert_eq!(stream_version("line 3: adding Table to forms of stream version 26 is not supported"), Some(26));
        assert_eq!(stream_version("no version"), None);
        assert_eq!(hint_section("FORM_EDIT_REJECTED", "unsupported-stream-version"), "Форма старой версии потока (25 / 26): что сказать пользователю");
        assert_eq!(hint_section("SOURCE_CHANGED", ""), "Изменить форму из выгрузки конфигурации (справочник, документ, обработка)");
        assert_eq!(hint_section("SOMETHING_NEW", "x"), GENERIC_HINT);
    }

    #[test]
    fn the_log_is_capped_and_a_missing_directory_is_silent() {
        let t = tempfile::tempdir().unwrap();
        for i in 0..7 {
            let r = Record { command: "pack".into(), exit_code: 1, message: format!("m{i}"), ..Default::default() };
            append(t.path(), &r);
        }
        assert_eq!(recent(t.path(), 100).len(), 7);
        append(&t.path().join("missing"), &Record::default());
        assert!(!t.path().join("missing").exists());
    }
}
