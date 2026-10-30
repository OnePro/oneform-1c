#!/usr/bin/env python3
"""Check the relative links of README.md, docs/*.md and skills/**/*.md: the target file must exist and, for `#anchor`, the heading
must exist in it (GitHub-style slug). Prints the broken links and exits 1 when there are any."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINK = re.compile(r"(?<!\!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")


def slug(heading):
    h = re.sub(r"`", "", heading.strip().lower())
    h = re.sub(r"[^\w\- ]", "", h, flags=re.UNICODE)
    return h.replace(" ", "-")


def anchors(path, cache={}):
    if path not in cache:
        found, in_block = set(), False
        for line in path.read_text(encoding="utf-8").split("\n"):
            if line.lstrip().startswith("```"):
                in_block = not in_block
            elif not in_block and line.startswith("#"):
                found.add(slug(line.lstrip("#")))
        cache[path] = found
    return cache[path]


def main():
    files = [ROOT / "README.md"] + sorted((ROOT / "docs").glob("*.md")) + sorted((ROOT / "skills").rglob("*.md"))
    bad = []
    for f in files:
        in_block = False
        for n, line in enumerate(f.read_text(encoding="utf-8").split("\n"), 1):
            if line.lstrip().startswith("```"):
                in_block = not in_block
                continue
            if in_block:
                continue
            for target in LINK.findall(re.sub(r"`[^`]*`", "", line)):
                if re.match(r"[a-z]+:", target):
                    continue
                path, _, frag = target.partition("#")
                dest = (f.parent / path).resolve() if path else f
                rel = f.relative_to(ROOT).as_posix()
                if not dest.exists():
                    bad.append(f"{rel}:{n}: missing file {target}")
                elif frag and dest.suffix == ".md" and slug(frag) not in anchors(dest):
                    bad.append(f"{rel}:{n}: no heading for #{frag} in {dest.relative_to(ROOT).as_posix()}")
    for b in bad:
        print(b)
    print(f"{len(bad)} broken link(s) in {len(files)} files")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
