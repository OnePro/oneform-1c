#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""F2: собирает `docs/form-design-standards.md` и `docs/design/design-rules.json` из результата `analyze_forms.py`.

Запуск (после `analyze_forms.py --out target/design ...`):

    python scripts/design/render_standards.py --rules target/design/design-rules.json \\
        --md docs/form-design-standards.md --json docs/design/design-rules.json

Из полного списка правил берутся только отобранные (список `SECTIONS`, `EXTRA_PATTERNS`); в JSON попадают
те же правила с полями id, kind, topic, parameter, unit, value, share, n, type, follows, distribution,
quantiles, by_corpus. В Markdown — матрицы «правило × вид формы», раздел «Где стили расходятся»
(правила, которым следует меньше порога, с конкурирующими вариантами) и различия корпусов.
"""
import argparse
import json
import os
import re
from collections import defaultdict

MAIN = ['catalog_item', 'catalog_group', 'catalog_list', 'catalog_choice', 'document', 'document_list',
        'document_choice', 'processor', 'report']
KIND_RU = {
    'catalog_item': 'элемент спр.', 'catalog_group': 'группа спр.', 'catalog_list': 'список спр.',
    'catalog_choice': 'выбор спр.', 'document': 'документ', 'document_list': 'список док.',
    'document_choice': 'выбор док.', 'processor': 'обработка', 'report': 'отчёт', 'common_form': 'общая форма',
    'register_list': 'список регистра', 'journal_list': 'журнал док.', 'other': 'прочие', 'all': 'все'}
KIND_NOTE = {
    'catalog_item': 'ФормаЭлемента / СправочникОбъект', 'catalog_group': 'ФормаГруппы',
    'catalog_list': 'ФормаСписка справочника', 'catalog_choice': 'ФормаВыбора* справочника',
    'document': 'ФормаДокумента / ДокументОбъект', 'document_list': 'ФормаСписка документа',
    'document_choice': 'ФормаВыбора документа', 'processor': 'формы обработок (в т. ч. внешних)',
    'report': 'формы отчётов', 'common_form': 'общие формы', 'register_list': 'списки регистров',
    'journal_list': 'журналы документов', 'other': 'планы обмена/счетов/видов, БП, задачи, менеджеры записи'}
CORP_SHORT = {}

# режимы ячейки: val — мода и доля; bool — доля «да»; q — p10/p50/p90 + мода; top2 — два главных значения
SECTIONS = [
    ('size', '1. Размер формы', [
        ('form.width', 'ширина формы, пикс', 'q'), ('form.height', 'высота формы, пикс', 'q'),
        ('form.stub', 'форма — заготовка (нет полей/таблиц/панелей)', 'bool')]),
    ('bars', '2. Командные панели формы', [
        ('bar.bottom.has', 'есть нижняя панель `ОсновныеДействияФормы`', 'bool'),
        ('bar.bottom.top_eq_h_minus_height', 'нижняя: Top = высота формы − высота панели', 'bool'),
        ('bar.bottom.left0_full_width', 'нижняя: Left=0, Width = ширина формы', 'bool'),
        ('bar.bottom.height', 'нижняя: высота', 'val'),
        ('bar.bottom.anchor_class', 'нижняя: привязки (гориз/верт)', 'val'),
        ('bar.bottom.halign', 'нижняя: HorizontalAlign', 'val'),
        ('bar.bottom.autofill', 'нижняя: AutoFill', 'val'),
        ('bar.bottom.is_last_tab', 'нижняя: наибольший TabOrder', 'bool'),
        ('bar.bottom.has_separators', 'нижняя: между кнопками разделители', 'bool'),
        ('bar.bottom.ok_spelling', 'нижняя: написание OK', 'val'),
        ('bar.bottom.shares_row_with_element', 'нижняя: в её строке лежит ещё элемент (подпись «автор»)', 'bool'),
        ('bar.top.has', 'есть верхняя панель `ДействияФормы`', 'bool'),
        ('bar.top.top0', 'верхняя: Top = 0', 'bool'),
        ('bar.top.left0_full_width', 'верхняя: Left=0, Width = ширина формы', 'bool'),
        ('bar.top.height', 'верхняя: высота', 'val'),
        ('bar.top.anchor_class', 'верхняя: привязки (гориз/верт)', 'val'),
        ('bar.top.autofill', 'верхняя: AutoFill', 'val'),
        ('bar.top.is_first_tab', 'верхняя: TabOrder = 1', 'bool'),
        ('bar.top.halign', 'верхняя: HorizontalAlign', 'val'),
        ('bar.other.has', 'есть иные панели команд на уровне формы', 'bool')]),
    ('margins', '3. Отступы и зазоры', [
        ('margin.left', 'левое поле (min Left)', 'val'), ('margin.right', 'правое поле', 'val'),
        ('margin.symmetric', 'левое поле = правому', 'bool'),
        ('margin.top_gap', 'зазор под верхней панелью до первого элемента', 'val'),
        ('margin.bottom_gap', 'зазор от последнего элемента до нижней панели', 'val'),
        ('lint.outside_form', 'есть элемент за краем формы', 'bool'),
        ('lint.overlap', 'есть перекрытие простых элементов', 'bool')]),
    ('pairs', '4. «Подпись — поле», шаг строк', [
        ('pair.position', 'подпись относительно поля', 'val'),
        ('pair.label_left', 'Left подписи', 'val'), ('pair.label_width', 'ширина подписи', 'q'),
        ('pair.gap', 'зазор подпись→поле (Left поля − правый край подписи)', 'val'),
        ('pair.field_left', 'Left поля', 'q'), ('pair.top_delta', 'Top подписи − Top поля', 'val'),
        ('pair.label_height', 'высота подписи', 'val'), ('pair.field_height', 'высота поля', 'val'),
        ('pair.label_halign', 'выравнивание подписи', 'val'),
        ('pair.label_column_same_width', 'подписи левой колонки одной ширины (≥80 %)', 'bool'),
        ('pair.field_column_aligned', 'поля выровнены по общему Left (≥80 %)', 'bool'),
        ('row.pitch', 'шаг строк (все пары соседних строк)', 'val'),
        ('row.pitch_form_mode', 'шаг строк: мода формы', 'val'),
        ('row.pitch_uniform', 'в форме ≥80 % шагов одинаковы', 'bool')]),
    ('anchors', '5. Привязки (растягивание)', [
        ('anchors.input_h_stretch', 'поле ввода растягивается по ширине', 'bool'),
        ('anchors.input_stretch_when_at_right_margin', '… если поле доходит до правого поля', 'bool'),
        ('anchors.input_stretch_when_not_at_right', '… если не доходит до правого поля', 'bool'),
        ('anchors.table_stretch_both', 'таблица растягивается по обеим осям', 'bool'),
        ('anchors.label_fixed', 'подпись не растягивается', 'bool')]),
    ('tab', '6. Порядок обхода и ZOrder', [
        ('tab.form.contiguous_from_1', 'TabOrder формы 1..N без пропусков', 'bool'),
        ('tab.form.reading_order_exact', 'обход = чтение (сверху вниз, слева направо), форма', 'bool'),
        ('tab.page.reading_order_exact', 'то же, страница панели', 'bool'),
        ('tab.page.contiguous_from_1', 'TabOrder страницы 1..N без пропусков', 'bool'),
        ('pair.tab_label_plus1', 'TabOrder поля = TabOrder подписи + 1', 'bool'),
        ('tab.form.z_perm', 'ZOrder — перестановка 0..N−1', 'bool'),
        ('tab.form.z_equals_tab', 'порядок ZOrder = порядку TabOrder', 'bool')]),
    ('panels', '7. Панели (вкладки) и страницы', [
        ('form.has_panel', 'на форме есть панель', 'bool'), ('panel.pages', 'число страниц', 'val'),
        ('panel.show_tabs', 'ShowTabs', 'val'), ('panel.left', 'Left панели', 'val'),
        ('panel.right_margin', 'правое поле панели', 'val'), ('panel.anchor', 'привязки панели', 'val'),
        ('panel.top_gap', 'зазор от верхней панели команд до панели', 'q'),
        ('panel.bottom_gap', 'зазор от низа панели до нижней панели команд', 'q'),
        ('page.geom', 'страница: Left, Top, потеря ширины, потеря высоты относительно панели', 'val'),
        ('page.left_margin', 'внутри страницы: левое поле', 'val'),
        ('page.top_first', 'внутри страницы: Top первого элемента', 'val'),
        ('page.right_margin', 'внутри страницы: правое поле', 'val'),
        ('page.bottom_gap', 'внутри страницы: зазор снизу', 'val'),
        ('panel.page_name_default', 'имя страницы «СтраницаN»', 'bool'),
        ('panel.page_name_eq_title', 'имя страницы = заголовок без пробелов', 'bool'),
        ('panel.page_title_upper', 'заголовок страницы с заглавной', 'bool'),
        ('panel.page_title_colon', 'заголовок страницы оканчивается «:»', 'bool')]),
    ('tables', '8. Таблицы', [
        ('table.has_own_bar', 'у таблицы своя командная панель', 'bool'),
        ('table.bar_flush', 'панель вплотную над таблицей, той же ширины', 'bool'),
        ('table.bar_height', 'высота панели таблицы', 'val'), ('table.bar_autofill', 'AutoFill панели таблицы', 'val'),
        ('table.bar_anchor', 'привязки панели таблицы', 'val'),
        ('name.table_bar', 'имя панели = `КоманднаяПанель`+имя таблицы', 'bool'),
        ('table.footer', 'включён подвал (Footer)', 'bool'), ('table.choice_mode', 'ChoiceMode', 'bool'),
        ('table.read_only', 'таблица ReadOnly', 'bool'), ('table.header_font_bold', 'HeaderFont жирный', 'bool'),
        ('table.columns_count', 'число колонок', 'q'), ('table.first_col_header', 'заголовок первой колонки', 'val'),
        ('table.col_header_upper', 'заголовок колонки с заглавной', 'bool'),
        ('table.col_header_colon', 'заголовок колонки оканчивается «:»', 'bool'),
        ('table.col_readonly', 'колонка только для чтения', 'bool'),
        ('table.height_ratio', 'высота таблицы / высота формы (таблицы формы)', 'val'),
        ('table.bottom_gap', 'зазор от низа таблицы до нижней панели/края', 'val'),
        ('table.left', 'Left таблицы формы', 'val'), ('table.right_margin', 'правое поле таблицы формы', 'val'),
        ('page.table_bottom_gap', 'зазор от низа таблицы до края страницы', 'val'),
        ('table.initial_tree_view', 'InitialTreeView', 'val'), ('table.horizontal_lines', 'HorizontalLines', 'val'),
        ('name.table_eq_attr', 'имя таблицы = имя реквизита', 'bool')]),
    ('names', '9. Имена и подписи', [
        ('name.field_eq_attr', 'имя поля = имя реквизита (DataPath)', 'bool'),
        ('name.field_prefix_pole', 'имя поля начинается с «Поле»', 'bool'),
        ('pair.label_name_matches', 'имя подписи = `Надпись`+имя поля', 'bool'),
        ('pair.label_colon', 'подпись поля оканчивается «:»', 'bool'),
        ('pair.label_upper', 'подпись поля с заглавной', 'bool'),
        ('name.label_prefix', 'префикс имени подписи', 'val'), ('name.group_prefix', 'префикс имени рамки', 'val'),
        ('name.button_prefix', 'префикс имени кнопки вне панели', 'val'),
        ('name.check_eq_attr', 'имя флажка = имя реквизита', 'bool'),
        ('check.caption_colon', 'подпись флажка оканчивается «:»', 'bool'),
        ('check.caption_upper', 'подпись флажка с заглавной', 'bool'),
        ('button.caption_ellipsis', 'подпись кнопки с многоточием', 'bool'),
        ('bar.name_class', 'имена панелей уровня формы', 'val')]),
    ('fonts', '10. Шрифты и цвета', [
        ('font.bold.UsualGroup', 'заголовок рамки жирный', 'bool'),
        ('group.border', 'BorderStyle рамки', 'val'), ('group.height', 'высота рамки-заголовка', 'val'),
        ('pair.label_bold', 'подпись поля жирная', 'bool'),
        ('font.nondefault.LabelDecoration', 'у подписи явный шрифт', 'bool'),
        ('font.nondefault.InputField', 'у поля ввода явный шрифт', 'bool'),
        ('font.nondefault.Button', 'у кнопки явный шрифт', 'bool'),
        ('font.nondefault.Table', 'у таблицы явный шрифт', 'bool'),
        ('color.text.LabelDecoration', 'у подписи явный цвет текста', 'bool'),
        ('color.back.LabelDecoration', 'у подписи явный цвет фона', 'bool'),
        ('color.back.Table', 'у таблицы явный цвет фона', 'bool'),
        ('font.face', 'гарнитура явных шрифтов', 'val'),
        ('field.auto_mark_incomplete', 'поля с AutoMarkIncomplete (всё, что отмечено)', 'bool'),
        ('pair.label_bold_given_fillchecking.required', 'жирная подпись, если реквизит обязателен (FillChecking)', 'bool'),
        ('pair.label_bold_given_fillchecking.optional', 'жирная подпись, если реквизит не обязателен', 'bool')]),
    ('buttons', '11. Самостоятельные кнопки', [
        ('button.height', 'высота кнопки', 'val'), ('button.width', 'ширина кнопки', 'q'),
        ('button.anchor', 'привязки кнопки', 'val')]),
]
EXTRA_PATTERNS = [r'^field_width\.', r'^anchors\.pattern\.', r'^bar\.(bottom|top)\.(core_seq|buttons_seq|first_button|last_button|buttons_count)$',
                  r'^bar\.bottom\.button_name$', r'^bar\.(bottom|top)\.button_caption$', r'^table\.bar_seq$', r'^table\.col_width\.',
                  r'^form\.size$', r'^list\.', r'^field\.type$', r'^field\.height$', r'^row\.pitch$',
                  r'^table\.(change_row_set|enable_start_drag|row_selection_mode|initial_list_view)$', r'^bar\.bottom\.row_element_kind$',
                  r'^pair\.mark_given_fillchecking', r'^form\.version$', r'^form\.count$', r'^name\.(picture_prefix|panel_name)$']
TOPIC = {sid: title for sid, title, _ in SECTIONS}


def pct(x):
    return '%d %%' % round(x * 100)


def fv(v):
    if isinstance(v, bool):
        return 'да' if v else 'нет'
    if isinstance(v, float):
        return ('%.1f' % v).rstrip('0').rstrip('.')
    return str(v)


def cell(r, mode):
    if r is None:
        return '—'
    n = r['n']
    if mode == 'bool' or r['type'] == 'boolean' and mode != 'val':
        return '%s (n=%d)' % (pct(r.get('share_true', 0)), n)
    if mode == 'q' and r.get('quantiles'):
        q = r['quantiles']
        return '%s · %s/%s/%s (%s, n=%d)' % (fv(r['value']), fv(q['p10']), fv(q['p50']), fv(q['p90']), pct(r['share']), n)
    if mode == 'med' and r.get('quantiles'):
        return '%s (мода %s, %s; n=%d)' % (fv(r['quantiles']['p50']), fv(r['value']), pct(r['share']), r['n'])
    if mode == 'top2':
        return '; '.join('%s %s' % (fv(d['value']), pct(d['share'])) for d in r['distribution'][:2]) + ' (n=%d)' % n
    if r['type'] == 'boolean':
        return '%s %s (n=%d)' % (fv(r['value']), pct(r['share']), n)
    return '%s (%s, n=%d)' % (fv(r['value']), pct(r['share']), n)


def trim(s, k=44):
    s = str(s)
    return s if len(s) <= k else s[:k - 1] + '…'


def variants(r, k=3):
    return '; '.join('%s %s' % (trim(fv(d['value'])), pct(d['share'])) for d in r['distribution'][:k])


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--rules', default='target/design/design-rules.json')
    ap.add_argument('--md', default='docs/form-design-standards.md')
    ap.add_argument('--json', default='docs/design/design-rules.json')
    ap.add_argument('--min-n', type=int, default=20, help='минимум наблюдений для строки матрицы/расхождений')
    a = ap.parse_args()
    data = json.load(open(a.rules, encoding='utf-8'))
    meta, rules = data['meta'], data['rules']
    thr = meta['threshold']
    by = defaultdict(dict)
    for r in rules:
        by[r['id']][r['kind']] = r
    corps = meta['corpora']
    spec_ids = {rid for _, _, rows in SECTIONS for rid, _, _ in rows}
    rid_topic = {rid: sid for sid, _, rows in SECTIONS for rid, _, _ in rows}
    pats = [re.compile(p) for p in EXTRA_PATTERNS]
    kinds_keep = set(MAIN) | {'all', 'common_form', 'register_list', 'journal_list'}
    keep = [r for r in rules if r['kind'] in kinds_keep and (r['id'] in spec_ids or any(p.search(r['id']) for p in pats))]

    # ---- JSON
    out = []
    for r in keep:
        e = {'id': r['id'], 'kind': r['kind'], 'topic': TOPIC.get(rid_topic.get(r['id']), 'прочее'), 'parameter': r['parameter'],
             'unit': r['unit'], 'type': r['type'], 'value': r['value'], 'share': r['share'], 'n': r['n'], 'follows': r['follows'],
             'distribution': r['distribution'][:4]}
        if 'share_true' in r:
            e['share_true'] = r['share_true']
        if 'quantiles' in r:
            e['quantiles'] = {k: (round(v, 1) if isinstance(v, float) else v) for k, v in r['quantiles'].items()}
        e['by_corpus'] = {c: {'n': v['n'], 'value': v['value'], 'share': v['share']} for c, v in r['by_corpus'].items()}
        out.append(e)
    os.makedirs(os.path.dirname(a.json) or '.', exist_ok=True)
    with open(a.json, 'w', encoding='utf-8') as f:
        json.dump({'meta': {'source': ' + '.join(corps), 'forms': meta['forms_total'], 'follow_threshold': thr,
                            'kinds': {k: v['n'] for k, v in meta['kinds'].items()},
                            'note': 'share = доля наблюдений с модальным значением (для boolean — также share_true); n — число наблюдений (форм или элементов, см. unit)'},
                   'rules': out}, f, ensure_ascii=False, separators=(',', ':'))

    # ---- Markdown
    L = []
    P = L.append
    P('# Стандарты дизайна обычных форм')
    P('')
    P('Правила выведены из двух реальных конфигураций: **%s** (формы обычного приложения). Числа посчитаны скриптами '
      '`scripts/design/analyze_forms.py` и `scripts/design/render_standards.py`; машинная версия — '
      '`docs/design/design-rules.json` (id правила, вид формы, параметр, значение, доля, n). Файл генерируется, '
      'руками не правится.' % ' и '.join(corps))
    P('')
    P('Как читать: ячейка «`8 (96 %%, n=162)`» — модальное значение, доля наблюдений с ним, число наблюдений n '
      '(форм или элементов — по смыслу строки; доли считаются только по формам/элементам, где объект есть). '
      '«да (…)» — доля `да`. «`p10/p50/p90`» — квантили. Порог «стиль един» — %d %%; всё, что ниже, '
      'вынесено в раздел «Где стили расходятся». Единицы — пиксели обычной формы.' % round(thr * 100))
    P('')
    P('Формы: всего %d (заготовки без полей/таблиц/панелей не участвуют в геометрии).' % meta['forms_total'])
    P('')
    P('| вид | что входит | форм | ' + ' | '.join(corps) + ' |')
    P('|---|---|---|' + '---|' * len(corps))
    for k, v in meta['kinds'].items():
        P('| %s | %s | %d | %s |' % (KIND_RU[k], KIND_NOTE[k], v['n'], ' | '.join(str(v['by_corpus'].get(c, 0)) for c in corps)))
    P('')

    def uniform(rid, kinds, minn):
        """Правило едино для всех видов: (значение, min доля, n) или None."""
        rs = [by.get(rid, {}).get(k) for k in kinds]
        rs = [r for r in rs if r and r['n'] >= minn]
        if len(rs) < 3:
            return None
        if all(r['type'] == 'boolean' for r in rs):
            t = [r['share_true'] for r in rs]
            if min(t) >= .95:
                return ('да', min(t), sum(r['n'] for r in rs))
            if max(t) <= .05:
                return ('нет', 1 - max(t), sum(r['n'] for r in rs))
            return None
        if len({fv(r['value']) for r in rs}) == 1 and min(r['share'] for r in rs) >= .95:
            return (fv(rs[0]['value']), min(r['share'] for r in rs), sum(r['n'] for r in rs))
        return None

    def matrix(rows, kinds, minn, fold=True):
        P('| правило | ' + ' | '.join(KIND_RU[k] for k in kinds) + ' |')
        P('|---|' + '---|' * len(kinds))
        folded = []
        for rid, label, mode in rows:
            u = uniform(rid, kinds, minn) if fold else None
            if u:
                folded.append((label, u))
                continue
            rr = by.get(rid, {})
            cells = []
            for k in kinds:
                r = rr.get(k)
                cells.append(cell(r if r and r['n'] >= minn else None, mode))
            if all(c == '—' for c in cells):
                continue
            P('| %s | %s |' % (label, ' | '.join(cells)))
        if folded:
            P('')
            P('Едино для всех видов (≥ 95 % в каждом виде): ' + '; '.join(
                '%s — %s (%s, n=%d)' % (lb, u[0], '≥' + pct(u[1]), u[2]) for lb, u in folded) + '.')

    # --- 0. рекомендуемые значения
    P('## 0. Рекомендуемые значения по умолчанию для новой формы')
    P('')
    P('Мода по корпусам (если доля < %d %% — значение «условное», см. раздел расхождений). Источник для навыка '
      '`oneform-form-design` и `new-form`.' % round(thr * 100))
    P('')
    DEF = [('form.width', 'ширина формы: медиана', 'med'), ('form.height', 'высота формы: медиана', 'med'),
           ('bar.bottom.has', 'нижняя панель `ОсновныеДействияФормы`', 'bool'), ('bar.top.has', 'верхняя панель `ДействияФормы`', 'bool'),
           ('bar.top.autofill', 'AutoFill верхней панели', 'val'),
           ('margin.left', 'левое/правое поле', 'val'), ('margin.top_gap', 'зазор под верхней панелью', 'val'),
           ('margin.bottom_gap', 'зазор над нижней панелью', 'val'), ('row.pitch_form_mode', 'шаг строк', 'val'),
           ('pair.field_height', 'высота поля', 'val'), ('pair.label_height', 'высота подписи', 'val'),
           ('pair.gap', 'зазор подпись→поле', 'val'), ('pair.label_left', 'Left подписи', 'val'),
           ('pair.label_width', 'ширина подписи', 'val'), ('anchors.input_h_stretch', 'поле растягивается по ширине', 'bool'),
           ('anchors.table_stretch_both', 'таблица растягивается', 'bool'), ('form.has_panel', 'есть панель вкладок', 'bool')]
    matrix(DEF, MAIN, 5, fold=False)
    P('')
    P('Состав нижней панели по видам (стандартные кнопки по порядку, три самых частых набора):')
    P('')
    for k in MAIN:
        r = by.get('bar.bottom.core_seq', {}).get(k)
        if r and r['n'] >= 5:
            P('- %s (n=%d): %s' % (KIND_RU[k], r['n'], variants(r, 3)))
    P('')
    P('Остальное оформление по умолчанию (медиана/мода по всем формам): высота кнопки панели 25, панели таблицы 24; '
      'подпись оканчивается «:» (%s); имя подписи `Надпись<ИмяПоля>`; имя поля = имя реквизита; заголовки рамок '
      'жирным шрифтом; панель таблицы называется `КоманднаяПанель<ИмяТаблицы>`.' %
      pct(by['pair.label_colon']['all']['share_true']))
    P('')

    for sid, title, rows in SECTIONS:
        P('## ' + title)
        P('')
        matrix(rows, MAIN, a.min_n)
        P('')
        if sid == 'size':
            P('Самые частые размеры целиком (Ш×В), все формы / по корпусам:')
            P('')
            for k in ['catalog_item', 'catalog_list', 'document', 'processor', 'report']:
                r = by['form.size'].get(k)
                if r:
                    P('- %s: %s' % (KIND_RU[k], variants(r, 4)))
            P('')
        if sid == 'bars':
            P('Нижняя панель — стандартные кнопки по порядку (разделители `|` не показаны):')
            P('')
            for k in MAIN:
                r = by['bar.bottom.buttons_seq'].get(k)
                if r and r['n'] >= 5:
                    P('- %s: %s' % (KIND_RU[k], variants(r, 3)))
            P('')
            P('Верхняя панель `ДействияФормы` в большинстве форм — стандартная (`AutoFill=да`, кнопки подставляет платформа); у документов — «Параметры/Действия/Перейти…», у выбора — «Выбрать». Полный список: `bar.top.buttons_seq`, `bar.top.first_button` в JSON.')
            P('')
        if sid == 'pairs':
            P('Ширины полей по типу значения (пиксели; мода · p10/p50/p90; по формам и страницам панелей):')
            P('')
            P('| тип | все формы | ' + ' | '.join(corps) + ' |')
            P('|---|---|' + '---|' * len(corps))
            for rid in sorted(i for i in by if i.startswith('field_width.')):
                r = by[rid].get('all')
                if not r or r['n'] < a.min_n:
                    continue
                q = r['quantiles']
                cs = []
                for c in corps:
                    bc = r['by_corpus'].get(c)
                    cs.append('%s (n=%d)' % (fv(bc['value']), bc['n']) if bc else '—')
                P('| %s | %s · %s/%s/%s (n=%d) | %s |' % (rid.split('.', 1)[1], fv(r['value']), fv(q['p10']), fv(q['p50']), fv(q['p90']), r['n'], ' | '.join(cs)))
            P('')
        if sid == 'anchors':
            P('Типичные привязки пяти самых частых видов элементов (гориз/верт; «растяг» — край следует за правым/нижним краем формы, «фикс» — размер постоянный, «к дальнему краю» — элемент прижат к правому/нижнему краю):')
            P('')
            tags = sorted((i for i in by if i.startswith('anchors.pattern.')), key=lambda i: -by[i].get('all', {'n': 0})['n'])[:6]
            for rid in tags:
                r = by[rid]['all']
                P('- %s (n=%d): %s' % (rid.split('.')[-1], r['n'], variants(r, 4)))
            P('')
        if sid == 'tables':
            P('Списки справочников: число таблиц на форме по иерархии (`Hierarchical` из XML объекта):')
            P('')
            for rid in sorted(i for i in by if i.startswith('list.tables_count')):
                for k in ('catalog_list', 'catalog_choice'):
                    r = by[rid].get(k)
                    if r:
                        P('- %s, %s (n=%d): %s' % (rid.split('.')[-1], KIND_RU[k], r['n'], variants(r, 3)))
            P('')
            P('Типичный состав панели таблицы (три частых набора, все формы): %s.' % variants(by['table.bar_seq']['all'], 3))
            P('')
        if sid == 'names':
            P('Имена командных панелей уровня формы — доли по видам см. строку «имена панелей». Имена кнопок нижней панели (общая часть `ОсновныеДействияФормы…`): %s.' % variants(by['bar.bottom.button_name']['all'], 6))
            P('')

    # --- расхождения
    P('## Где стили расходятся')
    P('')
    P('Правила из разделов выше, где модальное значение встречается реже %d %% (n ≥ %d). «Корпуса» — моды по '
      'корпусам, если они различаются и каждый корпус внутри себя един (≥ %d %%): такое расхождение — выбор '
      'человека между стилями УТ и Бух. Для распределений величин (ширина поля, число колонок, высота формы) '
      'разброс естественен; они даны с квантилями в разделах выше и здесь не повторяются.' % (round(thr * 100), a.min_n, round(thr * 100)))
    P('')
    skip_spread = {'form.width', 'form.height', 'pair.label_width', 'pair.field_left', 'panel.top_gap', 'panel.bottom_gap',
                   'table.columns_count', 'button.width', 'panel.pages', 'table.bottom_gap', 'table.height_ratio', 'row.pitch',
                   'form.stub', 'lint.overlap', 'lint.outside_form', 'bar.other.has', 'panel.left', 'panel.right_margin', 'table.left',
                   'table.right_margin', 'page.left_margin', 'page.right_margin', 'page.top_first', 'page.bottom_gap', 'page.table_bottom_gap'}
    div_n = 0
    for sid, title, rows in SECTIONS:
        for rid, label, mode in rows:
            if rid in skip_spread:
                continue
            parts = []
            for k in MAIN:
                r = by.get(rid, {}).get(k)
                if not r or r['n'] < a.min_n:
                    continue
                top = r.get('share_true') if r['type'] == 'boolean' else r['share']
                if (top >= thr or top <= 1 - thr) if r['type'] == 'boolean' else top >= thr:
                    continue
                bc = {c: e for c, e in r['by_corpus'].items() if e['n'] >= 10}
                tag = ''
                if len(bc) >= 2:
                    if r['type'] == 'boolean':
                        tt = {c: e.get('share_true', 0) for c, e in bc.items()}
                        if all(v >= thr or v <= 1 - thr for v in tt.values()) and len({v >= .5 for v in tt.values()}) > 1:
                            tag = ' [корпуса: ' + ', '.join('%s %s' % (c, pct(v)) for c, v in tt.items()) + ']'
                    elif all(e['share'] >= thr for e in bc.values()) and len({fv(e['value']) for e in bc.values()}) > 1:
                        tag = ' [корпуса: ' + ', '.join('%s %s' % (c, trim(fv(e['value']), 24)) for c, e in bc.items()) + ']'
                var = ('да %s' % pct(r['share_true'])) if r['type'] == 'boolean' else variants(r, 3)
                parts.append('%s: %s%s' % (KIND_RU[k], var, tag))
            if parts:
                P('- **%s** — %s' % (label, ' · '.join(parts)))
                div_n += 1
    P('')
    P('Всего правил с расхождениями: %d.' % div_n)
    P('')

    # --- различия корпусов
    P('## УТ и Бух.: что они делают по-разному')
    P('')
    P('Ключевые параметры по корпусам для двух главных видов форм (остальные виды — в JSON, поле `by_corpus`); мода и её доля внутри корпуса. Если значения разные — это два стиля, выбор за человеком; разница в одну-две единицы пикселей отражает разные эпохи/авторов.')
    P('')
    KEYROWS = [('pair.gap', 'зазор подпись→поле'), ('row.pitch_form_mode', 'шаг строк'), ('pair.label_width', 'ширина подписи'),
               ('pair.label_left', 'Left подписи'), ('pair.label_bold', 'подпись жирная'), ('pair.label_colon', 'подпись с «:»'),
               ('pair.label_name_matches', 'имя подписи `Надпись`+имя поля'), ('bar.bottom.core_seq', 'кнопки нижней панели'),
               ('bar.bottom.ok_spelling', 'написание OK'), ('bar.bottom.shares_row_with_element', 'в строке нижней панели есть подпись'),
               ('bar.top.buttons_seq', 'набор верхней панели'), ('bar.top.autofill', 'AutoFill верхней панели'),
               ('table.first_col_header', 'заголовок первой колонки'), ('table.has_own_bar', 'у таблицы своя панель'),
               ('table.bar_autofill', 'AutoFill панели таблицы'), ('panel.show_tabs', 'ShowTabs панели'),
               ('name.field_eq_attr', 'имя поля = имя реквизита'), ('name.table_bar', 'имя панели таблицы `КоманднаяПанель`+имя'),
               ('form.width', 'ширина формы'), ('margin.bottom_gap', 'зазор над нижней панелью'),
               ('tab.form.reading_order_exact', 'обход = чтение'), ('tab.form.contiguous_from_1', 'TabOrder 1..N')]
    for k in ('catalog_item', 'document'):
        P('**%s**' % KIND_RU[k])
        P('')
        P('| параметр | ' + ' | '.join(corps) + ' |')
        P('|---|' + '---|' * len(corps))
        for rid, label in KEYROWS:
            r = by.get(rid, {}).get(k)
            if not r:
                continue
            bc = r['by_corpus']
            if len(bc) < 2 or any(e['n'] < 10 for e in bc.values()):
                continue
            def one(e):
                if r['type'] == 'boolean':
                    return '%s (n=%d)' % (pct(e.get('share_true', 0)), e['n'])
                return '%s (%s, n=%d)' % (trim(fv(e['value']), 40), pct(e['share']), e['n'])
            P('| %s | %s |' % (label, ' | '.join(one(bc[c]) if c in bc else '—' for c in corps)))
        P('')

    P('## Что не измерено')
    P('')
    P('- Шрифт заголовков таблицы, шрифт колонок и цвета строк из бинарной записи `Appearance` не разбираются; в XML есть только явные `HeaderFont`/`Font`/`TextColor`/`BackColor` — их доли даны, остальное неизвестно.')
    P('- Реальные размеры шрифта и ширина текста в пикселях: нет данных, соотношение «длина подписи → ширина подписи» не выведено.')
    P('- Взаимное положение контролов внутри группы (`UsualGroup`) — в корпусах группа это рамка-заголовок без дочерних элементов, вложенной геометрии нет.')
    P('- Отношение «рамка — её содержимое» (элементы внутри рамки `UsualGroup` по геометрии) отдельным правилом не оформлено.')
    P('- Связь жирной подписи с обязательностью: `FillChecking=ShowError` в XML объектов у реквизитов, вынесенных на формы, редок (у документов n=12), поэтому вывод по нему статистически слаб.')
    P('- Модульные имена обработчиков, подсказки (`Tooltip`), события, мобильные формы, управляемые формы (корпусы — только обычные формы).')
    P('- Колонка «все» в JSON объединяет два корпуса пропорционально числу наблюдений; УТ даёт больше форм документов, Бух. — отчётов и обработок.')
    P('- Автоматически созданные заготовки (форма обработки с панелью, одной надписью и нижней панелью — 144 формы Бух., 4 формы УТ) исключены из геометрии (`form.stub`).')
    with open(a.md, 'w', encoding='utf-8') as f:
        f.write('\n'.join(L) + '\n')
    print('md lines: %d, json rules: %d, divergent rules: %d' % (len(L), len(out), div_n))


if __name__ == '__main__':
    main()
