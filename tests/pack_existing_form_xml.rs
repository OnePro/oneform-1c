//! Packing edited `Form.xml` for both pack targets (EPF and configuration `Form.bin`): supported edits of
//! existing controls are patched into the form stream, unsupported ones are rejected with
//! `FORM_EDIT_REJECTED`, and every failure leaves XML, BSL and containers untouched. Unchanged and
//! BSL-only packing keeps working.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type Tree = BTreeMap<String, Vec<u8>>;

#[derive(Clone, Copy, Debug)]
enum Target {
    Epf,
    Config,
}

struct Work {
    dir: tempfile::TempDir,
    target: Target,
}

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

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oneform"))
        .args(args)
        .arg("-j")
        .arg("1")
        .current_dir(root)
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Every file except tool-internal workspaces (locks, viewer data).
fn snapshot(root: &Path) -> Tree {
    fn walk(base: &Path, dir: &Path, out: &mut Tree) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            if rel == ".extract-work" || rel == ".oneform" {
                continue;
            }
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = Tree::new();
    walk(root, root, &mut out);
    out
}

impl Work {
    fn new(target: Target) -> Work {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        match target {
            Target::Epf => {
                fs::copy(
                    manifest.join("tests/fixtures/donor.epf"),
                    dir.path().join("donor.epf"),
                )
                .unwrap();
            }
            Target::Config => copy_dir(
                &manifest.join("tests/golden/config-extract/input"),
                dir.path(),
            ),
        }
        let out = run(dir.path(), &["extract", "."]);
        assert!(out.status.success(), "extract failed: {}", text(&out));
        Work { dir, target }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn snap(&self) -> Tree {
        snapshot(self.root())
    }

    fn src_file(&self, name: &str) -> PathBuf {
        let tree = self.snap();
        let hits: Vec<&String> = tree
            .keys()
            .filter(|p| p.contains("/src/") || p.starts_with("src/"))
            .filter(|p| p.ends_with(name))
            .collect();
        // The configuration fixture has two forms; the document form is the one with Form.xml edits.
        let pick = match self.target {
            Target::Epf => hits[0],
            Target::Config => hits
                .iter()
                .find(|p| p.starts_with("Documents/"))
                .expect("document form"),
        };
        self.root().join(pick)
    }

    fn edit_xml(&self) {
        let p = self.src_file("Form.xml");
        let xml = fs::read_to_string(&p).unwrap();
        let at = xml.find("<Width>").expect("Width") + "<Width>".len();
        let end = at + xml[at..].find('<').unwrap();
        fs::write(&p, format!("{}-1{}", &xml[..at], &xml[end..])).unwrap();
    }

    fn edit_bsl(&self) {
        let p = self.src_file("FormModule.bsl");
        let mut bsl = fs::read(&p).unwrap();
        bsl.extend_from_slice("\n// edited\n".as_bytes());
        fs::write(&p, bsl).unwrap();
    }
}

fn assert_rejected(target: Target, xml: bool, bsl: bool) {
    let w = Work::new(target);
    if xml {
        w.edit_xml();
    }
    if bsl {
        w.edit_bsl();
    }
    let before = w.snap();
    let out = run(w.root(), &["pack", "."]);
    let msg = text(&out);
    assert!(
        !out.status.success(),
        "{target:?} xml={xml} bsl={bsl}: pack accepted edited Form.xml: {msg}"
    );
    assert!(
        msg.contains("FORM_EDIT_REJECTED"),
        "{target:?}: expected FORM_EDIT_REJECTED, got: {msg}"
    );
    assert!(
        msg.contains("Form.xml"),
        "{target:?}: error must name Form.xml: {msg}"
    );
    let after = w.snap();
    let changed: Vec<&String> = before
        .keys()
        .chain(after.keys())
        .filter(|k| before.get(*k) != after.get(*k))
        .collect();
    assert!(
        changed.is_empty(),
        "{target:?} xml={xml} bsl={bsl}: files changed after rejection: {changed:?}"
    );
}

#[test]
fn epf_rejects_xml_only_edit() {
    assert_rejected(Target::Epf, true, false);
}

#[test]
fn epf_rejects_xml_and_bsl_edit() {
    assert_rejected(Target::Epf, true, true);
}

#[test]
fn config_rejects_xml_only_edit() {
    assert_rejected(Target::Config, true, false);
}

#[test]
fn config_rejects_xml_and_bsl_edit() {
    assert_rejected(Target::Config, true, true);
}

fn assert_unchanged(target: Target) {
    let w = Work::new(target);
    let before = w.snap();
    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(
        text(&out).contains("UNCHANGED"),
        "{target:?}: {}",
        text(&out)
    );
    assert_eq!(
        before,
        w.snap(),
        "{target:?}: unchanged pack modified files"
    );
}

#[test]
fn epf_unchanged_pack_is_noop() {
    assert_unchanged(Target::Epf);
}

#[test]
fn config_unchanged_pack_is_noop() {
    assert_unchanged(Target::Config);
}

fn assert_bsl_only(target: Target) {
    let w = Work::new(target);
    w.edit_bsl();
    let before = w.snap();
    let bsl_path = w.src_file("FormModule.bsl");
    let xml_path = w.src_file("Form.xml");
    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    let after = w.snap();
    let changed: Vec<&String> = before
        .keys()
        .filter(|k| before.get(*k) != after.get(*k))
        .filter(|k| !k.contains("/src/") && !k.starts_with("src/"))
        .collect();
    assert!(
        !changed.is_empty(),
        "{target:?}: target container was not rewritten"
    );
    assert!(
        changed
            .iter()
            .all(|k| k.ends_with(".epf") || k.ends_with("Form.bin")),
        "{target:?}: unexpected changes {changed:?}"
    );
    let rel = |p: &Path| {
        p.strip_prefix(w.root())
            .unwrap()
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    };
    assert_eq!(
        before[&rel(&xml_path)],
        after[&rel(&xml_path)],
        "{target:?}: Form.xml changed"
    );
    assert_eq!(
        before[&rel(&bsl_path)],
        after[&rel(&bsl_path)],
        "{target:?}: edited BSL changed"
    );

    // Fresh extraction of the packed target reproduces the BSL edit.
    let fresh = tempfile::tempdir().unwrap();
    for (p, data) in after
        .iter()
        .filter(|(p, _)| !p.contains("/src/") && !p.starts_with("src/"))
    {
        let dest = fresh.path().join(p);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, data).unwrap();
    }
    let out = run(fresh.path(), &["extract", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    let fresh_tree = snapshot(fresh.path());
    assert_eq!(
        fresh_tree[&rel(&bsl_path)],
        after[&rel(&bsl_path)],
        "{target:?}: packed BSL differs"
    );
}

#[test]
fn epf_bsl_only_edit_still_packs() {
    assert_bsl_only(Target::Epf);
}

#[test]
fn config_bsl_only_edit_still_packs() {
    assert_bsl_only(Target::Config);
}

// ---------------------------------------------------------------------------------------------------------
// Supported edits of existing controls (tasks 5.1-5.3)
// ---------------------------------------------------------------------------------------------------------

use oneform::errors::{ExtractionError, FORM_EDIT_REJECTED, PUBLISH_VERIFY_FAILED};
use oneform::pack::set_patch_hook;
use oneform::transaction::{read_tree, set_rename_hook};

const LABEL: &str = "name=\"Надпись1\"";
const CAPTION: (&str, &str) = ("<Item language=\"ru\">Надпись1</Item>", "<Item language=\"ru\">Подпись</Item>");
const LEFT: (&str, &str) = ("<Left>79</Left>", "<Left>90</Left>");

/// Replaces `from_tail` by `to_tail` once, where it follows `head` and an optional `<Appearance .../>` line.
fn replace_after_appearance(xml: &str, head: &str, from_tail: &str, to_tail: &str, count: usize) -> String {
    assert_eq!(count, 1);
    let at = xml.find(head).unwrap_or_else(|| panic!("head not found: {head}")) + head.len();
    let mut tail_at = at;
    if let Some(rest) = xml[at..].strip_prefix("\n          <Appearance ") {
        tail_at = at + 1 + (xml[at + 1..].find('\n').unwrap_or(rest.len()));
    }
    assert!(xml[tail_at..].starts_with(from_tail), "tail not found after {head}");
    format!("{}{}{}", &xml[..tail_at], to_tail, &xml[tail_at + from_tail.len()..])
}

/// `Form.xml` without the verbatim carriers (`raw`/`sig` attributes and the record elements): `pack` writes them
/// again from the packed stream, so an edited form is compared modulo them.
fn no_carriers(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    let elements = regex::Regex::new(r"(?m)^[ \t]*<(Appearance|InfoRecord|PanelRecord|FormRecord|ColumnProps)\b[^>]*/>\n").unwrap();
    let attrs = regex::Regex::new(r#" (raw|sig)=("[^"]*"|'[^']*')"#).unwrap();
    attrs.replace_all(&elements.replace_all(&text, ""), "").into_owned()
}

fn rel(w: &Work, p: &Path) -> String {
    p.strip_prefix(w.root())
        .unwrap()
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn is_src(p: &str) -> bool {
    p.contains("/src/") || p.starts_with("src/")
}

/// Extract the non-`src` files of `tree` into a fresh workspace, as if only the packed target existed.
fn fresh_extract(tree: &Tree) -> Tree {
    let fresh = tempfile::tempdir().unwrap();
    for (p, data) in tree
        .iter()
        .filter(|(p, _)| !is_src(p) && !p.ends_with(".bak"))
    {
        let dest = fresh.path().join(p);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, data).unwrap();
    }
    let out = run(fresh.path(), &["extract", "."]);
    assert!(out.status.success(), "{}", text(&out));
    snapshot(fresh.path())
}

impl Work {
    /// The container that pack rewrites: `donor.epf` or the document's `Ext/Form.bin`.
    fn target_path(&self) -> String {
        self.snap()
            .keys()
            .find(|p| match self.target {
                Target::Epf => p.ends_with(".epf"),
                Target::Config => p.starts_with("Documents/") && p.ends_with("Ext/Form.bin"),
            })
            .expect("target container")
            .clone()
    }

    /// Replace `from` -> `to` for each pair, searching after the `Надпись1` control element.
    fn edit_label(&self, edits: &[(&str, &str)]) {
        let p = self.src_file("Form.xml");
        let mut xml = fs::read_to_string(&p).unwrap();
        let at = xml.find(LABEL).expect("label control");
        for (from, to) in edits {
            let off = at + xml[at..].find(from).unwrap_or_else(|| panic!("missing {from}"));
            xml.replace_range(off..off + from.len(), to);
        }
        fs::write(p, xml).unwrap();
    }

    fn stream_path(&self) -> String {
        rel(self, &self.src_file("FormStream.bin"))
    }
}

/// `(common prefix, common suffix)` of two byte strings, never overlapping.
fn affixes(a: &[u8], b: &[u8]) -> (usize, usize) {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (prefix, suffix)
}

fn assert_label_packed(target: Target, with_bsl: bool, backup: bool) {
    let w = Work::new(target);
    let before = w.snap();
    let target_path = w.target_path();
    w.edit_label(&[CAPTION, LEFT]);
    if with_bsl {
        w.edit_bsl();
    }
    let edited = w.snap();
    let xml_path = rel(&w, &w.src_file("Form.xml"));
    let bsl_path = rel(&w, &w.src_file("FormModule.bsl"));
    let stream_path = w.stream_path();
    let out = if backup {
        run(w.root(), &["pack", ".", "--backup"])
    } else {
        run(w.root(), &["pack", "."])
    };
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));
    let after = w.snap();

    assert_ne!(
        before[&target_path], after[&target_path],
        "{target:?}: target container was not rewritten"
    );
    // the user's XML (and BSL) edits are still in src, byte for byte
    assert_eq!(no_carriers(&after[&xml_path]), no_carriers(&edited[&xml_path]), "{target:?}: Form.xml changed");
    assert_eq!(after[&bsl_path], edited[&bsl_path], "{target:?}: FormModule.bsl changed");
    // only the container, regenerated previews and (on request) the backup changed outside src
    let backup_path = format!("{target_path}.bak");
    let changed: Vec<&String> = after
        .keys()
        .filter(|k| !is_src(k) && before.get(*k) != after.get(*k))
        .collect();
    assert!(
        changed.iter().all(|k| **k == target_path || (backup && **k == backup_path)),
        "{target:?}: unexpected changes {changed:?}"
    );
    if backup {
        assert_eq!(
            after[&backup_path], before[&target_path],
            "{target:?}: backup is not the original target"
        );
    }

    // the packed target reproduces the requested XML (and BSL) after a fresh extraction
    let fresh = fresh_extract(&after);
    assert_eq!(
        no_carriers(&fresh[&xml_path]), no_carriers(&edited[&xml_path]),
        "{target:?}: re-extracted Form.xml differs from the requested one"
    );
    assert_eq!(fresh[&bsl_path], edited[&bsl_path], "{target:?}: re-extracted BSL differs");
    assert_eq!(
        fresh[&stream_path], after[&stream_path],
        "{target:?}: published stream differs from the packed one"
    );

    // the uncompressed stream differs from the original only around the edited control's atoms
    let (old, new) = (&before[&stream_path], &after[&stream_path]);
    assert_ne!(old, new);
    let (prefix, suffix) = affixes(old, new);
    assert!(
        old.len() - prefix - suffix < 4000,
        "{target:?}: patch touched {} bytes",
        old.len() - prefix - suffix
    );
    assert!(String::from_utf8_lossy(new).contains("Подпись"));
}

#[test]
fn epf_xml_only_edit_packs() {
    assert_label_packed(Target::Epf, false, false);
}

#[test]
fn epf_xml_and_bsl_edit_packs() {
    assert_label_packed(Target::Epf, true, false);
}

#[test]
fn config_xml_only_edit_packs() {
    assert_label_packed(Target::Config, false, false);
}

#[test]
fn config_xml_and_bsl_edit_packs() {
    assert_label_packed(Target::Config, true, false);
}

#[test]
fn epf_backup_keeps_original_target() {
    assert_label_packed(Target::Epf, true, true);
}

#[test]
fn config_backup_keeps_original_target() {
    assert_label_packed(Target::Config, true, true);
}

#[test]
fn single_atom_edit_changes_only_that_atom() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        let before = w.snap();
        w.edit_label(&[LEFT]);
        let stream_path = w.stream_path();
        let out = run(w.root(), &["pack", "."]);
        assert!(out.status.success(), "{target:?}: {}", text(&out));
        let after = w.snap();
        let (old, new) = (&before[&stream_path], &after[&stream_path]);
        // left 79 -> 90 and right 132 -> 143 keep the byte lengths: only two atoms of one control differ
        assert_eq!(old.len(), new.len());
        let differing: Vec<usize> = (0..old.len()).filter(|i| old[*i] != new[*i]).collect();
        assert!(!differing.is_empty() && differing.len() <= 6, "{target:?}: {differing:?}");
        assert!(differing.last().unwrap() - differing[0] < 80, "{target:?}: {differing:?}");
    }
}

