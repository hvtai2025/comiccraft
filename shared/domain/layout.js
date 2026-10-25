/**
 * Deterministic page and panel layout. **Computes a plan and returns it** — it never
 * touches the document. Applying the plan is the command layer's job, which is what makes
 * the result one undoable step instead of a dozen.
 *
 * @module shared/domain/layout
 */

import { rect, right, bottom, intersects, intersection, area, isPositive, quantize } from './geom.js';

/**
 * @typedef {{ rows: number, cols: number, gutter: number, margin: number }} GridTemplate
 */

/** Named templates. Rectangular on purpose: frames are ordinary paths, so irregular
 * shapes are an extension, not a prerequisite (plan §8). */
export const TEMPLATES = /** @type {Record<string, GridTemplate>} */ ({
  'single':    { rows: 1, cols: 1, gutter: 0,  margin: 0 },
  'two-up':    { rows: 1, cols: 2, gutter: 24, margin: 0 },
  'three-up':  { rows: 1, cols: 3, gutter: 24, margin: 0 },
  'two-by-two':{ rows: 2, cols: 2, gutter: 24, margin: 0 },
  'three-by-two': { rows: 2, cols: 3, gutter: 20, margin: 0 },
  'five-grid': { rows: 3, cols: 2, gutter: 20, margin: 0 },
  'splash':    { rows: 1, cols: 1, gutter: 0,  margin: 24 },
  'six-grid':  { rows: 2, cols: 3, gutter: 18, margin: 18 },
});

/** @returns {string[]} every template name, in a stable order */
export const templateNames = () => Object.keys(TEMPLATES).sort();

/** @param {string} name @returns {GridTemplate} */
export function template(name) {
  const t = TEMPLATES[name];
  if (!t) throw new Error(`unknown template: ${name}`);
  return t;
}

/**
 * The gutter is the space *between* panels, so a grid of r×c cells has (c-1) vertical
 * gutters; a template with a zero gutter tiles the page exactly.
 *
 * `direction` reorders the **panels**, never their coordinates: a right-to-left page has
 * exactly the same geometry as a left-to-right one, which is why reading order is stored
 * rather than derived (plan §8).
 *
 * @param {{ width: number, height: number }} pageSize
 * @param {GridTemplate} t
 * @param {'ltr'|'rtl'} [direction]
 * @returns {import('./geom.js').Rect[]} frames in reading order
 */
export function gridFrames(pageSize, t, direction = 'ltr') {
  const usableW = pageSize.width - t.margin * 2 - t.gutter * (t.cols - 1);
  const usableH = pageSize.height - t.margin * 2 - t.gutter * (t.rows - 1);
  if (usableW <= 0 || usableH <= 0) {
    throw new Error(
      `template ${t.rows}x${t.cols} with gutter ${t.gutter} does not fit a ` +
      `${pageSize.width}x${pageSize.height} page`
    );
  }
  const cellW = usableW / t.cols;
  const cellH = usableH / t.rows;
  /** @type {import('./geom.js').Rect[]} */
  const frames = [];
  for (let r = 0; r < t.rows; r++) {
    /** @type {import('./geom.js').Rect[]} */
    const row = [];
    for (let c = 0; c < t.cols; c++) {
      row.push(rect(
        quantize(t.margin + c * (cellW + t.gutter)),
        quantize(t.margin + r * (cellH + t.gutter)),
        quantize(cellW),
        quantize(cellH),
      ));
    }
    if (direction === 'rtl') row.reverse();
    frames.push(...row);
  }
  return frames;
}

/**
 * Template warnings that do not depend on the page's existing panels: a gutter narrower than
 * a hairline, a margin that leaves no usable page, or a cell too small to letter.
 * @param {{ width: number, height: number }} pageSize
 * @param {GridTemplate} t
 * @returns {{ code: string, message: string }[]}
 */
export function templateWarnings(pageSize, t) {
  /** @type {{ code: string, message: string }[]} */
  const warnings = [];
  const usableW = pageSize.width - t.margin * 2 - t.gutter * (t.cols - 1);
  const usableH = pageSize.height - t.margin * 2 - t.gutter * (t.rows - 1);
  const cellW = usableW / t.cols;
  const cellH = usableH / t.rows;
  if (t.gutter > 0 && t.gutter < 8) {
    warnings.push({ code: 'gutter-narrow', message: `a ${t.gutter}-unit gutter is narrower than 8 units and may close up in print` });
  }
  if (t.margin < 0) {
    warnings.push({ code: 'margin-negative', message: `a ${t.margin}-unit margin is negative` });
  }
  if (cellW < 24 || cellH < 24) {
    warnings.push({ code: 'panel-too-small', message: `a ${quantize(cellW)}x${quantize(cellH)} cell is too small for lettering` });
  }
  return warnings;
}

/**
 * @typedef {object} OverlapFinding
 * @property {number} a index of the first frame
 * @property {number} b index of the second frame
 * @property {number} overlapArea
 */

/**
 * Panels may legitimately touch — a shared border is a layout choice — so overlap is
 * reported only when the shared area is large enough to hide artwork.
 * @param {import('./geom.js').Rect[]} frames
 * @param {number} [threshold] fraction of the smaller frame's area
 * @returns {OverlapFinding[]}
 */
export function findOverlaps(frames, threshold = 0.01) {
  /** @type {OverlapFinding[]} */
  const out = [];
  for (let i = 0; i < frames.length; i++) {
    for (let j = i + 1; j < frames.length; j++) {
      if (!intersects(frames[i], frames[j])) continue;
      const shared = intersection(frames[i], frames[j]);
      if (!shared) continue;
      const smaller = Math.min(area(frames[i]), area(frames[j]));
      const overlapArea = area(shared);
      if (smaller > 0 && overlapArea / smaller > threshold) {
        out.push({ a: i, b: j, overlapArea: quantize(overlapArea) });
      }
    }
  }
  return out;
}

/**
 * A frame that leaves the page will print cropped, so it is a warning rather than an error:
 * a bleed panel is deliberate.
 * @param {import('./geom.js').Rect[]} frames
 * @param {{ width: number, height: number }} pageSize
 * @returns {number[]} indices of frames reaching outside the page
 */
export function framesOutsidePage(frames, pageSize) {
  const page = rect(0, 0, pageSize.width, pageSize.height);
  const out = [];
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    if (f.x < page.x - 1e-6 || f.y < page.y - 1e-6 ||
        right(f) > right(page) + 1e-6 || bottom(f) > bottom(page) + 1e-6) {
      out.push(i);
    }
  }
  return out;
}

/**
 * Non-positive frames cannot be drawn. Separate from the outside-page check because a
 * zero-size frame is always a bug, while an oversized one may be a bleed.
 * @param {import('./geom.js').Rect[]} frames
 * @returns {number[]}
 */
export const degenerateFrames = (frames) =>
  frames.map((f, i) => (isPositive(f) ? -1 : i)).filter((i) => i !== -1);
