#!/usr/bin/env python3
"""Build previews of all sample forms (tools/samples/*/Form.xml) with YOUR assets and, optionally, PNG screenshots.

  python build_samples.py [--overlay response/theme.css] [--theme assets/form_theme.css] [--render assets/form_render.js]
                          [--screenshot] [--mode designer|runtime] [--theme-name v82|xp]

Output: tools/out/<sample>.html (+ <sample>.png with --screenshot; needs Microsoft Edge or Google Chrome installed).
Open the .html in a browser; add ?chrome=0 to the address to see only the form canvas, ?mode=runtime for the
'Предприятие' look (no sample values), ?theme=xp for the alternative palette."""
import argparse, glob, os, subprocess, sys, shutil, pathlib

HERE = os.path.dirname(os.path.abspath(__file__))

def browser():
    for p in (r'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe', r'C:\Program Files\Microsoft\Edge\Application\msedge.exe',
              r'C:\Program Files\Google\Chrome\Application\chrome.exe', r'C:\Program Files (x86)\Google\Chrome\Application\chrome.exe'):
        if os.path.isfile(p): return p
    for n in ('msedge', 'google-chrome', 'chromium', 'chrome'):
        w = shutil.which(n)
        if w: return w
    return None

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--theme', default=os.path.join(HERE, 'assets', 'form_theme.css'))
    ap.add_argument('--overlay', help='your theme.css: appended after the stock theme, overrides its rules')
    ap.add_argument('--render', default=os.path.join(HERE, 'assets', 'form_render.js'))
    ap.add_argument('--screenshot', action='store_true')
    ap.add_argument('--mode', default='designer', choices=['designer', 'runtime'])
    ap.add_argument('--theme-name', default='v82', choices=['v82', 'xp'])
    a = ap.parse_args()
    out = os.path.join(HERE, 'out'); os.makedirs(out, exist_ok=True)
    br = browser() if a.screenshot else None
    if a.screenshot and not br: sys.exit('no Edge/Chrome found: open the .html files manually')
    for d in sorted(glob.glob(os.path.join(HERE, 'samples', '*'))):
        name = os.path.basename(d)
        html = os.path.join(out, name + '.html')
        subprocess.check_call([sys.executable, os.path.join(HERE, 'rebuild_preview.py'), d, '-o', html,
                               '--theme', a.theme, '--render', a.render] + (['--theme-extra', a.overlay] if a.overlay else []))
        if br:
            png = os.path.join(out, name + '.png')
            url = pathlib.Path(html).resolve().as_uri() + '?chrome=0&mode=%s&theme=%s' % (a.mode, a.theme_name)
            subprocess.call([br, '--headless', '--disable-gpu', '--hide-scrollbars', '--screenshot=' + png,
                             '--window-size=1000,800', url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            print('screenshot', png, os.path.isfile(png))

if __name__ == '__main__':
    main()