fn append_to_stream(w: &Work) {
    let p = w.src_file("FormStream.bin");
    let mut data = fs::read(&p).unwrap();
    data.extend_from_slice(b"\r\n");
    fs::write(p, data).unwrap();
}

#[test]
fn independent_stream_edit_still_packs_when_xml_is_unchanged() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        let stream_path = w.stream_path();
        append_to_stream(&w);
        let edited = w.snap();
        let out = run(w.root(), &["pack", "."]);
        assert!(out.status.success(), "{target:?}: {}", text(&out));
        let after = w.snap();
        assert_eq!(after[&stream_path], edited[&stream_path]);
        let fresh = fresh_extract(&after);
        assert_eq!(fresh[&stream_path], edited[&stream_path], "{target:?}: packed stream differs");
    }
}

#[test]
fn simultaneous_stream_and_xml_layout_edits_are_ambiguous() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        w.edit_label(&[LEFT]);
        append_to_stream(&w);
        let before = w.snap();
        let out = run(w.root(), &["pack", "."]);
        let msg = text(&out);
        assert!(!out.status.success(), "{target:?}: {msg}");
        assert!(
            msg.contains("FORM_EDIT_REJECTED") && msg.contains("ambiguous"),
            "{target:?}: {msg}"
        );
        assert_eq!(before, w.snap(), "{target:?}: files changed after rejection");
    }
}

