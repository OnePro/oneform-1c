//! `oneform feedback add|list|show`: a report for the developers of oneform, kept in
//! `<project dir>/.oneform/feedback/<YYYYMMDD-HHMMSS>-<kind>/` (and mirrored to `ONEFORM_FEEDBACK_DIR`).
//!
//! The report has `report.md` (for people), `report.json` (for tools), and, unless `--no-files`, the text files of the
//! history snapshot (`files/before`, `files/after`). Binaries (`Form.bin`, `.epf`) are copied only with
//! `--include-binaries`: module code may contain tokens or passwords.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::cli_epf::{clean_path, EXIT_FAILED, EXIT_OK, EXIT_USAGE};
use crate::diagnostics;
use crate::history::{self, Item};
use crate::{lint, validate};

pub const FEEDBACK_DIR: &str = "feedback";
pub const MIRROR_ENV: &str = "ONEFORM_FEEDBACK_DIR";
const KINDS: [&str; 2] = ["bug", "wish"];
const USAGE: &str = "usage: oneform feedback add [PATH] [--history ID] [--note TEXT] [--1c-error TEXT] [--kind bug|wish] [--no-files] [--include-binaries] | oneform feedback list [PATH] [--json] | oneform feedback show [PATH] <id> [--json]";
const SECRETS_WARNING: &str = "warning: the report holds module code (Form.xml, FormModule.bsl) that may contain tokens or passwords; read it before you share it (--no-files leaves the files out)";

/// The options of `feedback add`.
#[derive(Debug, Clone, Default)]
pub struct AddOptions {
    pub history_id: Option<String>,
    pub note: Option<String>,
    pub onec_error: Option<String>,
    pub kind: Option<String>,
    pub no_files: bool,
    pub include_binaries: bool,
}

impl AddOptions {
    fn is_set(&self) -> bool {
        self.history_id.is_some()
            || self.note.is_some()
            || self.onec_error.is_some()
            || self.kind.is_some()
            || self.no_files
            || self.include_binaries
    }
}

fn usage(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_USAGE
}

fn failure(message: &str) -> i32 {
    eprintln!("oneform: error: {message}");
    EXIT_FAILED
}

/// `oneform feedback add|list|show`; `args` are the positionals after `feedback`.
pub fn run_cli(args: &[String], opts: &AddOptions, json_output: bool) -> i32 {
    let Some(sub) = args.first().map(String::as_str) else { return usage(USAGE) };
    let rest = &args[1..];
    match sub {
        "add" => {
            if json_output {
                return usage("--json is only valid for feedback list and feedback show");
            }
            match rest {
                [] => add(Path::new("."), opts),
                [path] => add(Path::new(path), opts),
                _ => usage(USAGE),
            }
        }
        "list" | "show" => {
            if opts.is_set() {
                return usage("--history, --note, --1c-error, --kind, --no-files and --include-binaries are only valid for feedback add");
            }
            match (sub, rest) {
                ("list", []) => list_cmd(Path::new("."), json_output),
                ("list", [path]) => list_cmd(Path::new(path), json_output),
                ("show", [id]) => show_cmd(Path::new("."), id, json_output),
                ("show", [path, id]) => show_cmd(Path::new(path), id, json_output),
                _ => usage(USAGE),
            }
        }
        _ => usage(USAGE),
    }
}

/// The project directory a report belongs to when no snapshot says so: the only project below `root`, else the one with
/// the newest diagnostics line.
fn pick_project(root: &Path) -> Result<PathBuf, String> {
    let mut projects = diagnostics::project_dirs(root);
    match projects.len() {
        0 => Ok(root.to_path_buf()),
        1 => Ok(projects.remove(0)),
        _ => {
            let newest = projects
                .iter()
                .filter_map(|p| diagnostics::recent(p, 1).pop().map(|e| (e["time"].as_str().unwrap_or("").to_string(), p.clone())))
                .max_by(|a, b| a.0.cmp(&b.0));
            newest.map(|(_, p)| p).ok_or_else(|| {
                let names: Vec<String> = projects.iter().map(|p| history::rel_of(root, p)).collect();
                format!("several projects under {} ({}); point PATH at one of them", root.display(), names.join(", "))
            })
        }
    }
}

fn is_text(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".xml") || l.ends_with(".bsl")
}

