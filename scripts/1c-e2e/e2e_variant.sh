#!/bin/bash
# usage: e2e_variant.sh <tag> <edits,comma>
cd "C:/Users/egor/projects/oneform-1c"
tag=$1; edits=$2
rm -rf target/e2e-$tag && mkdir -p target/e2e-$tag && cp target/tp-original.epf target/e2e-$tag/tp.epf
target/release/oneform.exe extract target/e2e-$tag --no-progress >/dev/null 2>&1
PYTHONIOENCODING=utf-8 python target/e2e_edit.py target/e2e-$tag $edits >/dev/null
target/release/oneform.exe pack target/e2e-$tag --no-progress 2>&1 | head -1 | cut -c1-120
cp target/e2e-$tag/tp.epf target/e2e-$tag.epf
PYTHONIOENCODING=utf-8 python target/epf_dump.py target/e2e-$tag.epf e2e-$tag 2>&1 | tail -1
PYTHONIOENCODING=utf-8 python target/dump_diff.py target/dumps/orig2.txt target/dumps/e2e-$tag.txt post 2>&1 | grep -E "^ +[0-9]+ |total differing|only in"