#[test]
fn formatting_only_xml_edit_is_not_a_layout_edit() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        let p = w.src_file("Form.xml");
        let xml = fs::read_to_string(&p).unwrap();
        let reformatted = xml
            .replacen("\n  <Caption>", "\n\n  <!-- reviewed -->\n  <Caption>", 1)
            .replace('\n', "\r\n");
        assert_ne!(xml, reformatted);
        fs::write(&p, &reformatted).unwrap();
        let target_path = w.target_path();
        let before = w.snap();
        let out = run(w.root(), &["pack", "."]);
        assert!(out.status.success(), "{target:?}: {}", text(&out));
        let after = w.snap();
        assert_eq!(
            before[&target_path], after[&target_path],
            "{target:?}: container rewritten for a formatting-only edit"
        );
        let streams: Vec<&String> = before.keys().filter(|k| k.ends_with("FormStream.bin")).collect();
        assert!(streams.iter().all(|k| before[*k] == after[*k]));
    }
}

#[test]
fn unsupported_control_edits_are_rejected_without_changes() {
    let edits = [
        ("name=\"Надпись1\" id=\"5\"", "name=\"Кнопка1\" id=\"5\""),
        ("<Click id=\"0\">Надпись1Нажатие", "<Click id=\"999\">Надпись1Нажатие"),
    ];
    for target in [Target::Epf, Target::Config] {
        for edit in edits {
            let w = Work::new(target);
            let p = w.src_file("Form.xml");
            let xml = fs::read_to_string(&p).unwrap();
            assert!(xml.contains(edit.0), "{}", edit.0);
            fs::write(&p, xml.replacen(edit.0, edit.1, 1)).unwrap();
            let before = w.snap();
            let out = run(w.root(), &["pack", "."]);
            let msg = text(&out);
            assert!(!out.status.success(), "{target:?} {edit:?}: {msg}");
            assert!(msg.contains("FORM_EDIT_REJECTED"), "{target:?} {edit:?}: {msg}");
            assert_eq!(before, w.snap(), "{target:?} {edit:?}: files changed after rejection");
        }
    }
}

// ----- staged verification and publication failures (in-process: the test hooks are thread-local)

fn pack_in_process(w: &Work, backup: bool) -> i32 {
    let mut args = vec![
        "pack".to_string(),
        w.root().to_string_lossy().into_owned(),
        "-j".into(),
        "1".into(),
        "--no-progress".into(),
    ];
    if backup {
        args.push("--backup".into());
    }
    oneform::cli_oneform::main(&args)
}

/// The staged pack of the current workspace through the library, without publishing.
fn stage(w: &Work) -> Result<(), ExtractionError> {
    use oneform::formbin::{discover_config_objects, form_bin_rel, object_dir, src_dir, stage_config_pack, Sources};
    match w.target {
        Target::Epf => {
            let src = read_tree(&w.root().join("src")).unwrap();
            let epf = fs::read(w.root().join("donor.epf")).unwrap();
            let cfg = serde_json::json!({"schemaVersion": 1});
            let opts = oneform::stage::StageOpts {
                use_original_names: true,
                ..Default::default()
            };
            let lock = oneform::cli_oneform::DEFAULT_TOOLS_LOCK.as_bytes();
            oneform::pack::stage_pack(&epf, "donor.epf", &src, &cfg, &cfg, lock, &opts).map(|_| ())
        }
        Target::Config => {
            let obj = discover_config_objects(w.root())
                .into_iter()
                .find(|o| o.rel.starts_with("Documents/"))
                .unwrap();
            let sources: Sources = obj
                .forms
                .iter()
                .map(|n| {
                    let bin = object_dir(w.root(), &obj).join(form_bin_rel(&obj, n));
                    (n.clone(), fs::read(bin).unwrap())
                })
                .collect();
            let existing = read_tree(&src_dir(w.root(), &obj)).unwrap();
            stage_config_pack(w.root(), &obj, &sources, &existing, None, None).map(|_| ())
        }
    }
}

#[test]
fn staged_verification_rejects_changes_outside_authorized_ranges() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        w.edit_label(&[LEFT]);
        w.edit_bsl();
        let before = w.snap();
        // A defective planner that also touches a byte before the authorized range.
        set_patch_hook(Some(Box::new(|p| {
            let start = p.replacements[0].range.start;
            let i = p.bytes[..start].iter().rposition(u8::is_ascii_digit).unwrap();
            p.bytes[i] = if p.bytes[i] == b'1' { b'2' } else { b'1' };
        })));
        let err = stage(&w).expect_err("tampered patch must not verify");
        let code = pack_in_process(&w, true);
        set_patch_hook(None);
        assert_eq!(err.code, PUBLISH_VERIFY_FAILED, "{target:?}: {err}");
        assert_eq!(err.params["kind"], "unauthorized-stream-change", "{target:?}: {err}");
        assert_ne!(code, 0, "{target:?}");
        let after = w.snap();
        let changed: Vec<&String> = before
            .keys()
            .chain(after.keys())
            .filter(|k| before.get(*k) != after.get(*k))
            .collect();
        assert!(changed.is_empty(), "{target:?}: {changed:?}");
    }
}

#[test]
fn staged_verification_rejects_a_mismatch_with_the_requested_xml() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        w.edit_label(&[LEFT]);
        let before = w.snap();
        // Consistent but wrong: the recorded replacement and the stream both hold 91 instead of 90.
        set_patch_hook(Some(Box::new(|p| {
            let r = p.replacements.iter().position(|r| r.bytes == b"90").expect("left replacement");
            let start = p.replacements[r].range.start;
            // same length as the original value, so no later offset moves
            p.bytes[start + 1] = b'1';
            p.replacements[r].bytes = b"91".to_vec();
        })));
        let err = stage(&w).expect_err("wrong value must not verify");
        let code = pack_in_process(&w, false);
        set_patch_hook(None);
        assert_eq!(err.code, PUBLISH_VERIFY_FAILED, "{target:?}: {err}");
        assert_eq!(err.params["kind"], "xml-mismatch", "{target:?}: {err}");
        assert_ne!(code, 0, "{target:?}");
        assert_eq!(before, w.snap(), "{target:?}: files changed after a failed verification");
    }
}

#[test]
fn failed_publication_restores_target_and_keeps_user_edits() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        w.edit_label(&[CAPTION, LEFT]);
        w.edit_bsl();
        let target_path = w.target_path();
        let before = w.snap();
        // src publication fails after the container has been replaced
        set_rename_hook(Some(Box::new(|a, b| {
            if a.file_name().is_some_and(|n| n == "NewSrc") {
                Err(std::io::Error::other("disk says no"))
            } else {
                fs::rename(a, b)
            }
        })));
        let code = pack_in_process(&w, true);
        set_rename_hook(None);
        assert_ne!(code, 0, "{target:?}");
        let after = w.snap();
        assert_eq!(
            after[&target_path], before[&target_path],
            "{target:?}: original target was not restored"
        );
        assert_eq!(
            after[&format!("{target_path}.bak")],
            before[&target_path],
            "{target:?}: backup differs from the original"
        );
        let changed: Vec<&String> = before
            .keys()
            .chain(after.keys())
            .filter(|k| !k.ends_with(".bak") && before.get(*k) != after.get(*k))
            .collect();
        assert!(changed.is_empty(), "{target:?}: user edits or sources changed: {changed:?}");
        // the same edits pack once the failure is gone
        assert_eq!(pack_in_process(&w, false), 0, "{target:?}");
        assert_ne!(w.snap()[&target_path], before[&target_path]);
    }
}

#[test]
fn unsupported_edit_error_is_actionable() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        w.edit_xml();
        let err = stage(&w).expect_err("form-level edit is unsupported");
        assert_eq!(err.code, FORM_EDIT_REJECTED);
        assert!(err.to_string().contains("Form.xml"), "{err}");
        assert!(err.params["problems"].to_string().contains("Width"), "{err:?}");
    }
}

