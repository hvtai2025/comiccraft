/**
 * A minimal PDF writer, with no dependency.
 *
 * PDF is a text format with a precise byte layout, so writing one is a matter of getting the
 * cross-reference table right. This emits vector content — rectangles for panel frames and
 * balloon bodies, and text with a standard Type-1 base font — which is what makes the output
 * genuinely printable rather than a rasterised screenshot.
 *
 * **Page geometry is the criterion.** Each page's MediaBox is its artboard, measured in PDF
 * points at 72 per inch, so a 300 ppi artboard becomes a correctly sized page rather than one
 * scaled to fit. A test reads the boxes back and asserts this.
 *
 * @module server/export/pdf
 */

import { artboardNodes, isFrame, isImage, isText } from '../../shared/domain/document.js';
import { fitText, stubMeasure, tailGeometry } from '../../shared/domain/lettering.js';
import { quantize } from '../../shared/domain/geom.js';
import { presetGeometry, presetOf } from '../../shared/domain/publish.js';

/** Page units are pixels at 300 ppi, which is the reference's print default. */
export const PAGE_UNITS_PER_INCH = 300;
/** A PDF point is 1/72 inch. */
export const POINTS_PER_INCH = 72;
/** One page unit in PDF points. */
export const PT_PER_UNIT = POINTS_PER_INCH / PAGE_UNITS_PER_INCH;

/**
 * @param {number} units
 * @returns {string} the value in PDF points, as a number literal
 */
const pt = (units) => String(quantize(units * PT_PER_UNIT, 4));

/**
 * @param {any} bundle @param {any[]} pages
 * @param {{ measure?: import('../../shared/domain/lettering.js').MeasureText, title?: string, preset?: string }} [options]
 * @returns {Buffer}
 */
export function bookToPdf(bundle, pages, options = {}) {
  if (pages.length === 0) throw new Error('cannot write a PDF with no pages');
  const measure = options.measure ?? stubMeasure(0.5);
  const preset = options.preset ? presetOf(options.preset) : null;
  const bodies = pages.map((page) => {
    const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
    if (!artboard) throw new Error(`page ${page.id} has no artboard`);
    const geometry = preset ? presetGeometry(artboard.size, preset) : null;
    return pageContent(bundle, page, measure, geometry);
  });

  /** @type {string[]} */
  const objects = [];
  const now = new Date(0).toISOString().replace(/\.\d+Z$/, 'Z');

  // 1: catalogue, 2: page tree, 3: info. Then 2 objects per page: the page and its content.
  const pageObjectIds = pages.map((_, i) => 4 + i * 2);
  objects.push(`<< /Type /Catalog /Pages 2 0 R >>`);
  objects.push(`<< /Type /Pages /Count ${pages.length} /Kids [${pageObjectIds.map((id) => `${id} 0 R`).join(' ')}] >>`);
  objects.push(
    `<< /Title (${escapeText(options.title ?? bundle.project.title ?? 'Comic')}) ` +
    `/Producer (ComicCraft) /CreationDate (D:${now.replace(/[-:]/g, '')}) >>`,
  );

  pages.forEach((page, i) => {
    const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
    if (!artboard) throw new Error(`page ${page.id} has no artboard`);
    const contentId = pageObjectIds[i] + 1;
    const geometry = preset ? presetGeometry(artboard.size, preset) : null;
    const media = geometry
      ? `/MediaBox [0 0 ${pt(geometry.media.width)} ${pt(geometry.media.height)}] ` +
        `/TrimBox [${pt(geometry.trim.x)} ${pt(geometry.trim.y)} ${pt(geometry.trim.x + geometry.trim.width)} ${pt(geometry.trim.y + geometry.trim.height)}] ` +
        `/BleedBox [${pt(geometry.bleed.x)} ${pt(geometry.bleed.y)} ${pt(geometry.bleed.x + geometry.bleed.width)} ${pt(geometry.bleed.y + geometry.bleed.height)}] `
      : `/MediaBox [0 0 ${pt(artboard.size.width)} ${pt(artboard.size.height)}] `;
    objects.push(
      `<< /Type /Page /Parent 2 0 R ${media}` +
      `/Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> ` +
      `/F2 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >> >> >> ` +
      `/Contents ${contentId} 0 R >>`,
    );
    const body = bodies[i];
    objects.push(`<< /Length ${Buffer.byteLength(body, 'latin1')} >>\nstream\n${body}\nendstream`);
  });

  return assemble(objects);
}

