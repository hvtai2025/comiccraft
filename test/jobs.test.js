/**
 * M4's exit criteria: a character bible that versions references, capability-based providers,
 * pollable jobs, provenance on every asset, and reconciliation that refuses to overwrite newer
 * state.
 * @module test/jobs
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { createBundle } from '../shared/domain/project.js';
import { createJobManager } from '../server/jobs/manager.js';
import { createProviders } from '../server/ai/providers.js';
import { canApply } from '../shared/domain/jobs.js';

/** @param {any} session */
async function pageWithPanel(session) {
  must(await session.run('comic.project.create', { title: 'Jobs' }));
  const page = must(await session.run('comic.page.add', { name: 'P', template: 'single' }));
  return { pageId: page.pageId, panelId: session.bundle.project.pages[page.pageId].panelIds[0] };
}

test('providers declare capabilities, and the offline provider needs no credentials', async (t) => {
  const session = sessionHarness(t);
  const listed = must(await session.run('comic.provider.list', {}));
  const local = listed.providers.find((/** @type {any} */ p) => p.id === 'local-vector');
  assert.ok(local);
  assert.equal(local.configured, true);
  assert.ok(local.capabilities.includes('text-to-image'));
  const http = listed.providers.find((/** @type {any} */ p) => p.id === 'http');
  assert.equal(http.configured, false, 'no credentials are configured in the test environment');
  assert.equal(listed.defaultProvider, 'local-vector');
});

test('submit returns before the provider runs; a result is an asset awaiting review, not a document edit', async (t) => {
  const session = sessionHarness(t);
  const { panelId } = await pageWithPanel(session);
  const panel = session.bundle.project.panels[panelId];
  const refsBefore = [...panel.artworkRefs];

  const submitted = must(await session.run('comic.job.submit', {
    operation: 'text-to-image', prompt: 'A rainy rooftop at night', panelId,
  }));
  assert.equal(submitted.jobId.startsWith('job_'), true);
  assert.equal(submitted.status, 'queued');
  assert.deepEqual(session.bundle.project.panels[panelId].artworkRefs, refsBefore, 'submit did not touch the panel');

  const job = await session.jobs.settled(submitted.jobId);
  assert.equal(job.status, 'awaiting_review');
  assert.deepEqual(session.bundle.project.panels[panelId].artworkRefs, refsBefore, 'completion did not touch the panel either');

  const assetId = job.resultAssetIds[0];
  const asset = session.bundle.project.assets[assetId];
  assert.ok(asset, 'the result is an asset');
  assert.equal(asset.assetType, 'generated');
  assert.equal(asset.approvalStatus, 'pending');
  assert.equal(asset.provenance.provider, 'local-vector');
  assert.equal(asset.provenance.model, 'deterministic-vector-v1');
  assert.equal(asset.provenance.request.prompt, 'A rainy rooftop at night');
  assert.ok(asset.data.startsWith('data:image/svg+xml'));
  assert.equal(job.targetRevision, session.bundle.project.revision);

  const serialized = JSON.stringify(session.bundle);
  assert.ok(!serialized.includes('COMICCRAFT_AI_API_KEY'), 'a credential name is never written into the bundle');
});

test('apply writes one undoable artwork replacement and keeps the superseded asset', async (t) => {
  const session = sessionHarness(t);
  const { panelId } = await pageWithPanel(session);
  const first = must(await session.run('comic.artwork.place', {
    panelId, name: 'Old art', contentHash: 'old-hash',
  }));
  const oldAssetId = first.assetId;

  const submitted = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'New art', panelId }));
  const job = await session.jobs.settled(submitted.jobId);
  assert.equal(job.status, 'awaiting_review');

  const applied = must(await session.run('comic.job.apply', { jobId: submitted.jobId }));
  assert.equal(applied.panelId, panelId);
  assert.ok(session.bundle.project.panels[panelId].artworkRefs.includes(applied.artworkNodeId));
  assert.ok(session.bundle.project.assets[oldAssetId], 'the old asset is retained, not destroyed');
  assert.equal(session.bundle.project.assets[applied.assetId].approvalStatus, 'approved');
  assert.equal(applied.supersededAssetId, oldAssetId);
  assert.ok(session.bundle.project.jobs[submitted.jobId].appliedAt > 0);
  const validated = must(await session.run('comic.project.validate', {}));
  assert.equal(validated.errors, 0, 'removing the superseded object does not leave a dangling asset node');

  must(await session.run('comic.edit.undo', {}));
  assert.deepEqual(session.bundle.project.panels[panelId].artworkRefs, [first.artworkNodeId], 'applying is one undo step');
});