#[test]
fn panel_resize_packs_with_derived_page_geometry() {
    for target in [Target::Epf, Target::Config] {
        let w = Work::new(target);
        let p = w.src_file("Form.xml");
        let xml = fs::read_to_string(&p).unwrap();
        let (from, to) = (xml.find("name=\"Панель1\"").unwrap(), xml.find("name=\"Панель2\"").unwrap());
        let block = &xml[from..to];
        let edited_block = block
            .replacen("<Width>278</Width>", "<Width>300</Width>", 1)
            .replacen("<Height>154</Height>", "<Height>160</Height>", 1);
        // the pages follow the panel: 276x134 -> 298x140, a derived change the user does not edit
        let derived_block = edited_block
            .replace("<Width>276</Width>", "<Width>298</Width>")
            .replace("<Height>134</Height>", "<Height>140</Height>");
        assert_eq!(block.matches("<Width>276</Width>").count(), 2);
        let (head, tail) = (&xml[..from], &xml[to..]);
        fs::write(&p, format!("{head}{edited_block}{tail}")).unwrap();
        let out = run(w.root(), &["pack", "."]);
        assert!(out.status.success(), "{target:?}: {}", text(&out));
        let after = w.snap();
        let fresh = fresh_extract(&after);
        let xml_path = rel(&w, &p);
        assert_eq!(
            no_carriers(&fresh[&xml_path]),
            no_carriers(format!("{head}{derived_block}{tail}").as_bytes()),
            "{target:?}: re-extracted panel and page geometry"
        );
    }
}

// ---------------------------------------------------------------- structural pack (add/remove controls)

/// Add a LabelDecoration and remove ПолеВвода1 in Form.xml, pack, verify both targets.
fn assert_structural_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    // add a new label after Надпись1
    let anchor = xml.find(LABEL).expect("label");
    let insert_at = xml[anchor..].find("</LabelDecoration>").map(|i| anchor + i + "</LabelDecoration>".len()).unwrap();
    let new_control = "\n        <LabelDecoration name=\"Добавленная\" id=\"999\">\n          <Caption>\n            <Item language=\"ru\">Новая надпись</Item>\n          </Caption>\n          <Left>500</Left>\n          <Top>500</Top>\n          <Width>80</Width>\n          <Height>20</Height>\n          <ZOrder>50</ZOrder>\n          <TabOrder>50</TabOrder>\n        </LabelDecoration>";
    xml.insert_str(insert_at, new_control);
    // remove ПолеВвода1
    let at = xml.find("<InputField name=\"ПолеВвода1\"").expect("input");
    let end = at + xml[at..].find("</InputField>").unwrap() + "</InputField>\n".len();
    let start = xml[..at].rfind('\n').unwrap() + 1;
    xml.replace_range(start..end, "");
    fs::write(&xml_file, &xml).unwrap();
    let bsl_file = w.src_file("FormModule.bsl");
    let bsl_before = fs::read(&bsl_file).unwrap();
    let target_path = w.target_path();
    let before = w.snap();

    let out = run(w.root(), &["pack", ".", "--backup"]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));
    let after = w.snap();
    assert_ne!(before[&target_path], after[&target_path], "{target:?}: target not rewritten");
    assert_eq!(fs::read(&bsl_file).unwrap(), bsl_before, "{target:?}: BSL changed");

    // the published src Form.xml is the canonical regenerated projection with the allocated id
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap().replace("\r\n", "\n");
    assert!(packed_xml.contains("name=\"Добавленная\" id=\"37\""), "{target:?}: added control missing/renumbered\n{packed_xml}");
    assert!(!packed_xml.contains("<InputField name=\"ПолеВвода1\""), "{target:?}: removed control still present");
    assert!(packed_xml.contains("name=\"ПолеВвода1\""), "{target:?}: attribute ПолеВвода1 must stay");

    // fresh extraction of the packed target reproduces the published XML exactly
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
}

#[test]
fn epf_structural_add_remove_packs() {
    assert_structural_pack(Target::Epf);
}

#[test]
fn config_structural_add_remove_packs() {
    assert_structural_pack(Target::Config);
}

fn assert_discovered_properties_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    // Patch ПолеВвода1: Enabled=false, ReadOnly=true, ChoiceButton=false, ClearButton=false, MarkNegatives=true
    let inp_at = xml.find("<InputField name=\"ПолеВвода1\"").expect("input");
    let inp_end = inp_at + xml[inp_at..].find("</InputField>").unwrap();
    let inp_block = xml[inp_at..inp_end]
        .replacen(
            "<ChoiceButton>true</ChoiceButton>",
            "<Enabled>false</Enabled>\n          <ChoiceButton>false</ChoiceButton>",
            1,
        )
        .replacen("<ClearButton>true</ClearButton>", "<ClearButton>false</ClearButton>", 1)
        .replacen(
            "<OpenButton>true</OpenButton>",
            "<OpenButton>true</OpenButton>\n          <ReadOnly>true</ReadOnly>\n          <MarkNegatives>true</MarkNegatives>",
            1,
        );
    xml.replace_range(inp_at..inp_end, &inp_block);

    // Patch Флажок1: ThreeState=true, TitleLocation=Left
    let chk_at = xml.find("<CheckBox name=\"Флажок1\"").expect("checkbox");
    let chk_end = chk_at + xml[chk_at..].find("</CheckBox>").unwrap();
    let chk_block = xml[chk_at..chk_end].replacen(
        "<TabOrder>5</TabOrder>",
        "<TabOrder>5</TabOrder>\n          <TitleLocation>Left</TitleLocation>\n          <ThreeState>true</ThreeState>",
        1,
    );
    xml.replace_range(chk_at..chk_end, &chk_block);

    // Patch Надпись1: Hyperlink=true, HorizontalAlign=Center
    let lbl_at = xml.find(LABEL).expect("label");
    let lbl_end = lbl_at + xml[lbl_at..].find("</LabelDecoration>").unwrap();
    let lbl_block = xml[lbl_at..lbl_end].replacen(
        "<TabOrder>3</TabOrder>",
        "<TabOrder>3</TabOrder>\n          <HorizontalAlign>Center</HorizontalAlign>\n          <Hyperlink>true</Hyperlink>",
        1,
    );
    xml.replace_range(lbl_at..lbl_end, &lbl_block);

    // Patch ТабличноеПоле1: Footer=true, SelectionMode=Multi
    let tbl_at = xml.find("<Table name=\"ТабличноеПоле1\"").expect("table");
    let tbl_end = tbl_at + xml[tbl_at..].find("</Table>").unwrap();
    let tbl_block = xml[tbl_at..tbl_end].replacen(
        "<Footer>false</Footer>",
        "<Footer>true</Footer>\n          <SelectionMode>Multi</SelectionMode>",
        1,
    );
    xml.replace_range(tbl_at..tbl_end, &tbl_block);

    fs::write(&xml_file, &xml).unwrap();
    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    for needle in [
        "<Enabled>false</Enabled>",
        "<ReadOnly>true</ReadOnly>",
        "<MarkNegatives>true</MarkNegatives>",
        "<TitleLocation>Left</TitleLocation>",
        "<ThreeState>true</ThreeState>",
        "<HorizontalAlign>Center</HorizontalAlign>",
        "<Hyperlink>true</Hyperlink>",
        "<Footer>true</Footer>",
        "<SelectionMode>Multi</SelectionMode>",
    ] {
        assert!(packed_xml.contains(needle), "{target:?}: missing {needle} in:\n{packed_xml}");
    }
}

#[test]
fn epf_discovered_control_properties_pack() {
    assert_discovered_properties_pack(Target::Epf);
}

#[test]
fn config_discovered_control_properties_pack() {
    assert_discovered_properties_pack(Target::Config);
}

