#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Выгружает из docs/design/design-rules.json компактную таблицу порогов для `oneform lint` (src/lint_table.csv).

Запуск (из корня репозитория):

    python scripts/design/export_lint_table.py            # перезаписывает src/lint_table.csv

Файл design-rules.json (~950 КБ) в бинарник не входит: берутся только правила, которые использует `src/lint.rs`
(список RULES ниже), для каждого вида формы (`kind`, без сводного `all`). Столбцы CSV:

    id          id правила из design-rules.json
    kind        вид формы (catalog_item, document, processor, ...)
    value       обычное значение (число; для булевых правил «1»)
    share       доля форм/элементов, следующих правилу, в процентах (целое); для булевых правил - доля «да»
    n           число наблюдений
    divergent   1, если моды по корпусам (УТ 10.3 и Бух.) различаются: такой стиль - выбор человека, lint его не навязывает
    min, max    только для form.width / form.height: наименьшее и наибольшее значение в корпусах

Режимы: `true` - берётся доля значения true (правило «должно быть да»), `modal` - доля модального значения,
`match:<значение>` - доля указанного значения распределения, `range` - границы min/max из квантилей.
"""
import csv
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
SRC = os.path.join(ROOT, 'docs', 'design', 'design-rules.json')
OUT = os.path.join(ROOT, 'src', 'lint_table.csv')

RULES = [
    ('bar.bottom.top_eq_h_minus_height', 'true'),
    ('bar.bottom.left0_full_width', 'true'),
    ('bar.bottom.anchor_class', 'match:растяг/к дальнему краю'),
    ('bar.bottom.height', 'modal'),
    ('bar.top.top0', 'true'),
    ('bar.top.left0_full_width', 'true'),
    ('bar.top.height', 'modal'),
    ('margin.left', 'modal'),
    ('margin.right', 'modal'),
    ('margin.top_gap', 'modal'),
    ('margin.bottom_gap', 'modal'),
    ('field.height', 'modal'),
    ('pair.label_height', 'modal'),
    ('pair.top_delta', 'modal'),
    ('anchors.input_stretch_when_at_right_margin', 'true'),
    ('anchors.table_stretch_both', 'true'),
    ('tab.form.reading_order_exact', 'true'),
    ('tab.page.reading_order_exact', 'true'),
    ('form.width', 'range'),
    ('form.height', 'range'),
]


def share_of(rule, mode):
    if mode == 'true':
        return rule.get('share_true', 0.0) if rule['type'] == 'boolean' else 0.0
    if mode == 'modal' or mode == 'range':
        return rule['share'] if mode == 'modal' else 1.0
    want = mode.split(':', 1)[1]
    for d in rule.get('distribution', []):
        if str(d['value']) == want:
            return d['share']
    return 0.0


def num(v):
    if isinstance(v, bool):
        return '1' if v else '0'
    if isinstance(v, (int, float)):
        return str(int(v)) if float(v).is_integer() else str(v)
    return ''


def main():
    with open(SRC, encoding='utf-8') as f:
        rules = json.load(f)['rules']
    modes = dict(RULES)
    rows = []
    for r in rules:
        mode = modes.get(r['id'])
        if mode is None or r['kind'] == 'all':
            continue
        bc = r.get('by_corpus') or {}
        modal = {json.dumps(v['value']) for v in bc.values()}
        divergent = 1 if len(modal) > 1 else 0
        q = r.get('quantiles') or {}
        lo = num(q.get('min')) if mode == 'range' else ''
        hi = num(q.get('max')) if mode == 'range' else ''
        value = '1' if mode == 'true' else num(r['value']) if mode == 'modal' else ('' if mode == 'range' else '1')
        rows.append([r['id'], r['kind'], value, int(round(share_of(r, mode) * 100)), r['n'], divergent, lo, hi])
    order = {rid: i for i, (rid, _) in enumerate(RULES)}
    rows.sort(key=lambda x: (order[x[0]], x[1]))
    with open(OUT, 'w', encoding='utf-8', newline='') as f:
        w = csv.writer(f, lineterminator='\n')
        w.writerow(['id', 'kind', 'value', 'share', 'n', 'divergent', 'min', 'max'])
        w.writerows(rows)
    print('%d rows -> %s (%d bytes)' % (len(rows), os.path.relpath(OUT, ROOT), os.path.getsize(OUT)))


if __name__ == '__main__':
    sys.exit(main())
