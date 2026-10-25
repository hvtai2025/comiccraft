---
name: comiccraft-verify
description: 'Verify a ComicCraft milestone independently. Use when closing a milestone, when asked to verify, validate or prove that ComicCraft work is done, before accepting a claim that something is complete, or when checking that a hazard from the reference audit has a covering test. Runs the npm run verify gate, turns each exit criterion into a runnable check, and returns an accept or reject verdict.'
argument-hint: 'M0 | M1 | ...'
---

# Verifying a ComicCraft milestone

A milestone is accepted on evidence, not on a summary. This skill produces the verdict and
the report, and it refuses to accept a criterion nobody ran.

## When to use

- Closing a milestone.
- Before accepting any claim that work is complete.
- After a schema or domain change, to re-prove comic behaviour.
- When asked whether a hazard from the reference audit is covered by a test.

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

The gate is `npm run verify` — type check (JSDoc, no emit), ESLint including the boundary
rules, the layering unit test, `node --test`, the schema-fixture validation and the server
smoke test — and it stops at the first failure. It writes `docs/ci-<sha>.md`.

The script refuses to run outside the ComicCraft checkout, on a Node older than 22, or
without `node_modules`, because each of those produces a failure that looks like a code
defect and is not one.

**The gate must never need a Rust toolchain, a font file, or a browser.** If it does, that
is itself the finding: a test that requires one of the three is a broken test on this stack
(plan §17.1).

### 4. Check each criterion

Hand the milestone to `verification-engineer` with the criteria and the gate result. They
test at the seams the implementer cannot see: bundle round trips, cross-runtime behaviour,
the command surface over HTTP and MCP, and the rendered result.

Guidance for the checks themselves:

- Rendered pages: compare with a perceptual diff and keep the artifact. Never assert byte
  equality of a PNG.
- **Both runtimes**: a criterion about the domain is not proven until it holds when the
  module is loaded by Node *and* when it is loaded in a browser context. The cheapest honest
  check is a `node --test` case plus the boundary test, not a browser launch.
- End to end: drive it through the HTTP command endpoint and through MCP, the way an agent
  would.
- Failure paths are criteria too: interrupted save, provider failure, cancelled job, missing
  asset, missing font, deleted artboard.
- Performance: record page-load and render timings against the numbers recorded at M1.
  Report noise rather than claiming a regression.
- Every new comic command gets a malformed-parameter case in `test/fuzz.js`.

### 5. Check the hazards

Cross-check the reference audit's §12 hazards against the milestone. A hazard that has closed
should have a test that fails without the fix — especially §12.3, the artboard-id reuse
hazard, which is silent when it breaks.

### 6. Verdict and report

Accept only if **every** criterion has evidence, every implementation's review record is
complete, the gate passes, and no hazard is uncovered that this milestone claimed to close.
Otherwise reject, and name the one thing that must change.

Write `docs/milestones/M<n>.md` using the lead's format, and include the gate result
verbatim — a milestone that claims `npm run verify` passes must show it.

```text
Milestone:   M<n>
Status:      complete | incomplete | rejected
Criteria:    <each one: pass | fail | not run, with the command and its output>
Reviews:     <per implementation: round 1 verdict, round 2 verdict, unit tests — from check-reviews.sh>
Regressions: <what broke, with a minimal reproduction>
Hazards:     <which audit hazards have a covering test, and which do not>
Gate:        npm run verify — pass | fail (<first failing step, verbatim>)
Runtime:     node <version>  (no Rust toolchain, no bundled font, no browser required)
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
- **A gate that needs a Rust toolchain, a font file or a browser is a defect**, not a
  prerequisite. Report it.

## Resources

- [Gate script](./scripts/gate.sh) — runs `npm run verify`, saves evidence, reports the first failure
- [Review-record checker](./scripts/check-reviews.sh) — two rounds, verdicts, unit tests, revision moved
- [Verification playbook](./references/verification-playbook.md) — per-criterion checks and the harness to use
