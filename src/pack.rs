//! Repacking of edited `src/` files into the EPF (the inverse of extraction; port of `pack.py`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::Value;

use crate::brace::parse_spanned;
use crate::canonical::sha256_hex;
use crate::config::{effective_config, load_json_bytes};
use crate::container::{deflate, inflate, parse_container, serialize_container, EntryOut, Limits};
use crate::encoding::decode_payload;
use crate::errors::{
    ExtractionError, Result, ENCODING_INVALID, FORM_EDIT_REJECTED, GENERATED_FILE_MODIFIED, IO_ERROR, OUTPUT_NOT_OWNED, PAYLOAD_MISSING,
    PUBLISH_VERIFY_FAILED, SOURCE_CHANGED,
};
use crate::form_patch::{diff_forms_full, parse_form_xml, FormDiff, plan_form_patch, verify_preserved, FormXml, PatchedForm};
use crate::manifest::MANIFEST_PATH;
use crate::metadata::discover;
use crate::object_schema::with_object_schema;
use crate::stage::{stage_processor, ExtractionResult, StageOpts};
use crate::transaction::{
    manifest_hashes, publish, verify_source_unchanged, work_dir, PublishResult, Snapshot, Tree,
};

pub const PACK_UNCHANGED: &str = "UNCHANGED";
pub const PACK_PACKED: &str = "PACKED";
pub const PACK_SRC_ONLY: &str = "SRC_REFRESHED";
const UTF8_BOM: [u8; 3] = [0xef, 0xbb, 0xbf];
const BASE64_MARK: &[u8] = b"{#base64:";
pub const READ_ONLY_ROLES: [&str; 2] = ["form-definition", "template-definition"];

type PatchHook = Box<dyn FnMut(&mut PatchedForm)>;

thread_local! {
    static PATCH_HOOK: RefCell<Option<PatchHook>> = const { RefCell::new(None) };
}

/// Test hook: may alter the planned form patch of the current thread before it is staged, to simulate a planner
/// defect that the staged verification must catch (`None` removes it).
pub fn set_patch_hook(hook: Option<PatchHook>) {
    PATCH_HOOK.with(|h| *h.borrow_mut() = hook);
}

/// A supported `Form.xml` edit of one form, planned against its original `Reference/FormStream.bin`.
#[derive(Debug, Clone)]
pub struct LayoutPatch {
    /// Edited `Form.xml` (src-relative); the only file reported as packed for this edit.
    pub xml_path: String,
    /// `Reference/FormStream.bin` (src-relative) that receives the patched stream.
    pub stream_path: String,
    /// Uncompressed stream as extracted from the target.
    pub original: Vec<u8>,
    pub patched: PatchedForm,
    /// The user's edited `Form.xml`, parsed.
    pub requested: FormXml,
    /// The supported edits of `requested` against `baseline`, as the plan found them (with the object schema of the form
    /// in scope; a diff made later would not resolve the tables bound to a tabular section).
    pub diff: FormDiff,
    /// `Form.xml` of the unmodified target, parsed: controls the edit does not touch must decode as they did.
    pub baseline: FormXml,
    /// Non-fatal findings of the edit (a handler button whose procedure the form module does not declare).
    pub warnings: Vec<String>,
}

