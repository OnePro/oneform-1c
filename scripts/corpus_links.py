#!/usr/bin/env python3
"""Corpus check of lossless data links. It works on a COPY of an extracted configuration; the original is never touched.

    python scripts/corpus_links.py prepare <corpus> <copy>   # copy <corpus> to <copy> and run `oneform extract` there
    python scripts/corpus_links.py check   <copy>            # rows, columns, [token] counts, `pack --dry-run`
    python scripts/corpus_links.py edit    <copy> [N]        # +1 to the Width of one control with a >= 3-row link in up to N forms
    oneform pack <copy> --no-progress
    python scripts/corpus_links.py verify  <copy>            # the link rows of those forms are byte-identical, the width changed

`check` verifies, for every `Reference/FormStream.bin` with a sibling `Form.xml`:
  * every link row `{control id, {K,{aid},step...}}` is shown by exactly one `<DataPath attributeId steps>` of that
    control, `(attributeId, steps)` re-encode to the same brace text and the text has one segment per step;
  * no control has a `<DataPath>` that has no row;
  * every column step (DATA, CHECK, PIC not `{-1}`) is shown by exactly one `DataPath` / `CheckBoxDataPath` /
    `PictureDataPath` of a `<Column>`;
  * the number of remaining `[token]` segments by kind;
  * `oneform pack --dry-run` says NO CHANGES for the objects whose forms have a link of three or more rows.
Environment: ONEFORM=path/to/oneform.exe (default target/release/oneform[.exe]).
"""
import collections
import json
import os
import random
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ONEFORM = os.environ.get("ONEFORM", str(ROOT / "target" / "release" / ("oneform.exe" if os.name == "nt" else "oneform")))
COLUMN_GUIDS = {"c2cf1953-2796-4fe2-b78c-ff84140b124e", "48a6ebc3-fcc8-4f8f-b399-459fb32aa46b"}
PATH_TAGS = {"DataPath": 0, "CheckBoxDataPath": 1, "PictureDataPath": 2}
SPACE = ", \r\n\t\ufeff"


def parse_brace(text):
    pos = text.index("{")

    def node():
        nonlocal pos
        pos += 1
        items = []
        while True:
            c = text[pos]
            if c == "}":
                pos += 1
                return items
            if c in SPACE:
                pos += 1
            elif c == "{":
                items.append(node())
            elif c == '"':
                end = pos + 1
                while True:
                    end = text.index('"', end)
                    if text[end + 1: end + 2] == '"':
                        end += 2
                        continue
                    break
                items.append(text[pos: end + 1])
                pos = end + 1
            else:
                end = pos
                while text[end] not in ",}":
                    end += 1
                items.append(text[pos:end].strip())
                pos = end

    return node()


def dump(n):
    return "{" + ",".join(dump(i) if isinstance(i, list) else i for i in n) + "}"


def read_stream(stream):
    return parse_brace(Path(stream).read_text(encoding="utf-8-sig"))


def token_of(step):
    """The `steps` token of a stream step, `None` when the shape is not one of the three."""
    if len(step) == 1:
        return step[0]
    if len(step) == 2:
        return step[1] if step[0] == "0" else step[0] + ":" + step[1]
    return None


def brace_of(token):
    if ":" in token:
        q, u = token.split(":")
        return "{" + q + "," + u + "}"
    if token[:1] == "-" or token.isdigit():
        return "{" + token + "}"
    return "{0," + token + "}"


def link_rows(root):
    return [(r[0], r[1]) for r in root[2][3][1:]]


def column_steps(root):
    """`[(slot, token)]` of every column step that is set (not `{-1}`)."""
    out = []

    def walk(n):
        if len(n) > 1 and isinstance(n[0], str) and n[0] in COLUMN_GUIDS and isinstance(n[1], list) and len(n[1]) > 1 and isinstance(n[1][1], list):
            x = n[1][1]
            if x and x[0] == "8" and len(x) >= 5 and isinstance(x[1], list) and len(x[1]) >= 39:
                for slot, s in enumerate(x[2:5]):
                    if isinstance(s, list) and s != ["-1"]:
                        out.append((slot, token_of(s)))
        for i in n:
            if isinstance(i, list):
                walk(i)

    walk(root)
    return out


