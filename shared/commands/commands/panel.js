/**
 * `comic.panel.*` — creating, reordering, resizing and locking panels.
 *
 * A panel **never carries its own geometry**: `frameNodeId` points at the document object
 * that owns it. A second copy of a rectangle is the failure this architecture exists to
 * prevent (design §6.1), so a resize edits the object and the panel just keeps looking at it.
 *
 * @module shared/commands/commands/panel
 */

import { fail, ok } from '../registry.js';
import { allocate, unlinkAssetNodes } from '../../domain/project.js';
import { addNode, findArtboard, getNode, removeNode, retargetArtwork, updateNode } from '../../domain/document.js';
import { quantize, rect } from '../../domain/geom.js';
import { degenerateFrames, findOverlaps, framesOutsidePage } from '../../domain/layout.js';

/** @param {any} bundle @param {string} panelId @returns {any | null} */
function panelOf(bundle, panelId) {
  const panel = bundle.project.panels[panelId];
  if (!panel) return null;
  const node = getNode(bundle.document, panel.frameNodeId);
  if (!node) return null;
  return { panel, node };
}

/**
 * Warnings a panel geometry change produces. Warnings, not errors: an overlapping panel and
 * a bleed panel are both deliberate choices sometimes, and refusing them would make the
 * tool argue with the user.
 * @param {import('../../domain/project.js').Bundle} bundle
 * @param {import('../../domain/project.js').Page} page
 * @returns {{ code: string, message: string, panelIds?: string[] }[]}
 */
