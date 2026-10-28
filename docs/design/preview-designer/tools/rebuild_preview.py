#!/usr/bin/env python3
"""Rebuild the HTML preview of an ordinary 1C form from Form.xml, using your (edited) CSS/JS copies.

  python rebuild_preview.py <form_dir | Form.xml> [-o out.html]
         [--theme assets/form_theme.css] [--render assets/form_render.js] [--hljs assets/hljs_1c_xml.js]
         [--theme-extra your/theme.css] [--rs skeleton/preview_template.rs.txt] [--check]

Self-contained package version: the defaults point to the files next to this script (tools/assets/*, tools/skeleton/*).
The page skeleton (topbar, inspector pane, CSS/JS consts, <script> order) is read from the template snapshot.
Default out: tools/out/<form folder name>.html. --check compares with FormPreview.html next to Form.xml
(only meaningful for stock assets) and prints IDENTICAL / DIFFERENT.
Python 3.8+, stdlib only."""
import argparse, html, json, os, re, sys
import xml.etree.ElementTree as ET

def read_text_lf(p):
    with open(p, 'rb') as f:
        return f.read().decode('utf-8').replace('\r\n', '\n').replace('\r', '\n')

def rs_const(src, name):
    m = re.search(r'const %s: &str = r(#+)"(.*?)"\1;' % re.escape(name), src, re.S)
    if not m: sys.exit('const %s not found in the template snapshot' % name)
    return m.group(2)

def rs_template(src):
    m = re.search(r'let page = format!\(\s*r(#+)"(.*?)"\1\s*\)', src, re.S)
    if not m: sys.exit('page template not found in the template snapshot')
    return m.group(2)

BS = chr(92)

def lf(t):
    return t.replace(chr(13)+chr(10), chr(10)).replace(chr(13), chr(10))

def json_embed(v):
    s = json.dumps(v, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
    for a, b in (('<', BS+'u003c'), ('>', BS+'u003e'), ('&', BS+'u0026'), (chr(0x2028), BS+'u2028'), (chr(0x2029), BS+'u2029')):
        s = s.replace(a, b)
    return s

def py_int(s):
    try: return int(s.strip())
    except Exception: return 0

def read_lossy(p):
    if not os.path.isfile(p): return ''
    with open(p, 'rb') as f:
        return f.read().decode('utf-8', errors='replace')

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('path'); ap.add_argument('-o', '--out')
    root = os.path.dirname(os.path.abspath(__file__))
    ap.add_argument('--theme', default=os.path.join(root, 'assets', 'form_theme.css'))
    ap.add_argument('--render', default=os.path.join(root, 'assets', 'form_render.js'))
    ap.add_argument('--hljs', default=os.path.join(root, 'assets', 'hljs_1c_xml.js'))
    ap.add_argument('--rs', default=os.path.join(root, 'skeleton', 'preview_template.rs.txt'))
    ap.add_argument('--object-module', help='ObjectModule.bsl for the "Модуль объекта" tab (optional)')
    ap.add_argument('--theme-extra', help='extra CSS appended AFTER the theme (your theme.css overrides the stock rules)')
    ap.add_argument('--check', action='store_true')
    a = ap.parse_args()
    form_dir = os.path.dirname(a.path) if a.path.lower().endswith('.xml') else a.path
    xml_path = a.path if a.path.lower().endswith('.xml') else os.path.join(a.path, 'Form.xml')
    alias = os.path.basename(os.path.normpath(form_dir))
    src_root = os.path.dirname(os.path.dirname(os.path.abspath(form_dir)))
    xml_lossy = read_lossy(xml_path)
    r = ET.fromstring(xml_lossy.lstrip('\ufeff').encode('utf-8'))
    def ic(t):
        e = r.find(t); return py_int(e.text or '') if e is not None else 0
    name = r.get('name') or alias
    cap = {}
    for it in r.findall('./Caption/Item'): cap[it.get('language') or ''] = it.text or ''
    mod = ''
    for n in ('FormModule.bsl', 'Module.bsl'):
        if os.path.isfile(os.path.join(form_dir, n)): mod = read_lossy(os.path.join(form_dir, n)); break
    obj = read_lossy(a.object_module) if a.object_module else read_lossy(os.path.join(src_root, 'ObjectModule.bsl'))
    if not obj and not a.object_module:
        ext = os.path.join(os.path.dirname(src_root), 'Ext')
        for n in ('ObjectModule.bsl', 'Module.bsl'):
            if os.path.isfile(os.path.join(ext, n)):
                obj = lf(read_lossy(os.path.join(ext, n)).lstrip(chr(0xFEFF))); break
    procs = {}
    pre = re.compile(r'^\s*(Процедура|Функция|Procedure|Function)\s+(\w+)', re.I)
    for i, line in enumerate(mod.replace('\r\n', '\n').replace('\r', '\n').split('\n')):
        m = pre.match(line)
        if m and m.group(2).casefold() not in procs: procs[m.group(2).casefold()] = i + 1
    ev = [{'name': e.tag, 'handler': (e.text or '').strip()} for evs in r.findall('Events') for e in evs]
    data = {'xml': xml_lossy, 'formModule': mod, 'objectModule': obj, 'alias': alias, 'procs': procs, 'formEvents': ev}
    rs = read_text_lf(a.rs)
    page = rs_template(rs)
    q = lambda s: html.escape(s, quote=True).replace('&#x27;', '&#x27;')
    sub = {
        'name_e': q(name), 'caption_e': q(cap.get('ru', '')), 'width': str(ic('Width')), 'height': str(ic('Height')),
        'obj_tab': '' if not obj else '<button class="tab" data-tab="object">Модуль объекта</button>',
        'obj_pane': '' if not obj else '<div class="pane" id="p-object"></div>',
        'theme': read_text_lf(a.theme) + (chr(10) + read_text_lf(a.theme_extra) if a.theme_extra else ''),
        'CODE_CSS': rs_const(rs, 'CODE_CSS'), 'CSS': rs_const(rs, 'CSS'),
        'hljs': read_text_lf(a.hljs), 'render': read_text_lf(a.render), 'data_json': json_embed(data), 'JS': rs_const(rs, 'JS'),
    }
    out = re.sub(r'\{(\w+)\}', lambda m: sub[m.group(1)], page)
    if a.out: outp = a.out
    else:
        os.makedirs(os.path.join(root, 'out'), exist_ok=True)
        outp = os.path.join(root, 'out', alias + '.html')
    with open(outp, 'wb') as f: f.write(out.encode('utf-8'))
    print('wrote', outp, len(out.encode('utf-8')), 'bytes')
    if a.check:
        real = os.path.join(form_dir, 'FormPreview.html')
        same = os.path.isfile(real) and open(real, 'rb').read() == out.encode('utf-8')
        print('IDENTICAL to FormPreview.html' if same else 'DIFFERENT from FormPreview.html')
        sys.exit(0 if same else 1)

if __name__ == '__main__':
    main()