def bracket_kind(seg):
    t = seg[1:-1]
    if ":" in t:
        return "q:uuid"
    if re.fullmatch(r"[0-9a-fA-F-]{36}", t):
        return "uuid"
    if t.startswith("-"):
        return "-N"
    return "N"


def forms(root):
    for xml in sorted(Path(root).rglob("Form.xml")):
        stream = xml.parent / "Reference" / "FormStream.bin"
        if stream.is_file():
            yield xml, stream


def check_form(xml, stream, stats):
    root = read_stream(stream)
    tree = ET.parse(xml).getroot()
    parent = {c: p for p in tree.iter() for c in p}
    controls = collections.defaultdict(list)
    for e in tree.iter():
        if parent.get(e) is not None and parent[e].tag == "ChildItems" and "id" in e.attrib:
            controls[e.get("id")].append(e)
    bad = []
    seen = set()
    big = False
    for cid, body in link_rows(root):
        stats["rows"] += 1
        big = big or len(body) - 1 >= 3
        stats["rows with K=%d" % (len(body) - 1)] += 1
        if cid in seen:
            bad.append("control %s: two rows" % cid)
        seen.add(cid)
        ctrl = controls.get(cid)
        if not ctrl:
            stats["orphan rows (control not in Form.xml)"] += 1
            continue
        paths = [c for e in ctrl for c in e if c.tag == "DataPath"]
        if len(paths) != 1:
            bad.append("control %s: %d DataPath elements for %s" % (cid, len(paths), dump(body)))
            continue
        dp = paths[0]
        toks = dp.get("steps", "").split()
        if [token_of(s) for s in body[2:]] != toks:
            bad.append("control %s: row %s but steps '%s'" % (cid, dump(body), dp.get("steps")))
            continue
        rebuilt = "{%d,{%s}" % (len(toks) + 1, dp.get("attributeId")) + "".join("," + brace_of(t) for t in toks) + "}"
        if rebuilt != dump(body):
            bad.append("control %s: row %s re-encodes to %s" % (cid, dump(body), rebuilt))
        text = (dp.text or "").strip()
        if text and len(text.split(".")) != len(toks) + 1:
            bad.append("control %s: text '%s' has no segment per step of %s" % (cid, text, dump(body)))
    for cid, ctrl in controls.items():
        for e in ctrl:
            if e.tag != "Column" and any(c.tag == "DataPath" for c in e) and cid not in seen:
                bad.append("control %s: DataPath without a row" % cid)
    want = sorted(column_steps(root), key=str)
    have = []
    for col in tree.iter("Column"):
        for c in col:
            if c.tag in PATH_TAGS:
                if c.get("steps"):
                    have.append((PATH_TAGS[c.tag], c.get("steps")))
                else:
                    stats["column paths without steps"] += 1
    stats["column steps"] += len(want)
    if sorted(have, key=str) != want:
        missing = [w for w in want if w not in have]
        extra = [h for h in have if h not in want]
        bad.append("columns: stream steps not shown %s, shown steps not in the stream %s" % (missing[:3], extra[:3]))
    for e in tree.iter():
        if e.tag in PATH_TAGS:
            for i, seg in enumerate((e.text or "").strip().split(".")):
                if seg.startswith("[") and seg.endswith("]"):
                    stats["[token] segments of kind " + ("root" if i == 0 else bracket_kind(seg))] += 1
                    if i:
                        stats["seg " + seg] += 1
    return bad, big


def object_label(xml):
    parts = xml.parts
    src = len(parts) - 1 - parts[::-1].index("src")
    return "/".join(parts[src - 2: src])


