//! `epf-extract props`: inspect the raw property indices of ordinary-form controls
//! (port of `tools/props.py`).
//!
//! `props <path> [--control Name]` prints the `info` (properties) and `geom` trees of the controls with
//! `[index]` labels. `props <path> --diff <other>` prints only the indices whose values differ between the
//! same-named controls of two form streams. `<path>` is a `FormStream.bin` (brace tree) or an
//! `Ext/Form.bin` container with a `form` entry.

use std::collections::HashMap;

use crate::brace::{parse_metadata, Atom, Item, Node, BASE64, QUOTED};
use crate::container::{looks_like_container, parse_container, Limits};
use crate::oof::{control_parts, table, ControlParts};
use crate::oof_props::{str_val, OofError};

pub const MISSING: &str = "<missing>";
const INLINE_MAX: usize = 16;
const BLOB_MAX: usize = 24;

const USAGE: &str = "usage: epf-extract props [-h] [--control CONTROL] [--diff OTHER] path\n";
const HELP: &str = "\n``epf-extract props``: inspect the raw property indices of ordinary-form\ncontrols.\n\npositional arguments:\n  path               FormStream.bin or Ext/Form.bin\n\noptions:\n  -h, --help         show this help message and exit\n  --control CONTROL  only the control with this name\n  --diff OTHER       print only the indices that differ from OTHER\n";

/// The form brace stream of `data` (a container's `form` entry or the stream itself).
pub fn form_stream(data: &[u8]) -> Result<Vec<u8>, OofError> {
    if looks_like_container(data) {
        let container = parse_container(data, &Limits::default(), 0, None)?;
        return match container.get("form") {
            Some(entry) => Ok(entry.raw.clone()),
            None => Err(OofError::new("container has no 'form' entry")),
        };
    }
    Ok(data.to_vec())
}

/// `(label, name, type, info, geom)` of one control.
pub struct PropsControl<'a> {
    pub label: String,
    pub name: String,
    pub type_name: String,
    pub info: &'a Node,
    pub geom: &'a Node,
}

fn node_at<'a>(n: &'a Node, i: usize, what: &str) -> Result<&'a Node, OofError> {
    match n.children.get(i) {
        Some(Item::Node(x)) => Ok(x),
        _ => Err(OofError(format!("{what}: unexpected structure"))),
    }
}

/// Every control in document order (labels are unique).
pub fn controls(root: &Node) -> Result<Vec<PropsControl<'_>>, OofError> {
    let section = node_at(root, 1, "form section")?;
    let panel = node_at(section, 2, "root panel")?;
    let mut out: Vec<PropsControl<'_>> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    fn walk<'a>(
        items: &'a [Item],
        out: &mut Vec<PropsControl<'a>>,
        seen: &mut HashMap<String, usize>,
    ) -> Result<(), OofError> {
        for c in items {
            let ControlParts { type_name, info, geom, meta, children, .. } = control_parts(c)?;
            let name = str_val(&meta.children[1], "control name")?.to_string();
            let count = seen.entry(name.clone()).or_insert(0);
            *count += 1;
            let label = if *count == 1 { name.clone() } else { format!("{name}#{count}") };
            out.push(PropsControl {
                label,
                name,
                type_name: type_name.unwrap_or("UnknownControl").to_string(),
                info,
                geom,
            });
            walk(table(Some(children), "control children")?, out, seen)?;
        }
        Ok(())
    }

    walk(table(panel.children.get(2), "root panel children")?, &mut out, &mut seen)?;
    Ok(out)
}

fn atom_text(a: &Atom) -> String {
    if a.kind == QUOTED {
        return format!("\"{}\"", a.value);
    }
    if a.kind == BASE64 {
        return if a.value.chars().count() > BLOB_MAX {
            format!("<base64 {} chars>", a.value.chars().count())
        } else {
            format!("<base64 {}>", a.value)
        };
    }
    a.value.clone()
}