test('a job built against an older revision is refused rather than overwriting newer work', async (t) => {
  const session = sessionHarness(t);
  const { panelId } = await pageWithPanel(session);
  const submitted = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'Later', panelId }));
  const job = await session.jobs.settled(submitted.jobId);
  assert.equal(job.status, 'awaiting_review');

  // The user keeps editing while the job runs; the revision moves past the one the job saw.
  must(await session.run('comic.panel.move', { panelId, dx: 5, dy: 5 }));
  const conflict = refused(await session.run('comic.job.apply', { jobId: submitted.jobId }));
  assert.equal(conflict.kind, 'conflict');
  assert.match(conflict.message, /was built against revision/);
  assert.equal(session.bundle.project.panels[panelId].artworkRefs.length, 0, 'nothing was applied');
});

test('cancelling a queued job leaves the project untouched and the result is not kept', async (t) => {
  const session = sessionHarness(t);
  const { panelId } = await pageWithPanel(session);
  const submitted = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'Cancel me', panelId }));
  const cancelled = must(await session.run('comic.job.cancel', { jobId: submitted.jobId }));
  assert.equal(cancelled.status, 'cancelled');
  const job = await session.jobs.settled(submitted.jobId);
  assert.equal(job.status, 'cancelled');
  assert.deepEqual(job.resultAssetIds, []);
  assert.deepEqual(Object.keys(session.bundle.project.assets), []);
  assert.deepEqual(session.bundle.project.panels[panelId].artworkRefs, []);
});

test('undo never reissues a job or asset id allocated after the snapshot', async (t) => {
  const session = sessionHarness(t);
  const { panelId } = await pageWithPanel(session);
  must(await session.run('comic.panel.move', { panelId, dx: 2, dy: 2 }));
  const first = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'first', panelId }));
  const firstJob = await session.jobs.settled(first.jobId);
  const firstAssetId = firstJob.resultAssetIds[0];

  must(await session.run('comic.edit.undo', {}));
  const second = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'second', panelId }));
  const secondJob = await session.jobs.settled(second.jobId);
  assert.notEqual(second.jobId, first.jobId, 'the job id was not reissued');
  assert.notEqual(secondJob.resultAssetIds[0], firstAssetId, 'the asset id was not reissued');
  assert.equal(session.bundle.project.assets[firstAssetId].provenance.request.prompt, 'first', 'the first asset was not overwritten');
});

test('undoing an edit made before a job submission keeps the job and its result', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Before job' }));
  const page = must(await session.run('comic.page.add', { name: 'P', template: 'single' }));
  const panelId = session.bundle.project.pages[page.pageId].panelIds[0];
  const submitted = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'keep me', panelId }));
  const job = await session.jobs.settled(submitted.jobId);
  assert.equal(job.status, 'awaiting_review');

  must(await session.run('comic.edit.undo', {}));
  assert.ok(session.bundle.project.jobs[submitted.jobId], 'the provenance record survives an unrelated undo');
  assert.ok(session.bundle.project.assets[job.resultAssetIds[0]], 'and so does its generated asset');
});

test('a page-scoped undo does not delete a generated asset created after its snapshot', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Preserve result' }));
  const first = must(await session.run('comic.page.add', { name: 'A', template: 'two-up' }));
  const second = must(await session.run('comic.page.add', { name: 'B', template: 'single' }));
  const target = session.bundle.project.pages[second.pageId].panelIds[0];

  must(await session.run('comic.page.template.apply', { pageId: first.pageId, template: 'three-up', replace: true }));
  const submitted = must(await session.run('comic.job.submit', { operation: 'text-to-image', prompt: 'keep result', panelId: target }));
  const job = await session.jobs.settled(submitted.jobId);
  const assetId = job.resultAssetIds[0];

  must(await session.run('comic.edit.undo', {}));
  assert.ok(session.bundle.project.assets[assetId], 'the generated result survives the unrelated undo');
  assert.ok(session.bundle.project.jobs[submitted.jobId], 'and so does its job');
  const applied = must(await session.run('comic.job.apply', { jobId: submitted.jobId }));
  assert.equal(applied.assetId, assetId);
});