/// Plan the edit of one form's `Form.xml` (`layout` is the form's manifest `layout` record). `existing` is the
/// edited `src`, `current` the extraction of the unmodified target. `None` means "no layout edit": the XML is
/// unchanged or differs only in formatting. Unsupported differences, simultaneous independent `FormStream.bin`
/// and XML layout edits and failed patch verification are errors; nothing is written.
pub fn plan_layout_patch(layout: &Value, existing: &Tree, current: &Tree) -> Result<Option<LayoutPatch>> {
    let Some(xml_path) = layout["xml"].as_str() else { return Ok(None) };
    let stream_path = layout["binary"]["path"].as_str().unwrap_or("");
    let (Some(edited), Some(baseline)) = (existing.get(xml_path), current.get(xml_path)) else { return Ok(None) };
    if edited == baseline {
        return Ok(None);
    }
    let original = current.get(stream_path).ok_or_else(|| {
        ExtractionError::new(PAYLOAD_MISSING, "form stream of the edited Form.xml is missing").with("path", stream_path)
    })?;
    let source = parse_spanned(original).map_err(|e| e.with("path", stream_path))?;
    let plan = plan_form_patch(&source, baseline, edited).map_err(|e| e.with("path", xml_path))?;
    if plan.is_empty() {
        return Ok(None);
    }
    if existing.get(stream_path) != Some(original) {
        return Err(ExtractionError::new(
            FORM_EDIT_REJECTED,
            format!(
                "{xml_path} layout edits and independent {stream_path} edits cannot be combined: it is ambiguous which one wins; edit only one of them"
            ),
        )
        .with("kind", "ambiguous-layout-edit")
        .with("paths", Value::Array(vec![Value::from(xml_path), Value::from(stream_path)])));
    }
    let mut warnings = handler_warnings(&plan, existing, xml_path);
    warnings.extend(layout_warnings(&plan, existing, xml_path));
    let (requested, diff) = (plan.edited.clone(), plan.diff.clone());
    let mut patched = plan.apply().map_err(|e| e.with("path", xml_path))?;
    PATCH_HOOK.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(&mut patched);
        }
    });
    let baseline_xml = parse_form_xml(baseline, "baseline").map_err(|e| e.with("path", xml_path))?;
    Ok(Some(LayoutPatch {
        xml_path: xml_path.to_string(),
        stream_path: stream_path.to_string(),
        original: original.clone(),
        patched,
        requested,
        diff,
        baseline: baseline_xml,
        warnings,
    }))
}

/// A new or renamed command bar handler whose procedure `FormModule.bsl` (next to `Form.xml`) does not declare.
/// The handler is linked by name only, so a missing procedure is a warning, never an error; without the module
/// nothing is checked.
fn handler_warnings(plan: &crate::form_patch::FormPatchPlan, existing: &Tree, xml_path: &str) -> Vec<String> {
    let module_path = match xml_path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/FormModule.bsl"),
        None => "FormModule.bsl".to_string(),
    };
    let Some(bytes) = existing.get(&module_path) else { return Vec::new() };
    let text = String::from_utf8_lossy(bytes.strip_prefix(&UTF8_BOM[..]).unwrap_or(bytes));
    let mut out = Vec::new();
    for edit in &plan.diff.edits {
        for (button, proc) in crate::form_patch::new_handlers(edit) {
            if !crate::form_patch::module_declares(&text, &proc) {
                out.push(format!(
                    "  warning {xml_path}: button '{button}' of '{}' calls the procedure '{proc}', which {module_path} does not declare",
                    edit.name
                ));
            }
        }
    }
    for added in plan.diff.additions.iter().filter(|c| c.type_name == "CommandBar") {
        for (button, proc) in crate::form_patch::added_handlers(&added.values.buttons) {
            if !crate::form_patch::module_declares(&text, &proc) {
                out.push(format!(
                    "  warning {xml_path}: button '{button}' of '{}' calls the procedure '{proc}', which {module_path} does not declare",
                    added.name
                ));
            }
        }
    }
    out
}

/// Advisory findings of the edit: references to removed or renamed controls in `FormModule.bsl`, added fields that keep
/// the donor value type, added or moved controls beyond their container.
fn layout_warnings(plan: &crate::form_patch::FormPatchPlan, existing: &Tree, xml_path: &str) -> Vec<String> {
    let module_path = match xml_path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/FormModule.bsl"),
        None => "FormModule.bsl".to_string(),
    };
    let mut out = Vec::new();
    if let Some(bytes) = existing.get(&module_path) {
        let text = String::from_utf8_lossy(bytes.strip_prefix(&UTF8_BOM[..]).unwrap_or(bytes));
        out.extend(crate::layout_warnings::module_warnings(&plan.diff, &plan.edited, "FormModule.bsl", &text, true));
    }
    out.extend(crate::form_patch::unresolved_type_warnings(&plan.diff));
    out.extend(crate::layout_warnings::tabs_warnings(&plan.diff, &plan.edited));
    if let Some(bytes) = existing.get(xml_path) {
        let text = String::from_utf8_lossy(bytes.strip_prefix(&UTF8_BOM[..]).unwrap_or(bytes));
        out.extend(crate::layout_warnings::container_warnings(&plan.diff, &text));
    }
    out.into_iter().map(|w| format!("  warning {xml_path}: {w}")).collect()
}