/**
 * @param {any} bundle @param {any} page
 * @param {import('../../shared/domain/lettering.js').MeasureText} measure
 * @param {{ media: any, trim: any, bleed: any, contentOffset: { x: number, y: number }, marks: boolean } | null} [geometry]
 * @returns {string}
 */
function pageContent(bundle, page, measure, geometry = null) {
  const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  const nodes = artboardNodes(bundle.document, artboard.id);
  /** @type {Map<number, any>} */
  const letteringByNode = new Map();
  for (const l of Object.values(bundle.project.lettering ?? {})) {
    const e = /** @type {any} */ (l);
    letteringByNode.set(e.nodeId, e);
  }

  /** @type {string[]} */
  const ops = [];
  if (geometry) {
    // Translate the artboard content into the trim box. The offset includes bleed and the
    // mark margin, so the same drawing sits exactly inside its trim rectangle.
    ops.push('q');
    ops.push(`1 0 0 1 ${pt(geometry.contentOffset.x)} ${pt(geometry.contentOffset.y)} cm`);
  }
  ops.push('1 1 1 rg');
  ops.push(`0 0 ${pt(artboard.size.width)} ${pt(artboard.size.height)} re f`);

  /** @param {import('../../shared/domain/geom.js').Rect} r */
  const y = (r) => artboard.size.height - r.y - r.height;

  for (const node of nodes.filter(isFrame).filter((n) => n.name !== 'Balloon Tail')) {
    const fill = parseColour(node.fill ?? '#ffffff');
    ops.push(`${fill} rg`);
    ops.push(`${pt(node.frame.x)} ${pt(y(node.frame))} ${pt(node.frame.width)} ${pt(node.frame.height)} re f`);
    if (node.stroke) {
      const stroke = parseColour(node.stroke);
      const w = quantize((node.strokeWidth ?? 1) * PT_PER_UNIT, 3);
      ops.push(`${stroke} RG ${w} w`);
      ops.push(`${pt(node.frame.x)} ${pt(y(node.frame))} ${pt(node.frame.width)} ${pt(node.frame.height)} re S`);
    }
  }

  for (const node of nodes.filter(isImage)) {
    const clip = node.clip ? y(node.clip) : null;
    if (clip !== null && node.clip) {
      ops.push('q');
      ops.push(`${pt(node.clip.x)} ${pt(clip)} ${pt(node.clip.width)} ${pt(node.clip.height)} re W n`);
    }
    ops.push('0.87 0.9 0.94 rg');
    ops.push(`${pt(node.frame.x)} ${pt(y(node.frame))} ${pt(node.frame.width)} ${pt(node.frame.height)} re f`);
    ops.push('0.72 0.78 0.84 RG 1 w');
    ops.push(`${pt(node.frame.x)} ${pt(y(node.frame))} ${pt(node.frame.width)} ${pt(node.frame.height)} re S`);
    if (clip !== null) ops.push('Q');
  }

  for (const node of nodes.filter(isText)) {
    const lettering = letteringByNode.get(node.id);
    const r = node.frame;
    const defaults = bundle.project.defaultLetteringStyle;
    const style = {
      family: lettering?.fontFamily ?? defaults.family,
      sizePt: lettering?.fontSize ?? node.fontSize ?? defaults.sizePt,
      align: lettering?.align ?? node.align ?? defaults.align ?? 'left',
      padding: lettering?.padding ?? defaults.padding ?? 8,
    };
    const variant = lettering?.kind === 'balloon' ? (lettering.variant ?? 'speech') : null;

    if (lettering?.kind === 'balloon') {
      ops.push('1 1 1 rg 0.07 0.07 0.07 RG 0.5 w');
      if (variant === 'thought') {
        pdfEllipse(ops, r, artboard.size.height);
        for (let i = 0; i < 5; i++) {
          const angle = (Math.PI * 2 * i) / 5;
          const bump = Math.min(r.width, r.height) * 0.12;
          const cx = r.x + r.width / 2 + Math.cos(angle) * r.width * 0.42;
          const cy = r.y + r.height / 2 + Math.sin(angle) * r.height * 0.4;
          pdfEllipse(ops, { x: cx - bump, y: cy - bump, width: bump * 2, height: bump * 2 }, artboard.size.height);
        }
      } else if (variant === 'shout') {
        const cx = r.x + r.width / 2;
        const cy = r.y + r.height / 2;
        const points = [];
        for (let i = 0; i < 28; i++) {
          const angle = (Math.PI * i) / 14;
          const rx = i % 2 === 0 ? r.width / 2 : r.width * 0.42;
          const ry = i % 2 === 0 ? r.height / 2 : r.height * 0.38;
          points.push({ x: cx + Math.cos(angle) * rx, y: cy + Math.sin(angle) * ry });
        }
        ops.push(pdfPolygon(points, artboard.size.height) + ' B');
      } else {
        if (variant === 'whisper') ops.push('[4 3] 0 d');
        ops.push(`${pt(r.x)} ${pt(y(r))} ${pt(r.width)} ${pt(r.height)} re B`);
        if (variant === 'whisper') ops.push('[] 0 d');
      }

      const tailNode = lettering.tailNodeId ? bundle.document.nodes[String(lettering.tailNodeId)] : null;
      if (tailNode) {
        const tail = tailGeometry(r, tailNode);
        if (variant === 'thought') {
          for (let i = 1; i <= 3; i++) {
            const t = i / 4;
            const radius = (8 - i * 1.6);
            const cx = tail.base.x + (tail.tip.x - tail.base.x) * t;
            const cy = tail.base.y + (tail.tip.y - tail.base.y) * t;
            pdfEllipse(ops, { x: cx - radius, y: cy - radius, width: radius * 2, height: radius * 2 }, artboard.size.height);
          }
        } else {
          const dx = tail.tip.x - tail.base.x;
          const dy = tail.tip.y - tail.base.y;
          const len = Math.hypot(dx, dy) || 1;
          const px = (-dy / len) * 7;
          const py = (dx / len) * 7;
          ops.push(pdfPolygon([
            { x: tail.base.x + px, y: tail.base.y + py },
            { x: tail.base.x - px, y: tail.base.y - py },
            tail.tip,
          ], artboard.size.height) + ' B');
        }
      }
    } else if (lettering?.kind === 'caption') {
      ops.push('0.99 0.96 0.85 rg');
      ops.push(`${pt(r.x)} ${pt(y(r))} ${pt(r.width)} ${pt(r.height)} re f`);
      ops.push('0.79 0.74 0.53 RG 0.4 w');
      ops.push(`${pt(r.x)} ${pt(y(r))} ${pt(r.width)} ${pt(r.height)} re S`);
    }

    const fit = fitText({
      text: node.text ?? '',
      frame: r,
      style,
      measure,
      policy: lettering?.overflowPolicy ?? 'warn',
      padding: style.padding,
    });

    // A standard base font, so the file opens anywhere without embedding a typeface. The
    // comic faces arrive when the project supplies them; a PDF that cannot open is worse
    // than one set in Helvetica.
    const size = quantize(fit.fontSize * PT_PER_UNIT, 2);
    ops.push(`BT /F1 ${size} Tf 0 0 0 rg`);
    for (let i = 0; i < fit.lines.length; i++) {
      const lineY = r.y + style.padding + fit.fontSize * 0.95 + i * fit.lineHeight;
      let x = r.x + style.padding;
      if (style.align === 'center') x = r.x + r.width / 2 - (fit.usedWidth / 2);
      if (style.align === 'right') x = r.x + r.width - style.padding - measure(fit.lines[i], { family: 'sans', sizePt: fit.fontSize });
      ops.push(`1 0 0 1 ${pt(x)} ${pt(artboard.size.height - lineY)} Tm (${escapeText(fit.lines[i])}) Tj`);
    }
    ops.push('ET');

    if (fit.overflowed) {
      ops.push('0.75 0.22 0.17 RG 1 w [3 2] 0 d');
      ops.push(`${pt(r.x)} ${pt(y(r))} ${pt(r.width)} ${pt(r.height)} re S`);
      ops.push('[] 0 d');
    }
  }

  if (geometry) {
    ops.push('Q');
    if (geometry.marks) {
      // Crop marks sit outside the bleed, in the mark margin. They are deliberately simple
      // hairlines: a printer needs their position, not a decoration.
      const c = geometry.trim;
      const gap = 6;
      const markLength = 24;
      const left = c.x;
      const right = c.x + c.width;
      const top = c.y + c.height;
      const bottom = c.y;
      /** @param {number} x1 @param {number} y1 @param {number} x2 @param {number} y2 */
      const mark = (x1, y1, x2, y2) => {
        ops.push(`0 0 0 RG 0.5 w`);
        ops.push(`${pt(x1)} ${pt(y1)} m ${pt(x2)} ${pt(y2)} l S`);
      };
      for (const y of [bottom, top]) {
        mark(left - gap - markLength, y, left - gap, y);
        mark(right + gap, y, right + gap + markLength, y);
      }
      for (const x of [left, right]) {
        mark(x, bottom - gap - markLength, x, bottom - gap);
        mark(x, top + gap, x, top + gap + markLength);
      }
    }
  }
  return ops.join('\n');
}

