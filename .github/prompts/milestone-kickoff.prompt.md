---
description: "Kick off a ComicCraft milestone (M1-M8): read its phase and exit criteria, assign module trees to the specialists, run two rounds of code review with unit tests per implementation, run the gate, get an independent verdict, and report."
agent: comiccraft-lead
argument-hint: "M<n>, e.g. M1"
tools: [read, search, edit, execute, agent, todo]
---
Run the milestone named in the input, using `docs/comiccraft-porting-plan.md`.

## 1. Establish the milestone

- Read the phase for this milestone and list its **exit criteria** verbatim. These are the
  definition of done; do not substitute your own.
- Confirm the previous milestone's exit criteria are met. If they are not, say so and stop.
- List the module trees this milestone touches, and the design sections that constrain them.

## 2. Assign owners

One owner per module tree, from the ownership table in
`.github/instructions/comiccraft-work.instructions.md`. Give each assignment:

- the module tree,
- the design section that constrains it,
- the exit criteria it is responsible for,
- its own `CARGO_TARGET_DIR`.

Serialise anything touching `crates/engine/src/cmd/comic/`, `Cargo.toml`,
`xtask/src/layers.rs` or a crate's `lib.rs` through the single owner for that surface.

## 3. Build

Delegate. Do not implement a specialist's tree yourself. Review each diff before
integrating: does it respect the two structural rules (no comic data in the visual
document; no duplicated geometry or page size) and its own constraints?

## 4. Review loop — two rounds per implementation

**Every implementation gets two rounds of code review on `deepseek-v4.1-flash`, with unit tests.**
This is not optional and not a formality, and the lead spawns the reviewer — implementers
do not review their own work.

For each implementation as its owner reports ready:

1. Spawn `code-reviewer` for **round 1**: correctness, and whether the unit tests test
   behaviour rather than restating the implementation.
2. Relay the findings to the owner. The owner fixes them and adds the missing unit tests.
3. Spawn `code-reviewer` for **round 2** on the **post-fix revision**: the hard rules,
   failure paths and edge cases.
4. Confirm `docs/reviews/<milestone>-<owner>-<slug>.md` records both rounds, each with a
   verdict and a unit-test result.

Repeat until both rounds are approved. A round 2 that reviews the revision round 1 already
saw, after round 1 requested changes, is not a round — check the revision moved.

## 5. Gate

Integrate, then run `cargo xtask ci` on the whole tree. Fix nothing by weakening a gate.

## 6. Verdict

Hand the milestone to `verification-engineer` and take their verdict over your own reading
of the code. A reject means the milestone is not done, however good the diff looks.

## 7. Report

Use the lead's milestone format — status, exit criteria with the evidence for each, the
per-implementation review rounds, gate result, divergence, new or amended ADRs, what is not
done, and what the next milestone needs first. State completed behaviour separately from
planned behaviour.

Write the report to `docs/milestones/M<n>.md` as well as returning it, so the status
scripts and the next milestone can find it.