fn assert_rename_and_events_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    // Rename Надпись1 -> НадписьПереименованная and change its Click handler
    xml = xml
        .replacen(r#"<LabelDecoration name="Надпись1" id="5">"#, r#"<LabelDecoration name="НадписьПереименованная" id="5">"#, 1)
        .replacen("<Click id=\"0\">Надпись1Нажатие</Click>", "<Click id=\"0\">НовыйОбработчикНадписи</Click>", 1);
    // Clear Events on Кнопка1
    xml = xml.replacen("          <Events>\n            <Click id=\"0\">Кнопка1Нажатие</Click>\n          </Events>\n", "", 1);
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    assert!(packed_xml.contains(r#"<LabelDecoration name="НадписьПереименованная" id="5">"#), "{target:?}: missing renamed label");
    assert!(packed_xml.contains("<Click id=\"0\">НовыйОбработчикНадписи</Click>"), "{target:?}: missing updated event");
    assert!(!packed_xml.contains("Кнопка1Нажатие"), "{target:?}: removed event still present");
}

#[test]
fn epf_existing_control_rename_and_events_pack() {
    assert_rename_and_events_pack(Target::Epf);
}

#[test]
fn config_existing_control_rename_and_events_pack() {
    assert_rename_and_events_pack(Target::Config);
}

fn assert_panel_pages_and_table_columns_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    // Edit Панель1 Page Страница2 title and Страница1 Enabled/Visible
    xml = xml
        .replacen(
            "<Page name=\"Страница2\">\n              <Title>\n                <Item language=\"ru\">Страница2</Item>",
            "<Page name=\"Страница2\">\n              <Title>\n                <Item language=\"ru\">Вкладка 2</Item>",
            1,
        )
        .replacen("              <Visible>false</Visible>", "              <Enabled>false</Enabled>", 1);
    // Edit ТабличноеПоле1 Column Колонка1
    xml = xml
        .replacen(r#"<Column name="Колонка1" id="0">"#, r#"<Column name="КолонкаОсновная" id="0">"#, 1)
        .replacen("<Item language=\"ru\">Колонка1</Item>", "<Item language=\"ru\">Заголовок 1</Item>", 1)
        .replacen(
            "<Width>100</Width>",
            "<FooterText>\n                <Item language=\"ru\">Итого</Item>\n              </FooterText>\n              <Tooltip>\n                <Item language=\"ru\">Подсказка колонки</Item>\n              </Tooltip>\n              <Width>155</Width>\n              <Visible>false</Visible>\n              <ReadOnly>true</ReadOnly>",
            1,
    );
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    for needle in [
        "<Item language=\"ru\">Вкладка 2</Item>",
        "<Enabled>false</Enabled>",
        r#"<Column name="КолонкаОсновная" id="0">"#,
        "<Item language=\"ru\">Заголовок 1</Item>",
        "<Item language=\"ru\">Итого</Item>",
        "<Item language=\"ru\">Подсказка колонки</Item>",
        "<Width>155</Width>",
        "<ReadOnly>true</ReadOnly>",
    ] {
        assert!(packed_xml.contains(needle), "{target:?}: missing {needle} in:\n{packed_xml}");
    }
}

#[test]
fn epf_panel_pages_and_table_columns_pack() {
    assert_panel_pages_and_table_columns_pack(Target::Epf);
}

#[test]
fn config_panel_pages_and_table_columns_pack() {
    assert_panel_pages_and_table_columns_pack(Target::Config);
}

fn assert_form_attributes_and_datapath_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    // Add attribute id=18 name=НовыйСтроковыйРеквизит
    xml = xml.replacen(
        "  </Attributes>",
        "    <Attribute id=\"18\" name=\"НовыйСтроковыйРеквизит\">\n      <Main>true</Main>\n      <ValueType kind=\"String\" composite=\"false\"/>\n    </Attribute>\n    <Attribute id=\"19\" name=\"СтрокаСДлиной\">\n      <ValueType kind=\"String\" composite=\"false\" length=\"100\"/>\n    </Attribute>\n  </Attributes>",
        1,
    );
    // Rebind ПолеВвода1 to attributeId=18
    xml = xml.replacen(
        "<DataPath attributeId=\"3\">ПолеВвода1</DataPath>",
        "<DataPath attributeId=\"18\">НовыйСтроковыйРеквизит</DataPath>",
        1,
    );
    // Add a new InputField bound to attributeId=14 (ПолеВвода7)
    let new_inp = "\n        <InputField name=\"НовоеПолеСПривязкой\" id=\"999\">\n          <DataPath attributeId=\"14\">ПолеВвода7</DataPath>\n          <Left>520</Left>\n          <Top>500</Top>\n          <Width>120</Width>\n          <Height>20</Height>\n          <ZOrder>55</ZOrder>\n          <TabOrder>55</TabOrder>\n        </InputField>";
    let anchor = xml.find(LABEL).expect("label");
    let insert_at = xml[anchor..].find("</LabelDecoration>").map(|i| anchor + i + "</LabelDecoration>".len()).unwrap();
    xml.insert_str(insert_at, new_inp);
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    for needle in [
        r#"<Attribute id="18" name="НовыйСтроковыйРеквизит">"#,
        r#"<ValueType kind="String" composite="false" length="100"/>"#, // a string with a length reads back equal to the request
        r#"<DataPath attributeId="18">НовыйСтроковыйРеквизит</DataPath>"#,
        r#"<InputField name="НовоеПолеСПривязкой" id="37">"#,
        r#"<DataPath attributeId="14">ПолеВвода7</DataPath>"#,
    ] {
        assert!(packed_xml.contains(needle), "{target:?}: missing {needle} in:\n{packed_xml}");
    }
}

#[test]
fn epf_form_attributes_and_datapath_pack() {
    assert_form_attributes_and_datapath_pack(Target::Epf);
}

#[test]
fn config_form_attributes_and_datapath_pack() {
    assert_form_attributes_and_datapath_pack(Target::Config);
}

fn assert_usualgroup_and_choicefield_donor_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    let additions = "\n        <UsualGroup name=\"ГруппаНовая\" id=\"998\">\n          <Caption>\n            <Item language=\"ru\">Группа 1</Item>\n          </Caption>\n          <Left>500</Left>\n          <Top>500</Top>\n          <Width>200</Width>\n          <Height>80</Height>\n          <ZOrder>56</ZOrder>\n          <TabOrder>56</TabOrder>\n          <BorderStyle code=\"4\">SingleLine</BorderStyle>\n        </UsualGroup>\n        <ChoiceField name=\"ПолеВыбораНовое\" id=\"999\">\n          <Left>510</Left>\n          <Top>520</Top>\n          <Width>140</Width>\n          <Height>20</Height>\n          <ZOrder>57</ZOrder>\n          <TabOrder>57</TabOrder>\n          <Events>\n            <OnChange id=\"2147483647\">ПолеВыбораНовоеПриИзменении</OnChange>\n          </Events>\n        </ChoiceField>";
    let anchor = xml.find(LABEL).expect("label");
    let insert_at = xml[anchor..].find("</LabelDecoration>").map(|i| anchor + i + "</LabelDecoration>".len()).unwrap();
    xml.insert_str(insert_at, additions);
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    for needle in [
        r#"<UsualGroup name="ГруппаНовая" id="37">"#,
        r#"<BorderStyle code="4">SingleLine</BorderStyle>"#,
        r#"<ChoiceField name="ПолеВыбораНовое" id="38">"#,
        r#"<OnChange id="2147483647">ПолеВыбораНовоеПриИзменении</OnChange>"#,
    ] {
        assert!(packed_xml.contains(needle), "{target:?}: missing {needle} in:\n{packed_xml}");
    }
}

#[test]
fn epf_usualgroup_and_choicefield_donor_pack() {
    assert_usualgroup_and_choicefield_donor_pack(Target::Epf);
}

#[test]
fn config_usualgroup_and_choicefield_donor_pack() {
    assert_usualgroup_and_choicefield_donor_pack(Target::Config);
}

fn assert_remaining_properties_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    let inp_at = xml.find("<InputField name=\"ПолеВвода1\"").expect("input");
    let inp_end = inp_at + xml[inp_at..].find("</InputField>").unwrap();
    let inp_block = xml[inp_at..inp_end].replacen(
        "<ValueType kind=\"ValueList\" composite=\"false\"/>",
        "<BackColor kind=\"rgb\" value=\"#FF0000\"/>\n          <TextColor kind=\"rgb\" value=\"#0000FF\"/>\n          <BorderColor kind=\"rgb\" value=\"#00FF00\"/>\n          <Font bold=\"true\" source=\"bin\"/>\n          <ValueType kind=\"String\" composite=\"false\"/>\n          <HorizontalScrollBar>Use</HorizontalScrollBar>\n          <VerticalScrollBar>Auto</VerticalScrollBar>",
        1,
    );
    xml.replace_range(inp_at..inp_end, &inp_block);
    xml = xml
        .replacen(
            "<Item language=\"ru\">Обработка  ВнешняяОбработка1</Item>",
            "<Item language=\"ru\">Новый заголовок формы</Item>",
            1,
        )
        .replacen(
            "<BeforeOpen id=\"70000\">ПередОткрытием</BeforeOpen>",
            "<BeforeOpen id=\"70000\">ПередОткрытиемНовый</BeforeOpen>",
            1,
        )
        .replacen(
            "<Footer>false</Footer>",
            "<Footer>false</Footer>\n          <InitialTreeView>ExpandTopLevel</InitialTreeView>",
            1,
        )
        .replacen(
            "<DataPath attributeId=\"2\">ТабличноеПоле1.Колонка1</DataPath>",
            "<DataPath attributeId=\"2\">ТабличноеПоле1.КолонкаПервая</DataPath>",
            1,
        )
        .replacen(
            "<StartChoice id=\"2\">ТабличноеПоле1Колонка1НачалоВыбора</StartChoice>",
            "<StartChoice id=\"2\">НовыйОбработчикКолонки</StartChoice>",
            1,
        )
        .replacen(
            "action=\"КнопкаВыполнитьНажатие\"",
            "action=\"КнопкаВыполнитьНовоеНажатие\"",
            1,
    );
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    for needle in [
        "<Item language=\"ru\">Новый заголовок формы</Item>",
        "<BeforeOpen id=\"70000\">ПередОткрытиемНовый</BeforeOpen>",
        r##"<BackColor kind="rgb" value="#FF0000"/>"##,
        r##"<TextColor kind="rgb" value="#0000FF"/>"##,
        r##"<BorderColor kind="rgb" value="#00FF00"/>"##,
        r#"<Font bold="true" source="bin"/>"#,
        "<HorizontalScrollBar>Use</HorizontalScrollBar>",
        "<VerticalScrollBar>Auto</VerticalScrollBar>",
        "<InitialTreeView>ExpandTopLevel</InitialTreeView>",
        "<DataPath attributeId=\"2\">ТабличноеПоле1.КолонкаПервая</DataPath>",
        "<StartChoice id=\"2\">НовыйОбработчикКолонки</StartChoice>",
        "action=\"КнопкаВыполнитьНовоеНажатие\"",
    ] {
        assert!(packed_xml.contains(needle), "{target:?}: missing {needle} in:\n{packed_xml}");
    }
}

