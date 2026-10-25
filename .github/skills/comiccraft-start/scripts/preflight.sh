#!/usr/bin/env bash
# ComicCraft preflight: decide whether the team can start, and at which stage.
#
# Reports PASS / WARN / FAIL per check and ends with a verdict:
#   READY       start the milestone
#   NEEDS-SETUP M0's repository layout has not been created yet; start M0
#   BLOCKED     a FAIL must be fixed first (the FAIL lines say what)
#
# Read-only: creates nothing, changes nothing.
#
# Usage: bash preflight.sh [repo-root]

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$SCRIPT_DIR/../../../.." && pwd)}"
cd "$ROOT" || { echo "FAIL cannot enter $ROOT"; exit 1; }

PASS=0; WARN=0; FAIL=0
pass() { printf '  \033[32mPASS\033[0m %s\n' "$*"; PASS=$((PASS+1)); }
warn() { printf '  \033[33mWARN\033[0m %s\n' "$*"; WARN=$((WARN+1)); }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$*"; FAIL=$((FAIL+1)); }
info() { printf '       %s\n' "$*"; }
head_() { printf '\n%s\n' "$*"; }

echo "ComicCraft preflight"
echo "root: $ROOT"

# ---------------------------------------------------------------- stage
head_ "Stage"
SETUP=0
if [ -f package.json ] && [ -d shared ] && [ -d server ] && [ -d public ]; then
  SETUP=1
  pass "ComicCraft checkout (package.json with shared/, server/ and public/)"
else
  warn "not a ComicCraft checkout yet — M0 has not created the layout"
  info "expected after M0: package.json, server.js, shared/, server/, public/, schema/, test/"
fi

# The reference tree is evidence, not a dependency. Its presence is good; being *used* is not.
if [ -d reference/vectorcraft ]; then
  REF_HEAD="$(git -C reference/vectorcraft rev-parse --short HEAD 2>/dev/null || echo '?')"
  pass "reference tree present at reference/vectorcraft ($REF_HEAD)"
else
  warn "reference/vectorcraft absent — M0 moves the audited baseline there"
fi

# ---------------------------------------------------------------- team files
head_ "Team files"
CHECKER=".github/skills/comiccraft-start/scripts/check-team-files.py"
if [ -f "$CHECKER" ]; then
  if command -v python3 >/dev/null 2>&1; then
    if out="$(python3 "$CHECKER" "$ROOT" 2>&1)"; then
      pass "agent-customization files valid"
      info "$(printf '%s' "$out" | grep -E '^  agents:' || true)"
    else
      fail "agent-customization files invalid"
      printf '%s\n' "$out" | grep -E 'FAIL|problem' | sed 's/^/       /'
    fi
  else
    warn "python3 not found; skipped team-file validation"
  fi
else
  fail "$CHECKER is missing"
fi

# ---------------------------------------------------------------- documents
head_ "Documents"
if [ -f docs/comiccraft-design.md ] && [ -f docs/comiccraft-porting-plan.md ]; then
  pass "design and plan are in docs/"
elif [ -f porting_plan_and_design/comiccraft_design.md ]; then
  warn "documents still in porting_plan_and_design/ — M0 moves them to docs/"
else
  fail "neither docs/ nor porting_plan_and_design/ has the design documents"
fi
[ -f docs/comiccraft-porting-audit.md ] && pass "reference audit present" \
  || warn "docs/comiccraft-porting-audit.md absent"

if [ -f docs/comiccraft-project-schema.md ]; then
  pass "project schema document frozen (M0 deliverable)"
  if [ -d schema/v1 ] && [ -n "$(ls -A schema/v1 2>/dev/null)" ]; then
    pass "machine-readable schema present: schema/v1/"
  else
    warn "schema/v1/ empty or absent — the fixture gate will fail without it"
  fi
else
  warn "docs/comiccraft-project-schema.md absent — M0 deliverable"
fi

# ---------------------------------------------------------------- toolchain
head_ "Toolchain"
REQUIRED_NODE=22
if command -v node >/dev/null 2>&1; then
  NODE_V="$(node --version 2>/dev/null | sed 's/^v//')"
  NODE_MAJOR="${NODE_V%%.*}"
  if [ "${NODE_MAJOR:-0}" -ge "$REQUIRED_NODE" ] 2>/dev/null; then
    pass "node v$NODE_V satisfies >= $REQUIRED_NODE"
  else
    fail "node v$NODE_V is older than the required $REQUIRED_NODE"
    info "the built-in test runner (node --test) and several standard-library APIs need 22+."
  fi
else
  fail "node not found on PATH"
