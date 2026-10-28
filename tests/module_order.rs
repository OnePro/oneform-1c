//! `pack` refuses an edit of `FormModule.bsl` that puts a procedure after the statements of the module body
//! («Определения процедур и функций должны размещаться перед операторами тела модуля»), for both pack targets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

fn run(root: &Path, command: &str) -> (bool, String) {
    let o: Output = Command::new(env!("CARGO_BIN_EXE_oneform")).args([command, ".", "-j", "1"]).current_dir(root).output().unwrap();
    (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn setup(epf: bool) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    if epf {
        fs::copy(manifest.join("tests/fixtures/donor.epf"), dir.path().join("donor.epf")).unwrap();
    } else {
        copy_dir(&manifest.join("tests/golden/config-extract/input"), dir.path());
    }
    let (ok, out) = run(dir.path(), "extract");
    assert!(ok, "{out}");
    dir
}

fn module_path(root: &Path, epf: bool) -> PathBuf {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| n == "FormModule.bsl") {
                out.push(p);
            }
        }
    }
    let mut all = Vec::new();
    walk(root, &mut all);
    all.sort();
    let rel = |p: &PathBuf| p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
    all.into_iter().find(|p| epf || rel(p).starts_with("Documents/")).expect("a FormModule.bsl")
}

fn append(path: &Path, text: &str) {
    let mut bsl = fs::read_to_string(path).unwrap();
    bsl.push_str(text);
    fs::write(path, bsl).unwrap();
}

const PROCEDURE: &str = "\nПроцедура ДобавленнаяПроцедура()\n\tВозврат;\nКонецПроцедуры\n";
const LATE: &str = "\nПроцедура ПослеТела()\n\tВозврат;\nКонецПроцедуры\n";

fn scenario(epf: bool) {
    let dir = setup(epf);
    let module = module_path(dir.path(), epf);

    // a procedure appended to a module that has no body statements packs
    append(&module, PROCEDURE);
    let (ok, out) = run(dir.path(), "pack");
    assert!(ok && out.contains("PACKED"), "{out}");

    // a statement of the module body packs (no procedure is introduced)
    append(&module, "\nДобавленноеЗначение = 1;\n");
    let (ok, out) = run(dir.path(), "pack");
    assert!(ok && out.contains("PACKED"), "{out}");

    // a procedure after that statement is refused, and the edited module stays as it is
    append(&module, LATE);
    let edited = fs::read_to_string(&module).unwrap();
    let (ok, out) = run(dir.path(), "pack");
    assert!(!ok, "{out}");
    assert!(out.contains("bsl-procedure-after-body") || out.contains("module body"), "{out}");
    assert!(out.contains("ПослеТела"), "{out}");
    assert_eq!(fs::read_to_string(&module).unwrap(), edited);

    // the procedure placed before the statement is accepted
    let without = edited.replace(LATE, "");
    let at = without.find("\nДобавленноеЗначение = 1;").expect("statement");
    let fixed = format!("{}\nПроцедура ПередТелом()\n\tВозврат;\nКонецПроцедуры\n{}", &without[..at], &without[at..]);
    fs::write(&module, fixed).unwrap();
    let (ok, out) = run(dir.path(), "pack");
    assert!(ok && out.contains("PACKED"), "{out}");
}

#[test]
fn epf_refuses_a_procedure_appended_after_the_module_body() {
    scenario(true);
}

#[test]
fn config_refuses_a_procedure_appended_after_the_module_body() {
    scenario(false);
}

/// Corpus sanity check (`cargo test --release -- --ignored module_scanner_on_corpus`; `ONEFORM_CORPUS` = directories
/// separated by `;`): reports the form modules that the scanner flags. Every flagged module is printed.
#[test]
#[ignore]
fn module_scanner_on_corpus() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| n == "FormModule.bsl") {
                out.push(p);
            }
        }
    }
    let roots = std::env::var("ONEFORM_CORPUS").expect("ONEFORM_CORPUS");
    let mut files = Vec::new();
    for r in roots.split(';') {
        walk(Path::new(r), &mut files);
    }
    let mut flagged = 0;
    for f in &files {
        let text = String::from_utf8_lossy(&fs::read(f).unwrap()).into_owned();
        let found = oneform::bsl_order::misplaced_definitions(&text);
        if let Some(m) = found.first() {
            flagged += 1;
            println!("{}: line {} '{}' after line {}", f.display(), m.line, m.name, m.body_line);
        }
    }
    println!("{} modules, {} flagged", files.len(), flagged);
}
