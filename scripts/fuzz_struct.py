#!/usr/bin/env python3
"""Structural mutation fuzzing of `oneform pack`: add, remove, rename, move/resize, recaption one control per object.

usage: fuzz_struct.py <extracted-corpus-dir> <work-dir> [seed] [oneform.exe]
Checks per object: pack must either succeed or reject with a clear error (no crash); after a successful pack the
packed result must be stable (a second pack reports UNCHANGED) and a fresh extract must keep the edit.
"""
import collections, json, os, random, re, shutil, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OPEN = re.compile(r'^(\s+)<([A-Z][A-Za-z]*) name="([^"]+)" id="(\d+)"[^>]*>$')
NL = chr(10)


def forms(corpus):
    for dp, _dn, fn in os.walk(corpus):
        parts = dp.split(os.sep)
        if "src" in parts and "Forms" in parts[parts.index("src"):] and "Form.xml" in fn and ".oneform" not in parts:
            yield os.path.join(dp, "Form.xml")


def object_of(path):
    parts = path.split(os.sep)
    return os.sep.join(parts[:parts.index("src")])


def blocks(lines):
    """(start, end, indent, tag, name, id) of every control element (Buttons items excluded)."""
    res, in_buttons = [], False
    for i, l in enumerate(lines):
        if l.strip().startswith("<Buttons"):
            in_buttons = True
        if l.strip().startswith("</Buttons>"):
            in_buttons = False
        m = OPEN.match(l)
        if not m or in_buttons or m.group(2) in ("Button", "Column", "Page", "Attribute", "Item", "Form"):
            continue
        ind, tag = m.group(1), m.group(2)
        for j in range(i + 1, len(lines)):
            if lines[j] == f"{ind}</{tag}>":
                res.append((i, j, ind, tag, m.group(3), int(m.group(4))))
                break
    return res


def mutate(lines, rng):
    bl = blocks(lines)
    if not bl:
        return None
    kind = rng.choice(os.environ.get("FUZZ_KINDS", "add,remove,rename,move,caption").split(","))
    if kind == "add":
        ids = [int(x) for x in re.findall(r' id="(\d+)"', NL.join(lines))]
        at = next((i for i, l in enumerate(lines) if l == "  <ChildItems>"), None)
        end = next((i for i, l in enumerate(lines) if l == "  </ChildItems>"), None)
        if at is None or end is None:
            return None
        top = [b for b in bl if b[2] == "    "]
        zs, ts = [], []
        for st, en, _i, _t, _n, _c in top:
            for k in range(st, en):
                m = re.match(r"^      <(ZOrder|TabOrder)>(\d+)<", lines[k])
                if m:
                    (zs if m.group(1) == "ZOrder" else ts).append(int(m.group(2)))
        allts = [int(x) for x in re.findall(r"<TabOrder>(\d+)<", NL.join(lines))]
        nid = max(ids) + 1000
        z, t = (max(zs) + 1 if zs else 0), (max(allts) + 1 if allts else 0)
        new = [f'    <LabelDecoration name="ФаззНовая{nid}" id="{nid}">',
               '      <Caption><Item language="ru">Новая надпись</Item></Caption>',
               '      <Left>500</Left><Top>500</Top><Width>80</Width><Height>20</Height>',
               f'      <ZOrder>{z}</ZOrder><TabOrder>{t}</TabOrder>',
               '    </LabelDecoration>']
        return lines[:end] + new + lines[end:], f"add LabelDecoration {nid}"
    s, e, ind, tag, name, cid = rng.choice(bl)
    if kind == "remove":
        if any(lines[k].strip() == "<ChildItems>" for k in range(s, e)):
            return None
        return lines[:s] + lines[e + 1:], f"remove {tag} {name}"
    if kind == "rename":
        lines = list(lines)
        lines[s] = lines[s].replace(f'name="{name}"', f'name="{name}Х"', 1)
        return lines, f"rename {tag} {name}"
    lines = list(lines)
    for k in range(s, e):
        if kind == "move" and lines[k].strip().startswith("<Left>"):
            v = int(re.search(r">(-?\d+)<", lines[k]).group(1))
            lines[k] = re.sub(r">(-?\d+)<", f">{v + 3}<", lines[k], 1)
            return lines, f"move {tag} {name}"
        if kind == "caption" and re.search(r'<Item language="ru">', lines[k]) and lines[k - 1].strip() == "<Caption>":
            lines[k] = re.sub(r'(<Item language="ru">)[^<]*', r"\1Фазз", lines[k], 1)
            return lines, f"caption {tag} {name}"
    return None


def parse(log):
    results, cur = {}, None
    for l in log.split(NL):
        m = re.match(r"^(\S.*?): (PACKED|UNCHANGED|FAILED|UPDATED)(?:, (\w+))?", l)
        if m:
            cur = m.group(1)
            results[cur] = [m.group(2), m.group(3), []]
        elif cur and l.startswith(" "):
            results[cur][2].append(l.strip())
    return results


def main():
    corpus, work = sys.argv[1], sys.argv[2]
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    exe = sys.argv[4] if len(sys.argv) > 4 else os.path.join(ROOT, "target", "release", "oneform.exe")
    rng = random.Random(seed)
    copy = os.path.join(work, "run")
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    shutil.copytree(corpus, copy)
    by_obj = collections.defaultdict(list)
    for f in forms(copy):
        by_obj[object_of(f)].append(f)
    plan = {}
    sample = int(os.environ.get("FUZZ_SAMPLE", "0"))
    objs = list(by_obj.items())
    rng.shuffle(objs)
    for obj, fl in (objs[:sample] if sample else objs):
        rng.shuffle(fl)
        for f in fl:
            lines = open(f, encoding="utf-8", newline="").read().replace("\r\n", NL).split(NL)
            r = mutate(lines, rng)
            if r:
                open(f, "w", encoding="utf-8", newline="").write(NL.join(r[0]))
                plan[obj] = (f, r[1])
                break
    run = lambda *a: subprocess.run([exe, *a, "--no-progress"], capture_output=True, text=True, encoding="utf-8", errors="replace")
    p1 = run("pack", copy)
    res1 = parse(p1.stdout + p1.stderr)
    p2 = run("pack", copy)
    res2 = parse(p2.stdout + p2.stderr)
    stats, out = collections.Counter(), open(os.path.join(work, "findings.jsonl"), "w", encoding="utf-8")
    for obj, (f, what) in plan.items():
        rel = os.path.relpath(obj, copy).replace(os.sep, "/")
        r1, r2 = res1.get(rel), res2.get(rel)
        kind0 = what.split()[0]
        if r1 is None:
            stats[f"{kind0}:no-result"] += 1
            out.write(json.dumps({"obj": rel, "what": what, "outcome": "no-result"}, ensure_ascii=False) + NL)
            continue
        outcome = r1[0] if r1[1] is None else f"{r1[0]}:{r1[1]}"
        stats[f"{kind0}:{outcome}"] += 1
        bad = outcome.startswith("FAILED") and r1[1] in ("PUBLISH_VERIFY_FAILED", "METADATA_LAYOUT_UNSUPPORTED") or r1[0] == "PACKED" and (r2 is None or r2[0] != "UNCHANGED")
        out.write(json.dumps({"obj": rel, "what": what, "outcome": outcome, "second": r2 and r2[0], "msg": r1[2][:3], "bad": bool(bad)}, ensure_ascii=False) + NL)
    out.close()
    print("exit", p1.returncode, p2.returncode, dict(sorted(stats.items())))


if __name__ == "__main__":
    main()
