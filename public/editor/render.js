/**
 * The canvas renderer — the real one.
 *
 * Distinct from `server/export/png.js`, which produces a deterministic raster for tests and
 * thumbnails and says plainly that it approximates glyphs. Here the browser's text engine
 * draws real letterforms, and the two share the **same domain**: geometry, wrapping and
 * overflow policy all come from `shared/domain`, so the preview and the export cannot
 * disagree about where a line breaks.
 *
 * @module public/editor/render
 */

import { artboardNodes } from '../../shared/domain/document.js';
import { fitText } from '../../shared/domain/lettering.js';

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {any} bundle
 * @param {any} page
 * @param {import('../../shared/domain/lettering.js').MeasureText} measure
 * @param {{ scale: number, selectedId: number | null, showHandles?: boolean }} options
 */
export function drawPage(ctx, bundle, page, measure, options) {
  const { scale, selectedId } = options;
  const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  if (!artboard) return;

  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  ctx.scale(scale, scale);

  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, artboard.size.width, artboard.size.height);

  const nodes = artboardNodes(bundle.document, artboard.id);
  /** @type {Map<number, any>} */
  const letteringByNode = new Map();
  for (const l of Object.values(bundle.project.lettering ?? {})) {
    const e = /** @type {any} */ (l);
    letteringByNode.set(e.nodeId, e);
  }
  /** @type {Map<string, any>} */
  const assetById = new Map();
  for (const a of Object.values(bundle.project.assets ?? {})) {
    const e = /** @type {any} */ (a);
    assetById.set(e.id, e);
  }
  /** @type {Map<number, any>} */
  const panelByFrame = new Map();
  for (const panel of Object.values(bundle.project.panels ?? {})) {
    const p = /** @type {any} */ (panel);
    panelByFrame.set(p.frameNodeId, p);
  }

  // Panel frames first, then artwork inside them, then lettering on top: the order a reader
  // expects, and a stable order so the canvas does not flicker as nodes are added.
  for (const node of nodes.filter((n) => n.kind === 'frame' && n.name !== 'Balloon Tail')) {
    const f = node.frame;
    ctx.fillStyle = node.fill ?? '#ffffff';
    ctx.fillRect(f.x, f.y, f.width, f.height);
    if (node.stroke) {
      ctx.strokeStyle = node.stroke;
      ctx.lineWidth = node.strokeWidth ?? 1;
      ctx.strokeRect(f.x + (node.strokeWidth ?? 1) / 2, f.y + (node.strokeWidth ?? 1) / 2,
        f.width - (node.strokeWidth ?? 1), f.height - (node.strokeWidth ?? 1));
    }
    const panel = panelByFrame.get(node.id);
    if (panel?.locked) {
      // Locking is persisted model state, so the canvas must show it rather than only the
      // inspector: a user who cannot move a panel should be able to see why.
      ctx.save();
      ctx.strokeStyle = '#8a4b0f';
      ctx.lineWidth = 1.5;
      ctx.setLineDash([8, 6]);
      ctx.strokeRect(f.x + 2, f.y + 2, f.width - 4, f.height - 4);
      ctx.setLineDash([]);
      ctx.fillStyle = '#8a4b0f';
      ctx.font = '10px sans-serif';
      ctx.fillText('LOCKED', f.x + 8, f.y + 16);
      ctx.restore();
    }
  }

  for (const node of nodes.filter((n) => n.kind === 'image')) {
    drawImage(ctx, node, assetById);
  }

  for (const node of nodes.filter((n) => n.kind === 'text')) {
    drawLettering(ctx, bundle, node, measure, letteringByNode.get(node.id), nodes);
  }

  if (options.showHandles !== false && selectedId !== null) {
    const node = bundle.document.nodes[String(selectedId)];
    if (node) drawHandles(ctx, node.frame, scale);
  }

  ctx.restore();
}

/**
 * Bitmaps loaded from a data URL, keyed by asset id. The cache lives here rather than on the
 * asset record: an `Image` is not JSON, and putting one in the bundle would break saving.
 * @type {Map<string, HTMLImageElement>}
 */
const imageCache = new Map();
/** @type {Set<string>} */
const imageLoading = new Set();

