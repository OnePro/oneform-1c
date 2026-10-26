#!/bin/bash
# usage: e2e_small.sh <FormName> : add a label, change a caption in a small form of the test processor, pack, dump in 1C, diff
cd "C:/Users/egor/projects/oneform-1c"
form=$1; tag=small-$form
rm -rf target/e2e-$tag && mkdir -p target/e2e-$tag && cp target/tp-original.epf target/e2e-$tag/tp.epf
target/release/oneform.exe extract target/e2e-$tag --no-progress >/dev/null 2>&1
PYTHONIOENCODING=utf-8 python - "$form" "target/e2e-$tag" <<'PY'
import re, sys
form, d = sys.argv[1], sys.argv[2]
p = f"{d}/src/Forms/{form}/Form.xml"
t = open(p, encoding="utf-8", newline="").read().replace("\r\n", "\n")
L = t.split("\n")
# the first top-level control sits at indent 4 or 8; find the root ChildItems closing line
ci = [i for i, l in enumerate(L) if l == "  </ChildItems>"]
assert ci, "no root ChildItems"
end = ci[-1]
ids = [int(x) for x in re.findall(r' id="(\d+)"', t)]
tabs = [int(x) for x in re.findall(r"<TabOrder>(\d+)<", t)]
zs = [int(x) for x in re.findall(r"^    {1}<[A-Z]\w+ name=.*\n(?:.*\n){0,12}?      <ZOrder>(\d+)<", t, re.M)] or [0]
new = ['    <LabelDecoration name="АгентНовая" id="%d">' % (max(ids) + 1),
       '      <Caption><Item language="ru">Добавлено агентом</Item></Caption>',
       '      <Left>10</Left><Top>2</Top><Width>150</Width><Height>18</Height>',
       '      <ZOrder>%d</ZOrder><TabOrder>%d</TabOrder>' % (len(re.findall(r"^    <[A-Z]\w+ name=", t, re.M)), max(tabs + [0]) + 1),
       '    </LabelDecoration>']
L[end:end] = new
open(p, "w", encoding="utf-8", newline="").write("\n".join(L))
PY
target/release/oneform.exe pack target/e2e-$tag --no-progress 2>&1 | head -4 | cut -c1-200
cp target/e2e-$tag/tp.epf target/e2e-$tag.epf
[ -f target/dumps/orig-$form.txt ] || PYTHONIOENCODING=utf-8 python target/epf_dump.py target/tp-original.epf orig-$form "$form" 2>&1 | tail -1
PYTHONIOENCODING=utf-8 python target/epf_dump.py target/e2e-$tag.epf e2e-$tag "$form" 2>&1 | tail -1
PYTHONIOENCODING=utf-8 python target/dump_diff.py target/dumps/orig-$form.txt target/dumps/e2e-$tag.txt post 2>&1 | grep -E "^ +[0-9]+ [А-Я]|total differing|only in|keys"
