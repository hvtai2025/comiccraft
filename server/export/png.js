/**
 * A minimal rasteriser and PNG encoder, with **no native dependency**.
 *
 * This exists so that PNG export is real and testable in `node --test` — no canvas package,
 * no headless browser, no font file. It draws what the document model actually contains:
 * axis-aligned rectangles (panels, balloon bodies, image placeholders) and a block-glyph
 * approximation for text.
 *
 * **What it is not.** It does not render real glyphs, curves, gradients or blend modes. The
 * browser draws the real thing with canvas; this produces a deterministic raster for tests,
 * thumbnails and CI, and it says so rather than pretending otherwise.
 *
 * @module server/export/png
 */

import { deflateSync } from 'node:zlib';
import { artboardNodes, isFrame, isImage, isText } from '../../shared/domain/document.js';
import { fitText, stubMeasure, tailGeometry } from '../../shared/domain/lettering.js';
import { right, bottom, intersection, quantize } from '../../shared/domain/geom.js';

/** @typedef {{ r: number, g: number, b: number, a: number }} RGBA */

/**
 * @typedef {object} Surface
 * @property {number} width
 * @property {number} height
 * @property {Uint8ClampedArray} pixels  RGBA, row-major, top-left origin
 */

/**
 * @param {number} width @param {number} height @returns {Surface}
 */
export function createSurface(width, height) {
  return { width, height, pixels: new Uint8ClampedArray(width * height * 4) };
}

/**
 * @param {string} colour
 * @returns {RGBA}
 */
export function parseColour(colour) {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(String(colour).trim());
  if (!hex) return { r: 0, g: 0, b: 0, a: 255 };
  let value = hex[1];
  if (value.length === 3) value = value.split('').map((c) => c + c).join('');
  const n = Number.parseInt(value, 16);
  if (value.length === 6) return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255, a: 255 };
  return { r: (n >>> 24) & 255, g: (n >> 16) & 255, b: (n >> 8) & 255, a: n & 255 };
}

/**
 * Source-over alpha blend of one pixel.
 * @param {Surface} surface @param {number} x @param {number} y @param {RGBA} colour @param {number} alpha
 */
function blend(surface, x, y, colour, alpha) {
  if (x < 0 || y < 0 || x >= surface.width || y >= surface.height) return;
  const a = Math.max(0, Math.min(1, alpha));
  if (a === 0) return;
  const i = (y * surface.width + x) * 4;
  const dstA = surface.pixels[i + 3] / 255;
  const outA = a + dstA * (1 - a);
  if (outA === 0) return;
  surface.pixels[i] = (colour.r * a + surface.pixels[i] * dstA * (1 - a)) / outA;
  surface.pixels[i + 1] = (colour.g * a + surface.pixels[i + 1] * dstA * (1 - a)) / outA;
  surface.pixels[i + 2] = (colour.b * a + surface.pixels[i + 2] * dstA * (1 - a)) / outA;
  surface.pixels[i + 3] = outA * 255;
}

/**
 * Fill a rectangle, optionally covering only part of it.
 * @param {Surface} surface
 * @param {import('../../shared/domain/geom.js').Rect} r
 * @param {RGBA} colour
 * @param {{ clip?: import('../../shared/domain/geom.js').Rect | null, alpha?: number }} [options]
 */
export function fillRect(surface, r, colour, options = {}) {
  const alpha = options.alpha ?? colour.a / 255;
  let area = { x: r.x, y: r.y, width: r.width, height: r.height };
  if (options.clip) {
    const shared = intersection(area, options.clip);
    if (!shared) return;
    area = shared;
  }
  const x0 = Math.max(0, Math.floor(area.x));
  const y0 = Math.max(0, Math.floor(area.y));
  const x1 = Math.min(surface.width, Math.ceil(right(area)));
  const y1 = Math.min(surface.height, Math.ceil(bottom(area)));
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) blend(surface, x, y, colour, alpha);
  }
}

/**
 * A rectangle outline of `width` pixels, drawn inside the rect so the frame does not grow.
 * @param {Surface} surface
 * @param {import('../../shared/domain/geom.js').Rect} r
 * @param {RGBA} colour
 * @param {number} width
 * @param {import('../../shared/domain/geom.js').Rect | null} [clip]
 */
