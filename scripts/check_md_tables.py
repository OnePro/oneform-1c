#!/usr/bin/env python3
"""Check that every Markdown table in the given files/dirs has the same number of cells in every row.
A cell separator is any '|' that is not escaped with a backslash.

  python scripts/check_md_tables.py docs/design/preview-designer"""
import os, sys

BS = chr(92)


def cells(line):
    s = line.strip()
    n = 0
    prev = ''
    for ch in s:
        if ch == '|' and prev != BS:
            n += 1
        prev = ch
    # leading and trailing pipes are borders, not separators
    if s.startswith('|'):
        n -= 1
    if s.endswith('|') and not s.endswith(BS + '|'):
        n -= 1
    return n + 1


def check(path):
    bad = tables = rows = 0
    hdr = None
    fence = False
    for i, line in enumerate(open(path, encoding='utf-8'), 1):
        if line.lstrip().startswith('```'):
            fence = not fence
        if fence or not line.lstrip().startswith('|'):
            hdr = None
            continue
        n = cells(line)
        if hdr is None:
            hdr = n
            tables += 1
        else:
            rows += 1
            if n != hdr:
                bad += 1
                print('%s:%d: %d cells, header has %d' % (path, i, n, hdr))
    return tables, rows, bad


def main():
    files = []
    for a in sys.argv[1:] or ['.']:
        if os.path.isdir(a):
            for d, _, fs in os.walk(a):
                files += [os.path.join(d, f) for f in fs if f.endswith('.md') and 'gallery' not in d.replace(BS, '/').split('/')]
        else:
            files.append(a)
    tot = [0, 0, 0]
    for f in sorted(files):
        t, r, b = check(f)
        tot = [tot[0] + t, tot[1] + r, tot[2] + b]
        print('%-70s tables=%d rows=%d bad=%d' % (f, t, r, b))
    print('TOTAL tables=%d rows=%d bad=%d' % tuple(tot))
    sys.exit(1 if tot[2] else 0)


if __name__ == '__main__':
    main()
