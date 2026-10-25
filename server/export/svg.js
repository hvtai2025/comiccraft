/**
 * Document → SVG. Pure and deterministic: same bundle in, byte-identical SVG out.
 *
 * SVG is the interchange format here — it is what the browser can display directly and what
 * a rasteriser or a print pipeline can consume. Text is emitted as `<text>` with a
 * font-family, not as outlines, so the output stays editable and diffable.
 *
 * @module server/export/svg
 */

import { artboardNodes, isFrame, isImage, isText } from '../../shared/domain/document.js';
import { fitText, stubMeasure, tailGeometry } from '../../shared/domain/lettering.js';
import { quantize } from '../../shared/domain/geom.js';

/**
 * @param {unknown} value
 * @returns {string} escaped for use in XML text and attribute values
 */
export function escapeXml(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/**
 * @param {number} n
 * @returns {string}
 */
const num = (n) => String(quantize(n, 3));

/**
 * @param {any} bundle
 * @param {any} page
 * @param {{ measure?: import('../../shared/domain/lettering.js').MeasureText }} [options]
 * @returns {string}
 */
export function pageToSvg(bundle, page, options = {}) {
  const measure = options.measure ?? stubMeasure(0.5);
  const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  if (!artboard) throw new Error(`page ${page.id} has no artboard`);

  const nodes = artboardNodes(bundle.document, artboard.id);

  // Panels draw first, then their artwork, then lettering: a fixed order keeps the output
  // stable, and it matches what a reader expects to see on top.
  const frames = nodes.filter(isFrame);
  const images = nodes.filter(isImage);
  const texts = nodes.filter(isText);

  /** @type {string[]} */
  const parts = [];
  parts.push(`<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="${num(artboard.size.width)}" height="${num(artboard.size.height)}" viewBox="0 0 ${num(artboard.size.width)} ${num(artboard.size.height)}">`);
  parts.push(`<title>${escapeXml(page.name ?? page.id)}</title>`);
  parts.push(`<rect x="0" y="0" width="${num(artboard.size.width)}" height="${num(artboard.size.height)}" fill="#ffffff"/>`);

  for (const frame of frames) parts.push(frameSvg(frame));

  for (const image of images) {
    parts.push(imageSvg(bundle, image));
  }

  const letteringByNode = new Map();
  for (const l of Object.values(bundle.project.lettering ?? {})) {
    const e = /** @type {any} */ (l);
    letteringByNode.set(e.nodeId, e);
  }

  for (const text of texts) {
    parts.push(textSvg(text, bundle, measure, letteringByNode.get(text.id), nodes));
  }

  parts.push('</svg>');
  return `${parts.join('\n')}\n`;
}

/** @param {any} node @returns {string} */
function frameSvg(node) {
  const f = node.frame;
  const stroke = node.stroke ? ` stroke="${escapeXml(node.stroke)}" stroke-width="${num(node.strokeWidth ?? 1)}"` : '';
  return `<rect x="${num(f.x)}" y="${num(f.y)}" width="${num(f.width)}" height="${num(f.height)}" fill="${escapeXml(node.fill ?? 'none')}"${stroke}/>`;
}

/**
 * An image is emitted as a labelled placeholder when the asset bytes are not in the bundle.
 * A missing asset is a **visible** rectangle with its name on it, never a silent gap: a
 * panel that looks empty is indistinguishable from a panel that failed.
 * @param {any} bundle @param {any} node
 * @returns {string}
 */
function imageSvg(bundle, node) {
  const clip = node.clip ?? (node.clipToFrame ? node.frame : null);
  const f = node.frame;
  const asset = Object.values(bundle.project.assets ?? {}).find((a) => /** @type {any} */ (a).id === node.assetId);
  const a = /** @type {any} */ (asset);
  const label = a ? a.name : `missing asset ${node.assetId}`;
  const clipId = `clip-${node.id}`;

  // The crop is a viewBox over the source: cropping changes which part of the image is
  // shown, never the object's geometry, so no panel needs to be resized to crop its art.
  const crop = node.crop ?? { x: 0, y: 0, width: 1, height: 1 };
  const sourceW = 1000, sourceH = 1000;
  /** @type {string[]} */
  const body = [];
  body.push(`<svg x="${num(f.x)}" y="${num(f.y)}" width="${num(f.width)}" height="${num(f.height)}" viewBox="${num(crop.x * sourceW)} ${num(crop.y * sourceH)} ${num(crop.width * sourceW)} ${num(crop.height * sourceH)}" preserveAspectRatio="${node.fit === 'contain' ? 'xMidYMid meet' : 'xMidYMid slice'}">`);
  if (a && typeof a.data === 'string' && a.data.startsWith('data:')) {
    body.push(`<image href="${escapeXml(a.data)}" x="0" y="0" width="${sourceW}" height="${sourceH}"/>`);
  } else {
    body.push(`<rect x="0" y="0" width="${sourceW}" height="${sourceH}" fill="#e9eef5"/>`);
    body.push(`<text x="${sourceW / 2}" y="${sourceH / 2}" font-family="sans-serif" font-size="48" fill="#5b6b7f" text-anchor="middle" dominant-baseline="middle">${escapeXml(label)}</text>`);
  }
  body.push('</svg>');

  if (!clip) return `<g>${body.join('')}</g>`;
  return [
    `<clipPath id="${clipId}"><rect x="${num(clip.x)}" y="${num(clip.y)}" width="${num(clip.width)}" height="${num(clip.height)}"/></clipPath>`,
    `<g clip-path="url(#${clipId})">${body.join('')}</g>`,
  ].join('\n');
}

/**
 * @param {any} node
 * @param {any} bundle
 * @param {import('../../shared/domain/lettering.js').MeasureText} measure
 * @param {any} lettering
 * @param {any[]} nodes
 * @returns {string}
 */
function textSvg(node, bundle, measure, lettering, nodes) {
  const defaults = bundle.project.defaultLetteringStyle;
  const style = {
    family: lettering?.fontFamily ?? defaults.family,
    sizePt: lettering?.fontSize ?? node.fontSize ?? defaults.sizePt,
    align: lettering?.align ?? node.align ?? defaults.align ?? 'left',
    padding: lettering?.padding ?? defaults.padding ?? 8,
    bold: lettering?.bold ?? false,
    italic: lettering?.italic ?? false,
    color: lettering?.color ?? node.color ?? '#111111',
  };
  const fit = fitText({
    text: node.text ?? '',
    frame: node.frame,
    style,
    measure,
    policy: lettering?.overflowPolicy ?? 'warn',
    padding: style.padding,
  });

  const f = node.frame;
  const pad = style.padding;
  const anchor = style.align === 'center' ? 'middle' : style.align === 'right' ? 'end' : 'start';
  const tx = style.align === 'center' ? f.x + f.width / 2 : style.align === 'right' ? f.x + f.width - pad : f.x + pad;
  const variant = lettering?.kind === 'balloon' ? (lettering.variant ?? 'speech') : null;

  /** @type {string[]} */
  const parts = [];
  if (lettering?.kind === 'balloon') {
    if (variant === 'thought') {
      parts.push(`<ellipse cx="${num(f.x + f.width / 2)}" cy="${num(f.y + f.height / 2)}" rx="${num(f.width / 2)}" ry="${num(f.height / 2)}" fill="#ffffff" stroke="#111111" stroke-width="2"/>`);
      const bump = Math.min(f.width, f.height) * 0.12;
      for (let i = 0; i < 5; i++) {
        const angle = (Math.PI * 2 * i) / 5;
        parts.push(`<circle cx="${num(f.x + f.width / 2 + Math.cos(angle) * f.width * 0.42)}" cy="${num(f.y + f.height / 2 + Math.sin(angle) * f.height * 0.4)}" r="${num(bump)}" fill="#ffffff" stroke="#111111" stroke-width="2"/>`);
      }
    } else if (variant === 'shout') {
      const cx = f.x + f.width / 2;
      const cy = f.y + f.height / 2;
      /** @type {string[]} */
      const points = [];
      for (let i = 0; i < 28; i++) {
        const angle = (Math.PI * i) / 14;
        const rx = (i % 2 === 0 ? f.width / 2 : f.width * 0.42);
        const ry = (i % 2 === 0 ? f.height / 2 : f.height * 0.38);
        points.push(`${num(cx + Math.cos(angle) * rx)},${num(cy + Math.sin(angle) * ry)}`);
      }
      parts.push(`<polygon points="${points.join(' ')}" fill="#fffdf2" stroke="#111111" stroke-width="2"/>`);
    } else {
      const dash = variant === 'whisper' ? ' stroke-dasharray="5 4"' : '';
      parts.push(`<rect x="${num(f.x)}" y="${num(f.y)}" width="${num(f.width)}" height="${num(f.height)}" rx="${num(Math.min(f.width, f.height) * 0.35)}" fill="#ffffff" stroke="#111111" stroke-width="2"${dash}/>`);
    }

    const tailNode = lettering.tailNodeId ? nodes.find((/** @type {any} */ n) => n.id === lettering.tailNodeId) : null;
    if (tailNode) {
      const tail = tailGeometry(f, tailNode);
      if (variant === 'thought') {
        for (let i = 1; i <= 3; i++) {
          const t = i / 4;
          parts.push(`<circle cx="${num(tail.base.x + (tail.tip.x - tail.base.x) * t)}" cy="${num(tail.base.y + (tail.tip.y - tail.base.y) * t)}" r="${num(8 - i * 1.6)}" fill="#ffffff" stroke="#111111" stroke-width="1.5"/>`);
        }
      } else {
        const dx = tail.tip.x - tail.base.x;
        const dy = tail.tip.y - tail.base.y;
        const len = Math.hypot(dx, dy) || 1;
        const px = (-dy / len) * 7;
        const py = (dx / len) * 7;
        parts.push(`<polygon points="${num(tail.base.x + px)},${num(tail.base.y + py)} ${num(tail.base.x - px)},${num(tail.base.y - py)} ${num(tail.tip.x)},${num(tail.tip.y)}" fill="#ffffff" stroke="#111111" stroke-width="2"/>`);
      }
    }
  } else if (lettering?.kind === 'caption') {
    parts.push(`<rect x="${num(f.x)}" y="${num(f.y)}" width="${num(f.width)}" height="${num(f.height)}" fill="#fdf6d8" stroke="#c9bd86" stroke-width="1.5"/>`);
  }

  const startY = f.y + pad + fit.fontSize * 0.95;
  const weight = style.bold ? ' font-weight="bold"' : '';
  const italic = style.italic ? ' font-style="italic"' : '';
  for (let i = 0; i < fit.lines.length; i++) {
    parts.push(
      `<text x="${num(tx)}" y="${num(startY + i * fit.lineHeight)}" ` +
      `font-family="${escapeXml(style.family)}, sans-serif" font-size="${num(fit.fontSize)}"${weight}${italic} ` +
      `fill="${escapeXml(style.color)}" text-anchor="${anchor}">${escapeXml(fit.lines[i])}</text>`,
    );
  }
  if (fit.overflowed) {
    parts.push(`<rect x="${num(f.x)}" y="${num(f.y)}" width="${num(f.width)}" height="${num(f.height)}" fill="none" stroke="#c0392b" stroke-width="1.5" stroke-dasharray="6 4"/>`);
  }
  return parts.join('\n');
}
