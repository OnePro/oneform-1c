#!/usr/bin/env python3
"""List archived OpenSpec changes that mention a check in 1C / live 1C but have no docs/evidence/<change>.md.

  python scripts/check_evidence.py [--strict]

`openspec/` is in .gitignore, so the script reads it only when it exists (a fresh clone prints a note and exits 0).
The change name is the archive directory without the date prefix (2026-10-26-bind-controls-... -> bind-controls-...).
With --strict the exit code is 1 when at least one such change has no evidence file (a gate for archiving).
"""
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
MENTION = re.compile(r"\b1C\b|1С|\blive\b|живая", re.IGNORECASE)  # Latin "1C" and Cyrillic "1С"
NEGATED = re.compile(r"not\s+live", re.IGNORECASE)  # "Not live-1C related"
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}-")


def mentions(tasks: Path):
    out = []
    for n, line in enumerate(tasks.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
        if MENTION.search(line) and not NEGATED.search(line):
            out.append((n, line.strip()))
    return out


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    archive = REPO / "openspec" / "changes" / "archive"
    evidence = REPO / "docs" / "evidence"
    if not archive.is_dir():
        print("openspec/changes/archive not found (openspec/ is not in git): nothing to check")
        return 0
    rows = []
    for d in sorted(p for p in archive.iterdir() if p.is_dir()):
        tasks = d / "tasks.md"
        if not tasks.is_file():
            continue
        found = mentions(tasks)
        if not found:
            continue
        name = DATE.sub("", d.name)
        has = (evidence / f"{name}.md").is_file()
        rows.append((d.name, name, len(found), has, found[0][1]))
    print("| Изменение (архив) | Упоминаний 1С/live в tasks.md | docs/evidence/<имя>.md | Первое упоминание |")
    print("|---|---|---|---|")
    for arch, name, n, has, first in rows:
        first = first.replace("|", "\\|")
        first = first if len(first) <= 110 else first[:107] + "..."
        print(f"| {arch} | {n} | {'есть' if has else '**нет**'} | {first} |")
    missing = [r for r in rows if not r[3]]
    print()
    print(f"Всего с упоминанием: {len(rows)}; без файла доказательств: {len(missing)}")
    return 1 if (missing and "--strict" in sys.argv) else 0


if __name__ == "__main__":
    sys.exit(main())
