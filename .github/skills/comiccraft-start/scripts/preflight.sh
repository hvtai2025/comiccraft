#!/usr/bin/env bash
# ComicCraft preflight: decide whether the team can start, and at which stage.
#
# Reports PASS / WARN / FAIL per check and ends with a verdict:
#   READY       start the milestone
#   NEEDS-FORK  M0's fork has not been created yet; start M0
#   BLOCKED     a FAIL must be fixed first (the FAIL lines say what)
#
# Read-only: creates nothing, changes nothing.
#
# Usage: bash preflight.sh [repo-root]

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$SCRIPT_DIR/../../../.." && pwd)}"
cd "$ROOT" || { echo "FAIL cannot enter $ROOT"; exit 1; }

# rustup installs to ~/.cargo/bin and only a login shell has it on PATH. Source it here so the
# toolchain check reflects what is actually installed rather than how this script was invoked —
# without this, running from a non-login shell reports a false BLOCKED on the apt rustc.
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi

PASS=0; WARN=0; FAIL=0
pass() { printf '  \033[32mPASS\033[0m %s\n' "$*"; PASS=$((PASS+1)); }
warn() { printf '  \033[33mWARN\033[0m %s\n' "$*"; WARN=$((WARN+1)); }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$*"; FAIL=$((FAIL+1)); }
info() { printf '       %s\n' "$*"; }
head_() { printf '\n%s\n' "$*"; }

# 1.75 < 1.95, and 1.9 < 1.10 is wrong with a plain string compare.
ver_ge() {
  local a="$1" b="$2"
  [ "$a" = "$b" ] && return 0
  [ "$(printf '%s\n%s\n' "$a" "$b" | sort -V | head -n1)" = "$b" ]
}

echo "ComicCraft preflight"
echo "root: $ROOT"

# ---------------------------------------------------------------- stage
head_ "Stage"
FORK=0
if [ -f Cargo.toml ] && grep -q '^\[workspace\]' Cargo.toml 2>/dev/null && [ -d crates ] && [ -d xtask ]; then
  FORK=1
  pass "fork checkout (workspace root with crates/ and xtask/)"
else
  warn "not a fork checkout yet — M0 has not created it"
  info "expected after M0: Cargo.toml ([workspace]), crates/, xtask/, apps/"
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
  pass "design, plan and audit are in docs/ (M0's move is done)"
elif [ -f porting_plan_and_design/comiccraft_design.md ]; then
  warn "documents still in porting_plan_and_design/ — M0 moves them to docs/"
  info "docs/comiccraft-design.md, docs/comiccraft-porting-plan.md, docs/comiccraft-porting-audit.md"
else
  fail "neither docs/ nor porting_plan_and_design/ has the design documents"
fi
[ -f docs/comiccraft-project-schema.md ] && pass "project schema frozen (M0 deliverable)" \
  || warn "docs/comiccraft-project-schema.md absent — M0 deliverable"

# ---------------------------------------------------------------- toolchain
head_ "Toolchain"
REQUIRED="1.95"
if [ -f Cargo.toml ]; then
  parsed="$(grep -E '^rust-version' Cargo.toml 2>/dev/null | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
  [ -n "$parsed" ] && REQUIRED="$parsed"
else
  # Pre-fork: take the floor from the baseline checkout if it is beside us.
  for cand in ../vectorcraft/Cargo.toml vectorcraft/Cargo.toml; do
    if [ -f "$cand" ]; then
      parsed="$(grep -E '^rust-version' "$cand" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
      [ -n "$parsed" ] && REQUIRED="$parsed"
      break
    fi
  done
fi

if command -v cargo >/dev/null 2>&1; then
  CARGO_V="$(cargo --version 2>/dev/null | awk '{print $2}')"
  RUSTC_V="$(rustc --version 2>/dev/null | awk '{print $2}')"
  info "cargo ${CARGO_V:-?}  rustc ${RUSTC_V:-?}  required >= ${REQUIRED}"
  if [ -n "${RUSTC_V:-}" ] && ver_ge "$RUSTC_V" "$REQUIRED"; then
    pass "rustc $RUSTC_V satisfies rust-version = $REQUIRED"
  else
    fail "rustc ${RUSTC_V:-none} is older than the required $REQUIRED"
    info "the workspace declares resolver = \"3\" and edition 2024; an older cargo cannot"
    info "even parse the manifest. Install $REQUIRED+ (rustup default $REQUIRED) before M0's gate step."
  fi
else
  fail "cargo not found on PATH"
fi

# ---------------------------------------------------------------- git
head_ "Git"
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  pass "inside a git work tree"
  REMOTES="$(git remote -v 2>/dev/null | awk '{print $1}' | sort -u | tr '\n' ' ')"
  info "remotes: ${REMOTES:-none}"
  if [ "$FORK" = 1 ]; then
    git remote get-url upstream >/dev/null 2>&1 \
      && pass "upstream remote present" \
      || warn "no 'upstream' remote — divergence cannot be measured (M0 sets it)"
    if git cat-file -e 3da4080b^{commit} >/dev/null 2>&1; then
      pass "baseline commit 3da4080b is reachable"
      if git remote get-url upstream >/dev/null 2>&1; then
        DIV="$(git rev-list --count upstream/main..HEAD 2>/dev/null || echo '?')"
        info "divergence: ${DIV} commit(s) ahead of upstream/main"
      fi
    else
      warn "baseline commit 3da4080b not found — was the fork made from a different revision?"
    fi
  else
    warn "not a git work tree yet — M0 creates the fork repository"
    info "the planning documents are currently unversioned"
  fi
else
  warn "not a git work tree — M0 creates the fork repository"
  info "the planning documents are currently unversioned"
fi

# ---------------------------------------------------------------- comic crates
head_ "Comic crates"
if [ "$FORK" = 1 ]; then
  for c in comic-model comic-ai comic-layout; do
    if [ -d "crates/$c" ]; then
      if grep -q "\"$c\"" xtask/src/layers.rs 2>/dev/null; then
        pass "crates/$c present and registered in xtask/src/layers.rs"
      else
        fail "crates/$c exists but is not registered in xtask/src/layers.rs (cargo xtask layers will fail)"
      fi
    else
      warn "crates/$c absent — M0 adds it (L2, L3, L4)"
    fi
  done
  if [ -f xtask/src/layers.rs ] && grep -q 'comic-' xtask/src/layers.rs; then
    pass "comic crates referenced by the layer table"
  fi
else
  info "not applicable before the fork exists"
fi

# ---------------------------------------------------------------- artifacts
head_ "Artifacts"
shopt -s nullglob
base=(docs/ci-baseline-*.md)
reports=(docs/milestones/M*.md)
shopt -u nullglob
if [ ${#base[@]} -gt 0 ]; then
  pass "baseline gate evidence: ${base[*]}"
else
  warn "no docs/ci-baseline-<short-sha>.md — M0 records it"
fi
if [ ${#reports[@]} -gt 0 ]; then
  pass "milestone report(s): ${reports[*]}"
else
  info "no docs/milestones/M<n>.md yet"
fi

# ---------------------------------------------------------------- verdict
head_ "Verdict"
printf '  %d pass, %d warn, %d fail\n' "$PASS" "$WARN" "$FAIL"
if [ "$FAIL" -gt 0 ]; then
  echo "  BLOCKED — fix the FAIL lines above before starting."
  exit 2
elif [ "$FORK" = 1 ]; then
  echo "  READY — start the milestone with /milestone-kickoff."
  exit 0
else
  echo "  NEEDS-FORK — start M0 with /m0-fork-and-baseline."
  exit 0
fi
