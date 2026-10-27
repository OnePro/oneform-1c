#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""F1: статистика дизайна обычных форм по распакованным корпусам.

Читает `<корпус>/**/src/Forms/<Форма>/Form.xml` (результат `oneform extract`, ordinaryFormVersion 2.1/2.2),
определяет вид формы и считает долю форм/элементов, подчиняющихся каждому правилу оформления
(размеры, командные панели, отступы, шаг строк, «подпись — поле», привязки, порядок обхода,
панели, таблицы, имена, подписи, шрифты). Корпус только читается.

Запуск (из корня репозитория):

    python scripts/design/analyze_forms.py --out target/design  "УТ 10.3=target/corpus/ut2"  "Бух.=<каталог корпуса Бух.>"

Аргументы `МЕТКА=каталог` (метка попадает в отчёты; без `МЕТКА=` берётся имя каталога).
Результат в `--out`:
  * `design-rules.json` — машинные правила: список `rules` (id, kind, parameter, unit, value, share, n,
    распределение, квантили, разбивка по корпусам) и `meta` (число форм по видам);
  * `tables.md` — те же числа таблицами (вход для `docs/form-design-standards.md`).
Опции: `--limit N` — взять не больше N форм с корпуса (для отладки); `--threshold 0.8` — порог «стиль един».
Только стандартная библиотека; на ~3000 форм работает около минуты.
"""
import argparse
import glob
import json
import os
import re
import time
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict

KINDS = ['catalog_item', 'catalog_group', 'catalog_list', 'catalog_choice', 'document', 'document_list',
         'document_choice', 'processor', 'report', 'common_form', 'register_list', 'journal_list', 'other']
KIND_RU = {
    'catalog_item': 'элемент справочника', 'catalog_group': 'группа справочника', 'catalog_list': 'список справочника',
    'catalog_choice': 'выбор из справочника', 'document': 'документ', 'document_list': 'список документов',
    'document_choice': 'выбор документа', 'processor': 'обработка', 'report': 'отчёт', 'common_form': 'общая форма',
    'register_list': 'список регистра', 'journal_list': 'журнал документов', 'other': 'прочие', 'all': 'все формы'}
STD_BUTTONS = {'OK', 'ОК', 'Записать', 'Провести', 'Закрыть', 'Отмена', 'Справка', 'Выполнить', 'Выбрать', 'Применить', 'Сформировать'}
FIELD_TAGS = ('InputField', 'ChoiceField')
ROW_TAGS = ('InputField', 'ChoiceField', 'CheckBox')
PLAIN_TAGS = ('InputField', 'ChoiceField', 'CheckBox', 'LabelDecoration', 'Button', 'RadioButton')
AXIS = {'h': ('left', 'right', 'L', 'R'), 'v': ('top', 'bottom', 'T', 'B')}
EDGE_AXIS = {'Left': 'h', 'Right': 'h', 'Top': 'v', 'Bottom': 'v'}


# ----------------------------------------------------------------------------- накопитель
class Acc:
    def __init__(self):
        self.d = defaultdict(lambda: defaultdict(Counter))  # (rule, kind) -> corpus -> Counter
        self.meta = {}  # rule -> (parameter, unit)

    def rec(self, rule, kind, corp, val, desc=None, unit='элемент'):
        if rule not in self.meta:
            self.meta[rule] = (desc or rule, unit)
        self.d[(rule, kind)][corp][val] += 1
        self.d[(rule, 'all')][corp][val] += 1


def num(x):
    if x is None:
        return None
    t = x.text if hasattr(x, 'text') else x
    try:
        return int(str(t).strip())
    except (TypeError, ValueError):
        try:
            return int(float(str(t).strip()))
        except (TypeError, ValueError):
            return None


def cap_of(x, tag='Caption'):
    c = x.find(tag)
    if c is None:
        return ''
    items = c.findall('Item')
    if not items:
        return (c.text or '').strip()
    for it in items:
        if it.get('language') == 'ru':
            return (it.text or '').strip()
    return (items[0].text or '').strip()


def camel_prefix(name):
    m = re.match(r'[А-ЯЁA-Z][а-яёa-z0-9]*', name or '')
    return m.group(0) if m else '(иное)'


# ----------------------------------------------------------------------------- разбор формы
def classify(family, fname, main):
    f, m = fname or '', main or ''
    if family in ('Catalogs',):
        if f.startswith('ФормаВыбора'):
            return 'catalog_choice'
        if f == 'ФормаСписка':
            return 'catalog_list'
        if f == 'ФормаГруппы':
            return 'catalog_group'
        if f == 'ФормаЭлемента':
            return 'catalog_item'
        if 'Список' in m:
            return 'catalog_list'
        if m.startswith('СправочникОбъект'):
            return 'catalog_item'
        return 'other'
    if family == 'Documents':
        if f.startswith('ФормаВыбора'):
            return 'document_choice'
        if f == 'ФормаСписка' or m.startswith('ДокументСписок'):
            return 'document_list'
        if f == 'ФормаДокумента' or m.startswith('ДокументОбъект'):
            return 'document'
        return 'other'
    if family == 'DataProcessors':
        return 'processor'
    if family == 'Reports':
        return 'report'
    if family == 'CommonForms':
        return 'common_form'
    if family in ('InformationRegisters', 'AccumulationRegisters', 'AccountingRegisters'):
        return 'register_list' if 'Список' in m or f == 'ФормаСписка' else 'other'
    if family == 'DocumentJournals':
        return 'journal_list'
    return 'other'


def mk_el(x, container):
    a = x.find('Anchors')
    f = x.find('Font')
    return {
        'tag': x.tag, 'name': x.get('name') or '', 'id': x.get('id'), 'x': x, 'cont': container,
        'L': num(x.find('Left')), 'T': num(x.find('Top')), 'W': num(x.find('Width')), 'H': num(x.find('Height')),
        'Z': num(x.find('ZOrder')), 'Tab': num(x.find('TabOrder')),
        'anc': dict(a.attrib) if a is not None else None,
        'bold': f is not None and f.get('bold') == 'true',
        'font': f is not None,
    }


def collect(ci, container, containers, idmap):
    """Рекурсивно собирает элементы: форма -> Panel -> Page -> элементы."""
    for x in ci:
        if x.get('name') is None and x.tag != 'Page':
            continue
        if x.tag == 'Column':
            continue
        e = mk_el(x, container)
        if x.tag == 'Page':
            e['name'] = x.get('name') or ''
        container['els'].append(e)
        if e['id'] is not None:
            idmap[e['id']] = e
        sub = x.find('ChildItems')
        if sub is None or x.tag in ('Button', 'CommandBar'):
            continue
        if x.tag == 'Panel':
            collect(sub, container, containers, idmap)  # страницы лежат в списке контейнера как Page
        elif x.tag == 'Page':
            pc = {'kind': 'page', 'W': e['W'], 'H': e['H'], 'els': [], 'page': e}
            containers.append(pc)
            collect(sub, pc, containers, idmap)
        # вложенность прочих элементов (группы) в корпусе не встречается: пропускаем


def collect_flat(ci):
    out = []
    for x in ci.iter():
        if x.get('name') is not None and x.tag not in ('Column', 'Button'):
            out.append({'tag': x.tag})
    return out


def edge_ref(el, edge, idmap, depth=0):
    """К какому краю формы/контейнера «привязан» край элемента ('L'/'R' или 'T'/'B')."""
    axis = 'h' if edge in ('left', 'right') else 'v'
    lo_e, hi_e, lo, hi = AXIS[axis]
    a = el['anc'] or {}
    v = a.get(edge)
    if depth > 8:
        return lo
    if v is None:
        if edge == lo_e:
            return lo
        return edge_ref(el, lo_e, idmap, depth + 1)
    t = a.get(edge + 'Target')
    lo_name = 'Left' if axis == 'h' else 'Top'
    if t is None:
        return lo if v == lo_name else hi
    tgt = el if t == el['id'] else idmap.get(t)
    if tgt is None or EDGE_AXIS.get(v) != axis:
        return lo if v == lo_name else hi
    return edge_ref(tgt, v.lower(), idmap, depth + 1)


def anchor_class(el, idmap):
    if el['anc'] is None:
        return 'нет/нет'
    out = []
    for ax in ('h', 'v'):
        lo_e, hi_e, lo, hi = AXIS[ax]
        p = (edge_ref(el, lo_e, idmap), edge_ref(el, hi_e, idmap))
        out.append({(lo, lo): 'фикс', (lo, hi): 'растяг', (hi, hi): 'к дальнему краю'}.get(p, 'иное'))
    return '/'.join(out)


def modal(vals):
    c = Counter(vals)
    return c.most_common(1)[0][0] if c else None


def cluster_tops(tops, tol=3):
    out = []
    for t in sorted(set(tops)):
        if out and t - out[-1] <= tol:
            continue
        out.append(t)
    return out


def overlap(a, b):
    return (a['L'] < b['L'] + b['W'] and b['L'] < a['L'] + a['W'] and a['T'] < b['T'] + b['H']
            and b['T'] < a['T'] + a['H'])


def valid(e, *keys):
    return all(e[k] is not None for k in keys)


def type_bucket(vt):
    """Ключ типа реквизита поля для таблицы ширин."""
    if vt is None:
        return None
    k = vt.get('kind')
    if k == 'String':
        n = vt.get('length')
        try:
            n = int(n)
        except (TypeError, ValueError):
            n = 0
        if n == 0:
            return 'String.неограниченная'
        if n <= 10:
            return 'String.1-10'
        if n <= 25:
            return 'String.11-25'
        if n <= 50:
            return 'String.26-50'
        if n <= 100:
            return 'String.51-100'
        return 'String.>100'
    if k == 'Number':
        try:
            fr = int(vt.get('fractionDigits') or 0)
        except ValueError:
            fr = 0
        return 'Number.целое' if fr == 0 else 'Number.дробное'
    if k in ('Date', 'DateTime', 'Ref', 'Boolean', 'Time'):
        return k
    return None


def seq_of_bar(x):
    """Последовательность кнопок командной панели: подписи (разделитель = '|')."""
    b = x.find('Buttons')
    out, names = [], []
    if b is None:
        return out, names
    for bt in b.findall('Button'):
        if bt.get('kind') == 'Separator':
            out.append('|')
            names.append('|')
            continue
        c = cap_of(bt) or '<' + (bt.get('name') or '') + '>'
        out.append(c.replace('&', ''))
        names.append(bt.get('name') or '')
    return out, names


def analyze_form(path, corp, family, obj, acc, corp_dir):
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        acc.rec('parse.error', 'all', corp, True, 'формы, не разобранные парсером', 'форма')
        return None
    fname = root.get('name') or os.path.basename(os.path.dirname(path))
    attrs = {a.get('id'): a for a in root.iter('Attribute')}
    ma = root.find('MainAttribute')
    main = attrs[ma.get('attributeId')].get('name') if ma is not None and ma.get('attributeId') in attrs else ''
    kind = classify(family, fname, main)
    W, H = num(root.find('Width')), num(root.find('Height'))
    ci = root.find('ChildItems')
    R = lambda rule, val, desc, unit='элемент': acc.rec(rule, kind, corp, val, desc, unit)
    F = lambda rule, val, desc: acc.rec(rule, kind, corp, val, desc, 'форма')
    F('form.count', True, 'число форм (для знаменателей)')
    if W is None or H is None or ci is None:
        return kind
    stub = not any(e['tag'] not in ('CommandBar', 'LabelDecoration', 'PictureDecoration', 'UsualGroup', 'Page')
                   for e in collect_flat(ci))
    F('form.stub', stub, 'заготовка: на форме только панели команд, надписи, рамки и картинки (нет полей, таблиц, панелей)')
    if stub:
        return kind
    F('form.width', W, 'ширина формы, пикс')
    F('form.height', H, 'высота формы, пикс')
    F('form.size', '%dx%d' % (W, H), 'размер формы Ш×В')
    F('form.version', root.get('ordinaryFormVersion') or '?', 'версия формата обычной формы')

    info = obj_info(corp_dir, family, obj)
    form_c = {'kind': 'form', 'W': W, 'H': H, 'els': []}
    containers = [form_c]
    idmap = {}
    collect(ci, form_c, containers, idmap)
    els = form_c['els']
    fl = [e for e in els if e['tag'] != 'Page']
    F('form.has_panel', any(e['tag'] == 'Panel' for e in fl), 'на форме есть панель (вкладки)')

    if kind in ('catalog_list', 'catalog_choice') and info['hier'] is not None:
        tb = [e for e in fl if e['tag'] == 'Table']
        hk = 'иерархический' if info['hier'] else 'плоский'
        F('list.tables_count.' + hk, len(tb), 'число таблиц на форме списка/выбора (%s справочник)' % hk)
        F('list.tree_view.' + hk, any(e['x'].findtext('InitialTreeView') for e in tb), 'у таблицы задан InitialTreeView (%s справочник)' % hk)
    # ---- командные панели формы
    bars = [e for e in fl if e['tag'] == 'CommandBar']
    bottom = next((e for e in bars if e['name'] == 'ОсновныеДействияФормы'), None)
    topbar = next((e for e in bars if e['name'] == 'ДействияФормы'), None)
    F('bar.bottom.has', bottom is not None, 'есть нижняя панель ОсновныеДействияФормы')
    F('bar.top.has', topbar is not None, 'есть верхняя панель ДействияФормы')
    others = [e for e in bars if e is not bottom and e is not topbar]
    F('bar.other.has', bool(others), 'есть иные командные панели на уровне формы')
    for e in bars:
        if e['name'] in ('ОсновныеДействияФормы', 'ДействияФормы'):
            cls = e['name']
        elif e['name'].startswith('КоманднаяПанель'):
            cls = 'КоманднаяПанель…'
        else:
            cls = '(иное имя)'
        R('bar.name_class', cls, 'имена командных панелей уровня формы')
    for key, e, lab in (('bottom', bottom, 'нижняя ОсновныеДействияФормы'), ('top', topbar, 'верхняя ДействияФормы')):
        if e is None or not valid(e, 'L', 'T', 'W', 'H'):
            continue
        if key == 'bottom':
            F('bar.bottom.top_eq_h_minus_height', e['T'] == H - e['H'], 'нижняя панель: Top = высота формы − высота панели')
            F('bar.bottom.left0_full_width', e['L'] == 0 and e['W'] == W, 'нижняя панель: Left=0 и Width = ширина формы')
            F('bar.bottom.is_last_tab', e['Tab'] is not None and e['Tab'] == max(
                (x['Tab'] for x in fl if x['Tab'] is not None), default=-1), 'нижняя панель имеет наибольший TabOrder')
        else:
            F('bar.top.top0', e['T'] == 0, 'верхняя панель: Top = 0')
            F('bar.top.left0_full_width', e['L'] == 0 and e['W'] == W, 'верхняя панель: Left=0 и Width = ширина формы')
            F('bar.top.is_first_tab', e['Tab'] == 1, 'верхняя панель: TabOrder = 1')
            F('bar.top.is_first_z', e['Z'] is not None and e['Z'] == min(x['Z'] for x in fl if x['Z'] is not None),
              'верхняя панель: ZOrder минимален')
        F('bar.%s.height' % key, e['H'], 'высота панели %s, пикс' % lab)
        F('bar.%s.anchor_class' % key, anchor_class(e, idmap), 'привязки панели %s (гориз/верт)' % lab)
        x = e['x']
        F('bar.%s.autofill' % key, (x.findtext('AutoFill') or '?'), 'AutoFill панели %s' % lab)
        F('bar.%s.halign' % key, (x.findtext('HorizontalAlign') or 'не задано'), 'HorizontalAlign панели %s' % lab)
        seq, names = seq_of_bar(x)
        F('bar.%s.buttons_seq' % key, ' '.join(seq) if seq else '(пусто)', 'набор и порядок кнопок панели %s' % lab)
        real = [s for s in seq if s != '|']
        core = [s for s in real if s in STD_BUTTONS]
        F('bar.%s.core_seq' % key, ' '.join(core) if core else '(нет стандартных)', 'стандартные кнопки панели %s по порядку' % lab)
        okc = [s for s in real if s in ('OK', 'ОК')]
        if okc:
            F('bar.%s.ok_spelling' % key, 'латинские OK' if okc[0] == 'OK' else 'кириллица ОК', 'написание кнопки ОК в панели %s' % lab)
        F('bar.%s.buttons_count' % key, len(real), 'число кнопок панели %s' % lab)
        if real:
            F('bar.%s.first_button' % key, real[0], 'первая кнопка панели %s' % lab)
            F('bar.%s.last_button' % key, real[-1], 'последняя кнопка панели %s' % lab)
        F('bar.%s.has_separators' % key, '|' in seq, 'между кнопками панели %s есть разделители' % lab)
        for s, nm in zip(seq, names):
            if s != '|':
                F('bar.%s.button_caption' % key, s, 'подписи кнопок панели %s (доля форм, где кнопка есть — см. n)' % lab)
        if key == 'bottom':
            for nm in names:
                if nm != '|':
                    R('bar.bottom.button_name', nm.replace('ОсновныеДействияФормы', '…'),
                      'имена кнопок нижней панели (общий префикс ОсновныеДействияФормы заменён на «…»)')
    if bottom is not None and valid(bottom, 'T'):
        bt = bottom['T']
    else:
        bt = H
    topbottom = (topbar['T'] + topbar['H']) if topbar is not None and valid(topbar, 'T', 'H') else 0

    # ---- поля отступов уровня формы
    body = [e for e in fl if e['tag'] != 'CommandBar' and valid(e, 'L', 'T', 'W', 'H') and e['W'] > 0 and e['H'] > 0]
    if body:
        lm = min(e['L'] for e in body)
        rm = W - max(e['L'] + e['W'] for e in body)
        tm = min(e['T'] for e in body) - topbottom
        above = [e for e in body if bottom is None or e['T'] < bt]
        bm = bt - max(e['T'] + e['H'] for e in above) if above else None
        if bottom is not None:
            inrow = [e for e in body if e['T'] >= bt]
            F('bar.bottom.shares_row_with_element', bool(inrow), 'в строке нижней панели (Top ≥ Top панели) лежит иной элемент (напр. «автор»)')
            for e in inrow:
                R('bar.bottom.row_element_kind', e['tag'] + ':' + camel_prefix(e['name']), 'элемент в строке нижней панели', 'элемент')
        F('margin.left', lm, 'левое поле: минимальный Left элементов (кроме панелей команд)')
        F('margin.right', rm, 'правое поле: ширина формы − максимальный правый край')
        F('margin.top_gap', tm, 'зазор от нижнего края верхней панели (или формы) до первого элемента')
        if bm is not None:
            F('margin.bottom_gap', bm, 'зазор от нижнего края элементов до нижней панели (или края формы)')
        F('margin.symmetric', lm == rm, 'левое поле = правому')
        F('margin.left_eq_right_pm1', abs(lm - rm) <= 1, 'левое поле = правому (±1)')
    # ---- вне формы / перекрытия
    plain = [e for e in fl if e['tag'] in PLAIN_TAGS and valid(e, 'L', 'T', 'W', 'H') and e['W'] > 0 and e['H'] > 0]
    out = [e for e in body if e['L'] < 0 or e['T'] < 0 or e['L'] + e['W'] > W or e['T'] + e['H'] > H]
    F('lint.outside_form', bool(out), 'есть элемент за краем формы')
    ov = False
    for i in range(len(plain)):
        for j in range(i + 1, len(plain)):
            if overlap(plain[i], plain[j]):
                ov = True
                break
        if ov:
            break
    F('lint.overlap', ov, 'есть перекрытие простых элементов (поле, подпись, флажок, кнопка)')
    F('anchors.form_has_any', all(e['anc'] is not None for e in fl), 'у всех элементов формы есть Anchors')

    # ---- анализ по контейнерам
    for cont in containers:
        analyze_container(cont, kind, corp, acc, idmap, F, R, W, H, bt, topbottom, info)

    # ---- порядок обхода (по контейнерам)
    for cont in containers:
        tabs = [e for e in cont['els'] if e['tag'] != 'Page' and e['Tab'] is not None and valid(e, 'T', 'L')]
        if len(tabs) < 4:
            continue
        pre = 'tab.form' if cont['kind'] == 'form' else 'tab.page'
        vals = sorted(e['Tab'] for e in tabs)
        F(pre + '.contiguous_from_1', vals == list(range(1, len(vals) + 1)), 'TabOrder уровня %s идёт 1..N без пропусков' % cont['kind'])
        order = sorted(tabs, key=lambda e: (e['Tab'], e['Z'] or 0))
        bad = 0
        pairs = 0
        for a, b in zip(order, order[1:]):
            pairs += 1
            if b['T'] < a['T'] - 6:
                bad += 1
            elif abs(b['T'] - a['T']) <= 6 and b['L'] < a['L'] - 2:
                bad += 1
        frac = bad / pairs if pairs else 0
        F(pre + '.reading_order_exact', bad == 0, 'порядок обхода совпадает с чтением (сверху вниз, слева направо), %s' % cont['kind'])
        F(pre + '.reading_order_viol_bucket',
          '0' if bad == 0 else ('≤10%' if frac <= .10 else ('≤25%' if frac <= .25 else '>25%')),
          'доля нарушений порядка чтения среди соседних пар обхода (%s)' % cont['kind'])
        zs = [e['Z'] for e in tabs if e['Z'] is not None]
        if len(zs) == len(tabs):
            F(pre + '.z_perm', sorted(zs) == list(range(len(zs))), 'ZOrder — перестановка 0..N−1')
            zo = [e['Tab'] for e in sorted(tabs, key=lambda e: e['Z'])]
            F(pre + '.z_equals_tab', zo == sorted(zo), 'порядок ZOrder совпадает с порядком TabOrder')
        # метка и её поле: Tab поля = Tab метки + 1
    return kind


def find_label(f, labels, byname):
    """Подпись слева от поля: по имени Надпись<Имя>, иначе геометрически. -> (label, how)"""
    lab = byname.get('Надпись' + f['name'])
    if lab is not None and lab['tag'] == 'LabelDecoration' and valid(lab, 'L', 'T', 'W', 'H'):
        return lab, 'имя'
    best = None
    for l in labels:
        if not valid(l, 'L', 'T', 'W', 'H'):
            continue
        if l['L'] + l['W'] <= f['L'] + 2 and l['T'] < f['T'] + f['H'] and l['T'] + l['H'] > f['T'] \
                and f['L'] - (l['L'] + l['W']) <= 40:
            if best is None or l['L'] + l['W'] > best['L'] + best['W']:
                best = l
    return (best, 'геометрия') if best else (None, None)


def analyze_container(cont, kind, corp, acc, idmap, F, R, W, H, bt, topbottom, info):
    isf = cont['kind'] == 'form'
    pre = '' if isf else 'page.'
    els = [e for e in cont['els'] if e['tag'] != 'Page']
    CW, CH = cont['W'], cont['H']
    sfx = ' (форма)' if isf else ' (страница панели)'
    byname = {e['name']: e for e in els}
    fields = [e for e in els if e['tag'] in FIELD_TAGS and valid(e, 'L', 'T', 'W', 'H')]
    labels = [e for e in els if e['tag'] == 'LabelDecoration' and e['x'].find('Caption') is not None
              and e['x'].find('Hyperlink') is None and cap_of(e['x']) != '']
    # --- поля страницы
    if not isf and CW is not None and CH is not None:
        allp = [e for e in els if valid(e, 'L', 'T', 'W', 'H') and e['W'] > 0 and e['H'] > 0]
        if allp:
            F('page.left_margin', min(e['L'] for e in allp), 'страница: левое поле (min Left)')
            F('page.right_margin', CW - max(e['L'] + e['W'] for e in allp), 'страница: правое поле')
            F('page.top_first', min(e['T'] for e in allp), 'страница: Top первого элемента')
            F('page.bottom_gap', CH - max(e['T'] + e['H'] for e in allp), 'страница: зазор снизу до края страницы')
    # --- привязки
    for e in els:
        cls = anchor_class(e, idmap)
        R(pre + 'anchors.pattern.' + e['tag'], cls, 'привязки (гориз/верт) для %s%s' % (e['tag'], sfx))
    for f in fields:
        cls = anchor_class(f, idmap)
        stretch = cls.startswith('растяг')
        R(pre + 'anchors.input_h_stretch', stretch, 'поле ввода растягивается по ширине (правый край привязан к правому краю)' + sfx)
        if CW is not None:
            rmg = CW - (f['L'] + f['W'])
            R(pre + 'anchors.input_stretch_when_at_right_margin' if rmg <= 12 else pre + 'anchors.input_stretch_when_not_at_right',
              stretch, 'поле растягивается, если оно доходит до правого поля (≤12 пикс)' if rmg <= 12 else
              'поле растягивается, если оно НЕ доходит до правого поля', 'элемент')
    for t in [e for e in els if e['tag'] == 'Table']:
        cls = anchor_class(t, idmap)
        R(pre + 'anchors.table_stretch_both', cls.startswith('растяг/растяг'), 'таблица растягивается по обеим осям' + sfx)
    for t in [e for e in els if e['tag'] == 'LabelDecoration']:
        R(pre + 'anchors.label_fixed', anchor_class(t, idmap) in ('фикс/фикс', 'нет/нет'),
          'подпись не растягивается (фикс/фикс)' + sfx)
    # --- метка — поле
    pairs = []
    for f in fields:
        lab, how = find_label(f, labels, byname)
        if lab is not None:
            pairs.append((lab, f, how))
            R(pre + 'pair.how', how, 'как найдена подпись поля: имя Надпись<Имя> / геометрия' + sfx)
        else:
            above = [l for l in labels if valid(l, 'L', 'T', 'W', 'H') and l['T'] + l['H'] <= f['T'] + 2
                     and f['T'] - (l['T'] + l['H']) <= 12 and l['L'] < f['L'] + f['W'] and l['L'] + l['W'] > f['L']]
            R(pre + 'pair.position', 'слева' if lab else ('сверху' if above else 'нет подписи'),
              'положение подписи относительно поля' + sfx)
        if lab is not None:
            R(pre + 'pair.position', 'слева', 'положение подписи относительно поля' + sfx)
    for lab, f, how in pairs:
        R(pre + 'pair.gap', f['L'] - (lab['L'] + lab['W']), 'зазор подпись→поле (Left поля − правый край подписи)' + sfx)
        R(pre + 'pair.top_delta', lab['T'] - f['T'], 'Top подписи − Top поля' + sfx)
        R(pre + 'pair.label_height', lab['H'], 'высота подписи пары' + sfx)
        R(pre + 'pair.field_height', f['H'], 'высота поля пары' + sfx)
        R(pre + 'pair.label_width', lab['W'], 'ширина подписи пары' + sfx)
        R(pre + 'pair.label_left', lab['L'], 'Left подписи пары' + sfx)
        R(pre + 'pair.field_left', f['L'], 'Left поля пары' + sfx)
        cp = cap_of(lab['x'])
        R(pre + 'pair.label_colon', cp.rstrip().endswith(':'), 'подпись пары оканчивается на «:»' + sfx)
        R(pre + 'pair.label_upper', cp[:1].isupper(), 'подпись пары начинается с заглавной' + sfx)
        R(pre + 'pair.label_halign', lab['x'].findtext('HorizontalAlign') or 'не задано', 'HorizontalAlign подписи пары' + sfx)
        R(pre + 'pair.label_bold', lab['bold'], 'подпись пары набрана жирным' + sfx)
        am = f['x'].findtext('AutoMarkIncomplete')
        R(pre + 'pair.label_bold_given_incomplete_mark.' + ('mark' if am == 'true' else 'nomark'), lab['bold'],
          'подпись жирная, если у поля AutoMarkIncomplete=%s' % ('true' if am == 'true' else 'нет') + sfx)
        R(pre + 'pair.label_name_matches', lab['name'] == 'Надпись' + f['name'], 'имя подписи = «Надпись»+имя поля' + sfx)
        tail = (f['x'].findtext('DataPath') or '').split('.')[-1]
        if tail in info['fill']:
            req = info['fill'][tail] != 'DontCheck'
            R(pre + 'pair.label_bold_given_fillchecking.' + ('required' if req else 'optional'), lab['bold'],
              'подпись жирная, если реквизит объекта %s (FillChecking)' % ('обязателен' if req else 'не обязателен') + sfx)
            R(pre + 'pair.mark_given_fillchecking.' + ('required' if req else 'optional'), am == 'true',
              'у поля AutoMarkIncomplete=true, если реквизит объекта %s (FillChecking)' % ('обязателен' if req else 'не обязателен') + sfx)
        R(pre + 'pair.tab_label_plus1', (lab['Tab'] is not None and f['Tab'] is not None and f['Tab'] == lab['Tab'] + 1),
          'TabOrder поля = TabOrder подписи + 1' + sfx)
    # колонка подписей: одинаковая ширина
    if len(pairs) >= 3:
        ll = Counter(l['L'] for l, _, _ in pairs)
        ml = ll.most_common(1)[0][0]
        ws = [l['W'] for l, _, _ in pairs if l['L'] == ml]
        if len(ws) >= 3:
            F(pre + 'pair.label_column_same_width', Counter(ws).most_common(1)[0][1] / len(ws) >= 0.8,
              'подписи левой колонки (общий Left) ≥80%% одной ширины' + sfx)
    # колонка полей выровнена
    if len(pairs) >= 3:
        lefts = [f['L'] for _, f, _ in pairs if f['L'] > 0]
        c = Counter(lefts)
        mv, mc = c.most_common(1)[0]
        F(pre + 'pair.field_column_aligned', mc / len(lefts) >= 0.8, 'в форме ≥80% полей с подписями слева выровнены по общему Left' + sfx)
    # --- ширина поля по типу
    for f in fields:
        vt = f['x'].find('ValueType')
        b = type_bucket(vt)
        if b and f['W'] and f['W'] > 0:
            R(pre + 'field_width.' + b, f['W'], 'ширина поля типа %s' % b + sfx)
        if f['H']:
            R(pre + 'field.height', f['H'], 'высота поля ввода' + sfx)
        if vt is not None and b:
            R(pre + 'field.type', vt.get('kind'), 'типы полей ввода' + sfx)
        ch = [f['x'].findtext(k) == 'true' for k in ('ChoiceButton', 'ClearButton', 'OpenButton')]
        if vt is not None and vt.get('kind') == 'Ref':
            R(pre + 'field.ref_choice_button', ch[0], 'у поля ссылочного типа включена кнопка выбора' + sfx)
        R(pre + 'field.text_align', f['x'].findtext('HorizontalAlign') or 'не задано', 'HorizontalAlign поля ввода' + sfx)
        if f['x'].findtext('AutoMarkIncomplete') == 'true':
            R(pre + 'field.auto_mark_incomplete', True, 'поля с AutoMarkIncomplete=true' + sfx)
    # --- шаг строк
    rows = cluster_tops([e['T'] for e in els if e['tag'] in ROW_TAGS and e['T'] is not None], 3)
    diffs = [b - a for a, b in zip(rows, rows[1:]) if 10 <= b - a <= 60]
    for d in diffs:
        R(pre + 'row.pitch', d, 'шаг по вертикали между соседними строками полей' + sfx)
    if len(rows) >= 4 and diffs:
        m = modal(diffs)
        F(pre + 'row.pitch_form_mode', m, 'модальный шаг строк формы' + sfx)
        F(pre + 'row.pitch_uniform', sum(1 for d in diffs if d == m) / len(diffs) >= 0.8,
          '≥80% шагов строк формы одинаковы' + sfx)
    # --- горизонтальные зазоры в строке
    pl = sorted([e for e in els if e['tag'] in PLAIN_TAGS and valid(e, 'L', 'T', 'W', 'H') and e['W'] > 0], key=lambda e: e['L'])
    for i, a in enumerate(pl):
        best = None
        for b in pl[i + 1:]:
            if b['L'] >= a['L'] + a['W'] - 1 and b['T'] < a['T'] + a['H'] and b['T'] + b['H'] > a['T']:
                best = b
                break
        if best is not None and a['tag'] != 'LabelDecoration':
            R(pre + 'gap.h_after_nonlabel', best['L'] - (a['L'] + a['W']), 'зазор по горизонтали справа от поля/флажка/кнопки до соседа в строке' + sfx)
    # --- флажки и кнопки
    for e in els:
        if e['tag'] == 'CheckBox':
            cp = cap_of(e['x'])
            R(pre + 'check.height', e['H'], 'высота флажка' + sfx)
            R(pre + 'check.caption_colon', cp.rstrip().endswith(':'), 'подпись флажка оканчивается на «:»' + sfx)
            R(pre + 'check.caption_upper', cp[:1].isupper(), 'подпись флажка начинается с заглавной' + sfx)
            dp = e['x'].findtext('DataPath') or ''
            R(pre + 'name.check_eq_attr', e['name'] == dp.split('.')[-1], 'имя флажка = имя реквизита' + sfx)
        if e['tag'] == 'Button':
            R(pre + 'button.height', e['H'], 'высота самостоятельной кнопки' + sfx)
            R(pre + 'button.width', e['W'], 'ширина самостоятельной кнопки' + sfx)
            R(pre + 'name.button_prefix', camel_prefix(e['name']), 'префикс имени самостоятельной кнопки' + sfx)
            cp = cap_of(e['x'])
            R(pre + 'button.caption_ellipsis', cp.endswith('...') or cp.endswith('…'), 'подпись кнопки оканчивается многоточием' + sfx)
            R(pre + 'button.anchor', anchor_class(e, idmap), 'привязки самостоятельной кнопки' + sfx)
    # --- имена
    for f in fields:
        dp = f['x'].findtext('DataPath') or ''
        if dp:
            R(pre + 'name.field_eq_attr', f['name'] == dp.split('.')[-1], 'имя поля ввода = имя реквизита из DataPath' + sfx)
            R(pre + 'name.field_prefix_pole', f['name'].startswith('Поле'), 'имя поля ввода начинается с «Поле»' + sfx)
    for e in els:
        if e['tag'] == 'LabelDecoration':
            R(pre + 'name.label_prefix', camel_prefix(e['name']), 'префикс имени подписи' + sfx)
        elif e['tag'] == 'UsualGroup':
            R(pre + 'name.group_prefix', camel_prefix(e['name']), 'префикс имени рамки (группы)' + sfx)
        elif e['tag'] == 'Panel':
            R(pre + 'name.panel_name', e['name'] if e['name'] in ('ОсновнаяПанель', 'Панель') else '(иное)', 'имя панели' + sfx)
        elif e['tag'] == 'PictureDecoration':
            R(pre + 'name.picture_prefix', camel_prefix(e['name']), 'префикс имени картинки' + sfx)
    # --- шрифты и цвета
    for e in els:
        if e['tag'] in ('LabelDecoration', 'InputField', 'Button', 'Table', 'Panel', 'UsualGroup', 'CheckBox', 'CommandBar', 'ChoiceField'):
            R(pre + 'font.nondefault.' + e['tag'], e['font'], 'явно заданный шрифт у %s' % e['tag'] + sfx)
            R(pre + 'font.bold.' + e['tag'], e['bold'], 'жирный шрифт у %s' % e['tag'] + sfx)
            x = e['x']
            R(pre + 'color.text.' + e['tag'], x.find('TextColor') is not None, 'явный цвет текста у %s' % e['tag'] + sfx)
            R(pre + 'color.back.' + e['tag'], x.find('BackColor') is not None, 'явный цвет фона у %s' % e['tag'] + sfx)
            if e['font']:
                R(pre + 'font.face', x.find('Font').get('face') or 'по умолчанию', 'гарнитура явно заданных шрифтов' + sfx)
    for e in els:
        if e['tag'] == 'UsualGroup':
            R(pre + 'group.caption_bold', e['bold'], 'заголовок рамки жирный' + sfx)
            R(pre + 'group.height', e['H'], 'высота рамки-заголовка' + sfx)
            R(pre + 'group.border', e['x'].findtext('BorderStyle') or 'не задано', 'BorderStyle рамки' + sfx)
    # --- панели и страницы
    for p in [e for e in els if e['tag'] == 'Panel']:
        sub = p['x'].find('ChildItems')
        pages = sub.findall('Page') if sub is not None else []
        R(pre + 'panel.pages', len(pages), 'число страниц в панели' + sfx)
        R(pre + 'panel.show_tabs', p['x'].find('ShowTabs').get('member') if p['x'].find('ShowTabs') is not None else 'нет', 'ShowTabs панели' + sfx)
        R(pre + 'panel.anchor', anchor_class(p, idmap), 'привязки панели (вкладок)' + sfx)
        if valid(p, 'L', 'T', 'W', 'H') and CW is not None:
            R(pre + 'panel.left', p['L'], 'Left панели' + sfx)
            R(pre + 'panel.right_margin', CW - p['L'] - p['W'], 'правое поле панели' + sfx)
            if isf:
                R('panel.bottom_gap', bt - p['T'] - p['H'], 'зазор от низа панели до нижней панели команд (или края формы)' + sfx)
                R('panel.top_gap', p['T'] - topbottom, 'зазор от верхней панели команд до панели (вкладок)' + sfx)
        for pg in pages:
            nm = pg.get('name') or ''
            ti = cap_of(pg, 'Title')
            R('panel.page_name_default', bool(re.fullmatch(r'Страница\d+', nm)), 'имя страницы — «СтраницаN» (по умолчанию)' + sfx)
            R('panel.page_name_eq_title', nm == re.sub(r'[^0-9A-Za-zА-Яа-яЁё_]', '', ti), 'имя страницы = заголовок без пробелов/знаков' + sfx)
            R('panel.page_title_colon', ti.endswith(':'), 'заголовок страницы оканчивается на «:»' + sfx)
            R('panel.page_title_upper', ti[:1].isupper(), 'заголовок страницы с заглавной' + sfx)
            gl, gt, gw, gh = (num(pg.find(k)) for k in ('Left', 'Top', 'Width', 'Height'))
            if None not in (gl, gt, gw, gh) and valid(p, 'W', 'H'):
                R('page.geom', 'L%d T%d dW%d dH%d' % (gl, gt, p['W'] - gw, p['H'] - gh),
                  'геометрия страницы: Left, Top, (ширина панели − ширина страницы), (высота панели − высота страницы)', 'элемент')
    # --- таблицы
    tables = [e for e in els if e['tag'] == 'Table']
    for t in tables:
        x = t['x']
        pre_t = ''
        R(pre_t + 'table.footer', x.findtext('Footer') == 'true', 'у таблицы включён подвал (Footer)' + sfx)
        R(pre_t + 'table.choice_mode', x.findtext('ChoiceMode') == 'true', 'ChoiceMode=true' + sfx)
        R(pre_t + 'table.read_only', x.findtext('ReadOnly') == 'true', 'таблица только для чтения (ReadOnly)' + sfx)
        R(pre_t + 'table.header_font_bold', (x.find('HeaderFont') is not None and x.find('HeaderFont').get('bold') == 'true'),
          'заголовок таблицы жирным (HeaderFont bold)' + sfx)
        R(pre_t + 'table.horizontal_lines', x.findtext('HorizontalLines') or 'не задано', 'HorizontalLines таблицы' + sfx)
        R(pre_t + 'table.vertical_lines', x.findtext('VerticalLines') or 'не задано', 'VerticalLines таблицы' + sfx)
        R(pre_t + 'table.selection_mode', x.findtext('SelectionMode') or 'не задано', 'SelectionMode таблицы' + sfx)
        R('table.initial_tree_view', x.findtext('InitialTreeView') or 'не задано', 'InitialTreeView таблицы' + sfx)
        R('table.initial_list_view', x.findtext('InitialListView') or 'не задано', 'InitialListView таблицы' + sfx)
        R('table.change_row_set', x.findtext('ChangeRowSet') or 'не задано', 'ChangeRowSet таблицы (false = нельзя добавлять/удалять строки)' + sfx)
        R('table.enable_start_drag', x.findtext('EnableStartDrag') or 'не задано', 'EnableStartDrag таблицы' + sfx)
        R('table.row_selection_mode', x.findtext('RowSelectionMode') or 'не задано', 'RowSelectionMode таблицы' + sfx)
        dp = x.findtext('DataPath') or ''
        if dp:
            R(pre_t + 'name.table_eq_attr', t['name'] == dp.split('.')[-1], 'имя таблицы = имя реквизита/табличной части' + sfx)
        cols = x.find('Columns')
        cl = cols.findall('Column') if cols is not None else []
        R(pre_t + 'table.columns_count', len(cl), 'число колонок' + sfx)
        for c in cl:
            h = cap_of(c, 'Header')
            w = num(c.find('Width'))
            if h:
                R('table.col_header_colon', h.rstrip().endswith(':'), 'заголовок колонки оканчивается на «:»', 'элемент')
                R('table.col_header_upper', h[:1].isupper(), 'заголовок колонки с заглавной', 'элемент')
            if w:
                vtc = type_bucket(c.find('ValueType'))
                if vtc:
                    R('table.col_width.' + vtc, w, 'ширина колонки таблицы типа %s' % vtc, 'элемент')
            R('table.col_readonly', c.findtext('ReadOnly') == 'true', 'колонка только для чтения', 'элемент')
            f = c.find('Font')
            R('table.col_font_bold', f is not None and f.get('bold') == 'true', 'колонка жирным шрифтом', 'элемент')
        if cl:
            R('table.first_col_header', cap_of(cl[0], 'Header') or '(пусто)', 'заголовок первой колонки', 'элемент')
        # геометрия
        if valid(t, 'L', 'T', 'W', 'H') and CW is not None and CH is not None:
            R(pre + 'table.left', t['L'], 'Left таблицы' + sfx)
            R(pre + 'table.right_margin', CW - t['L'] - t['W'], 'правое поле таблицы' + sfx)
            if isf:
                R('table.height_ratio', round((t['H'] / H) * 10) / 10, 'высота таблицы / высота формы (округл. 0.1) — для таблиц формы', 'элемент')
                R('table.bottom_gap', bt - t['T'] - t['H'], 'зазор от низа таблицы до нижней панели (или края формы)', 'элемент')
            else:
                R('page.table_bottom_gap', CH - t['T'] - t['H'], 'зазор от низа таблицы до края страницы', 'элемент')
        # своя командная панель
        bar = next((b for b in els if b['tag'] == 'CommandBar' and b['x'].findtext('ActionSource') == t['id']), None)
        R(pre_t + 'table.has_own_bar', bar is not None, 'у таблицы есть своя командная панель (ActionSource)' + sfx)
        if bar is not None and valid(bar, 'L', 'T', 'W', 'H') and valid(t, 'L', 'T', 'W', 'H'):
            R('table.bar_above', bar['T'] + bar['H'] <= t['T'] + 1, 'панель таблицы над таблицей', 'элемент')
            R('table.bar_flush', bar['L'] == t['L'] and bar['W'] == t['W'] and bar['T'] + bar['H'] == t['T'],
              'панель таблицы вплотную над таблицей, той же ширины', 'элемент')
            R('table.bar_height', bar['H'], 'высота командной панели таблицы', 'элемент')
            R('table.bar_autofill', bar['x'].findtext('AutoFill') or '?', 'AutoFill панели таблицы', 'элемент')
            R('table.bar_anchor', anchor_class(bar, idmap), 'привязки панели таблицы', 'элемент')
            seq, _ = seq_of_bar(bar['x'])
            R('table.bar_seq', ' '.join(seq) if seq else '(пусто)', 'набор кнопок панели таблицы', 'элемент')
            R('name.table_bar', bar['name'] == 'КоманднаяПанель' + t['name'], 'имя панели таблицы = «КоманднаяПанель»+имя таблицы', 'элемент')
    # --- надписи
    for l in labels:
        cp = cap_of(l['x'])
        if (l['name'] or '').startswith('Надпись'):
            R(pre + 'label.height', l['H'], 'высота подписи (имя Надпись…)' + sfx)
            R(pre + 'label.halign', l['x'].findtext('HorizontalAlign') or 'не задано', 'HorizontalAlign надписи' + sfx)


# ----------------------------------------------------------------------------- прочее: свойства объекта
def obj_info(corp_dir, family, obj):
    """Свойства объекта метаданных: Hierarchical и FillChecking реквизитов (если XML объекта есть)."""
    out = {'hier': None, 'fill': {}}
    try:
        root = ET.parse(os.path.join(corp_dir, family, obj + '.xml')).getroot()
    except (OSError, ET.ParseError):
        return out
    for el in root.iter():
        t = el.tag.rsplit('}', 1)[-1]
        if t == 'Hierarchical' and out['hier'] is None:
            out['hier'] = (el.text or '').strip() == 'true'
        elif t in ('Attribute', 'Dimension', 'Resource'):
            nm = fc = None
            for c in el.iter():
                ct = c.tag.rsplit('}', 1)[-1]
                if ct == 'Name' and nm is None:
                    nm = (c.text or '').strip()
                elif ct == 'FillChecking':
                    fc = (c.text or '').strip()
            if nm and fc:
                out['fill'][nm] = fc
    return out


# ----------------------------------------------------------------------------- статистика -> JSON
def quantile(vs, q):
    vs = sorted(vs)
    if not vs:
        return None
    i = (len(vs) - 1) * q
    lo = int(i)
    hi = min(lo + 1, len(vs) - 1)
    return vs[lo] + (vs[hi] - vs[lo]) * (i - lo)


def summarize(cnt, topn=8):
    n = sum(cnt.values())
    items = sorted(cnt.items(), key=lambda kv: (-kv[1], str(kv[0])))
    out = {'n': n, 'distribution': [{'value': k, 'share': round(v / n, 4)} for k, v in items[:topn]]}
    mv, mc = items[0]
    out['value'] = mv
    out['share'] = round(mc / n, 4)
    isnum = all(isinstance(k, (int, float)) and not isinstance(k, bool) for k in cnt)
    if isnum:
        vs = [k for k, c in cnt.items() for _ in range(c)] if n < 200000 else list(cnt)
        out['type'] = 'numeric'
        out['quantiles'] = {'min': min(cnt), 'p10': quantile(vs, .1), 'p50': quantile(vs, .5), 'p90': quantile(vs, .9), 'max': max(cnt)}
    elif all(isinstance(k, bool) for k in cnt):
        out['type'] = 'boolean'
        out['share_true'] = round(cnt.get(True, 0) / n, 4)
    else:
        out['type'] = 'categorical'
    return out


def build_rules(acc, threshold, min_n):
    rules = []
    for (rule, kind), per in sorted(acc.d.items()):
        total = Counter()
        for c in per.values():
            total.update(c)
        n = sum(total.values())
        if n < min_n:
            continue
        s = summarize(total)
        r = {'id': rule, 'kind': kind, 'parameter': acc.meta[rule][0], 'unit': acc.meta[rule][1]}
        r.update(s)
        r['follows'] = s['share'] >= threshold
        r['by_corpus'] = {}
        for corp, c in per.items():
            sc = summarize(c, topn=4)
            e = {'n': sc['n'], 'value': sc['value'], 'share': sc['share'], 'distribution': sc['distribution']}
            if sc['type'] == 'numeric':
                e['quantiles'] = sc['quantiles']
            if sc['type'] == 'boolean':
                e['share_true'] = sc['share_true']
            r['by_corpus'][corp] = e
        rules.append(r)
    return rules


def fmt(v):
    if isinstance(v, bool):
        return 'да' if v else 'нет'
    if isinstance(v, float):
        return ('%g' % round(v, 1))
    return str(v)


def write_tables(rules, meta, path, threshold):
    L = ['# Измеренные таблицы дизайна форм (генерируется `analyze_forms.py`)', '',
         'Формы по видам: ' + ', '.join('%s — %d' % (KIND_RU[k], v['n']) for k, v in meta['kinds'].items()), '']
    byrule = defaultdict(dict)
    for r in rules:
        byrule[r['id']][r['kind']] = r
    for rid in sorted(byrule):
        L.append('## `%s`' % rid)
        any_r = next(iter(byrule[rid].values()))
        L.append(any_r['parameter'] + ' (единица: ' + any_r['unit'] + ')')
        L.append('')
        L.append('| вид | n | мода | доля | p10/p50/p90 | распределение | по корпусам |')
        L.append('|---|---|---|---|---|---|---|')
        for k in KINDS + ['all']:
            r = byrule[rid].get(k)
            if not r:
                continue
            q = r.get('quantiles')
            qs = '%s/%s/%s' % (fmt(q['p10']), fmt(q['p50']), fmt(q['p90'])) if q else ''
            dist = '; '.join('%s %d%%' % (fmt(d['value']), round(d['share'] * 100)) for d in r['distribution'][:5])
            bc = '; '.join('%s: %s %d%% (n=%d)' % (c, fmt(e['value']), round(e['share'] * 100), e['n']) for c, e in r['by_corpus'].items())
            L.append('| %s | %d | %s | %d%% | %s | %s | %s |' % (KIND_RU[k], r['n'], fmt(r['value']), round(r['share'] * 100), qs, dist.replace('|', '¦'), bc.replace('|', '¦')))
        L.append('')
    with open(path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(L))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('corpora', nargs='+', help='МЕТКА=каталог корпуса')
    ap.add_argument('--out', default='target/design')
    ap.add_argument('--limit', type=int, default=0)
    ap.add_argument('--threshold', type=float, default=0.8)
    ap.add_argument('--min-n', type=int, default=5)
    a = ap.parse_args()
    t0 = time.time()
    acc = Acc()
    kinds = defaultdict(lambda: defaultdict(int))
    labels = []
    nfiles = 0
    for spec in a.corpora:
        label, _, d = spec.partition('=')
        if not d:
            d, label = label, os.path.basename(os.path.normpath(label))
        labels.append(label)
        files = sorted(glob.glob(os.path.join(d, '*', '*', 'src', 'Forms', '*', 'Form.xml')))
        if a.limit:
            files = files[:a.limit]
        for p in files:
            rel = os.path.relpath(p, d).split(os.sep)
            family, obj = rel[0], rel[1]
            k = analyze_form(p, label, family, obj, acc, d)
            if k:
                kinds[k][label] += 1
            nfiles += 1
    meta = {'corpora': labels, 'forms_total': nfiles, 'threshold': a.threshold,
            'kinds': {k: {'n': sum(kinds[k].values()), 'by_corpus': dict(kinds[k])} for k in KINDS if k in kinds}}
    rules = build_rules(acc, a.threshold, a.min_n)
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, 'design-rules.json'), 'w', encoding='utf-8') as f:
        json.dump({'meta': meta, 'rules': rules}, f, ensure_ascii=False, indent=1)
    write_tables(rules, meta, os.path.join(a.out, 'tables.md'), a.threshold)
    print('forms: %d, rules: %d, time: %.1f s' % (nfiles, len(rules), time.time() - t0))
    for k, v in meta['kinds'].items():
        print('  %-16s %5d  %s' % (k, v['n'], v['by_corpus']))


if __name__ == '__main__':
    main()
