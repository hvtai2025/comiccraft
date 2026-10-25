/**
 * `comic.lettering.*` and `comic.dialogue.*` — balloons, captions and sound effects.
 *
 * One entity with a `kind`: a balloon, a caption and a sound effect differ in presentation,
 * not in structure, so they share a schema and a command group (design §5.2).
 *
 * Text measurement is **injected** through `ctx.services.measure`, so these commands run
 * unchanged in the browser (real canvas metrics) and in tests (a deterministic stub). That
 * is why no font file is needed for the lettering rules to be tested.
 *
 * @module shared/commands/commands/lettering
 */

import { fail, ok } from '../registry.js';
import { allocate } from '../../domain/project.js';
import { addNode, findArtboard, getNode, removeNode, updateNode } from '../../domain/document.js';
import { quantize, rect } from '../../domain/geom.js';
import { fitText, requiredFrame, stubMeasure } from '../../domain/lettering.js';
import { moveTail } from '../../domain/lettering.js';

/** The default measure, used when a host supplies none. Deterministic, never a real font. */
const defaultMeasure = stubMeasure(0.5);

/** @param {any} ctx @returns {import('../../domain/lettering.js').MeasureText} */
const measureOf = (ctx) => (ctx.services?.measure ?? defaultMeasure);

/** @param {any} bundle @param {string} letteringId */
const letteringOf = (bundle, letteringId) => bundle.project.lettering[letteringId] ?? null;

/**
 * Where a balloon sits inside its panel when no frame is given: centred, with a margin, so a
 * new balloon is never placed on top of the panel's edge.
 * @param {import('../../domain/geom.js').Rect} panelFrame
 * @returns {import('../../domain/geom.js').Rect}
 */
