//! Packing edited `Form.xml` for both pack targets (EPF and configuration `Form.bin`): supported edits of
//! existing controls are patched into the form stream, unsupported ones are rejected with
//! `GENERATED_FILE_MODIFIED`, and every failure leaves XML, BSL and containers untouched. Unchanged and
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
        let width: u32 = xml[at..end].parse().unwrap();
        fs::write(&p, format!("{}{}{}", &xml[..at], width + 1, &xml[end..])).unwrap();
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
        msg.contains("GENERATED_FILE_MODIFIED"),
        "{target:?}: expected GENERATED_FILE_MODIFIED, got: {msg}"
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

use oneform::errors::{ExtractionError, GENERATED_FILE_MODIFIED, PUBLISH_VERIFY_FAILED};
use oneform::pack::set_patch_hook;
use oneform::transaction::{read_tree, set_rename_hook};

const LABEL: &str = "name=\"Надпись1\"";
const CAPTION: (&str, &str) = ("<Item language=\"ru\">Надпись1</Item>", "<Item language=\"ru\">Подпись</Item>");
const LEFT: (&str, &str) = ("<Left>79</Left>", "<Left>90</Left>");

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
    assert_eq!(after[&xml_path], edited[&xml_path], "{target:?}: Form.xml changed");
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
        fresh[&xml_path], edited[&xml_path],
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
            msg.contains("GENERATED_FILE_MODIFIED") && msg.contains("ambiguous"),
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
        ("name=\"Надпись1\" id=\"5\"", "name=\"Надпись2\" id=\"5\""),
        ("<Click id=\"0\">Надпись1Нажатие", "<Click id=\"0\">Другой"),
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
            assert!(msg.contains("GENERATED_FILE_MODIFIED"), "{target:?} {edit:?}: {msg}");
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
        assert_eq!(err.code, GENERATED_FILE_MODIFIED);
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
            String::from_utf8(fresh[&xml_path].clone()).unwrap(),
            format!("{head}{derived_block}{tail}"),
            "{target:?}: re-extracted panel and page geometry"
        );
    }
}
