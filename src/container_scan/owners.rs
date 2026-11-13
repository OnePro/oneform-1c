//! Owners of container records from XML dumps of a configuration (`container-diff --owners`).
//!
//! A record named `<uuid>[.<suffix>]` belongs to the top-level metadata object whose XML file has `<uuid>` as its first
//! `uuid="..."` attribute (a sub-object file such as a form belongs to the object whose directory holds it). The index is
//! built with plain byte scans of the first 64 KiB of every `.xml` file; no XML library, nothing is written.

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rayon::prelude::*;

use super::ScanWarning;

/// Bytes of an XML file that are looked at.
const HEAD: usize = 64 * 1024;

pub const WARN_DUPLICATE_UUID: &str = "DUPLICATE_UUID";
pub const WARN_OWNERS_UNREADABLE: &str = "OWNERS_UNREADABLE";
/// Owner of a record that is not a UUID (`root`, `version`, `versions`, ...).
pub const OWNER_SERVICE: &str = "service";
/// Owner of a record whose UUID is in no given dump.
pub const OWNER_UNMAPPED: &str = "unmapped";

/// `lowercase uuid -> owner`, first dump wins.
#[derive(Debug, Default)]
pub struct OwnerIndex {
    map: HashMap<String, String>,
    pub warnings: Vec<ScanWarning>,
}

/// 8-4-4-4-12 hexadecimal digits.
pub fn is_uuid(s: &[u8]) -> bool {
    s.len() == 36
        && s.iter().enumerate().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { *c == b'-' } else { c.is_ascii_hexdigit() })
}

fn read_head(path: &Path) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    File::open(path).ok()?.take(HEAD as u64).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// The first `uuid="<36 chars>"` attribute (lowercased).
fn first_uuid(bytes: &[u8]) -> Option<String> {
    const KEY: &[u8] = b"uuid=\"";
    let mut from = 0;
    while let Some(p) = bytes[from..].windows(KEY.len()).position(|w| w == KEY).map(|p| p + from) {
        let start = p + KEY.len();
        let preceded = p > 0 && bytes[p - 1].is_ascii_whitespace();
        if preceded && bytes.len() >= start + 37 && bytes[start + 36] == b'"' && is_uuid(&bytes[start..start + 36]) {
            return Some(String::from_utf8_lossy(&bytes[start..start + 36]).to_ascii_lowercase());
        }
        from = p + 1;
    }
    None
}

/// Index just after the `>` that closes the tag whose name ended at `i` (quoted attribute values may hold `>`).
fn tag_end(b: &[u8], mut i: usize) -> Option<usize> {
    let mut quote = 0u8;
    while i < b.len() {
        let c = b[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

/// Local name of the first child element of the root element, from the start tags only.
fn first_child_element(bytes: &[u8]) -> Option<String> {
    let b = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let mut i = 0;
    let mut depth = 0usize;
    while i < b.len() {
        let lt = b[i..].iter().position(|&c| c == b'<')? + i;
        let rest = &b[lt..];
        if rest.starts_with(b"<?") {
            i = lt + rest.windows(2).position(|w| w == b"?>")? + 2;
        } else if rest.starts_with(b"<!--") {
            i = lt + rest.windows(3).position(|w| w == b"-->")? + 3;
        } else if rest.starts_with(b"<!") {
            i = lt + rest.iter().position(|&c| c == b'>')? + 1;
        } else if rest.starts_with(b"</") {
            return None;
        } else {
            let name_end = rest[1..].iter().position(|&c| c.is_ascii_whitespace() || c == b'/' || c == b'>').map(|p| p + 1)?;
            let end = tag_end(b, lt + name_end)?;
            if depth == 1 {
                let name = std::str::from_utf8(&rest[1..name_end]).ok()?;
                return Some(name.rsplit(':').next().unwrap_or(name).to_string());
            }
            if b[end - 2] == b'/' {
                return None;
            }
            depth += 1;
            i = end;
        }
    }
    None
}

struct Found {
    /// Path components below the dump directory.
    comps: Vec<String>,
}

/// Every `*.xml` file below `dir` (no `Ext` directories, no links, no `ConfigDumpInfo.xml`), sorted by relative path.
fn collect(dir: &Path, rel: &mut Vec<String>, out: &mut Vec<Found>, warnings: &mut Vec<ScanWarning>) {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            warnings.push(ScanWarning {
                code: WARN_OWNERS_UNREADABLE.into(),
                message: format!("cannot read directory {}: {e}", dir.display()),
                entry: None,
            });
            return;
        }
    };
    let mut names: Vec<(String, PathBuf)> = Vec::new();
    for e in rd {
        match e {
            Ok(e) => names.push((e.file_name().to_string_lossy().into_owned(), e.path())),
            Err(e) => warnings.push(ScanWarning {
                code: WARN_OWNERS_UNREADABLE.into(),
                message: format!("cannot read directory {}: {e}", dir.display()),
                entry: None,
            }),
        }
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, path) in names {
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        let ft = meta.file_type();
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if name == "Ext" {
                continue;
            }
            rel.push(name);
            collect(&path, rel, out, warnings);
            rel.pop();
        } else if ft.is_file() && name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".xml") && name != "ConfigDumpInfo.xml" {
            let mut comps = rel.clone();
            comps.push(name);
            out.push(Found { comps });
        }
    }
}

