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

/// Parse a UTF-8 (BOM optional) brace-list stream into a single root [`Node`].
/// Positions in errors are character (code point) indexes, as in the Reference.
///
/// All delimiters are ASCII, so the scan runs on bytes; positions are converted to code points only on error.
pub fn parse_metadata(data: &[u8]) -> Result<Node> {
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(data);
    let text = std::str::from_utf8(data)
        .map_err(|_| ExtractionError::new(METADATA_PARSE_ERROR, "metadata stream is not valid UTF-8"))?;
    let t = text.as_bytes();
    let n = t.len();
    let prefix = BASE64_PREFIX.as_bytes();
    let err = |message: &str, i: usize| perr(message, char_pos(t, i));

    // Stack of (node under construction, element_pending). Finished nodes are attached to their parent on pop.
    let mut stack: Vec<(Node, bool)> = Vec::new();
    let mut root: Option<Node> = None;
    let mut i = 0usize;

    fn add(stack: &mut [(Node, bool)], v: Item) -> std::result::Result<(), &'static str> {
        let Some((node, pending)) = stack.last_mut() else {
            return Err("value outside of the root list");
        };
        if *pending {
            return Err("missing comma between values");
        }
        node.children.push(v);
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
                add(&mut stack, Item::Atom(Atom::new(BASE64, body))).map_err(|m| err(m, i))?;
                i = end + 1;
                continue;
            }
            if root.is_some() && stack.is_empty() {
                return Err(err("data after the root list", i));
            }
            // Python links the child to its parent immediately (and checks pending); mirror the checks.
            if let Some((_, pending)) = stack.last() {
                if *pending {
                    return Err(err("missing comma between values", i));
                }
            }
            if stack.len() >= MAX_PARSE_DEPTH {
                return Err(err("nesting too deep", i));
            }
            stack.push((Node::default(), false));
            i += 1;
        } else if c == b'}' {
            let Some((mut node, pending)) = stack.pop() else {
                return Err(err("unbalanced closing brace", i));
            };
            if !pending && !node.children.is_empty() {
                node.children.push(Item::Atom(Atom::new(BARE, "")));
            }
            match stack.last_mut() {
                Some((parent, p)) => {
                    parent.children.push(Item::Node(node));
                    *p = true;
                }
                None => root = Some(node),
            }
            i += 1;
        } else if c == b',' {
            let Some((node, pending)) = stack.last_mut() else {
                return Err(err("comma outside of a list", i));
            };
            if !*pending {
                node.children.push(Item::Atom(Atom::new(BARE, "")));
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
            add(&mut stack, Item::Atom(Atom::new(QUOTED, s))).map_err(|m| err(m, i))?;
        } else {
            let mut j = i;
            while j < n && !is_delim(t[j]) {
                j += 1;
            }
            add(&mut stack, Item::Atom(Atom::new(BARE, &text[i..j]))).map_err(|m| err(m, i))?;
            i = j;
        }
    }
    if !stack.is_empty() {
        return Err(err("unbalanced braces: missing closing brace", n));
    }
    root.ok_or_else(|| perr("empty metadata stream", 0))
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
}
