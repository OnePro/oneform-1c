#!/usr/bin/env python3
"""Check that every tracked path is valid on Windows, macOS and Linux.

Checks over `git ls-files`:
  * no reserved Windows names (CON, PRN, AUX, NUL, COM1-9, LPT1-9, with or without extension);
  * no component ending with a dot or a space; no `<>:"|?*`, backslash or control characters;
  * checkout path (root of 60 characters + tracked path) shorter than 240 characters;
  * no two paths that differ only by case (or only by Unicode normalization);
  * non-ASCII names are in Unicode NFC.

Prints findings and exits 1 when there are any.
"""
import re
import subprocess
import sys
import unicodedata

CHECKOUT_ROOT_LEN = 60
MAX_PATH_LEN = 240
RESERVED = {"CON", "PRN", "AUX", "NUL"} | {f"COM{i}" for i in range(1, 10)} | {f"LPT{i}" for i in range(1, 10)}
BAD_CHARS = re.compile(r'[<>:"|?*\\\x00-\x1f]')


def tracked_paths():
    out = subprocess.run(
        ["git", "-c", "core.quotepath=off", "ls-files", "-z"],
        check=True,
        capture_output=True,
    ).stdout
    return [p.decode("utf-8") for p in out.split(b"\0") if p]


def check(paths):
    findings = []
    seen = {}
    for path in paths:
        if len(path) + CHECKOUT_ROOT_LEN + 1 >= MAX_PATH_LEN:
            findings.append(f"path too long ({len(path) + CHECKOUT_ROOT_LEN + 1} with a {CHECKOUT_ROOT_LEN}-char root): {path}")
        if unicodedata.normalize("NFC", path) != path:
            findings.append(f"not NFC: {ascii(path)}")
        for part in path.split("/"):
            stem = part.split(".")[0].rstrip(" ").upper()
            if stem in RESERVED:
                findings.append(f"reserved Windows name '{part}': {path}")
            if part.endswith(".") or part.endswith(" "):
                findings.append(f"component ends with a dot or space '{part}': {path}")
            if BAD_CHARS.search(part):
                findings.append(f"invalid character in '{part}': {path}")
        key = unicodedata.normalize("NFC", path).casefold()
        if key in seen and seen[key] != path:
            findings.append(f"differs only by case/normalization: {path} <-> {seen[key]}")
        seen.setdefault(key, path)
    # directories (or a directory and a file) whose names collide case-insensitively
    dirs = {}
    for path in paths:
        parts = path.split("/")
        for i in range(1, len(parts)):
            d = "/".join(parts[:i])
            k = unicodedata.normalize("NFC", d).casefold()
            if k in dirs and dirs[k] != d:
                findings.append(f"directories differ only by case: {d} <-> {dirs[k]}")
            dirs.setdefault(k, d)
    return sorted(set(findings))


def main():
    paths = tracked_paths()
    findings = check(paths)
    for f in findings:
        print(f)
    if findings:
        print(f"check_repo_portable: {len(findings)} finding(s) in {len(paths)} tracked paths")
        return 1
    print(f"check_repo_portable: ok ({len(paths)} tracked paths)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
