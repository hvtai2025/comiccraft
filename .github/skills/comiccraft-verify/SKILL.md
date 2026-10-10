---
name: comiccraft-verify
description: 'Verify a ComicCraft milestone independently. Use when closing a milestone, when asked to verify, validate or prove that ComicCraft work is done, before accepting a claim that something is complete, or when checking that a hazard from the baseline audit has a covering test. Runs the cargo xtask ci gate, turns each exit criterion into a runnable check, and returns an accept or reject verdict.'
argument-hint: 'M0 | M1 | ...'
---

# Verifying a ComicCraft milestone

A milestone is accepted on evidence, not on a summary. This skill produces the verdict and
the report, and it refuses to accept a criterion nobody ran.

## When to use

- Closing a milestone.
- Before accepting any claim that work is complete.
- After merging upstream, to re-prove comic behaviour.
- When asked whether a hazard from the audit is covered by a test.

## Procedure

### 1. Get the exit criteria

Open `docs/comiccraft-porting-plan.md` and copy the phase's **exit criteria verbatim**.
Those are the test list — not a summary of them, and not your own idea of done. If a
criterion is not runnable as written, that is a finding: say so and get it sharpened before
verifying.

### 2. Check the review records

```sh
bash ./scripts/check-reviews.sh "" M<n>
```

Every implementation must have two review rounds on `deepseek-v4.1-flash` with unit tests, recorded
in `docs/reviews/<milestone>-<owner>-<slug>.md`. The script checks that both rounds exist,
each has a verdict and a unit-test result, and that round 2 reviewed a **different revision**
whenever round 1 requested changes.

An incomplete record is a **reject**, not a note: the rule is that an implementation with an
incomplete record is not done. This is the most commonly skipped step in the whole process,
which is why it is a script and not a reminder.

Exit codes: `0` complete, `1` gaps (including a named milestone with no records at all),
`3` nothing to check. A `3` for a milestone you named is not a pass — but the script exits
`1` in that case, so a missing review is never silent.

### 3. Run the gate

```sh
bash ./scripts/gate.sh
```

The gate is `cargo xtask ci` — fmt, clippy, test, assets, brands, layers, wasm — and it
stops at the first failure. It takes a long time: run it with a generous wait or in the
background, and read the tail when it finishes. It writes `docs/ci-<sha>.md` — Markdown on
purpose, because `cargo xtask brands` scans non-Markdown text files under `docs/` and a
build log contains vendor names in upstream test names.

The script refuses to run outside the fork, and refuses to run on a toolchain older than
the workspace's `rust-version`, because an older cargo cannot even parse the manifest.

### 4. Check each criterion

Hand the milestone to `verification-engineer` with the criteria and the gate result. They
test at the seams the implementer cannot see: bundle round trips, cross-module behaviour,
the CLI, MCP, and the rendered result.

Guidance for the checks themselves:

- Rendered pages: compare perceptually with `testkit::raster::assert_similar` and keep the
  diff artifact. Never assert byte equality of a PNG.
- End to end: drive it through `vectorcraft-cli run` and `mcp --headless` under
  `--automation-read-root` / `--automation-write-root`, the way an agent would.
- Failure paths are criteria too: interrupted save, provider failure, cancelled job,
  missing asset, missing font, deleted artboard.
- Performance: `vectorcraft-cli perf` before and after, against M0's saved baseline. Report
  noise rather than claiming a regression.
- Every new comic command gets a malformed-input case in `engine/tests/command_sweep.rs`.

### 5. Check the hazards

Cross-check the baseline audit's §12 hazards against the milestone. A hazard that has
closed should have a test that fails without the fix — especially §12.3, the artboard-id
reuse hazard, which is silent when it breaks.

### 6. Verdict and report

Accept only if **every** criterion has evidence, every implementation's review record is
complete, the gate passes, and no hazard is uncovered that this milestone claimed to close.
Otherwise reject, and name the one thing that must change.

Write `docs/milestones/M<n>.md` using the lead's format, and include the gate result
verbatim — a milestone that claims `cargo xtask ci` passes must show it.

```text
Milestone:   M<n>
Status:      complete | incomplete | rejected
Criteria:    <each one: pass | fail | not run, with the command and its output>
Reviews:     <per implementation: round 1 verdict, round 2 verdict, unit tests — from check-reviews.sh>
Regressions: <what broke, with a minimal reproduction>
Hazards:     <which audit hazards have a covering test, and which do not>
Gate:        cargo xtask ci — pass | fail (<first failing step, verbatim>)
Verdict:     accept | reject (<the one thing that must change>)
```

## Rules

- **An implementation with an incomplete review record is not done.** Two rounds on
  `deepseek-v4.1-flash`, with unit tests, on the post-fix revision.
- **The verifier does not fix production code.** Report the failure to the owning agent
  with a minimal reproduction; the owner changes their module.
- Do not weaken a test to make it pass, and do not delete a failing case.
- Do not accept "it should work". Run it, and cite the command and its output.
- Report `not run` honestly rather than guessing at a criterion's outcome.
- A criterion that cannot be run as written is a defect in the criterion; raise it.

## Resources

- [Gate script](./scripts/gate.sh) — runs `cargo xtask ci`, saves evidence, reports the first failure
- [Review-record checker](./scripts/check-reviews.sh) — two rounds, verdicts, unit tests, revision moved
- [Verification playbook](./references/verification-playbook.md) — per-criterion checks and the harness to use
