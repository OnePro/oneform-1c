//! Anchor graph consistency of every form stream of a real corpus (`ONEFORM_CORPUS=target/corpus/ut2`).

use std::path::{Path, PathBuf};

use oneform::brace::parse_spanned;
use oneform::form_patch::{anchor_graph_stale_entries, anchor_graph_violations};

fn streams(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            streams(&p, out);
        } else if p.file_name().is_some_and(|n| n == "FormStream.bin") {
            out.push(p);
        }
    }
}

#[test]
#[ignore = "needs ONEFORM_CORPUS"]
fn corpus_anchor_graph_is_consistent() {
    let root = std::env::var("ONEFORM_CORPUS").expect("set ONEFORM_CORPUS to the corpus directory");
    let mut files = Vec::new();
    streams(Path::new(&root), &mut files);
    files.sort();
    let (mut bad_forms, mut total, mut unindexable, mut stale_forms, mut stale) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut first = Vec::new();
    for f in &files {
        let bytes = std::fs::read(f).expect("read FormStream.bin");
        let meta = parse_spanned(&bytes).expect("parse FormStream.bin");
        let s = anchor_graph_stale_entries(&meta);
        stale += s.len();
        stale_forms += usize::from(!s.is_empty());
        let (cannot_index, v): (Vec<_>, Vec<_>) = anchor_graph_violations(&meta).into_iter().partition(|m| m.starts_with("cannot index"));
        unindexable += cannot_index.len();
        if !v.is_empty() {
            bad_forms += 1;
            total += v.len();
            first.extend(v.into_iter().take(20).map(|m| format!("{}: {m}", f.display())));
        }
    }
    println!(
        "forms scanned: {}, not indexable (skipped): {unindexable}, forms with violations: {bad_forms}, violations: {total}; \
         informational leftovers: {stale} entries in {stale_forms} forms",
        files.len()
    );
    first.truncate(20);
    assert!(first.is_empty(), "anchor graph violations:\n{}", first.join("\n"));
}
