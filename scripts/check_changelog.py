#!/usr/bin/env python3
"""Проверка CHANGELOG.md и работа с ним при выпуске релиза (только стандартная библиотека).

  python3 scripts/check_changelog.py [check]                     формат файла (по умолчанию)
  python3 scripts/check_changelog.py release X.Y.Z               можно ли выпускать X.Y.Z: запись есть, версия = Cargo.toml, [Unreleased] пуст
  python3 scripts/check_changelog.py prepare X.Y.Z [--date YYYY-MM-DD] [--dry-run]
                                                                 перенести [Unreleased] в раздел [X.Y.Z]
  python3 scripts/check_changelog.py notes X.Y.Z                 текст раздела для описания релиза GitHub

Общие параметры: --file PATH (по умолчанию CHANGELOG.md в корне репозитория), --cargo PATH (для release; по умолчанию Cargo.toml).
Код выхода: 0 — всё в порядке, 1 — нарушение (сообщения со строками в stderr), 2 — неверные аргументы.
"""
import argparse
import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SUBSECTIONS = ("Добавлено", "Изменено", "Исправлено", "Удалено", "Безопасность", "Экспериментальное")
SEMVER = r"\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?"
RE_SEMVER = re.compile(SEMVER)
RE_UNRELEASED = re.compile(r"^## \[Unreleased\]\s*$")
RE_RELEASED = re.compile(r"^## \[([^\]]*)\] - (.*?)\s*$")
RE_DATE = re.compile(r"\d{4}-\d{2}-\d{2}")
COMMANDS = ("check", "release", "prepare", "notes")


class Section:
    def __init__(self, line, version, date, kind):
        self.line = line  # 1-based number of the heading
        self.kind = kind  # "unreleased", "released" or "bad" (a heading in a wrong format)
        self.version = version  # None for [Unreleased]
        self.date = date
        self.end = line  # 1-based number of the last line of the section
        self.subsections = []  # [name, line, [bullet line numbers]]

    def bullets(self):
        return sum(len(s[2]) for s in self.subsections)


def semver_key(v):
    """Sort key: higher means a later version; a pre-release sorts before its release (SemVer 2.0.0, item 11)."""
    core, _, pre = v.partition("-")
    nums = tuple(int(x) for x in core.split("."))
    if not pre:
        return (nums, 1, ())
    ids = tuple((0, int(x), "") if x.isdigit() else (1, 0, x) for x in pre.split("."))
    return (nums, 0, ids)


def valid_date(s):
    if not RE_DATE.fullmatch(s):
        return None
    try:
        return datetime.date.fromisoformat(s)
    except ValueError:
        return None


def parse(lines):
    """Return (sections, errors). errors: list of (line number or None, message)."""
    sections, errors = [], []
    cur = None
    sub = None
    for n, raw in enumerate(lines, 1):
        line = raw.rstrip()
        if line.startswith("## "):
            sub = None
            if RE_UNRELEASED.match(line):
                cur = Section(n, None, None, "unreleased")
            else:
                m = RE_RELEASED.match(line)
                if not m or not RE_SEMVER.fullmatch(m.group(1)):
                    errors.append((n, "заголовок раздела должен быть `## [Unreleased]` или `## [X.Y.Z] - YYYY-MM-DD`"))
                    cur = Section(n, None, None, "bad")
                else:
                    if valid_date(m.group(2)) is None:
                        errors.append((n, f"неверная дата `{m.group(2)}`: нужен формат YYYY-MM-DD и существующий день"))
                    cur = Section(n, m.group(1), m.group(2), "released")
            sections.append(cur)
            continue
        if cur is None:
            continue  # preamble
        cur.end = n
        if line.startswith("### "):
            name = line[4:].strip()
            if name not in SUBSECTIONS:
                errors.append((n, f"неизвестный подраздел `{name}`; допустимы: {', '.join(SUBSECTIONS)}"))
            elif any(s[0] == name for s in cur.subsections):
                errors.append((n, f"подраздел `{name}` повторяется в этом разделе"))
            sub = [name, n, []]
            cur.subsections.append(sub)
        elif line.startswith("#"):
            errors.append((n, "заголовок такого уровня здесь не допускается (только `##` и `###`)"))
        elif not line.strip():
            continue
        elif line.startswith("- "):
            if line[2:].strip() == "":
                errors.append((n, "пустой пункт списка"))
            if sub is None:
                errors.append((n, "пункт списка вне подраздела (`### Добавлено` и т. п.)"))
            else:
                sub[2].append(n)
        elif line[0] in " \t" and sub is not None and sub[2]:
            continue  # continuation of the previous bullet
        else:
            errors.append((n, "строка должна начинаться с `- ` (пункт списка) или быть заголовком"))
    return sections, errors


