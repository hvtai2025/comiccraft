/**
 * `comic.panel.*` tools beyond the M1 set: move, resize, describe and label artwork roles.
 *
 * Geometry still lives in the document object and the panel keeps pointing at it. A move is
 * therefore a frame edit, never a second rectangle; a locked panel refuses both.
 *
 * @module shared/commands/commands/panel-tools
 */

import { fail, ok } from '../registry.js';
import { getNode, retargetArtwork, updateNode } from '../../domain/document.js';
import { quantize, rect } from '../../domain/geom.js';
import { findPageOfPanel, geometryWarnings } from './panel.js';

/** @typedef {'primary'|'background'|'overlay'} ArtworkRole */

/** @param {any} bundle @param {string} panelId @returns {any | null} */
function panelOf(bundle, panelId) {
  const panel = bundle.project.panels[panelId];
  if (!panel) return null;
  const node = getNode(bundle.document, panel.frameNodeId);
  if (!node) return null;
  return { panel, node };
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const panelToolCommands = [
  {
    id: 'comic.panel.move',
    label: 'Move Panel',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        dx: { type: 'number' },
        dy: { type: 'number' },
      },
      required: ['panelId', 'dx', 'dy'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const found = panelOf(bundle, params.panelId);
      if (!found) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (found.panel.locked) {
        return fail('conflict', `panel ${params.panelId} is locked; unlock it before moving it`, 'panelId');
      }
      const before = found.node.frame;
      const frame = rect(
        quantize(before.x + params.dx),
        quantize(before.y + params.dy),
        before.width,
        before.height,
      );
      updateNode(bundle.document, found.node.id, { frame });
      retargetArtwork(bundle.document, found.panel.artworkRefs, before, frame);
      const page = findPageOfPanel(bundle, params.panelId);
      return ok({ panelId: params.panelId, frame, warnings: page ? geometryWarnings(bundle, page) : [] });
    },
  },

  {
    id: 'comic.panel.resize',
    label: 'Resize Panel',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        width: { type: 'number', minimum: 1 },
        height: { type: 'number', minimum: 1 },
        anchor: { type: 'string', enum: ['top-left', 'center'], description: 'Which point stays put. Default top-left.' },
      },
      required: ['panelId', 'width', 'height'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const found = panelOf(bundle, params.panelId);
      if (!found) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (found.panel.locked) {
        return fail('conflict', `panel ${params.panelId} is locked; unlock it before resizing it`, 'panelId');
      }
      const before = found.node.frame;
      const cx = before.x + before.width / 2;
      const cy = before.y + before.height / 2;
      const frame = params.anchor === 'center'
        ? rect(quantize(cx - params.width / 2), quantize(cy - params.height / 2), quantize(params.width), quantize(params.height))
        : rect(before.x, before.y, quantize(params.width), quantize(params.height));
      updateNode(bundle.document, found.node.id, { frame });
      retargetArtwork(bundle.document, found.panel.artworkRefs, before, frame);
      const page = findPageOfPanel(bundle, params.panelId);
      return ok({ panelId: params.panelId, frame, warnings: page ? geometryWarnings(bundle, page) : [] });
    },
  },

  {
    id: 'comic.panel.setShot',
    label: 'Set Panel Shot',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        shotDescription: { type: 'string', maxLength: 2000 },
        camera: { type: 'string', maxLength: 200 },
        artworkPrompt: { type: 'string', maxLength: 2000 },
      },
      required: ['panelId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const panel = ctx.bundle.project.panels[params.panelId];
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (params.shotDescription !== undefined) panel.shotDescription = params.shotDescription;
      if (params.camera !== undefined) panel.camera = params.camera;
      if (params.artworkPrompt !== undefined) panel.artworkPrompt = params.artworkPrompt;
      return ok({
        panelId: params.panelId,
        shotDescription: panel.shotDescription ?? '',
        camera: panel.camera ?? '',
        artworkPrompt: panel.artworkPrompt ?? '',
      });
    },
  },

  {
    id: 'comic.panel.setArtworkRole',
    label: 'Set Artwork Role',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        nodeId: { type: 'integer', minimum: 1 },
        role: { type: 'string', enum: ['primary', 'background', 'overlay'] },
      },
      required: ['panelId', 'nodeId', 'role'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const panel = ctx.bundle.project.panels[params.panelId];
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (!panel.artworkRefs.map(Number).includes(params.nodeId)) {
        return fail('not-found', `panel ${params.panelId} has no artwork object ${params.nodeId}`, 'nodeId');
      }
      /** @type {Record<string, 'primary'|'background'|'overlay'>} */
      const roles = panel.artworkRoles ?? {};
      roles[String(params.nodeId)] = params.role;
      panel.artworkRoles = roles;
      return ok({ panelId: params.panelId, nodeId: params.nodeId, role: params.role, roles: { ...roles } });
    },
  },

  {
    id: 'comic.panel.list',
    label: 'List Panels',
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
      return ok({
        pageId: params.pageId,
        panels: page.panelIds.map((panelId, index) => {
          const panel = bundle.project.panels[panelId];
          const node = panel ? getNode(bundle.document, panel.frameNodeId) : null;
          return {
            panelId,
            index,
            frame: node?.frame ?? null,
            locked: panel?.locked ?? false,
            shotDescription: panel?.shotDescription ?? '',
            artworkRefs: panel ? [...panel.artworkRefs] : [],
            letteringIds: panel ? [...panel.letteringIds] : [],
          };
        }),
      });
    },
  },
];
