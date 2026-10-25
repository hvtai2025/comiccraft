/**
 * M8's release criteria on the representative book, plus the failure paths a gate must prove:
 * an interrupted save, a deleted artboard, a retired-id reuse, and long-book timings.
 * @module test/release
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { buildTestBook } from './book.js';
import { buildRegistry } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { History } from '../shared/domain/history.js';
import { createBundle } from '../shared/domain/project.js';
import { serialize, deserialize } from '../shared/domain/bundle.js';
import { validateBundle, errorsOf } from '../shared/domain/validate.js';
import { createMemoryStorage } from '../server/storage/fs.js';
import { createRenderer } from '../server/export/renderer.js';
import { deleteArtboard } from '../shared/domain/document.js';

test('the representative book builds, validates and exports for screen, print and proofing', async (t) => {
  const session = sessionHarness(t);
  await buildTestBook(session);

  const validated = must(await session.run('comic.book.validate', { preset: 'print' }));
  assert.equal(validated.errors, 0, `the book has errors: ${JSON.stringify(validated.problems.filter((p) => p.severity === 'error'))}`);
  assert.ok(validated.problems.some((/** @type {any} */ p) => p.code === 'missing-font'), 'the fontless host reports its missing face');

  const print = must(await session.run('comic.book.export', { path: 'out/book.pdf', preset: 'print', format: 'pdf' }));
  assert.equal(print.pages, 5);
  const pdf = await session.storage.readBytes('out/book.pdf');
  assert.ok(pdf);
  assert.equal(new TextDecoder().decode(pdf.subarray(0, 5)), '%PDF-');

  const proof = must(await session.run('comic.book.export', {
    path: 'out/proof', preset: 'proofing', format: 'images', scale: 0.18,
  }));
  assert.equal(proof.pages, 5);
  assert.ok(proof.files.some((/** @type {any} */ f) => f.path === 'out/proof/contact-sheet.png'));
});

test('save and reopen preserve every id in the book, and the package round trip agrees', async (t) => {
  const session = sessionHarness(t);
  const book = await buildTestBook(session);
  const before = {
    pages: book.pages.slice().sort(),
    panels: book.panels.slice().sort(),
    characters: book.characters.slice().sort(),
    nodes: Object.keys(session.bundle.document.nodes).sort(),
    assets: Object.keys(session.bundle.project.assets).sort(),
    lettering: Object.keys(session.bundle.project.lettering).sort(),
  };

  must(await session.run('comic.project.save', { path: 'book' }));
  must(await session.run('comic.project.open', { path: 'book' }));
  const after = {
    pages: Object.keys(session.bundle.project.pages).sort(),
    panels: Object.keys(session.bundle.project.panels).sort(),
    characters: Object.keys(session.bundle.project.characters).sort(),
    nodes: Object.keys(session.bundle.document.nodes).sort(),
    assets: Object.keys(session.bundle.project.assets).sort(),
    lettering: Object.keys(session.bundle.project.lettering).sort(),
  };
  assert.deepEqual(after, before, 'every id survived the save/reopen round trip');
  assert.deepEqual(errorsOf(validateBundle(session.bundle)), []);
});

test('panel and lettering edits are reversible as one step each', async (t) => {
  const session = sessionHarness(t);
  const book = await buildTestBook(session);
  const panelId = book.panels[0];
  const frameNodeId = session.bundle.project.panels[panelId].frameNodeId;
  const frameBefore = { ...session.bundle.document.nodes[String(frameNodeId)].frame };

  must(await session.run('comic.panel.move', { panelId, dx: 37, dy: 19 }));
  const moved = { ...session.bundle.document.nodes[String(frameNodeId)].frame };
  assert.notDeepEqual(moved, frameBefore);
  must(await session.run('comic.edit.undo', {}));
  assert.deepEqual(session.bundle.document.nodes[String(frameNodeId)].frame, frameBefore, 'one undo restores the frame');
  must(await session.run('comic.edit.redo', {}));
  assert.deepEqual(session.bundle.document.nodes[String(frameNodeId)].frame, moved, 'and redo restores the edit');

  const letteringId = Object.keys(session.bundle.project.lettering)[0];
  const node = session.bundle.document.nodes[String(session.bundle.project.lettering[letteringId].nodeId)];
  const positionBefore = { ...node.frame };
  must(await session.run('comic.lettering.move', { letteringId, dx: -20, dy: 30 }));
  must(await session.run('comic.edit.undo', {}));
  const restored = session.bundle.document.nodes[String(session.bundle.project.lettering[letteringId].nodeId)];
  assert.deepEqual(restored.frame, positionBefore);
});