fn stem(file: &str) -> &str {
    file.get(..file.len().saturating_sub(4)).unwrap_or(file)
}

/// `(type directory, object name)` of the file, or `None` when it does not count.
fn object_of(comps: &[String]) -> Option<(&str, &str)> {
    match comps.len() {
        0 => None,
        1 => (comps[0] == "Configuration.xml").then_some(("", "")),
        2 => Some((&comps[0], stem(&comps[1]))),
        _ => Some((&comps[0], &comps[1])),
    }
}

/// One dump: `uuid -> owner` plus its warnings.
fn index_dump(dir: &Path, workers: usize) -> (HashMap<String, String>, Vec<ScanWarning>) {
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    collect(dir, &mut Vec::new(), &mut files, &mut warnings);
    files.sort_by_cached_key(|f| f.comps.join("/"));
    let types: Mutex<HashMap<(String, String), String>> = Mutex::new(HashMap::new());
    let scan = |f: &Found| -> Option<(String, String)> {
        let (type_dir, name) = object_of(&f.comps)?;
        let head = read_head(&dir.join(f.comps.join("/")))?;
        let uuid = first_uuid(&head)?;
        if type_dir.is_empty() {
            return Some((uuid, "Configuration".into()));
        }
        // the type is the first child element of `<TypeDir>/<Name>.xml`: that is this very file at depth 2
        let ty = if f.comps.len() == 2 {
            first_child_element(&head)
        } else {
            let key = (type_dir.to_string(), name.to_string());
            let cached = types.lock().ok().and_then(|m| m.get(&key).cloned());
            match cached {
                Some(t) => Some(t),
                None => {
                    let t = read_head(&dir.join(type_dir).join(format!("{name}.xml"))).and_then(|b| first_child_element(&b));
                    if let (Some(t), Ok(mut m)) = (&t, types.lock()) {
                        m.insert(key, t.clone());
                    }
                    t
                }
            }
        };
        Some((uuid, format!("{}.{name}", ty.as_deref().unwrap_or(type_dir))))
    };
    let found: Vec<Option<(String, String)>> = match rayon::ThreadPoolBuilder::new().num_threads(workers.max(1)).build() {
        Ok(pool) => pool.install(|| files.par_iter().map(scan).collect()),
        Err(_) => files.iter().map(scan).collect(),
    };
    let mut map: HashMap<String, String> = HashMap::new();
    let mut first_file: HashMap<String, String> = HashMap::new();
    for (f, hit) in files.iter().zip(found) {
        let Some((uuid, owner)) = hit else { continue };
        let path = f.comps.join("/");
        if let Some(prev) = first_file.get(&uuid) {
            warnings.push(ScanWarning {
                code: WARN_DUPLICATE_UUID.into(),
                message: format!("uuid {uuid} is the first uuid of both {prev} and {path} in {}", dir.display()),
                entry: None,
            });
        } else {
            first_file.insert(uuid.clone(), path);
            map.insert(uuid, owner);
        }
    }
    (map, warnings)
}

impl OwnerIndex {
    /// Index the dumps in order; the first dump that has a UUID wins.
    pub fn build(dirs: &[PathBuf], workers: usize) -> OwnerIndex {
        let mut index = OwnerIndex::default();
        for d in dirs {
            let (map, warnings) = index_dump(d, workers);
            index.warnings.extend(warnings);
            for (k, v) in map {
                index.map.entry(k).or_insert(v);
            }
        }
        index
    }

    /// `<Type>.<Name>`, `service` (the name before the first dot is not a UUID) or `unmapped` (a UUID of no dump).
    pub fn owner_of(&self, record: &str) -> String {
        let base = record.split('.').next().unwrap_or(record);
        if !is_uuid(base.as_bytes()) {
            return OWNER_SERVICE.into();
        }
        self.map.get(&base.to_ascii_lowercase()).cloned().unwrap_or_else(|| OWNER_UNMAPPED.into())
    }
}

/// Owners named in an `--expect-owners` file: one per line, `#` comments, empty lines ignored, BOM and CRLF tolerated.
pub fn parse_expected(bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| "is not valid UTF-8".to_string())?;
    Ok(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect())
}
