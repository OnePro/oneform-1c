#!/usr/bin/env python3
"""Release oneform: tag, push, let GitHub Actions build Windows, macOS and Linux, publish the GitHub release.

  python scripts/release.py [patch|minor|major|X.Y.Z[-pre]] [--local [--no-mac]] [--dry-run] [--skip-tests]

* Version: `patch`, `minor` or `major` is computed from the version of Cargo.toml (SemVer); `X.Y.Z[-pre]` is taken as is.
  Without an argument the version of Cargo.toml is released, unless its tag already exists: then the script refuses and
  lists the three next versions.
* Order: gh is logged in, branch is main, working tree is clean, tag is absent locally and on origin -> `check_changelog.py check`
  -> `[Unreleased]` in CHANGELOG.md must hold entries (`prepare --dry-run` shows what moves) -> `cargo test --locked` and
  `cargo clippy` (unless --skip-tests) -> bump Cargo.toml / Cargo.lock -> `check_changelog.py prepare` (moves [Unreleased] to
  [X.Y.Z]) -> `check_changelog.py release` (must pass) -> ONE commit `release X.Y.Z` (Cargo.toml, Cargo.lock, CHANGELOG.md)
  -> tag `vX.Y.Z` -> push main and the tag. A failure before the commit restores the three files (`git checkout HEAD --`).
  A release without a changelog entry is impossible; there is no flag to skip this.
* Default: pushes main and the tag `v<version>`; the workflow `.github/workflows/release.yml` builds
  windows-x64, macos-arm64, macos-x64, linux-x64 and publishes the release with archives and sha256 files. The script waits for the run.
* --local: no CI; builds the binary of this machine (windows-x64, macos-arm64/x64, linux-x64/arm64 by platform.system() and
  platform.machine()) and, unless the host is a Mac, the macOS binaries with Zig (scripts/build_mac.py; not run or tested on
  a Mac), and publishes the release with these archives and the notes of CHANGELOG.md (`--notes-file`); --no-mac builds only the host archive. Works on Windows, macOS, Linux.
* --dry-run: prints what would happen, changes nothing.
"""
import argparse, json, platform, re, subprocess, sys, time
from pathlib import Path

from check_changelog import semver_key

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = "release.yml"


def run(cmd, check=True, capture=False, dry=False, **kw):
    print("+", " ".join(cmd), flush=True)
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


def bump(current: str, kind: str) -> str:
    major, minor, patch = (int(x) for x in current.split("-")[0].split("."))
    return {"major": f"{major + 1}.0.0", "minor": f"{major}.{minor + 1}.0", "patch": f"{major}.{minor}.{patch + 1}"}[kind]


def tag_exists(tag: str) -> bool:
    return bool(run(["git", "tag", "-l", tag], capture=True)) or bool(run(["git", "ls-remote", "--tags", "origin", tag], capture=True))


def changelog(*args: str, capture: bool = False) -> str:
    """Run scripts/check_changelog.py; its messages (Russian, UTF-8) go to the console, or its stdout is returned."""
    cmd = [sys.executable, "scripts/check_changelog.py", *args]
    print("+", " ".join(cmd), flush=True)
    r = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.PIPE if capture else None, encoding="utf-8" if capture else None)
    if r.returncode != 0:
        sys.exit(f"CHANGELOG.md does not allow this release (check_changelog.py {' '.join(args)} exited with {r.returncode})")
    return r.stdout if capture else ""


def restore_files() -> None:
    subprocess.run(["git", "checkout", "HEAD", "--", "Cargo.toml", "Cargo.lock", "CHANGELOG.md"], cwd=ROOT)
    print("Cargo.toml, Cargo.lock and CHANGELOG.md restored: nothing was committed", file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("version", nargs="?", help="patch | minor | major | X.Y.Z[-pre]; empty: the version of Cargo.toml")
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

    current = cargo_version()
    if a.version in ("patch", "minor", "major"):
        version = bump(current, a.version)
        print(f"{a.version}: {current} -> {version}")
    elif a.version:
        version = a.version
    elif tag_exists(f"v{current}"):
        sys.exit("\n".join([
            f"tag v{current} already exists: the version of Cargo.toml is already released. Choose the next version:",
            f"  python scripts/release.py patch   # {bump(current, 'patch')}",
            f"  python scripts/release.py minor   # {bump(current, 'minor')}",
            f"  python scripts/release.py major   # {bump(current, 'major')}",
            "  python scripts/release.py X.Y.Z   # an explicit version",
            "(just release patch|minor|major|X.Y.Z)"]))
    else:
        version = current
    if not re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?", version):
        sys.exit(f"bad version: {version}")
    if version != current and semver_key(version) <= semver_key(current):
        sys.exit(f"version {version} is not greater than the current {current}")
    tag = f"v{version}"
    if run(["git", "tag", "-l", tag], capture=True):
        sys.exit(f"tag {tag} already exists")
    if run(["git", "ls-remote", "--tags", "origin", tag], capture=True):
        sys.exit(f"tag {tag} already exists on origin")

    # the changelog rule: no release without a list of changes
    changelog("check")
    changelog("prepare", version, "--dry-run")

    if not a.skip_tests:
        run(["cargo", "test", "--locked"], dry=dry)
        run(["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"], dry=dry)

    if dry:
        if version != current:
            print(f"bump version {current} -> {version} in Cargo.toml and Cargo.lock")
        print(f"move [Unreleased] to [{version}] in CHANGELOG.md (shown above)")
        print(f"commit `release {version}` (Cargo.toml, Cargo.lock, CHANGELOG.md)")
    host = host_platform()
    with_mac = not a.no_mac and not host.startswith("macos")
    archives = []
    notes_file = ROOT / "target" / f"release-notes-{version}.md"
    try:
        if not dry:
            if version != current:
                print(f"bump version {current} -> {version}")
                text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
                text = re.sub(r'^version\s*=\s*"[^"]+"', f'version = "{version}"', text, count=1, flags=re.M)
                with open(ROOT / "Cargo.toml", "w", encoding="utf-8", newline="\n") as f:
                    f.write(text)
                run(["cargo", "update", "-p", "oneform", "--offline"])
            changelog("prepare", version)
            changelog("release", version)
            if a.local:
                notes_file.parent.mkdir(parents=True, exist_ok=True)
                with open(notes_file, "w", encoding="utf-8", newline="\n") as f:
                    f.write(changelog("notes", version, capture=True))
                run(["cargo", "build", "--release", "--locked"])
                exe = ROOT / "target" / "release" / ("oneform.exe" if host.startswith("windows") else "oneform")
                archives.append(run([sys.executable, "scripts/package_release.py", str(exe), host], capture=True))
                if with_mac:
                    out = run([sys.executable, "scripts/build_mac.py"], capture=True)
                    archives += [l.strip() for l in out.splitlines() if l.strip()]
            run(["git", "add", "Cargo.toml", "Cargo.lock", "CHANGELOG.md"])
            run(["git", "commit", "-m", f"release {version}"])
    except BaseException:
        restore_files()
        raise

    if a.local:
        if dry:
            run(["cargo", "build", "--release", "--locked"], dry=True)
            archives = [f"<{host} archive>"] + (["<macos-arm64 archive>", "<macos-x64 archive>"] if with_mac else [])
        files = []
        for x in archives:
            files += [x, x + ".sha256"]
        run(["git", "tag", tag], dry=dry)
        run(["git", "push", "origin", "main", tag], dry=dry)
        run(["gh", "release", "create", tag, *files, "--title", f"oneform {version}", "--notes-file", str(notes_file)], dry=dry)
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
