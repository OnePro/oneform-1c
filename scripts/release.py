#!/usr/bin/env python3
"""Release oneform: tag, push, let GitHub Actions build Windows, macOS and Linux, publish the GitHub release.

  python scripts/release.py [VERSION] [--local [--no-mac]] [--dry-run] [--skip-tests]

* Without VERSION the version of Cargo.toml is released. With VERSION (for example 0.2.0) Cargo.toml and
  Cargo.lock are bumped and committed first.
* Checks: `gh` is logged in, the branch is main, the working tree is clean, the tag does not exist, `cargo test --locked`
  and `cargo clippy` pass (unless --skip-tests).
* Default: pushes main and the tag `v<version>`; the workflow `.github/workflows/release.yml` builds
  windows-x64, macos-arm64, macos-x64, linux-x64 and publishes the release with archives and sha256 files. The script waits for the run.
* --local: no CI; builds the binary of this machine (windows-x64, macos-arm64/x64, linux-x64/arm64 by platform.system() and
  platform.machine()) and, unless the host is a Mac, the macOS binaries with Zig (scripts/build_mac.py; not run or tested on
  a Mac), and publishes the release with these archives; --no-mac builds only the host archive. Works on Windows, macOS, Linux.
* --dry-run: prints what would happen, changes nothing.
"""
import argparse, json, platform, re, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = "release.yml"


def run(cmd, check=True, capture=False, dry=False, **kw):
    print("+", " ".join(cmd))
    if dry:
        return ""
    r = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=capture, **kw)
    if check and r.returncode != 0:
        sys.exit(f"command failed ({r.returncode}): {' '.join(cmd)}\n{(r.stdout or '') + (r.stderr or '')}")
    return (r.stdout or "").strip() if capture else ""


def host_platform() -> str:
    """Platform name of this machine for the archive name: windows-x64, macos-arm64, macos-x64, linux-x64, linux-arm64."""
    os_name = {"Windows": "windows", "Darwin": "macos", "Linux": "linux"}.get(platform.system(), platform.system().lower())
    arch = "arm64" if platform.machine().lower() in ("arm64", "aarch64") and os_name != "windows" else "x64"
    return f"{os_name}-{arch}"


def cargo_version() -> str:
    m = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    return m.group(1)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("version", nargs="?")
    ap.add_argument("--local", action="store_true")
    ap.add_argument("--no-mac", action="store_true", help="with --local: skip the Zig macOS build (always skipped on a Mac)")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--skip-tests", action="store_true")
    a = ap.parse_args()
    dry = a.dry_run

    if subprocess.run(["gh", "auth", "status"], cwd=ROOT, capture_output=True).returncode != 0:
        sys.exit("gh is not logged in: run `gh auth login`")
    branch = run(["git", "rev-parse", "--abbrev-ref", "HEAD"], capture=True)
    if branch != "main":
        sys.exit(f"release only from main (current branch: {branch})")
    if run(["git", "status", "--porcelain"], capture=True):
        sys.exit("working tree is not clean: commit or stash changes first")

    version = a.version or cargo_version()
    if not re.fullmatch(r"\d+\.\d+\.\d+([-.][0-9A-Za-z.]+)?", version):
        sys.exit(f"bad version: {version}")
    tag = f"v{version}"
    if run(["git", "tag", "-l", tag], capture=True):
        sys.exit(f"tag {tag} already exists")
    if run(["git", "ls-remote", "--tags", "origin", tag], capture=True):
        sys.exit(f"tag {tag} already exists on origin")

    if not a.skip_tests:
        run(["cargo", "test", "--locked"], dry=dry)
        run(["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"], dry=dry)

    if version != cargo_version():
        print(f"bump version {cargo_version()} -> {version}")
        if not dry:
            text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
            text = re.sub(r'^version\s*=\s*"[^"]+"', f'version = "{version}"', text, count=1, flags=re.M)
            with open(ROOT / "Cargo.toml", "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        run(["cargo", "update", "-p", "oneform", "--offline"], dry=dry)
        run(["git", "add", "Cargo.toml", "Cargo.lock"], dry=dry)
        run(["git", "commit", "-m", f"release {version}"], dry=dry)

    if a.local:
        run(["cargo", "build", "--release", "--locked"], dry=dry)
        host = host_platform()
        exe = ROOT / "target" / "release" / ("oneform.exe" if host.startswith("windows") else "oneform")
        with_mac = not a.no_mac and not host.startswith("macos")
        archives = []
        if dry:
            archives = [f"<{host} archive>"] + (["<macos-arm64 archive>", "<macos-x64 archive>"] if with_mac else [])
        else:
            archives.append(run([sys.executable, "scripts/package_release.py", str(exe), host], capture=True))
            if with_mac:
                out = run([sys.executable, "scripts/build_mac.py"], capture=True)
                archives += [l.strip() for l in out.splitlines() if l.strip()]
        files = []
        for x in archives:
            files += [x, x + ".sha256"]
        run(["git", "tag", tag], dry=dry)
        run(["git", "push", "origin", "main", tag], dry=dry)
        run(["gh", "release", "create", tag, *files, "--title", f"oneform {version}", "--generate-notes"], dry=dry)
        print(f"released {tag} ({host if not with_mac else host + ' + macOS built with Zig, not tested on a Mac'})")
        return

    run(["git", "tag", tag], dry=dry)
    run(["git", "push", "origin", "main", tag], dry=dry)
    if dry:
        print("dry run: CI would build windows-x64, macos-arm64, macos-x64, linux-x64 and publish the release")
        return
    print("waiting for the workflow run ...")
    run_id = ""
    for _ in range(30):
        out = run(["gh", "run", "list", "--workflow", WORKFLOW, "--branch", tag, "--limit", "1", "--json", "databaseId"],
                  capture=True, check=False)
        try:
            runs = json.loads(out or "[]")
        except json.JSONDecodeError:
            runs = []
        if runs:
            run_id = str(runs[0]["databaseId"])
            break
        time.sleep(5)
    if not run_id:
        sys.exit(f"the workflow run for {tag} did not start: see the Actions tab")
    rc = subprocess.run(["gh", "run", "watch", run_id, "--exit-status"], cwd=ROOT).returncode
    if rc != 0:
        sys.exit(f"workflow failed: gh run view {run_id} --log-failed")
    print(run(["gh", "release", "view", tag, "--json", "url", "-q", ".url"], capture=True))


if __name__ == "__main__":
    main()
