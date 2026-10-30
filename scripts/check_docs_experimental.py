#!/usr/bin/env python3
"""Check that documentation examples of `oneform pack|new-form|clone-form` that change the form layout carry `--experimental`.

Scans docs/*.md (not docs/evidence, docs/archive, docs/design), skills/**/*.md and README.md for commands in fenced code blocks and
inline code. Rules:

* `new-form` and `clone-form` always need `--experimental`.
* In docs/experimental-form-editing.md and skills/oneform-forms-experimental/SKILL.md every `pack` needs `--experimental`
  (`--dry-run` lines are exempt: a dry run works without the flag).
* In every other file `pack` without the flag is allowed when its context (the paragraph or the code block with the lines just
  before it) is about module text: it names FormModule / ObjectModule / "модул" / "стабильн"; `--dry-run` is exempt.

A bare inline mention (`oneform pack` without arguments) is not an example and is skipped.

Prints the findings and exits 1 when there are any.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXPERIMENTAL_FILES = {"docs/experimental-form-editing.md", "skills/oneform-forms-experimental/SKILL.md"}
CMD = re.compile(r"\boneform\s+(pack|new-form|clone-form)\b")
MODULE_WORDS = re.compile(r"FormModule|ObjectModule|модул|стабильн|module", re.I)


def files():
    out = [ROOT / "README.md"]
    out += sorted((ROOT / "docs").glob("*.md"))
    out += sorted((ROOT / "skills").rglob("*.md"))
    return [f for f in out if f.is_file()]


def commands(line, in_block):
    """Yield (command name, text of the command) for each oneform pack|new-form|clone-form in a code line or inline code span."""
    if in_block:
        spans = [line]
    else:
        spans = re.findall(r"`([^`]*)`", line)
    for span in spans:
        for m in CMD.finditer(span):
            # the command runs to the end of the span or the next `;` / `#` / `&&`
            rest = re.split(r"\s#\s|;|&&|\s\|\s", span[m.start():])[0]
            yield m.group(1), rest


def context(lines, i, in_block):
    """The paragraph (or code block with the 3 lines before it) around line i."""
    a = i
    while a > 0 and lines[a - 1].strip() and not lines[a - 1].startswith("|"):
        a -= 1
    b = i
    while b + 1 < len(lines) and lines[b + 1].strip() and not lines[b + 1].startswith("|"):
        b += 1
    if lines[i].startswith("|"):
        a = b = i
    text = "\n".join(lines[a : b + 1])
    if in_block or a == i:
        text = "\n".join(lines[max(0, a - 3) : a]) + "\n" + text
    return text


def scan(path):
    rel = path.relative_to(ROOT).as_posix()
    lines = path.read_text(encoding="utf-8").split("\n")
    experimental = rel in EXPERIMENTAL_FILES
    findings = []
    in_block = False
    for i, line in enumerate(lines):
        if line.lstrip().startswith("```"):
            in_block = not in_block
            continue
        for name, cmd in commands(line, in_block):
            if "--experimental" in cmd:
                continue
            if not in_block and cmd.strip() == f"oneform {name}":
                continue  # a bare mention of the command name, not an example
            if name == "pack" and "--dry-run" in cmd:
                continue
            if name == "pack" and not experimental and MODULE_WORDS.search(context(lines, i, in_block)):
                continue
            findings.append((rel, i + 1, cmd.strip()))
    return findings


def main():
    findings = [f for p in files() for f in scan(p)]
    for rel, n, cmd in findings:
        print(f"{rel}:{n}: no --experimental in a layout context: {cmd}")
    if findings:
        print(f"{len(findings)} finding(s)")
        return 1
    print("ok: every layout example of pack / new-form / clone-form carries --experimental")
    return 0


if __name__ == "__main__":
    sys.exit(main())