/// `path -> value` of every leaf below `n` (`[2][0][7]` style paths; empty lists are leaves).
pub fn flatten(n: &Node, prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    flatten_node(n, prefix, &mut out);
    out
}

fn flatten_node(node: &Node, prefix: &str, out: &mut Vec<(String, String)>) {
    if node.is_empty() {
        out.push((prefix.to_string(), "{}".to_string()));
    }
    for (i, c) in node.children.iter().enumerate() {
        let path = format!("{prefix}[{i}]");
        match c {
            Item::Atom(a) => out.push((path, atom_text(a))),
            Item::Node(n) => flatten_node(n, &path, out),
        }
    }
}

fn render_node(node: &Node, label: &str, depth: usize, lines: &mut Vec<String>) {
    let pad = "  ".repeat(depth);
    if node.children.iter().all(|c| matches!(c, Item::Atom(_))) && node.len() <= INLINE_MAX {
        let parts: Vec<String> = node.children.iter().filter_map(Item::as_atom).map(atom_text).collect();
        lines.push(format!("{pad}{label} {{{}}}", parts.join(", ")));
        return;
    }
    lines.push(format!("{pad}{label} ({})", node.len()));
    for (i, c) in node.children.iter().enumerate() {
        let label = format!("[{i}]");
        match c {
            Item::Atom(a) => lines.push(format!("{}{label} {}", "  ".repeat(depth + 1), atom_text(a))),
            Item::Node(n) => render_node(n, &label, depth + 1, lines),
        }
    }
}

fn select<'a>(root: &'a Node, control: Option<&str>) -> Result<Vec<PropsControl<'a>>, OofError> {
    let mut ctrls = controls(root)?;
    if let Some(name) = control {
        ctrls.retain(|c| c.name == name);
        if ctrls.is_empty() {
            return Err(OofError(format!("control {} not found", py_repr(name))));
        }
    }
    Ok(ctrls)
}

/// Python `repr(str)`.
fn py_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Readable `info`/`geom` trees of all controls (or only those named `control`).
pub fn dump_props(raw: &[u8], control: Option<&str>) -> Result<String, OofError> {
    let root = parse_metadata(raw)?;
    let mut lines: Vec<String> = Vec::new();
    for c in select(&root, control)? {
        let suffix = if c.label != c.name { format!(" ({})", c.label) } else { String::new() };
        lines.push(format!("{} {}{suffix}", c.type_name, c.name));
        render_node(c.info, "info", 1, &mut lines);
        render_node(c.geom, "geom", 1, &mut lines);
    }
    Ok(lines.join("\n") + "\n")
}

/// `(path, value in a, value in b)` of the indices that differ between same-named controls.
pub fn diff_props(raw_a: &[u8], raw_b: &[u8], control: Option<&str>) -> Result<Vec<(String, String, String)>, OofError> {
    let root_a = parse_metadata(raw_a)?;
    let ctrls_a = select(&root_a, control)?;
    let root_b = parse_metadata(raw_b)?;
    let ctrls_b = select(&root_b, control)?;

    // `{c[0]: c for c in ...}`: first position, last value
    fn keyed<'a, 'b>(cs: &'b [PropsControl<'a>]) -> (Vec<&'b str>, HashMap<&'b str, &'b PropsControl<'a>>) {
        let mut order = Vec::new();
        let mut map = HashMap::new();
        for c in cs {
            if map.insert(c.label.as_str(), c).is_none() {
                order.push(c.label.as_str());
            }
        }
        (order, map)
    }
    let (order_a, a) = keyed(&ctrls_a);
    let (_, b) = keyed(&ctrls_b);

    let mut rows = Vec::new();
    for key in order_a {
        let Some(cb) = b.get(key) else { continue };
        let ca = a[key];
        for (part, na, nb) in [("info", ca.info, cb.info), ("geom", ca.geom, cb.geom)] {
            let fa = flatten(na, "");
            let fb = flatten(nb, "");
            let ma: HashMap<&str, &str> = fa.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let mb: HashMap<&str, &str> = fb.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let paths = fa.iter().map(|(k, _)| k.as_str()).chain(fb.iter().map(|(k, _)| k.as_str()).filter(|p| !ma.contains_key(p)));
            for path in paths {
                let va = ma.get(path).copied().unwrap_or(MISSING);
                let vb = mb.get(path).copied().unwrap_or(MISSING);
                if va != vb {
                    rows.push((format!("{key}.{part}{path}"), va.to_string(), vb.to_string()));
                }
            }
        }
    }
    Ok(rows)
}