#[test]
fn epf_remaining_form_and_control_properties_pack() {
    assert_remaining_properties_pack(Target::Epf);
}

#[test]
fn config_remaining_form_and_control_properties_pack() {
    assert_remaining_properties_pack(Target::Config);
}

const CONTRACTOR_GUID: &str = "47e6dd42-cd5c-4e3a-9bc4-14816ed53d38";

/// Minimal configuration metadata file declaring the `CatalogRef.Контрагенты` type of the test configuration.
fn write_contractor_catalog(root: &Path) {
    fs::create_dir_all(root.join("Catalogs")).unwrap();
    let xml = format!(
        "<MetaDataObject><Catalog><InternalInfo>\n<xr:GeneratedType name=\"CatalogRef.Контрагенты\" category=\"Ref\"><xr:TypeId>{CONTRACTOR_GUID}</xr:TypeId></xr:GeneratedType>\n</InternalInfo></Catalog></MetaDataObject>"
    );
    fs::write(root.join("Catalogs/Контрагенты.xml"), xml).unwrap();
}

fn assert_reference_and_qualified_valuetypes_pack(target: Target) {
    let w = Work::new(target);
    let type_attr = match target {
        Target::Epf => format!("refGuid=\"{CONTRACTOR_GUID}\""),
        Target::Config => {
            write_contractor_catalog(w.root());
            "ref=\"Справочник.Контрагенты\"".to_string()
        }
    };
    let xml_file = w.src_file("Form.xml");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    let new_attr = format!("    <Attribute id=\"16\" name=\"КонтрагентРеквизит\">\n      <Main>false</Main>\n      <ValueType kind=\"Ref\" composite=\"false\" {type_attr}/>\n    </Attribute>\n  </Attributes>");
    xml = xml.replacen("  </Attributes>", &new_attr, 1);
    xml = replace_after_appearance(&xml,
        "<InputField name=\"ПолеВвода7\" id=\"33\">\n          <DataPath attributeId=\"14\">ПолеВвода7</DataPath>\n          <Left>303</Left>\n          <Top>606</Top>\n          <Width>136</Width>\n          <Height>21</Height>\n          <ZOrder>25</ZOrder>\n          <TabOrder>26</TabOrder>\n          <Anchors top=\"Bottom\" topTarget=\"15\" topOffset=\"35\" bottom=\"Top\" bottomTarget=\"33\" bottomOffset=\"21\" left=\"Right\" leftTarget=\"31\" leftOffset=\"5\" right=\"Right\" rightOffset=\"-496\"/>",
        "\n          <ValueType kind=\"String\" composite=\"false\"/>",
        "\n          <ValueType kind=\"String\" composite=\"false\" length=\"50\"/>",
        1,
    );
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let packed_xml = String::from_utf8(after[&rel(&w, &xml_file)].clone()).unwrap();
    assert!(packed_xml.contains("name=\"КонтрагентРеквизит\""), "{target:?}: missing attribute in:\n{packed_xml}");
    assert!(packed_xml.contains("refGuid=\"47e6dd42-cd5c-4e3a-9bc4-14816ed53d38\""), "{target:?}: missing refGuid in:\n{packed_xml}");
    if matches!(target, Target::Config) {
        assert!(packed_xml.contains("title=\"Справочник.Контрагенты\""), "{target:?}: missing title in:\n{packed_xml}");
    }
    assert!(packed_xml.contains("length=\"50\""), "{target:?}: missing length in:\n{packed_xml}");
}

#[test]
fn epf_reference_and_qualified_valuetypes_pack() {
    assert_reference_and_qualified_valuetypes_pack(Target::Epf);
}

#[test]
fn config_reference_and_qualified_valuetypes_pack() {
    assert_reference_and_qualified_valuetypes_pack(Target::Config);
}

fn assert_anchors_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let stream_file = w.src_file("FormStream.bin");
    let before = w.snap();
    let stream_before = String::from_utf8_lossy(&before[&rel(&w, &stream_file)]).into_owned();
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    xml = xml
        .replacen(
            r#"<Anchors bottom="Top" bottomOffset="218" bottomAlt="Bottom" bottomAltOffset="-455" right="Left" rightOffset="214" rightAlt="Left" rightAltTarget="14" rightAltOffset="-489"/>"#,
            r#"<Anchors left="Left" top="Top" right="Right" bottom="Bottom" leftOffset="6" topOffset="24" rightOffset="-6" bottomOffset="-6"/>"#,
            1,
        )
        .replacen(
            r#"<Anchors bottom="Top" bottomTarget="7" bottomOffset="19" right="Left" rightTarget="14" rightOffset="-288"/>"#,
            r#"<Anchors right="Right" rightOffset="-10"/>"#,
            1,
    );
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    assert_eq!(fresh[&rel(&w, &stream_file)], after[&rel(&w, &stream_file)], "{target:?}: re-extracted FormStream.bin differs");
    let stream_after = String::from_utf8_lossy(&after[&rel(&w, &stream_file)]).into_owned();
    for edge in ["{2,0,0,24}", "{2,0,1,-6}", "{2,0,2,6}", "{2,0,3,-6}", "{2,0,3,-10}"] {
        let count = |s: &str| s.matches(edge).count();
        assert!(count(&stream_after) > count(&stream_before), "{target:?}: {edge} not added to the packed stream");
    }
}

#[test]
fn epf_anchors_pack() {
    assert_anchors_pack(Target::Epf);
}

#[test]
fn config_anchors_pack() {
    assert_anchors_pack(Target::Config);
}

/// `extract` now shows the extended properties, so a test that inserts a property line next to the baseline's own
/// would duplicate it: of two identical-tag single-line properties of one control the later (the edit) wins.
fn dedupe_control_properties(xml: &str) -> String {
    let lines: Vec<&str> = xml.split('\n').collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut last: std::collections::HashMap<(usize, String), usize> = std::collections::HashMap::new();
    let mut keys: Vec<Option<(usize, String)>> = vec![None; lines.len()];
    for (n, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        if !t.starts_with('<') || t.starts_with("</") || t.starts_with("<?") {
            continue;
        }
        let ind = indent(l);
        while stack.last().is_some_and(|&(i, _)| i >= ind) {
            stack.pop();
        }
        let single = t.ends_with("/>") || (t.contains("</") && !t.starts_with("<!--"));
        if let Some(&(pi, pn)) = stack.last() {
            let parent = lines[pn];
            if ind == pi + 2 && single && parent.contains(" name=\"") && parent.contains(" id=\"") {
                let tag: String = t[1..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                keys[n] = Some((pn, tag.clone()));
                last.insert((pn, tag), n);
            }
        }
        if !single {
            stack.push((ind, n));
        }
    }
    let kept: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(n, _)| keys[*n].as_ref().is_none_or(|k| last[k] == *n))
        .map(|(_, l)| *l)
        .collect();
    kept.join("\n")
}