/**
 * A four-curve ellipse in PDF user space.
 * @param {string[]} ops
 * @param {{ x: number, y: number, width: number, height: number }} r
 * @param {number} pageHeight
 */
function pdfEllipse(ops, r, pageHeight) {
  const k = 0.5522847498;
  const cx = r.x + r.width / 2;
  const cy = r.y + r.height / 2;
  const rx = r.width / 2;
  const ry = r.height / 2;
  const py = (/** @type {number} */ y) => pageHeight - y;
  ops.push(`${pt(cx + rx)} ${pt(py(cy))} m`);
  ops.push(`${pt(cx + rx)} ${pt(py(cy + ry * k))} ${pt(cx + rx * k)} ${pt(py(cy + ry))} ${pt(cx)} ${pt(py(cy + ry))} c`);
  ops.push(`${pt(cx - rx * k)} ${pt(py(cy + ry))} ${pt(cx - rx)} ${pt(py(cy + ry * k))} ${pt(cx - rx)} ${pt(py(cy))} c`);
  ops.push(`${pt(cx - rx)} ${pt(py(cy - ry * k))} ${pt(cx - rx * k)} ${pt(py(cy - ry))} ${pt(cx)} ${pt(py(cy - ry))} c`);
  ops.push(`${pt(cx + rx * k)} ${pt(py(cy - ry))} ${pt(cx + rx)} ${pt(py(cy - ry * k))} ${pt(cx + rx)} ${pt(py(cy))} c`);
  ops.push('B');
}