fn files_of(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        fs::read_dir(dir).map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for e in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else {
            fs::copy(e.path(), &dest).map_err(|err| format!("{}: {err}", dest.display()))?;
        }
    }
    Ok(())
}

/// Files copied into the report: `(side, name)`.
fn snapshot_files(item: &Item, include_binaries: bool) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for side in ["before", "after"] {
        for name in files_of(&item.path.join(side)) {
            if is_text(&name) || include_binaries {
                out.push((side, name));
            }
        }
    }
    out
}

fn add(path: &Path, o: &AddOptions) -> i32 {
    let kind = o.kind.clone().unwrap_or_else(|| "bug".to_string());
    if !KINDS.contains(&kind.as_str()) {
        return usage(&format!("--kind for feedback add is bug or wish, not '{kind}'"));
    }
    let root = clean_path(path);
    if !root.is_dir() {
        return usage(&format!("{} is not a directory", root.display()));
    }
    let snapshot: Option<(PathBuf, Item)> = match o.history_id.as_deref() {
        Some(id) => match history::resolve_snapshot(&root, Some(id)) {
            Ok(s) => Some(s),
            Err(m) => return failure(&m),
        },
        None => history::latest_under(&root),
    };
    let project = match &snapshot {
        Some((dir, _)) => dir.clone(),
        None => match pick_project(&root) {
            Ok(p) => p,
            Err(m) => return failure(&m),
        },
    };

    let mut secs = history::local_secs();
    let feedback_root = project.join(".oneform").join(FEEDBACK_DIR);
    let id = loop {
        let id = format!("{}-{kind}", history::stamp(secs));
        if !feedback_root.join(&id).exists() {
            break id;
        }
        secs += 1;
    };
    let dir = feedback_root.join(&id);

    let entries = diagnostics::recent(&project, diagnostics::RECENT);
    let form = snapshot.as_ref().and_then(|(_, i)| i.manifest["form"].as_str().map(str::to_string));
    let config_form = snapshot.as_ref().is_some_and(|(_, i)| i.manifest["kind"].as_str() == Some("config-form"));
    let verdicts = validate::validate_root(&project, 1, None);
    let verdict_json: Vec<Value> = match &verdicts {
        Ok(v) => v.iter().filter(|x| !config_form || Some(&x.form) == form.as_ref()).map(validate::verdict_json).collect(),
        Err(_) => Vec::new(),
    };
    let verdict_lines: Vec<String> = match &verdicts {
        Ok(v) => v.iter().filter(|x| !config_form || Some(&x.form) == form.as_ref()).map(validate::verdict_line).collect(),
        Err(e) => vec![format!("validate не выполнен: {e}")],
    };
    let findings: Vec<Value> = lint::findings_json(&project)
        .into_iter()
        .filter(|f| !config_form || form.as_deref().is_some_and(|n| f["form"].as_str().is_some_and(|l| l.contains(n))))
        .collect();
    let stream_version = entries.iter().rev().find_map(|e| e["stream_version"].as_i64());
    let config_root = std::env::var_os("ONEFORM_CONFIG_ROOT").is_some_and(|v| !v.is_empty());
    let files = match &snapshot {
        Some((_, item)) if !o.no_files => snapshot_files(item, o.include_binaries),
        _ => Vec::new(),
    };
    let diff: Vec<String> = match &snapshot {
        Some((_, item)) if !o.no_files => history::snapshot_diff(item),
        _ => Vec::new(),
    };
    let time = history::time_text(secs);

    let report = json!({
        "id": id,
        "kind": kind,
        "time": time,
        "note": o.note,
        "1c_error": o.onec_error,
        "history": snapshot.as_ref().map(|(_, i)| i.id.clone()),
        "snapshot": snapshot.as_ref().map(|(_, i)| i.manifest.clone()),
        "diagnostics": entries,
        "validate": verdict_json,
        "lint": findings,
        "files": files.iter().map(|(s, n)| format!("files/{s}/{n}")).collect::<Vec<_>>(),
        "versions": {"oneform": env!("CARGO_PKG_VERSION"), "stream_version": stream_version},
        "environment": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH, "oneform_config_root_set": config_root},
    });
    let text = serde_json::to_string_pretty(&report).unwrap_or_default();
    let md = report_md(&report, &verdict_lines, &diff, o, config_root);

    let written = (|| -> Result<(), String> {
        write(&dir.join("report.json"), text.as_bytes())?;
        write(&dir.join("report.md"), md.as_bytes())?;
        if let Some((_, item)) = &snapshot {
            for (side, name) in &files {
                let from = item.path.join(side).join(name);
                let data = fs::read(&from).map_err(|e| format!("{}: {e}", from.display()))?;
                write(&dir.join("files").join(side).join(name), &data)?;
            }
        }
        Ok(())
    })();
    if let Err(m) = written {
        let _ = fs::remove_dir_all(&dir);
        return failure(&format!("the report was not written: {m}"));
    }

    println!("feedback {id}: {}", dir.display());
    println!("  report.md, report.json{}", if files.is_empty() { String::new() } else { format!(", {} file(s) of the snapshot", files.len()) });
    if snapshot.is_none() {
        println!("  no history snapshot: the report has no form files (pack has not changed a form yet)");
    }
    if let Some(mirror) = std::env::var_os(MIRROR_ENV).filter(|v| !v.is_empty()) {
        let name = project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "project".to_string());
        let target = PathBuf::from(mirror).join(name).join(&id);
        let _ = fs::remove_dir_all(&target);
        match copy_tree(&dir, &target) {
            Ok(()) => println!("  mirrored to {}", target.display()),
            Err(m) => eprintln!("oneform: warning: the report was not mirrored to {}: {m}", target.display()),
        }
    }
    if !files.is_empty() {
        println!("{SECRETS_WARNING}");
    }
    EXIT_OK
}

