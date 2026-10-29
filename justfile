set shell := ["sh", "-cu"]
set windows-shell := ["cmd.exe", "/D", "/C"]

bin := justfile_directory() / "target/release/oneform" + (if os() == "windows" { ".exe" } else { "" })

# Build the release binary (`target/release/oneform`).
build:
    cargo build --release --locked

# Run all unit, fixture, and golden integration tests.
test:
    cargo test --locked

# Run clippy lints across all targets.
clippy:
    cargo clippy --all-targets -- -D warnings

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

# `just release [VERSION]`: test, tag, push; CI builds Windows + macOS and publishes the GitHub release (VERSION bumps Cargo.toml first).
release version="":
    python scripts/release.py {{version}}

# Release with the Windows binary built on this machine (no macOS archive, no CI).
release-local version="":
    python scripts/release.py {{version}} --local

# Show what `just release` would do, without changing anything.
release-dry version="":
    python scripts/release.py {{version}} --dry-run --skip-tests

# Build the release archive for this machine into target/release-dist (no publishing).
dist: build
    python scripts/package_release.py "{{bin}}" {{ if os() == "windows" { "windows-x64" } else { "host" } }}

