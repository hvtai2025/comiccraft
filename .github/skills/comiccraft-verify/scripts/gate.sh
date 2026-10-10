#!/usr/bin/env bash
# Run the ComicCraft gate: `cargo xtask ci`, saved as evidence.
#
# The gate runs fmt, clippy, test, assets, brands, layers and wasm in order and stops at
# the first failure. It takes a long time — run it with a generous wait, or in the
# background, and read the tail when it finishes.
#
# Writes docs/ci-<short-sha>.md and exits with cargo's status.
#
# The log is Markdown on purpose: `cargo xtask brands` scans every non-Markdown text file
# under docs/, and a build log contains upstream test names such as
# `an_illustrator_ai_file_opens_from_its_editing_data`. A .txt log in docs/ therefore
# fails the brands gate. Do not change this extension.
#
# Usage: bash gate.sh [repo-root]

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$SCRIPT_DIR/../../../.." && pwd)}"
cd "$ROOT" || { echo "cannot enter $ROOT"; exit 1; }

# rustup installs to ~/.cargo/bin and only a login shell has it on PATH. Source it here so the
# toolchain check reflects what is actually installed rather than how this script was invoked —
# without this, running from a non-login shell reports a false BLOCKED on the apt rustc.
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi

if [ ! -f Cargo.toml ] || ! grep -q '^\[workspace\]' Cargo.toml 2>/dev/null; then
  echo "This is not the fork checkout (no workspace Cargo.toml). Run M0 first."
  exit 3
fi

REQUIRED="$(grep -E '^rust-version' Cargo.toml | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
if command -v rustc >/dev/null 2>&1; then
  HAVE="$(rustc --version | awk '{print $2}')"
  if [ "$(printf '%s\n%s\n' "$REQUIRED" "$HAVE" | sort -V | head -n1)" != "$REQUIRED" ]; then
    echo "rustc $HAVE is older than the required $REQUIRED; the manifest will not parse."
    echo "Install $REQUIRED+ and retry."
    exit 3
  fi
fi

SHA="$(git rev-parse --short HEAD 2>/dev/null || echo nogit)"
OUT="docs/ci-${SHA}.md"
mkdir -p docs

echo "Running: cargo xtask ci"
echo "Output:  $OUT"
START="$(date +%s)"

# xtask ci stops at the first failing step, so the log names the failure.
set -o pipefail
cargo xtask ci 2>&1 | tee "$OUT"
STATUS="${PIPESTATUS[0]}"

ELAPSED="$(( $(date +%s) - START ))"
echo
echo "elapsed: ${ELAPSED}s   exit: $STATUS"

if [ "$STATUS" -eq 0 ]; then
  echo "GATE PASS"
else
  echo "GATE FAIL — first failing step:"
  grep -nE '^(\s*)(cargo xtask|error:|warning: unused|Step )' "$OUT" | head -n 12 || tail -n 20 "$OUT"
fi
exit "$STATUS"
