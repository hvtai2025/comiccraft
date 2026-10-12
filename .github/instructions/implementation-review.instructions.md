---
description: "Use when implementing, reviewing or accepting any ComicCraft code change: the mandatory two-round code review and unit-test rule. Covers when a review is required, what each round must cover, the review record, who may fix production code, and what counts as evidence. Applies to every implementation, in every crate, including crate skeletons, layer-table edits and documentation."
applyTo: ["crates/**", "apps/**", "xtask/**", "Cargo.toml", "docs/**"]
---
# Two-round review and unit test — mandatory

**No implementation is complete until it has had two rounds of code review on
`deepseek-v4.1-flash`, and its unit tests pass.** This is a process rule, not a suggestion: an
unreviewed change is not done, and neither is a reviewed change without tests.

## Scope

Applies to:

- every code change, in every crate, however small — including crate skeletons, a
  `xtask/src/layers.rs` registration, and `Cargo.toml` edits;
- every new or changed command, schema, layout rule, lettering rule, adapter or panel;
- documentation, with the test requirement adjusted (see *Documentation-only changes*).

One review covers one change by one owner. A change that spans two module trees gets a
review per tree, and the lead reviews the seam.

## Who reviews

`code-reviewer`, running on `deepseek-v4.1-flash`. It is spawned by the lead — implementers do not
review their own work, and the verifier is not the reviewer.

If the reviewer is not available on `deepseek-v4.1-flash`, the lead either re-runs it there or
records an explicit waiver in the review record saying who reviewed and why. A silent
substitution is a violation.

## The two rounds

Two rounds with **different focus**, so the second is not a rubber stamp.

| Round | Focus | Revision |
|---|---|---|
| 1 | Correctness: does it do what the design says; do the tests test behaviour; silent failures | the implementation |
| 2 | The hard rules, failure paths and edge cases | the **post-fix** revision |

Round 2 must review the revision the fixes produced. If round 1 requested changes and round
2 reviews the same revision, round 2 did not happen. If both rounds are clean, the report
must state what the reviewer tried to falsify.

The reviewer **writes unit tests** — independently of the implementer's, aimed at what the
implementer did not cover. The implementer also writes unit tests for their own module.
Both sets are required.

## Test ownership

| Layer | Owner |
|---|---|
| Inline `#[cfg(test)]` unit tests in a module | the implementer of that module |
| Additional unit and integration tests for a reviewed change | `code-reviewer` (test code only) |
| Milestone acceptance, golden fixtures, CLI/MCP end-to-end | `verification-engineer` |

`code-reviewer` may edit test code and nothing else. **Nobody reviews their own work, and
the reviewer never fixes production code** — findings go back to the owner with a minimal
reproduction.

### Land the tests before you write the record

**Write the failing test file to disk and run it, then write the record.** A record is prose; a test is
executable evidence, and only one of the two survives losing your session.

This is not hypothetical. Two reviewers were cleared mid-task in M2 and left no records at all. One of
them had already written `crates/comic-layout/tests/m2_w4_round1_reviewer.rs`, and running it later
recovered **two findings that would otherwise have been lost silently** — including a defect introduced
by the fix for a previous one. The other left six passing tests, which at least showed *what* it had
cleared. **A lost record is a lost finding; a test file is not**, because whoever comes next can execute
it.

The corollary is about temporary files: a scratch script or probe is **not** evidence, so **do not leave
one in `tests/`** — it would be committed as if it were a test. Delete it as part of writing the record,
and say so.

## The review record

Every change leaves one, at `docs/reviews/<milestone>-<owner>-<slug>.md`:

```text
Reviewer:      code-reviewer on deepseek-v4.1-flash
Waiver:        <only if the reviewer was NOT on deepseek-v4.1-flash: who reviewed, and why>
Revision:      <sha or 'working tree'>   Files: <paths>
Design refs:   <the sections this must satisfy>

## Round 1 — correctness
Verdict:       approved | changes-requested
Unit tests:    <paths> — <command> — <pass/fail>
Findings:      <n> — <file:line, input, expected vs actual>   (or: none, and what was tried)

## Round 2 — rules, failure paths, edges
Revision reviewed: <the post-fix revision>
Verdict:       approved | changes-requested
Unit tests:    <paths> — <command> — <pass/fail>
Findings:      <n> — <file:line, input, expected vs actual>   (or: none, and what was tried)

Gate:          cargo xtask ci — pass | fail (<first failing step>)
Unverified:    <what was not checked>
```

Every value above is real: a record that still contains `<paths>`, `<command>`,
`approved | changes-requested` or `<pass/fail>` is an unfilled template and fails the
check. **When round 1 requested changes, `Revision reviewed:` must name the post-fix
revision and must differ from `Revision:`.** If the fix is not committed, add a
distinguishing marker (`working tree after the null-panel fix`) — two identical revision
strings cannot show that round 2 reviewed the fix.

Check compliance with `bash .github/skills/comiccraft-verify/scripts/check-reviews.sh "" M<n>`.
Exit 0 means complete, 1 means gaps (including a milestone with no records at all), 3 means
there was nothing to check.

## What counts as evidence

- A verdict with the command that produced it and its output.
- A test that fails when the behaviour is broken. A test that cannot fail is not evidence.
- The gate result, quoted, not summarised.

Not evidence: "it compiles", "the gate passes", "looks good", an implementer's own summary,
or a round-2 approval on a revision that did not move.

## Documentation-only changes

Two review rounds still apply. Unit tests are required only where the change has testable
behaviour (a schema example, a fixture, a rule a test can assert); otherwise the record
states what was verified instead — every claim checked against the code it describes.

## Escalation

- A finding the owner disputes goes to the lead, who decides against the design and records
  an ADR if the design is wrong.
- A change that cannot be tested must say so explicitly in the record. "Untestable" is a
  finding about the design, not an exemption.
- Two rounds with unresolved findings means the change is not done. Do not merge it and do
  not close the milestone.
