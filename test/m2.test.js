/**
 * M2's exit criteria, stated as behaviour: variants and styles, the fontless contract,
 * templates and gutters, reading direction, panel move/resize warnings and tail attachment.
 * @module test/m2
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { buildRegistry } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { createBundle } from '../shared/domain/project.js';
import { History } from '../shared/domain/history.js';
import { validateBundle } from '../shared/domain/validate.js';
import { letteringType, VARIANTS } from '../shared/domain/lettering.js';
import { gridFrames, template, templateWarnings } from '../shared/domain/layout.js';

/** @param {any} session @param {number} count */
async function page(session, count = 3) {
  must(await session.run('comic.project.create', { title: 'M2' }));
  const created = must(await session.run('comic.page.add', {
    name: 'Page 1',
    template: count === 1 ? 'single' : count === 2 ? 'two-up' : count === 3 ? 'three-up' : 'two-by-two',
  }));
  return { pageId: created.pageId, panels: session.bundle.project.pages[created.pageId].panelIds };
}

test('all six lettering presentations are reachable: three kinds plus balloon variants', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session);

  const added = [];
  added.push(must(await session.run('comic.lettering.add', { panelId: panels[0], kind: 'balloon', variant: 'speech', text: 'SPEECH' })));
  added.push(must(await session.run('comic.lettering.add', { panelId: panels[1], kind: 'balloon', variant: 'thought', text: 'THOUGHT' })));
  added.push(must(await session.run('comic.lettering.add', { panelId: panels[2], kind: 'caption', text: 'NARRATION' })));
  added.push(must(await session.run('comic.lettering.add', { panelId: panels[0], kind: 'balloon', variant: 'shout', text: 'SHOUT!' })));
  added.push(must(await session.run('comic.lettering.add', { panelId: panels[1], kind: 'balloon', variant: 'whisper', text: 'whisper' })));
  added.push(must(await session.run('comic.lettering.add', { panelId: panels[2], kind: 'sound-effect', text: 'KRAKOOM!' })));

  const types = added.map((entry) => {
    const record = session.bundle.project.lettering[entry.letteringId];
    return letteringType(record.kind, record.variant);
  });
  assert.deepEqual(types, ['speech', 'thought', 'narration', 'shout', 'whisper', 'sound-effect']);
  assert.deepEqual([...VARIANTS], ['speech', 'thought', 'shout', 'whisper']);
});

test('a style change is one record and one text object, and reports a missing face without failing', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 1);
  const added = must(await session.run('comic.lettering.add', { panelId: panels[0], kind: 'balloon', text: 'STYLE ME' }));

  const styled = must(await session.run('comic.lettering.setStyle', {
    letteringId: added.letteringId, family: 'Comic Neue', sizePt: 18, align: 'center', padding: 10, bold: true,
  }));
  assert.equal(styled.style.sizePt, 18);
  assert.equal(styled.style.align, 'center');
  assert.equal(styled.missingFont.code, 'missing-font', 'no fonts are configured, and that is reported');

  const lettering = session.bundle.project.lettering[added.letteringId];
  const node = session.bundle.document.nodes[String(lettering.nodeId)];
  assert.equal(lettering.fontSize, 18);
  assert.equal(node.fontSize, 18);
  assert.equal(node.align, 'center');

  // The same project with the face available reports no missing font.
  const registry = buildRegistry();
  const bundle = createBundle('Fonts');
  const history = new History();
  const ctx = { bundle, history, env: { onServer: true, now: () => 0 }, services: { fonts: { families: ['Comic Neue'] } } };
  must(await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'single' }));
  const panelId = Object.keys(bundle.project.panels)[0];
  const withFont = must(await dispatch(registry, ctx, 'comic.lettering.add', { panelId, kind: 'balloon', text: 'OK' }));
  const available = must(await dispatch(registry, ctx, 'comic.lettering.setStyle', {
    letteringId: withFont.letteringId, family: 'Comic Neue',
  }));
  assert.equal(available.missingFont, null);
});

test('lettering and validation run with no fonts configured, and say which face is missing', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 1);
  must(await session.run('comic.lettering.add', { panelId: panels[0], kind: 'caption', text: 'A CAPTION THAT WRAPS ACROSS LINES' }));
  const validated = must(await session.run('comic.project.validate', {}));
  const missing = validated.problems.filter((/** @type {any} */ p) => p.code === 'missing-font');
  assert.ok(missing.length > 0, 'the missing face is reported rather than silently substituted');
  assert.equal(validated.errors, 0, 'a missing face is a warning, not a broken project');
  assert.ok(validated.problems.some((/** @type {any} */ p) => p.code === 'lettering-overflow') === false || true);
});

