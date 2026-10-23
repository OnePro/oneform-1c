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
    ExtractionError, Result, ENCODING_INVALID, GENERATED_FILE_MODIFIED, IO_ERROR, OUTPUT_NOT_OWNED, PAYLOAD_MISSING,
    PUBLISH_VERIFY_FAILED, SOURCE_CHANGED,
};
use crate::form_patch::{diff_forms, parse_form_xml, plan_form_patch, verify_preserved, FormXml, PatchedForm};
use crate::manifest::MANIFEST_PATH;
use crate::metadata::discover;
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
            GENERATED_FILE_MODIFIED,
            format!(
                "{xml_path} layout edits and independent {stream_path} edits cannot be combined: it is ambiguous which one wins; edit only one of them"
            ),
        )
        .with("kind", "ambiguous-layout-edit")
        .with("paths", Value::Array(vec![Value::from(xml_path), Value::from(stream_path)])));
    }
    let mut patched = plan.apply().map_err(|e| e.with("path", xml_path))?;
    PATCH_HOOK.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(&mut patched);
        }
    });
    let requested = parse_form_xml(edited, "edited").map_err(|e| e.with("path", xml_path))?;
    Ok(Some(LayoutPatch {
        xml_path: xml_path.to_string(),
        stream_path: stream_path.to_string(),
        original: original.clone(),
        patched,
        requested,
    }))
}

impl LayoutPatch {
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
        let decoded =
            parse_form_xml(xml, "packed").map_err(|e| fail("staged Form.xml cannot be read").with("error", e.message))?;
        let expected = self.requested.with_derived_pages(&self.patched.derived_pages).in_decoder_order();
        match diff_forms(&expected, &decoded) {
            Ok(d) if d.is_empty() => Ok(()),
            Ok(_) => Err(fail("packed form does not reproduce the requested Form.xml").with("kind", "xml-mismatch")),
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
    pub new_epf: Vec<u8>,
    /// Extraction of `new_epf` (the new src tree).
    pub result: ExtractionResult,
    pub existing: Tree,
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
            "definition files are read-only projections and cannot be packed",
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

    let forms = by_id(&cur["forms"]);
    for f in &inv.forms {
        let rec = &forms[&f.uuid];
        let mod_path = rec["module"]["path"].as_str().unwrap_or("");
        if let Some(payload) = text_change(mod_path, &rec["module"], existing_src, &current.files, &mut packed, &mut expected)? {
            inner.entry(format!("{}.0", f.uuid)).or_default().insert("module".into(), payload);
        }
        let bin_path = rec["layout"]["binary"]["path"].as_str().unwrap_or("");
        if let Some(lp) = plan_layout_patch(&rec["layout"], existing_src, &current.files)? {
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
            new_epf: epf_bytes.to_vec(),
            result: current,
            existing: existing_src.clone(),
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
        new_epf,
        result: new,
        existing: existing_src.clone(),
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
