#!/usr/bin/env python3
"""Cross-build the macOS binaries on any host with Zig (no Mac, no Xcode) and package them.

  python scripts/build_mac.py [--arch arm64|x64|both] [--out DIR]

Needs: rustup, `cargo-zigbuild` (cargo install cargo-zigbuild --locked) and Zig (`zig` on PATH, or `pip install ziglang`).
Builds `aarch64-apple-darwin` (macos-arm64) and `x86_64-apple-darwin` (macos-x64), then runs scripts/package_release.py.
Prints the archive paths, one per line. The binaries are NOT run here: tests run only in CI on a real Mac.
Zig adds an ad-hoc code signature; macOS may still need `xattr -d com.apple.quarantine ./oneform` after download.
"""
import argparse, os, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {"arm64": ("aarch64-apple-darwin", "macos-arm64"), "x64": ("x86_64-apple-darwin", "macos-x64")}


def find_zig() -> str:
    z = shutil.which("zig")
    if z:
        return z
    try:
        import ziglang  # type: ignore
        p = Path(ziglang.__file__).parent / ("zig.exe" if os.name == "nt" else "zig")
        if p.exists():
            return str(p)
    except ImportError:
        pass
    sys.exit("Zig not found: install it (https://ziglang.org/download/) or run `pip install ziglang`")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--arch", choices=["arm64", "x64", "both"], default="both")
    ap.add_argument("--out", default=str(ROOT / "target" / "release-dist"))
    a = ap.parse_args()
    if not shutil.which("cargo-zigbuild"):
        sys.exit("cargo-zigbuild not found: cargo install cargo-zigbuild --locked")
    env = dict(os.environ, CARGO_ZIGBUILD_ZIG_PATH=find_zig())
    for arch in (["arm64", "x64"] if a.arch == "both" else [a.arch]):
        target, name = TARGETS[arch]
        subprocess.run(["rustup", "target", "add", target], cwd=ROOT, check=True, stdout=sys.stderr)
        r = subprocess.run(["cargo", "zigbuild", "--release", "--locked", "--target", target], cwd=ROOT, env=env, stdout=sys.stderr)
        if r.returncode != 0:
            sys.exit(f"build failed for {target}")
        binary = ROOT / "target" / target / "release" / "oneform"
        r = subprocess.run([sys.executable, str(ROOT / "scripts" / "package_release.py"), str(binary), name, "--out", a.out],
                           cwd=ROOT, capture_output=True, text=True)
        if r.returncode != 0:
            sys.exit(r.stderr or r.stdout)
        print(r.stdout.strip())


if __name__ == "__main__":
    main()