function defaultBalloonFrame(panelFrame) {
  const width = Math.max(40, Math.round(panelFrame.width * 0.6));
  const height = Math.max(30, Math.round(panelFrame.height * 0.22));
  return rect(
    quantize(panelFrame.x + (panelFrame.width - width) / 2),
    quantize(panelFrame.y + panelFrame.height - height - 12),
    quantize(width),
    quantize(height),
  );
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const letteringCommands = [
  {
    id: 'comic.lettering.add',
    label: 'Add Lettering',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        kind: { type: 'string', enum: ['balloon', 'caption', 'sound-effect'] },
        variant: { type: 'string', enum: ['speech', 'thought', 'shout', 'whisper'], description: 'Presentation for a balloon: speech, thought, shout or whisper. Omit for speech.' },
        text: { type: 'string', maxLength: 4000, description: 'The lettering\u2019s text. May be empty.' },
        characterId: { type: 'string', minLength: 1, description: 'Who says it, for a balloon. Creates the dialogue record.' },
        overflowPolicy: { type: 'string', enum: ['warn', 'shrink', 'resize-frame'] },
        x: { type: 'number' }, y: { type: 'number' },
        width: { type: 'number', minimum: 1 }, height: { type: 'number', minimum: 1 },
        tailToX: { type: 'number', description: 'Tip of the tail, for a balloon. Defaults to below the balloon.' },
        tailToY: { type: 'number' },
      },
      required: ['panelId', 'kind', 'text'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages', 'panels', 'lettering', 'dialogue'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const panel = bundle.project.panels[params.panelId];
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      const page = pageOfPanel(bundle, params.panelId);
      if (!page) return fail('conflict', `panel ${params.panelId} is not on any page`, 'panelId');
      const frameNode = getNode(bundle.document, panel.frameNodeId);
      if (!frameNode) return fail('conflict', `panel ${params.panelId} has no frame`, 'panelId');

      const hasBox = params.x !== undefined && params.y !== undefined && params.width !== undefined && params.height !== undefined;
      const frame = hasBox
        ? rect(quantize(params.x), quantize(params.y), quantize(params.width), quantize(params.height))
        : defaultBalloonFrame(frameNode.frame);

      const style = bundle.project.defaultLetteringStyle;
      const fit = fitText({
        text: params.text,
        frame,
        style,
        measure: measureOf(ctx),
        policy: params.overflowPolicy ?? 'warn',
      });

      let appliedFrame = frame;
      if (params.overflowPolicy === 'resize-frame' && fit.overflowed) {
        const needed = requiredFrame(fit);
        appliedFrame = rect(frame.x, frame.y, Math.max(frame.width, needed.width), Math.max(frame.height, needed.height));
      }

      const nodeId = addNode(bundle.document, page.artboardId, {
        kind: 'text',
        name: params.kind === 'sound-effect' ? 'Sound Effect' : params.kind === 'caption' ? 'Caption' : params.variant === 'thought' ? 'Thought Balloon' : 'Balloon',
        frame: appliedFrame,
        text: params.text,
        fontSize: style.sizePt,
        lineHeight: 1.25,
        align: params.kind === 'sound-effect' ? 'center' : 'left',
        color: params.kind === 'sound-effect' ? '#111111' : '#111111',
      });

      const letteringId = allocate(bundle, 'lettering');
      /** @type {any} */
      const lettering = {
        id: letteringId,
        kind: params.kind,
        nodeId,
        text: params.text,
        overflowPolicy: params.overflowPolicy ?? 'warn',
      };
      if (params.variant !== undefined) lettering.variant = params.variant;

      // A balloon gets a tail drawn to the speaker. The tail is its own object, so moving
      // the balloon updates the tail rather than unlinking it.
      if (params.kind === 'balloon') {
        const tipX = params.tailToX ?? quantize(appliedFrame.x + appliedFrame.width * 0.32);
        const tipY = params.tailToY ?? quantize(appliedFrame.y + appliedFrame.height + 36);
        const tailNodeId = addNode(bundle.document, page.artboardId, {
          kind: 'frame',
          name: 'Balloon Tail',
          frame: rect(quantize(tipX - 6), quantize(tipY - 6), 12, 12),
          fill: '#ffffff',
          stroke: '#111111',
          strokeWidth: 2,
        });
        lettering.tailNodeId = tailNodeId;
        lettering.tailTip = { x: quantize(tipX), y: quantize(tipY) };
        lettering.tailAttached = true;
      }

      bundle.project.lettering[letteringId] = lettering;
      panel.letteringIds.push(letteringId);

      let dialogueId = null;
      if (params.kind !== 'sound-effect') {
        dialogueId = allocate(bundle, 'dialogue');
        bundle.project.dialogue[dialogueId] = {
          id: dialogueId,
          characterId: params.characterId ?? null,
          letteringId,
          text: params.text,
        };
        panel.dialogueIds.push(dialogueId);
      }

      return ok({
        letteringId,
        nodeId,
        dialogueId,
        frame: appliedFrame,
        variant: lettering.variant ?? (params.kind === 'balloon' ? 'speech' : null),
        fit: { lines: fit.lines, fontSize: fit.fontSize, overflowed: fit.overflowed, shrunk: fit.shrunk },
      });
    },
  },

  {
    id: 'comic.lettering.setText',
    label: 'Set Lettering Text',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        text: { type: 'string', maxLength: 4000 },
      },
      required: ['letteringId', 'text'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering', 'dialogue'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const lettering = letteringOf(bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      const node = getNode(bundle.document, lettering.nodeId);
      if (!node) return fail('conflict', `lettering ${params.letteringId} has no object`, 'letteringId');

      lettering.text = params.text;
      updateNode(bundle.document, node.id, { text: params.text });

      const fit = fitText({
        text: params.text,
        frame: node.frame,
        style: bundle.project.defaultLetteringStyle,
        measure: measureOf(ctx),
        policy: lettering.overflowPolicy,
      });

      // The dialogue that presents this line keeps the same words: one text, two views of
      // it, and neither is allowed to drift from the other.
      for (const d of Object.values(bundle.project.dialogue)) {
        const e = /** @type {any} */ (d);
        if (e.letteringId === params.letteringId) e.text = params.text;
      }

      return ok({
        letteringId: params.letteringId,
        fit: { lines: fit.lines, fontSize: fit.fontSize, overflowed: fit.overflowed, shrunk: fit.shrunk },
      });
    },
  },

  {
    id: 'comic.lettering.move',
    label: 'Move Lettering',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        dx: { type: 'number' },
        dy: { type: 'number' },
      },
      required: ['letteringId', 'dx', 'dy'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering', 'panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const lettering = letteringOf(bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      const node = getNode(bundle.document, lettering.nodeId);
      if (!node) return fail('conflict', `lettering ${params.letteringId} has no object`, 'letteringId');

      const before = { ...node.frame };
      const moved = rect(
        quantize(before.x + params.dx),
        quantize(before.y + params.dy),
        before.width,
        before.height,
      );
      updateNode(bundle.document, node.id, { frame: moved });

      // The tail follows the balloon. It is never detached: a balloon that loses its tail
      // when moved is a balloon whose speaker became ambiguous.
      let tail = null;
      if (lettering.tailNodeId !== undefined && lettering.tailNodeId !== null) {
        const tip = lettering.tailTip ?? { x: before.x + before.width * 0.32, y: before.y + before.height + 36 };
        const tipNode = getNode(bundle.document, lettering.tailNodeId);
        if (tipNode) {
          const next = moveTail(
            { x: params.dx, y: params.dy },
            before,
            { base: centreOf(before), tip },
          );
          lettering.tailTip = { x: quantize(next.tip.x), y: quantize(next.tip.y) };
          updateNode(bundle.document, tipNode.id, {
            frame: rect(quantize(next.tip.x - 6), quantize(next.tip.y - 6), 12, 12),
          });
          tail = { nodeId: tipNode.id, base: next.base, tip: next.tip, attached: true };
        }
      }

      return ok({ letteringId: params.letteringId, frame: moved, tail });
    },
  },

  {
    id: 'comic.lettering.fit',
    label: 'Fit Lettering',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        policy: { type: 'string', enum: ['warn', 'shrink', 'resize-frame'] },
      },
      required: ['letteringId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const lettering = letteringOf(bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      const node = getNode(bundle.document, lettering.nodeId);
      if (!node) return fail('conflict', `lettering ${params.letteringId} has no object`, 'letteringId');

      const policy = params.policy ?? lettering.overflowPolicy;
      lettering.overflowPolicy = policy;
      const fit = fitText({
        text: lettering.text,
        frame: node.frame,
        style: bundle.project.defaultLetteringStyle,
        measure: measureOf(ctx),
        policy,
      });

      let frame = node.frame;
      if (policy === 'resize-frame' && fit.overflowed) {
        const needed = requiredFrame(fit);
        frame = rect(frame.x, frame.y, Math.max(frame.width, needed.width), Math.max(frame.height, needed.height));
        updateNode(bundle.document, node.id, { frame });
      }
      if (policy === 'shrink') {
        updateNode(bundle.document, node.id, { fontSize: fit.fontSize });
      }

      return ok({
        letteringId: params.letteringId,
        policy,
        frame,
        fit: { lines: fit.lines, fontSize: fit.fontSize, overflowed: fit.overflowed, shrunk: fit.shrunk },
      });
    },
  },

  {
    id: 'comic.lettering.list',
    label: 'List Lettering',
    params: {
      type: 'object',
      properties: { pageId: { type: 'string', minLength: 1 } },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const { bundle } = ctx;
      const page = bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const out = [];
      for (const panelId of page.panelIds) {
        const panel = bundle.project.panels[panelId];
        if (!panel) continue;
        for (const lid of panel.letteringIds) {
          const l = bundle.project.lettering[lid];
          if (!l) continue;
          const node = getNode(bundle.document, l.nodeId);
          const fit = node
            ? fitText({
                text: l.text,
                frame: node.frame,
                style: bundle.project.defaultLetteringStyle,
                measure: measureOf(ctx),
                policy: l.overflowPolicy,
              })
            : null;
          out.push({
            letteringId: l.id,
            kind: l.kind,
            panelId,
            text: l.text,
            overflowPolicy: l.overflowPolicy,
            frame: node?.frame ?? null,
            overflowed: fit?.overflowed ?? true,
            lines: fit?.lines ?? [],
            tailNodeId: l.tailNodeId ?? null,
          });
        }
      }
      return ok({ pageId: params.pageId, lettering: out, overflowed: out.filter((l) => l.overflowed).length });
    },
  },

  {
    id: 'comic.lettering.delete',
    label: 'Delete Lettering',
    params: {
      type: 'object',
      properties: { letteringId: { type: 'string', minLength: 1 } },
      required: ['letteringId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels', 'lettering', 'dialogue'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const lettering = letteringOf(bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');

      for (const d of Object.values(bundle.project.dialogue)) {
        const e = /** @type {any} */ (d);
        if (e.letteringId !== params.letteringId) continue;
        delete bundle.project.dialogue[e.id];
        for (const panel of Object.values(bundle.project.panels)) {
          const p = /** @type {any} */ (panel);
          const i = p.dialogueIds.indexOf(e.id);
          if (i !== -1) p.dialogueIds.splice(i, 1);
        }
      }
      removeNode(bundle.document, lettering.nodeId);
      if (lettering.tailNodeId !== undefined && lettering.tailNodeId !== null) {
        removeNode(bundle.document, lettering.tailNodeId);
      }
      for (const panel of Object.values(bundle.project.panels)) {
        const p = /** @type {any} */ (panel);
        const i = p.letteringIds.indexOf(params.letteringId);
        if (i !== -1) p.letteringIds.splice(i, 1);
      }
      delete bundle.project.lettering[params.letteringId];
      return ok({ letteringId: params.letteringId });
    },
  },

  {
    id: 'comic.character.add',
    label: 'Add Character',
    params: {
      type: 'object',
      properties: {
        name: { type: 'string', minLength: 1, maxLength: 200 },
        description: { type: 'string', maxLength: 2000, description: 'Continuity note: what this character must look like in every panel.' },
      },
      required: ['name'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['characters'] },
    run(ctx, params) {
      const id = allocate(ctx.bundle, 'character');
      ctx.bundle.project.characters[id] = {
        id,
        name: params.name,
        description: params.description ?? '',
        referenceAssetIds: [],
      };
      return ok({ characterId: id, name: params.name });
    },
  },
];

/** @param {import('../../domain/geom.js').Rect} r */
const centreOf = (r) => ({ x: r.x + r.width / 2, y: r.y + r.height / 2 });

/**
 * @param {any} bundle @param {string} panelId @returns {any | null}
 */
function pageOfPanel(bundle, panelId) {
  for (const page of Object.values(bundle.project.pages)) {
    const p = /** @type {any} */ (page);
    if (p.panelIds.includes(panelId)) return p;
  }
  return null;
}
