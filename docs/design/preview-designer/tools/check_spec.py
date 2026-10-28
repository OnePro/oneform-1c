#!/usr/bin/env python3
"""Self-check of the returned folder (stdlib only).

  python tools/check_spec.py [--dir <folder with style-sheet.html and render-spec.md>]

Checks:
  1. every catalog ID (from ../02-property-catalog.md, rows whose 'Что делает превью сейчас' is not 'рисуется')
     has a rule row in render-spec.md (a table row starting with `ID`, with a non-empty 'действие', not '…');
  2. every ID with a rule (other than 'не отображать') is illustrated on style-sheet.html (data-xml="...ID...");
  3. style-sheet.html is self-contained: no http(s):// references except inside comments/text, no <link>, no @import,
     no external src;
  4. theme.css exists and is non-empty.
Exit code 1 when something is missing (the list is printed)."""
import argparse, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))


def catalog_ids(path):
    ids = []
    for line in open(path, encoding='utf-8'):
        m = re.match(r'\| `([A-Za-z]+\.[A-Za-z@\.]+)` \|', line)
        if m and line.count('|') > 8:
            ids.append(m.group(1))
    return sorted(set(ids))


def spec_rules(path):
    rules = {}
    for line in open(path, encoding='utf-8'):
        m = re.match(r'\|\s*`([A-Za-z]+\.[A-Za-z@\.]+)`\s*\|(.*)\|\s*$', line)
        if not m:
            continue
        cells = [c.strip() for c in m.group(2).split('|')]
        if len(cells) >= 2 and cells[-1] not in ('', '…'):
            rules[m.group(1)] = cells[-1]
    return rules


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--dir', default=os.getcwd())
    ap.add_argument('--catalog', default=os.path.join(HERE, '..', '02-property-catalog.md'))
    a = ap.parse_args()
    bad = 0
    ids = catalog_ids(a.catalog)
    spec_p = os.path.join(a.dir, 'render-spec.md')
    sheet_p = os.path.join(a.dir, 'style-sheet.html')
    rules = spec_rules(spec_p) if os.path.isfile(spec_p) else {}
    sheet = open(sheet_p, encoding='utf-8').read() if os.path.isfile(sheet_p) else ''
    if not rules:
        print('render-spec.md missing or has no filled rule rows'); bad += 1
    if not sheet:
        print('style-sheet.html missing'); bad += 1
    annotated = ' '.join(re.findall(r'data-xml="([^"]*)"', sheet))
    no_rule = [i for i in ids if i not in rules]
    no_specimen = [i for i in ids if i in rules and not rules[i].lower().startswith('не отображать') and i not in annotated]
    for title, lst in (('IDs without a rule in render-spec.md', no_rule), ('IDs with a rule but no specimen (data-xml) on the sheet', no_specimen)):
        if lst:
            bad += 1
            print('%s (%d):' % (title, len(lst)))
            for i in lst[:80]:
                print('  ', i)
            if len(lst) > 80:
                print('   … and %d more' % (len(lst) - 80))
    body = re.sub(r'<!--.*?-->', '', sheet, flags=re.S)
    ext = re.findall(r'(?:src|href)\s*=\s*["\']https?://[^"\']*|@import|url\(\s*["\']?https?://|<link\b', body, flags=re.I)
    if ext:
        bad += 1
        print('style-sheet.html is not self-contained:', ext[:5])
    t = os.path.join(a.dir, 'theme.css')
    if not (os.path.isfile(t) and os.path.getsize(t) > 0):
        bad += 1
        print('theme.css missing or empty')
    print('catalog IDs: %d; with rule: %d; illustrated: %d; verdict: %s' % (
        len(ids), len([i for i in ids if i in rules]), len([i for i in ids if i in annotated]), 'OK' if not bad else 'PROBLEMS'))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
