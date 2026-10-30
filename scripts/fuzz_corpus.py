#!/usr/bin/env python3
"""Mutation fuzzing of `oneform pack` over an extracted corpus.

usage: fuzz_corpus.py <extracted-corpus-dir> <work-dir> [rounds] [oneform.exe]

Each round copies the corpus, mutates exactly one readable property in one form of every object, runs
`oneform pack` once for the whole copy and checks the outcome per object:
  PACKED   -> Form.xml after pack must differ from before only on the mutated line (carriers aside);
  FAILED   -> classified by error code (a clear rejection is fine, a crash or verify failure is a finding);
Findings are written to <work-dir>/findings.jsonl.
"""
import collections
import difflib
import json
import os
import random
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SUPPORTED = set(open(os.path.join(ROOT, "target", "corpus", "supported_tags.txt")).read().split()) if os.path.exists(
    os.path.join(ROOT, "target", "corpus", "supported_tags.txt")) else set()
STRUCTURAL = {"Left", "Top", "Width", "Height", "ZOrder", "TabOrder", "Caption", "Visible", "DataPath", "Events",
              "Anchors", "ChildItems", "ContextMenu", "ActionSource", "ValueType"}
LINE = re.compile(r"^(\s+)<([A-Za-z]+)>([^<]*)</\2>$")
FONT = re.compile(r"^(\s+<[A-Za-z]*Font\b[^>]*\bsize=\")(\d+)(\".*)$")
COLOR = re.compile(r'^(\s+<[A-Za-z]*Color\b[^>]*\bvalue=")(#[0-9A-Fa-f]{6})(".*)$')
CARRIER_ATTR = re.compile(r" (raw|sig)=(\"[^\"]*\"|'[^']*')")
CARRIER_EL = re.compile(r"^\s*<(Appearance|InfoRecord|PanelRecord|FormRecord|ColumnProps)\b.*/>$")


def forms(corpus):
    for dp, _dn, fn in os.walk(corpus):
        parts = dp.split(os.sep)
        if "src" in parts and "Forms" in parts[parts.index("src"):] and "Form.xml" in fn and ".oneform" not in parts:
            yield os.path.join(dp, "Form.xml")


def object_of(path):
    parts = path.split(os.sep)
    i = parts.index("src")
    return os.sep.join(parts[:i])


def clean(text):
    out = []
    for l in text.replace("\r\n", "\n").split("\n"):
        if CARRIER_EL.match(l):
            continue
        out.append(CARRIER_ATTR.sub("", l))
    return out


def domains(corpus):
    dom = collections.defaultdict(collections.Counter)
    for f in forms(corpus):
        for l in open(f, encoding="utf-8").read().split("\n"):
            m = LINE.match(l)
            if m and m.group(2) in SUPPORTED:
                dom[m.group(2)][m.group(3)] += 1
    return dom


def candidates(path, dom):
    res = []
    lines = open(path, encoding="utf-8", newline="").read().replace("\r\n", "\n").split("\n")
    for i, l in enumerate(lines):
        m = LINE.match(l)
        if m and m.group(2) in SUPPORTED and m.group(2) not in STRUCTURAL:
            tag, val = m.group(2), m.group(3)
            if val in ("true", "false"):
                new = "false" if val == "true" else "true"
            else:
                others = [v for v, _c in dom[tag].most_common(6) if v != val and v.strip()]
                if not others:
                    continue
                new = others[0]
            res.append((i, f"{m.group(1)}<{tag}>{new}</{tag}>", f"{tag} {val} -> {new}"))
            continue
        m = FONT.match(l)
        if m:
            res.append((i, f"{m.group(1)}{int(m.group(2)) + 1}{m.group(3)}", f"Font size {m.group(2)} -> +1"))
            continue
        m = COLOR.match(l)
        if m and m.group(2) != "#123456":
            res.append((i, f"{m.group(1)}#123456{m.group(3)}", f"Color {m.group(2)} -> #123456"))
    return lines, res