export function strokeRect(surface, r, colour, width, clip = null) {
  const w = Math.max(1, Math.round(width));
  fillRect(surface, { x: r.x, y: r.y, width: r.width, height: w }, colour, { clip });
  fillRect(surface, { x: r.x, y: bottom(r) - w, width: r.width, height: w }, colour, { clip });
  fillRect(surface, { x: r.x, y: r.y, width: w, height: r.height }, colour, { clip });
  fillRect(surface, { x: right(r) - w, y: r.y, width: w, height: r.height }, colour, { clip });
}

/**
 * A 5×7 bitmap font for the printable ASCII range. It exists so a rasterised page shows
 * which words are where — enough to test wrapping, overflow and placement, and honest about
 * being an approximation.
 * @type {Record<string, string[]>}
 */
const GLYPHS = (() => {
  /** @type {Record<string, string[]>} */
  const g = {};
  const def = ['01110', '10001', '10001', '11111', '10001', '10001', '10001'];
  for (const ch of 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789') {
    // A deterministic pattern per character: stable across runs and platforms, and visibly
    // a placeholder rather than a wrong letterform.
    let h = 0;
    for (const c of ch) h = (h * 31 + /** @type {number} */ (c.codePointAt(0))) % 21846;
    const bits = [];
    for (let row = 0; row < 7; row++) {
      let line = '';
      for (let col = 0; col < 5; col++) {
        const bit = (h >> ((row * 5 + col) % 15)) & 1;
        line += bit === 1 || row === 0 || row === 6 || col === 0 ? '1' : '0';
      }
      bits.push(line);
    }
    g[ch] = bits;
  }
  g[' '] = ['00000', '00000', '00000', '00000', '00000', '00000', '00000'];
  g['.'] = ['00000', '00000', '00000', '00000', '00000', '01100', '01100'];
  g[','] = ['00000', '00000', '00000', '00000', '01100', '01100', '11000'];
  g['!'] = ['00100', '00100', '00100', '00100', '00100', '00000', '00100'];
  g['?'] = ['01110', '10001', '00001', '00110', '00100', '00000', '00100'];
  g['-'] = ['00000', '00000', '00000', '11111', '00000', '00000', '00000'];
  g["'"] = ['00100', '00100', '00000', '00000', '00000', '00000', '00000'];
  g['"'] = ['01010', '01010', '00000', '00000', '00000', '00000', '00000'];
  for (const ch of '0123456789') if (!g[ch]) g[ch] = def;
  return g;
})();

/**
 * Draw a line of placeholder glyphs.
 * @param {Surface} surface
 * @param {string} text
 * @param {number} x @param {number} y   baseline-ish, top-left of the first glyph cell
 * @param {number} pixelSize             height of one glyph cell in pixels
 * @param {RGBA} colour
 * @param {import('../../shared/domain/geom.js').Rect | null} [clip]
 * @param {'left'|'center'|'right'} [align]
 * @param {number} [advance]             horizontal advance per character, in pixels
 */
export function drawText(surface, text, x, y, pixelSize, colour, clip = null, align = 'left', advance = pixelSize / 7 * 6) {
  const size = Math.max(2, Math.round(pixelSize));
  const rowHeight = Math.max(1, Math.floor(size / 7));
  const colWidth = Math.max(1, Math.round(rowHeight * 0.8));
  const cellWidth = advance;
  const width = text.length * cellWidth;
  let startX = x;
  if (align === 'center') startX = x - width / 2;
  if (align === 'right') startX = x - width;

  for (let i = 0; i < text.length; i++) {
    const glyph = GLYPHS[text[i]] ?? GLYPHS['?'];
    for (let row = 0; row < 7; row++) {
      for (let col = 0; col < 5; col++) {
        if (glyph[row][col] !== '1') continue;
        fillRect(surface, {
          x: Math.round(startX + i * cellWidth + col * colWidth),
          y: Math.round(y + row * rowHeight),
          width: colWidth,
          height: rowHeight,
        }, colour, { clip });
      }
    }
  }
}

/**
 * @param {Surface} surface
 * @param {{ points: { x: number, y: number }[] }} triangle
 * @param {RGBA} colour
 */
