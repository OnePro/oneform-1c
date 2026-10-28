#!/usr/bin/env python3
"""Geometry invariance check of the HTML preview: bounding boxes of every `.c` control.

  python scripts/preview_geometry_check.py --save scripts/preview_geometry_baseline.json   # measure, store
  python scripts/preview_geometry_check.py --compare scripts/preview_geometry_baseline.json # measure, diff (exit 1 on any diff)

Forms: the five designer samples (docs/design/preview-designer/tools/samples) and the gallery forms
(target/designer/gallery/work/<Kind>/<Object>/src/Forms/<Form>/Form.xml listed in target/designer/gallery/forms.txt).
Each form is built with tools/rebuild_preview.py from assets/form_theme.css + assets/form_render.js
(override with --theme/--render) and measured in headless Edge in designer and runtime modes."""
import argparse, concurrent.futures, glob, html, json, os, re, subprocess, sys, tempfile, pathlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOLS = os.path.join(ROOT, 'docs', 'design', 'preview-designer', 'tools')
WORK = os.path.join(ROOT, 'target', 'designer', 'gallery', 'work')
EDGES = [r'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe', r'C:\Program Files\Microsoft\Edge\Application\msedge.exe']
INJECT = ('<script>window.addEventListener("load",function(){setTimeout(function(){var o=[];'
          'document.querySelectorAll(".c").forEach(function(e){var r=e.getBoundingClientRect();'
          'o.push([e.id||e.getAttribute("data-k")||"",Math.round(r.left*100)/100,Math.round(r.top*100)/100,'
          'Math.round(r.width*100)/100,Math.round(r.height*100)/100,getComputedStyle(e).zIndex])});'
          'var p=document.createElement("pre");p.id="geomdump";p.textContent=JSON.stringify(o);document.body.appendChild(p)},300)});</script>')

def forms():
    out = {}
    for d in sorted(glob.glob(os.path.join(TOOLS, 'samples', '*'))):
        out['sample/' + os.path.basename(d)] = os.path.join(d, 'Form.xml')
    lst = os.path.join(ROOT, 'target', 'designer', 'gallery', 'forms.txt')
    for line in open(lst, encoding='utf-8'):
        n, s, k, o, f, m = line.split()
        x = os.path.join(WORK, k, o, 'src', 'Forms', f, 'Form.xml')
        if os.path.isfile(x):
            out['gallery/%s-%s' % (n, s)] = x
    return out

def one(args):
    edge, tmp, i, name, xml, theme, render = args
    h = os.path.join(tmp, '%03d.html' % i)
    subprocess.check_call([sys.executable, os.path.join(TOOLS, 'rebuild_preview.py'), xml, '-o', h,
                           '--theme', theme, '--render', render], stdout=subprocess.DEVNULL)
    t = open(h, encoding='utf-8').read()
    k = t.rfind('</body>')
    open(h, 'w', encoding='utf-8').write(t[:k] + INJECT + t[k:])
    res = {}
    for mode in ('designer', 'runtime'):
        url = pathlib.Path(h).resolve().as_uri() + '?chrome=0&mode=' + mode
        prof = os.path.join(tmp, 'p%03d%s' % (i, mode))
        r = subprocess.run([edge, '--headless', '--disable-gpu', '--hide-scrollbars', '--window-size=1600,1200',
                            '--user-data-dir=' + prof, '--virtual-time-budget=2000', '--dump-dom', url],
                           capture_output=True, timeout=120)
        m = re.search(r'<pre id="geomdump">(.*?)</pre>', r.stdout.decode('utf-8', 'replace'), re.S)
        if not m:
            sys.exit('no geometry dump for %s (%s)' % (name, mode))
        res['%s|%s' % (name, mode)] = json.loads(html.unescape(m.group(1)))
    print('measured', name, file=sys.stderr)
    return res

def measure(theme, render, only=()):
    edge = next(p for p in EDGES if os.path.isfile(p))
    tmp = tempfile.mkdtemp(prefix='geom')
    jobs = [(edge, tmp, i, n, x, theme, render) for i, (n, x) in enumerate(forms().items()) if not only or any(o in n for o in only)]
    res = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as ex:
        for r in ex.map(one, jobs):
            res.update(r)
    return res

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--save'); ap.add_argument('--compare')
    ap.add_argument('--only', nargs='*', default=[], help='measure only forms whose name contains one of these substrings (--compare then checks just those runs)')
    ap.add_argument('--theme', default=os.path.join(ROOT, 'assets', 'form_theme.css'))
    ap.add_argument('--render', default=os.path.join(ROOT, 'assets', 'form_render.js'))
    a = ap.parse_args()
    cur = measure(a.theme, a.render, a.only)
    if a.save:
        json.dump(cur, open(a.save, 'w', encoding='utf-8'), ensure_ascii=False, separators=(',', ':'))
        print('saved', a.save, sum(len(v) for v in cur.values()), 'boxes')
    if a.compare:
        base = json.load(open(a.compare, encoding='utf-8'))
        bad = 0
        for key in sorted(set(base) & set(cur) if a.only else set(base) | set(cur)):
            b, c = base.get(key), cur.get(key)
            if b is None or c is None:
                print('MISSING', key); bad += 1; continue
            if len(b) != len(c):
                print('COUNT', key, len(b), len(c)); bad += 1; continue
            for x, y in zip(b, c):
                if x != y:
                    print('DIFF', key, x, y); bad += 1
        print('geometry: %d differences in %d runs, %d boxes' % (bad, len(cur), sum(len(v) for v in cur.values())))
        sys.exit(1 if bad else 0)

if __name__ == '__main__':
    main()
