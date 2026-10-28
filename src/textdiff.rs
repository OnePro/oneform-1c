//! A small unified line diff (common prefix/suffix trimmed, LCS on the middle, context 3) for `oneform history diff`
//! and the feedback report. No dependencies; the output is stable (the same inputs give the same text).

const CONTEXT: usize = 3;
/// Cells of the LCS table; a bigger changed middle part is shown as one replaced block.
const MAX_CELLS: usize = 16_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Del,
    Ins,
}

/// Lines of a text: split on `\n`, one trailing newline does not make an extra line, `\r` is dropped.
pub fn lines_of(text: &str) -> Vec<&str> {
    let body = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    body.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect()
}

/// Edit script of two line lists: `Keep` consumes one line of both, `Del` one of `a`, `Ins` one of `b`.
fn script(a: &[&str], b: &[&str]) -> Vec<Op> {
    let pre = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suf = a[pre..].iter().rev().zip(b[pre..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut ops = vec![Op::Keep; pre];
    if am.len().saturating_mul(bm.len()) > MAX_CELLS {
        ops.extend(std::iter::repeat_n(Op::Del, am.len()));
        ops.extend(std::iter::repeat_n(Op::Ins, bm.len()));
    } else {
        let (n, m) = (am.len(), bm.len());
        // lcs[i][j]: length of the LCS of am[i..] and bm[j..]
        let mut lcs = vec![0u32; (n + 1) * (m + 1)];
        let at = |i: usize, j: usize| i * (m + 1) + j;
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[at(i, j)] = if am[i] == bm[j] { lcs[at(i + 1, j + 1)] + 1 } else { lcs[at(i + 1, j)].max(lcs[at(i, j + 1)]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && am[i] == bm[j] {
                ops.push(Op::Keep);
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {
                ops.push(Op::Del);
                i += 1;
            } else {
                ops.push(Op::Ins);
                j += 1;
            }
        }
    }
    ops.extend(vec![Op::Keep; suf]);
    ops
}

/// Unified diff body (`@@` hunks, no file header) of `old` against `new`; empty when the texts have the same lines.
pub fn unified(old: &str, new: &str) -> Vec<String> {
    let (a, b) = (lines_of(old), lines_of(new));
    let ops = script(&a, &b);
    let changed: Vec<usize> = ops.iter().enumerate().filter(|(_, o)| **o != Op::Keep).map(|(i, _)| i).collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < changed.len() {
        // one hunk: changes closer than twice the context are merged
        let first = changed[k];
        let mut last = first;
        while k + 1 < changed.len() && changed[k + 1] - last <= 2 * CONTEXT {
            k += 1;
            last = changed[k];
        }
        k += 1;
        let from = first.saturating_sub(CONTEXT);
        let to = (last + 1 + CONTEXT).min(ops.len());
        // line counters before `from`
        let (mut ai, mut bi) = (0usize, 0usize);
        for op in &ops[..from] {
            match op {
                Op::Keep => {
                    ai += 1;
                    bi += 1;
                }
                Op::Del => ai += 1,
                Op::Ins => bi += 1,
            }
        }
        let (a0, b0) = (ai, bi);
        let mut body = Vec::new();
        for op in &ops[from..to] {
            match op {
                Op::Keep => {
                    body.push(format!(" {}", a[ai]));
                    ai += 1;
                    bi += 1;
                }
                Op::Del => {
                    body.push(format!("-{}", a[ai]));
                    ai += 1;
                }
                Op::Ins => {
                    body.push(format!("+{}", b[bi]));
                    bi += 1;
                }
            }
        }
        let (alen, blen) = (ai - a0, bi - b0);
        let start = |s: usize, len: usize| if len == 0 { s } else { s + 1 };
        out.push(format!("@@ -{},{} +{},{} @@", start(a0, alen), alen, start(b0, blen), blen));
        out.extend(body);
    }
    out
}

/// Full unified diff of one file: `---`/`+++` header (`/dev/null` for a missing side) and the hunks; empty when equal.
pub fn file_diff(name: &str, old: Option<&str>, new: Option<&str>) -> Vec<String> {
    let body = unified(old.unwrap_or(""), new.unwrap_or(""));
    if body.is_empty() && old.is_some() == new.is_some() {
        return Vec::new();
    }
    let side = |s: Option<&str>, dir: &str| if s.is_some() { format!("{dir}/{name}") } else { "/dev/null".to_string() };
    let mut out = vec![format!("--- {}", side(old, "before")), format!("+++ {}", side(new, "after"))];
    out.extend(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_changed_line_with_context() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nb\nc\nD\ne\nf\ng\nh\n";
        assert_eq!(unified(old, new), ["@@ -1,7 +1,7 @@", " a", " b", " c", "-d", "+D", " e", " f", " g"]);
        assert!(unified(old, old).is_empty());
    }

    #[test]
    fn distant_changes_make_two_hunks_and_added_files_are_whole() {
        let old: String = (1..=20).map(|i| format!("{i}\n")).collect();
        let new: String = (1..=20)
            .map(|i| match i {
                2 => "two".to_string(),
                19 => "nineteen".to_string(),
                _ => i.to_string(),
            } + "\n")
            .collect();
        let d = unified(&old, &new);
        assert_eq!(d.iter().filter(|l| l.starts_with("@@")).count(), 2, "{d:?}");
        assert_eq!(file_diff("X.bsl", None, Some("a\nb\n")), ["--- /dev/null", "+++ after/X.bsl", "@@ -0,0 +1,2 @@", "+a", "+b"]);
        assert_eq!(file_diff("X.bsl", Some("a\n"), None), ["--- before/X.bsl", "+++ /dev/null", "@@ -1,1 +0,0 @@", "-a"]);
        assert!(file_diff("X.bsl", Some("a\n"), Some("a\n")).is_empty());
    }

    #[test]
    fn insertion_in_the_middle_is_not_a_rewrite() {
        let d = unified("a\nb\nc\n", "a\nb\nx\nc\n");
        assert_eq!(d, ["@@ -1,3 +1,4 @@", " a", " b", "+x", " c"]);
    }
}
