/**
 * M7's exit criteria: preset geometry, the validation gate in front of export, a deterministic
 * package round trip, previews and contact sheets, and editable data independent of output.
 * @module test/publish
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { PRESETS, presetGeometry } from '../shared/domain/publish.js';
import { bookToPdf, PT_PER_UNIT } from '../server/export/pdf.js';
import { packageProject } from '../server/export/package.js';
import { createHash } from 'node:crypto';

/**
 * @param {any} session
 * @param {{ withData?: boolean }} [options]
 */
async function book(session, options = {}) {
  must(await session.run('comic.project.create', { title: 'Publish' }));
  const page = must(await session.run('comic.page.add', { name: 'Page 1', template: 'three-up' }));
  const panels = session.bundle.project.pages[page.pageId].panelIds;
  for (const panelId of panels) {
    must(await session.run('comic.artwork.place', {
      panelId, name: 'Art', contentHash: `publish-${panelId}`,
      ...(options.withData ? { data: 'data:image/svg+xml;base64,PHN2Zy8+' } : {}),
    }));
  }
  must(await session.run('comic.lettering.add', {
    panelId: panels[0], kind: 'balloon', text: 'A LINE OF DIALOGUE', overflowPolicy: 'shrink',
  }));
  return { pageId: page.pageId, panels };
}

test('preset geometry reserves bleed and marks on print, and nothing on screen', () => {
  const size = { width: 1275, height: 1950 };
  const screen = presetGeometry(size, PRESETS.screen);
  assert.deepEqual(screen.media, size);
  assert.deepEqual(screen.trim, { x: 0, y: 0, width: 1275, height: 1950 });
  assert.equal(screen.marks, false);

  const print = presetGeometry(size, PRESETS.print);
  assert.equal(print.marks, true);
  assert.equal(print.media.width, 1275 + (36 + 60) * 2);
  assert.deepEqual(print.trim, { x: 96, y: 96, width: 1275, height: 1950 });
  assert.deepEqual(print.bleed, { x: 60, y: 60, width: 1275 + 72, height: 1950 + 72 });
  assert.deepEqual(print.contentOffset, { x: 96, y: 96 });
});

