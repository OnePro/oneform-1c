#!/usr/bin/env python3
"""Diff two introspection dumps. python target/dump_diff.py orig.txt rebuilt.txt [phase]"""
import collections, re, sys
def load(p, phase):
    d = {}
    for ln in open(p, encoding="utf-8-sig").read().splitlines():
        if "=" not in ln or not ln.startswith(phase + ":"): continue
        k, v = ln.split("=", 1)
        d[k] = v
    return d
phase = sys.argv[3] if len(sys.argv) > 3 else "post"
a, b = load(sys.argv[1], phase), load(sys.argv[2], phase)
print(f"[{phase}] keys orig={len(a)} rebuilt={len(b)}  common={len(set(a)&set(b))}")
def prop(k):
    rest = k.split(":", 1)[1]
    parts = rest.split(".")
    if ".Кол." in rest: return "Кол." + parts[-1]
    if ".Кн." in rest: return "Кн." + parts[-1]
    if ".Стр." in rest: return "Стр"
    if ".Привязка." in rest: return "Привязка." + parts[-1]
    return parts[-1] if len(parts) > 1 else "(тип)"
miss = collections.Counter(prop(k) for k in a if k not in b)
extra = collections.Counter(prop(k) for k in b if k not in a)
diff = collections.defaultdict(list)
for k in a:
    if k in b and a[k] != b[k]: diff[prop(k)].append((k, a[k], b[k]))
print("\n-- present in orig, MISSING in rebuilt (by property):")
for p, n in miss.most_common(18): print(f"   {n:5} {p}")
print("\n-- only in rebuilt:", sum(extra.values()), dict(extra.most_common(6)))
print("\n-- DIFFERENT values (by property): ")
for p, lst in sorted(diff.items(), key=lambda kv: -len(kv[1]))[:22]:
    k, x, y = lst[0]
    print(f"   {len(lst):5} {p:28} e.g. {k.split(':',1)[1][:46]}: orig={x[:30]!r} new={y[:30]!r}")
print("\ntotal differing:", sum(len(v) for v in diff.values()), " total missing:", sum(miss.values()))
