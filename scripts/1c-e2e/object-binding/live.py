#!/usr/bin/env python3
"""python live.py <Вид.Имя> <Форма> <mode:first|new> <out.txt> [shot.png] [Реквизит=s:..;..]  (База Бух.)"""
import os, subprocess, sys, time
here = os.path.dirname(os.path.abspath(__file__))
spec, form, mode, out = sys.argv[1], sys.argv[2], sys.argv[3], os.path.abspath(sys.argv[4])
shot = os.path.abspath(sys.argv[5]) if len(sys.argv) > 5 and sys.argv[5] != "-" else None
vals = sys.argv[6] if len(sys.argv) > 6 else ""
DB = os.environ.get("A1_DB", r"C:\Users\egor\Documents\1с\bases\dental_vlad_buh")
EXE = r"C:\Program Files\1cv8\8.3.27.2342\bin\1cv8.exe"
EPF = os.path.join(here, "a1-launcher.epf")
if os.path.exists(out): os.remove(out)
if shot and os.path.exists(shot): os.remove(shot)
subprocess.run(["taskkill", "/F", "/IM", "1cv8.exe"], capture_output=True); time.sleep(1)
cmd = f'"{EXE}" ENTERPRISE /F "{DB}" /N admin /P "" /RunModeOrdinaryApplication /DisableUnsafeActionProtection /Execute "{EPF}" /C"{spec}|{out}|{form}|{mode}|{vals}" /DisableStartupDialogs /DisableStartupMessages'
def task(name, cmdline, vbsname):
    cf = os.path.join(here, vbsname + ".cmd"); open(cf, "w", encoding="utf-8").write("@echo off\nchcp 65001 >nul\n" + cmdline + "\n")
    vbs = os.path.join(here, vbsname + ".vbs")
    open(vbs, "w").write('CreateObject("WScript.Shell").Run """' + cf + '""", 0, True\n')
    subprocess.run(["schtasks", "/create", "/tn", name, "/tr", f'wscript.exe //B //Nologo "{vbs}"', "/sc", "once", "/st", "23:59", "/ru", "egor", "/it", "/f"], capture_output=True)
    subprocess.run(["schtasks", "/run", "/tn", name], capture_output=True)
task("oneform-a1", cmd, "run-a1")
t0 = time.time(); status = "TIMEOUT"
while time.time() - t0 < 150:
    time.sleep(2)
    if os.path.exists(out):
        try: txt = open(out, encoding="utf-8-sig", errors="replace").read()
        except OSError: continue
        if "## READY" in txt or "LAUNCH-ERROR" in txt: status = "READY" if "## READY" in txt else "ERROR"; break
if shot and status == "READY":
    time.sleep(5)
    scr = os.path.join(here, "scr.ps1")
    task("oneform-a1shot", f'powershell -NoProfile -ExecutionPolicy Bypass -File "{os.path.abspath(scr)}" "{shot}" > "{shot}.log" 2>&1', "run-a1shot")
    time.sleep(8)
subprocess.run(["taskkill", "/F", "/IM", "1cv8.exe"], capture_output=True)
print(f"{status} ({time.time()-t0:.0f}s) -> {out}" + (f" shot={os.path.exists(shot)}" if shot else ""))