def check(copy):
    stats = collections.Counter()
    bad_all = []
    big_objects = set()
    nforms = 0
    for xml, stream in forms(copy):
        nforms += 1
        bad, big = check_form(xml, stream, stats)
        bad_all += ["%s: %s" % (xml, b) for b in bad]
        if big:
            big_objects.add(object_label(xml))
            stats["forms with a link of >= 3 rows"] += 1
    print("forms %d; mismatches %d" % (nforms, len(bad_all)))
    segs = [(k, v) for k, v in stats.items() if k.startswith("seg ")]
    for k in sorted(stats):
        if not k.startswith("seg "):
            print("  %s: %d" % (k, stats[k]))
    print("  most frequent [token] segments:", sorted(segs, key=lambda kv: -kv[1])[:8])
    for b in bad_all[:30]:
        print("MISMATCH", b)
    out = subprocess.run([ONEFORM, "pack", str(copy), "--dry-run", "--no-progress"], capture_output=True, text=True, encoding="utf-8")
    state = {}
    for line in out.stdout.splitlines():
        m = re.match(r"(.+?): (.*)$", line)
        if m:
            state[m.group(1)] = m.group(2)
    unchanged = sorted(o for o in big_objects if state.get(o) == "NO CHANGES")
    changed = sorted(o for o in big_objects if state.get(o) != "NO CHANGES")
    lines = out.stdout.strip().splitlines()
    print("pack --dry-run:", lines[-1] if lines else out.stderr[-300:])
    print("objects with forms that have a link of >= 3 rows: %d; NO CHANGES %d; other %d" % (len(big_objects), len(unchanged), len(changed)))
    for o in changed[:20]:
        print("NOT UNCHANGED", o, state.get(o))
    sys.exit(1 if bad_all or changed or out.returncode else 0)


def prepare(corpus, copy):
    shutil.copytree(corpus, copy)
    r = subprocess.run([ONEFORM, "extract", str(copy), "--no-progress"], capture_output=True, text=True, encoding="utf-8")
    lines = r.stdout.strip().splitlines()
    print(lines[-1] if lines else r.stderr[-300:])
    sys.exit(r.returncode)


def link_text_rows(stream):
    return [dump(r) for r in link_rows(read_stream(stream))]


def edit(copy, limit):
    rng = random.Random(1)
    done = {}
    todo = []
    for xml, stream in forms(copy):
        text = xml.read_text(encoding="utf-8")
        for m in re.finditer(r'<(\w+) name="[^"]*" id="(\d+)">\s*<DataPath attributeId="-?\d+" steps="([^"]+)"', text):
            if len(m.group(3).split()) >= 2:
                todo.append((xml, stream, m))
                break
    rng.shuffle(todo)
    for xml, stream, m in todo[:limit]:
        text = xml.read_text(encoding="utf-8")
        w = re.compile(r"<Width>(\d+)</Width>").search(text, m.end())
        if not w:
            continue
        new = text[: w.start()] + "<Width>%d</Width>" % (int(w.group(1)) + 1) + text[w.end():]
        xml.write_text(new, encoding="utf-8", newline="")
        done[str(xml)] = {"id": m.group(2), "width": int(w.group(1)), "rows": link_text_rows(stream)}
    Path(copy, ".corpus_links.json").write_text(json.dumps(done, ensure_ascii=False), encoding="utf-8")
    print("edited %d forms" % len(done))


def verify(copy):
    done = json.loads(Path(copy, ".corpus_links.json").read_text(encoding="utf-8"))
    bad = 0
    for xml, rec in done.items():
        stream = Path(xml).parent / "Reference" / "FormStream.bin"
        if link_text_rows(stream) != rec["rows"]:
            bad += 1
            print("LINK ROWS CHANGED", xml)
        if "<Width>%d</Width>" % (rec["width"] + 1) not in Path(xml).read_text(encoding="utf-8"):
            bad += 1
            print("WIDTH LOST", xml)
    print("verified %d forms, %d problems" % (len(done), bad))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "prepare":
        prepare(sys.argv[2], sys.argv[3])
    elif cmd == "check":
        check(sys.argv[2])
    elif cmd == "edit":
        edit(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 100000)
    elif cmd == "verify":
        verify(sys.argv[2])
    else:
        sys.exit(__doc__)
