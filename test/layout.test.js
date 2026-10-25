/**
 * Phase 2's layout criteria: templates, gutters, validation warnings, and reading order that
 * is **independent of panel coordinates**.
 * @module test/layout
 */

import { test } from 'node:test';
import { must, refused } from './helpers.js';
import assert from 'node:assert/strict';
import {
  TEMPLATES, template, gridFrames, findOverlaps, framesOutsidePage, degenerateFrames,
  rect, right, bottom, quantize,
} from '../shared/domain/index.js';
import { createBundle, createArtboard, addNode } from '../shared/domain/index.js';
import { allocate } from '../shared/domain/project.js';
import { buildRegistry } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { History } from '../shared/domain/history.js';

const PAGE = { width: 1200, height: 1800 };

/** @returns {{ registry: any, ctx: any }} */
function harness() {
  const bundle = createBundle('Layout');
  return {
    registry: buildRegistry(),
    ctx: { bundle, history: new History(), env: { onServer: true, now: () => 0 }, services: {} },
  };
}

test('every named template produces the frame count it advertises', () => {
  for (const [name, t] of Object.entries(TEMPLATES)) {
    const frames = gridFrames(PAGE, t);
    assert.equal(frames.length, t.rows * t.cols, `${name} should produce ${t.rows * t.cols} frames`);
    assert.deepEqual(degenerateFrames(frames), [], `${name} produced a degenerate frame`);
  }
});

test('a gutter is space *between* panels, so a zero gutter tiles the page exactly', () => {
  const tiled = template('two-by-two');
  const frames = gridFrames(PAGE, { ...tiled, gutter: 0 });
  assert.equal(quantize(frames[0].width * 2), PAGE.width, 'two columns fill the width');
  assert.equal(quantize(frames[0].height * 2), PAGE.height, 'two rows fill the height');
  assert.equal(quantize(right(frames[1])), PAGE.width, 'the second column ends at the page edge');
  assert.equal(quantize(bottom(frames[3])), PAGE.height, 'the last row ends at the page edge');
});

test('a template with a gutter leaves exactly the gutter between neighbours', () => {
  const frames = gridFrames(PAGE, { rows: 2, cols: 2, gutter: 24, margin: 0 });
  assert.equal(quantize(frames[1].x - right(frames[0])), 24, 'the horizontal gap is the gutter');
  assert.equal(quantize(frames[2].y - bottom(frames[0])), 24, 'the vertical gap is the gutter');
  const total = frames[0].width * 2 + 24;
  assert.equal(quantize(total), PAGE.width, 'panels plus gutters fill the page exactly');
});

test('a template that cannot fit is refused rather than producing negative panels', () => {
  assert.throws(
    () => gridFrames({ width: 100, height: 100 }, { rows: 8, cols: 8, gutter: 40, margin: 0 }),
    /does not fit/,
  );
  assert.throws(() => template('does-not-exist'), /unknown template/);
});

test('panels that merely touch are not reported, and panels that hide art are', () => {
  const touching = [rect(0, 0, 100, 100), rect(100, 0, 100, 100)];
  assert.deepEqual(findOverlaps(touching), [], 'a shared border is a layout choice, not a defect');

  // 10 units wide and the full 100 tall: the area is what hides artwork, which is why the
  // report is an area rather than a yes/no.
  const hidden = [rect(0, 0, 100, 100), rect(90, 0, 100, 100)];
  const overlaps = findOverlaps(hidden);
  assert.equal(overlaps.length, 1);
  assert.deepEqual(overlaps[0], { a: 0, b: 1, overlapArea: 1000 });

  // A sliver below the threshold is tolerated: a 1-unit kiss is a rounding artefact, not a
  // panel hiding another's art.
  assert.deepEqual(findOverlaps([rect(0, 0, 100, 100), rect(99.9, 0, 100, 100)]), []);
});

test('a bleed panel is a warning, and a zero-size panel is a defect', () => {
  const outside = framesOutsidePage([rect(-10, 0, 100, 100), rect(0, 0, 50, 50)], PAGE);
  assert.deepEqual(outside, [0], 'the panel reaching past the page is reported; the other is not');
  assert.deepEqual(degenerateFrames([rect(0, 0, 0, 10), rect(0, 0, 10, 10)]), [0]);
  assert.deepEqual(framesOutsidePage([rect(0, 0, PAGE.width, PAGE.height)], PAGE), [], 'an exact fit is not outside');
});

test('panel commands report the warnings the criteria require', async () => {
  const { registry, ctx } = harness();
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'Warnings' });
  ctx.bundle = created.value.bundle;

  const page = await dispatch(registry, ctx, 'comic.page.add', { name: 'Page 1', template: 'three-up' });
  assert.equal(page.ok, true);
  const panelIds = ctx.bundle.project.pages[must(page).pageId].panelIds;

  // Reorder must not warn: moving a panel's reading position is not a geometry change.
  const reordered = await dispatch(registry, ctx, 'comic.panel.reorder', {
    pageId: must(page).pageId,
    order: [panelIds[2], panelIds[0], panelIds[1]],
  });
  assert.equal(reordered.ok, true, reordered.ok ? '' : reordered.message);
  assert.deepEqual(must(reordered).order, [panelIds[2], panelIds[0], panelIds[1]]);
  assert.deepEqual(ctx.bundle.project.pages[must(page).pageId].panelIds, [panelIds[2], panelIds[0], panelIds[1]]);

  // Overlap must warn.
  const moved = await dispatch(registry, ctx, 'comic.panel.setFrame', {
    panelId: panelIds[0], x: 10, y: 10, width: 600, height: 400,
  });
  assert.equal(moved.ok, true, moved.ok ? '' : moved.message);
  const overlapped = await dispatch(registry, ctx, 'comic.panel.setFrame', {
    panelId: panelIds[1], x: 100, y: 100, width: 600, height: 400,
  });
  assert.ok(
    overlapped.value.warnings.some((w) => w.code === 'panel-overlap'),
    `expected an overlap warning, got ${JSON.stringify(overlapped.value.warnings)}`,
  );

  // A degenerate frame is reported.
  const degenerate = await dispatch(registry, ctx, 'comic.panel.setFrame', {
    panelId: panelIds[2], x: 0, y: 0, width: 1, height: 1,
  });
  assert.deepEqual(degenerate.value.warnings.filter((w) => w.code === 'panel-degenerate'), []);
});