fn fence(out: &mut String, lang: &str, lines: &[String]) {
    out.push_str(&format!("```{lang}\n"));
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str("```\n\n");
}

fn report_md(r: &Value, verdicts: &[String], diff: &[String], o: &AddOptions, config_root: bool) -> String {
    let mut m = String::new();
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    m.push_str(&format!("# Отчёт oneform: {} ({})\n\n", s(&r["id"]), if s(&r["kind"]) == "wish" { "пожелание" } else { "ошибка" }));
    m.push_str(&format!("- Время: {}\n- oneform: {}\n", s(&r["time"]), s(&r["versions"]["oneform"])));
    if let Some(v) = r["versions"]["stream_version"].as_i64() {
        m.push_str(&format!("- Версия потока формы: {v}\n"));
    }
    m.push('\n');
    m.push_str("## Заметка\n\n");
    m.push_str(&format!("{}\n\n", o.note.as_deref().filter(|t| !t.trim().is_empty()).unwrap_or("(не указана)")));
    m.push_str("## Ошибка 1С\n\n");
    match o.onec_error.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(t) => fence(&mut m, "", &t.lines().map(str::to_string).collect::<Vec<_>>()),
        None => m.push_str("(не указана)\n\n"),
    }
    m.push_str("## Снимок истории\n\n");
    if r["snapshot"].is_null() {
        m.push_str("Снимка нет: pack ещё не менял форму.\n\n");
    } else {
        let snap = &r["snapshot"];
        m.push_str(&format!("- id: {}\n- форма: {}\n- время: {}\n- причина: {}\n", s(&snap["id"]), s(&snap["form"]), s(&snap["time"]), s(&snap["reason"])));
        for line in snap["summary"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            m.push_str(&format!("- {line}\n"));
        }
        for line in snap["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            m.push_str(&format!("- {line}\n"));
        }
        m.push('\n');
    }
    m.push_str("## Последние записи диагностики\n\n");
    let entries: Vec<String> = r["diagnostics"].as_array().into_iter().flatten().map(|e| diagnostics::entry_line(e).trim().to_string()).collect();
    if entries.is_empty() {
        m.push_str("Записей нет.\n\n");
    } else {
        fence(&mut m, "", &entries);
    }
    m.push_str("## Проверка формы (validate)\n\n");
    if verdicts.is_empty() {
        m.push_str("Форм не найдено.\n\n");
    } else {
        fence(&mut m, "", verdicts);
    }
    let lint = r["lint"].as_array().cloned().unwrap_or_default();
    m.push_str(&format!("## Проверка раскладки (lint): {} замечаний\n\n", lint.len()));
    if !lint.is_empty() {
        let lines: Vec<String> = lint
            .iter()
            .take(20)
            .map(|f| format!("{} {} line {}: {}: {}", s(&f["severity"]), s(&f["rule"]), f["line"], s(&f["control"]), s(&f["message"])))
            .collect();
        fence(&mut m, "", &lines);
    }
    m.push_str("## Что изменил pack\n\n");
    if r["snapshot"].is_null() {
        m.push_str("Нет снимка.\n\n");
    } else if o.no_files {
        m.push_str("Не включено (`--no-files`).\n\n");
    } else if diff.is_empty() {
        m.push_str("Form.xml и FormModule.bsl не изменились.\n\n");
    } else {
        fence(&mut m, "diff", diff);
    }
    m.push_str("## Окружение\n\n");
    m.push_str(&format!(
        "- ОС: {} ({})\n- ONEFORM_CONFIG_ROOT: {}\n\n",
        s(&r["environment"]["os"]),
        s(&r["environment"]["arch"]),
        if config_root { "задан" } else { "не задан" }
    ));
    m.push_str("## Файлы\n\n");
    let files: Vec<String> = r["files"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    if files.is_empty() {
        m.push_str("Файлы формы не приложены.\n");
    } else {
        for f in &files {
            m.push_str(&format!("- {f}\n"));
        }
        m.push_str("\nВнимание: модули могут содержать токены и пароли; проверьте отчёт, прежде чем передавать его.\n");
    }
    m
}

