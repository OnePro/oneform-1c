import os, re, sys, random, subprocess, collections, json
sys.path.insert(0, "scripts")
ROOT = os.getcwd()
tag = sys.argv[1] if len(sys.argv) > 1 else "sweep1"
n = int(sys.argv[2]) if len(sys.argv) > 2 else 40
seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
d = f"target/e2e-{tag}"
subprocess.run(["rm", "-rf", d]); os.makedirs(d)
subprocess.run(["cp", "target/tp-original.epf", f"{d}/tp.epf"])
subprocess.run(["target/release/oneform.exe", "extract", d, "--no-progress"], capture_output=True)
p = f"{d}/src/Forms/ОсновнаяФорма/Form.xml"
L = open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n").split("\n")
sup = set(open("target/corpus/supported_tags.txt").read().split())
STRUCT = {"Left", "Top", "Width", "Height", "ZOrder", "TabOrder", "Visible", "DataPath", "Events", "Anchors", "ChildItems", "ContextMenu", "ActionSource", "ValueType", "Caption"}
LINE = re.compile(r"^(\s+)<([A-Za-z]+)>([^<]*)</\2>$")
FONT = re.compile(r'^(\s+<Font\b[^>]*\bsize=")(\d+)(".*)$')
COLOR = re.compile(r'^(\s+<[A-Za-z]*Color\b[^>]*\bvalue=")(#[0-9A-Fa-f]{6})(".*)$')
OPEN = re.compile(r'^(\s+)<([A-Z][A-Za-z]*) name="([^"]+)" id="(\d+)"')
# domains
dom = collections.defaultdict(collections.Counter)
for l in L:
    m = LINE.match(l)
    if m: dom[m.group(2)][m.group(3)] += 1
owner = {}
cur = None
stack = []
cands = []
for i, l in enumerate(L):
    m = OPEN.match(l)
    if m and m.group(2) not in ("Button", "Column", "Page", "Attribute"):
        ind = len(m.group(1))
        while stack and stack[-1][0] >= ind: stack.pop()
        stack.append((ind, m.group(3), m.group(2)))
        continue
    ind = len(l) - len(l.lstrip())
    if not stack or ind != stack[-1][0] + 2: continue
    name, typ = stack[-1][1], stack[-1][2]
    mm = LINE.match(l)
    if mm and mm.group(2) in sup and mm.group(2) not in STRUCT:
        tg, v = mm.group(2), mm.group(3)
        if v in ("true", "false"): new = "false" if v == "true" else "true"
        else:
            others = [x for x, _ in dom[tg].most_common(6) if x != v and x.strip()]
            if not others: continue
            new = others[0]
        cands.append((i, f"{mm.group(1)}<{tg}>{new}</{tg}>", name, f"{tg}: {v} -> {new}"))
        continue
    mm = FONT.match(l)
    if mm: cands.append((i, f"{mm.group(1)}{int(mm.group(2)) + 1}{mm.group(3)}", name, "Font size +1")); continue
    mm = COLOR.match(l)
    if mm and mm.group(2) != "#123456": cands.append((i, f"{mm.group(1)}#123456{mm.group(3)}", name, "Color -> #123456"))
random.Random(seed).shuffle(cands)
chosen, seen = [], set()
for c in cands:
    if c[2] in seen: continue
    seen.add(c[2]); chosen.append(c)
    if len(chosen) >= n: break
for i, new, name, what in chosen: L[i] = new
open(p, "w", encoding="utf-8", newline="").write("\n".join(L))
json.dump([(c[2], c[3]) for c in chosen], open(f"{d}/mutations.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
r = subprocess.run(["target/release/oneform.exe", "pack", d, "--no-progress"], capture_output=True, text=True, encoding="utf-8", errors="replace")
print(r.stdout.split("\n")[0][:200]); 
for l in r.stdout.split("\n")[1:4]: print(l[:200])
print(len(chosen), "mutations on", len(seen), "controls")