def main():
    corpus, work = sys.argv[1], sys.argv[2]
    rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    exe = sys.argv[4] if len(sys.argv) > 4 else os.path.join(ROOT, "target", "release", "oneform.exe" if os.name == "nt" else "oneform")
    dom = domains(corpus)
    os.makedirs(work, exist_ok=True)
    findings = open(os.path.join(work, "findings.jsonl"), "a", encoding="utf-8")
    stats = collections.Counter()
    for rnd in range(rounds):
        rng = random.Random(1000 + rnd)
        copy = os.path.join(work, "run")
        shutil.rmtree(copy, ignore_errors=True)
        shutil.copytree(corpus, copy)
        plan = {}
        by_obj = collections.defaultdict(list)
        for f in forms(copy):
            by_obj[object_of(f)].append(f)
        for obj, fl in by_obj.items():
            rng.shuffle(fl)
            for f in fl:
                lines, cands = candidates(f, dom)
                if not cands:
                    continue
                i, new, what = rng.choice(cands)
                before = clean("\n".join(lines))
                old_line = lines[i]
                lines[i] = new
                open(f, "w", encoding="utf-8", newline="").write("\n".join(lines))
                plan[obj] = (f, i, what, before, old_line, new)
                break
        proc = subprocess.run([exe, "pack", copy, "--no-progress"], capture_output=True, text=True, encoding="utf-8",
                              errors="replace")
        log = proc.stdout + proc.stderr
        results = {}
        cur = None
        for l in log.split("\n"):
            m = re.match(r"^(\S.*?): (PACKED|UNCHANGED|FAILED|UPDATED)(?:, (\w+))?", l)
            if m:
                cur = m.group(1)
                results[cur] = [m.group(2), m.group(3), []]
            elif cur and l.startswith(" "):
                results[cur][2].append(l.strip())
        stats["objects"] += len(plan)
        for obj, (f, i, what, before, old_line, new_line) in plan.items():
            rel = os.path.relpath(obj, copy).replace(os.sep, "/")
            res = results.get(rel)
            if res is None:
                stats["no-result"] += 1
                findings.write(json.dumps({"round": rnd, "obj": rel, "what": what, "outcome": "no-result"}, ensure_ascii=False) + "\n")
                continue
            kind = res[0] if res[1] is None else f"{res[0]}:{res[1]}"
            stats[kind] += 1
            if res[0] == "PACKED":
                after = clean(open(f, encoding="utf-8", newline="").read())
                ops = [o for o in difflib.SequenceMatcher(None, before, after, autojunk=False).get_opcodes() if o[0] != "equal"]
                strip = lambda t: CARRIER_ATTR.sub("", t)
                ok = len(ops) == 0 or (len(ops) == 1 and (
                    (ops[0][0] == "replace" and ops[0][2] - ops[0][1] == 1 and ops[0][4] - ops[0][3] == 1 and after[ops[0][3]] == strip(new_line))
                    or (ops[0][0] == "delete" and ops[0][2] - ops[0][1] == 1 and before[ops[0][1]] == strip(old_line))))
                if not ok:
                    stats["PACKED:extra-diff"] += 1
                    findings.write(json.dumps({"round": rnd, "obj": rel, "what": what, "outcome": "packed-extra-diff",
                                               "ops": [(o[0], before[o[1]:o[2]][:2], after[o[3]:o[4]][:2]) for o in ops[:3]]}, ensure_ascii=False) + "\n")
            elif res[0] == "FAILED" and res[1] not in ("GENERATED_FILE_MODIFIED", "FORM_EDIT_REJECTED"):
                findings.write(json.dumps({"round": rnd, "obj": rel, "what": what, "outcome": kind, "msg": res[2][:3]}, ensure_ascii=False) + "\n")
            elif res[0] == "FAILED":
                findings.write(json.dumps({"round": rnd, "obj": rel, "what": what, "outcome": kind, "msg": res[2][:2]}, ensure_ascii=False) + "\n")
        findings.flush()
        print(f"round {rnd}: exit {proc.returncode}", dict(stats), flush=True)
    findings.close()


if __name__ == "__main__":
    main()