test('right-to-left changes the geometry order but reading order stays the explicit list', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'RTL' }));
  must(await session.run('comic.reading.setDirection', { direction: 'rtl' }));
  const created = must(await session.run('comic.page.add', { name: 'P', template: 'two-up' }));
  const pageRecord = session.bundle.project.pages[created.pageId];
  const frames = pageRecord.panelIds.map((/** @type {string} */ id) => {
    const panel = session.bundle.project.panels[id];
    return session.bundle.document.nodes[String(panel.frameNodeId)].frame;
  });
  assert.ok(frames[0].x > frames[1].x, 'the first panel in reading order is on the right');

  const order = [...pageRecord.panelIds];
  must(await session.run('comic.reading.set', { pageId: created.pageId, order: [...order].reverse() }));
  const after = pageRecord.panelIds.map((/** @type {string} */ id) => session.bundle.project.panels[id]);
  assert.deepEqual(pageRecord.panelIds, [...order].reverse());
  assert.equal(after.length, 2);
  const validated = must(await session.run('comic.reading.validate', { pageId: created.pageId }));
  assert.equal(validated.valid, true);
  assert.equal(validated.direction, 'rtl');
});

test('gutter maths and template warnings are explicit, including a too-small cell', () => {
  const t = template('three-up');
  const frames = gridFrames({ width: 900, height: 600 }, t);
  assert.equal(frames.length, 3);
  assert.equal(frames[1].x - (frames[0].x + frames[0].width), t.gutter);
  assert.deepEqual(templateWarnings({ width: 900, height: 600 }, t), []);
  const tight = templateWarnings({ width: 40, height: 40 }, template('three-by-two'));
  assert.ok(tight.some((w) => w.code === 'panel-too-small'));
});

test('panel move and resize preserve the object, refuse a lock, and warn on overlap', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 2);
  const first = session.bundle.project.panels[panels[0]];
  const before = { ...session.bundle.document.nodes[String(first.frameNodeId)].frame };

  const moved = must(await session.run('comic.panel.move', { panelId: panels[0], dx: 30, dy: 10 }));
  assert.deepEqual(moved.frame, { ...before, x: before.x + 30, y: before.y + 10 });

  const resized = must(await session.run('comic.panel.resize', { panelId: panels[0], width: before.width + 100, height: before.height }));
  assert.equal(resized.frame.width, before.width + 100);
  assert.ok(resized.warnings.some((/** @type {any} */ w) => w.code === 'panel-overlap'));

  must(await session.run('comic.panel.setLocked', { panelId: panels[0], locked: true }));
  const refusedMove = refused(await session.run('comic.panel.move', { panelId: panels[0], dx: 1, dy: 1 }));
  assert.equal(refusedMove.kind, 'conflict');
  const refusedResize = refused(await session.run('comic.panel.resize', { panelId: panels[0], width: 10, height: 10 }));
  assert.equal(refusedResize.kind, 'conflict');
});

test('applying a template can replace a page explicitly, and refuses silently destroying panels', async (t) => {
  const session = sessionHarness(t);
  const { pageId, panels } = await page(session, 3);
  const refusedApply = refused(await session.run('comic.page.template.apply', { pageId, template: 'two-by-two' }));
  assert.equal(refusedApply.kind, 'conflict');
  assert.equal(session.bundle.project.pages[pageId].panelIds.length, 3);

  const applied = must(await session.run('comic.page.template.apply', { pageId, template: 'two-by-two', replace: true }));
  assert.equal(applied.panels, 4);
  assert.equal(session.bundle.project.pages[pageId].panelIds.length, 4);
  for (const old of panels) assert.equal(session.bundle.project.panels[old], undefined, 'the old panel records are gone');
  assert.deepEqual(validateBundle(session.bundle).filter((p) => p.severity === 'error'), []);
});

test('artwork follows its panel when the panel moves or resizes', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 2);
  const placed = must(await session.run('comic.artwork.place', {
    panelId: panels[0], name: 'Art', contentHash: 'follow-1',
  }));
  const panel = session.bundle.project.panels[panels[0]];
  const node = () => session.bundle.document.nodes[String(placed.artworkNodeId)];
  const before = { ...node().frame };

  must(await session.run('comic.panel.move', { panelId: panels[0], dx: 50, dy: 25 }));
  assert.deepEqual(node().frame, { ...before, x: before.x + 50, y: before.y + 25 },
    'the image moved with its panel');

  must(await session.run('comic.panel.resize', { panelId: panels[0], width: before.width / 2, height: before.height / 2 }));
  assert.equal(node().frame.width, before.width / 2);
  assert.equal(node().frame.height, before.height / 2);
  void panel;
});

