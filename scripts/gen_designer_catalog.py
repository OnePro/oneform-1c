#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate docs/design/preview-designer/02-property-catalog.md: the reference catalog of visually relevant
Form.xml properties per element class (what 1C does, what the preview does now, how often it occurs).

  python scripts/gen_designer_catalog.py [--out-dir docs/design/preview-designer] [--roots label=path ...]
                                         [--time-limit 240] [--json]

Inputs (inside the repo, its target/ corpora and the research folder):
  docs/coverage/data/properties.json     610 catalog properties (names, types, platform help excerpts)
  docs/xml-reference.md                  Form.xml tags per control type
  scripts/designer_catalog_data.py       curated rows: which tags are visual, preview behaviour, 1C references
  assets/form_render.js                  to compute JS line numbers of the functions named in the rows
  research catalog.json                  enum value names (--research; optional, values degrade to the type name)
  corpora (Form.xml of ordinary forms)   frequencies; see --roots

Only ordinary forms are counted (a Form.xml whose root has an XML namespace is a managed form and is skipped).
Files with identical bytes are counted once. Output is deterministic for a fixed corpus."""
import argparse, collections, hashlib, json, os, re, sys, time
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)
import designer_catalog_data as D  # noqa: E402

DEFAULT_ROOTS = [
    'Бух.=target/a1-live/cfgx',
    'УТ=target/corpus/ut2',
    'УТ 10.3=target/ut103-extracted',
    'tests=tests',
]
DEFAULT_RESEARCH = 'C:/Users/egor/projects/donor1c_objects/1c-ordinary-forms-research/catalog.json'
STRUCT = {'ChildItems', 'Columns', 'Buttons', 'Pages'}
LAYOUT = ['Left', 'Top', 'Width', 'Height', 'ZOrder', 'Anchors', 'TabOrder']
RAW = {'Appearance', 'InfoRecord', 'GeometryFlags', 'ColumnProps', 'PanelRecord', 'FormRecord'}
SHARED_TAGS = {'Font': 'Font', 'HeaderFont': 'Font', 'FooterFont': 'Font', 'TextFont': 'Font',
               'BorderStyle': 'Border', 'Picture': 'Picture', 'Shortcut': 'Shortcut'}


# ------------------------------------------------------------------ scan
def ser(e, depth=0):
    a = ''.join(' %s="%s"' % (k, v if len(v) < 40 else v[:37] + '…') for k, v in e.attrib.items())
    kids = [c for c in e]
    t = (e.text or '').strip()
    if len(t) > 40:
        t = t[:37] + '…'
    if not kids:
        return '<%s%s>%s</%s>' % (e.tag, a, t, e.tag) if t else '<%s%s/>' % (e.tag, a)
    if depth > 1:
        return '<%s%s>…</%s>' % (e.tag, a, e.tag)
    return '<%s%s>%s</%s>' % (e.tag, a, ''.join(ser(c, depth + 1) for c in kids[:2]), e.tag)


def sig(e):
    if len(e):
        return None
    t = (e.text or '').strip()
    return t if t and len(t) < 40 else ('' if not t else None)


class Stat:
    __slots__ = ('files', 'nd_files', 'elems', 'vals', 'sers')

    def __init__(self):
        self.files = set(); self.nd_files = set(); self.elems = 0
        self.vals = collections.Counter(); self.sers = {}


def norm_default(d):
    d = (d or '').strip()
    if d in ('-', 'нет', '') or ' ' in d:
        return None
    return d.lower()


def scan(roots, time_limit, defaults):
    t0 = time.time()
    stats = collections.defaultdict(Stat)           # (cls, key) -> Stat
    ecount = collections.Counter()
    efiles = collections.defaultdict(set)
    seen = set(); info = collections.OrderedDict(); fid = 0
    skipped_managed = 0; broken = 0; dup = 0
    for label, root in roots:
        n = 0; timeout = False
        for d, ds, fs in os.walk(root):
            ds.sort()
            for f in sorted(fs):
                if f != 'Form.xml':
                    continue
                if time.time() - t0 > time_limit:
                    timeout = True
                    break
                p = os.path.join(d, f)
                try:
                    b = open(p, 'rb').read()
                    h = hashlib.md5(b).hexdigest()
                    if h in seen:
                        dup += 1; continue
                    r = ET.fromstring(b)
                except Exception:
                    broken += 1; continue
                seen.add(h)
                if r.tag.startswith('{'):
                    skipped_managed += 1; continue
                n += 1; fid += 1
                walk_form(r, fid, os.path.relpath(p, REPO).replace('\\', '/'), stats, ecount, efiles, defaults)
            if timeout:
                break
        info[label] = {'files': n, 'timeout': timeout}
    return stats, ecount, efiles, {'roots': info, 'files': fid, 'managed_skipped': skipped_managed, 'duplicates_skipped': dup,
                                   'broken': broken, 'seconds': round(time.time() - t0, 1)}


def rec(stats, cls, key, fid, path, e, defaults, text=None, own=None):
    s = stats[(cls, key)]
    s.elems += 1
    s.files.add(fid)
    v = text if text is not None else sig(e)
    if v is not None:
        s.vals[v] += 1
    d = defaults.get((cls, key))
    if d is None or v is None or v.lower() != d:
        s.nd_files.add(fid)
    sr = own if own is not None else ser(e)
    if sr in s.sers:
        s.sers[sr][0] += 1
    elif len(s.sers) < 300:
        s.sers[sr] = [1, path]


def walk_form(root, fid, path, stats, ecount, efiles, defaults):
    for k in root:
        if k.tag in STRUCT:
            continue
        rec(stats, 'Form', k.tag, fid, path, k, defaults)
        shared(stats, k, fid, path)
    for ch in root.findall('ChildItems'):
        visit(ch, 'ChildItems', fid, path, stats, ecount, efiles, defaults)


def shared(stats, k, fid, path):
    t = k.tag
    st = SHARED_TAGS.get(t) or ('Color' if t.endswith('Color') else None)
    if not st:
        return
    for a, v in k.attrib.items():
        rec(stats, '*', st + '@' + a, fid, path, k, {}, text=v, own=ser(k))
    if st in ('Border', 'Picture'):
        rec(stats, '*', st, fid, path, k, {}, text=(k.text or '').strip() or k.get('kind', ''), own=ser(k))


def visit(node, container, fid, path, stats, ecount, efiles, defaults):
    for c in node:
        cls = 'CommandBarButton' if container == 'Buttons' else {'Column': 'TableColumn', 'Page': 'PanelPage'}.get(c.tag, c.tag)
        ecount[cls] += 1; efiles[cls].add(fid)
        for a, v in c.attrib.items():
            if a in ('id', 'name'):
                continue
            rec(stats, cls, '@' + a, fid, path, c, defaults, text=v, own='<%s … %s="%s" …>' % (c.tag, a, v))
        for k in c:
            if k.tag in STRUCT:
                visit(k, k.tag, fid, path, stats, ecount, efiles, defaults)
                continue
            rec(stats, cls, k.tag, fid, path, k, defaults)
            shared(stats, k, fid, path)


# ------------------------------------------------------------------ inputs
def load_catalog():
    rows = json.load(open(os.path.join(REPO, 'docs/coverage/data/properties.json'), encoding='utf-8'))
    idx = collections.defaultdict(list)
    for r in rows:
        if r['xml_tag'] != '-':
            idx[(r['owner_xml'], r['xml_tag'])].append(r)
    return idx


def load_enums(path):
    try:
        c = json.load(open(path, encoding='utf-8'))
    except Exception:
        print('warning: research catalog not readable (%s): enum values degrade to the type name' % path, file=sys.stderr)
        return {}
    return {e['name_ru']: e['values'] for e in c['enums']}


def load_xml_reference():
    t = open(os.path.join(REPO, 'docs/xml-reference.md'), encoding='utf-8').read()
    return {m.group(1): re.findall(r'`(\w+)`', m.group(2)) for m in re.finditer(r'^## (\w+)\n\n(.+)$', t, re.M)}


def js_lines():
    out = {}
    for i, line in enumerate(open(os.path.join(REPO, 'assets/form_render.js'), encoding='utf-8'), 1):
        m = re.match(r'\s*function (\w+)\(', line)
        if m and m.group(1) not in out:
            out[m.group(1)] = i
    return out


def parse_rows(cls):
    rows = []
    for line in D.ROWS[cls].strip().splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith('='):
            line = D.COMMON[line[1:]]
        f = line.split('|')
        if len(f) < 6:
            sys.exit('bad row in %s: %r' % (cls, line))
        rows.append(dict(tag=f[0], st=f[1], fn=f[2], now=D._f(f[3]), ref=D._f(f[4]), default=f[5], values=f[6] if len(f) > 6 else ''))
    return rows


def esc(s):
    return str(s).replace('|', '/').replace('\n', ' ').replace('\r', ' ')


def link_refs(text):
    return re.sub(r'\b(\d\d-[a-z0-9-]+-1c\.png)', lambda m: '[%s](gallery/%s)' % (m.group(1), m.group(1)), text)


def fmt_vals(counter, n=3):
    return ', '.join('`%s`×%d' % (esc(k)[:30], c) for k, c in counter.most_common(n + 1)[:n] if k != '')


def best_ser(s):
    return sorted(s.sers.items(), key=lambda kv: (len(kv[0]) > 150, -kv[1][0], len(kv[0])))[0]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--out-dir', default=os.path.join(REPO, 'docs/design/preview-designer'))
    ap.add_argument('--roots', nargs='*', default=DEFAULT_ROOTS, help='label=path (relative to the repo); default: %s' % '; '.join(DEFAULT_ROOTS))
    ap.add_argument('--time-limit', type=float, default=240, help='seconds for the whole corpus scan (default 240)')
    ap.add_argument('--research', default=DEFAULT_RESEARCH, help='research catalog.json (enum value names); optional')
    ap.add_argument('--json', action='store_true', help='also write internal/scan-summary.json')
    a = ap.parse_args()
    roots = []
    for r in a.roots:
        label, _, p = r.partition('=')
        roots.append((label, p if os.path.isabs(p) else os.path.join(REPO, p)))

    cat = load_catalog(); enums = load_enums(a.research)
    xref = load_xml_reference(); jl = js_lines()
    rows_by_cls = {c[0]: parse_rows(c[0]) for c in D.CLASSES}
    defaults = {}
    for cid, rr in rows_by_cls.items():
        for r in rr:
            nd = norm_default(r['default'])
            if nd is not None:
                defaults[(cid, r['tag'])] = nd
    stats, ecount, efiles, info = scan(roots, a.time_limit, defaults)
    T = info['files']
    ecount['Form'] = T
    efiles['Form'] = set(range(1, T + 1))

    def catrow(owner, tag):
        t = tag.lstrip('@')
        for o in (owner, 'Extension for controls located in a form', 'Panel controls extension', 'Extension for controls located in a table box', 'Panel'):
            lst = cat.get((o, t))
            if lst:
                for r in lst:
                    if r['name_en'] == t:
                        return r
                return lst[0]
        return None

    def values_cell(r, cr):
        if r['values']:
            return esc(r['values'])
        if not cr:
            return 'см. справку'
        t = cr['type_ru'] or ''
        if t == 'Булево':
            return '`true` / `false`'
        if t in enums:
            return esc('; '.join('`%s` — %s' % (v['name_en'], v['name_ru']) for v in enums[t]))
        return {'Цвет': 'цвет, см. `Color.*`', 'Шрифт': 'см. `Font.*`', 'Картинка': 'см. `Picture.*`', 'Рамка': 'см. `Border.*`'}.get(t, esc(t or 'см. справку'))

    STAT = {'D': 'рисуется', 'P': 'частично', 'I': 'игнорируется', 'S': 'только inspector'}

    def now_cell(r):
        now = esc(r['now'])
        if r['st'] == 'I':
            now = re.sub(r'^игнорируется[:;]?\s*', '', now)
            now = now if not now or now.startswith('(') else '(' + now + ')'
        sep = ' ' if now.startswith('(') else '. '
        s = '**%s**%s%s' % (STAT[r['st']], sep, now) if now else '**%s**' % STAT[r['st']]
        if r['fn'] != '-':
            s += ' (`%s`%s)' % (r['fn'], ', JS:%d' % jl[r['fn']] if r['fn'] in jl else '')
        return s

    def freq(cls, key):
        s = stats.get((cls, key))
        if not s:
            return 0, '0 / %d (в корпусе не встречается)' % T
        txt = '%d / %d; эл.: %d' % (len(s.nd_files), T, s.elems)
        v = fmt_vals(s.vals)
        return len(s.nd_files), txt + ('; значения: ' + v if v else '')

    def snippet(cls, key, xmlname):
        s = stats.get((cls, key))
        if s and s.sers:
            b = best_ser(s)
            return '`%s`' % esc(b[0]).replace('`', "'"), b[1][1]
        t = key.lstrip('@')
        if key.startswith('@'):
            return '`<%s … %s="…" …>` ⚠ в корпусе не встречается' % (xmlname.split('@')[0], t), None
        return '`<%s>…</%s>` ⚠ в корпусе не встречается (по xml-reference)' % (t, t), None

    def rank(st):
        return 0 if st in 'IS' else 1 if st == 'P' else 2

    sections = []; ref_needed = []
    summary = {'corpus': info, 'classes': {}, 'rows': {}}
    for cid, owner, xmlname, ru, desc in D.CLASSES:
        rr = rows_by_cls[cid]
        out_rows = []
        for r in rr:
            key = r['tag']
            cr = catrow(owner or 'Form', key)
            nd, ftxt = freq(cid, key)
            snip, ex = snippet(cid, key, xmlname)
            rid = '%s.%s' % (cid, key.lstrip('@'))
            name = cr['name_ru'] if cr else '—'
            exc = esc((cr or {}).get('source_excerpt') or '')
            exc = exc[:170] + ('…' if len(exc) > 170 else '')
            if r['ref'] != '-':
                looks = link_refs(esc(r['ref'])) + ('. Справка: ' + exc if exc else '')
            else:
                looks = '**нет эталона**' + ('. Справка: ' + exc if exc else '')
                ref_needed.append((rid, ru, name, values_cell(r, cr), nd, ftxt))
            out_rows.append(((rank(r['st']), -nd, rid), ['`%s`' % rid, snip, esc(name), values_cell(r, cr),
                             esc(r['default']) if r['default'] != '-' else '—', looks, now_cell(r), ftxt]))
            summary['rows'][rid] = {'files_nondefault': nd, 'example': ex, 'status': r['st']}
        out_rows.sort(key=lambda x: x[0])
        used = {r['tag'] for r in rr}
        names = set(xref.get(xmlname, [])) | {k for (c, k) in stats if c == cid and not k.startswith('@')}
        names -= used | STRUCT
        layout = [t for t in LAYOUT if t in names]; names -= set(LAYOUT)
        raws = sorted(names & RAW); names -= RAW
        events = 'Events' in names; names -= {'Events'}
        sections.append((cid, xmlname, ru, desc, out_rows, layout, raws, sorted(names), events))
        summary['classes'][cid] = {'elements': ecount.get(cid, 0), 'files': len(efiles.get(cid, ())), 'rows': len(out_rows)}

    shared_rows = []
    for sname, items in D.SHARED.items():
        rows = []
        for (sid, xml, ru, vals, default, st, now, ref) in items:
            ref = D._f(ref)
            skey = 'Border' if xml == 'BorderStyle' else xml.replace('*Color', 'Color')
            s = stats.get(('*', skey))
            rid = '%s.%s' % (sname, sid)
            if s:
                nd = len(s.files)
                ftxt = '%d / %d; эл.: %d; значения: %s' % (nd, T, s.elems, fmt_vals(s.vals, 4))
                b = best_ser(s)
                snip = '`%s`' % esc(b[0]).replace('`', "'")
                summary['rows'][rid] = {'files_nondefault': nd, 'example': b[1][1], 'status': st}
            else:
                nd = 0
                ftxt = '0 / %d (в корпусе не встречается)' % T
                snip = '`%s` ⚠ в корпусе не встречается' % xml
            if ref != '-':
                looks = link_refs(esc(ref))
            else:
                looks = '**нет эталона**'
                ref_needed.append((rid, sname, ru, esc(vals), nd, ftxt))
            rows.append(((rank(st), -nd, rid), ['`%s`' % rid, snip, esc(ru), esc(vals), esc(default) if default != '-' else '—', looks,
                                                '**%s**. %s' % (STAT[st], esc(now)), ftxt]))
        rows.sort(key=lambda x: x[0])
        shared_rows.append((sname, rows))

    L = []
    w = L.append
    nshared = sum(len(r) for _, r in shared_rows)
    nrows = sum(len(s[4]) for s in sections) + nshared
    w('# 02. Каталог свойств (справочник дизайнера)\n')
    w('Это **справочник**: его читают, в нём ничего не заполняют. Результат вашей работы — макет `style-sheet.html` и инструкции `render-spec.md` (см. `05-what-to-return.md`); в них каждое свойство из этого каталога по ID получает вид и правило для HTML/CSS. Файл создан скриптом `scripts/gen_designer_catalog.py` (данные строк — `scripts/designer_catalog_data.py`); руками не правится.\n')
    w('## Как читать\n')
    w('* **ID** — устойчивое имя вида `Класс.Тег` (для атрибутов `Класс.атрибут`, для общих структур `Font.bold`). На него ссылайтесь в `data-xml="…"` листа образцов и в правилах `render-spec.md`.')
    w('* **XML** — как свойство выглядит в настоящем `Form.xml` (взято из реального файла корпуса; `⚠` — в корпусе не встречается, пример по `docs/xml-reference.md`). В Form.xml записываются только значения, отличающиеся от «нового элемента»; отсутствие тега = значение по умолчанию.')
    w('* **Конфигуратор** — название свойства в окне «Свойства» (русское имя из справочника платформы).')
    w('* **Как выглядит в 1С** — что известно о настоящем виде: ссылка на картинку из `gallery/` или `нет эталона` + выдержка из справки платформы.')
    w('* **Что делает превью сейчас** — **рисуется** / **частично** / **игнорируется** / **только inspector** (значение видно только в боковой панели, на холсте нет) и имя функции в `assets/form_render.js` (номер строки — в текущей версии файла из `tools/assets/`).')
    w('* **Частота** — `N / T`: в скольких формах корпуса свойство задано **не по умолчанию** (для тегов без известного умолчания — просто присутствует) из T форм; `эл.` — число элементов; затем самые частые значения.')
    w('* Внутри класса строки отсортированы: сначала то, что превью **игнорирует**, и чаще встречающееся; затем частично рисуемое; затем рисуемое.\n')
    w('## Корпус для частот\n')
    w('Учтено **%d** обычных форм (уникальных по содержимому); пропущено: %d управляемых форм, %d дублей, %d не разобрано. Источники (Form.xml в формате нашего инструмента, `ordinaryFormVersion`):\n' % (T, info['managed_skipped'], info['duplicates_skipped'], info['broken']))
    for label, v in info['roots'].items():
        w('* %s — %d форм%s' % (label, v['files'], ' (прервано по лимиту времени)' if v['timeout'] else ''))
    w('\nЭто выгрузки конфигураций «Управление торговлей» и «Бух.», где обычные формы хранятся в том же `Form.xml`, что и в нашем инструменте. Время сканирования: %s с.\n' % info['seconds'])
    w('## Классы элементов\n')
    w('| Класс | Что это | Строк в каталоге | Элементов в корпусе | В скольких формах |')
    w('|---|---|---|---|---|')
    for cid, xmlname, ru, desc, out_rows, *_ in sections:
        w('| `%s` (%s) | %s | %d | %d | %d |' % (cid, esc(ru), esc(desc), len(out_rows), ecount.get(cid, 0), len(efiles.get(cid, ()))))
    w('| Общие структуры | Font, Color, Border, Picture, Shortcut | %d | — | — |' % nshared)
    w('')
    ref_needed.sort(key=lambda x: (-x[4], x[0]))
    w('## Нужны эталоны из 1С\n')
    w('Для этих свойств у нас нет скриншота настоящей 1С (%d строк). Если без него вы не можете решить, как рисовать, **напишите это в `questions.md`** (ID и что именно показать) — мы снимем. Отсортировано по частоте в корпусе; нижние строки в корпусе не встречаются (по ним эталон нужен только при желании).\n' % len(ref_needed))
    w('| ID | Что снять в 1С | Частота |')
    w('|---|---|---|')
    for rid, cru, name, vals, nd, ftxt in ref_needed:
        w('| `%s` | %s | %s |' % (rid, esc(('Элемент «%s» с установленным «%s» (значения: %s)' % (cru, name, vals.replace('`', '')))[:230]), esc(ftxt.split(';')[0])))
    w('')
    HDR = '| ID | XML | Конфигуратор | Значения | По умолчанию | Как выглядит в 1С | Что делает превью сейчас | Частота |'
    SEP = '|---|---|---|---|---|---|---|---|'
    for cid, xmlname, ru, desc, out_rows, layout, raws, other, events in sections:
        w('## %s — `%s`\n' % (ru, cid))
        w('%s Элемент XML: `<%s>`. Элементов в корпусе: %d (в %d формах).\n' % (desc, xmlname.split('@')[0], ecount.get(cid, 0), len(efiles.get(cid, ()))))
        w(HDR); w(SEP)
        for _, cells in out_rows:
            w('| ' + ' | '.join(cells) + ' |')
        w('')
        ex = []
        if layout:
            ex.append('координаты и привязки — **менять нельзя**: ' + ', '.join('`%s`' % t for t in layout))
        if other:
            ex.append(', '.join('`%s`' % t for t in other))
        if events:
            ex.append('обработчики событий `Events`')
        if raws:
            ex.append('служебные бинарные блоки (`raw={…}`, превью их не разбирает): ' + ', '.join('`%s`' % t for t in raws))
        w('**Исключены как не влияющие на вид:** ' + ('; '.join(ex) if ex else '—') + '.\n')
    w('## Общие структуры\n')
    w('Шрифт, цвет, рамка, картинка и сочетание клавиш встречаются в разных классах; их атрибуты описаны здесь один раз. Частоты — по всем классам вместе (число форм с атрибутом).\n')
    for sname, rows in shared_rows:
        w('### %s\n' % sname)
        w(HDR); w(SEP)
        for _, cells in rows:
            w('| ' + ' | '.join(cells) + ' |')
        w('')
    os.makedirs(a.out_dir, exist_ok=True)
    outp = os.path.join(a.out_dir, '02-property-catalog.md')
    with open(outp, 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(L) + '\n')
    if a.json:
        os.makedirs(os.path.join(a.out_dir, 'internal'), exist_ok=True)
        summary['rows_total'] = nrows; summary['need_reference'] = len(ref_needed)
        with open(os.path.join(a.out_dir, 'internal', 'scan-summary.json'), 'w', encoding='utf-8') as f:
            json.dump(summary, f, ensure_ascii=False, indent=1, sort_keys=True)
    print('wrote %s: %d rows (%d classes + %d shared); need reference: %d; forms scanned %d (%.1fs)' % (
        outp, nrows, len(sections), nshared, len(ref_needed), T, info['seconds']))
    print(' '.join('%s=%d' % (c, v['rows']) for c, v in summary['classes'].items()))


if __name__ == '__main__':
    main()
