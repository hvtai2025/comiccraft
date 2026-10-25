# Agent brief template

Fill every field. A brief with a placeholder in it produces work you have to throw away.

```text
You are <agent-name>. Milestone: M<n> — <name>.

Scope:            <the module tree(s) you own, as paths>
Do not touch:     <the paths and surfaces that belong to others>

Read first, in this order:
  1. AGENTS.md
  2. docs/comiccraft-design.md <sections that constrain this work>
  3. docs/comiccraft-porting-plan.md §<phase>
  4. docs/comiccraft-project-schema.md <the parts you need>

Deliverable:      <the concrete artifact — code, a plan type, a document>
Exit criteria:    <copied verbatim from the plan's phase; these are the definition of done>
Evidence:         <the test or command that must pass, per criterion>

Constraints:      <the three or four rules from the dispatch table that apply here>
Interface:        <what you consume, and what you expose to the command layer>
Blocker to raise: <the decision this task depends on, if any>

Environment:      node <version>; no Rust toolchain, no font files, no browser needed
Report back:      <the output format from your agent file>
```

## Notes

- **Copy the exit criteria verbatim.** Paraphrasing them is how a milestone closes with an
  unmet requirement.
- **Name the interface, not just the deliverable.** Most integration cost is a type that
  two agents disagreed about.
- **State the blocker even if it is not yours to solve.** An unresolved dependency decision
  (a new crate, provider terms, a missing font) must surface at dispatch time, not at the
  gate.
- **Keep the gate cheap.** There is no build step and no compiled output, so a full
  `npm run verify` run is seconds rather than minutes. Run it often — the thing to avoid is
  not the cost of the gate but the cost of a finding that arrives late. What each agent needs
  is its own module tree, not its own build directory:
  roughly 30 GB.
