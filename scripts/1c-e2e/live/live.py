#!/usr/bin/env python3
"""Open one form in the test infobase and dump the runtime properties of every control.

  python live.py <Вид.Имя> <Форма> first|new <out.txt> [shot.png|-] [Реквизит=s:..;..] [Панель.Страница]

<Вид.Имя>   object of the configuration (Справочники.X, Документы.X), or an external file:
            ВнешнийОтчет.<путь к .erf> / ВнешняяОбработка.<путь к .epf> (the form is taken from the created object)
first|new   open the first existing object or create a new one
out.txt     lines `pre:<element>.<property>=<Type>|<value>` (`Данные` = binding, `Значение` = what the field shows),
            ends with `## READY`
shot.png    optional screenshot of the desktop (scheduled task `oneform-shot`)
Панель.Страница  optional: make this page of the panel current before the dump (and the screenshot)
Needs ONEFORM_BUH_DB (see README.md), an interactive Windows session, a built launcher (build_launcher.py).
"""
import os
import subprocess
import sys
import time

from onec import WORK, enterprise_cmd, kill_1c, run_in_session, screenshot

spec, form, mode, out = sys.argv[1], sys.argv[2], sys.argv[3], os.path.abspath(sys.argv[4])
shot = os.path.abspath(sys.argv[5]) if len(sys.argv) > 5 and sys.argv[5] != "-" else None
vals = sys.argv[6] if len(sys.argv) > 6 else ""
page = sys.argv[7] if len(sys.argv) > 7 else ""
epf = os.path.join(WORK, "a1-launcher.epf")
if not os.path.exists(epf):
    subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "build_launcher.py")], check=True)
for p in (out, shot):
    if p and os.path.exists(p):
        os.remove(p)
kill_1c()
run_in_session("oneform-a1", enterprise_cmd(epf, f"{spec}|{out}|{form}|{mode}|{vals}|{page}"), "run-a1")
t0 = time.time()
status = "TIMEOUT"
while time.time() - t0 < 150:
    time.sleep(2)
    if os.path.exists(out):
        try:
            txt = open(out, encoding="utf-8-sig", errors="replace").read()
        except OSError:
            continue
        if "## READY" in txt or "LAUNCH-ERROR" in txt:
            status = "READY" if "## READY" in txt else "ERROR"
            break
if shot and status == "READY":
    time.sleep(5)  # let the form paint
    screenshot(shot, "oneform-shot", "run-a1shot")
    time.sleep(8)
kill_1c()
print(f"{status} ({time.time()-t0:.0f}s) -> {out}" + (f" shot={os.path.exists(shot)}" if shot else ""))
