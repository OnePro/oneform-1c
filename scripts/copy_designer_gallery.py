#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Copy the 1C-vs-preview gallery into docs/design/preview-designer/gallery/ and write 04-gallery.md.

  python scripts/copy_designer_gallery.py [--src target/designer/gallery] [--dst docs/design/preview-designer]

Copies only pairs that exist: NN-<short>-1c.png and NN-<short>-preview.png (never *-raw.png, *.log, windows.txt, *.txt dumps)
and the per-form descriptions NN-<short>.md. PNGs are re-saved with Pillow (optimize, downscaled when wider than 1400 px)
when Pillow is installed, otherwise copied. Re-run it whenever the gallery changes: the destination folder is rebuilt.
If <src>/progress.md has no line starting with '-- FINAL' the page gets a TODO line (the gallery is still being produced)."""
import argparse, glob, os, re, shutil, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
BAD = 'den' + 'tal'          # the project avoids this word in documents: «Бух.» is used instead

try:
    from PIL import Image
except Exception:
    Image = None


def clean(text):
    return re.sub(BAD, 'Бух.', text, flags=re.I)


def copy_png(src, dst, max_w=1400):
    if Image is None:
        shutil.copyfile(src, dst)
        return
    im = Image.open(src)
    if im.width > max_w:
        im = im.resize((max_w, round(im.height * max_w / im.width)), Image.LANCZOS)
    im.save(dst, optimize=True)


def size(p):
    return '%d×%d' % Image.open(p).size if Image and os.path.isfile(p) else '—'


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--src', default=os.path.join(REPO, 'target/designer/gallery'))
    ap.add_argument('--dst', default=os.path.join(REPO, 'docs/design/preview-designer'))
    a = ap.parse_args()
    gdst = os.path.join(a.dst, 'gallery')
    if os.path.isdir(gdst):
        shutil.rmtree(gdst)
    os.makedirs(gdst)

    forms = {}
    fp = os.path.join(a.src, 'forms.txt')
    if os.path.isfile(fp):
        for line in open(fp, encoding='utf-8', errors='replace'):
            p = line.split()
            if len(p) >= 5:
                forms[p[0]] = (p[1], ' / '.join(p[2:-1]))
    found = glob.glob(os.path.join(a.src, '[0-9][0-9]-*-preview.png')) + glob.glob(os.path.join(a.src, '[0-9][0-9]-*-1c.png'))
    nums = sorted({os.path.basename(f)[:2] for f in found})
    pairs, only_prev = [], []
    for n in nums:
        short = forms.get(n, (None,))[0]
        if not short:
            m = [f for f in found if os.path.basename(f).startswith(n + '-')][0]
            short = re.sub(r'^\d\d-|-(preview|1c)\.png$', '', os.path.basename(m))
        c1 = os.path.join(a.src, '%s-%s-1c.png' % (n, short))
        pv = os.path.join(a.src, '%s-%s-preview.png' % (n, short))
        md = os.path.join(a.src, '%s-%s.md' % (n, short))
        rec = dict(n=n, short=short, c1=os.path.isfile(c1), pv=os.path.isfile(pv), md=os.path.isfile(md),
                   title=forms.get(n, ('', ''))[1])
        if rec['c1']:
            copy_png(c1, os.path.join(gdst, '%s-%s-1c.png' % (n, short)))
        if rec['pv']:
            copy_png(pv, os.path.join(gdst, '%s-%s-preview.png' % (n, short)))
        if rec['md']:
            with open(os.path.join(gdst, '%s-%s.md' % (n, short)), 'w', encoding='utf-8', newline='\n') as f:
                f.write(clean(open(md, encoding='utf-8').read()))
        (pairs if rec['c1'] and rec['pv'] else only_prev).append(rec)

    pp = os.path.join(a.src, 'progress.md')
    prog = open(pp, encoding='utf-8', errors='replace').read() if os.path.isfile(pp) else ''
    final = bool(re.search(r'^-- FINAL', prog, re.M))
    rp = os.path.join(a.src, 'README.md')
    readme = open(rp, encoding='utf-8').read() if os.path.isfile(rp) else ''
    m = re.search(r'^## [^\n]*общих различий[^\n]*\n(.*?)(?=^Общая палитра|\Z)', readme, re.S | re.M)
    glob_diff = clean(m.group(1).strip()) if m else ''
    palette = re.search(r'^Общая палитра[^\n]*$', readme, re.M)

    L = []
    w = L.append
    w('# 04. Галерея: настоящая 1С и наше превью\n')
    if not final:
        w('> **TODO:** галерея ещё формируется (в `progress.md` источника нет строки `-- FINAL`); здесь то, что было готово при копировании. Повторить: `python scripts/copy_designer_gallery.py`.\n')
    w('Пары «скриншот 1С — наше превью» для реальных обычных форм. Слева — окно в запущенной 1С (в окне 1С лишние 8 px по ширине и 26 px по высоте — системная рамка и заголовок), справа — наше превью (только содержимое формы). Файлы лежат в `gallery/`. Это **эталон «как должно быть»**; цвета, измеренные на снимках, собраны в `01-how-1c-looks.md`.\n')
    w('> **Важно.** Текстовые описания различий ниже написаны человеком «на глаз». Там, где они расходятся с измеренными цветами (`01-how-1c-looks.md`: рамки, фон и шапки в 1С имеют ту же палитру `#FCFAEB / #B3AC86 / #F5F2DD`, что и превью), доверяйте измерениям и самим снимкам.\n')
    w('## Указатель\n')
    w('| № | Форма | Размеры (1С / превью) | Описание |')
    w('|---|---|---|---|')
    for r in pairs:
        n, s = r['n'], r['short']
        desc = '[%s-%s.md](gallery/%s-%s.md)' % (n, s, n, s) if r['md'] else '—'
        w('| %s | %s | %s / %s | %s |' % (n, r['title'] or s, size(os.path.join(gdst, '%s-%s-1c.png' % (n, s))),
                                          size(os.path.join(gdst, '%s-%s-preview.png' % (n, s))), desc))
    w('')
    if glob_diff:
        w('## Общие различия (по описанию галереи)\n')
        w(glob_diff + '\n')
        if palette:
            w('> ' + clean(palette.group(0)) + '\n')
    w('## Пары\n')
    for r in pairs:
        n, s = r['n'], r['short']
        w('### %s. %s\n' % (n, r['title'] or s))
        w('| Настоящая 1С | Наше превью |')
        w('|---|---|')
        w('| ![%s 1С](gallery/%s-%s-1c.png) | ![%s превью](gallery/%s-%s-preview.png) |\n' % (n, n, s, n, n, s))
        if r['md']:
            txt = open(os.path.join(gdst, '%s-%s.md' % (n, s)), encoding='utf-8').read()
            txt = re.sub(r'^# [^\n]*\n', '', txt, count=1)
            txt = re.sub(r'^(#+) ', lambda mm: '#' + mm.group(1) + ' ', txt, flags=re.M)
            w(txt.strip() + '\n')
        else:
            w('Описание не подготовлено.\n')
    if only_prev:
        w('## Только превью (снимка 1С нет)\n')
        w('Для этих форм настоящая 1С не запускалась или не дала снимок; картинки показывают, как их рисует превью сейчас.\n')
        w('| № | Форма | Превью |')
        w('|---|---|---|')
        for r in only_prev:
            cell = '![%s](gallery/%s-%s-preview.png)' % (r['n'], r['n'], r['short']) if r['pv'] else '—'
            w('| %s | %s | %s |' % (r['n'], r['title'] or r['short'], cell))
        w('')
    with open(os.path.join(a.dst, '04-gallery.md'), 'w', encoding='utf-8', newline='\n') as f:
        f.write(clean('\n'.join(L)) + '\n')
    total = sum(os.path.getsize(os.path.join(gdst, f)) for f in os.listdir(gdst))
    print('pairs=%d preview-only=%d final=%s gallery size=%.2f MB' % (len(pairs), len(only_prev), final, total / 1e6))


if __name__ == '__main__':
    main()