fn assert_inputfield_flags_and_typelink_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let stream_file = w.src_file("FormStream.bin");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    xml = replace_after_appearance(&xml,
            "<InputField name=\"ПолеВвода7\" id=\"33\">\n          <DataPath attributeId=\"14\">ПолеВвода7</DataPath>\n          <Left>303</Left>\n          <Top>606</Top>\n          <Width>136</Width>\n          <Height>21</Height>\n          <ZOrder>25</ZOrder>\n          <TabOrder>26</TabOrder>\n          <Anchors top=\"Bottom\" topTarget=\"15\" topOffset=\"35\" bottom=\"Top\" bottomTarget=\"33\" bottomOffset=\"21\" left=\"Right\" leftTarget=\"31\" leftOffset=\"5\" right=\"Right\" rightOffset=\"-496\"/>",
            "\n          <ValueType kind=\"String\" composite=\"false\"/>\n          <ChoiceButton>false</ChoiceButton>\n          <ClearButton>false</ClearButton>\n          <OpenButton>false</OpenButton>",
            "\n          <ValueType kind=\"String\" composite=\"false\"/>\n          <ChoiceButton>false</ChoiceButton>\n          <ClearButton>false</ClearButton>\n          <OpenButton>false</OpenButton>\n          <ModifiesData>true</ModifiesData>\n          <SkipOnInput>true</SkipOnInput>\n          <FirstInGroup>true</FirstInGroup>\n          <MultiLine>true</MultiLine>\n          <PasswordMode>true</PasswordMode>\n          <ExtendedEdit>true</ExtendedEdit>\n          <Mask>@@@-999</Mask>\n          <ChooseType>false</ChooseType>\n          <TextEdit>false</TextEdit>\n          <HorizontalAlign>Center</HorizontalAlign>\n          <VerticalAlign>Bottom</VerticalAlign>",
            1,
    );
    xml = replace_after_appearance(&xml,
            "<InputField name=\"ПолеВвода6\" id=\"32\">\n          <DataPath attributeId=\"13\">ПолеВвода6</DataPath>\n          <Left>303</Left>\n          <Top>580</Top>\n          <Width>136</Width>\n          <Height>21</Height>\n          <ZOrder>24</ZOrder>\n          <TabOrder>23</TabOrder>\n          <Anchors top=\"Bottom\" topTarget=\"15\" topOffset=\"9\" bottom=\"Top\" bottomTarget=\"32\" bottomOffset=\"21\" right=\"Right\" rightOffset=\"-496\"/>",
            "\n          <ValueType kind=\"Ref\" composite=\"true\"/>\n          <ChoiceButton>true</ChoiceButton>\n          <ClearButton>true</ClearButton>\n          <OpenButton>false</OpenButton>",
            "\n          <ValueType kind=\"Ref\" composite=\"true\"/>\n          <ChoiceButton>true</ChoiceButton>\n          <ClearButton>true</ClearButton>\n          <OpenButton>false</OpenButton>\n          <TypeLink attributeId=\"14\">ПолеВвода7</TypeLink>",
            1,
    );
    xml = dedupe_control_properties(&xml);
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let stream_after: String = String::from_utf8_lossy(&after[&rel(&w, &stream_file)])
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(stream_after.contains("{14,\"ПолеВвода7\",4294967295,1,1,1}"), "{target:?}: meta flags missing");
    assert!(stream_after.contains("9a7643d2-19e9-45e2-8893-280bc9195a97,{4,{\"U\"},{\"U\"},1,\"@@@-999\",1,1}"), "{target:?}: string ext missing");
    assert!(stream_after.contains("{1,1,{0,c866d8e7-76ff-4334-9d02-144f1efe5e1d,{2,1,{14}}}}"), "{target:?}: TypeLink missing");
}

#[test]
fn epf_inputfield_flags_and_typelink_pack() {
    assert_inputfield_flags_and_typelink_pack(Target::Epf);
}

#[test]
fn config_inputfield_flags_and_typelink_pack() {
    assert_inputfield_flags_and_typelink_pack(Target::Config);
}

fn assert_table_grid_and_flags_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let stream_file = w.src_file("FormStream.bin");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    xml = xml.replacen(
        "<Footer>false</Footer>",
        "<Footer>true</Footer>\n          <Header>false</Header>\n          <HorizontalLines>false</HorizontalLines>\n          <VerticalLines>false</VerticalLines>\n          <ReadOnly>true</ReadOnly>\n          <AutoInsertNewRow>true</AutoInsertNewRow>\n          <ChangeRowSet>false</ChangeRowSet>\n          <ChangePositionOfColumns>false</ChangePositionOfColumns>\n          <ChangeSettingOfColumns>false</ChangeSettingOfColumns>\n          <HorizontalScrollBar>DontUse</HorizontalScrollBar>\n          <VerticalScrollBar>Use</VerticalScrollBar>\n          <InitialListView>End</InitialListView>\n          <Output>Enable</Output>",
        1,
    );
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let expected_flags = (117644301i64 | 2 | 32768) & !1 & !4 & !8 & !1024 & !33554432 & !67108864;
    let stream_after = String::from_utf8_lossy(&after[&rel(&w, &stream_file)]);
    assert!(stream_after.contains(&format!("{{19,{expected_flags},")), "{target:?}: table bitmask missing");
}

#[test]
fn epf_table_grid_and_flags_pack() {
    assert_table_grid_and_flags_pack(Target::Epf);
}

#[test]
fn config_table_grid_and_flags_pack() {
    assert_table_grid_and_flags_pack(Target::Config);
}

fn assert_extended_catalog_properties_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let stream_file = w.src_file("FormStream.bin");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    xml = xml.replacen(
        "<Click id=\"0\">Кнопка1Нажатие</Click>",
        "<Click id=\"0\">Кнопка1Нажатие</Click>\n          </Events>\n          <ButtonBackColor kind=\"rgb\" value=\"#FF0000\"/>\n          <HorizontalAlign>Left</HorizontalAlign>\n          <VerticalAlign>Top</VerticalAlign>\n          <PictureLocation>Right</PictureLocation>\n          <PictureSize>Stretch</PictureSize>\n          <MultiLine>true</MultiLine",
        0,
    );
    xml = xml.replacen(
        "<Click id=\"0\">Кнопка1Нажатие</Click>\n          </Events>",
        "<Click id=\"0\">Кнопка1Нажатие</Click>\n          </Events>\n          <ButtonBackColor kind=\"rgb\" value=\"#FF0000\"/>\n          <HorizontalAlign>Left</HorizontalAlign>\n          <VerticalAlign>Top</VerticalAlign>\n          <PictureLocation>Right</PictureLocation>\n          <PictureSize>Stretch</PictureSize>\n          <MultiLine>true</MultiLine>",
        1,
    );
    xml = xml.replacen(
        "<OnChange id=\"2147483647\">Переключатель1ПриИзменении</OnChange>\n          </Events>",
        "<OnChange id=\"2147483647\">Переключатель1ПриИзменении</OnChange>\n          </Events>\n          <TitleLocation>Left</TitleLocation>\n          <HorizontalAlign>Right</HorizontalAlign>\n          <VerticalAlign>Bottom</VerticalAlign>\n          <SelectionValue>1</SelectionValue>",
        1,
    );
    xml = dedupe_control_properties(&xml);
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let stream_after: String = String::from_utf8_lossy(&after[&rel(&w, &stream_file)])
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(stream_after.contains("{\"N\",1}"), "{target:?}: RadioButton SelectionValue missing");
}

#[test]
fn epf_extended_catalog_properties_pack() {
    assert_extended_catalog_properties_pack(Target::Epf);
}

#[test]
fn config_extended_catalog_properties_pack() {
    assert_extended_catalog_properties_pack(Target::Config);
}