/// Python `OSError.strerror` of the errno-based errors raised by `open()`.
fn errno_text(e: &std::io::Error) -> &'static str {
    use std::io::ErrorKind::*;
    match e.kind() {
        NotFound => "No such file or directory",
        PermissionDenied => "Permission denied",
        AlreadyExists => "File exists",
        InvalidInput => "Invalid argument",
        _ => "OSError",
    }
}

fn read(path: &str) -> Result<Vec<u8>, OofError> {
    match std::fs::read(path) {
        Ok(data) => form_stream(&data),
        Err(e) => {
            let text = if e.raw_os_error() == Some(21) { "Is a directory" } else { errno_text(&e) };
            Err(OofError(format!("cannot read {path}: {text}")))
        }
    }
}

struct Parsed {
    path: String,
    control: Option<String>,
    diff: Option<String>,
}

fn arg_error(message: &str) -> (i32, String, String) {
    (2, String::new(), format!("{USAGE}epf-extract props: error: {message}\n"))
}

/// `-5`, `-.5`, `-1.5`: argparse treats these as values, not options.
fn looks_negative_number(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('-') else { return false };
    let all_digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    match rest.split_once('.') {
        None => all_digits(rest),
        Some((a, b)) => (a.is_empty() || all_digits(a)) && all_digits(b),
    }
}

fn looks_like_option(s: &str) -> bool {
    s.len() > 1 && s.starts_with('-') && !looks_negative_number(s)
}

/// Minimal `argparse` stand-in for `path [--control NAME] [--diff OTHER] [-h]`.
fn parse_args(args: &[String]) -> Result<Parsed, (i32, String, String)> {
    let mut path: Option<String> = None;
    let mut extras: Vec<String> = Vec::new();
    let (mut control, mut diff): (Option<String>, Option<String>) = (None, None);
    let mut i = 0;
    let mut only_positional = false;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        if only_positional || !looks_like_option(arg) {
            if path.is_none() {
                path = Some(arg.clone());
            } else {
                extras.push(arg.clone());
            }
            continue;
        }
        if arg == "--" {
            only_positional = true;
            continue;
        }
        let (name, explicit) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let full = if name == "-h" {
            Some("--help")
        } else if name.starts_with("--") && name.len() > 2 {
            ["--help", "--control", "--diff"].into_iter().find(|o| *o == name).or_else(|| {
                let m: Vec<&str> = ["--help", "--control", "--diff"].into_iter().filter(|o| o.starts_with(&name)).collect();
                if m.len() == 1 {
                    Some(m[0])
                } else {
                    None
                }
            })
        } else {
            None
        };
        match full {
            Some("--help") => return Err((0, format!("{USAGE}{HELP}"), String::new())),
            Some(opt @ ("--control" | "--diff")) => {
                let value = match explicit {
                    Some(v) => v,
                    None => match args.get(i) {
                        Some(next) if !looks_like_option(next) || next == "-" => {
                            i += 1;
                            next.clone()
                        }
                        _ => return Err(arg_error(&format!("argument {opt}: expected one argument"))),
                    },
                };
                if opt == "--control" {
                    control = Some(value);
                } else {
                    diff = Some(value);
                }
            }
            _ => extras.push(arg.clone()),
        }
    }
    let Some(path) = path else {
        return Err(arg_error("the following arguments are required: path"));
    };
    if !extras.is_empty() {
        return Err(arg_error(&format!("unrecognized arguments: {}", extras.join(" "))));
    }
    Ok(Parsed { path, control, diff })
}

