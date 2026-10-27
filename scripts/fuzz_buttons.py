#!/usr/bin/env python3
"""Command bar button edits (add / remove / reorder) over a sample of an extracted configuration dump.

usage: fuzz_buttons.py <extracted-corpus-dir> <work-dir> [objects-per-scenario=150] [oneform.exe]

Copies only the smallest objects that suit a scenario into <work-dir> (the corpus itself is read-only), re-extracts the
copy with the binary under test, then for each scenario edits one panel per form and runs the binary:
  append  - a new handler button at the end of a panel with two or more handler buttons
  remove  - the last root button of such a panel that is not a sub-menu
  swap    - the first two handler buttons of such a panel exchange their places
  prepend - a separator and a handler button before the first button of a panel with buttons
  fill    - two handlers and a separator in a panel without buttons
Per scenario: `pack --dry-run` must say WOULD PACK, a real `pack` + `extract` + a second `pack --dry-run` must say
NO CHANGES (idempotent). Numbers and the refusal reasons are printed; <work-dir> is deleted at the end.
"""
import collections, os, re, shutil, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = sys.argv[4] if len(sys.argv) > 4 else os.path.join(ROOT, "target", "release", "oneform.exe")
SCENARIOS = ("append", "remove", "swap", "prepend", "fill")
CAPTION = '<Caption><Item language="ru">%s</Item></Caption>'


def nl_of(line):
    return "\r\n" if line.endswith("\r\n") else "\n"


def panels(lines):
    """[(buttons_open, buttons_close, [(start, end, is_handler, is_submenu)])] of the root buttons of every CommandBar."""
    out = []
    for i, l in enumerate(lines):
        m = re.match(r"^(\s*)<CommandBar name=", l)
        if not m or l.rstrip().endswith("/>"):
            continue
        ind = m.group(1)
        end = next((j for j in range(i + 1, len(lines)) if lines[j].rstrip("\r\n") == f"{ind}</CommandBar>"), None)
        if end is None:
            continue
        bo = next((j for j in range(i + 1, end) if lines[j].rstrip("\r\n") == f"{ind}  <Buttons>"), None)
        if bo is None:
            continue
        bc = next((j for j in range(bo + 1, end + 1) if lines[j].rstrip("\r\n") == f"{ind}  </Buttons>"), None)
        if bc is None:
            continue
        bi, items, j = ind + "    ", [], bo + 1
        while j < bc:
            if lines[j].startswith(bi + "<Button "):
                last = j
                if not lines[j].rstrip().endswith("/>"):
                    last = next(k for k in range(j + 1, bc + 1) if lines[k].rstrip("\r\n") == f"{bi}</Button>")
                items.append((j, last, 'actionKind="Handler"' in lines[j], 'kind="Submenu"' in lines[j]))
                j = last
            j += 1
        out.append((bo, bc, items))
    return out


def empty_panel(lines):
    """Index of the closing line of the first CommandBar without <Buttons>."""
    for i, l in enumerate(lines):
        m = re.match(r"^(\s*)<CommandBar name=", l)
        if not m or l.rstrip().endswith("/>"):
            continue
        end = next((j for j in range(i + 1, len(lines)) if lines[j].rstrip("\r\n") == f"{m.group(1)}</CommandBar>"), None)
        if end is not None and not any("<Buttons" in x for x in lines[i:end]):
            return end
    return None


def read(path):
    with open(path, encoding="utf-8", newline="") as f:
        return f.read().splitlines(keepends=True)


def write(path, lines):
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write("".join(lines))


def edit(lines, scenario):
    """The edited lines, or None when the form has no panel for the scenario."""
    if scenario == "fill":
        end = empty_panel(lines)
        if end is None:
            return None
        nl = nl_of(lines[end])
        new = ["<Buttons>" + nl,
               '<Button name="ФаззПервая" kind="Action" action="ФаззПервая">' + CAPTION % "Первая" + "</Button>" + nl,
               '<Button name="ФаззРазделитель" kind="Separator"/>' + nl,
               '<Button name="ФаззВторая" kind="Action" action="ФаззВторая">' + CAPTION % "Вторая" + "</Button>" + nl,
               "</Buttons>" + nl]
        return lines[:end] + new + lines[end:]
    if scenario == "prepend":
        p = next((p for p in panels(lines) if p[2]), None)
        if p is None:
            return None
        nl = nl_of(lines[p[0]])
        new = ['<Button name="ФаззРазделитель" kind="Separator"/>' + nl,
               '<Button name="ФаззПервая" kind="Action" action="ФаззПервая">' + CAPTION % "Первая" + "</Button>" + nl]
        return lines[:p[0] + 1] + new + lines[p[0] + 1:]
    p = next((p for p in panels(lines) if sum(1 for it in p[2] if it[2]) >= 2), None)
    if p is None:
        return None
    bo, bc, items = p
    if scenario == "append":
        new = ['<Button name="ФаззКнопка" kind="Action" action="ФаззПроцедура">' + CAPTION % "Новая" + "</Button>" + nl_of(lines[bo])]
        return lines[:bc] + new + lines[bc:]
    if scenario == "remove":
        flat = [it for it in items if not it[3]]
        if not flat:
            return None
        return lines[:flat[-1][0]] + lines[flat[-1][1] + 1:]
    if scenario == "swap":
        (s1, e1, *_), (s2, e2, *_) = [it for it in items if it[2]][:2]
        return lines[:s1] + lines[s2:e2 + 1] + lines[e1 + 1:s2] + lines[s1:e1 + 1] + lines[e2 + 1:]
    raise SystemExit(scenario)