/**
 * @param {{ x: number, y: number }[]} points
 * @param {number} pageHeight
 * @returns {string} a path operand ending in `h`
 */
function pdfPolygon(points, pageHeight) {
  const parts = [];
  points.forEach((point, i) => {
    parts.push(`${pt(point.x)} ${pt(pageHeight - point.y)} ${i === 0 ? 'm' : 'l'}`);
  });
  parts.push('h');
  return parts.join(' ');
}

/**
 * @param {string} colour
 * @returns {string} an `r g b` operand triple, each component 0–1
 */
function parseColour(colour) {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(colour).trim());
  if (!hex) return '0 0 0';
  let v = hex[1];
  if (v.length === 3) v = v.split('').map((c) => c + c).join('');
  const n = Number.parseInt(v, 16);
  const r = ((n >> 16) & 255) / 255;
  const g = ((n >> 8) & 255) / 255;
  const b = (n & 255) / 255;
  return `${quantize(r, 3)} ${quantize(g, 3)} ${quantize(b, 3)}`;
}

/**
 * PDF string literals escape `\`, `(` and `)`, and non-ASCII has to go.
 * @param {unknown} value
 * @returns {string}
 */
export function escapeText(value) {
  return String(value)
    // The base-14 fonts are Latin-1 and the escape is a byte-wise replacement, so anything
    // outside printable ASCII becomes a visible '?' rather than a byte that breaks the file.
    .replace(/[^\x20-\x7e]/g, '?')
    .replace(/\\/g, '\\\\')
    .replace(/\(/g, '\\(')
    .replace(/\)/g, '\\)');
}

/**
 * Join the objects into a file, computing the cross-reference table. A PDF with a wrong
 * xref does not open, so this is the part that has to be exact.
 * @param {string[]} objects
 * @returns {Buffer}
 */
function assemble(objects) {
  /** @type {string[]} */
  const parts = [];
  parts.push('%PDF-1.4\n');
  // A binary comment marks the file as not-plain-text, which is what every writer does.
  parts.push('%\xE2\xE3\xCF\xD3\n');

  const offsets = [0];
  let cursor = parts.join('').length;

  objects.forEach((body, index) => {
    const id = index + 1;
    const text = `${id} 0 obj\n${body}\nendobj\n`;
    offsets[id] = cursor;
    cursor += Buffer.byteLength(text, 'latin1');
    parts.push(text);
  });

  const xrefOffset = cursor;
  const count = objects.length + 1;
  let xref = `xref\n0 ${count}\n0000000000 65535 f \n`;
  for (let id = 1; id < count; id++) {
    xref += `${String(offsets[id]).padStart(10, '0')} 00000 n \n`;
  }
  parts.push(xref);
  parts.push(`trailer\n<< /Size ${count} /Root 1 0 R /Info 3 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`);

  return Buffer.from(parts.join(''), 'latin1');
}
