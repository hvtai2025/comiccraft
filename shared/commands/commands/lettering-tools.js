/**
 * `comic.lettering.*` tools that complete the M2 lettering surface.
 *
 * These are additive commands: the M1 commands keep their ids and behaviour. A style change
 * edits the lettering record and its text object together, so the model and the picture
 * cannot disagree; a tail change edits the tail object, so a balloon never loses its speaker.
 *
 * @module shared/commands/commands/lettering-tools
 */

import { fail, ok } from '../registry.js';
import { addNode, getNode, removeNode, updateNode } from '../../domain/document.js';
import { quantize, rect } from '../../domain/geom.js';
import { fitText, requiredFrame, stubMeasure } from '../../domain/lettering.js';
import { fontEnvironment, missingFont, catalogue } from '../../domain/fonts.js';

/** @param {any} ctx @returns {import('../../domain/lettering.js').MeasureText} */
const measureOf = (ctx) => (ctx.services?.measure ?? stubMeasure(0.5));

/** @param {any} ctx @returns {import('../../domain/fonts.js').FontEnvironment} */
const fontsOf = (ctx) => fontEnvironment(ctx.services?.fonts);

/** @param {any} bundle @param {string} letteringId @returns {any | null} */
const letteringOf = (bundle, letteringId) => bundle.project.lettering[letteringId] ?? null;

/**
 * The style a lettering record draws with. A record may override the project's defaults,
 * which is what makes per-balloon typography possible without a second style entity.
 * @param {any} bundle
 * @param {any} lettering
 * @param {any} [node]
 * @returns {{ family: string, sizePt: number, align: 'left'|'center'|'right', padding: number, bold: boolean, italic: boolean, color: string }}
 */
export function styleOf(bundle, lettering, node) {
  const defaults = bundle.project.defaultLetteringStyle;
  return {
    family: lettering.fontFamily ?? defaults.family,
    sizePt: lettering.fontSize ?? node?.fontSize ?? defaults.sizePt,
    align: lettering.align ?? node?.align ?? defaults.align ?? 'left',
    padding: lettering.padding ?? defaults.padding ?? 8,
    bold: lettering.bold ?? defaults.bold ?? false,
    italic: lettering.italic ?? defaults.italic ?? false,
    color: lettering.color ?? node?.color ?? '#111111',
  };
}

/**
 * @param {any} ctx
 * @param {any} lettering
 * @param {any} node
 * @returns {any}
 */
function fitOf(ctx, lettering, node) {
  const style = styleOf(ctx.bundle, lettering, node);
  return fitText({
    text: lettering.text ?? node.text ?? '',
    frame: node.frame,
    style,
    measure: measureOf(ctx),
    policy: lettering.overflowPolicy ?? 'warn',
    padding: style.padding,
  });
}