def validate(lines):
    sections, errors = parse(lines)
    if not sections:
        errors.append((None, "нет ни одного раздела `## `"))
        return sections, errors
    if sections[0].kind != "unreleased":
        errors.append((sections[0].line, "первым разделом должен быть `## [Unreleased]`"))
    for s in sections:
        for name, line, bullets in s.subsections:
            if not bullets:
                errors.append((line, f"подраздел `{name}` пуст: уберите заголовок или добавьте пункты `- `"))
    released = [s for s in sections[1:] if s.version is not None] if sections[0].version is None else \
        [s for s in sections if s.version is not None]
    seen = {}
    prev = None
    for s in released:
        if s.version in seen:
            errors.append((s.line, f"версия {s.version} уже есть (строка {seen[s.version]})"))
        seen.setdefault(s.version, s.line)
        if s.bullets() == 0:
            errors.append((s.line, f"в разделе [{s.version}] нет ни одного пункта списка: релиз без списка изменений запрещён"))
        if prev is not None:
            if s.version != prev.version and semver_key(s.version) >= semver_key(prev.version):
                errors.append((s.line, f"версии должны убывать сверху вниз: {s.version} после {prev.version}"))
            d1, d2 = valid_date(prev.date or ""), valid_date(s.date or "")
            if d1 and d2 and d2 > d1:
                errors.append((s.line, f"дата {s.date} позже даты {prev.date} раздела выше: даты не должны расти вниз по файлу"))
        prev = s
    errors.sort(key=lambda e: (e[0] is None, e[0] or 0))
    return sections, errors


def read_lines(path):
    if not path.is_file():
        fail([(None, f"нет файла {path}")], path)
    return path.read_bytes().decode("utf-8").splitlines()


def fail(errors, path, code=1):
    for line, msg in errors:
        print(f"{path.name}:{line}: {msg}" if line else f"{path.name}: {msg}", file=sys.stderr)
    sys.exit(code)


def load(path):
    """Read and validate the file; exit 1 on format errors."""
    lines = read_lines(path)
    sections, errors = validate(lines)
    if errors:
        fail(errors, path)
    return lines, sections


def find(sections, version):
    return next((s for s in sections if s.version == version), None)


def need_version(version):
    if not RE_SEMVER.fullmatch(version):
        print(f"неверная версия `{version}`: нужен формат X.Y.Z или X.Y.Z-pre", file=sys.stderr)
        sys.exit(2)


def cargo_version(path):
    if not path.is_file():
        print(f"нет файла {path}", file=sys.stderr)
        sys.exit(1)
    text = path.read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        print(f"{path.name}: не найдена строка version", file=sys.stderr)
        sys.exit(1)
    return m.group(1)


def cmd_check(a, path):
    lines, sections = load(path)
    released = [s.version for s in sections if s.version]
    print(f"{path.name}: формат верен; в [Unreleased] пунктов: {sections[0].bullets()}; выпущенные версии: {', '.join(released) or 'нет'}")


