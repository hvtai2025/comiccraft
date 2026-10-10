#!/usr/bin/env bash
# Check the two-round review records in docs/reviews/.
#
# The rule: every implementation has two rounds of review on deepseek-v4.1-flash, with unit tests,
# and round 2 reviews the revision the round-1 fixes produced. This script verifies the
# records are complete and internally consistent. It cannot judge review quality — only
# that the evidence exists, is filled in, and is not self-contradictory.
#
# It is deliberately strict about template placeholders: a record that merely copies the
# template must NOT pass.
#
# Usage: bash check-reviews.sh [repo-root] [milestone]
#   repo-root  default: the nearest ancestor directory containing .github
#   milestone  optional: M0, M1, ... filters to docs/reviews/<milestone>-*.md
#
# Exit: 0 all records complete
#       1 gaps found (including: a named milestone has no records at all)
#       3 nothing to check (no filter given, and no records exist)

set -uo pipefail

# The model every review must run on. Change this one line when the team's model rule changes,
# and add a `Waiver:` line to any record that was reviewed under the previous rule.
EXPECTED_MODEL="${COMICCRAFT_REVIEW_MODEL:-deepseek-v4.1-flash}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Root: explicit argument, else the nearest ancestor with .github (so a copied or symlinked
# script still finds the right tree instead of silently finding nothing).
if [ -n "${1:-}" ]; then
  ROOT="$1"
else
  ROOT="$SCRIPT_DIR"
  while [ "$ROOT" != "/" ] && [ ! -d "$ROOT/.github" ]; do ROOT="$(dirname "$ROOT")"; done
  if [ ! -d "$ROOT/.github" ]; then
    echo "check-reviews: cannot find a .github directory above $SCRIPT_DIR"
    exit 2
  fi
fi
FILTER="${2:-}"
cd "$ROOT" || { echo "cannot enter $ROOT"; exit 2; }

if [ -t 1 ]; then RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; OFF=$'\033[0m'
else RED=; GREEN=; YELLOW=; OFF=; fi

DIR="docs/reviews"
shopt -s nullglob
if [ -n "$FILTER" ]; then files=("$DIR/$FILTER"-*.md); else files=("$DIR"/*.md); fi
shopt -u nullglob

if [ ${#files[@]} -eq 0 ]; then
  if [ -n "$FILTER" ]; then
    echo "${RED}check-reviews: no review records for $FILTER in $DIR${OFF}"
    echo "Every implementation needs docs/reviews/<milestone>-<owner>-<slug>.md with two"
    echo "rounds of review on deepseek-v4.1-flash and unit tests. Nothing to accept yet."
    exit 1
  fi
  echo "check-reviews: no review records in $DIR"
  echo "Every implementation needs docs/reviews/<milestone>-<owner>-<slug>.md with two rounds."
  exit 3
fi

# A template placeholder is a value that still has an angle-bracket slot or an "a | b"
# choice in it. Real values never do.
is_placeholder() { case "$1" in *"<"*|*"|"*) return 0 ;; *) return 1 ;; esac; }

# The body of one "## Round <n>" section, stopping at ANY following heading — otherwise a
# later section's Verdict:/Tests: lines would satisfy this round.
round_block() {
  awk -v want="$2" '
    /^#+[ \t]/ {
      n = -1
      if ($0 ~ /^#+[ \t]*Round[ \t]*[0-9]/) {
        l = $0
        sub(/^#+[ \t]*Round[ \t]*/, "", l)
        sub(/[^0-9].*/, "", l)
        n = l + 0
      }
      inside = (n == want) ? 1 : 0
      next
    }
    inside { print }
  ' "$1"
}

# Markdown emphasis and backticks around a value are not part of it.
strip_md() { sed -E 's/^[*_`]+//; s/[*_`]+$//; s/^[[:space:]]+//; s/[[:space:]]+$//'; }

# The value after "Key:" on the first matching line, trimmed, with markdown stripped.
value_of() { sed -E 's/^[^:]*:[[:space:]]*//' | tr -d '\r' | sed -E 's/[[:space:]]+$//' | strip_md; }

# Join a field whose value continues below it, so a record that writes either
#   Unit tests:
#     crates/x.rs — cargo test — 3 passed
# or
#   Unit tests:
#   - crates/x.rs — cargo test — 3 passed
# is read as one value instead of an empty one. Continuation lines are indented lines and
# list items; a new "Key:" line or any other line ends the value.
flatten_block() {
  awk '
    /^[A-Za-z][A-Za-z ]*:[[:space:]]*$/ { print pending; pending=$0; collecting=1; next }
    collecting && (/^[[:space:]]+[^[:space:]]/ || /^[-*+][[:space:]]/) { pending = pending " " $0; next }
    collecting { print pending; pending=""; collecting=0 }
    { print }
    END { if (collecting) print pending }
  '
}

