//! Optional large-corpus verification test (ignored by default).
//!
//! `ONEFORM_CORPUS_DIR=<export dir> cargo test --release --test oof_corpus -- --ignored --nocapture`
//!
//! For every object with an `src/ExtractionManifest.json`, the forms listed there are decoded
//! from their `Form.bin`; `Form.xml` must equal the file on disk byte for byte and the form
//! diagnostics must equal those of the manifest.

mod common;

use std::path::{Path, PathBuf};

use oneform::container::{parse_container, Limits};
use oneform::oof::build_oof;
use serde_json::Value;

fn manifests(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            manifests(&p, out);
        } else if p.file_name().is_some_and(|n| n == "ExtractionManifest.json")
            && p.parent().and_then(Path::file_name).is_some_and(|n| n == "src")
        {
            out.push(p);
        }
    }
}

fn canon(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

#[test]
#[ignore]
fn corpus_forms_match_manifests() {
    let Ok(corpus) = std::env::var("ONEFORM_CORPUS_DIR") else {
        eprintln!("ONEFORM_CORPUS_DIR is not set; skipped");
        return;
    };
    let mut found = Vec::new();
    manifests(Path::new(&corpus), &mut found);
    let (mut total, mut xml_mismatch, mut diag_mismatch, mut skipped) = (0usize, 0usize, 0usize, 0usize);
    for manifest_path in found {
        let src = manifest_path.parent().unwrap();
        let base = src.parent().unwrap();
        let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        let meta_xml = base.parent().unwrap().join(format!("{}.xml", base.file_name().unwrap().to_string_lossy()));
        let lookup = common::read_meta_lookup(&meta_xml);
        let diagnostics = manifest["diagnostics"].as_array().cloned().unwrap_or_default();
        for form in manifest["forms"].as_array().cloned().unwrap_or_default() {
            let layout = &form["layout"];
            let Some(locator) = layout["binary"]["sourceLocator"].as_str() else {
                skipped += 1;
                continue;
            };
            let bin_rel = locator.split('#').next().unwrap();
            let Ok(data) = std::fs::read(base.join(bin_rel)) else {
                skipped += 1;
                continue;
            };
            let container = parse_container(&data, &Limits::default(), 0, None).unwrap();
            let Some(entry) = container.get("form") else {
                skipped += 1;
                continue;
            };
            let (id, alias) = (form["id"].as_str().unwrap(), form["alias"].as_str().unwrap());
            let info = common::form_info(id, form["originalName"].as_str().unwrap(), &entry.raw);
            let result = build_oof(&info, alias, None, Some(&lookup));
            total += 1;

            let expected_xml = layout["xml"].as_str().and_then(|rel| std::fs::read(src.join(rel)).ok());
            if result.xml != expected_xml {
                xml_mismatch += 1;
                if xml_mismatch <= 10 {
                    eprintln!("XML MISMATCH {} ({})", base.join(bin_rel).display(), alias);
                }
            }
            let component = format!("Forms/{id}");
            let mut want: Vec<String> =
                diagnostics.iter().filter(|d| d["component"] == component.as_str()).map(canon).collect();
            let mut got: Vec<String> = result.diagnostics.iter().map(canon).collect();
            want.sort();
            got.sort();
            if want != got {
                diag_mismatch += 1;
                if diag_mismatch <= 10 {
                    eprintln!(
                        "DIAG MISMATCH {} ({}): want {} got {}",
                        base.join(bin_rel).display(),
                        alias,
                        want.len(),
                        got.len()
                    );
                }
            }
        }
    }
    eprintln!("forms: {total}, xml mismatches: {xml_mismatch}, diagnostic mismatches: {diag_mismatch}, skipped: {skipped}");
    assert_eq!((xml_mismatch, diag_mismatch), (0, 0));
}