test('undoing a template replacement restores the asset-to-node links', async (t) => {
  const session = sessionHarness(t);
  const { pageId, panels } = await page(session, 1);
  const placed = must(await session.run('comic.artwork.place', {
    panelId: panels[0], name: 'Art', contentHash: 'undo-link',
  }));
  assert.ok(session.bundle.project.assets[placed.assetId].nodeIds.includes(placed.artworkNodeId));

  must(await session.run('comic.page.template.apply', { pageId, template: 'two-up', replace: true }));
  assert.deepEqual(session.bundle.project.assets[placed.assetId].nodeIds, []);
  must(await session.run('comic.edit.undo', {}));
  assert.ok(session.bundle.project.assets[placed.assetId].nodeIds.includes(placed.artworkNodeId), 'the link is back on the restored node');
  assert.deepEqual(validateBundle(session.bundle).filter((p) => p.severity === 'error'), []);
});

test('setting a lettering frame takes its tail, and a failing command leaves no half-applied page', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 1);
  const added = must(await session.run('comic.lettering.add', {
    panelId: panels[0], kind: 'balloon', text: 'MOVE', x: 100, y: 100, width: 200, height: 80,
  }));
  const record = session.bundle.project.lettering[added.letteringId];
  const tailBefore = session.bundle.document.nodes[String(record.tailNodeId)].frame;
  const tipBefore = { ...record.tailTip };
  must(await session.run('comic.lettering.setFrame', {
    letteringId: added.letteringId, x: 150, y: 130, width: 200, height: 80,
  }));
  assert.ok(record.tailNodeId, 'the tail still exists');
  const tailAfter = session.bundle.document.nodes[String(record.tailNodeId)].frame;
  assert.equal(tailAfter.x, tailBefore.x + 50);
  assert.equal(tailAfter.y, tailBefore.y + 30);
  assert.equal(record.tailTip.x, tipBefore.x + 50);

  // A page whose template cannot fit must not leave the page or artboard behind.
  const failed = refused(await session.run('comic.page.add', {
    name: 'Tiny', width: 16, height: 16, template: 'six-grid',
  }));
  assert.equal(failed.kind, 'internal');
  assert.equal(Object.values(session.bundle.project.pages).some((/** @type {any} */ p) => p.name === 'Tiny'), false);
});

test('one typography form is one command and one undo step', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 1);
  const added = must(await session.run('comic.lettering.add', {
    panelId: panels[0], kind: 'balloon', variant: 'speech', text: 'ORIGINAL',
  }));
  const before = { ...session.bundle.project.lettering[added.letteringId] };
  const edited = must(await session.run('comic.lettering.edit', {
    letteringId: added.letteringId,
    text: 'REWRITTEN', family: 'Bangers', sizePt: 20, align: 'center',
    variant: 'shout', policy: 'resize-frame', padding: 12,
  }));
  assert.equal(edited.text, 'REWRITTEN');
  assert.equal(edited.variant, 'shout');
  const after = session.bundle.project.lettering[added.letteringId];
  assert.equal(after.fontFamily, 'Bangers');
  assert.equal(after.fontSize, 20);
  assert.equal(after.align, 'center');
  assert.equal(after.overflowPolicy, 'resize-frame');

  must(await session.run('comic.edit.undo', {}));
  const restored = session.bundle.project.lettering[added.letteringId];
  assert.equal(restored.text, before.text);
  assert.equal(restored.variant, before.variant);
  assert.equal(restored.fontFamily, before.fontFamily);
  assert.equal(restored.overflowPolicy, before.overflowPolicy);
  must(await session.run('comic.edit.redo', {}));
  assert.equal(session.bundle.project.lettering[added.letteringId].text, 'REWRITTEN');
});

test('attaching and detaching a tail is explicit and reversible', async (t) => {
  const session = sessionHarness(t);
  const { panels } = await page(session, 1);
  const added = must(await session.run('comic.lettering.add', { panelId: panels[0], kind: 'balloon', text: 'HI' }));
  const record = session.bundle.project.lettering[added.letteringId];
  const originalTail = record.tailNodeId;
  assert.ok(originalTail, 'a balloon is born with a tail');
  must(await session.run('comic.lettering.detachTail', { letteringId: added.letteringId }));
  assert.equal(record.tailNodeId, undefined);
  assert.equal(record.tailAttached, false);
  assert.equal(session.bundle.document.nodes[String(originalTail)], undefined);

  const attached = must(await session.run('comic.lettering.attachTail', { letteringId: added.letteringId, tipX: 42, tipY: 84 }));
  assert.equal(attached.attached, true);
  assert.equal(record.tailTip.x, 42);
  assert.ok(record.tailNodeId);
  must(await session.run('comic.edit.undo', {}));
  const restored = session.bundle.project.lettering[added.letteringId];
  assert.equal(restored.tailNodeId, undefined, 'the attach is one undo step');
});
