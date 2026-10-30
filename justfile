set shell := ["sh", "-cu"]
set windows-shell := ["cmd.exe", "/D", "/C"]

# Python 3 interpreter: `python` on Windows, `python3` on macOS and Linux.
py := if os() == "windows" { "python" } else { "python3" }

# Platform name of this machine for release archives: windows-x64, macos-arm64, macos-x64, linux-x64, linux-arm64.
os_name := if os() == "windows" { "windows" } else if os() == "macos" { "macos" } else { "linux" }
cpu := if arch() == "aarch64" { "arm64" } else { "x64" }
platform := os_name + "-" + (if os() == "windows" { "x64" } else { cpu })

bin := justfile_directory() / "target/release/oneform" + (if os() == "windows" { ".exe" } else { "" })

# Build the release binary (`target/release/oneform`).
build:
    cargo build --release --locked

# Run all unit, fixture, and golden integration tests.
test:
    cargo test --locked

# Run clippy lints across all targets.
clippy:
    cargo clippy --locked --all-targets -- -D warnings

# Extract all EPF/ERF and Ext/Form.bin configuration objects under DIR (default: .) and build DIR/.oneform/index.html.
extract dir=".":
    "{{bin}}" extract "{{dir}}"

# Pack edited BSL modules / FormStream.bin / templates from src/ back into Form.bin / EPF files.
pack dir=".":
    "{{bin}}" pack "{{dir}}"

# Rebuild FormPreview.html and DIR/.oneform/index.html across DIR.
preview dir=".":
    "{{bin}}" preview "{{dir}}"

# Verify that committed src/ trees match their binary sources (exit 3 on drift).
check dir=".":
    "{{bin}}" check "{{dir}}"

# `just release [VERSION]`: test, tag, push; CI builds Windows, macOS and Linux and publishes the GitHub release (VERSION bumps Cargo.toml first).
release version="":
    {{py}} scripts/release.py {{version}}

# Release without CI: this machine built here, macOS cross-built with Zig unless this is a Mac (needs zig + cargo-zigbuild; `--no-mac` via the script builds only this machine).
release-local version="":
    {{py}} scripts/release.py {{version}} --local

# Show what `just release` would do, without changing anything.
release-dry version="":
    {{py}} scripts/release.py {{version}} --dry-run --skip-tests

# Build the release archive for this machine into target/release-dist (no publishing).
dist: build
    {{py}} scripts/package_release.py "{{bin}}" {{platform}}

# Cross-build and package macOS (arm64 + x64) without a Mac, with Zig (cargo install cargo-zigbuild; pip install ziglang).
dist-mac arch="both":
    {{py}} scripts/build_mac.py --arch {{arch}}
