/**
 * Phase 2's lettering criteria: every kind renders, wraps, detects overflow and applies its
 * policy; and moving a balloon preserves its tail without detaching it.
 *
 * Every test here runs with **no font configured** — measurement is injected, so a stub stands
 * in for real metrics. That is the point: the wrapping and policy rules are logic, and logic
 * that needs a font installed to run is logic that will not run in CI.
 *
 * @module test/lettering
 */

import { test } from 'node:test';
import { must, refused } from './helpers.js';
import assert from 'node:assert/strict';
import {
  fitText, wrap, stubMeasure, requiredFrame, moveTail, rect, bottom, right, quantize,
} from '../shared/domain/index.js';
import { buildRegistry } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { History } from '../shared/domain/history.js';
import { createBundle } from '../shared/domain/project.js';

const STYLE = { family: 'Comic Neue', sizePt: 12 };
const measure = stubMeasure(0.5);

/** @returns {{ registry: any, ctx: any }} */
const harness = () => ({
  registry: buildRegistry(),
  ctx: { bundle: createBundle('Lettering'), history: new History(), env: { onServer: true, now: () => 0 }, services: { measure } },
});

/** Build a page with one panel of the given size, returning the panel id. */
async function pageWithPanel(ctx, registry, width, height) {
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'T' });
  ctx.bundle = created.value.bundle;
  const page = await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'single' });
  const panelId = ctx.bundle.project.pages[page.value.pageId].panelIds[0];
  await dispatch(registry, ctx, 'comic.panel.setFrame', { panelId, x: 0, y: 0, width, height });
  return { pageId: page.value.pageId, panelId };
}

test('a line breaks at the last space that fits, and a word wider than the line is broken rather than dropped', () => {
  const lines = wrap('ONE TWO THREE FOUR', 60, STYLE, measure);
  assert.ok(lines.length > 1);
  for (const line of lines) assert.ok(measure(line, STYLE) <= 60, `"${line}" fits`);

  const long = wrap('SUPERCALIFRAGILISTICEXPIALIDOCIOUS', 40, STYLE, measure);
  assert.ok(long.length > 1, 'an unbreakable word is split');
  assert.equal(long.join(''), 'SUPERCALIFRAGILISTICEXPIALIDOCIOUS', 'and no character is lost');
});

test('a blank line and an empty string are handled without inventing content', () => {
  assert.deepEqual(wrap('', 100, STYLE, measure), ['']);
  assert.deepEqual(wrap('A\n\nB', 100, STYLE, measure), ['A', '', 'B']);
});

test('the warn policy keeps the text at its size and reports the overflow', () => {
  const fit = fitText({
    text: 'MANY WORDS THAT WILL NOT FIT IN A SMALL BALLOON AT ALL',
    frame: rect(0, 0, 80, 30), style: STYLE, measure, policy: 'warn',
  });
  assert.equal(fit.overflowed, true);
  assert.equal(fit.shrunk, false);
  assert.equal(fit.fontSize, STYLE.sizePt, 'warn does not change the size');
});

test('the shrink policy reduces the size until it fits, and stops at the floor', () => {
  const frame = rect(0, 0, 120, 60);
  const fit = fitText({
    text: 'A MODEST AMOUNT OF DIALOGUE THAT NEEDS A LITTLE LESS TYPE',
    frame, style: STYLE, measure, policy: 'shrink', minScale: 0.7,
  });
  assert.equal(fit.overflowed, false, 'shrink found a size that fits');
  assert.equal(fit.shrunk, true);
  assert.ok(fit.fontSize < STYLE.sizePt && fit.fontSize >= STYLE.sizePt * 0.7);
  assert.ok(fit.usedHeight <= frame.height - 16 + 1e-6, 'the shrunken text fits the frame');

  // Nothing fits in a frame this small, so the floor is what the caller gets — and the
  // overflow is still reported rather than hidden behind an unreadable size.
  const impossible = fitText({
    text: 'AN ENORMOUS AMOUNT OF DIALOGUE FOR A TINY BALLOON THAT CANNOT HOLD IT',
    frame: rect(0, 0, 40, 20), style: STYLE, measure, policy: 'shrink', minScale: 0.9,
  });
  assert.equal(impossible.fontSize, quantize(STYLE.sizePt * 0.9), 'it stopped at the floor');
  assert.equal(impossible.overflowed, true, 'and said so');
});

