"""Shared helpers of the live-1C scripts: paths from environment, interactive launch through a scheduled task.

Environment (see README.md):
  ONEFORM_BUH_DB     required  test infobase "Бух." (file base directory), user admin without password
  ONEFORM_1CV8       optional  path to 1cv8.exe (default: 8.3.27.2342 in Program Files)
  ONEFORM_LIVE_WORK  optional  working directory for generated files (default: <repo>/target/1c-live, not in git)
"""
import getpass
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
EXE = os.environ.get("ONEFORM_1CV8", r"C:\Program Files\1cv8\8.3.27.2342\bin\1cv8.exe")
WORK = os.path.abspath(os.environ.get("ONEFORM_LIVE_WORK", os.path.join(REPO, "target", "1c-live")))
os.makedirs(WORK, exist_ok=True)


def db_path():
    v = os.environ.get("ONEFORM_BUH_DB")
    if not v:
        sys.exit("set ONEFORM_BUH_DB to the directory of the test infobase 'Бух.' (see scripts/1c-e2e/live/README.md)")
    return v


def kill_1c():
    """One 1C instance at a time: a leftover 1cv8 keeps the file base locked."""
    subprocess.run(["taskkill", "/F", "/IM", "1cv8.exe"], capture_output=True)
    time.sleep(1)


def enterprise_cmd(epf, param):
    """Command line that starts the infobase as user admin, runs `epf` (launcher) with parameter `param`."""
    return (f'"{EXE}" ENTERPRISE /F "{db_path()}" /N admin /P "" /RunModeOrdinaryApplication '
            f'/DisableUnsafeActionProtection /Execute "{epf}" /C"{param}" '
            f'/DisableStartupDialogs /DisableStartupMessages')


def run_in_session(task, cmdline, name):
    """Run `cmdline` in the interactive desktop session (1C UI needs it; plain subprocess has no desktop).
    Writes WORK/<name>.cmd and WORK/<name>.vbs (hidden window), registers and starts scheduled task `task`."""
    cf = os.path.join(WORK, name + ".cmd")
    with open(cf, "w", encoding="utf-8") as f:
        f.write("@echo off\nchcp 65001 >nul\n" + cmdline + "\n")
    vbs = os.path.join(WORK, name + ".vbs")
    with open(vbs, "w") as f:
        f.write('CreateObject("WScript.Shell").Run """' + cf + '""", 0, True\n')
    subprocess.run(["schtasks", "/create", "/tn", task, "/tr", f'wscript.exe //B //Nologo "{vbs}"', "/sc", "once",
                    "/st", "23:59", "/ru", getpass.getuser(), "/it", "/f"], capture_output=True)
    subprocess.run(["schtasks", "/run", "/tn", task], capture_output=True)


def screenshot(png, task="oneform-shot", name="run-shot"):
    """Whole-desktop screenshot through scr.ps1 (minimises other windows, maximises 1C)."""
    scr = os.path.join(HERE, "scr.ps1")
    run_in_session(task, f'powershell -NoProfile -ExecutionPolicy Bypass -File "{scr}" "{png}" > "{png}.log" 2>&1', name)