export function fillTriangle(surface, triangle, colour) {
  const [a, b, c] = triangle.points;
  const minY = Math.max(0, Math.floor(Math.min(a.y, b.y, c.y)));
  const maxY = Math.min(surface.height - 1, Math.ceil(Math.max(a.y, b.y, c.y)));
  const minX = Math.max(0, Math.floor(Math.min(a.x, b.x, c.x)));
  const maxX = Math.min(surface.width - 1, Math.ceil(Math.max(a.x, b.x, c.x)));
  const edge = (/** @type {any} */ p, /** @type {any} */ q, /** @type {number} */ px, /** @type {number} */ py) =>
    (q.x - p.x) * (py - p.y) - (q.y - p.y) * (px - p.x);
  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const e0 = edge(a, b, x + 0.5, y + 0.5);
      const e1 = edge(b, c, x + 0.5, y + 0.5);
      const e2 = edge(c, a, x + 0.5, y + 0.5);
      const hasNeg = e0 < 0 || e1 < 0 || e2 < 0;
      const hasPos = e0 > 0 || e1 > 0 || e2 > 0;
      if (!(hasNeg && hasPos)) blend(surface, x, y, colour, colour.a / 255);
    }
  }
}

/**
 * PNG chunk with a CRC, as the format requires.
 * @param {string} type
 * @param {Uint8Array} data
 * @returns {Buffer}
 */
function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const typeAndData = Buffer.concat([Buffer.from(type, 'latin1'), Buffer.from(data)]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(typeAndData), 0);
  return Buffer.concat([length, typeAndData, crc]);
}

/** @type {Int32Array | null} */
let CRC_TABLE = null;
/** @param {Buffer} buf @returns {number} */
function crc32(buf) {
  if (!CRC_TABLE) {
    CRC_TABLE = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC_TABLE[n] = c;
    }
  }
  let c = 0xffffffff;
  for (const byte of buf) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/**
 * Fill an ellipse, one horizontal span per row.
 * @param {Surface} surface
 * @param {number} cx @param {number} cy @param {number} rx @param {number} ry
 * @param {RGBA} colour @param {import('../../shared/domain/geom.js').Rect|null} [clip]
 */
export function fillEllipse(surface, cx, cy, rx, ry, colour, clip = null) {
  if (rx <= 0 || ry <= 0) return;
  const y0 = Math.max(0, Math.floor(cy - ry));
  const y1 = Math.min(surface.height - 1, Math.ceil(cy + ry));
  for (let y = y0; y <= y1; y++) {
    const dy = (y - cy) / ry;
    const span = rx * Math.sqrt(Math.max(0, 1 - dy * dy));
    fillRect(surface, { x: cx - span, y, width: span * 2, height: 1 }, colour, { clip });
  }
}

/**
 * Fill a polygon with a scanline rule.
 * @param {Surface} surface
 * @param {{ x: number, y: number }[]} points
 * @param {RGBA} colour @param {import('../../shared/domain/geom.js').Rect|null} [clip]
 */
export function fillPolygon(surface, points, colour, clip = null) {
  if (points.length < 3) return;
  let minY = Infinity, maxY = -Infinity;
  for (const p of points) { minY = Math.min(minY, p.y); maxY = Math.max(maxY, p.y); }
  const y0 = Math.max(0, Math.floor(minY));
  const y1 = Math.min(surface.height - 1, Math.ceil(maxY));
  for (let y = y0; y <= y1; y++) {
    const scan = y + 0.5;
    /** @type {number[]} */
    const hits = [];
    for (let i = 0; i < points.length; i++) {
      const a = points[i];
      const b = points[(i + 1) % points.length];
      if ((a.y <= scan && b.y > scan) || (b.y <= scan && a.y > scan)) {
        hits.push(a.x + ((scan - a.y) / (b.y - a.y)) * (b.x - a.x));
      }
    }
    hits.sort((a, b) => a - b);
    for (let i = 0; i + 1 < hits.length; i += 2) {
      fillRect(surface, { x: hits[i], y, width: hits[i + 1] - hits[i], height: 1 }, colour, { clip });
    }
  }
}

/**
 * @param {Surface} surface
 * @returns {Buffer}
 */
export function encodePng(surface) {
  const { width, height, pixels } = surface;
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;   // bit depth
  ihdr[9] = 6;   // colour type: RGBA
  ihdr[10] = 0;  // deflate
  ihdr[11] = 0;  // adaptive filtering
  ihdr[12] = 0;  // no interlace

  // One filter byte per scanline; filter 0 (None) keeps the encoder simple and the output
  // still compresses well for flat comic art.
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    Buffer.from(pixels.buffer, pixels.byteOffset + y * width * 4, width * 4)
      .copy(raw, y * (width * 4 + 1) + 1);
  }

  return Buffer.concat([
    signature,
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', new Uint8Array(0)),
  ]);
}