test('the resize-frame policy reports the frame the text needs', () => {
  const frame = rect(0, 0, 60, 30);
  const fit = fitText({
    text: 'THIS NEEDS MORE ROOM THAN IT HAS BEEN GIVEN HERE',
    frame, style: STYLE, measure, policy: 'resize-frame',
  });
  assert.equal(fit.overflowed, true, 'the policy does not itself move the frame');
  const needed = requiredFrame(fit);
  assert.ok(needed.height > frame.height, `expected a taller frame than ${frame.height}, got ${needed.height}`);
});

test('all three lettering kinds are one entity with a kind, and each is created', async () => {
  const { registry, ctx } = harness();
  const { panelId } = await pageWithPanel(ctx, registry, 600, 400);

  for (const kind of ['balloon', 'caption', 'sound-effect']) {
    const added = await dispatch(registry, ctx, 'comic.lettering.add', {
      panelId, kind, text: `${kind} text`,
    });
    assert.equal(added.ok, true, added.ok ? '' : added.message);
    assert.equal(ctx.bundle.project.lettering[must(added).letteringId].kind, kind);

    // A sound effect has no speaker, so it creates no dialogue. The other two do.
    if (kind === 'sound-effect') assert.equal(must(added).dialogueId, null);
    else assert.ok(must(added).dialogueId, `${kind} should have a dialogue record`);
  }

  const listed = await dispatch(registry, ctx, 'comic.lettering.list', { pageId: Object.keys(ctx.bundle.project.pages)[0] });
  assert.equal(listed.value.lettering.length, 3);
});

test('the resize-frame policy grows the frame when the command applies it', async () => {
  const { registry, ctx } = harness();
  const { panelId } = await pageWithPanel(ctx, registry, 400, 300);
  const added = await dispatch(registry, ctx, 'comic.lettering.add', {
    panelId, kind: 'caption', text: 'THIS IS A CAPTION THAT WILL NOT FIT IN ITS FRAME',
    overflowPolicy: 'resize-frame', x: 0, y: 0, width: 80, height: 20,
  });
  assert.equal(added.ok, true, added.ok ? '' : added.message);
  assert.ok(must(added).frame.height > 20, `the frame grew to ${must(added).frame.height}`);
});

test('setting text keeps the dialogue and the lettering in step — one text, two views', async () => {
  const { registry, ctx } = harness();
  const { panelId } = await pageWithPanel(ctx, registry, 600, 400);
  const added = await dispatch(registry, ctx, 'comic.lettering.add', { panelId, kind: 'balloon', text: 'FIRST' });
  const { letteringId, dialogueId } = added.value;

  await dispatch(registry, ctx, 'comic.lettering.setText', { letteringId, text: 'SECOND' });
  assert.equal(ctx.bundle.project.lettering[letteringId].text, 'SECOND');
  assert.equal(ctx.bundle.project.dialogue[dialogueId].text, 'SECOND', 'the dialogue did not drift from the lettering');
});

test('moving a balloon moves its tail with it and never detaches it', async () => {
  const { registry, ctx } = harness();
  const { panelId } = await pageWithPanel(ctx, registry, 900, 700);
  const added = await dispatch(registry, ctx, 'comic.lettering.add', {
    panelId, kind: 'balloon', text: 'MOVE ME', x: 100, y: 100, width: 300, height: 100,
    tailToX: 140, tailToY: 300,
  });
  assert.equal(added.ok, true);
  const { letteringId } = added.value;
  const lettering = ctx.bundle.project.lettering[letteringId];
  assert.ok(lettering.tailNodeId, 'a balloon gets a tail');

  const tailBefore = ctx.bundle.document.nodes[String(lettering.tailNodeId)].frame;
  const balloonBefore = ctx.bundle.document.nodes[String(lettering.nodeId)].frame;
  const tipBefore = { x: tailBefore.x + tailBefore.width / 2, y: tailBefore.y + tailBefore.height / 2 };

  const moved = await dispatch(registry, ctx, 'comic.lettering.move', { letteringId, dx: 60, dy: -40 });
  assert.equal(moved.ok, true, moved.ok ? '' : moved.message);

  // The criterion: the tail is preserved *or updated*, and attached. Never detached.
  assert.equal(must(moved).tail.attached, true, 'the tail is still attached');
  assert.ok(lettering.tailNodeId !== undefined && lettering.tailNodeId !== null, 'and still has its object');

  const balloonAfter = ctx.bundle.document.nodes[String(lettering.nodeId)].frame;
  assert.equal(quantize(balloonAfter.x - balloonBefore.x), 60);
  assert.equal(quantize(balloonAfter.y - balloonBefore.y), -40);

  const tailAfter = ctx.bundle.document.nodes[String(lettering.tailNodeId)].frame;
  const tipAfter = { x: tailAfter.x + tailAfter.width / 2, y: tailAfter.y + tailAfter.height / 2 };
  assert.notDeepEqual(tipAfter, tipBefore, 'the tail followed the balloon rather than staying behind');

  // The tip must stay on the side it was pointing: a tail that flips sides has detached in
  // every way that matters to a reader.
  const beforeSide = Math.sign(tipBefore.y - (balloonBefore.y + balloonBefore.height / 2));
  const afterSide = Math.sign(tipAfter.y - (balloonAfter.y + balloonAfter.height / 2));
  assert.equal(afterSide, beforeSide, 'the tail still points the same way');
});