fn execute(p: &Parsed) -> Result<String, OofError> {
    let raw = read(&p.path)?;
    match &p.diff {
        None => dump_props(&raw, p.control.as_deref()),
        Some(other) => {
            let rows = diff_props(&raw, &read(other)?, p.control.as_deref())?;
            Ok(rows.iter().map(|(path, va, vb)| format!("{path}\t{va}\t{vb}\n")).collect())
        }
    }
}

/// `epf-extract props <args...>`: returns `(exit code, stdout, stderr)`; `args` excludes `props` itself.
pub fn run(args: &[String]) -> (i32, String, String) {
    let parsed = match parse_args(args) {
        Ok(p) => p,
        Err(r) => return r,
    };
    match execute(&parsed) {
        Ok(out) => (0, out, String::new()),
        Err(e) => (1, String::new(), format!("epf-extract props: error: {}\n", e.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::serialize_container;

    fn fixture(name: &str) -> Vec<u8> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/forms")
            .join(name)
            .join("FormStream.bin");
        std::fs::read(p).unwrap()
    }

    // port: tests/test_oof.py::test_props_dump_and_diff
    #[test]
    fn props_dump_and_diff() {
        let (a, b) = (fixture("DonorForm"), fixture("DonorFormAlt"));
        let text = dump_props(&a, Some("Кнопка1")).unwrap();
        assert!(text.starts_with("Button Кнопка1"));
        assert!(text.contains("info (") && text.contains("geom ("));
        assert!(text.contains("{2, 4, 2, 53}"));
        assert!(diff_props(&a, &a, None).unwrap().is_empty());
        let rows = diff_props(&a, &b, Some("Кнопка1")).unwrap();
        assert!(!rows.is_empty() && rows.iter().all(|r| r.0.starts_with("Кнопка1.")));
        assert!(rows.contains(&("Кнопка1.geom[9][1][3]".to_string(), "53".to_string(), "60".to_string())));
        assert!(!rows.iter().any(|r| r.1 == r.2));
        let err = dump_props(&a, Some("НетТакогоЭлемента")).unwrap_err();
        assert!(err.0.contains("not found"));
        assert_eq!(err.0, "control 'НетТакогоЭлемента' not found");
    }

    // port: tests/test_oof.py::test_props_reads_form_container
    #[test]
    fn props_reads_form_container() {
        let raw = fixture("DonorForm");
        let packed = serialize_container(&[("form", raw.as_slice(), None), ("module", b"", None)], 0x200).unwrap();
        assert_eq!(form_stream(&packed).unwrap(), raw);
        assert_eq!(form_stream(&raw).unwrap(), raw);
        assert!(dump_props(&form_stream(&packed).unwrap(), Some("Кнопка1")).unwrap().starts_with("Button Кнопка1"));
        let no_form = serialize_container(&[("module", b"x", None)], 0x200).unwrap();
        assert_eq!(form_stream(&no_form).unwrap_err().0, "container has no 'form' entry");
    }

    #[test]
    fn argument_errors_follow_argparse() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let (code, out, err) = run(&s(&[]));
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.ends_with("error: the following arguments are required: path\n") && err.starts_with("usage: "));
        assert!(run(&s(&["a", "--control"])).2.ends_with("argument --control: expected one argument\n"));
        assert!(run(&s(&["a", "b", "--zz"])).2.ends_with("unrecognized arguments: b --zz\n"));
        assert_eq!(run(&s(&["-h"])).0, 0);
        let (code, _, err) = run(&s(&["definitely-missing.bin"]));
        assert_eq!(code, 1);
        assert_eq!(err, "epf-extract props: error: cannot read definitely-missing.bin: No such file or directory\n");
    }

    #[test]
    fn python_repr_quotes() {
        assert_eq!(py_repr("Нет"), "'Нет'");
        assert_eq!(py_repr("it's"), "\"it's\"");
        assert_eq!(py_repr("a\"b'c"), "'a\"b\\'c'");
    }
}
