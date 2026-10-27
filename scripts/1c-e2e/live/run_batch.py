#!/usr/bin/env python3
"""Batch launcher (std_codes_module.bsl): runtime `Данные` of every control and table column for many forms.

  python run_batch.py <listfile> <outfile> [timeout_s]
listfile lines: <Вид>.<Имя>|<Форма>[|<ИмяГлавногоРеквизита>]   (Вид: Справочники, Документы, ПланыСчетов, ...)
outfile: `## <Вид>.<Имя>|<Форма>`, then `Имя|Данные` per control and `Имя.Колонка|Данные` per table column, `## END`.
Used to reconcile the binding codes of the catalog with the runtime (519 elements, task A1).
"""
import os
import subprocess
import sys
import time

from onec import HERE, WORK, enterprise_cmd, kill_1c, run_in_session

lst, out = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
timeout = int(sys.argv[3]) if len(sys.argv) > 3 else 500
epf = os.path.join(WORK, "std-codes.epf")
if not os.path.exists(epf):
    subprocess.run([sys.executable, os.path.join(HERE, "build_launcher.py"), os.path.join(HERE, "std_codes_module.bsl"), epf], check=True)
if os.path.exists(out):
    os.remove(out)
kill_1c()
run_in_session("oneform-stdcodes", enterprise_cmd(epf, f"batch:{lst}|{out}|-"), "task-run")
t0 = time.time()
last = (0, t0)
status = "TIMEOUT"
while time.time() - t0 < timeout:
    time.sleep(2)
    if os.path.exists(out):
        try:
            txt = open(out, encoding="utf-8-sig", errors="replace").read()
        except OSError:
            continue
        if "## END" in txt or "LAUNCH-ERROR" in txt:
            status = "DONE"
            break
        if len(txt) != last[0]:
            last = (len(txt), time.time())
        elif time.time() - last[1] > 90:
            status = "STALLED"
        
