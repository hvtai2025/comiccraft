/**
 * Contact sheets and page previews.
 *
 * A review proof is not a page render: it is a grid of thumbnails with page labels, so a
 * human can see pacing and page order at once. It is composed from the same deterministic
 * raster the PNG export uses, so the proof cannot disagree with the pages.
 *
 * @module server/export/contact
 */

import { createSurface, pageToSurface, encodePng, fillRect, strokeRect, drawText, parseColour } from './png.js';

export const THUMB_WIDTH = 260;
export const SHEET_COLUMNS = 3;

/**
 * @param {any} surface
 * @param {any} source
 * @param {number} dx
 * @param {number} dy
 */
function blit(surface, source, dx, dy) {
  for (let y = 0; y < source.height; y++) {
    const sy = Math.min(source.height - 1, y);
    for (let x = 0; x < source.width; x++) {
      const di = ((dy + y) * surface.width + dx + x) * 4;
      const si = (sy * source.width + x) * 4;
      surface.pixels[di] = source.pixels[si];
      surface.pixels[di + 1] = source.pixels[si + 1];
      surface.pixels[di + 2] = source.pixels[si + 2];
      surface.pixels[di + 3] = source.pixels[si + 3];
    }
  }
}

/**
 * @param {any} bundle
 * @param {any[]} pages
 * @param {{ columns?: number, thumbWidth?: number, gap?: number }} [options]
 * @returns {{ png: Buffer, width: number, height: number, columns: number }}
 */
export function contactSheet(bundle, pages, options = {}) {
  const columns = Math.max(1, Math.min(8, options.columns ?? SHEET_COLUMNS));
  const thumbWidth = Math.max(80, Math.min(800, options.thumbWidth ?? THUMB_WIDTH));
  const gap = Math.max(8, options.gap ?? 24);
  const labelHeight = 24;

  /** @type {{ surface: any, height: number, label: string }[]} */
  const thumbs = [];
  for (const page of pages) {
    const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
    if (!artboard) continue;
    const scale = thumbWidth / artboard.size.width;
    const surface = pageToSurface(bundle, page, scale);
    thumbs.push({ surface, height: surface.height, label: page.name ?? page.id });
  }
  if (thumbs.length === 0) {
    const empty = createSurface(thumbWidth, 80);
    fillRect(empty, { x: 0, y: 0, width: thumbWidth, height: 80 }, parseColour('#ffffff'));
    drawText(empty, 'No pages to proof.', 8, 32, 3, parseColour('#5b6b7f'));
    return { png: encodePng(empty), width: empty.width, height: empty.height, columns: 1 };
  }

  const cellHeight = Math.max(...thumbs.map((t) => t.height)) + labelHeight;
  const rows = Math.ceil(thumbs.length / columns);
  const width = columns * thumbWidth + (columns + 1) * gap;
  const height = rows * cellHeight + (rows + 1) * gap;
  const sheet = createSurface(width, height);
  fillRect(sheet, { x: 0, y: 0, width, height }, parseColour('#f4f6f9'));

  thumbs.forEach((thumb, i) => {
    const row = Math.floor(i / columns);
    const col = i % columns;
    const x = gap + col * (thumbWidth + gap);
    const y = gap + row * (cellHeight + gap);
    fillRect(sheet, { x: x - 4, y: y - 4, width: thumbWidth + 8, height: thumb.height + 8 }, parseColour('#ffffff'));
    strokeRect(sheet, { x: x - 4, y: y - 4, width: thumbWidth + 8, height: thumb.height + 8 }, parseColour('#8fa3bb'), 2);
    blit(sheet, thumb.surface, x, y);
    drawText(sheet, `${String(i + 1).padStart(2, '0')}  ${thumb.label}`.slice(0, 60), x, y + thumb.height + 7, 2, parseColour('#26313f'));
  });

  return { png: encodePng(sheet), width, height, columns };
}

/**
 * One PNG per page, at the preset's scale. Returned rather than written so the command layer
 * decides the paths: a renderer should not know about a storage root.
 * @param {any} bundle
 * @param {any[]} pages
 * @param {{ scale?: number }} [options]
 * @returns {{ pageId: string, name: string, png: Buffer, width: number, height: number }[]}
 */
export function pagePreviews(bundle, pages, options = {}) {
  const scale = Math.max(0.05, Math.min(4, options.scale ?? 1));
  return pages.map((page) => {
    const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
    const surface = pageToSurface(bundle, page, scale);
    return {
      pageId: page.id,
      name: page.name ?? page.id,
      png: encodePng(surface),
      width: surface.width,
      height: artboard ? artboard.size.height * scale : surface.height,
    };
  });
}
