//! Parser for 1C brace-list streams (port of the parser half of `metadata.py`).

use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::DecodePaddingMode;
use base64::{alphabet, Engine};

use crate::errors::{ExtractionError, Result, METADATA_PARSE_ERROR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomKind {
    Bare,
    Quoted,
    Base64,
}

pub use AtomKind::{Bare as BARE, Base64 as BASE64, Quoted as QUOTED};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub kind: AtomKind,
    pub value: String,
}

impl Atom {
    pub fn new(kind: AtomKind, value: impl Into<String>) -> Atom {
        Atom { kind, value: value.into() }
    }

    pub fn decode_base64(&self) -> Result<Vec<u8>> {
        if self.kind != BASE64 {
            return Err(ExtractionError::new(METADATA_PARSE_ERROR, "atom is not a base64 blob"));
        }
        // Python: b64decode(validate=True): canonical padding required, non-zero trailing bits accepted.
        let cfg = GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::RequireCanonical)
            .with_decode_allow_trailing_bits(true);
        GeneralPurpose::new(&alphabet::STANDARD, cfg)
            .decode(self.value.as_bytes())
            .map_err(|_| ExtractionError::new(METADATA_PARSE_ERROR, "invalid base64 blob"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Node(Node),
    Atom(Atom),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Node {
    pub children: Vec<Item>,
}

impl Node {
    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<&Item> {
        self.children.get(i)
    }

    pub fn node(&self, i: usize) -> Option<&Node> {
        match self.children.get(i) {
            Some(Item::Node(n)) => Some(n),
            _ => None,
        }
    }

    pub fn atom(&self, i: usize) -> Option<&Atom> {
        match self.children.get(i) {
            Some(Item::Atom(a)) => Some(a),
            _ => None,
        }
    }
}

impl Item {
    pub fn as_node(&self) -> Option<&Node> {
        match self {
            Item::Node(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_atom(&self) -> Option<&Atom> {
        match self {
            Item::Atom(a) => Some(a),
            _ => None,
        }
    }
}

pub const MAX_PARSE_DEPTH: usize = 256;
const BASE64_PREFIX: &str = "{#base64:";

fn perr(message: &str, pos: usize) -> ExtractionError {
    ExtractionError::new(METADATA_PARSE_ERROR, message).with("position", pos as u64)
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_delim(c: u8) -> bool {
    matches!(c, b'{' | b'}' | b',' | b'"') || is_ws(c)
}

/// Character (code point) index of the byte offset `i` of valid UTF-8 `b`.
fn char_pos(b: &[u8], i: usize) -> usize {
    b[..i.min(b.len())].iter().filter(|&&x| x & 0xC0 != 0x80).count()
}

fn find_byte(b: &[u8], from: usize, x: u8) -> Option<usize> {
    b[from..].iter().position(|&c| c == x).map(|r| from + r)
}

/// Half-open byte range `[start, end)` into the ORIGINAL input (BOM included, if any).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Span sidecar of a [`Node`]: `span` covers `{`..`}` inclusive; `children` parallels `Node::children`.
/// Implicit empty atoms (between/after commas) get a zero-length span at the delimiter that implied them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanNode {
    pub span: Span,
    pub children: Vec<SpanItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanItem {
    Node(SpanNode),
    /// Quoted atoms include both quotes; base64 atoms cover the whole `{#base64:...}`.
    Atom(Span),
}

impl SpanItem {
    pub fn span(&self) -> Span {
        match self {
            SpanItem::Node(n) => n.span,
            SpanItem::Atom(s) => *s,
        }
    }
}

/// Lossless parse result: the original bytes, the semantic tree and its span sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedMetadata {
    source: Vec<u8>,
    pub root: Node,
    pub spans: SpanNode,
}

/// A checked replacement of `range` (byte offsets into the original source) with `bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub range: Span,
    pub bytes: Vec<u8>,
}

impl SpannedMetadata {
    /// The original input bytes, exactly as given (BOM included).
    pub fn source(&self) -> &[u8] {
        &self.source
    }

    /// Lossless no-op emit: returns the original bytes unchanged.
    pub fn emit(&self) -> Vec<u8> {
        self.source.clone()
    }

    /// Original bytes of a span.
    pub fn slice(&self, span: Span) -> Option<&[u8]> {
        self.source.get(span.start..span.end)
    }

    /// Apply checked, non-overlapping replacements; all other bytes are copied verbatim.
    pub fn apply(&self, replacements: &[Replacement]) -> Result<Vec<u8>> {
        apply_replacements(&self.source, replacements)
    }
}

fn rerr(message: &str, index: usize) -> ExtractionError {
    ExtractionError::new(METADATA_PARSE_ERROR, message).with("replacement", index as u64)
}

/// Apply replacements to `source` atomically: either every range is valid and the result is returned,
/// or an error is returned and nothing is produced. Ranges must be in bounds, start/end on UTF-8
/// character boundaries, outside a leading BOM, and pairwise disjoint (no overlap, nesting, or equal starts).
/// Input order is irrelevant; application proceeds in descending offset order.
pub fn apply_replacements(source: &[u8], replacements: &[Replacement]) -> Result<Vec<u8>> {
    let bom = if source.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let boundary = |o: usize| o == source.len() || source[o] & 0xC0 != 0x80;
    for (k, r) in replacements.iter().enumerate() {
        let Span { start, end } = r.range;
        if start > end {
            return Err(rerr("replacement range is inverted", k));
        }
        if end > source.len() {
            return Err(rerr("replacement range is out of bounds", k));
        }
        if start < bom {
            return Err(rerr("replacement range touches the BOM", k));
        }
        if !boundary(start) || !boundary(end) {
            return Err(rerr("replacement range splits a UTF-8 character", k));
        }
        if std::str::from_utf8(&r.bytes).is_err() {
            return Err(rerr("replacement is not valid UTF-8", k));
        }
    }
    let mut order: Vec<usize> = (0..replacements.len()).collect();
    order.sort_by_key(|&k| (replacements[k].range.start, replacements[k].range.end));
    for w in order.windows(2) {
        let (a, b) = (&replacements[w[0]].range, &replacements[w[1]].range);
        if b.start < a.end || b.start == a.start {
            return Err(rerr("replacement ranges overlap or conflict", w[1]));
        }
    }
    let mut out = source.to_vec();
    for &k in order.iter().rev() {
        let r = &replacements[k];
        out.splice(r.range.start..r.range.end, r.bytes.iter().copied());
    }
    Ok(out)
}

/// Parse a UTF-8 (BOM optional) brace-list stream into a single root [`Node`].
/// Positions in errors are character (code point) indexes, as in the Reference.
///
/// All delimiters are ASCII, so the scan runs on bytes; positions are converted to code points only on error.
pub fn parse_metadata(data: &[u8]) -> Result<Node> {
    parse_spanned(data).map(|m| m.root)
}

/// Like [`parse_metadata`], but also keeps the original bytes and exact byte spans for every node and atom.
pub fn parse_spanned(input: &[u8]) -> Result<SpannedMetadata> {
    let bom = if input.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let data = &input[bom..];
    let text = std::str::from_utf8(data)
        .map_err(|_| ExtractionError::new(METADATA_PARSE_ERROR, "metadata stream is not valid UTF-8"))?;
    let t = text.as_bytes();
    let n = t.len();
    let prefix = BASE64_PREFIX.as_bytes();
    let err = |message: &str, i: usize| perr(message, char_pos(t, i));
    let sp = |start: usize, end: usize| Span { start: start + bom, end: end + bom };

    // Stack of (node under construction, its spans, element_pending). Finished nodes are attached to their parent on pop.
    type Frame = (Node, SpanNode, bool);
    let mut stack: Vec<Frame> = Vec::new();
    let mut root: Option<(Node, SpanNode)> = None;
    let mut i = 0usize;

    fn add(stack: &mut [Frame], v: Item, span: Span) -> std::result::Result<(), &'static str> {
        let Some((node, spans, pending)) = stack.last_mut() else {
            return Err("value outside of the root list");
        };
        if *pending {
            return Err("missing comma between values");
        }
        node.children.push(v);
        spans.children.push(SpanItem::Atom(span));
        *pending = true;
        Ok(())
    }

    while i < n {
        let c = t[i];
        if is_ws(c) {
            i += 1;
        } else if c == b'{' {
            if t[i..].starts_with(prefix) {
                let Some(end) = find_byte(t, i, b'}') else {
                    return Err(err("unterminated base64 blob", i));
                };
                let body: String =
                    text[i + prefix.len()..end].chars().filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n')).collect();
                add(&mut stack, Item::Atom(Atom::new(BASE64, body)), sp(i, end + 1)).map_err(|m| err(m, i))?;
                i = end + 1;
                continue;
            }
            if root.is_some() && stack.is_empty() {
                return Err(err("data after the root list", i));
            }
            // Python links the child to its parent immediately (and checks pending); mirror the checks.
            if let Some((_, _, pending)) = stack.last() {
                if *pending {
                    return Err(err("missing comma between values", i));
                }
            }
            if stack.len() >= MAX_PARSE_DEPTH {
                return Err(err("nesting too deep", i));
            }
            stack.push((Node::default(), SpanNode { span: sp(i, i), children: Vec::new() }, false));
            i += 1;
        } else if c == b'}' {
            let Some((mut node, mut spans, pending)) = stack.pop() else {
                return Err(err("unbalanced closing brace", i));
            };
            if !pending && !node.children.is_empty() {
                node.children.push(Item::Atom(Atom::new(BARE, "")));
                spans.children.push(SpanItem::Atom(sp(i, i)));
            }
            spans.span.end = i + 1 + bom;
            match stack.last_mut() {
                Some((parent, pspans, p)) => {
                    parent.children.push(Item::Node(node));
                    pspans.children.push(SpanItem::Node(spans));
                    *p = true;
                }
                None => root = Some((node, spans)),
            }
            i += 1;
        } else if c == b',' {
            let Some((node, spans, pending)) = stack.last_mut() else {
                return Err(err("comma outside of a list", i));
            };
            if !*pending {
                node.children.push(Item::Atom(Atom::new(BARE, "")));
                spans.children.push(SpanItem::Atom(sp(i, i)));
            }
            *pending = false;
            i += 1;
        } else if c == b'"' {
            let start = i;
            let mut j = i + 1;
            let mut s = String::new();
            loop {
                let Some(k) = find_byte(t, j, b'"') else {
                    return Err(err("unterminated string", start));
                };
                s.push_str(&text[j..k]);
                if k + 1 < n && t[k + 1] == b'"' {
                    s.push('"');
                    j = k + 2;
                } else {
                    i = k + 1;
                    break;
                }
            }
            // Python reports the position after the closing quote for add_value() errors.
            add(&mut stack, Item::Atom(Atom::new(QUOTED, s)), sp(start, i)).map_err(|m| err(m, i))?;
        } else {
            let mut j = i;
            while j < n && !is_delim(t[j]) {
                j += 1;
            }
            add(&mut stack, Item::Atom(Atom::new(BARE, &text[i..j])), sp(i, j)).map_err(|m| err(m, i))?;
            i = j;
        }
    }
    if !stack.is_empty() {
        return Err(err("unbalanced braces: missing closing brace", n));
    }
    let (root, spans) = root.ok_or_else(|| perr("empty metadata stream", 0))?;
    Ok(SpannedMetadata { source: input.to_vec(), root, spans })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Node {
        parse_metadata(s.as_bytes()).unwrap()
    }

    #[test]
    fn nested_and_atoms() {
        let n = p("{1,\"a\"\"b\",{2,3},abc}");
        assert_eq!(n.len(), 4);
        assert_eq!(n.atom(1).unwrap(), &Atom::new(QUOTED, "a\"b"));
        assert_eq!(n.node(2).unwrap().len(), 2);
        assert_eq!(n.atom(3).unwrap(), &Atom::new(BARE, "abc"));
    }

    #[test]
    fn empty_elements_and_bom() {
        let n = parse_metadata("\u{feff}{1,,2,}".as_bytes()).unwrap();
        assert_eq!(n.len(), 4);
        assert_eq!(n.atom(1).unwrap().value, "");
        assert_eq!(n.atom(3).unwrap().value, "");
    }

    #[test]
    fn base64_blob() {
        let n = p("{{#base64:aGVs bG8=}}");
        assert_eq!(n.atom(0).unwrap().decode_base64().unwrap(), b"hello");
    }

    #[test]
    fn errors() {
        for s in ["", "}", "{1}{2}", "{1 2}", "{1", "1", "{\"a}", "{#base64:aa"] {
            let e = parse_metadata(s.as_bytes()).unwrap_err();
            assert_eq!(e.code, METADATA_PARSE_ERROR, "{s}");
        }
        assert!(parse_metadata(&[0xff, 0xfe]).is_err());
    }

    #[test]
    fn error_positions_are_code_points() {
        // Python: _perr(..., pos) with str indexes (the stream is decoded first).
        let pos = |s: &str| parse_metadata(s.as_bytes()).unwrap_err().params["position"].as_u64().unwrap();
        assert_eq!(pos("{\"я\" 1}"), 5);
        assert_eq!(pos("\u{feff}{яя,ё}}"), 6);
        assert_eq!(pos("{\"ж\"\"\"ж\"x}"), 6);
        assert_eq!(pos("{\"жж"), 1);
        assert_eq!(pos("{ё,{"), 4);
        assert_eq!(pos("{ё}{"), 3);
        let n = p("{ж,\"a\"\"ё\",{#base64:aG Vs\r\nbG8=},\"\"}");
        assert_eq!(n.atom(0).unwrap(), &Atom::new(BARE, "ж"));
        assert_eq!(n.atom(1).unwrap(), &Atom::new(QUOTED, "a\"ё"));
        assert_eq!(n.atom(2).unwrap(), &Atom::new(BASE64, "aGVsbG8="));
        assert_eq!(n.atom(3).unwrap(), &Atom::new(QUOTED, ""));
    }

    fn rep(start: usize, end: usize, b: &str) -> Replacement {
        Replacement { range: Span { start, end }, bytes: b.as_bytes().to_vec() }
    }

    #[test]
    fn spans_are_exact() {
        let src = "\u{feff} {1, \"a\"\"ё\" ,,{x},\r\n{#base64:aG Vs\r\nbG8=} }";
        let m = parse_spanned(src.as_bytes()).unwrap();
        let txt = |s: Span| std::str::from_utf8(m.slice(s).unwrap()).unwrap().to_string();
        assert_eq!(txt(m.spans.span), "{1, \"a\"\"ё\" ,,{x},\r\n{#base64:aG Vs\r\nbG8=} }");
        let c = &m.spans.children;
        assert_eq!(c.len(), m.root.len());
        assert_eq!(txt(c[0].span()), "1");
        assert_eq!(txt(c[1].span()), "\"a\"\"ё\"");
        assert_eq!(txt(c[2].span()), "");
        assert_eq!(c[2].span().start, c[2].span().end);
        assert_eq!(txt(c[3].span()), "{x}");
        assert_eq!(txt(c[4].span()), "{#base64:aG Vs\r\nbG8=}");
        assert_eq!(m.root.atom(4).unwrap().value, "aGVsbG8=");
        assert_eq!(m.spans.span.start, 4);
        // trailing implicit empty after a trailing comma
        let m = parse_spanned(b"{1,}").unwrap();
        assert_eq!(m.spans.children[1].span(), Span { start: 3, end: 3 });
    }

    #[test]
    fn semantic_tree_matches_parse_metadata() {
        for s in ["{1,,2,}", "{ж,\"a\"\"ё\",{#base64:aG Vs\r\nbG8=},\"\"}", "\u{feff}{}", "{{},{}}"] {
            assert_eq!(parse_spanned(s.as_bytes()).unwrap().root, p(s), "{s}");
        }
    }

    #[test]
    fn noop_roundtrip_synthetic() {
        for s in [
            "{1,2}",
            "\u{feff}{1,2}",
            " \t\r\n{ 1 , \"a\"\"b\" ,\r\n , {  } ,\n}\r\n\r\n",
            "{\"\",,,\"x\"\"\"\"\"}",
            "{{#base64:QUJD\r\nREVG\r\n R0g=},{#base64:}}",
            "{\"ж\",яя,{ё}}\n",
        ] {
            let m = parse_spanned(s.as_bytes()).unwrap();
            assert_eq!(m.emit(), s.as_bytes(), "{s:?}");
            assert_eq!(m.apply(&[]).unwrap(), s.as_bytes());
        }
    }

    fn fixture_streams(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                fixture_streams(&path, out);
            } else if path.file_name().is_some_and(|n| n == "FormStream.bin") {
                out.push(path);
            }
        }
    }

    #[test]
    fn noop_roundtrip_all_repository_form_streams() {
        let mut files = Vec::new();
        fixture_streams(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"), &mut files);
        if let Ok(corpus) = std::env::var("ONEFORM_CORPUS_DIR") {
            fixture_streams(std::path::Path::new(&corpus), &mut files);
        }
        assert!(files.len() >= 11, "expected at least 11 FormStream.bin files, found {}", files.len());
        let mut ordinary = 0;
        for f in &files {
            let data = std::fs::read(f).unwrap();
            if data == b"layout" {
                // synthetic golden placeholder, not a brace stream
                assert!(parse_spanned(&data).is_err(), "{}", f.display());
                continue;
            }
            ordinary += 1;
            let m = parse_spanned(&data).unwrap_or_else(|e| panic!("{}: {e:?}", f.display()));
            assert_eq!(m.emit(), data, "{}", f.display());
            assert_eq!(m.apply(&[]).unwrap(), data, "{}", f.display());
            assert_eq!(m.root, parse_metadata(&data).unwrap());
            let trailing_ws = data.iter().rev().take_while(|&&b| is_ws(b)).count();
            assert_eq!(m.spans.span.end, data.len() - trailing_ws, "{}", f.display());
        }
        assert!(ordinary >= 6, "expected at least 6 ordinary streams, found {ordinary}");
    }

    #[test]
    fn replacements_change_only_authorized_ranges() {
        let src = "\u{feff}{1, \"a\"\"ё\" ,,{x},\r\n{#base64:aG Vs\r\nbG8=} }";
        let m = parse_spanned(src.as_bytes()).unwrap();
        let c = &m.spans.children;
        let reps = [
            rep(c[3].span().start, c[3].span().end, "{yy,zz}"),
            rep(c[1].span().start, c[1].span().end, "\"Я\""),
            rep(c[2].span().start, c[2].span().end, "Q"),
        ];
        let out = m.apply(&reps).unwrap();
        assert_eq!(out, "\u{feff}{1, \"Я\" ,Q,{yy,zz},\r\n{#base64:aG Vs\r\nbG8=} }".as_bytes());
        // input order does not matter
        let mut rev = reps.to_vec();
        rev.reverse();
        assert_eq!(m.apply(&rev).unwrap(), out);
        // bytes before the first and after the last range are identical
        let (first, last) = (c[1].span(), c[3].span());
        let a = src.as_bytes();
        assert_eq!(&a[..first.start], &out[..first.start]);
        assert_eq!(&a[last.end..], &out[out.len() - (a.len() - last.end)..]);
        assert_eq!(parse_metadata(&out).unwrap().atom(1).unwrap().value, "Я");
    }

    #[test]
    fn invalid_replacements_fail_atomically() {
        let src = "\u{feff}{1,\"ё\",{x}}";
        let m = parse_spanned(src.as_bytes()).unwrap();
        let len = src.len();
        let node = m.spans.children[2].span();
        let inner = match &m.spans.children[2] {
            SpanItem::Node(n) => n.children[0].span(),
            _ => unreachable!(),
        };
        let ok = rep(4, 5, "9");
        let cases: Vec<Vec<Replacement>> = vec![
            vec![ok.clone(), rep(5, len + 1, "")],
            vec![ok.clone(), rep(7, 6, "")],
            vec![ok.clone(), rep(1, 4, "")],
            vec![ok.clone(), rep(0, 0, "")],
            vec![ok.clone(), rep(8, 9, "")],
            vec![ok.clone(), rep(4, 5, "8")],
            vec![ok.clone(), rep(node.start, node.end, "{}"), rep(inner.start, inner.end, "y")],
            vec![rep(4, 8, "a"), rep(6, 9, "b")],
            vec![rep(5, 5, "a"), rep(5, 5, "b")],
            vec![ok.clone(), Replacement { range: Span { start: 5, end: 5 }, bytes: vec![0xff] }],
        ];
        for (k, reps) in cases.iter().enumerate() {
            let e = m.apply(reps).unwrap_err();
            assert_eq!(e.code, METADATA_PARSE_ERROR, "case {k}");
        }
        // touching ranges are allowed
        assert!(m.apply(&[rep(4, 5, "7"), rep(5, 6, "")]).is_ok());
        assert_eq!(m.source(), src.as_bytes());
    }
}