/// `(project dir, report dir, id)` of every report under `root`, oldest first.
type Report = (PathBuf, PathBuf, String);

fn reports(root: &Path) -> Vec<Report> {
    let mut out = Vec::new();
    for p in diagnostics::project_dirs(root) {
        let Ok(rd) = fs::read_dir(p.join(".oneform").join(FEEDBACK_DIR)) else { continue };
        for e in rd.flatten() {
            if e.path().join("report.json").is_file() {
                out.push((p.clone(), e.path(), e.file_name().to_string_lossy().into_owned()));
            }
        }
    }
    out.sort_by(|a, b| a.2.cmp(&b.2));
    out
}

fn list_cmd(path: &Path, json_output: bool) -> i32 {
    let root = clean_path(path);
    if !root.is_dir() {
        return usage(&format!("{} is not a directory", root.display()));
    }
    let all = reports(&root);
    let read = |dir: &Path| fs::read(dir.join("report.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
    if json_output {
        let items: Vec<Value> = all
            .iter()
            .map(|(p, d, id)| {
                let r = read(d);
                json!({"id": id, "project": history::rel_of(&root, p), "kind": r["kind"], "time": r["time"], "history": r["history"], "note": r["note"], "1c_error": r["1c_error"], "path": d.to_string_lossy()})
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items).unwrap_or_default());
        return EXIT_OK;
    }
    if all.is_empty() {
        println!("no feedback reports under {} (oneform feedback add)", root.display());
        return EXIT_OK;
    }
    for (p, d, id) in &all {
        let r = read(d);
        let what = [r["1c_error"].as_str(), r["note"].as_str()].into_iter().flatten().find_map(|t| t.lines().map(str::trim).find(|l| !l.is_empty())).unwrap_or("");
        println!("{id}  {}  history {}  {what}", history::rel_of(&root, p), r["history"].as_str().unwrap_or("-"));
    }
    EXIT_OK
}

fn show_cmd(path: &Path, id: &str, json_output: bool) -> i32 {
    let root = clean_path(path);
    if !root.is_dir() {
        return usage(&format!("{} is not a directory", root.display()));
    }
    let all = reports(&root);
    let exact: Vec<&Report> = all.iter().filter(|r| r.2 == id).collect();
    let hits: Vec<&Report> = if exact.is_empty() { all.iter().filter(|r| r.2.starts_with(id)).collect() } else { exact };
    let hit = match hits.as_slice() {
        [one] => *one,
        [] => {
            let ids: Vec<&str> = all.iter().map(|r| r.2.as_str()).collect();
            let known = if ids.is_empty() { "no reports under this path".to_string() } else { format!("available: {}", ids.join(", ")) };
            return failure(&format!("no feedback report '{id}' under {}; {known}", root.display()));
        }
        _ => return failure(&format!("feedback id '{id}' is ambiguous; give a longer prefix or point PATH at the project")),
    };
    let name = if json_output { "report.json" } else { "report.md" };
    match fs::read(hit.1.join(name)) {
        Ok(b) => {
            print!("{}", String::from_utf8_lossy(&b));
            EXIT_OK
        }
        Err(e) => failure(&format!("{}: {e}", hit.1.join(name).display())),
    }
}

