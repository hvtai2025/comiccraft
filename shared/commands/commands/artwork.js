/**
 * `comic.artwork.*` — placing, cropping and clipping artwork into a panel.
 *
 * An asset is content-addressed: the hash *is* the filename, so an interrupted save writes a
 * new file rather than overwriting an approved one (design §7.4). Replacing a panel's artwork
 * therefore never destroys the previous asset — it just stops referencing it.
 *
 * @module shared/commands/commands/artwork
 */

import { fail, ok } from '../registry.js';
import { allocate, unlinkAssetNodes } from '../../domain/project.js';
import { addNode, getNode, removeNode, updateNode } from '../../domain/document.js';
import { quantize, rect, right, bottom, isPositive, intersection } from '../../domain/geom.js';
import { ART_STYLES, makeDrawing, drawingFingerprint } from '../../domain/art.js';

/** @param {any} bundle @param {string} panelId @returns {any | null} */
const panelOf = (bundle, panelId) => bundle.project.panels[panelId] ?? null;

/** @type {import('../registry.js').RegisteredCommand[]} */
export const artworkCommands = [
  {
    id: 'comic.artwork.place',
    label: 'Place Artwork',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        name: { type: 'string', minLength: 1, maxLength: 200, description: 'Human-readable asset name.' },
        contentHash: { type: 'string', minLength: 1, description: 'Hash of the asset bytes; the asset is stored under it.' },
        data: { type: 'string', description: 'Asset payload as text, for a generated or uploaded file.' },
        fit: { type: 'string', enum: ['cover', 'contain'], description: 'How the artwork is fitted to the frame. Default cover.' },
        clip: { type: 'boolean', description: 'Clip the artwork to the panel frame. Default true.' },
        provider: { type: 'string', maxLength: 100, description: 'Who produced it, for provenance.' },
        model: { type: 'string', maxLength: 100 },
        request: { type: 'string', maxLength: 2000 },
      },
      required: ['panelId', 'name', 'contentHash'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels', 'assets'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const panel = panelOf(bundle, params.panelId);
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (panel.locked) {
        return fail('conflict', `panel ${params.panelId} is locked; unlock it before replacing its artwork`, 'panelId');
      }
      const frameNode = getNode(bundle.document, panel.frameNodeId);
      if (!frameNode) return fail('conflict', `panel ${params.panelId} has no frame object`, 'panelId');

      const page = pageOf(bundle, params.panelId);
      if (!page) return fail('conflict', `panel ${params.panelId} is not on any page`, 'panelId');

      let asset = /** @type {any} */ (Object.values(bundle.project.assets).find((a) => /** @type {any} */ (a).contentHash === params.contentHash));
      if (!asset) {
        const assetId = allocate(bundle, 'asset');
        asset = {
          id: assetId,
          name: params.name,
          contentHash: params.contentHash,
          ...(params.data !== undefined ? { data: params.data } : {}),
          assetType: 'source',
          approvalStatus: 'approved',
          provenance: {
            provider: params.provider ?? 'local',
            model: params.model ?? null,
            request: params.request ?? null,
            placedAt: ctx.env.now ? ctx.env.now() : null,
          },
          nodeIds: [],
        };
        bundle.project.assets[assetId] = asset;
      }
      const a = /** @type {any} */ (asset);

      // The image object covers the frame. `fit` says how the bitmap is fitted inside it,
      // which is a render-time decision and so is not baked into the geometry here.
      const nodeId = addNode(bundle.document, page.artboardId, {
        kind: 'image',
        name: params.name,
        frame: { ...frameNode.frame },
        assetId: a.id,
        fit: params.fit ?? 'cover',
        clipToFrame: params.clip !== false,
      });

      // Superseded artwork is unlinked but NOT deleted: a replaced panel must not destroy
      // the previous asset, because the user may want it back (plan §16).
      for (const ref of [...panel.artworkRefs]) {
        removeNode(bundle.document, Number(ref));
      }
      unlinkAssetNodes(bundle.project, panel.artworkRefs);
      panel.artworkRefs = [nodeId];
      if (!a.nodeIds.includes(nodeId)) a.nodeIds.push(nodeId);

      return ok({ panelId: params.panelId, artworkNodeId: nodeId, assetId: a.id, contentHash: a.contentHash });
    },
  },

  {
    id: 'comic.artwork.paint',
    label: 'Paint Local Drawing',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        prompt: { type: 'string', maxLength: 2000, description: 'What the panel shows. Defaults to the panel artwork prompt or shot description.' },
        style: { type: 'string', enum: [...ART_STYLES] },
        name: { type: 'string', minLength: 1, maxLength: 200 },
      },
      required: ['panelId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels', 'assets'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const panel = panelOf(bundle, params.panelId);
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (panel.locked) {
        return fail('conflict', `panel ${params.panelId} is locked; unlock it before painting artwork`, 'panelId');
      }
      const page = pageOf(bundle, params.panelId);
      if (!page) return fail('conflict', `panel ${params.panelId} is not on any page`, 'panelId');
      const frameNode = getNode(bundle.document, panel.frameNodeId);
      if (!frameNode) return fail('conflict', `panel ${params.panelId} has no frame`, 'panelId');

      const prompt = params.prompt
        ?? panel.artworkPrompt
        ?? panel.shotDescription
        ?? panel.camera
        ?? `Panel ${params.panelId}`;
      const drawing = makeDrawing({
        prompt, style: params.style ?? 'black ink',
        width: frameNode.frame.width, height: frameNode.frame.height,
        seed: params.panelId,
      });
      const contentHash = `drawing-${drawingFingerprint(drawing.data)}`;
      let asset = /** @type {any} */ (Object.values(bundle.project.assets)
        .find((a) => /** @type {any} */ (a).contentHash === contentHash));
      if (!asset) {
        const assetId = allocate(bundle, 'asset');
        asset = {
          id: assetId,
          name: params.name ?? `Drawing for ${params.panelId}`,
          contentHash,
          data: drawing.data,
          assetType: 'generated',
          mime: drawing.mime,
          mediaMetadata: { mime: drawing.mime, width: drawing.width, height: drawing.height },
          provenance: {
            provider: drawing.provider,
            model: drawing.model,
            request: { prompt: drawing.prompt, style: drawing.style },
            inputAssetIds: [],
            createdAt: ctx.env.now ? ctx.env.now() : 0,
          },
          parentAssetIds: [],
          inputAssetIds: [],
          approvalStatus: 'approved',
          nodeIds: [],
        };
        bundle.project.assets[assetId] = asset;
      }

      const nodeId = addNode(bundle.document, page.artboardId, {
        kind: 'image',
        name: asset.name,
        frame: { ...frameNode.frame },
        assetId: asset.id,
        fit: 'cover',
        clipToFrame: true,
      });
      for (const ref of [...panel.artworkRefs]) removeNode(bundle.document, Number(ref));
      unlinkAssetNodes(bundle.project, panel.artworkRefs);
      panel.artworkRefs = [nodeId];
      if (!asset.nodeIds.includes(nodeId)) asset.nodeIds.push(nodeId);
      return ok({
        panelId: params.panelId,
        artworkNodeId: nodeId,
        assetId: asset.id,
        contentHash,
        style: drawing.style,
        provider: drawing.provider,
        model: drawing.model,
      });
    },
  },

  {
    id: 'comic.artwork.crop',
    label: 'Crop Artwork',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        x: { type: 'number', minimum: 0, maximum: 1, description: 'Left edge as a fraction of the image.' },
        y: { type: 'number', minimum: 0, maximum: 1 },
        width: { type: 'number', minimum: 0.001, maximum: 1, description: 'Width as a fraction of the image.' },
        height: { type: 'number', minimum: 0.001, maximum: 1 },
      },
      required: ['panelId', 'x', 'y', 'width', 'height'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const panel = panelOf(bundle, params.panelId);
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (panel.locked) return fail('conflict', `panel ${params.panelId} is locked`, 'panelId');
      if (panel.artworkRefs.length === 0) {
        return fail('conflict', `panel ${params.panelId} has no artwork to crop`, 'panelId');
      }
      if (params.x + params.width > 1.000001 || params.y + params.height > 1.000001) {
        return fail(
          'bad-params',
          `the crop rect runs past the image: x+width is ${quantize(params.x + params.width)}, y+height is ${quantize(params.y + params.height)}; both must be at most 1`,
          'width',
        );
      }
      const node = getNode(bundle.document, Number(panel.artworkRefs[0]));
      if (!node) return fail('conflict', `panel ${params.panelId} references a missing artwork object`, 'panelId');
      updateNode(bundle.document, node.id, {
        crop: {
          x: quantize(params.x), y: quantize(params.y),
          width: quantize(params.width), height: quantize(params.height),
        },
      });
      return ok({ panelId: params.panelId, artworkNodeId: node.id, crop: /** @type {any} */ (getNode(bundle.document, node.id)).crop });
    },
  },

  {
    id: 'comic.artwork.clip',
    label: 'Clip Artwork to Panel',
    params: {
      type: 'object',
      properties: {
        panelId: { type: 'string', minLength: 1 },
        inset: { type: 'number', minimum: 0, description: 'Shrink the clip rect inward by this many units, for a gutter-safe border.' },
      },
      required: ['panelId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const panel = panelOf(bundle, params.panelId);
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      if (panel.locked) return fail('conflict', `panel ${params.panelId} is locked`, 'panelId');
      if (panel.artworkRefs.length === 0) {
        return fail('conflict', `panel ${params.panelId} has no artwork to clip`, 'panelId');
      }
      const frame = getNode(bundle.document, panel.frameNodeId);
      if (!frame) return fail('conflict', `panel ${params.panelId} has no frame`, 'panelId');

      const inset = params.inset ?? 0;
      const clip = rect(
        quantize(frame.frame.x + inset),
        quantize(frame.frame.y + inset),
        quantize(frame.frame.width - inset * 2),
        quantize(frame.frame.height - inset * 2),
      );
      if (!isPositive(clip)) {
        return fail('bad-params', `an inset of ${inset} leaves a ${clip.width}x${clip.height} clip rect`, 'inset');
      }
      for (const ref of panel.artworkRefs) {
        updateNode(bundle.document, Number(ref), { clip: { ...clip }, clipToFrame: true });
      }
      return ok({ panelId: params.panelId, clip });
    },
  },

  {
    id: 'comic.artwork.cover',
    label: 'Artwork Coverage',
    params: {
      type: 'object',
      properties: { panelId: { type: 'string', minLength: 1 } },
      required: ['panelId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const { bundle } = ctx;
      const panel = panelOf(bundle, params.panelId);
      if (!panel) return fail('not-found', `no panel ${params.panelId}`, 'panelId');
      const frame = getNode(bundle.document, panel.frameNodeId);
      if (!frame) return fail('conflict', `panel ${params.panelId} has no frame`, 'panelId');

      /** @type {{ nodeId: number, covers: boolean, gapUnits: number }[]} */
      const coverage = [];
      for (const ref of panel.artworkRefs) {
        const node = getNode(bundle.document, Number(ref));
        if (!node) continue;
        const shared = intersection(frame.frame, node.frame);
        const gap = shared ? 0 : gapBetween(frame.frame, node.frame);
        coverage.push({ nodeId: node.id, covers: shared !== null, gapUnits: quantize(gap) });
      }
      const uncovered = coverage.length === 0 || coverage.some((c) => !c.covers);
      return ok({ panelId: params.panelId, frame: frame.frame, coverage, uncovered });
    },
  },
];

/**
 * @param {any} bundle @param {string} panelId @returns {any | null}
 */
function pageOf(bundle, panelId) {
  for (const page of Object.values(bundle.project.pages)) {
    const p = /** @type {any} */ (page);
    if (p.panelIds.includes(panelId)) return p;
  }
  return null;
}

/**
 * The shortest distance from `a` to `b`. Used to say *how far* a misplaced artwork is from
 * its panel, because "does not cover" without a number is not actionable.
 * @param {import('../../domain/geom.js').Rect} a
 * @param {import('../../domain/geom.js').Rect} b
 * @returns {number}
 */
function gapBetween(a, b) {
  const dx = Math.max(a.x - right(b), b.x - right(a), 0);
  const dy = Math.max(a.y - bottom(b), b.y - bottom(a), 0);
  return Math.hypot(dx, dy);
}