impl LayoutPatch {
    /// Warn about the references to removed or renamed controls in a module of the object (`label` names it); the
    /// handlers of a removed control are looked for in the form module only.
    pub fn add_module_warnings(&mut self, label: &str, module: &[u8]) {
        let text = String::from_utf8_lossy(module.strip_prefix(&UTF8_BOM[..]).unwrap_or(module));
        for w in crate::layout_warnings::module_warnings(&self.diff, &self.requested, label, &text, false) {
            self.warnings.push(format!("  warning {}: {w}", self.xml_path));
        }
    }

    /// What the edit does, one line per control and one for each form-level change.
    pub fn summary(&self) -> Vec<String> {
        let mut out = Vec::new();
        let d = &self.diff;
        let at = &self.xml_path;
        for e in &d.edits {
            let props: Vec<&str> = e.changed.iter().map(|p| p.xml_name()).collect();
            out.push(format!("  edit {at}: {} '{}' (id {}): {}", e.type_name, e.name, e.id, props.join(", ")));
        }
        for a in &d.additions {
            out.push(format!("  edit {at}: add {} '{}'", a.type_name, a.name));
        }
        for x in &d.deletions {
            out.push(format!("  edit {at}: remove {} '{}' (id {})", x.type_name, x.name, x.id));
        }
        for (changed, what) in [
            (d.form_caption_changed, "form caption"),
            (d.form_size_changed, "form size"),
            (d.form_events_changed, "form events"),
            (d.attributes_changed, "form attributes"),
            (d.form_props_changed, "form window properties"),
        ] {
            if changed {
                out.push(format!("  edit {at}: {what}"));
            }
        }
        out.extend(self.warnings.iter().cloned());
        out
    }

