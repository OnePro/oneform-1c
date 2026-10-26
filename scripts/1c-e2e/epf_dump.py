#!/usr/bin/env python3
"""Run the introspection-dump launcher against an EPF form.  python target/epf_dump.py <epf> <tag> [FormName]"""
import os, subprocess, sys, time
sys.path.insert(0, "target")
epf = os.path.abspath(sys.argv[1]); tag = sys.argv[2]; form = sys.argv[3] if len(sys.argv) > 3 else "default"
out = os.path.abspath(f"target/dumps/{tag}.txt"); os.makedirs("target/dumps", exist_ok=True)
if os.path.exists(out): os.remove(out)
subprocess.run(["taskkill", "/F", "/IM", "1cv8.exe"], capture_output=True); time.sleep(1)
DB = r"C:\Users\egor\projects\oneform-1c\target\db-real"; EXE = r"C:\Program Files\1cv8\8.3.27.2342\bin\1cv8.exe"
launcher = os.path.abspath("target/dump-ws/dump.epf")
cmd = f'"{EXE}" ENTERPRISE /F "{DB}" /N "admin" /P "" /RunModeOrdinaryApplication /DisableUnsafeActionProtection /Execute "{launcher}" /C"{epf}|{out}|{form}" /DisableStartupDialogs /DisableStartupMessages'
cmdfile = os.path.abspath("target/task-dump.cmd"); open(cmdfile, "w", encoding="utf-8").write("@echo off\nchcp 65001 >nul\n" + cmd + "\n")
vbs = os.path.abspath("target/task-dump.vbs")
open(vbs, "w").write('CreateObject("WScript.Shell").Run """' + cmdfile + '""", 0, True' + chr(10))
subprocess.run(["schtasks", "/create", "/tn", "oneform-dump", "/tr", f'wscript.exe //B //Nologo "{vbs}"', "/sc", "once", "/st", "23:59", "/ru", "egor", "/it", "/f"], capture_output=True)
subprocess.run(["schtasks", "/run", "/tn", "oneform-dump"], capture_output=True)
t0 = time.time()
while time.time() - t0 < 90 and not os.path.exists(out): time.sleep(3)
time.sleep(1); subprocess.run(["taskkill", "/F", "/IM", "1cv8.exe"], capture_output=True)
if os.path.exists(out):
    n = len(open(out, encoding="utf-8-sig").read().splitlines()); print(f"DUMP [{tag}]: {n} lines -> {out}")
else:
    print(f"DUMP [{tag}]: NO OUTPUT (timeout)")
