# M4 (Node.js stack) — Character consistency and image generation

**Status:** Complete.
**Gate:** `npm run verify` green, including `test/jobs.test.js`.
**Exit criteria source:** [`comiccraft-porting-plan.md`](../comiccraft-porting-plan.md) §10.

## Exit criteria

| Criterion | Evidence |
|---|---|
| `comic.job.submit` returns a job id and never blocks | manager `submit` records the job and schedules the provider on the next turn; test asserts status `queued` before the run |
| Status, cancellation and application are separate commands | `comic.job.status`, `.cancel`, `.apply`, `.reject`; tests |
| `comic.job.apply` refuses to overwrite a target changed since `target_revision` and reports the conflict | `canApply` + revision check; test edits the panel after submit and asserts a conflict with the current revision in the message |
| A failed or cancelled job leaves the project and approved artwork untouched | provider-failure test and cancelled-job test both assert no asset and no artwork change |
| Every asset records provider, model and request | `Asset.provenance` written by the manager and `comic.asset.register`; tests assert the fields |
| The project opens, edits and exports with no provider configured | the default provider is the offline deterministic vector painter; `test/release.test.js` exports with no credentials |

## What landed

- `shared/domain/jobs.js`: statuses, terminal states, `canApply`, review decisions, monotonic revisions.
- `server/ai/providers.js`: capability descriptors, an offline deterministic provider, and an optional HTTP adapter that reads credentials from the environment only.
- `server/jobs/manager.js`: pollable in-process jobs, durable `jobs/<id>.json` metadata, cooperative cancellation, result assets awaiting review.
- Character bible: `comic.character.update`, `.reference.add`, `.approveDesign` (versions preserved, old designs retained).
- Assets: `comic.asset.register/list/get/approve/reject`; content-addressed, never deleted on rejection.
- `unlinkAssetNodes` keeps asset provenance valid when a panel or artwork object is removed.

## Deliberate limits

- The default provider paints deterministic SVG rather than calling a model. It is honest about that in its descriptor and provenance; the lifecycle above is the deliverable, not a specific model.
- The HTTP adapter is not exercised by the gate (no network, no credentials). Its descriptor reports `configured: false` unless both endpoint and key are set.
- Provider credentials never enter a job, asset, bundle, log or MCP tool definition; the boundary test forbids `fetch` in `shared/` and `public/`.
