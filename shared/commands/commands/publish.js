/**
 * `comic.book.*`, `comic.page.export` and `comic.project.package` — publishing.
 *
 * Every publishing command runs the **gate first**: a missing asset, an unsupported print
 * size or an unavailable face is reported before a file is written, and the command returns
 * the problems rather than a silently wrong page.
 *
 * @module shared/commands/commands/publish
 */

import { fail, ok } from '../registry.js';
import { findArtboard } from '../../domain/document.js';
import {
  PRESETS, presetOf, validateForExport, publishErrors,
} from '../../domain/publish.js';
import { orderedPages } from './export.js';

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

/** @param {any} ctx @returns {import('../../domain/fonts.js').FontEnvironment} */
const fontsOf = (ctx) => /** @type {any} */ (ctx.services?.fonts ?? { families: [] });

/** @param {any} bundle @param {string[]|undefined} pageIds @returns {any[]} */
function pagesFor(bundle, pageIds) {
  const ordered = orderedPages(bundle);
  if (!pageIds || pageIds.length === 0) return ordered;
  return pageIds.map((id) => bundle.project.pages[id]).filter(Boolean);
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const publishCommands = [
  {
    id: 'comic.book.validate',
    label: 'Validate for Publication',
    params: {
      type: 'object',
      properties: {
        preset: { type: 'string', enum: Object.keys(PRESETS) },
        pageIds: { type: 'array', items: { type: 'string', minLength: 1 } },
        allowMissingAssets: { type: 'boolean' },
        allowMissingFonts: { type: 'boolean' },
        checkOverflow: { type: 'boolean', description: 'Report lettering overflow. Default true.' },
      },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const problems = validateForExport(ctx.bundle, {
        preset: params.preset ?? 'print',
        pageIds: params.pageIds,
        fonts: fontsOf(ctx),
        allowMissingAssets: params.allowMissingAssets === true,
        allowMissingFonts: params.allowMissingFonts === true,
        checkOverflow: params.checkOverflow !== false,
      });
      const errors = publishErrors(problems);
      return ok({
        preset: params.preset ?? 'print',
        problems,
        errors: errors.length,
        warnings: problems.length - errors.length,
        ready: errors.length === 0,
      });
    },
  },

  {
    id: 'comic.book.export',
    label: 'Export Book',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        preset: { type: 'string', enum: Object.keys(PRESETS) },
        path: { type: 'string', minLength: 1, description: 'File for a PDF, or a directory for images.' },
        format: { type: 'string', enum: ['pdf', 'images', 'both'] },
        pageIds: { type: 'array', items: { type: 'string', minLength: 1 } },
        scale: { type: 'number', minimum: 0.05, maximum: 4, description: 'Preview scale for images. Defaults to the preset.' },
        contactSheet: { type: 'boolean', description: 'Write a contact sheet beside the images. Default true for images and both.' },
        allowMissingAssets: { type: 'boolean' },
        allowMissingFonts: { type: 'boolean' },
      },
      required: ['path'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const { bundle } = ctx;
      const preset = presetOf(params.preset ?? 'screen');
      const pages = pagesFor(bundle, params.pageIds);
      if (pages.length === 0) return fail('not-found', 'there are no pages to export', 'pageIds');

      const problems = validateForExport(bundle, {
        preset: preset.id,
        pageIds: pages.map((p) => p.id),
        fonts: fontsOf(ctx),
        allowMissingAssets: params.allowMissingAssets === true,
        allowMissingFonts: params.allowMissingFonts === true,
        checkOverflow: true,
      });
      const errors = publishErrors(problems);
      if (errors.length > 0) {
        return fail('conflict', `the export is blocked: ${errors[0].message}`);
      }

      const format = params.format ?? 'pdf';
      const storage = storageOf(ctx);
      const renderer = rendererOf(ctx);
      /** @type {{ path: string, bytes: number }[]} */
      const written = [];

      if (format === 'pdf' || format === 'both') {
        const pdfPath = format === 'both' ? `${params.path.replace(/\/+$/, '')}/book.pdf` : params.path;
        const pdf = renderer.bookToPdf(bundle, pages, { preset: preset.id });
        await writeBytes(storage, pdfPath, pdf);
        written.push({ path: pdfPath, bytes: pdf.length });
      }

      if (format === 'images' || format === 'both') {
        const pagePreviews = /** @type {any} */ (ctx.services?.pagePreviews);
        const contactSheet = /** @type {any} */ (ctx.services?.contactSheet);
        if (typeof pagePreviews !== 'function' || typeof contactSheet !== 'function') {
          throw new Error('this host has no image export services');
        }
        const scale = params.scale ?? preset.imageScale;
        const previews = pagePreviews(bundle, pages, { scale });
        for (let i = 0; i < previews.length; i++) {
          const out = `${params.path.replace(/\/+$/, '')}/page-${String(i + 1).padStart(3, '0')}.png`;
          await writeBytes(storage, out, previews[i].png);
          written.push({ path: out, bytes: previews[i].png.length });
        }
        if (params.contactSheet !== false) {
          const sheet = contactSheet(bundle, pages, {});
          const out = `${params.path.replace(/\/+$/, '')}/contact-sheet.png`;
          await writeBytes(storage, out, sheet.png);
          written.push({ path: out, bytes: sheet.png.length });
        }
      }

      const base = params.path.replace(/\/+$/, '');
      const report = format === 'pdf' ? `${base}.export.json` : `${base}/export.json`;
      const reportBody = `${JSON.stringify({
        preset: preset.id,
        pages: pages.map((p) => p.id),
        files: written,
        problems,
        generatedAt: ctx.env.now ? ctx.env.now() : 0,
      }, null, 2)}\n`;
      await storage.write(report, reportBody);
      written.push({ path: report, bytes: reportBody.length });

      return ok({
        preset: preset.id,
        format,
        pages: pages.length,
        files: written,
        warnings: problems.filter((p) => p.severity === 'warning').length,
      });
    },
  },

  {
    id: 'comic.page.render',
    label: 'Render Page',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        format: { type: 'string', enum: ['png', 'svg'] },
        scale: { type: 'number', minimum: 0.05, maximum: 4 },
      },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const format = params.format ?? 'png';
      const renderer = rendererOf(ctx);
      if (format === 'svg') {
        const svg = renderer.pageToSvg(ctx.bundle, page);
        return ok({ pageId: params.pageId, format, bytes: svg.length, data: svg });
      }
      const png = renderer.pageToPng(ctx.bundle, page, params.scale ?? 0.5);
      const artboard = findArtboard(ctx.bundle.document, page.artboardId);
      return ok({
        pageId: params.pageId, format, bytes: png.length,
        width: artboard ? Math.round(artboard.size.width * (params.scale ?? 0.5)) : 0,
        height: artboard ? Math.round(artboard.size.height * (params.scale ?? 0.5)) : 0,
        data: `data:image/png;base64,${png.toString('base64')}`,
      });
    },
  },

  {
    id: 'comic.page.export',
    label: 'Export Page',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        path: { type: 'string', minLength: 1 },
        format: { type: 'string', enum: ['png', 'svg', 'pdf'] },
        preset: { type: 'string', enum: Object.keys(PRESETS) },
        scale: { type: 'number', minimum: 0.05, maximum: 4 },
        allowMissingAssets: { type: 'boolean' },
        allowMissingFonts: { type: 'boolean' },
      },
      required: ['pageId', 'path'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const { bundle } = ctx;
      const page = bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const format = params.format ?? (params.path.endsWith('.pdf') ? 'pdf' : params.path.endsWith('.svg') ? 'svg' : 'png');
      const preset = presetOf(params.preset ?? 'screen');
      const problems = validateForExport(bundle, {
        preset: preset.id,
        pageIds: [page.id],
        fonts: fontsOf(ctx),
        allowMissingAssets: params.allowMissingAssets === true,
        allowMissingFonts: params.allowMissingFonts === true,
        checkOverflow: true,
      });
      const errors = publishErrors(problems);
      if (errors.length > 0) return fail('conflict', `the export is blocked: ${errors[0].message}`);
      const storage = storageOf(ctx);
      const renderer = rendererOf(ctx);
      if (format === 'pdf') {
        const pdf = renderer.bookToPdf(bundle, [page], { preset: preset.id });
        await writeBytes(storage, params.path, pdf);
        return ok({ pageId: page.id, format, path: params.path, bytes: pdf.length, preset: preset.id });
      }
      if (format === 'svg') {
        const svg = renderer.pageToSvg(bundle, page);
        await storage.write(params.path, svg);
        return ok({ pageId: page.id, format, path: params.path, bytes: svg.length });
      }
      const png = renderer.pageToPng(bundle, page, params.scale ?? preset.imageScale);
      await writeBytes(storage, params.path, png);
      return ok({ pageId: page.id, format, path: params.path, bytes: png.length });
    },
  },

  {
    id: 'comic.project.package',
    label: 'Package Project',
    serverOnly: true,
    params: {
      type: 'object',
      properties: { path: { type: 'string', minLength: 1, description: 'The .comiccraft archive to write.' } },
      required: ['path'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const packageProject = /** @type {any} */ (ctx.services?.packageProject);
      if (typeof packageProject !== 'function') throw new Error('this host cannot package a project');
      const packed = packageProject(ctx.bundle);
      if (!packed.ok) {
        const first = packed.problems.find((/** @type {any} */ p) => p.severity === 'error');
        return fail('conflict', `the project is not valid, so it cannot be packaged: ${first?.message ?? 'unknown problem'}`);
      }
      await writeBytes(storageOf(ctx), params.path, packed.bytes);
      return ok({
        path: params.path,
        bytes: packed.bytes.length,
        files: [...packed.files.keys()].sort().length,
        archiveVersion: '1.0',
      });
    },
  },

  {
    id: 'comic.project.unpack',
    label: 'Unpack Project',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        archivePath: { type: 'string', minLength: 1 },
        path: { type: 'string', minLength: 1, description: 'Directory to unpack into; open this path afterwards.' },
      },
      required: ['archivePath', 'path'],
      additionalProperties: false,
    },
    run: async (ctx, params) => {
      const unpackProject = /** @type {any} */ (ctx.services?.unpackProject);
      if (typeof unpackProject !== 'function') throw new Error('this host cannot unpack a project');
      const storage = storageOf(ctx);
      if (typeof storage.readBytes !== 'function') {
        return fail('disabled', 'this storage backend cannot read binary archives');
      }
      const bytes = await storage.readBytes(params.archivePath);
      if (!bytes) return fail('not-found', `no archive at ${params.archivePath}`, 'archivePath');
      const unpacked = unpackProject(bytes);
      if (!unpacked.ok) {
        return fail('conflict', `the package could not be unpacked: ${unpacked.problems[0]}`);
      }
      for (const [name, data] of unpacked.files) {
        await storage.write(`${params.path.replace(/\/+$/, '')}/${name}`, data);
      }
      return ok({
        path: params.path,
        files: unpacked.files.size,
        paths: [...unpacked.files.keys()].sort(),
      });
    },
  },
];

/**
 * @param {any} storage @param {string} path @param {Uint8Array|Buffer} bytes
 */
async function writeBytes(storage, path, bytes) {
  if (typeof storage.writeBytes === 'function') {
    await storage.writeBytes(path, bytes);
    return;
  }
  // A storage backend from an older test may only implement text. Base64 is not the archive
  // format; this branch is a fallback for a custom host, not a supported package encoding.
  await storage.write(path, Buffer.from(bytes).toString('base64'));
}
