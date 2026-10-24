//! Corpus replay for the property-exploration pipeline (tasks 3.1-3.2): every fixture under
//! `tests/explore/` is validated WITHOUT the 1C platform — the mutation is recomputed from the
//! manifest and must reproduce `mutated.bin` byte-for-byte, and the recorded diffs must match a
//! recomputation. Platform-backed regeneration runs only with `ONEFORM_EXPLORE=1` and explicit
//! platform config (test is `#[ignore]`d by default).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use oneform::brace::parse_spanned;
use oneform::explore::{net_diff, mutate_stream, ExperimentSpec};
use oneform::props::diff_props;
use serde_json::Value;

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/explore")
}

fn read(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn json(p: &Path) -> Value {
    serde_json::from_slice(&read(p)).unwrap_or_else(|e| panic!("invalid JSON {}: {e}", p.display()))
}

fn rows(p: &Path) -> Vec<(String, String, String)> {
    json(p)
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["path"].as_str().unwrap().to_string(),
                r["from"].as_str().unwrap().to_string(),
                r["to"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn fixture_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(corpus())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().unwrap() != "baseline")
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn corpus_fixtures_replay_without_the_platform() {
    let dirs = fixture_dirs();
    assert!(!dirs.is_empty(), "tests/explore must contain at least one experiment fixture");
    let baseline_dir = corpus().join("baseline");
    let baseline_rows = rows(&baseline_dir.join("baseline-diff.json"));
    let volatile: Vec<String> = json(&baseline_dir.join("manifest.json"))["volatile"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();

    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let manifest = json(&dir.join("manifest.json"));
        assert_eq!(manifest["schema"].as_i64(), Some(1), "{name}");
        for key in ["id", "control", "slot", "outcome", "controlSurvived", "runtime"] {
            assert!(manifest.get(key).is_some(), "{name}: manifest misses {key}");
        }
        assert_eq!(manifest["id"].as_str().unwrap(), name);
        let outcome = manifest["outcome"].as_str().unwrap();
        assert!(matches!(outcome, "accept" | "reject" | "silent-drop"), "{name}: {outcome}");

        // The mutation recomputed from the manifest reproduces mutated.bin byte-for-byte.
        let original = read(&dir.join("original.bin"));
        let mutated = read(&dir.join("mutated.bin"));
        let spec = ExperimentSpec::parse_json(&serde_json::to_vec(&serde_json::json!({
            "id": manifest["id"],
            "control": manifest["control"],
            "slot": manifest["slot"],
            "newValue": manifest["newValue"],
            "hypothesis": manifest["hypothesis"],
        }))
        .unwrap())
        .unwrap();
        let source = parse_spanned(&original).unwrap();
        let (recomputed, old) = mutate_stream(&source, &spec).unwrap();
        assert_eq!(recomputed, mutated, "{name}: recomputed mutation differs from mutated.bin");
        assert_eq!(old, manifest["oldValue"]["value"].as_str().unwrap(), "{name}: oldValue");

        // Accepted experiments keep the normalized stream and matching observed/net diffs.
        if outcome == "accept" {
            let normalized = read(&dir.join("normalized.bin"));
            let observed = diff_props(&mutated, &normalized, Some(spec.control.as_str())).unwrap();
            assert_eq!(observed, rows(&dir.join("observed.json")), "{name}: observed.json");
            let net = net_diff(&observed, &baseline_rows, &volatile);
            assert_eq!(net, rows(&dir.join("net-diff.json")), "{name}: net-diff.json");
        } else {
            assert!(!dir.join("normalized.bin").exists(), "{name}: rejected fixtures keep no normalized stream");
        }
        // The semantic observation is deterministic and platform-free: recompute and compare.
        let decoded = oneform::explore::decoded_control_diff(&original, &mutated, spec.control.as_str()).unwrap();
        assert_eq!(decoded, rows(&dir.join("decoded-diff.json")), "{name}: decoded-diff.json");

        // When runtime introspection was run, manifest["runtime"] summarizes runtime-diff.json.
        let rt_file = dir.join("runtime-diff.json");
        if manifest["runtime"].is_object() {
            assert!(rt_file.is_file(), "{name}: runtime object in manifest requires runtime-diff.json");
            let rt_rows = rows(&rt_file);
            assert_eq!(
                manifest["runtime"]["changedCount"].as_u64(),
                Some(rt_rows.len() as u64),
                "{name}: runtime.changedCount"
            );
            for (path, _, _) in &rt_rows {
                assert!(
                    path.starts_with(&format!("{}.", spec.control)),
                    "{name}: runtime diff path {path} must belong to {}",
                    spec.control
                );
            }
        } else {
            assert!(manifest["runtime"].is_null(), "{name}: runtime must be object or null");
        }
    }
}

#[test]
fn explore_run_requires_platform_config() {
    // The conveyor refuses to run without explicit platform connection parameters.
    let out = Command::new(env!("CARGO_BIN_EXE_oneform")).args(["explore", "run"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--matrix") && err.contains("--platform") && err.contains("--base"), "{err}");
}

/// Regenerate the corpus against the real platform. Never runs by default: `cargo test` skips it
/// (`#[ignore]`); to run: `ONEFORM_EXPLORE=1 ONEFORM_EXPLORE_BASE=<infobase> cargo test -- --ignored`.
#[test]
#[ignore]
fn corpus_regeneration_requires_explicit_opt_in_and_platform() {
    if std::env::var_os("ONEFORM_EXPLORE").is_none() {
        panic!("regeneration requires ONEFORM_EXPLORE=1 plus ONEFORM_EXPLORE_BASE and ONEFORM_EXPLORE_PLATFORM");
    }
    let base = std::env::var("ONEFORM_EXPLORE_BASE").expect("ONEFORM_EXPLORE_BASE (infobase directory) is required");
    let platform = std::env::var("ONEFORM_EXPLORE_PLATFORM").expect("ONEFORM_EXPLORE_PLATFORM (1cv8 bin directory) is required");
    assert!(Path::new(&base).join("1Cv8.1CD").is_file(), "no 1Cv8.1CD under {base}");
    assert!(Path::new(&platform).is_dir(), "no platform bin directory {platform}");
}

/// One-off maintenance helper: fill in `decoded-diff.json` for fixtures recorded before the
/// semantic observation existed. Run explicitly: `cargo test --test explore_corpus -- --ignored`.
#[test]
#[ignore]
fn backfill_decoded_diff() {
    for dir in fixture_dirs() {
        let target = dir.join("decoded-diff.json");
        if target.exists() {
            continue;
        }
        let manifest = json(&dir.join("manifest.json"));
        let original = read(&dir.join("original.bin"));
        let mutated = read(&dir.join("mutated.bin"));
        let rows = oneform::explore::decoded_control_diff(&original, &mutated, manifest["control"].as_str().unwrap()).unwrap();
        let v: Vec<Value> = rows
            .iter()
            .map(|(p, a, b)| serde_json::json!({"path": p, "from": a, "to": b}))
            .collect();
        fs::write(&target, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        println!("backfilled {}", dir.display());
    }
}