test('an interrupted save is survivable: the manifest is last, and a retry opens cleanly', async () => {
  const base = createMemoryStorage();
  const registry = buildRegistry();
  const history = new History();
  /** @type {{ bundle: any }} */
  const state = { bundle: createBundle('Interrupted') };
  let failAfter = Infinity;
  let writes = 0;
  const flaky = {
    ...base,
    async write(/** @type {string} */ path, /** @type {string} */ data) {
      writes += 1;
      if (writes > failAfter) throw new Error('simulated disk full');
      await base.write(path, data);
    },
  };
  const ctx = { bundle: state.bundle, history, env: { onServer: true, now: () => 0 }, services: { storage: flaky, renderer: createRenderer() } };
  const created = must(await dispatch(registry, ctx, 'comic.project.create', { title: 'Interrupted' }));
  ctx.bundle = created.bundle;
  must(await dispatch(registry, ctx, 'comic.page.add', { name: 'Page 1', template: 'three-up' }));

  failAfter = 2;
  const failed = refused(await dispatch(registry, ctx, 'comic.project.save', { path: 'book' }));
  assert.equal(failed.kind, 'internal');
  assert.match(failed.message, /disk full/);
  // The manifest is written last, so the failed attempt left no readable manifest.
  const partial = await base.list('book');
  assert.ok(partial.some((path) => path === 'book/document.json'));
  assert.ok(!partial.includes('book/manifest.json'), 'the manifest was not written before the failure');

  failAfter = Infinity;
  must(await dispatch(registry, ctx, 'comic.project.save', { path: 'book' }));
  const opened = must(await dispatch(registry, ctx, 'comic.project.open', { path: 'book' }));
  assert.equal(errorsOf(opened.problems).length, 0);
  assert.equal(Object.keys(opened.bundle.project.pages).length, 1);
});

test('a deleted artboard is reported, and a retired artboard id is never reused', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Retired ids' }));
  const first = must(await session.run('comic.page.add', { name: 'One', template: 'single' }));
  const oldArtboard = session.bundle.project.pages[first.pageId].artboardId;

  must(await session.run('comic.page.delete', { pageId: first.pageId }));
  const second = must(await session.run('comic.page.add', { name: 'Two', template: 'single' }));
  assert.notEqual(session.bundle.project.pages[second.pageId].artboardId, oldArtboard);
  assert.ok(session.bundle.document.retiredArtboards.includes(oldArtboard));

  // Simulate an external edit that drops an artboard the page still points at.
  const live = session.bundle.project.pages[second.pageId].artboardId;
  deleteArtboard(session.bundle.document, live);
  const errors = errorsOf(validateBundle(session.bundle));
  assert.ok(errors.some((problem) => problem.code === 'missing-artboard'),
    `expected a missing-artboard error, got ${errors.map((p) => p.code).join(', ')}`);
});

test('a long book stays within the recorded load, validate and render budgets', async (t) => {
  const session = sessionHarness(t);
  const started = performance.now();
  await buildTestBook(session, { pages: 18 });
  const buildMs = performance.now() - started;

  const serializeStart = performance.now();
  const serialized = serialize(session.bundle);
  const serializeMs = performance.now() - serializeStart;
  assert.equal(serialized.ok, true);
  if (!serialized.ok) return;

  const loadStart = performance.now();
  const parsed = deserialize(serialized.files);
  assert.equal(parsed.ok, true);
  if (!parsed.ok) return;
  const problems = validateBundle(parsed.bundle);
  const loadMs = performance.now() - loadStart;

  const pageId = Object.keys(parsed.bundle.project.pages)[0];
  const renderStart = performance.now();
  const png = createRenderer().pageToPng(parsed.bundle, parsed.bundle.project.pages[pageId], 0.35);
  const renderMs = performance.now() - renderStart;

  t.diagnostic(`book: ${Object.keys(parsed.bundle.project.pages).length} pages, build ${buildMs.toFixed(0)}ms, serialize ${serializeMs.toFixed(0)}ms, load+validate ${loadMs.toFixed(0)}ms, render ${renderMs.toFixed(0)}ms`);
  assert.equal(errorsOf(problems).length, 0);
  assert.ok(png.length > 1000);
  // Generous ceilings: this is a regression alarm, not a benchmark. The diagnostic above is
  // the number to compare across changes.
  assert.ok(buildMs < 20_000, `building the book took ${buildMs.toFixed(0)}ms`);
  assert.ok(serializeMs < 2_000, `serialisation took ${serializeMs.toFixed(0)}ms`);
  assert.ok(loadMs < 3_000, `load and validation took ${loadMs.toFixed(0)}ms`);
  assert.ok(renderMs < 5_000, `rendering took ${renderMs.toFixed(0)}ms`);
});

test('a project opens, edits and exports with no provider configured and no fonts installed', async (t) => {
  const session = sessionHarness(t);
  const book = await buildTestBook(session);
  const providers = must(await session.run('comic.provider.list', {}));
  assert.equal(providers.defaultProvider, 'local-vector', 'the offline provider is the default');
  const exported = must(await session.run('comic.book.export', {
    path: 'out/providerless.pdf', preset: 'screen', format: 'pdf',
  }));
  assert.equal(exported.pages, book.pages.length);
  const validated = must(await session.run('comic.project.validate', {}));
  assert.equal(validated.errors, 0);
});
