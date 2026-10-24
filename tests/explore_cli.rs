//! `oneform explore mutate` CLI: one experiment mutates exactly one donor slot.

use std::fs;
use std::process::Command;

#[test]
fn explore_mutate_cli_changes_exactly_the_addressed_slot() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let stream_in = root.join("FormStream.bin");
    fs::copy(manifest.join("tests/fixtures/forms/DonorForm/FormStream.bin"), &stream_in).unwrap();
    let spec = root.join("exp.json");
    fs::write(
        &spec,
        r#"{"id":"e1","control":"Надпись1","slot":"geom[5]","newValue":{"kind":"bare","value":"0"},"hypothesis":"Visible"}"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_oneform"))
            .args(args)
            .current_dir(root)
            .output()
            .unwrap()
    };
    let out = run(&[
        "explore",
        "mutate",
        "--spec",
        "exp.json",
        "--stream",
        "FormStream.bin",
        "--out",
        "out",
    ]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Надпись1") && stdout.contains("1 -> 0"), "{stdout}");

    // props --diff between the original and the mutated stream shows exactly the one slot.
    let diff = run(&[
        "props",
        "FormStream.bin",
        "--control",
        "Надпись1",
        "--diff",
        "out/mutated.bin",
    ]);
    assert!(diff.status.success(), "{}", String::from_utf8_lossy(&diff.stderr));
    let rows = String::from_utf8_lossy(&diff.stdout);
    assert_eq!(rows, "Надпись1.geom[5]\t1\t0\n", "{rows}");

    // Without --control, the same single row is the only difference in the whole form.
    let diff_all = run(&["props", "FormStream.bin", "--diff", "out/mutated.bin"]);
    assert_eq!(String::from_utf8_lossy(&diff_all.stdout), rows);
}

#[test]
fn explore_mutate_cli_errors_are_actionable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm/FormStream.bin"),
        root.join("FormStream.bin"),
    )
    .unwrap();
    fs::write(
        root.join("bad.json"),
        r#"{"id":"e1","control":"НетТакого","slot":"geom[5]","newValue":{"kind":"bare","value":"0"}}"#,
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oneform"))
        .args(["explore", "mutate", "--spec", "bad.json", "--stream", "FormStream.bin", "--out", "out"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("НетТакого") && err.contains("not found"), "{err}");
    assert!(!root.join("out/mutated.bin").exists(), "no output on failure");
}