/** @param {any} bundle @param {string} letteringId @returns {any | null} */
function pageOfLettering(bundle, letteringId) {
  for (const panel of Object.values(bundle.project.panels)) {
    const p = /** @type {any} */ (panel);
    if (!p.letteringIds.includes(letteringId)) continue;
    for (const page of Object.values(bundle.project.pages)) {
      const pg = /** @type {any} */ (page);
      if (pg.panelIds.includes(p.id)) return pg;
    }
  }
  return null;
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const letteringToolCommands = [
  {
    id: 'comic.fonts.list',
    label: 'Available Fonts',
    params: { type: 'object', properties: {}, additionalProperties: false },
    run(ctx) {
      const environment = fontsOf(ctx);
      const available = new Set(environment.families.map((f) => f.toLowerCase()));
      return ok({
        catalogue: catalogue().map((face) => ({ ...face, available: available.has(face.name.toLowerCase()) })),
        availableFamilies: [...environment.families],
      });
    },
  },

  {
    id: 'comic.lettering.setStyle',
    label: 'Set Lettering Style',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        family: { type: 'string', minLength: 1, maxLength: 100 },
        sizePt: { type: 'number', minimum: 4, maximum: 96 },
        align: { type: 'string', enum: ['left', 'center', 'right'] },
        padding: { type: 'number', minimum: 0, maximum: 80 },
        bold: { type: 'boolean' },
        italic: { type: 'boolean' },
        color: { type: 'string', minLength: 1, maxLength: 20 },
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

      if (params.family !== undefined) lettering.fontFamily = params.family;
      if (params.sizePt !== undefined) lettering.fontSize = params.sizePt;
      if (params.align !== undefined) lettering.align = params.align;
      if (params.padding !== undefined) lettering.padding = params.padding;
      if (params.bold !== undefined) lettering.bold = params.bold;
      if (params.italic !== undefined) lettering.italic = params.italic;
      if (params.color !== undefined) lettering.color = params.color;

      const style = styleOf(bundle, lettering, node);
      updateNode(bundle.document, node.id, {
        fontSize: style.sizePt,
        align: style.align,
        color: style.color,
      });

      const fit = fitOf(ctx, lettering, getNode(bundle.document, node.id) ?? node);
      let frame = node.frame;
      if (lettering.overflowPolicy === 'resize-frame' && fit.overflowed) {
        const needed = requiredFrame(fit, style.padding);
        frame = rect(frame.x, frame.y, Math.max(frame.width, needed.width), Math.max(frame.height, needed.height));
        updateNode(bundle.document, node.id, { frame });
      }
      if (lettering.overflowPolicy === 'shrink' && fit.shrunk) {
        // The preview already draws the shrunk size; the model must say the same thing, or a
        // reopen would draw at the larger size until the next fit.
        updateNode(bundle.document, node.id, { fontSize: fit.fontSize });
      }
      const missing = missingFont({ family: style.family }, fontsOf(ctx));
      return ok({
        letteringId: params.letteringId,
        style,
        frame,
        missingFont: missing,
        fit: { lines: fit.lines, fontSize: fit.fontSize, overflowed: fit.overflowed, shrunk: fit.shrunk },
      });
    },
  },

  {
    id: 'comic.lettering.setFrame',
    label: 'Set Lettering Frame',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        x: { type: 'number' },
        y: { type: 'number' },
        width: { type: 'number', minimum: 1 },
        height: { type: 'number', minimum: 1 },
      },
      required: ['letteringId', 'x', 'y', 'width', 'height'],
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
      const before = node.frame;
      const frame = rect(quantize(params.x), quantize(params.y), quantize(params.width), quantize(params.height));
      updateNode(bundle.document, node.id, { frame });
      // Moving a balloon frame must take its tail with it. The tail keeps its shape and
      // points at the same place relative to the balloon; a body that moves alone leaves a
      // detached triangle behind.
      const dx = frame.x - before.x;
      const dy = frame.y - before.y;
      if (lettering.tailNodeId !== undefined && lettering.tailNodeId !== null) {
        const tailNode = getNode(bundle.document, lettering.tailNodeId);
        if (tailNode) {
          updateNode(bundle.document, tailNode.id, {
            frame: rect(quantize(tailNode.frame.x + dx), quantize(tailNode.frame.y + dy), tailNode.frame.width, tailNode.frame.height),
          });
        }
        if (lettering.tailTip) {
          lettering.tailTip = { x: quantize(lettering.tailTip.x + dx), y: quantize(lettering.tailTip.y + dy) };
        }
      }
      const fit = fitOf(ctx, lettering, getNode(bundle.document, node.id) ?? node);
      return ok({
        letteringId: params.letteringId,
        frame,
        fit: { lines: fit.lines, fontSize: fit.fontSize, overflowed: fit.overflowed, shrunk: fit.shrunk },
      });
    },
  },

  {
    id: 'comic.lettering.edit',
    label: 'Edit Lettering',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        text: { type: 'string', maxLength: 4000 },
        family: { type: 'string', minLength: 1, maxLength: 100 },
        sizePt: { type: 'number', minimum: 4, maximum: 96 },
        align: { type: 'string', enum: ['left', 'center', 'right'] },
        padding: { type: 'number', minimum: 0, maximum: 80 },
        bold: { type: 'boolean' },
        italic: { type: 'boolean' },
        color: { type: 'string', minLength: 1, maxLength: 20 },
        variant: { type: 'string', enum: ['speech', 'thought', 'shout', 'whisper'] },
        policy: { type: 'string', enum: ['warn', 'shrink', 'resize-frame'] },
      },
      required: ['letteringId'],
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

      if (params.text !== undefined) {
        lettering.text = params.text;
        updateNode(bundle.document, node.id, { text: params.text });
        for (const dialogue of Object.values(bundle.project.dialogue)) {
          const d = /** @type {any} */ (dialogue);
          if (d.letteringId === params.letteringId) d.text = params.text;
        }
      }
      if (params.family !== undefined) lettering.fontFamily = params.family;
      if (params.sizePt !== undefined) lettering.fontSize = params.sizePt;
      if (params.align !== undefined) lettering.align = params.align;
      if (params.padding !== undefined) lettering.padding = params.padding;
      if (params.bold !== undefined) lettering.bold = params.bold;
      if (params.italic !== undefined) lettering.italic = params.italic;
      if (params.color !== undefined) lettering.color = params.color;
      if (params.policy !== undefined) lettering.overflowPolicy = params.policy;
      if (params.variant !== undefined) {
        if (lettering.kind !== 'balloon') {
          return fail('conflict', `${params.letteringId} is a ${lettering.kind}, so it has no balloon variant`, 'variant');
        }
        lettering.variant = params.variant;
      }

      const style = styleOf(bundle, lettering, node);
      updateNode(bundle.document, node.id, { fontSize: style.sizePt, align: style.align, color: style.color });
      const fit = fitOf(ctx, lettering, getNode(bundle.document, node.id) ?? node);
      let frame = node.frame;
      if (lettering.overflowPolicy === 'resize-frame' && fit.overflowed) {
        const needed = requiredFrame(fit, style.padding);
        frame = rect(frame.x, frame.y, Math.max(frame.width, needed.width), Math.max(frame.height, needed.height));
        updateNode(bundle.document, node.id, { frame });
      }
      if (lettering.overflowPolicy === 'shrink' && fit.shrunk) {
        updateNode(bundle.document, node.id, { fontSize: fit.fontSize });
      }
      return ok({
        letteringId: params.letteringId,
        text: lettering.text,
        style,
        variant: lettering.variant ?? null,
        policy: lettering.overflowPolicy,
        frame,
        fit: { lines: fit.lines, fontSize: fit.fontSize, overflowed: fit.overflowed, shrunk: fit.shrunk },
      });
    },
  },

  {
    id: 'comic.lettering.setVariant',
    label: 'Set Balloon Variant',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        variant: { type: 'string', enum: ['speech', 'thought', 'shout', 'whisper'] },
      },
      required: ['letteringId', 'variant'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering'] },
    run(ctx, params) {
      const lettering = letteringOf(ctx.bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      if (lettering.kind !== 'balloon') {
        return fail('conflict', `${params.letteringId} is a ${lettering.kind}, so it has no balloon variant`, 'variant');
      }
      lettering.variant = params.variant;
      return ok({ letteringId: params.letteringId, variant: params.variant });
    },
  },

  {
    id: 'comic.lettering.setOverflowPolicy',
    label: 'Set Overflow Policy',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        policy: { type: 'string', enum: ['warn', 'shrink', 'resize-frame'] },
      },
      required: ['letteringId', 'policy'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering'] },
    run(ctx, params) {
      const lettering = letteringOf(ctx.bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      lettering.overflowPolicy = params.policy;
      const node = getNode(ctx.bundle.document, lettering.nodeId);
      const fit = node ? fitOf(ctx, lettering, node) : null;
      return ok({
        letteringId: params.letteringId,
        policy: params.policy,
        overflowed: fit?.overflowed ?? true,
      });
    },
  },

  {
    id: 'comic.lettering.attachTail',
    label: 'Attach Balloon Tail',
    params: {
      type: 'object',
      properties: {
        letteringId: { type: 'string', minLength: 1 },
        tipX: { type: 'number', description: 'The speaker the tail points at, in page units.' },
        tipY: { type: 'number' },
      },
      required: ['letteringId', 'tipX', 'tipY'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const lettering = letteringOf(bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      if (lettering.kind !== 'balloon') {
        return fail('conflict', `${params.letteringId} is a ${lettering.kind}, so it cannot have a tail`, 'letteringId');
      }
      const node = getNode(bundle.document, lettering.nodeId);
      if (!node) return fail('conflict', `lettering ${params.letteringId} has no object`, 'letteringId');
      if (lettering.tailNodeId !== undefined && lettering.tailNodeId !== null && getNode(bundle.document, lettering.tailNodeId)) {
        updateNode(bundle.document, lettering.tailNodeId, {
          frame: rect(quantize(params.tipX - 6), quantize(params.tipY - 6), 12, 12),
        });
        lettering.tailTip = { x: quantize(params.tipX), y: quantize(params.tipY) };
        lettering.tailAttached = true;
        return ok({ letteringId: params.letteringId, tailNodeId: lettering.tailNodeId, tip: lettering.tailTip, attached: true });
      }
      const page = pageOfLettering(bundle, params.letteringId);
      if (!page) return fail('conflict', `lettering ${params.letteringId} is not on any page`, 'letteringId');
      const tailNodeId = addNode(bundle.document, page.artboardId, {
        kind: 'frame',
        name: 'Balloon Tail',
        frame: rect(quantize(params.tipX - 6), quantize(params.tipY - 6), 12, 12),
        fill: '#ffffff',
        stroke: '#111111',
        strokeWidth: 2,
      });
      lettering.tailNodeId = tailNodeId;
      lettering.tailTip = { x: quantize(params.tipX), y: quantize(params.tipY) };
      lettering.tailAttached = true;
      return ok({ letteringId: params.letteringId, tailNodeId, tip: lettering.tailTip, attached: true });
    },
  },

  {
    id: 'comic.lettering.detachTail',
    label: 'Detach Balloon Tail',
    params: {
      type: 'object',
      properties: { letteringId: { type: 'string', minLength: 1 } },
      required: ['letteringId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['lettering'] },
    run(ctx, params) {
      const lettering = letteringOf(ctx.bundle, params.letteringId);
      if (!lettering) return fail('not-found', `no lettering ${params.letteringId}`, 'letteringId');
      if (lettering.tailNodeId !== undefined && lettering.tailNodeId !== null) {
        removeNode(ctx.bundle.document, lettering.tailNodeId);
      }
      delete lettering.tailNodeId;
      delete lettering.tailTip;
      lettering.tailAttached = false;
      return ok({ letteringId: params.letteringId, attached: false });
    },
  },
];