def cmd_release(a, path):
    need_version(a.version)
    lines, sections = load(path)
    s = find(sections, a.version)
    errors = []
    if s is None:
        print(f"нет записи {a.version} в {path.name}", file=sys.stderr)
        sys.exit(1)
    first = next((x for x in sections if x.version is not None), None)
    if s is not first:
        errors.append((s.line, f"раздел [{a.version}] не самый новый: выше него [{first.version}]"))
    if not s.date:
        errors.append((s.line, f"у раздела [{a.version}] нет даты"))
    if s.bullets() == 0:
        errors.append((s.line, f"в разделе [{a.version}] нет пунктов списка"))
    cv = cargo_version(Path(a.cargo) if a.cargo else ROOT / "Cargo.toml")
    if cv != a.version:
        errors.append((None, f"версия {a.version} не равна версии Cargo.toml ({cv})"))
    u = sections[0]
    if u.bullets():
        errors.append((u.line, f"в [Unreleased] остались пункты ({u.bullets()}): при выпуске всё переносится в [{a.version}]"))
    if errors:
        fail(errors, path)
    print(f"{path.name}: запись {a.version} от {s.date} на месте, пунктов: {s.bullets()}, [Unreleased] пуст, Cargo.toml = {cv}")


def body_of(lines, s):
    body = lines[s.line:s.end]
    while body and not body[0].strip():
        body.pop(0)
    while body and not body[-1].strip():
        body.pop()
    return body


def cmd_prepare(a, path):
    need_version(a.version)
    date = a.date or datetime.date.today().isoformat()
    if valid_date(date) is None:
        print(f"неверная дата `{date}`: нужен формат YYYY-MM-DD", file=sys.stderr)
        sys.exit(2)
    lines, sections = load(path)
    u = sections[0]
    if u.bullets() == 0:
        print("в [Unreleased] нет записей: описывать нечего, релиз запрещён", file=sys.stderr)
        sys.exit(1)
    released = [s for s in sections if s.version is not None]
    if find(sections, a.version):
        print(f"версия {a.version} уже есть в {path.name} (строка {find(sections, a.version).line})", file=sys.stderr)
        sys.exit(1)
    if released and semver_key(a.version) <= semver_key(released[0].version):
        print(f"версия {a.version} не больше последней выпущенной {released[0].version}", file=sys.stderr)
        sys.exit(1)
    body = body_of(lines, u)
    out = lines[:u.line] + ["", f"## [{a.version}] - {date}", ""] + body + [""] + lines[u.end:]
    while out and not out[-1].strip():
        out.pop()
    _, errors = validate(out)
    if errors:
        fail(errors, path)
    print(f"[Unreleased] -> [{a.version}] - {date}: переносится пунктов {u.bullets()}")
    if a.dry_run:
        print("--dry-run: файл не изменён; переносится:")
        for l in body:
            print("  " + l if l else "")
        return
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")


def cmd_notes(a, path):
    need_version(a.version)
    lines, sections = load(path)
    s = find(sections, a.version)
    if s is None:
        print(f"нет записи {a.version} в {path.name}", file=sys.stderr)
        sys.exit(1)
    sys.stdout.write("\n".join(body_of(lines, s)) + "\n")


def main():
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", newline="\n")
    argv = sys.argv[1:]
    if not any(x in COMMANDS or x in ("-h", "--help") for x in argv):
        argv = argv + ["check"]
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--file", help="путь к CHANGELOG.md")
    ap.add_argument("--cargo", help="путь к Cargo.toml (для release)")
    sp = ap.add_subparsers(dest="cmd", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--file", default=argparse.SUPPRESS)
    common.add_argument("--cargo", default=argparse.SUPPRESS)
    sp.add_parser("check", parents=[common])
    sp.add_parser("release", parents=[common]).add_argument("version")
    p = sp.add_parser("prepare", parents=[common])
    p.add_argument("version")
    p.add_argument("--date")
    p.add_argument("--dry-run", action="store_true")
    sp.add_parser("notes", parents=[common]).add_argument("version")
    a = ap.parse_args(argv)
    path = Path(a.file) if a.file else ROOT / "CHANGELOG.md"
    {"check": cmd_check, "release": cmd_release, "prepare": cmd_prepare, "notes": cmd_notes}[a.cmd](a, path)


if __name__ == "__main__":
    main()
