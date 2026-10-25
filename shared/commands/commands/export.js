/**
 * `comic.export.*` — producing SVG, PNG and PDF.
 *
 * Rendering is a **server capability**, because it needs bytes and, for print, a PDF writer.
 * The commands are declared `serverOnly` for that reason, and the browser gets its pixels
 * from the canvas it already has.
 *
 * @module shared/commands/commands/export
 */

import { fail, ok } from '../registry.js';
import { findArtboard } from '../../domain/document.js';

/** @param {any} ctx @returns {any} */
const rendererOf = (ctx) => {
  const r = /** @type {any} */ (ctx.services ?? {}).renderer;
  if (!r) throw new Error('no renderer service is configured');
  return r;
};

/** @param {any} ctx @returns {any} */
const storageOf = (ctx) => {
  const s = /** @type {any} */ (ctx.services ?? {}).storage;
  if (!s) throw new Error('no storage service is configured');
  return s;
};

/** @type {import('../registry.js').RegisteredCommand[]} */
export const exportCommands = [
  {
    id: 'comic.export.svg',
    label: 'Export SVG',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        path: { type: 'string', minLength: 1, description: 'Where to write it, relative to the storage root.' },
      },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const svg = rendererOf(ctx).pageToSvg(ctx.bundle, page);
      if (params.path) await storageOf(ctx).write(params.path, svg);
      return ok({ pageId: params.pageId, path: params.path ?? null, bytes: svg.length, format: 'svg' });
    },
  },

  {
    id: 'comic.export.png',
    label: 'Export PNG',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        path: { type: 'string', minLength: 1 },
        scale: { type: 'number', minimum: 0.05, maximum: 4, description: 'Pixel scale. 1 renders one pixel per page unit.' },
      },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const artboard = findArtboard(ctx.bundle.document, page.artboardId);
      if (!artboard) return fail('conflict', `page ${params.pageId} has no artboard`, 'pageId');
      if (artboard.size.width * artboard.size.height > 40_000_000) {
        return fail('bad-params', `page ${params.pageId} is too large to rasterise at this scale`, 'scale');
      }
      const png = rendererOf(ctx).pageToPng(ctx.bundle, page, params.scale ?? 0.5);
      if (params.path) await storageOf(ctx).write(params.path, png);
      return ok({
        pageId: params.pageId,
        path: params.path ?? null,
        bytes: png.length,
        format: 'png',
        width: Math.round(artboard.size.width * (params.scale ?? 0.5)),
        height: Math.round(artboard.size.height * (params.scale ?? 0.5)),
      });
    },
  },

  {
    id: 'comic.export.pdf',
    label: 'Export PDF',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        path: { type: 'string', minLength: 1 },
        pageIds: {
          type: 'array',
          items: { type: 'string', minLength: 1 },
          description: 'Pages to include, in order. Omit for every page in the book.',
        },
      },
      required: ['path'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const { bundle } = ctx;
      const all = orderedPages(bundle);
      const wanted = params.pageIds ?? all.map((/** @type {any} */ p) => p.id);
      for (const id of wanted) {
        if (!bundle.project.pages[id]) return fail('not-found', `no page ${id}`, 'pageIds');
      }
      const pages = wanted.map((/** @type {string} */ id) => bundle.project.pages[id]);
      const pdf = rendererOf(ctx).bookToPdf(bundle, pages);
      await storageOf(ctx).write(params.path, pdf);
      return ok({ path: params.path, pages: pages.length, bytes: pdf.length, format: 'pdf' });
    },
  },
];

/**
 * Every page in narrative order: chapters, then their scenes, then their pages. Reading a
 * book in filename order is how a page ends up printed in the wrong place.
 * @param {any} bundle
 * @returns {any[]}
 */
export function orderedPages(bundle) {
  const { project } = bundle;
  /** @type {any[]} */
  const out = [];
  const seen = new Set();
  const chapterIds = Object.keys(project.chapters);
  if (chapterIds.length === 0) return Object.values(project.pages);
  for (const chapterId of chapterIds) {
    const chapter = project.chapters[chapterId];
    for (const sceneId of chapter.sceneIds) {
      const scene = project.scenes[sceneId];
      if (!scene) continue;
      for (const pageId of scene.pageIds) {
        const page = project.pages[pageId];
        if (page && !seen.has(pageId)) { seen.add(pageId); out.push(page); }
      }
    }
  }
  // A page not reachable from a scene still exists and must still print: a bundle edited by
  // hand can produce one, and silently dropping it is worse than printing it at the end.
  for (const page of Object.values(project.pages)) {
    const p = /** @type {any} */ (page);
    if (!seen.has(p.id)) out.push(p);
  }
  return out;
}