def run(*args):
    r = subprocess.run([EXE, *args, "--no-progress"], capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.stdout + r.stderr


def statuses(text):
    """object -> (status, [detail lines])"""
    res, cur = {}, None
    for l in text.splitlines():
        m = re.match(r"^(\S.*?): (WOULD PACK|PACKED|NO CHANGES|UNCHANGED|FAILED|WOULD UPDATE|UPDATED|SKIPPED)\b(.*)$", l)
        if m:
            cur = m.group(1)
            res[cur] = (m.group(2), [m.group(3)])
        elif cur and l.startswith("  "):
            res[cur][1].append(l.strip())
    return res


def main():
    corpus, work = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 150
    forms = []
    for dp, dn, fn in os.walk(corpus):
        dn[:] = [d for d in dn if d != ".oneform"]
        parts = dp.split(os.sep)
        if "src" not in parts or "Form.xml" not in fn or "Forms" not in parts[parts.index("src"):]:
            continue
        obj = os.sep.join(parts[:parts.index("src")])
        form = parts[-1]
        bin_path = os.path.join(obj, "Forms", form, "Ext", "Form.bin")
        if not os.path.isfile(bin_path):
            continue
        try:
            lines = read(os.path.join(dp, "Form.xml"))
        except UnicodeDecodeError:
            continue
        forms.append((os.path.getsize(bin_path), obj, form, lines))
    forms.sort(key=lambda f: f[0])
    chosen_by = {}
    for sc in SCENARIOS:
        pick, seen = [], set()
        for _size, obj, form, lines in forms:
            if obj in seen or edit(lines, sc) is None:
                continue
            seen.add(obj)
            pick.append((obj, form))
            if len(pick) >= limit:
                break
        chosen_by[sc] = pick
    chosen = sorted({x[0] for v in chosen_by.values() for x in v})
    print(f"forms: {len(forms)}; copied objects: {len(chosen)}; per scenario {({k: len(v) for k, v in chosen_by.items()})}")
    forms = None
    pristine = work + "-pristine"
    shutil.rmtree(work, ignore_errors=True)
    shutil.rmtree(pristine, ignore_errors=True)
    total = collections.Counter()
    try:
        for obj in chosen:
            rel = os.path.relpath(obj, corpus)
            shutil.copytree(obj, os.path.join(pristine, rel))
            if os.path.isfile(obj + ".xml"):
                shutil.copy(obj + ".xml", os.path.join(pristine, rel + ".xml"))
        # the corpus src may come from an older oneform: re-extract the copy with the binary under test
        run("extract", "--replace-generated", pristine)
        for scenario in SCENARIOS:
            shutil.rmtree(work, ignore_errors=True)
            shutil.copytree(pristine, work)
            edited = 0
            for obj, form in chosen_by[scenario]:
                path = os.path.join(work, os.path.relpath(obj, corpus), "src", "Forms", form, "Form.xml")
                new = edit(read(path), scenario)
                if new is not None:
                    write(path, new)
                    edited += 1
            dry = statuses(run("pack", "--dry-run", work))
            kinds = collections.Counter(v[0] for v in dry.values() if v[0] != "NO CHANGES")
            warn = sum(1 for v in dry.values() for d in v[1] if d.startswith("warning"))
            reasons = collections.Counter(" ".join(v[1][:2])[:230] for v in dry.values() if v[0] == "FAILED")
            real = statuses(run("pack", work))
            packed = [k for k, v in real.items() if v[0] == "PACKED"]
            run("extract", work)
            again = statuses(run("pack", "--dry-run", work))
            idem = [k for k in packed if again.get(k, ("?",))[0] == "NO CHANGES"]
            print(f"[{scenario}] edited {edited}; dry-run {dict(kinds)}; handler warnings {warn}; packed {len(packed)}; "
                  f"idempotent {len(idem)}/{len(packed)}")
            for r, n in reasons.most_common(8):
                print(f"    {n} x {r}")
            for k in packed:
                if k not in idem:
                    print("    NOT IDEMPOTENT:", k, again.get(k))
            total[scenario] = len(idem)
    finally:
        shutil.rmtree(work, ignore_errors=True)
        shutil.rmtree(pristine, ignore_errors=True)
    print("done", dict(total))


if __name__ == "__main__":
    main()