test('a provider failure leaves the project and approved artwork exactly as they were', async () => {
  const bundle = createBundle('Failure');
  const panelId = 'panel_missing';
  const jobManager = createJobManager({
    getBundle: () => bundle,
    now: () => 5,
    schedule: (_ms, fn) => setTimeout(fn, 0),
    providers: {
      defaultId: () => 'broken',
      resolve: () => ({ ok: true, provider: { generate: async () => { throw new Error('the model fell over'); } }, model: 'm' }),
      get: () => ({ generate: async () => { throw new Error('the model fell over'); } }),
    },
  });
  const submitted = jobManager.submit({ operation: 'text-to-image', prompt: 'anything' });
  assert.equal(submitted.ok, true);
  const job = await jobManager.settled(submitted.job.id);
  assert.equal(job.status, 'failed');
  assert.equal(job.error.code, 'provider-failed');
  assert.match(job.error.message, /fell over/);
  assert.deepEqual(Object.keys(bundle.project.assets), []);
  void panelId;
});

test('canApply also refuses a target that no longer exists', () => {
  const bundle = createBundle('Gone');
  bundle.project.jobs.job_1 = {
    id: 'job_1', status: 'awaiting_review', targetRevision: bundle.project.revision,
    targetPanelId: 'panel_999', resultAssetIds: ['asset_1'],
  };
  const verdict = canApply(bundle.project.jobs.job_1, bundle.project);
  assert.equal(verdict.ok, false);
  assert.match(/** @type {any} */ (verdict).reason, /no longer exists/);
});

test('the character bible versions approved references instead of rewriting them', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Bible' }));
  const character = must(await session.run('comic.character.add', { name: 'Maya' }));
  must(await session.run('comic.character.update', {
    characterId: character.characterId,
    canonicalDescription: 'Short black hair, red jacket',
    face: 'round face, one scar over the left brow',
    clothing: 'red windbreaker',
    continuityRules: ['the scar is always on the left'],
  }));
  const asset1 = must(await session.run('comic.asset.register', {
    name: 'Maya v1', contentHash: 'maya-v1', assetType: 'reference', data: 'data:image/svg+xml;base64,PHN2Zy8+',
  }));
  const asset2 = must(await session.run('comic.asset.register', {
    name: 'Maya v2', contentHash: 'maya-v2', assetType: 'reference', data: 'data:image/svg+xml;base64,PHN2Zy8+',
  }));
  must(await session.run('comic.character.reference.add', { characterId: character.characterId, assetId: asset1.assetId }));
  const approved1 = must(await session.run('comic.character.approveDesign', { characterId: character.characterId, assetId: asset1.assetId }));
  const approved2 = must(await session.run('comic.character.approveDesign', { characterId: character.characterId, assetId: asset2.assetId }));

  assert.equal(approved1.designVersion, 1);
  assert.equal(approved2.designVersion, 2);
  assert.equal(approved2.supersededAssetId, asset1.assetId);
  const record = session.bundle.project.characters[character.characterId];
  assert.equal(record.approvedDesignVersion, asset2.assetId);
  assert.equal(record.designVersions.length, 2, 'the previous design version is kept');
  assert.ok(record.referenceAssetIds.includes(asset1.assetId));
});

test('assets are content-addressed and approval is a decision, never a deletion', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Assets' }));
  const first = must(await session.run('comic.asset.register', { name: 'One', contentHash: 'same-hash', data: 'abc' }));
  const again = must(await session.run('comic.asset.register', { name: 'One again', contentHash: 'same-hash', data: 'abc' }));
  assert.equal(again.assetId, first.assetId);
  assert.equal(again.deduplicated, true);

  const rejected = must(await session.run('comic.asset.reject', { assetId: first.assetId, note: 'not this one' }));
  assert.equal(rejected.approvalStatus, 'rejected');
  assert.ok(session.bundle.project.assets[first.assetId].data, 'the bytes stay available');
  const approved = must(await session.run('comic.asset.approve', { assetId: first.assetId }));
  assert.equal(approved.approvalStatus, 'approved');
  must(await session.run('comic.edit.undo', {}));
  assert.equal(session.bundle.project.assets[first.assetId].approvalStatus, 'rejected', 'undo restores the approval decision');
  must(await session.run('comic.edit.redo', {}));
  assert.equal(session.bundle.project.assets[first.assetId].approvalStatus, 'approved');
});
