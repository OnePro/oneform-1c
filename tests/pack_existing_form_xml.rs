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

    // Patch Панель1: ShowTabs code=1 (Top)
    let pnl_at = xml.find("<Panel name=\"Панель1\"").expect("panel");
    let pnl_end = pnl_at + xml[pnl_at..].find("<ChildItems>").unwrap();
    let pnl_block = xml[pnl_at..pnl_end].replacen(
        "<ShowTabs type=\"ShowTabs\" member=\"Unknown\" code=\"2\"/>",
        "<ShowTabs type=\"ShowTabs\" member=\"Top\" code=\"1\"/>",
        1,
    );
    xml.replace_range(pnl_at..pnl_end, &pnl_block);

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
        "<ShowTabs type=\"ShowTabs\" member=\"Top\" code=\"1\"/>",
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
        "    <Attribute id=\"18\" name=\"НовыйСтроковыйРеквизит\">\n      <Main>true</Main>\n      <ValueType kind=\"String\" composite=\"false\"/>\n    </Attribute>\n  </Attributes>",
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
