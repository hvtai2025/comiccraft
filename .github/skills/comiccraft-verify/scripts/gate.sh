#!/usr/bin/env bash
# Run the ComicCraft gate: `npm run verify`, saved as evidence.
#
# The gate runs the type check, ESLint (including the boundary rules), the layering unit
# test, `node --test`, the schema-fixture validation and the server smoke test, in that
# order, and stops at the first failure. On this stack it takes seconds to low minutes —
# there is no build step and no compiled output.
#
# Writes docs/ci-<short-sha>.md and exits with npm's status.
#
# The log is Markdown on purpose, to match the existing evidence files under docs/. Keep
# the extension: the status and verify skills read `docs/ci-*.md`, and a mixed set of
# extensions makes "which file is the latest gate run?" ambiguous.
#
# Usage: bash gate.sh [repo-root]

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$SCRIPT_DIR/../../../.." && pwd)}"
cd "$ROOT" || { echo "cannot enter $ROOT"; exit 1; }

if [ ! -f package.json ]; then
  echo "This is not the ComicCraft checkout (no package.json). Run M0 first."
  exit 3
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node is not installed. The gate needs Node 22 or newer."
  exit 3
fi

NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
if [ "$NODE_MAJOR" -lt 22 ]; then
  echo "node $(node --version) is older than the required 22; the built-in test runner and"
  echo "the standard-library APIs this project relies on are not all present."
  echo "Install Node 22+ and retry."
  exit 3
fi

if [ ! -d node_modules ]; then
  echo "node_modules is missing — run: npm install"
  exit 3
fi

SHA="$(git rev-parse --short HEAD 2>/dev/null || echo nogit)"
OUT="docs/ci-${SHA}.md"
mkdir -p docs

echo "Running: npm run verify"
echo "Output:  $OUT"
START="$(date +%s)"

# verify stops at the first failing step, so the log names the failure.
set -o pipefail
npm run verify 2>&1 | tee "$OUT"
STATUS="${PIPESTATUS[0]}"

ELAPSED="$(( $(date +%s) - START ))"
echo
echo "elapsed: ${ELAPSED}s   exit: $STATUS"

if [ "$STATUS" -eq 0 ]; then
  echo "GATE PASS"
else
  echo "GATE FAIL — first failing step:"
  grep -nE '^(> |npm error|error|✖|not ok |FAIL|AssertionError)' "$OUT" | head -n 12 || tail -n 20 "$OUT"
fi
exit "$STATUS"