fn assert_table_and_column_catalog_properties_pack(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let stream_file = w.src_file("FormStream.bin");
    let mut xml = fs::read_to_string(&xml_file).unwrap().replace("\r\n", "\n");
    xml = xml.replacen(
        "<Footer>false</Footer>",
        "<Footer>false</Footer>\n          <UseAlternationRowColor>true</UseAlternationRowColor>\n          <EnableStartDrag>true</EnableStartDrag>\n          <FixedLeft>1</FixedLeft>\n          <FixedRight>2</FixedRight>\n          <RowInputMode>AtWindowBottom</RowInputMode>\n          <RowSelectionMode>Row</RowSelectionMode>\n          <HeaderHeight>2</HeaderHeight>\n          <FooterHeight>3</FooterHeight>",
        1,
    );
    xml = xml.replacen(
        "<Editor>InputField</Editor>\n              <Events>",
        "<Editor>InputField</Editor>\n              <ShowHierarchy>true</ShowHierarchy>\n              <ShowInHeader>false</ShowInHeader>\n              <ShowInFooter>false</ShowInFooter>\n              <ChangeSetting>false</ChangeSetting>\n              <ChangePosition>false</ChangePosition>\n              <HeaderHorizontalAlign>Center</HeaderHorizontalAlign>\n              <HorizontalAlignInColumn>Right</HorizontalAlignInColumn>\n              <FooterHorizontalAlign>Center</FooterHorizontalAlign>\n              <CellHeight>4</CellHeight>\n              <Events>",
        1,
    );
    fs::write(&xml_file, &xml).unwrap();

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success(), "{target:?}: {}", text(&out));
    assert!(text(&out).contains("PACKED"), "{target:?}: {}", text(&out));

    let after = w.snap();
    let fresh = fresh_extract(&after);
    assert_eq!(fresh[&rel(&w, &xml_file)], after[&rel(&w, &xml_file)], "{target:?}: re-extracted Form.xml differs");
    let stream_after = String::from_utf8_lossy(&after[&rel(&w, &stream_file)]);
    let expected_tbl_flags = 117644301i64 | 16 | 16384;
    let expected_col_flags = (12590592i64 | 1) & !2048 & !4096 & !4194304 & !8388608;
    assert!(stream_after.contains(&format!("{{19,{expected_tbl_flags},")), "{target:?}: Table bitmask missing");
    assert!(stream_after.contains(&format!(",{expected_col_flags},")), "{target:?}: Column bitmask missing");
}

#[test]
fn epf_table_and_column_catalog_properties_pack() {
    assert_table_and_column_catalog_properties_pack(Target::Epf);
}

#[test]
fn config_table_and_column_catalog_properties_pack() {
    assert_table_and_column_catalog_properties_pack(Target::Config);
}

/// A handler button added to the first command bar of the form: `pack` writes it, warns while the procedure is
/// missing from `FormModule.bsl`, stays silent once the module declares it, and the extracted XML shows the button.
fn assert_added_button_packs_with_a_warning(target: Target) {
    let w = Work::new(target);
    let xml_file = w.src_file("Form.xml");
    let xml = fs::read_to_string(&xml_file).unwrap();
    let start = xml.find("<CommandBar name=").expect("a command bar");
    let end = start + xml[start..].find("</CommandBar>").unwrap();
    let panel = &xml[start..end];
    let button = "<Button name=\"НоваяКнопкаПанели\" kind=\"Action\" action=\"НоваяКнопкаПанелиНажатие\"><Caption><Item language=\"ru\">Новая</Item></Caption></Button>";
    let panel = match (panel.find("<Buttons>"), panel.rfind("</Buttons>")) {
        (Some(_), Some(close)) => format!("{}{button}\n{}", &panel[..close], &panel[close..]),
        _ => format!("{panel}<Buttons>{button}</Buttons>\n"),
    };
    let edited = format!("{}{panel}{}", &xml[..start], &xml[end..]);
    fs::write(&xml_file, &edited).unwrap();

    let out = run(w.root(), &["pack", ".", "--dry-run"]);
    let t = text(&out);
    assert!(out.status.success() && t.contains("WOULD PACK"), "{target:?}: {t}");
    assert!(t.contains("warning") && t.contains("НоваяКнопкаПанелиНажатие"), "{target:?}: the missing procedure is reported: {t}");
    assert!(!t.contains("FAILED"), "{target:?}: {t}");

    // the module declares the procedure: no warning
    let bsl = w.src_file("FormModule.bsl");
    let mut module = fs::read(&bsl).unwrap();
    module.extend_from_slice("\nПроцедура НоваяКнопкаПанелиНажатие(Кнопка)\nКонецПроцедуры\n".as_bytes());
    fs::write(&bsl, module).unwrap();
    let out = run(w.root(), &["pack", ".", "--dry-run"]);
    let t = text(&out);
    assert!(out.status.success() && t.contains("WOULD PACK") && !t.contains("warning"), "{target:?}: {t}");

    let out = run(w.root(), &["pack", "."]);
    assert!(out.status.success() && text(&out).contains("PACKED"), "{target:?}: {}", text(&out));
    let again = run(w.root(), &["pack", ".", "--dry-run"]);
    assert!(text(&again).contains("NO CHANGES"), "{target:?}: {}", text(&again));
    let after = w.snap();
    let fresh = fresh_extract(&after);
    let extracted = String::from_utf8_lossy(&fresh[&rel(&w, &xml_file)]).into_owned();
    assert!(extracted.contains("name=\"НоваяКнопкаПанели\""), "{target:?}: the new button is extracted");
    assert!(extracted.contains("action=\"НоваяКнопкаПанелиНажатие\""), "{target:?}");
}

#[test]
fn epf_added_button_packs_and_reports_a_missing_procedure() {
    assert_added_button_packs_with_a_warning(Target::Epf);
}

#[test]
fn config_added_button_packs_and_reports_a_missing_procedure() {
    assert_added_button_packs_with_a_warning(Target::Config);
}

/// A form made by the Configurator for an external report (settings table `Отбор`, report form extension): new
/// controls may be written anywhere among their siblings, `pack` reads the packed form back; 1C keeps the control list
/// in creation order, so the new controls decode after the existing ones.
#[test]
fn report_form_accepts_new_controls_in_front_of_existing_ones() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_dir(&manifest.join("tests/fixtures/patch/ReportSettingsForm/dump"), dir.path());
    let out = run(dir.path(), &["extract", "."]);
    assert!(out.status.success(), "extract failed: {}", text(&out));
    let src = dir.path().join("ВнешнийОтчет1/src/Forms/ФормаОтчета");
    let xml_path = src.join("Form.xml");
    let xml = fs::read_to_string(&xml_path).unwrap();
    let label = "    <LabelDecoration name=\"НадписьАгента\" id=\"40\">\n      <Caption><Item language=\"ru\">Добавлено агентом</Item></Caption>\n      <Left>366</Left><Top>100</Top><Width>140</Width><Height>19</Height>\n      <ZOrder>30</ZOrder><TabOrder>30</TabOrder>\n    </LabelDecoration>\n";
    let button = "    <Button name=\"Сформировать\" id=\"41\">\n      <Caption><Item language=\"ru\">Сформировать</Item></Caption>\n      <Left>366</Left><Top>130</Top><Width>87</Width><Height>23</Height>\n      <ZOrder>31</ZOrder><TabOrder>31</TabOrder>\n      <Events><Click id=\"0\">СформироватьНажатие</Click></Events>\n    </Button>\n";
    let table = "    <Table name=\"Отбор\" id=\"7\">";
    assert_eq!(xml.matches(table).count(), 1);
    fs::write(&xml_path, xml.replacen(table, &format!("{label}{button}{table}"), 1)).unwrap();
    let bsl_path = src.join("FormModule.bsl");
    let mut bsl = fs::read_to_string(&bsl_path).unwrap();
    bsl.push_str("\nПроцедура СформироватьНажатие(Элемент)\n\tСообщить(\"Сформировать\");\nКонецПроцедуры\n");
    fs::write(&bsl_path, &bsl).unwrap();

    let out = run(dir.path(), &["pack", "."]);
    assert!(out.status.success(), "pack failed: {}", text(&out));
    let out = run(dir.path(), &["extract", ".", "--replace-generated"]);
    assert!(out.status.success(), "extract failed: {}", text(&out));
    let got = fs::read_to_string(&xml_path).unwrap();
    let names: Vec<&str> = got[got.find("<ChildItems>").unwrap()..]
        .lines()
        .filter(|l| l.starts_with("    <") && !l.starts_with("    </") && l.contains(" name=\""))
        .map(|l| l.split('"').nth(1).unwrap())
        .collect();
    assert_eq!(
        names,
        ["Результат", "ДействияФормы", "ПолеВвода1", "Надпись1", "Кнопка1", "Отбор", "НадписьАгента", "Сформировать"]
    );
    assert!(got.contains("<Click id=\"0\">СформироватьНажатие</Click>"), "{got}");
    assert_eq!(fs::read_to_string(&bsl_path).unwrap(), bsl);
}
