#!/usr/bin/env bash
# ComicCraft status: where is the team, and what is missing?
#
# Read-only and fast (no build). Reports the stage, the milestone trail, the artifacts
# that should exist, upstream divergence, and the comic crates' registration.
#
# Usage: bash status.sh [repo-root]

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$SCRIPT_DIR/../../../.." && pwd)}"
cd "$ROOT" || { echo "cannot enter $ROOT"; exit 1; }

echo "ComicCraft status"
echo "root: $ROOT"

# Stage
if [ -f Cargo.toml ] && grep -q '^\[workspace\]' Cargo.toml 2>/dev/null && [ -d crates ]; then
  STAGE="fork"
else
  STAGE="planning"
fi
echo "stage: $STAGE"

# Milestone trail, newest last
echo
echo "Milestones"
shopt -s nullglob
reports=(docs/milestones/M*.md)
shopt -u nullglob
if [ ${#reports[@]} -eq 0 ]; then
  echo "  none yet — the first is docs/milestones/M0.md"
else
  for r in "${reports[@]}"; do
    # Tolerate Markdown emphasis and a leading table pipe: "**Status:** x", "| Status | x |",
    # "Status: x".
    status="$(grep -iE '^[*|[:space:]]*status[*[:space:]]*[:|]' "$r" 2>/dev/null | head -n1 \
      | sed -E 's/^[*|[:space:]]*[Ss]tatus[*[:space:]]*[:|][*[:space:]]*//' | sed -E 's/[*|[:space:]]+$//' || true)"
    printf '  %-28s %s\n' "$r" "${status:-<no status line>}"
  done
fi

# Artifacts
echo
echo "Artifacts"
check() { if [ -e "$1" ]; then printf '  yes  %s\n' "$1"; else printf '  --   %s\n' "$1"; fi; }
check docs/comiccraft-design.md
check docs/comiccraft-porting-plan.md
check docs/comiccraft-porting-audit.md
check docs/comiccraft-project-schema.md
check docs/comiccraft-command-api.md
check docs/comiccraft-ai-workflow.md
check docs/comiccraft-publishing.md
shopt -s nullglob
for b in docs/ci-baseline-*.md; do printf '  yes  %s\n' "$b"; done
shopt -u nullglob

# Comic crates
echo
echo "Comic crates"
for c in comic-model comic-ai comic-layout; do
  if [ -d "crates/$c" ]; then
    reg="not in layers.rs"
    grep -q "\"$c\"" xtask/src/layers.rs 2>/dev/null && reg="registered"
    printf '  yes  crates/%-14s %s\n' "$c" "$reg"
  else
    printf '  --   crates/%-14s absent\n' "$c"
  fi
done

# Comic commands
echo
echo "Comic commands"
if [ -d crates/engine/src/cmd/comic ]; then
  n="$(grep -rho 'cmd!("comic\.[a-z0-9._]*"' crates/engine/src/cmd/comic 2>/dev/null | sort -u | wc -l | tr -d ' ')"
  echo "  $n comic command id(s) registered"
else
  echo "  crates/engine/src/cmd/comic/ absent"
fi

# Reviews
echo
echo "Reviews"
shopt -s nullglob
reviews=(docs/reviews/*.md)
shopt -u nullglob
if [ ${#reviews[@]} -eq 0 ]; then
  echo "  none yet — every implementation needs two rounds in docs/reviews/"
else
  # Delegate to the authoritative checker rather than reimplementing it: this script's own
  # round-counting was markdown-blind (it missed `Verdict: **approved**`) and reported complete
  # records as gaps. One implementation, one answer.
  checker=".github/skills/comiccraft-verify/scripts/check-reviews.sh"
  if [ -x "$checker" ] || [ -f "$checker" ]; then
    out="$(bash "$checker" "" "" 2>&1)"; rc=$?
    printf '  %s record(s); checker exit %s\n' "${#reviews[@]}" "$rc"
    printf '%s\n' "$out" | grep -E '^  (OK|GAP|SKIP)' | sed 's/^/  /' || true
    [ "$rc" -eq 0 ] && echo "  all complete" || echo "  incomplete — see the GAP lines above"
  else
    echo "  ${#reviews[@]} record(s); $checker not found, cannot verify"
  fi
  echo "  check them: bash $checker"
fi

# Divergence
echo
echo "Upstream"
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if git remote get-url upstream >/dev/null 2>&1; then
    echo "  ahead of upstream/main by $(git rev-list --count upstream/main..HEAD 2>/dev/null || echo '?') commit(s)"
    echo "  comic files diverging:"
    git diff --name-only upstream/main...HEAD 2>/dev/null \
      | grep -E 'crates/comic-|crates/engine/src/cmd/comic/|crates/ui-egui/src/panels/comic/|xtask/src/layers.rs' \
      | sed 's/^/    /' || true
  else
    echo "  no 'upstream' remote — divergence not measurable"
  fi
  echo "  working tree: $(git status --porcelain 2>/dev/null | wc -l | tr -d ' ') changed file(s)"
else
  echo "  not a git work tree"
fi