test('a print PDF carries MediaBox, TrimBox and BleedBox at the preset geometry', async (t) => {
  const session = sessionHarness(t);
  const { pageId } = await book(session, { withData: true });
  const page = session.bundle.project.pages[pageId];
  const artboard = session.bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  const pdf = bookToPdf(session.bundle, [page], { preset: 'print' });
  const text = pdf.toString('latin1');
  const geometry = presetGeometry(artboard.size, PRESETS.print);

  const media = /\/MediaBox \[0 0 ([\d.]+) ([\d.]+)\]/.exec(text);
  assert.ok(media);
  assert.equal(Number(media[1]), Math.round(geometry.media.width * PT_PER_UNIT * 1e4) / 1e4);
  const trim = /\/TrimBox \[([\d.]+) ([\d.]+) ([\d.]+) ([\d.]+)\]/.exec(text);
  assert.ok(trim, 'the trim box is declared');
  assert.equal(Number(trim[1]), Math.round(96 * PT_PER_UNIT * 1e4) / 1e4);
  assert.ok(/\/BleedBox \[/.test(text), 'the bleed box is declared');
  assert.ok(text.includes(' cm'), 'the content is translated into the trim box');
  assert.ok(text.includes(' m ') && text.includes(' l S'), 'crop marks are drawn');
  const streams = [...text.matchAll(/stream\n([\s\S]*?)\nendstream/g)].map((match) => match[1]);
  assert.ok(streams.length > 0);
  for (const stream of streams) {
    const opens = (stream.match(/(?:^|\s)q(?:\s|$)/g) ?? []).length;
    const closes = (stream.match(/(?:^|\s)Q(?:\s|$)/g) ?? []).length;
    assert.equal(opens, closes, 'q and Q are balanced in every content stream');
  }
});

test('the command path forwards the preset, so page.export really emits trim and bleed', async (t) => {
  const session = sessionHarness(t);
  const { pageId } = await book(session, { withData: true });
  must(await session.run('comic.page.export', {
    pageId, path: 'out/print-page.pdf', format: 'pdf', preset: 'print',
  }));
  const pdf = await session.storage.readBytes('out/print-page.pdf');
  assert.ok(pdf);
  const text = new TextDecoder('latin1').decode(pdf);
  assert.ok(text.includes('/TrimBox ['), 'the renderer received the print preset');
  assert.ok(text.includes('/BleedBox ['));
});

test('the export gate refuses a missing asset for print, and screen reports it as a warning', async (t) => {
  const session = sessionHarness(t);
  await book(session, { withData: false });

  const print = must(await session.run('comic.book.validate', { preset: 'print' }));
  assert.equal(print.ready, false);
  assert.ok(print.problems.some((/** @type {any} */ p) => p.code === 'missing-asset-bytes' && p.severity === 'error'),
    `expected a missing-asset error, got ${JSON.stringify(print.problems)}`);

  const screen = must(await session.run('comic.book.validate', { preset: 'screen' }));
  assert.equal(screen.ready, true, 'a screen preview may draw a labelled placeholder');
  assert.ok(screen.problems.some((/** @type {any} */ p) => p.code === 'missing-asset-bytes' && p.severity === 'warning'));

  const blocked = refused(await session.run('comic.book.export', { path: 'out/book.pdf', preset: 'print' }));
  assert.equal(blocked.kind, 'conflict');
  assert.match(blocked.message, /missing bytes|no bytes/i);
});

test('a configured host fails a strict print export when the project face is absent', async (t) => {
  const session = sessionHarness(t, { fonts: { families: ['Some Other Face'] } });
  await book(session, { withData: true });
  const print = must(await session.run('comic.book.validate', { preset: 'print' }));
  assert.ok(print.problems.some((/** @type {any} */ p) => p.code === 'missing-font' && p.severity === 'error'));
  const blocked = refused(await session.run('comic.book.export', { path: 'out/book.pdf', preset: 'print' }));
  assert.equal(blocked.kind, 'conflict');
  assert.match(blocked.message, /missing font/i);

  const allowed = must(await session.run('comic.book.export', {
    path: 'out/book.pdf', preset: 'print', allowMissingFonts: true, allowMissingAssets: true,
  }));
  assert.equal(allowed.preset, 'print');
});

test('with no fonts installed the project still exports, with the missing face reported', async (t) => {
  const session = sessionHarness(t);
  await book(session, { withData: true });
  const validated = must(await session.run('comic.book.validate', { preset: 'print' }));
  assert.equal(validated.ready, true);
  assert.ok(validated.problems.some((/** @type {any} */ p) => p.code === 'missing-font' && p.severity === 'warning'));
  const exported = await session.run('comic.book.export', { path: 'out/fontless.pdf', preset: 'print' });
  assert.equal(exported.ok, true, exported.ok ? '' : exported.message);
});

test('image export writes previews, a contact sheet and a machine-readable report', async (t) => {
  const session = sessionHarness(t);
  await book(session, { withData: true });
  const exported = must(await session.run('comic.book.export', {
    path: 'out/images', format: 'images', preset: 'proofing', scale: 0.2,
  }));
  assert.equal(exported.pages, 1);
  assert.ok(exported.files.some((/** @type {any} */ f) => f.path === 'out/images/page-001.png'));
  assert.ok(exported.files.some((/** @type {any} */ f) => f.path === 'out/images/contact-sheet.png'));
  assert.ok(exported.files.some((/** @type {any} */ f) => f.path === 'out/images/export.json'));

  const png = await session.storage.readBytes('out/images/page-001.png');
  assert.ok(png instanceof Uint8Array);
  assert.deepEqual([...png.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
  const sheet = await session.storage.readBytes('out/images/contact-sheet.png');
  assert.deepEqual([...sheet.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
  const report = JSON.parse(/** @type {string} */ (await session.storage.read('out/images/export.json')));
  assert.equal(report.preset, 'proofing');
});

test('page render returns data without writing, and page export writes the requested preset', async (t) => {
  const session = sessionHarness(t);
  const { pageId } = await book(session, { withData: true });
  const rendered = must(await session.run('comic.page.render', { pageId, format: 'png', scale: 0.2 }));
  assert.ok(rendered.data.startsWith('data:image/png;base64,'));
  assert.equal(rendered.width > 0, true);

  const exported = must(await session.run('comic.page.export', {
    pageId, path: 'out/page.pdf', format: 'pdf', preset: 'screen',
  }));
  assert.equal(exported.format, 'pdf');
  const pdf = await session.storage.readBytes('out/page.pdf');
  assert.ok(pdf);
  assert.equal(new TextDecoder().decode(pdf.subarray(0, 5)), '%PDF-');
});

test('package, unpack, reopen preserves every id, and packaging twice is byte-identical', async (t) => {
  const session = sessionHarness(t);
  await book(session, { withData: true });
  const idsBefore = {
    pages: Object.keys(session.bundle.project.pages).sort(),
    panels: Object.keys(session.bundle.project.panels).sort(),
    lettering: Object.keys(session.bundle.project.lettering).sort(),
    nodes: Object.keys(session.bundle.document.nodes).sort(),
  };

  const first = packageProject(session.bundle);
  const second = packageProject(session.bundle);
  assert.equal(first.ok, true);
  assert.equal(second.ok, true);
  if (!first.ok || !second.ok) return;
  assert.equal(
    createHash('sha256').update(first.bytes).digest('hex'),
    createHash('sha256').update(second.bytes).digest('hex'),
    'the package is deterministic',
  );

  must(await session.run('comic.project.package', { path: 'pkg/comic.comiccraft' }));
  const unpacked = must(await session.run('comic.project.unpack', {
    archivePath: 'pkg/comic.comiccraft', path: 'reopened',
  }));
  assert.ok(unpacked.files > 0);

  const reopened = must(await session.run('comic.project.open', { path: 'reopened' }));
  assert.equal(reopened.errors, 0);
  const idsAfter = {
    pages: Object.keys(session.bundle.project.pages).sort(),
    panels: Object.keys(session.bundle.project.panels).sort(),
    lettering: Object.keys(session.bundle.project.lettering).sort(),
    nodes: Object.keys(session.bundle.document.nodes).sort(),
  };
  assert.deepEqual(idsAfter, idsBefore, 'every id survived the package round trip');
});

test('rendered output is not authoritative: deleting exports leaves the project valid', async (t) => {
  const session = sessionHarness(t);
  await book(session, { withData: true });
  must(await session.run('comic.book.export', { path: 'out/deliver', format: 'both', preset: 'screen' }));
  await session.storage.remove('out/deliver');
  const validated = must(await session.run('comic.project.validate', {}));
  assert.equal(validated.errors, 0);
  const repackaged = packageProject(session.bundle);
  assert.equal(repackaged.ok, true);
});