    /// Verify the normally staged result before anything is published: the staged stream is the original with
    /// exactly the authorized replacements, and the `Form.xml` the normal decoder produces from it holds every
    /// requested edit (with the derived panel pages and decoder control order) and nothing else.
    pub fn verify_staged(&self, staged: &Tree) -> Result<()> {
        let fail = |message: &str| {
            ExtractionError::new(PUBLISH_VERIFY_FAILED, message.to_string()).with("path", self.xml_path.as_str())
        };
        let stream = staged.get(&self.stream_path).ok_or_else(|| fail("staged form stream is missing"))?;
        verify_preserved(&self.original, stream, &self.patched.replacements).map_err(|e| {
            fail("staged form stream differs from the original outside the authorized ranges")
                .with("kind", "unauthorized-stream-change")
                .with("error", e.message)
        })?;
        let xml = staged.get(&self.xml_path).ok_or_else(|| fail("staged Form.xml is missing"))?;
        let mut decoded =
            parse_form_xml(xml, "packed").map_err(|e| fail("staged Form.xml cannot be read").with("error", e.message))?;
        let mut expected = self
            .requested
            .with_allocated_ids(&self.patched.allocated)
            .with_additions_last(&self.patched.allocated.iter().map(|(_, id)| *id).collect::<Vec<_>>())
            .with_button_lists(&self.patched.button_lists)
            .with_derived_pages(&self.patched.derived_pages)
            .in_decoder_order();
        // A control the edit does not touch must decode exactly as it did before the edit: take it from the
        // decoded baseline instead of predicting the decoder's output for it.
        // A touched control decodes as its baseline with the requested properties applied.
        let mut removed: std::collections::HashSet<i64> = std::collections::HashSet::new();
        {
            let d = &self.diff;
            removed.extend(d.deletions.iter().map(|c| c.id));
            let touched: std::collections::HashMap<i64, &Vec<crate::form_patch::Property>> =
                d.edits.iter().map(|e| (e.id, &e.changed)).collect();
            let parents: std::collections::HashSet<i64> =
                self.baseline.controls.iter().chain(self.requested.controls.iter()).filter_map(|c| c.parent).collect();
            for c in &mut expected.controls {
                let Some(b) = self.baseline.control(c.id) else { continue };
                match touched.get(&c.id) {
                    None => {
                        c.values = b.values.clone();
                        // a container's `rest` holds stubs of its children: those may be added or removed
                        if !parents.contains(&c.id) {
                            c.rest = b.rest.clone();
                        }
                    }
                    Some(changed) => {
                        let mut values = b.values.clone();
                        for p in changed.iter() {
                            values.copy_property(&c.values, *p);
                        }
                        c.values = values;
                    }
                }
            }
        }
        // The columns of a table bound to a tabular section that the edit adds or rebinds decode with the
        // `<ValueType>` of their field, which the requested stub does not carry: those stubs are not compared
        // (the stream is read back column by column when it is patched).
        {
            let d = &self.diff;
            for e in d.edits.iter().filter(|e| e.changed.contains(&crate::form_patch::Property::Columns)) {
                let Some(skip) = crate::form_patch::changed_column_ids(&e.old.columns, &e.new.columns) else { continue };
                if !expected.control(e.id).is_some_and(crate::form_patch::is_section_table) {
                    continue;
                }
                for list in [&mut expected.controls, &mut decoded.controls] {
                    if let Some(c) = list.iter_mut().find(|c| c.id == e.id) {
                        c.rest = crate::form_patch::neutral_column_stubs(&c.rest, &c.values.columns, &skip);
                    }
                }
            }
        }
        // The edges of surviving controls that were attached to a removed control are re-anchored by the patch
        // (they keep their absolute position, the rectangle is still compared): they decode differently from the
        // baseline by design. Such an edge must not point to a removed control any more; its new value is not
        // compared here (the anchor graph of the stream is verified when the stream is patched).
        if !removed.is_empty() {
            for c in &mut expected.controls {
                let Some(a) = c.values.anchors.as_mut() else { continue };
                let Some(got) = decoded.control(c.id).and_then(|d| d.values.anchors.as_ref()) else { continue };
                let points = |e: &Option<crate::form_patch::AnchorEdge>| e.is_some_and(|e| removed.contains(&e.target));
                if let Some(bad) = [got.top, got.bottom, got.left, got.right].iter().chain(got.alt.iter()).find(|e| points(e)) {
                    return Err(fail("a surviving control still points to a removed control")
                        .with("kind", "xml-mismatch")
                        .with("id", c.id)
                        .with("target", bad.map_or(0, |e| e.target)));
                }
                for e in [&mut a.top, &mut a.bottom, &mut a.left, &mut a.right].into_iter().chain(a.alt.iter_mut()) {
                    if points(e) {
                        *e = None;
                    }
                }
            }
        }
        decoded.mask_anchors_like(&expected);
        // Added controls carry a sparse user-written projection; their identity and values are compared,
        // the full decoded rest is not.
        let skip_rest: std::collections::HashSet<i64> = self.patched.allocated.iter().map(|(_, new_id)| *new_id).collect();
        match diff_forms_full(&expected, &decoded, true, &skip_rest) {
            Ok(d) if d.is_empty() => Ok(()),
            Ok(d) => {
                let mut text = describe_differences(&d);
                if text.len() > 4000 {
                    // cut on a character boundary: the text holds Cyrillic names
                    let cut = (0..=4000).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
                    text.truncate(cut);
                }
                Err(fail("packed form does not reproduce the requested Form.xml").with("kind", "xml-mismatch").with("differences", text))
            }
            Err(e) => Err(fail("packed form does not reproduce the requested Form.xml")
                .with("kind", "xml-mismatch")
                .with("error", e.message)
                .with("problems", e.params.get("problems").cloned().unwrap_or(Value::Null))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackPlan {
    /// `PACK_UNCHANGED` | `PACK_PACKED` | `PACK_SRC_ONLY`
    pub status: &'static str,
    pub packed_paths: Vec<String>,
    /// One line per detected `Form.xml` edit (control, properties), for the pack report.
    pub edit_notes: Vec<String>,
    pub new_epf: Vec<u8>,
    /// Extraction of `new_epf` (the new src tree).
    pub result: ExtractionResult,
    pub existing: Tree,
    /// Extraction of the unmodified EPF (the projection before the edit); empty unless `PACK_PACKED`.
    pub baseline: Tree,
}

fn encode_with_codec(text: &str, codec: &str, path: &str) -> Result<Vec<u8>> {
    match codec {
        "utf-8" => Ok(text.as_bytes().to_vec()),
        "utf-16-le" => Ok(text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()),
        "utf-16-be" => Ok(text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()),
        "cp1251" => {
            let (bytes, _, unmappable) = encoding_rs::WINDOWS_1251.encode(text);
            if unmappable {
                Err(ExtractionError::new(ENCODING_INVALID, format!("text cannot be encoded as {codec}")).with("path", path))
            } else {
                Ok(bytes.into_owned())
            }
        }
        _ => Err(ExtractionError::new(ENCODING_INVALID, format!("text cannot be encoded as {codec}")).with("path", path)),
    }
}

fn unhex(h: &str) -> Vec<u8> {
    (0..h.len() / 2).filter_map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()).collect()
}

/// Return `(payload bytes, canonical text)` for an edited text file; `None` if logically unchanged.
pub fn encode_text(path: &str, src_bytes: &[u8], record: &Value) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
    let decoded = decode_payload(src_bytes, path, "utf-8")?; // strips an editor BOM, validates content
    if Some(decoded.output_sha256.as_str()) == record.get("outputSha256").and_then(Value::as_str) {
        return Ok(None);
    }
    let enc = record.get("encoding").unwrap_or(&Value::Null);
    let codec = enc.get("codec").and_then(Value::as_str).unwrap_or("utf-8");
    let mut bom = unhex(enc.get("bomHex").and_then(Value::as_str).unwrap_or(""));
    if bom.is_empty() && codec == "utf-8" && !decoded.text.is_empty() {
        // a BOM keeps non-ASCII text unambiguous on re-extraction; 1C writes it for non-empty modules
        if record.get("state").and_then(Value::as_str) == Some("empty") || !decoded.text.is_ascii() {
            bom = UTF8_BOM.to_vec();
        }
    }
    let kind = record.get("newlines").and_then(|n| n.get("kind")).and_then(Value::as_str);
    let sep = match kind {
        Some("LF") => "\n",
        Some("CR") => "\r",
        _ => "\r\n",
    };
    let mut payload = bom;
    payload.extend(encode_with_codec(&decoded.text.replace('\n', sep), codec, path)?);
    Ok(Some((payload, decoded.canonical)))
}

/// Put `html` into the `{#base64:...}` blob of the original template wrapper.
fn wrap_html(original: &[u8], html: &[u8]) -> Result<Vec<u8>> {
    let start = original
        .windows(BASE64_MARK.len())
        .position(|w| w == BASE64_MARK)
        .ok_or_else(|| ExtractionError::new(PAYLOAD_MISSING, "template wrapper has no base64 marker"))?
        + BASE64_MARK.len();
    let end = original[start..]
        .iter()
        .position(|&b| b == b'}')
        .map(|p| start + p)
        .ok_or_else(|| ExtractionError::new(PAYLOAD_MISSING, "unterminated base64 marker"))?;
    let raw_b64 = B64.encode(html).into_bytes();
    let b64: Vec<u8> = if original[start..end].contains(&b'\n') {
        raw_b64.chunks(64).collect::<Vec<_>>().join(&b"\r\n"[..])
    } else {
        raw_b64
    };
    let mut out = Vec::with_capacity(start + b64.len() + original.len() - end);
    out.extend_from_slice(&original[..start]);
    out.extend_from_slice(&b64);
    out.extend_from_slice(&original[end..]);
    Ok(out)
}

fn by_id(records: &Value) -> BTreeMap<String, Value> {
    records
        .as_array()
        .map(|a| a.iter().filter_map(|r| r.get("id").and_then(Value::as_str).map(|id| (id.to_string(), r.clone()))).collect())
        .unwrap_or_default()
}

/// Pure in-memory: pack edited `existing_src` files into `epf_bytes`.
pub fn stage_pack(
    epf_bytes: &[u8],
    epf_name: &str,
    existing_src: &Tree,
    processor_config: &Value,
    root_config: &Value,
    tools_lock_bytes: &[u8],
    opts: &StageOpts<'_>,
) -> Result<PackPlan> {
    if existing_src.is_empty() {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src is missing or empty; run extract first"));
    }
    let owned = manifest_hashes(existing_src)?;
    let unknown: Vec<Value> = existing_src
        .keys()
        .filter(|p| !owned.contains_key(*p) && p.as_str() != MANIFEST_PATH)
        .map(|p| Value::from(p.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src contains files not listed in the manifest")
            .with("paths", Value::Array(unknown)));
    }
    let missing: Vec<Value> = owned
        .keys()
        .filter(|p| !existing_src.contains_key(*p))
        .map(|p| Value::from(p.as_str()))
        .collect();
    if !missing.is_empty() {
        return Err(ExtractionError::new(PAYLOAD_MISSING, "expected src file is missing").with("paths", Value::Array(missing)));
    }
    let old_manifest = load_json_bytes(&existing_src[MANIFEST_PATH], MANIFEST_PATH)?;

    let cfg = effective_config(root_config, processor_config, opts.cli_overrides)?;
    let limits = Limits::from_json(&Value::Object(cfg.limits.clone()));
    let mut effective_opts = opts.clone();
    let backend_owned: Option<String> = if opts.form_backend.is_none() {
        old_manifest
            .get("tools")
            .and_then(|t| t.get("formBackend"))
            .and_then(Value::as_str)
            .filter(|b| matches!(*b, "basic" | "oof"))
            .map(str::to_string)
    } else {
        None
    };
    if let Some(ref b) = backend_owned {
        effective_opts.form_backend = Some(b.as_str());
    }
    let current = stage_processor(epf_bytes, epf_name, processor_config, root_config, tools_lock_bytes, &effective_opts)?;
    let outer = parse_container(epf_bytes, &limits, 0, None)?;
    let inv = discover(&outer, &limits, None, false)?;
    let source = old_manifest.get("source").filter(|s| s.is_object());
    let proc_id = source.and_then(|s| s.get("processorId")).cloned().unwrap_or(Value::Null);
    if proc_id.as_str() != Some(inv.processor.object_uuid.as_str()) {
        return Err(ExtractionError::new(OUTPUT_NOT_OWNED, "src manifest belongs to a different processor")
            .with("expected", inv.processor.object_uuid)
            .with("found", proc_id));
    }
    if source.and_then(|s| s.get("sha256")).and_then(Value::as_str) != Some(sha256_hex(epf_bytes).as_str()) {
        return Err(ExtractionError::new(SOURCE_CHANGED, "EPF changed since src was extracted; run extract before pack")
            .with("epf", epf_name));
    }

    let cur = load_json_bytes(&current.files[MANIFEST_PATH], MANIFEST_PATH)?;
    let mut readonly: Vec<String> = cur["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter_map(|f| {
                    let role = f["role"].as_str()?;
                    let path = f["path"].as_str()?;
                    if READ_ONLY_ROLES.contains(&role) && existing_src.get(path) != current.files.get(path) {
                        Some(path.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    readonly.sort();
    if !readonly.is_empty() {
        let paths: Vec<Value> = readonly.into_iter().map(Value::from).collect();
        return Err(ExtractionError::new(
            GENERATED_FILE_MODIFIED,
            "definition files are read-only projections and cannot be packed; if you did not edit them, they come from another oneform version: run `oneform extract` again",
        )
        .with("paths", Value::Array(paths)));
    }
    let mut top: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut inner: BTreeMap<String, BTreeMap<String, Vec<u8>>> = BTreeMap::new();
    let mut expected: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut packed: BTreeSet<String> = BTreeSet::new();
    let mut layouts: Vec<LayoutPatch> = Vec::new();

    fn text_change(
        path: &str,
        record: &Value,
        existing_src: &Tree,
        current: &Tree,
        packed: &mut BTreeSet<String>,
        expected: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<Option<Vec<u8>>> {
        if existing_src.get(path) == current.get(path) {
            return Ok(None);
        }
        let Some((payload, canonical)) = encode_text(path, &existing_src[path], record)? else {
            return Ok(None);
        };
        packed.insert(path.to_string());
        expected.insert(path.to_string(), canonical);
        Ok(Some(payload))
    }

    fn binary_change(
        path: &str,
        existing_src: &Tree,
        current: &Tree,
        packed: &mut BTreeSet<String>,
        expected: &mut BTreeMap<String, Vec<u8>>,
    ) -> Option<Vec<u8>> {
        if existing_src.get(path) == current.get(path) {
            return None;
        }
        let data = existing_src[path].clone();
        packed.insert(path.to_string());
        expected.insert(path.to_string(), data.clone());
        Some(data)
    }

    let om = &cur["objectModule"];
    let om_path = om["path"].as_str().unwrap_or("ObjectModule.bsl");
    if let Some(payload) = text_change(om_path, om, existing_src, &current.files, &mut packed, &mut expected)? {
        inner.entry(format!("{}.0", inv.processor.object_uuid)).or_default().insert("text".into(), payload);
    }

    let mut schema = inv.object_schema.clone();
    schema.resolve_ref_kinds(|| config_lookup(opts.epf_dir));
    let forms = by_id(&cur["forms"]);
    for f in &inv.forms {
        let rec = &forms[&f.uuid];
        let mod_path = rec["module"]["path"].as_str().unwrap_or("");
        if let Some(payload) = text_change(mod_path, &rec["module"], existing_src, &current.files, &mut packed, &mut expected)? {
            crate::bsl_order::check_module_edit(mod_path, current.files.get(mod_path).map(Vec::as_slice), &expected[mod_path])?;
            inner.entry(format!("{}.0", f.uuid)).or_default().insert("module".into(), payload);
        }
        let bin_path = rec["layout"]["binary"]["path"].as_str().unwrap_or("");
        let planned =
            with_object_schema(Some(schema.clone()), || plan_layout_patch(&rec["layout"], existing_src, &current.files))?;
        if let Some(mut lp) = planned {
            if let Some(module) = existing_src.get(om_path) {
                lp.add_module_warnings("ObjectModule.bsl", module);
            }
            inner.entry(format!("{}.0", f.uuid)).or_default().insert("form".into(), lp.patched.bytes.clone());
            expected.insert(bin_path.to_string(), lp.patched.bytes.clone());
            packed.insert(lp.xml_path.clone());
            layouts.push(lp);
        } else if let Some(payload) = binary_change(bin_path, existing_src, &current.files, &mut packed, &mut expected) {
            inner.entry(format!("{}.0", f.uuid)).or_default().insert("form".into(), payload);
        }
    }

    let templates = by_id(&cur["templates"]);
    for t in &inv.templates {
        let content = &templates[&t.uuid]["content"];
        let path = content["path"].as_str().unwrap_or("");
        if path.ends_with("/Template.html") {
            if let Some(html) = text_change(path, content, existing_src, &current.files, &mut packed, &mut expected)? {
                top.insert(format!("{}.0", t.uuid), wrap_html(&t.content_payload.raw, &html)?);
            }
        } else if let Some(payload) = binary_change(path, existing_src, &current.files, &mut packed, &mut expected) {
            top.insert(format!("{}.0", t.uuid), payload);
        }
    }

    if packed.is_empty() {
        let status = if existing_src == &current.files { PACK_UNCHANGED } else { PACK_SRC_ONLY };
        return Ok(PackPlan {
            status,
            packed_paths: vec![],
            edit_notes: Vec::new(),
            new_epf: epf_bytes.to_vec(),
            result: current,
            existing: existing_src.clone(),
            baseline: Tree::new(),
        });
    }

    let mut owned_raws: Vec<Vec<u8>> = Vec::with_capacity(outer.entries.len());
    for e in &outer.entries {
        if let Some(t) = top.get(&e.name) {
            owned_raws.push(deflate(t));
        } else if let Some(sub_changes) = inner.get(&e.name) {
            let sub = parse_container(&inflate(&e.raw, &limits, None)?, &limits, 1, None)?;
            let sub_out: Vec<EntryOut> = sub
                .entries
                .iter()
                .map(|s| (s.name.as_str(), sub_changes.get(&s.name).map_or(s.raw.as_slice(), Vec::as_slice), Some(s.header.as_slice())))
                .collect();
            owned_raws.push(deflate(&serialize_container(&sub_out, 0x200)?));
        } else {
            owned_raws.push(e.raw.clone());
        }
    }
    let out_entries: Vec<EntryOut> = outer
        .entries
        .iter()
        .zip(&owned_raws)
        .map(|(e, raw)| (e.name.as_str(), raw.as_slice(), Some(e.header.as_slice())))
        .collect();
    let new_epf = serialize_container(&out_entries, 0x200)?;

    let new = stage_processor(&new_epf, epf_name, processor_config, root_config, tools_lock_bytes, &effective_opts)?;
    let wrong: Vec<Value> = expected
        .iter()
        .filter(|(p, data)| new.files.get(*p) != Some(*data))
        .map(|(p, _)| Value::from(p.as_str()))
        .collect();
    if !wrong.is_empty() {
        return Err(ExtractionError::new(PUBLISH_VERIFY_FAILED, "packed EPF does not reproduce the edited files")
            .with("paths", Value::Array(wrong)));
    }
    for lp in &layouts {
        lp.verify_staged(&new.files)?;
    }
    Ok(PackPlan {
        status: PACK_PACKED,
        packed_paths: packed.into_iter().collect(),
        edit_notes: layouts.iter().flat_map(|lp| lp.summary()).collect(),
        new_epf,
        result: new,
        existing: existing_src.clone(),
        baseline: current.files,
    })
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp"));
    {
        let mut fh = File::create(&tmp)?;
        fh.write_all(data)?;
        fh.flush()?;
        fh.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn token_hex8() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

/// Publish a pack plan; call under the processor lock.
pub fn publish_pack(
    root: &Path,
    proc_rel: &str,
    plan: &PackPlan,
    epf_path: &Path,
    snap: &Snapshot,
    backup: bool,
    keep_work: bool,
) -> Result<PublishResult> {
    if plan.status == PACK_UNCHANGED {
        return Ok(PublishResult { status: "UNCHANGED", run_dir: None });
    }
    if plan.status == PACK_SRC_ONLY {
        return publish(root, proc_rel, &plan.result.files, epf_path, snap, keep_work, true);
    }

    let run_dir = work_dir(root).join(token_hex8());
    let outcome = (|| -> Result<()> {
        fs::create_dir_all(&run_dir)?;
        let new_epf_file = run_dir.join("NewEpf");
        {
            let mut fh = File::create(&new_epf_file)?;
            fh.write_all(&plan.new_epf)?;
            fh.flush()?;
            fh.sync_all()?;
        }
        verify_source_unchanged(epf_path, snap)?;
        if backup {
            let name = epf_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            atomic_write(&epf_path.with_file_name(format!("{name}.bak")), &snap.data)?;
        }
        fs::rename(&new_epf_file, epf_path)?;
        let new_snap = Snapshot {
            data: plan.new_epf.clone(),
            size: plan.new_epf.len(),
            sha256: sha256_hex(&plan.new_epf),
        };
        if let Err(e) = publish(root, proc_rel, &plan.result.files, epf_path, &new_snap, keep_work, true) {
            if let Err(re) = atomic_write(epf_path, &snap.data) {
                return Err(ExtractionError::new(IO_ERROR, "src publication failed and the EPF could not be restored")
                    .with("error", re.to_string()));
            }
            return Err(e);
        }
        Ok(())
    })();
    if run_dir.exists() && !keep_work {
        let _ = fs::remove_dir_all(&run_dir);
    }
    outcome?;
    Ok(PublishResult { status: "UPDATED", run_dir: if keep_work { Some(run_dir) } else { None } })
}

/// One line per control that the packed form does not reproduce: its properties and where the values part.
fn describe_differences(d: &crate::form_patch::FormDiff) -> String {
    let mut out = Vec::new();
    for e in &d.edits {
        let (a, b) = (format!("{:?}", e.old), format!("{:?}", e.new));
        let at = a.char_indices().zip(b.chars()).find(|((_, x), y)| x != y).map_or(0, |((i, _), _)| i);
        let window = |t: &str| {
            let from = (0..=at.saturating_sub(40)).rev().find(|&i| t.is_char_boundary(i)).unwrap_or(0);
            let to = (at + 60..=t.len()).find(|&i| t.is_char_boundary(i)).unwrap_or(t.len());
            t[from..to.min(t.len())].to_string()
        };
        out.push(format!(
            "control '{}' (id {}, {}): properties {:?} differ; requested ...{}... packed ...{}...",
            e.name,
            e.id,
            e.type_name,
            e.changed,
            window(&a),
            window(&b)
        ));
    }
    if out.is_empty() {
        format!("{d:?}")
    } else {
        out.join("; ")
    }
}

/// The configuration that names the types of reference columns of a processor: `ONEFORM_CONFIG_ROOT` (a
/// configuration export directory) or, failing that, the configuration around the `.epf`.
fn config_lookup(epf_dir: Option<&Path>) -> Option<std::sync::Arc<crate::metaref::ConfigTypeLookup>> {
    use crate::metaref::ConfigTypeLookup;
    let root = match std::env::var_os("ONEFORM_CONFIG_ROOT").filter(|v| !v.is_empty()) {
        Some(v) => Some(std::path::PathBuf::from(v)),
        None => epf_dir.and_then(ConfigTypeLookup::find_config_root),
    }?;
    Some(ConfigTypeLookup::for_config_root(&root))
}