test('a reorder that drops or duplicates a panel is refused', async () => {
  const { registry, ctx } = harness();
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'Order' });
  ctx.bundle = created.value.bundle;
  const page = await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'two-by-two' });
  const ids = ctx.bundle.project.pages[must(page).pageId].panelIds;

  const dropped = await dispatch(registry, ctx, 'comic.panel.reorder', { pageId: must(page).pageId, order: ids.slice(0, 3) });
  assert.equal(dropped.ok, false);
  assert.equal(dropped.kind, 'bad-params');

  const duplicated = await dispatch(registry, ctx, 'comic.panel.reorder', {
    pageId: must(page).pageId, order: [ids[0], ids[0], ids[1], ids[2]],
  });
  assert.equal(duplicated.ok, false);
  assert.equal(duplicated.kind, 'bad-params');
  assert.deepEqual(ctx.bundle.project.pages[must(page).pageId].panelIds, ids, 'the failed reorder changed nothing');
});

test('reading order is independent of panel coordinates — the property, not the shape', async () => {
  const { registry, ctx } = harness();
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'Order vs geometry' });
  ctx.bundle = created.value.bundle;
  const page = await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'two-by-two' });
  const ids = ctx.bundle.project.pages[must(page).pageId].panelIds;

  /** @returns {number[][]} the frames in reading order */
  const framesInOrder = () => ctx.bundle.project.pages[must(page).pageId].panelIds
    .map((id) => ctx.bundle.document.nodes[String(ctx.bundle.project.panels[id].frameNodeId)].frame)
    .map((f) => [f.x, f.y]);

  const geometryBefore = framesInOrder();
  // Reverse the reading order. If order were derived from coordinates — a naive
  // left-to-right, top-to-bottom sort — the list would snap back and this test would fail.
  await dispatch(registry, ctx, 'comic.panel.reorder', { pageId: must(page).pageId, order: [...ids].reverse() });

  const geometryAfter = framesInOrder();
  assert.deepEqual(geometryAfter, [...geometryBefore].reverse(), 'only the order changed, not the geometry');
  assert.deepEqual(
    ctx.bundle.project.pages[must(page).pageId].panelIds,
    [...ids].reverse(),
    'the reversed order survived, so it is stored rather than derived',
  );

  // And the converse: moving a panel must not reorder the list.
  const orderBefore = [...ctx.bundle.project.pages[must(page).pageId].panelIds];
  await dispatch(registry, ctx, 'comic.panel.setFrame', { panelId: orderBefore[0], x: 900, y: 1400, width: 200, height: 200 });
  assert.deepEqual(ctx.bundle.project.pages[must(page).pageId].panelIds, orderBefore, 'a geometry change is not a reorder');
});

test('a locked panel refuses frame and artwork changes, and unlocking restores them', async () => {
  const { registry, ctx } = harness();
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'Lock' });
  ctx.bundle = created.value.bundle;
  const page = await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'single' });
  const panelId = ctx.bundle.project.pages[must(page).pageId].panelIds[0];
  const before = ctx.bundle.document.nodes[String(ctx.bundle.project.panels[panelId].frameNodeId)].frame;

  await dispatch(registry, ctx, 'comic.panel.setLocked', { panelId, locked: true });
  const refused = await dispatch(registry, ctx, 'comic.panel.setFrame', { panelId, x: 5, y: 5, width: 50, height: 50 });
  assert.equal(refused.ok, false);
  assert.equal(refused.kind, 'conflict');
  assert.deepEqual(
    ctx.bundle.document.nodes[String(ctx.bundle.project.panels[panelId].frameNodeId)].frame,
    before,
    'the refused change left the geometry alone',
  );

  await dispatch(registry, ctx, 'comic.panel.setLocked', { panelId, locked: false });
  const allowed = await dispatch(registry, ctx, 'comic.panel.setFrame', { panelId, x: 5, y: 5, width: 50, height: 50 });
  assert.equal(allowed.ok, true, allowed.ok ? '' : allowed.message);
});

test('artboard ids and node ids are allocated from one space, so a page never shadows a panel', () => {
  const bundle = createBundle('Id space');
  const board = createArtboard(bundle.document, 'P', PAGE);
  const node = addNode(bundle.document, board, {
    kind: 'frame', name: 'Panel', frame: rect(0, 0, 10, 10), fill: '#fff', stroke: null, strokeWidth: 1,
  });
  const panelId = allocate(bundle, 'panel');
  assert.notEqual(board, node);
  assert.notEqual(String(board), panelId);
});