GAPS=0
SKIPPED=0
echo "Review records (${#files[@]} file(s)${FILTER:+ for $FILTER})"

for f in "${files[@]}"; do
  # Only <milestone>-<owner>-<slug>.md is a review record. A README or a scratch note in
  # the folder must not be parsed as one, nor reported as a gap.
  base="$(basename "$f")"
  if ! printf '%s' "$base" | grep -qE '^(M[0-9]+|sync)-.+\.md$'; then
    printf '  %sSKIP%s %s (not a review record: want <milestone>-<owner>-<slug>.md)\n' "$YELLOW" "$OFF" "$f"
    SKIPPED=$((SKIPPED + 1))
    continue
  fi

  problems=()

  # --- reviewer, and the explicit waiver the rule allows ---
  esc="${EXPECTED_MODEL//./\\.}"
  if ! grep -qiE '^Reviewer:' "$f"; then
    problems+=("no 'Reviewer:' line")
  elif ! grep -qiE '^Reviewer:.*code-reviewer' "$f"; then
    problems+=("'Reviewer:' does not name code-reviewer")
  elif ! grep -qiE "^Reviewer:.*${esc}" "$f"; then
    w="$(grep -iE '^Waiver:' "$f" | head -n1 | value_of)"
    if [ -z "$w" ]; then
      problems+=("reviewer is not ${EXPECTED_MODEL} and there is no 'Waiver:' line naming who reviewed and why")
    elif is_placeholder "$w"; then
      problems+=("the 'Waiver:' line is an unfilled placeholder")
    fi
  fi

  # --- round 1's revision ---
  rev1="$(grep -iE '^Revision:' "$f" | head -n1 | value_of | sed -E 's/[[:space:]]+Files:.*$//')"
  if [ -z "$rev1" ]; then
    problems+=("no 'Revision:' line")
  elif is_placeholder "$rev1"; then
    problems+=("'Revision:' is an unfilled placeholder ($rev1)")
  fi

  # --- both rounds: present, with a real verdict and a real unit-test result ---
  V1=""
  for n in 1 2; do
    if ! grep -qE "^#+[[:space:]]*Round[[:space:]]*$n([^0-9]|$)" "$f"; then
      problems+=("missing Round $n section")
      continue
    fi
    block="$(round_block "$f" "$n" | flatten_block)"

    v="$(printf '%s\n' "$block" | grep -iE '^Verdict:' | head -n1 | value_of)"
    if [ -z "$v" ]; then
      problems+=("Round $n has no verdict")
    elif is_placeholder "$v"; then
      problems+=("Round $n verdict is the unfilled template placeholder ($v)")
    elif ! printf '%s' "$v" | grep -qiE '^(approved|changes-requested)'; then
      problems+=("Round $n verdict is neither 'approved' nor 'changes-requested' ($v)")
    fi
    [ "$n" = 1 ] && V1="$v"

    t="$(printf '%s\n' "$block" | grep -iE '^(Unit tests|Tests):' | head -n1 | value_of)"
    if [ -z "$t" ]; then
      problems+=("Round $n names no unit tests")
    elif is_placeholder "$t"; then
      problems+=("Round $n unit-test line is the unfilled template placeholder")
    elif ! printf '%s' "$t" | grep -qiE '(pass|fail|ok|passed|failed)'; then
      problems+=("Round $n unit-test line records no result (need pass/fail/ok/N passed)")
    fi
  done

  # --- round 2 must review a revision that moved, when round 1 asked for changes ---
  if printf '%s' "$V1" | grep -qi 'changes-requested'; then
    rev2="$(round_block "$f" 2 | flatten_block | grep -iE '^Revision reviewed:' | head -n1 | value_of)"
    if [ -z "$rev2" ]; then
      problems+=("round 1 requested changes but round 2 names no 'Revision reviewed:'")
    elif is_placeholder "$rev2"; then
      problems+=("round 2's 'Revision reviewed:' is an unfilled placeholder ($rev2)")
    elif [ "$rev1" = "$rev2" ]; then
      problems+=("round 1 requested changes but round 2 reviewed the same revision ($rev2) — name the post-fix revision, or add a marker if it is the working tree")
    fi
  fi

  if [ ${#problems[@]} -eq 0 ]; then
    printf '  %sOK%s   %s\n' "$GREEN" "$OFF" "$f"
  else
    printf '  %sGAP%s  %s\n' "$RED" "$OFF" "$f"
    for p in "${problems[@]}"; do printf '         - %s\n' "$p"; done
    GAPS=$((GAPS + 1))
  fi
done

echo
if [ "$SKIPPED" -gt 0 ]; then
  echo "$SKIPPED file(s) skipped as not review records."
fi
if [ "$GAPS" -gt 0 ]; then
  echo "${RED}$GAPS record(s) incomplete. An implementation with an incomplete record is not done.${OFF}"
  exit 1
fi
echo "${GREEN}All review records complete.${OFF}"