export function geometryWarnings(bundle, page) {
  /** @type {{ code: string, message: string, panelIds?: string[] }[]} */
  const warnings = [];
  const artboard = findArtboard(bundle.document, page.artboardId);
  if (!artboard) return warnings;

  /** @type {{ pid: string, node: import('../../domain/document.js').DocNode }[]} */
  const entries = [];
  for (const pid of page.panelIds) {
    const panel = bundle.project.panels[pid];
    if (!panel) continue;
    const node = getNode(bundle.document, panel.frameNodeId);
    if (!node) continue;
    entries.push({ pid, node });
  }

  const frames = entries.map((e) => e.node.frame);
  for (const i of degenerateFrames(frames)) {
    warnings.push({
      code: 'panel-degenerate',
      message: `panel ${entries[i].pid} has a zero or negative size and cannot be drawn`,
      panelIds: [entries[i].pid],
    });
  }
  for (const o of findOverlaps(frames.filter((f) => f.width > 0 && f.height > 0))) {
    const a = entries[o.a];
    const b = entries[o.b];
    if (!a || !b) continue;
    warnings.push({
      code: 'panel-overlap',
      message: `panels ${a.pid} and ${b.pid} overlap over ${Math.round(o.overlapArea)} units, which will hide artwork`,
      panelIds: [a.pid, b.pid],
    });
  }
  for (const i of framesOutsidePage(frames, artboard.size)) {
    warnings.push({
      code: 'panel-outside-page',
      message: `panel ${entries[i].pid} reaches outside the page and will be cropped when printed`,
      panelIds: [entries[i].pid],
    });
  }
  return warnings;
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const panelCommands = [
  {
    id: 'comic.panel.add',
    label: 'Add Panel',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        x: { type: 'number', description: 'Left edge in page units.' },
        y: { type: 'number', description: 'Top edge in page units.' },
        width: { type: 'number', minimum: 1, description: 'Panel width, in page units. Must be positive.' },
        height: { type: 'number', minimum: 1, description: 'Panel height, in page units. Must be positive.' },
        after: {
          type: 'string',
          description: 'Panel id to insert after in reading order. Omit to append.',
        },
      },
      required: ['pageId', 'x', 'y', 'width', 'height'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages', 'panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const page = bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const artboard = findArtboard(bundle.document, page.artboardId);
      if (!artboard) return fail('conflict', `page ${params.pageId} has no artboard`, 'pageId');

      const nodeId = addNode(bundle.document, page.artboardId, {
        kind: 'frame',
        name: 'Panel',
        frame: rect(quantize(params.x), quantize(params.y), quantize(params.width), quantize(params.height)),
        fill: '#ffffff',
        stroke: '#111111',
        strokeWidth: 3,
      });
      const panelId = allocate(bundle, 'panel');
      bundle.project.panels[panelId] = {
        id: panelId,
        frameNodeId: nodeId,
        dialogueIds: [],
        letteringIds: [],
        artworkRefs: [],
        continuityIds: [],
        locked: false,
      };

      if (params.after !== undefined) {
        const i = page.panelIds.indexOf(params.after);
        if (i === -1) {
          return fail('not-found', `panel ${params.after} is not on page ${params.pageId}`, 'after');
        }
        page.panelIds.splice(i + 1, 0, panelId);
      } else {
        page.panelIds.push(panelId);
      }

      const created = getNode(bundle.document, nodeId);
      return ok({
        panelId,
        frameNodeId: nodeId,
        frame: created ? created.frame : null,
        warnings: geometryWarnings(bundle, page),
      });
    },
  },

  {
    id: 'comic.panel.setFrame',
    label: 'Set Panel Frame',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        x: { type: 'number' },
        y: { type: 'number' },
        width: { type: 'number', minimum: 1 },
        height: { type: 'number', minimum: 1 },
      },
      required: ['panelId', 'x', 'y', 'width', 'height'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const found = panelOf(bundle, params.panelId);
      if (!found) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (found.panel.locked) {
        return fail('conflict', `panel ${params.panelId} is locked; unlock it before moving or resizing it`, 'panelId');
      }
      const frame = rect(quantize(params.x), quantize(params.y), quantize(params.width), quantize(params.height));
      const before = { ...found.node.frame };
      updateNode(bundle.document, found.panel.frameNodeId, { frame });
      retargetArtwork(bundle.document, found.panel.artworkRefs, before, frame);
      const page = findPageOfPanel(bundle, params.panelId);
      return ok({
        panelId: params.panelId,
        frame,
        warnings: page ? geometryWarnings(bundle, page) : [],
      });
    },
  },

  {
    id: 'comic.panel.setLocked',
    label: 'Lock Panel',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        locked: { type: 'boolean', description: 'A locked panel refuses frame and artwork changes.' },
      },
      required: ['panelId', 'locked'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const panel = ctx.bundle.project.panels[params.panelId];
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      panel.locked = params.locked;
      return ok({ panelId: params.panelId, locked: panel.locked });
    },
  },

  {
    id: 'comic.panel.reorder',
    label: 'Reorder Panels',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        order: {
          type: 'array',
          minItems: 1,
          items: { type: 'string', minLength: 1 },
          description: 'Every panel id on the page, in the intended reading order.',
        },
      },
      required: ['pageId', 'order'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const page = bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');

      const current = [...page.panelIds].sort();
      const proposed = [...params.order].sort();
      if (current.length !== proposed.length || current.some((id, i) => id !== proposed[i])) {
        return fail(
          'bad-params',
          `the order must list exactly the ${current.length} panel(s) on page ${params.pageId}, ` +
          `each once. Got ${params.order.length} id(s): ${[...new Set(params.order)].length} distinct.`,
          'order',
        );
      }
      // Geometry is untouched on purpose: reading order is explicit state, and a reorder
      // that moved frames would be a layout change wearing a reorder's name.
      page.panelIds = [...params.order];
      return ok({ pageId: params.pageId, order: [...page.panelIds] });
    },
  },

  {
    id: 'comic.panel.readingOrder',
    label: 'Reading Order',
    params: {
      type: 'object',
      properties: { pageId: { type: 'string', minLength: 1 } },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      return ok({
        pageId: page.id,
        panels: page.panelIds.map((pid) => {
          const panel = ctx.bundle.project.panels[pid];
          const node = panel ? getNode(ctx.bundle.document, panel.frameNodeId) : null;
          return { panelId: pid, frame: node?.frame ?? null, locked: panel?.locked ?? false };
        }),
      });
    },
  },

  {
    id: 'comic.panel.delete',
    label: 'Delete Panel',
    params: {
      type: 'object',
      properties: { panelId: { type: 'string', minLength: 1 } },
      required: ['panelId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages', 'panels', 'dialogue', 'lettering'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const found = panelOf(bundle, params.panelId);
      if (!found) return fail('not-found', `no panel ${params.panelId}`, 'panelId');

      // A deleted panel takes its own objects with it: the objects are reachable from
      // nothing else, so leaving them would leave orphans the document still draws.
      for (const did of [...found.panel.dialogueIds]) delete bundle.project.dialogue[did];
      for (const lid of [...found.panel.letteringIds]) {
        const l = bundle.project.lettering[lid];
        if (l) {
          removeNode(bundle.document, l.nodeId);
          if (l.tailNodeId) removeNode(bundle.document, l.tailNodeId);
        }
        delete bundle.project.lettering[lid];
      }
      for (const ref of [...found.panel.artworkRefs]) removeNode(bundle.document, Number(ref));
      unlinkAssetNodes(bundle.project, found.panel.artworkRefs);
      removeNode(bundle.document, found.panel.frameNodeId);

      const page = findPageOfPanel(bundle, params.panelId);
      if (page) {
        const i = page.panelIds.indexOf(params.panelId);
        if (i !== -1) page.panelIds.splice(i, 1);
      }
      delete bundle.project.panels[params.panelId];
      return ok({ panelId: params.panelId, warnings: page ? geometryWarnings(bundle, page) : [] });
    },
  },
];

/**
 * The page a panel belongs to. A panel is reachable from exactly one page, so this is a
 * search rather than a stored back-reference — a stored one could disagree with the list.
 * @param {any} bundle @param {string} panelId @returns {any | null}
 */
export function findPageOfPanel(bundle, panelId) {
  for (const page of Object.values(bundle.project.pages)) {
    const p = /** @type {any} */ (page);
    if (p.panelIds.includes(panelId)) return p;
  }
  return null;
}
