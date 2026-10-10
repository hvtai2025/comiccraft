---
name: comiccraft-start
description: 'Start or resume the ComicCraft engineering team. Use when beginning M0-M8, creating the fork, dispatching work to a specialist agent (schema-architect, command-engineer, layout-engineer, lettering-engineer, publishing-engineer, ai-pipeline-engineer, studio-ui-engineer, verification-engineer), or resuming after an interruption. Runs a preflight, resolves the target, and invokes the owning agent with a complete brief.'
argument-hint: 'M0 | M1 | schema-architect | resume'
---

# Starting the ComicCraft team

ComicCraft is built by nine agents with non-overlapping module trees. This skill is the
front door: it decides whether the team can start, who should start, and hands that agent a
brief it cannot misinterpret.

## When to use

- Beginning a milestone (M0–M8).
- Creating the fork — the current state.
- Handing one scoped task to a specialist.
- Resuming after an interruption, or after a milestone was rejected.

## Procedure

### 1. Preflight

```sh
bash ./scripts/preflight.sh
```

Read the verdict and obey it:

| Verdict | Meaning | Action |
|---|---|---|
| `READY` | fork exists, toolchain and artifacts are in order | go to step 2 |
| `NEEDS-FORK` | the fork has not been created yet | the target is **M0** |
| `BLOCKED` | a `FAIL` line must be fixed first | **stop** and report the FAIL lines; dispatch nothing |

Do not dispatch past a `FAIL`. The checks that block are real: an older toolchain cannot
parse the manifest at all, and a crate that is not in the layer table fails `cargo xtask ci`.

### 2. Resolve the target

Take the argument, or default to the next milestone in `docs/comiccraft-porting-plan.md` §15.

| Argument | Agent to invoke | Task |
|---|---|---|
| `M0` | `comiccraft-lead` | create the fork, record the baseline gate, freeze the schema, register the crates |
| `M1`…`M8` | `comiccraft-lead` | run that milestone via `/milestone-kickoff` |
| an agent name | that agent | the scoped task you were given, or its next unblocked item |
| `review` | `code-reviewer` | two rounds of code review plus unit tests for the implementation named. See the `comiccraft-verify` skill |
| `resume` | `comiccraft-lead` | read the last `docs/milestones/M*.md`, then continue from its "Not done" and "Next" |
| `verify` | `verification-engineer` | use the `comiccraft-verify` skill instead |

### 3. Assemble the brief

Fill in [the agent brief](./references/agent-brief.md) completely — no placeholders, no
"as appropriate". Look up the agent's module tree and design sections in
[the dispatch table](./references/dispatch-table.md).

### 4. Invoke

Launch the named custom agent as a subagent with the assembled brief. If custom-agent
subagent invocation is not available in this session, print the brief and tell the user to
select that agent from the picker and paste it — do not silently skip the dispatch.

### 5. Confirm the start

The agent's first response must state back: the milestone or scope, its exit criteria, its
owned paths, and what it will not touch. If it does not, or if it begins editing a path it
does not own, stop it and re-issue the brief with the boundary made explicit. A wrong start
is cheaper to fix than a wrong diff.

### 6. Hand off

- One owner per module tree, always. Never dispatch two agents into the same tree.
- Give every parallel agent its own `CARGO_TARGET_DIR`.
- Serialise anything touching `crates/engine/src/cmd/comic/`, `Cargo.toml`,
  `xtask/src/layers.rs` or a crate's `lib.rs` through its single owner.
- Do not start a milestone whose predecessor's exit criteria are unmet.
- **Every implementation gets two rounds of code review on `deepseek-v4.1-flash` with unit tests.**
  You spawn `code-reviewer`; nobody reviews their own work. See the `comiccraft-verify`
  skill, step 2.

## After the agents return

Integrate, run `cargo xtask ci`, then use the `comiccraft-verify` skill for the independent
verdict before calling the milestone done. Record the result in `docs/milestones/M<n>.md`.

## Resources

- [Dispatch table](./references/dispatch-table.md) — agent, module tree, design sections, constraints
- [Agent brief template](./references/agent-brief.md) — the brief you hand over
- [Preflight script](./scripts/preflight.sh) — stage, toolchain, git, crates, artifacts
- [Team-file validator](./scripts/check-team-files.py) — frontmatter and reference integrity