test('moveTail keeps the tip on the same side, on its own terms', () => {
  const frame = rect(0, 0, 200, 100);
  const tail = { base: { x: 100, y: 50 }, tip: { x: 140, y: 300 } };
  const moved = moveTail({ x: 0, y: 500 }, frame, tail);
  assert.ok(moved.tip.y > 500, 'the tip moved down with the balloon');
  assert.ok(moved.tip.x > 100, 'and stayed to the right');
  // The base sits on the balloon's edge, so the tail is visually attached rather than floating.
  assert.ok(moved.base.y >= 500 + 100 / 2 - 1e-6);
});

test('an overflowing caption is listed as overflowing, so the UI can mark it', async () => {
  const { registry, ctx } = harness();
  const { pageId, panelId } = await pageWithPanel(ctx, registry, 600, 400);
  await dispatch(registry, ctx, 'comic.lettering.add', {
    panelId, kind: 'caption', text: 'A VERY LONG CAPTION INDEED THAT CANNOT POSSIBLY FIT IN THIS SMALL BOX',
    x: 0, y: 0, width: 70, height: 24, overflowPolicy: 'warn',
  });
  const listed = await dispatch(registry, ctx, 'comic.lettering.list', { pageId });
  assert.equal(listed.value.overflowed, 1);
  assert.ok(listed.value.lettering[0].lines.length > 1, 'and the wrapped lines are reported');
});

test('deleting lettering takes its objects and its dialogue with it', async () => {
  const { registry, ctx } = harness();
  const { panelId } = await pageWithPanel(ctx, registry, 600, 400);
  const added = await dispatch(registry, ctx, 'comic.lettering.add', { panelId, kind: 'balloon', text: 'BYE' });
  const { letteringId, dialogueId, nodeId } = added.value;
  const lettering = ctx.bundle.project.lettering[letteringId];
  const tailId = lettering.tailNodeId;

  const deleted = await dispatch(registry, ctx, 'comic.lettering.delete', { letteringId });
  assert.equal(deleted.ok, true);
  assert.equal(ctx.bundle.project.lettering[letteringId], undefined);
  assert.equal(ctx.bundle.project.dialogue[dialogueId], undefined);
  assert.equal(ctx.bundle.document.nodes[String(nodeId)], undefined, 'the text object went too');
  assert.equal(ctx.bundle.document.nodes[String(tailId)], undefined, 'and the tail');
  assert.deepEqual(ctx.bundle.project.panels[panelId].letteringIds, []);
});

test('the frame needed by a fit is the text plus its padding, and never smaller than the text', () => {
  const fit = fitText({ text: 'ONE\nTWO\nTHREE', frame: rect(0, 0, 500, 500), style: STYLE, measure, policy: 'warn' });
  const needed = requiredFrame(fit, 8);
  assert.ok(needed.height >= fit.usedHeight, 'the frame holds the lines');
  assert.ok(needed.width >= fit.usedWidth, 'and the widest line');
  assert.equal(needed.height, Math.ceil(fit.usedHeight + 16));
});

test('a tail keeps its length when the balloon moves, so it never collapses or stretches', () => {
  const frame = rect(0, 0, 200, 100);
  const tail = { base: { x: 100, y: 50 }, tip: { x: 100, y: 150 } };
  const before = Math.hypot(tail.tip.x - tail.base.x, tail.tip.y - tail.base.y);
  const moved = moveTail({ x: 25, y: 25 }, frame, tail);
  const after = Math.hypot(moved.tip.x - moved.base.x, moved.tip.y - moved.base.y);
  assert.equal(quantize(after), quantize(before), 'the tail is the same length');
  void bottom; void right;
});