/**
 * @param {any} asset
 * @returns {HTMLImageElement | null}
 */
function imageFor(asset) {
  if (!asset || typeof asset.data !== 'string' || !asset.data.startsWith('data:image')) return null;
  // Content hash, not per-project asset id: every project has an `asset_1`, and a cache keyed
  // by id would show the previous comic's art after New/Open.
  const key = asset.contentHash ?? asset.id;
  const hit = imageCache.get(key);
  if (hit) return hit;
  if (typeof Image === 'undefined' || imageLoading.has(key)) return null;
  imageLoading.add(key);
  const image = new Image();
  image.onload = () => {
    imageCache.set(key, image);
    imageLoading.delete(key);
    // Tell the application to redraw; the renderer itself never owns application state.
    try { window.dispatchEvent(new Event('comiccraft:asset-loaded')); } catch { /* no window in a worker */ }
  };
  image.onerror = () => imageLoading.delete(key);
  image.src = asset.data;
  return null;
}

/**
 * Drop decoded images when a different project is adopted. The cache is intentionally global
 * (so a redraw does not decode the same artwork twice) but a project switch must start clean.
 */
export function clearImageCache() {
  imageCache.clear();
  imageLoading.clear();
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {any} node
 * @param {Map<string, any>} assetById
 */
function drawImage(ctx, node, assetById) {
  const f = node.frame;
  ctx.save();
  if (node.clip) {
    const c = node.clip;
    ctx.beginPath();
    ctx.rect(c.x, c.y, c.width, c.height);
    ctx.clip();
  } else if (node.clipToFrame) {
    ctx.beginPath();
    ctx.rect(f.x, f.y, f.width, f.height);
    ctx.clip();
  }

  const asset = assetById.get(node.assetId);
  const image = asset?.__image ?? imageFor(asset);
  if (image) {
    drawFitted(ctx, image, f, node.crop, node.fit);
  } else {
    // A missing bitmap is drawn as a marked placeholder, never as empty space: a panel that
    // looks blank is indistinguishable from a panel that failed.
    ctx.fillStyle = '#e9eef5';
    ctx.fillRect(f.x, f.y, f.width, f.height);
    ctx.strokeStyle = '#b9c6d6';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(f.x, f.y);
    ctx.lineTo(f.x + f.width, f.y + f.height);
    ctx.moveTo(f.x + f.width, f.y);
    ctx.lineTo(f.x, f.y + f.height);
    ctx.stroke();
    ctx.fillStyle = '#5b6b7f';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(asset?.name ?? `missing ${node.assetId}`, f.x + f.width / 2, f.y + f.height / 2);
    ctx.textAlign = 'start';
    ctx.textBaseline = 'alphabetic';
  }
  ctx.restore();
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {CanvasImageSource & { width: number, height: number }} image
 * @param {import('../../shared/domain/geom.js').Rect} frame
 * @param {{ x: number, y: number, width: number, height: number } | undefined} crop
 * @param {'cover'|'contain'} [fit]
 */
function drawFitted(ctx, image, frame, crop, fit = 'cover') {
  const c = crop ?? { x: 0, y: 0, width: 1, height: 1 };
  const sw = image.width * c.width;
  const sh = image.height * c.height;
  const sx = image.width * c.x;
  const sy = image.height * c.y;

  if (fit === 'contain') {
    const scale = Math.min(frame.width / sw, frame.height / sh);
    const w = sw * scale, h = sh * scale;
    ctx.drawImage(image, sx, sy, sw, sh, frame.x + (frame.width - w) / 2, frame.y + (frame.height - h) / 2, w, h);
    return;
  }
  const scale = Math.max(frame.width / sw, frame.height / sh);
  const w = sw * scale, h = sh * scale;
  ctx.drawImage(image, sx, sy, sw, sh, frame.x + (frame.width - w) / 2, frame.y + (frame.height - h) / 2, w, h);
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {any} bundle
 * @param {any} node
 * @param {import('../../shared/domain/lettering.js').MeasureText} measure
 * @param {any} lettering
 * @param {any[]} nodes
 */
function drawLettering(ctx, bundle, node, measure, lettering, nodes) {
  const f = node.frame;
  const defaults = bundle.project.defaultLetteringStyle;
  const style = {
    family: lettering?.fontFamily ?? defaults.family,
    sizePt: lettering?.fontSize ?? node.fontSize ?? defaults.sizePt,
    align: lettering?.align ?? node.align ?? defaults.align ?? 'left',
    padding: lettering?.padding ?? defaults.padding ?? 8,
    bold: lettering?.bold ?? false,
    italic: lettering?.italic ?? false,
  };
  const variant = lettering?.kind === 'balloon' ? (lettering.variant ?? 'speech') : null;

  if (lettering?.kind === 'balloon') {
    if (variant === 'thought') {
      ctx.fillStyle = '#ffffff';
      ctx.strokeStyle = '#111111';
      ctx.lineWidth = 2;
      const bump = Math.min(f.width, f.height) * 0.12;
      ctx.beginPath();
      ctx.ellipse(f.x + f.width / 2, f.y + f.height / 2, f.width / 2, f.height / 2, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      for (let i = 0; i < 5; i++) {
        const angle = (Math.PI * 2 * i) / 5;
        ctx.beginPath();
        ctx.arc(f.x + f.width / 2 + Math.cos(angle) * f.width * 0.42, f.y + f.height / 2 + Math.sin(angle) * f.height * 0.4, bump, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
      }
    } else if (variant === 'shout') {
      ctx.beginPath();
      const spikes = 14;
      const cx = f.x + f.width / 2, cy = f.y + f.height / 2;
      for (let i = 0; i < spikes * 2; i++) {
        const angle = (Math.PI * i) / spikes;
        const rx = i % 2 === 0 ? f.width / 2 : f.width * 0.42;
        const ry = i % 2 === 0 ? f.height / 2 : f.height * 0.38;
        const x = cx + Math.cos(angle) * rx;
        const y = cy + Math.sin(angle) * ry;
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      }
      ctx.closePath();
      ctx.fillStyle = '#fffdf2';
      ctx.fill();
      ctx.strokeStyle = '#111111';
      ctx.lineWidth = 2;
      ctx.stroke();
    } else {
      roundRect(ctx, f, Math.min(f.width, f.height) * 0.35);
      ctx.fillStyle = '#ffffff';
      ctx.fill();
      ctx.strokeStyle = '#111111';
      ctx.lineWidth = 2;
      ctx.setLineDash(variant === 'whisper' ? [5, 4] : []);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    const tailNode = lettering.tailNodeId ? nodes.find((n) => n.id === lettering.tailNodeId) : null;
    if (tailNode) {
      const tip = { x: tailNode.frame.x + tailNode.frame.width / 2, y: tailNode.frame.y + tailNode.frame.height / 2 };
      const centre = { x: f.x + f.width / 2, y: f.y + f.height / 2 };
      const dx = tip.x - centre.x, dy = tip.y - centre.y;
      const len = Math.hypot(dx, dy) || 1;
      const ux = dx / len, uy = dy / len;
      const edge = Math.min(
        Math.abs(ux) > 1e-9 ? f.width / 2 / Math.abs(ux) : Infinity,
        Math.abs(uy) > 1e-9 ? f.height / 2 / Math.abs(uy) : Infinity,
      );
      const base = { x: centre.x + ux * (Number.isFinite(edge) ? edge : 0) * 0.9, y: centre.y + uy * (Number.isFinite(edge) ? edge : 0) * 0.9 };
      if (variant === 'thought') {
        // A thought tail is a chain of bubbles, not a pointed triangle.
        const steps = 3;
        for (let i = 1; i <= steps; i++) {
          const t = i / (steps + 1);
          const radius = 8 - i * 1.6;
          ctx.beginPath();
          ctx.arc(base.x + (tip.x - base.x) * t, base.y + (tip.y - base.y) * t, radius, 0, Math.PI * 2);
          ctx.fillStyle = '#ffffff';
          ctx.fill();
          ctx.strokeStyle = '#111111';
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }
      } else {
        const px = -uy, py = ux;
        ctx.beginPath();
        ctx.moveTo(base.x + px * 7, base.y + py * 7);
        ctx.lineTo(base.x - px * 7, base.y - py * 7);
        ctx.lineTo(tip.x, tip.y);
        ctx.closePath();
        ctx.fillStyle = '#ffffff';
        ctx.fill();
        ctx.strokeStyle = '#111111';
        ctx.lineWidth = 2;
        ctx.stroke();
      }
    }
  } else if (lettering?.kind === 'caption') {
    ctx.fillStyle = '#fdf6d8';
    ctx.fillRect(f.x, f.y, f.width, f.height);
    ctx.strokeStyle = '#c9bd86';
    ctx.lineWidth = 1.5;
    ctx.strokeRect(f.x, f.y, f.width, f.height);
  }

  // The same fitText the export uses, with the same measured widths: a line breaks in the
  // preview exactly where it breaks in the file.
  const fit = fitText({
    text: node.text ?? '',
    frame: f,
    style,
    measure,
    policy: lettering?.overflowPolicy ?? 'warn',
    padding: style.padding,
  });

  ctx.fillStyle = lettering?.color ?? node.color ?? '#111111';
  const weight = style.bold ? 'bold ' : '';
  const italic = style.italic ? 'italic ' : '';
  ctx.font = `${italic}${weight}${fit.fontSize}px ${style.family}, sans-serif`;
  ctx.textBaseline = 'alphabetic';

  for (let i = 0; i < fit.lines.length; i++) {
    const y = f.y + style.padding + fit.fontSize * 0.95 + i * fit.lineHeight;
    if (style.align === 'center') {
      ctx.textAlign = 'center';
      ctx.fillText(fit.lines[i], f.x + f.width / 2, y);
    } else if (style.align === 'right') {
      ctx.textAlign = 'end';
      ctx.fillText(fit.lines[i], f.x + f.width - style.padding, y);
    } else {
      ctx.textAlign = 'start';
      ctx.fillText(fit.lines[i], f.x + style.padding, y);
    }
  }
  ctx.textAlign = 'start';

  if (fit.overflowed) {
    // Drawn, not hidden. A clipped balloon looks like a font bug; a marked one looks like a
    // problem the user can act on.
    ctx.save();
    ctx.strokeStyle = '#c0392b';
    ctx.lineWidth = 1.5;
    ctx.setLineDash([6, 4]);
    ctx.strokeRect(f.x, f.y, f.width, f.height);
    ctx.restore();
  }
}

/** @param {CanvasRenderingContext2D} ctx @param {any} r @param {number} radius */
function roundRect(ctx, r, radius) {
  const rad = Math.max(0, Math.min(radius, r.width / 2, r.height / 2));
  ctx.beginPath();
  ctx.moveTo(r.x + rad, r.y);
  ctx.lineTo(r.x + r.width - rad, r.y);
  ctx.quadraticCurveTo(r.x + r.width, r.y, r.x + r.width, r.y + rad);
  ctx.lineTo(r.x + r.width, r.y + r.height - rad);
  ctx.quadraticCurveTo(r.x + r.width, r.y + r.height, r.x + r.width - rad, r.y + r.height);
  ctx.lineTo(r.x + rad, r.y + r.height);
  ctx.quadraticCurveTo(r.x, r.y + r.height, r.x, r.y + r.height - rad);
  ctx.lineTo(r.x, r.y + rad);
  ctx.quadraticCurveTo(r.x, r.y, r.x + rad, r.y);
  ctx.closePath();
}

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {import('../../shared/domain/geom.js').Rect} frame
 * @param {number} scale
 */
function drawHandles(ctx, frame, scale) {
  const size = 5 / scale;
  ctx.save();
  ctx.strokeStyle = '#2f5d9e';
  ctx.lineWidth = 1 / scale;
  ctx.strokeRect(frame.x, frame.y, frame.width, frame.height);
  ctx.fillStyle = '#ffffff';
  for (const [x, y] of [
    [frame.x, frame.y],
    [frame.x + frame.width, frame.y],
    [frame.x, frame.y + frame.height],
    [frame.x + frame.width, frame.y + frame.height],
  ]) {
    ctx.fillRect(x - size / 2, y - size / 2, size, size);
    ctx.strokeRect(x - size / 2, y - size / 2, size, size);
  }
  ctx.restore();
}