fi

# Rust is deliberately NOT required. Report it only so a stale toolchain is not mistaken
# for a prerequisite.
if command -v cargo >/dev/null 2>&1; then
  info "cargo $(cargo --version 2>/dev/null | awk '{print $2}') is installed but NOT required"
  info "nothing in this project builds with it; the gate must pass without it"
else
  pass "no Rust toolchain on PATH — correct for this stack"
fi

if [ "$SETUP" = 1 ]; then
  if [ -d node_modules ]; then
    pass "node_modules present"
  else
    warn "node_modules missing — run: npm install"
  fi
  RUNTIME_DEPS="$(node -e 'try{const p=require("./package.json");console.log(Object.keys(p.dependencies||{}).join(","))}catch(e){console.log("?")}' 2>/dev/null)"
  info "runtime dependencies: ${RUNTIME_DEPS:-none}"
  case "$RUNTIME_DEPS" in
    fastify) pass "runtime dependency list is as designed (fastify only)" ;;
    "")      warn "no runtime dependencies listed — expected fastify" ;;
    *)       warn "unexpected runtime dependencies: $RUNTIME_DEPS"
             info "each one is a decision; state what it replaces from the standard library" ;;
  esac
else
  info "toolchain checks for the app apply once M0 creates package.json"
fi

# ---------------------------------------------------------------- the boundary
head_ "Shared boundary"
if [ "$SETUP" = 1 ]; then
  if [ -f test/boundary.test.js ]; then
    if out="$(node --test test/boundary.test.js 2>&1)"; then
      pass "boundary test passes (shared/ loads in Node with no DOM)"
    else
      fail "boundary test fails — shared/ reaches for a runtime, which breaks both sides"
      printf '%s\n' "$out" | tail -n 12 | sed 's/^/       /'
    fi
  else
    warn "test/boundary.test.js absent — M0 adds it"
  fi
  # A cheap independent check: shared/ must not mention a node builtin or a DOM global.
  if [ -d shared ]; then
    HITS="$(grep -rlnE "from ['\"]node:|require\(['\"]node:|document\.|window\.|\bfetch\(" shared/ 2>/dev/null || true)"
    if [ -z "$HITS" ]; then
      pass "no runtime import or DOM global found in shared/"
    else
      fail "shared/ references a runtime:"
      printf '%s\n' "$HITS" | sed 's/^/       /'
    fi
  fi
else
  info "not applicable before the layout exists"
fi

# ---------------------------------------------------------------- git
head_ "Git"
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  pass "inside a git work tree"
  if git cat-file -e 3da4080b^{commit} >/dev/null 2>&1; then
    pass "reference baseline commit 3da4080b is reachable"
  else
    info "reference baseline 3da4080b not in this repository's history (expected if the"
    info "reference lives in its own checkout under reference/vectorcraft/)"
  fi
  if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    warn "working tree is not clean — verify only on a quiescent tree"
    info "$(git status --porcelain | wc -l) changed path(s)"
  else
    pass "working tree is clean"
  fi
else
  warn "not a git work tree — M0 creates the repository"
fi

# ---------------------------------------------------------------- artifacts
head_ "Artifacts"
shopt -s nullglob
base=(docs/ci-baseline-*.md)
gates=(docs/ci-*.md)
reports=(docs/milestones/M*.md)
reviews=(docs/reviews/*.md)
shopt -u nullglob
[ ${#base[@]} -gt 0 ] && pass "reference gate evidence: ${base[*]}" \
  || warn "no docs/ci-baseline-*.md — the reference gate result should be recorded"
[ ${#gates[@]} -gt 0 ] && pass "gate evidence: ${#gates[@]} file(s)" \
  || info "no docs/ci-<sha>.md yet"
[ ${#reports[@]} -gt 0 ] && pass "milestone report(s): ${#reports[@]}" \
  || info "no docs/milestones/M<n>.md yet"
[ ${#reviews[@]} -gt 0 ] && pass "review record(s): ${#reviews[@]}" \
  || info "no docs/reviews/*.md yet"

# ---------------------------------------------------------------- verdict
head_ "Verdict"
printf '  %d pass, %d warn, %d fail\n' "$PASS" "$WARN" "$FAIL"
if [ "$FAIL" -gt 0 ]; then
  echo "  BLOCKED — fix the FAIL lines above before starting."
  exit 2
elif [ "$SETUP" = 1 ]; then
  echo "  READY — start the milestone with /milestone-kickoff."
  exit 0
else
  echo "  NEEDS-SETUP — start M0 with /m0-node-baseline."
  exit 0
fi
