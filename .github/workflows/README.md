# Workflows

Two sets live here, and they belong to different things.

## ComicCraft's gate

| Workflow | What it does |
|---|---|
| [`verify.yml`](verify.yml) | The gate: `npm run verify` on Node 22 and 24, with no Rust toolchain and no font files. Fast. This is the one that guards the product |

`verify.yml` is the CI form of the same command the local gate runs and the verify skill
records as evidence (plan §17.1). A second workflow is not added unless it guards something
`verify` cannot.

## The reference implementation's release pipeline

These belong to **VectorCraft**, the audited baseline. They build and sign the Rust desktop
application for each platform. ComicCraft has no desktop binary and no Rust build, so none of
them run for ComicCraft and none of them are part of its gate.

| Workflow | What it does | Depends on |
|---|---|---|
| [`release.yml`](release.yml) | Builds signed installers per platform, drafts the GitHub release | cargo, signing secrets, `packaging/` |
| [`windows7.yml`](windows7.yml) | Builds the Windows 7 target, which needs a pinned nightly and a statically linked CRT | cargo, a Windows runner, `rust-src` |
| [`windows-arm64.yml`](windows-arm64.yml) | Builds the Windows ARM64 target | cargo, a Windows runner |
| [`freebsd.yml`](freebsd.yml) | Builds and tests on FreeBSD in a VM | cargo, a FreeBSD VM |
| [`affinity-corpus.yml`](affinity-corpus.yml) | Runs the Affinity import corpus tests | cargo, `crates/affinity` |
| [`packaging-lint.yml`](packaging-lint.yml) | Static checks on the release pipeline and the Linux desktop metadata | none (seconds, no build) |

### What happens to them at M0

M0 moves the audited baseline to `reference/vectorcraft/` and builds the ComicCraft layout
over the top. At that point these six files travel with the reference tree, because that is
what they describe — they are not ComicCraft's CI, they are the reference's.

They are left in place until then rather than deleted up front, because deleting them would
destroy the only description of how that tree is built, for a restructure that has not
happened yet. **Delete them in the same commit that moves the reference tree**, not before.

### The rule that matters

**Nothing under `.github/workflows/` for ComicCraft may require a Rust toolchain, a font
file, or a browser.** The gate is `npm run verify`, and a test that needs any of the three
is a broken test on this stack (plan §17.1). If a workflow here starts installing `rustup`
to make a check pass, the check is wrong, not the workflow.
