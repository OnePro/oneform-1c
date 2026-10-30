#!/usr/bin/env python3
"""Package a built oneform binary into a release archive.

Usage: python scripts/package_release.py <binary> <platform-name> [--out DIR]

Archive (zip on Windows, tar.gz elsewhere): `oneform[.exe]`, `README.md`, `INSTALL.txt`, `VERSION.txt`,
`docs/agent-guide.md`, `docs/experimental-form-editing.md`, `skills/` (the agent skills). Name: `oneform-<version>-<platform-name>.<ext>`.
Prints the archive path. Version is read from Cargo.toml.
"""
import argparse, hashlib, os, re, shutil, sys, tarfile, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def cargo_version() -> str:
    m = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    if not m:
        sys.exit("version not found in Cargo.toml")
    return m.group(1)


INSTALL = r"""oneform {version} ({platform})

1. Put `{exe}` in a folder that is on PATH (or call it by full path).
2. Check: `{exe} --help`.
3. Agent skills are in `skills/`: copy them to your agent's skills folder (for example %USERPROFILE%\.agents\skills on Windows,
   ~/.agents/skills on macOS). The guide for agents is `docs/agent-guide.md` (stable mode); writing the form layout is
   experimental and described in `docs/experimental-form-editing.md` (it needs the flag `--experimental`).
{mac}
Status of features (stable / experimental): see the first section of `docs/agent-guide.md` and `docs/experimental-form-editing.md`.
"""

MAC = """
macOS: the binary is not signed. If macOS refuses to run it:
   xattr -d com.apple.quarantine ./oneform && chmod +x ./oneform
"""


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("binary")
    ap.add_argument("platform")
    ap.add_argument("--out", default=str(ROOT / "target" / "release-dist"))
    a = ap.parse_args()
    binary = Path(a.binary)
    if not binary.is_file():
        sys.exit(f"binary not found: {binary}")
    version = cargo_version()
    is_win = a.platform.startswith("windows")
    exe = "oneform.exe" if is_win else "oneform"
    base = f"oneform-{version}-{a.platform}"
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    stage = out / base
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(binary, stage / exe)
    shutil.copy2(ROOT / "README.md", stage / "README.md")
    (stage / "docs").mkdir()
    for doc in ("agent-guide.md", "experimental-form-editing.md"):
        shutil.copy2(ROOT / "docs" / doc, stage / "docs" / doc)
    shutil.copytree(ROOT / "skills", stage / "skills")
    (stage / "VERSION.txt").write_text(f"oneform {version}\nplatform: {a.platform}\n", encoding="utf-8")
    (stage / "INSTALL.txt").write_text(
        INSTALL.format(version=version, platform=a.platform, exe=exe, mac="" if is_win else MAC), encoding="utf-8")
    if is_win:
        archive = out / f"{base}.zip"
        archive.unlink(missing_ok=True)
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
            for d, _, fs in os.walk(stage):
                for n in sorted(fs):
                    p = Path(d) / n
                    z.write(p, Path(base) / p.relative_to(stage))
    else:
        archive = out / f"{base}.tar.gz"
        archive.unlink(missing_ok=True)
        with tarfile.open(archive, "w:gz") as t:
            def flt(ti: tarfile.TarInfo) -> tarfile.TarInfo:
                if ti.name.endswith("/" + exe):
                    ti.mode = 0o755
                return ti
            t.add(stage, arcname=base, filter=flt)
    shutil.rmtree(stage)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    (out / f"{archive.name}.sha256").write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(archive)


if __name__ == "__main__":
    main()
