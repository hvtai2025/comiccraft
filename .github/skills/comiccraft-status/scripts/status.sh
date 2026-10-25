#!/usr/bin/env bash
# ComicCraft status: where is the team, and what is missing?
#
# Read-only and fast (no build). Reports the stage, the milestone trail, the artifacts
# that should exist, the shared-boundary health, and what the module trees contain.
#
# Usage: bash status.sh [repo-root]

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-$(cd "$SCRIPT_DIR/../../../.." && pwd)}"
cd "$ROOT" || { echo "cannot enter $ROOT"; exit 1; }

echo "ComicCraft status"
echo "root: $ROOT"

# Stage
if [ -f package.json ] && [ -d shared ] && [ -d server ] && [ -d public ]; then
  STAGE="app"
elif [ -f docs/comiccraft-porting-plan.md ]; then
  STAGE="planned (M0 has not created the layout)"
else
  STAGE="empty"
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

# The shared boundary — the one structural rule whose violation breaks both runtimes.
echo
echo "Shared boundary"
if [ -d shared ]; then
  HITS="$(grep -rlnE "from ['\"]node:|require\(['\"]node:|document\.|window\.|\bfetch\(" shared/ 2>/dev/null || true)"
  if [ -z "$HITS" ]; then
    echo "  ok   shared/ has no runtime import and no DOM global"
  else
    echo "  BAD  shared/ reaches for a runtime (this breaks the browser and the server):"
    printf '%s\n' "$HITS" | sed 's/^/         /'
  fi
  n_modules="$(find shared -name '*.js' 2>/dev/null | wc -l | tr -d ' ')"
  echo "       $n_modules module(s) in shared/"
  if [ -f test/boundary.test.js ]; then
    echo "       test: test/boundary.test.js"
  else
    echo "       no test/boundary.test.js yet — M0 adds it"
  fi
else
  echo "  --   shared/ absent (M0 creates it)"
fi

# Comic commands
echo
echo "Comic commands"
if [ -d shared/commands ]; then
  # The id is a string literal in a command definition. Match the literal rather than a line
  # shape, so a formatter change cannot silently report 0 — that failure mode cost this project
  # a false "0 commands registered" reading once already.
  n="$(grep -rhoE "['\"]comic\.[a-z0-9._]+['\"]" shared/commands server/commands 2>/dev/null \
      | tr -d "'\"" | sort -u | wc -l | tr -d ' ')"
  srv="$(grep -rlE "serverOnly[[:space:]]*:[[:space:]]*true" shared/commands 2>/dev/null | wc -l | tr -d ' ')"
  echo "  $n comic command id(s) defined ($srv module(s) marking a server-only handler)"
  if [ -f shared/commands/index.js ]; then
    echo "       registry: shared/commands/index.js"
  fi
else
  echo "  shared/commands/ absent"
fi

# Dependency footprint
echo
echo "Dependencies"
if [ -f package.json ]; then
  node -e '
    const p = require("./package.json");
    const rt = Object.keys(p.dependencies || {});
    const dev = Object.keys(p.devDependencies || {});
    console.log("  runtime : " + (rt.length ? rt.join(", ") : "none"));
    console.log("  dev     : " + (dev.length ? dev.join(", ") : "none"));
  ' 2>/dev/null || echo "  cannot read package.json"
else
  echo "  no package.json (M0 creates it; fastify should be the only runtime dependency)"
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

# Reference and working tree
echo
echo "Reference"
if [ -d reference/vectorcraft ]; then
  echo "  ref: $(git -C reference/vectorcraft rev-parse --short HEAD 2>/dev/null || echo '?')  (read-only; never imported)"
  USED="$(grep -rlE "reference/vectorcraft" shared/ server/ public/ package.json 2>/dev/null || true)"
  if [ -z "$USED" ]; then
    echo "  ok   nothing in shared/, server/, public/ or package.json references it"
  else
    echo "  BAD  the reference tree is referenced as a dependency:"
    printf '%s\n' "$USED" | sed 's/^/         /'
  fi
else
  echo "  reference/vectorcraft/ absent — M0 moves the audited baseline there"
fi

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "  working tree: $(git status --porcelain 2>/dev/null | wc -l | tr -d ' ') changed file(s)"
  [ -n "$(git status --porcelain 2>/dev/null)" ] && echo "  NOTE: verify only on a quiescent tree"
else
  echo "  not a git work tree"
fi
