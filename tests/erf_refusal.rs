//! Binary `.erf` (external report): the container is the same as of `.epf`, but the root metadata is a report's,
//! so `oneform` does not read it and says so. `tests/fixtures/report.erf` was written by the platform (`erf-build`
//! from an `erf-init` dump with one attribute and one ordinary form).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use oneform::container::{inflate, parse_container, Limits};

const HINT: &str = "внешний отчёт: выгрузите в XML (erf-dump), oneform работает с XML-выгрузкой";
const REPORT_CLASS: &str = "e41aff26-25cf-4bb6-b6c1-3f478a75f374";
const PROCESSOR_CLASS: &str = "c3831ec8-d8d5-4f93-8a22-f9bfae07327f";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oneform")).args(args).output().unwrap()
}

fn root_class(path: &Path) -> String {
    let limits = Limits::default();
    let c = parse_container(&fs::read(path).unwrap(), &limits, 0, None).unwrap();
    let root = inflate(&c.get("root").unwrap().raw, &limits, None).unwrap();
    let text = String::from_utf8(root).unwrap();
    let uuid = text.trim_start_matches('\u{feff}').trim_start_matches("{2,").split(',').next().unwrap().to_string();
    let meta = inflate(&c.get(&uuid).unwrap().raw, &limits, None).unwrap();
    let meta = String::from_utf8(meta).unwrap();
    [REPORT_CLASS, PROCESSOR_CLASS].into_iter().find(|id| meta.contains(id)).unwrap_or("unknown").to_string()
}

#[test]
fn erf_shares_the_container_but_not_the_root_metadata() {
    // The container parses (same format as .epf) ...
    let limits = Limits::default();
    let erf = parse_container(&fs::read(fixture("report.erf")).unwrap(), &limits, 0, None).unwrap();
    for name in ["root", "version", "versions", "copyinfo"] {
        assert!(erf.get(name).is_some(), "{name}");
    }
    // ... but the root metadata class is the report's, which the processor reader rejects.
    assert_eq!(root_class(&fixture("report.erf")), REPORT_CLASS);
    assert_eq!(root_class(&fixture("donor.epf")), PROCESSOR_CLASS);
}

#[test]
fn every_command_skips_erf_with_a_hint_and_leaves_it_untouched() {
    let original = fs::read(fixture("report.erf")).unwrap();
    for command in ["extract", "pack", "check", "preview", "lint", "validate"] {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("Reports/R")).unwrap();
        // upper-case suffix is an .erf too; a backup copy is not
        fs::write(tmp.path().join("Reports/R/Отчёт.ERF"), &original).unwrap();
        fs::write(tmp.path().join("Reports/R/Отчёт.erf.bak"), &original).unwrap();
        let mut args = vec![command, tmp.path().to_str().unwrap()];
        if !matches!(command, "lint" | "validate") {
            args.push("--no-progress");
        }
        let out = run(&args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(HINT), "{command}: {stderr}");
        assert!(stderr.contains("Reports/R/Отчёт.ERF"), "{command}: {stderr}");
        assert!(!stderr.contains(".erf.bak"), "{command}: {stderr}");
        assert_eq!(fs::read(tmp.path().join("Reports/R/Отчёт.ERF")).unwrap(), original, "{command}");
        assert!(!tmp.path().join("Reports/R/src").exists(), "{command}: nothing may be extracted");
        assert!(!tmp.path().join(".oneform").exists(), "{command}");
    }
}

#[test]
fn extract_of_a_directory_with_only_an_erf_fails_with_the_hint() {
    let tmp = tempfile::tempdir().unwrap();
    fs::copy(fixture("report.erf"), tmp.path().join("r.erf")).unwrap();
    let out = run(&["extract", tmp.path().to_str().unwrap(), "--no-progress"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(HINT) && stderr.contains("r.erf") && stderr.contains("nothing to process"), "{stderr}");
}

#[test]
fn erf_next_to_an_epf_does_not_stop_the_epf() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("P")).unwrap();
    fs::copy(fixture("donor.epf"), tmp.path().join("P/donor.epf")).unwrap();
    fs::copy(fixture("report.erf"), tmp.path().join("P/r.erf")).unwrap();
    let out = run(&["extract", tmp.path().to_str().unwrap(), "--no-progress"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains(HINT));
    assert!(tmp.path().join("P/src").is_dir(), "the processor is extracted");
    assert_eq!(fs::read(tmp.path().join("P/r.erf")).unwrap(), fs::read(fixture("report.erf")).unwrap());
}

#[test]
fn an_erf_renamed_to_epf_is_rejected_as_unsupported_layout() {
    // This is why .erf is not simply added to the discovery: the processor reader needs the processor root class.
    let tmp = tempfile::tempdir().unwrap();
    fs::copy(fixture("report.erf"), tmp.path().join("r.epf")).unwrap();
    let out = run(&["extract", tmp.path().to_str().unwrap(), "--no-progress"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("METADATA_LAYOUT_UNSUPPORTED"));
}