/**
 * @param {Uint8Array} bytes
 * @returns {Promise<{ width: number, height: number }>}
 */
export async function pngSize(bytes) {
  if (bytes.length < 24) throw new Error('not a PNG: too short');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(0) !== 0x89504e47) throw new Error('not a PNG: bad signature');
  return { width: view.getUint32(16), height: view.getUint32(20) };
}

/**
 * Rasterise one page.
 * @param {any} bundle
 * @param {any} page
 * @param {number} scale
 * @param {{ measure?: import('../../shared/domain/lettering.js').MeasureText, drawText?: boolean }} [options]
 * @returns {Surface}
 */
export function pageToSurface(bundle, page, scale = 1, options = {}) {
  const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  if (!artboard) throw new Error(`page ${page.id} has no artboard`);
  const width = Math.max(1, Math.round(artboard.size.width * scale));
  const height = Math.max(1, Math.round(artboard.size.height * scale * 1));
  const surface = createSurface(width, height);
  const s = scale;

  fillRect(surface, { x: 0, y: 0, width, height }, parseColour('#ffffff'));

  const nodes = artboardNodes(bundle.document, artboard.id);
  const white = parseColour('#ffffff');
  const ink = parseColour('#111111');

  for (const node of nodes.filter(isFrame).filter((n) => n.name !== 'Balloon Tail')) {
    const r = { x: node.frame.x * s, y: node.frame.y * s, width: node.frame.width * s, height: node.frame.height * s };
    fillRect(surface, r, parseColour(node.fill ?? '#ffffff'));
    if (node.stroke) strokeRect(surface, r, parseColour(node.stroke), (node.strokeWidth ?? 1) * s);
  }

  for (const node of nodes.filter(isImage)) {
    const clip = node.clip
      ? { x: node.clip.x * s, y: node.clip.y * s, width: node.clip.width * s, height: node.clip.height * s }
      : null;
    const r = { x: node.frame.x * s, y: node.frame.y * s, width: node.frame.width * s, height: node.frame.height * s };
    fillRect(surface, r, parseColour('#dfe6ef'), { clip });
    // A diagonal cross marks a placeholder asset, so a rasterised page cannot be mistaken
    // for one with real art in it.
    const steps = Math.max(4, Math.round(Math.hypot(r.width, r.height) / 8));
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      fillRect(surface, { x: r.x + r.width * t, y: r.y + r.height * (1 - t), width: 2, height: 2 }, parseColour('#b9c6d6'), { clip });
    }
    if (node.clip) strokeRect(surface, r, clip ? parseColour('#8fa3bb') : ink, 1, clip);
  }

  /** @type {Map<number, any>} */
  const letteringByNode = new Map();
  for (const l of Object.values(bundle.project.lettering ?? {})) {
    const e = /** @type {any} */ (l);
    letteringByNode.set(e.nodeId, e);
  }

  // Balloon bodies and tails, then the text on top of them.
  for (const node of nodes.filter(isText)) {
    const lettering = letteringByNode.get(node.id);
    const r = { x: node.frame.x * s, y: node.frame.y * s, width: node.frame.width * s, height: node.frame.height * s };
    const variant = lettering?.kind === 'balloon' ? (lettering.variant ?? 'speech') : null;
    if (lettering?.kind === 'balloon') {
      if (variant === 'thought') {
        fillEllipse(surface, r.x + r.width / 2, r.y + r.height / 2, r.width / 2, r.height / 2, white);
        const bump = Math.min(r.width, r.height) * 0.12;
        for (let i = 0; i < 5; i++) {
          const angle = (Math.PI * 2 * i) / 5;
          fillEllipse(surface, r.x + r.width / 2 + Math.cos(angle) * r.width * 0.42, r.y + r.height / 2 + Math.sin(angle) * r.height * 0.4, bump, bump, white);
        }
      } else if (variant === 'shout') {
        const cx = r.x + r.width / 2, cy = r.y + r.height / 2;
        /** @type {{ x: number, y: number }[]} */
        const points = [];
        for (let i = 0; i < 28; i++) {
          const angle = (Math.PI * i) / 14;
          const rx = i % 2 === 0 ? r.width / 2 : r.width * 0.42;
          const ry = i % 2 === 0 ? r.height / 2 : r.height * 0.38;
          points.push({ x: cx + Math.cos(angle) * rx, y: cy + Math.sin(angle) * ry });
        }
        fillPolygon(surface, points, white);
      } else {
        fillRect(surface, r, white);
        strokeRect(surface, r, ink, 2 * s);
        if (variant === 'whisper') {
          // A dashed outline is drawn as alternating spans, which keeps the raster readable.
          for (let x = r.x; x < r.x + r.width; x += 12 * s) {
            strokeRect(surface, { x, y: r.y, width: Math.min(6 * s, r.x + r.width - x), height: 2 * s }, ink, 1);
            strokeRect(surface, { x, y: r.y + r.height - 2 * s, width: Math.min(6 * s, r.x + r.width - x), height: 2 * s }, ink, 1);
          }
        }
      }
      const tailNode = lettering.tailNodeId
        ? nodes.find((/** @type {any} */ n) => n.id === lettering.tailNodeId)
        : null;
      if (tailNode) {
        const tail = tailGeometry(node.frame, tailNode);
        const base = { x: tail.base.x * s, y: tail.base.y * s };
        const tip = { x: tail.tip.x * s, y: tail.tip.y * s };
        if (variant === 'thought') {
          for (let i = 1; i <= 3; i++) {
            const t = i / 4;
            fillEllipse(surface, base.x + (tip.x - base.x) * t, base.y + (tip.y - base.y) * t, (8 - i * 1.6) * s, (8 - i * 1.6) * s, white);
          }
        } else {
          const dx = tip.x - base.x;
          const dy = tip.y - base.y;
          const len = Math.hypot(dx, dy) || 1;
          const px = (-dy / len) * 7 * s;
          const py = (dx / len) * 7 * s;
          fillTriangle(surface, { points: [
            { x: base.x + px, y: base.y + py },
            { x: base.x - px, y: base.y - py },
            tip,
          ] }, white);
        }
      }
    } else if (lettering?.kind === 'caption') {
      fillRect(surface, r, parseColour('#fdf6d8'));
      strokeRect(surface, r, parseColour('#c9bd86'), 1.5 * s);
    }
  }

  if (options.drawText !== false) {
    const measure = options.measure ?? stubMeasure(0.5);
    for (const node of nodes.filter(isText)) {
      const lettering = letteringByNode.get(node.id);
      const defaults = bundle.project.defaultLetteringStyle;
      const style = {
        family: lettering?.fontFamily ?? defaults.family,
        sizePt: lettering?.fontSize ?? node.fontSize ?? defaults.sizePt,
        padding: lettering?.padding ?? defaults.padding ?? 8,
      };
      const fit = fitText({
        text: node.text ?? '',
        frame: node.frame,
        style,
        measure,
        policy: lettering?.overflowPolicy ?? 'warn',
        padding: style.padding,
      });
      const pad = style.padding * s;
      const align = (lettering?.align ?? node.align) === 'center' ? 'center' : (lettering?.align ?? node.align) === 'right' ? 'right' : 'left';
      const tx = align === 'center' ? (node.frame.x + node.frame.width / 2) * s
        : align === 'right' ? (node.frame.x + node.frame.width) * s - pad
        : node.frame.x * s + pad;
      const clipRect = { x: node.frame.x * s, y: node.frame.y * s, width: node.frame.width * s, height: node.frame.height * s };
      for (let i = 0; i < fit.lines.length; i++) {
        drawText(
          surface, fit.lines[i], tx, (node.frame.y + style.padding) * s + i * fit.lineHeight * s,
          fit.fontSize * s, ink, clipRect, align,
        );
      }
      if (fit.overflowed) strokeRect(surface, clipRect, parseColour('#c0392b'), 1.5 * s);
    }
  }

  return surface;
}

/**
 * @param {any} bundle @param {any} page @param {number} scale
 * @param {{ measure?: import('../../shared/domain/lettering.js').MeasureText }} [options]
 * @returns {Buffer}
 */
export function pageToPng(bundle, page, scale = 1, options = {}) {
  if (!Number.isFinite(scale) || scale <= 0) throw new Error(`scale must be positive, got ${scale}`);
  return encodePng(pageToSurface(bundle, page, scale, options));
}

