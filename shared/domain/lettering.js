/**
 * Lettering: balloons, captions and sound effects are **one entity with a `kind`** — their
 * difference is presentation, not structure (design §5.2).
 *
 * Text measurement is **injected**, never imported. That is what lets this module run
 * unchanged in the browser (real canvas metrics) and in `node --test` (a deterministic
 * stub), and it is why no font file is needed to test the wrapping rules.
 *
 * @module shared/domain/lettering
 */

/**
 * @typedef {(text: string, style: { family: string, sizePt: number }) => number} MeasureText
 *   Returns the advance width of `text` at the given size, in page units.
 */

/**
 * A deterministic stand-in for real metrics: every character advances by `sizePt * factor`.
 * Tests use this, and the browser never does.
 * @param {number} [factor]
 * @returns {MeasureText}
 */
export const stubMeasure = (factor = 0.5) => (text, style) => text.length * style.sizePt * factor;

/** @typedef {'warn'|'shrink'|'resize-frame'} OverflowPolicy */

/**
 * The six presentations a reader sees, expressed as three structural kinds plus a variant.
 * A thought balloon is still a balloon; only its drawing and tail change. Keeping the
 * structure small is what lets one command and one schema cover all six.
 */
export const VARIANTS = /** @type {const} */ (['speech', 'thought', 'shout', 'whisper']);

/** @param {string} kind @param {string|undefined} variant @returns {'speech'|'thought'|'narration'|'shout'|'whisper'|'sound-effect'} */
export function letteringType(kind, variant) {
  if (kind === 'caption') return 'narration';
  if (kind === 'sound-effect') return 'sound-effect';
  return /** @type {any} */ (VARIANTS.includes(/** @type {any} */ (variant)) ? variant : 'speech');
}

/** @param {string} kind @returns {boolean} */
export const isSpoken = (kind) => kind === 'balloon';

/**
 * @typedef {object} FitResult
 * @property {string[]} lines
 * @property {number} fontSize    the size actually used, after any shrink
 * @property {number} lineHeight
 * @property {number} usedWidth
 * @property {number} usedHeight
 * @property {boolean} overflowed whether the text did not fit at the starting size
 * @property {boolean} shrunk     whether the size was reduced
 */

/**
 * Wrap `text` to `maxWidth`, then apply the overflow policy.
 *
 * Word wrapping is greedy: break at the last space that fits, and break a word that is
 * longer than the line on its own (a URL in a balloon is real, and refusing it would be a
 * worse failure than a mid-word break).
 *
 * @param {object} args
 * @param {string} args.text
 * @param {import('./geom.js').Rect} args.frame
 * @param {{ family: string, sizePt: number }} args.style
 * @param {MeasureText} args.measure
 * @param {OverflowPolicy} [args.policy]
 * @param {number} [args.padding]
 * @param {number} [args.lineHeightRatio]
 * @param {number} [args.minScale] floor for shrink, so text never becomes unreadable
 * @returns {FitResult}
 */
export function fitText({
  text, frame, style, measure,
  policy = 'warn', padding = 8, lineHeightRatio = 1.25, minScale = 0.5,
}) {
  const maxWidth = frame.width - padding * 2;
  const maxHeight = frame.height - padding * 2;
  if (maxWidth <= 0 || maxHeight <= 0) {
    return { lines: [], fontSize: style.sizePt, lineHeight: style.sizePt * lineHeightRatio,
             usedWidth: 0, usedHeight: 0, overflowed: true, shrunk: false };
  }

  const startSize = style.sizePt;
  const atSize = (/** @type {number} */ sizePt) => {
    const effective = { family: style.family, sizePt };
    const lines = wrap(text, maxWidth, effective, measure);
    const lineHeight = sizePt * lineHeightRatio;
    let widest = 0;
    for (const line of lines) widest = Math.max(widest, measure(line, effective));
    return { lines, lineHeight, usedWidth: widest, usedHeight: lines.length * lineHeight };
  };

  let attempt = atSize(startSize);
  let overflowed = attempt.usedHeight > maxHeight + 1e-6 || attempt.usedWidth > maxWidth + 1e-6;

  if (!overflowed) {
    return { ...attempt, fontSize: startSize, overflowed: false, shrunk: false };
  }

  if (policy === 'shrink') {
    const floor = startSize * minScale;
    let size = startSize;
    while (size > floor) {
      size = Math.max(floor, size - 0.5);
      attempt = atSize(size);
      const fits = attempt.usedHeight <= maxHeight + 1e-6 && attempt.usedWidth <= maxWidth + 1e-6;
      if (fits) {
        return { ...attempt, fontSize: Math.round(size * 100) / 100, overflowed: false, shrunk: size < startSize };
      }
      if (size === floor) break;
    }
    return { ...attempt, fontSize: size, overflowed: true, shrunk: size < startSize };
  }

  if (policy === 'resize-frame') {
    // The frame grows to fit; the caller applies it. Reported through `usedHeight`.
    return { ...attempt, fontSize: startSize, overflowed: true, shrunk: false };
  }

  // 'warn': keep the text at its size and let the caller report the overflow.
  return { ...attempt, fontSize: startSize, overflowed: true, shrunk: false };
}

/**
 * @param {string} text
 * @param {number} maxWidth
 * @param {{ family: string, sizePt: number }} style
 * @param {MeasureText} measure
 * @returns {string[]}
 */
export function wrap(text, maxWidth, style, measure) {
  const paragraphs = String(text ?? '').split('\n');
  /** @type {string[]} */
  const lines = [];
  for (const paragraph of paragraphs) {
    const words = paragraph.split(/\s+/).filter((w) => w.length > 0);
    if (words.length === 0) { lines.push(''); continue; }
    let current = '';
    for (const word of words) {
      const candidate = current === '' ? word : `${current} ${word}`;
      if (measure(candidate, style) <= maxWidth) { current = candidate; continue; }
      if (current !== '') lines.push(current);
      if (measure(word, style) <= maxWidth) { current = word; continue; }
      // A single word wider than the line: break it rather than overflow or drop it.
      let rest = word;
      while (measure(rest, style) > maxWidth && rest.length > 1) {
        let cut = rest.length - 1;
        while (cut > 1 && measure(rest.slice(0, cut), style) > maxWidth) cut--;
        lines.push(rest.slice(0, cut));
        rest = rest.slice(cut);
      }
      current = rest;
    }
    lines.push(current);
  }
  return lines;
}

/**
 * The frame needed to hold the text without shrinking it. Used by `resize-frame`.
 * @param {FitResult} result
 * @param {number} [padding]
 * @returns {{ width: number, height: number }}
 */
export const requiredFrame = (result, padding = 8) => ({
  width: Math.ceil(result.usedWidth + padding * 2),
  height: Math.ceil(result.usedHeight + padding * 2),
});

/**
 * The two points a renderer needs to draw a tail: the base on the balloon's edge and the tip
 * where the speaker is. The base is found by projecting outwards from the balloon centre, so
 * a thought, shout or speech balloon all attach at their own edge.
 * @param {import('./geom.js').Rect} balloonFrame
 * @param {{ frame: import('./geom.js').Rect }} tailNode
 * @returns {{ base: { x: number, y: number }, tip: { x: number, y: number } }}
 */
export function tailGeometry(balloonFrame, tailNode) {
  const tip = {
    x: tailNode.frame.x + tailNode.frame.width / 2,
    y: tailNode.frame.y + tailNode.frame.height / 2,
  };
  const centre = {
    x: balloonFrame.x + balloonFrame.width / 2,
    y: balloonFrame.y + balloonFrame.height / 2,
  };
  const dx = tip.x - centre.x;
  const dy = tip.y - centre.y;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len;
  const uy = dy / len;
  const edge = Math.min(
    Math.abs(ux) > 1e-9 ? balloonFrame.width / 2 / Math.abs(ux) : Infinity,
    Math.abs(uy) > 1e-9 ? balloonFrame.height / 2 / Math.abs(uy) : Infinity,
  );
  const distance = Number.isFinite(edge) ? edge : 0;
  return { base: { x: centre.x + ux * distance, y: centre.y + uy * distance }, tip };
}

/**
 * Move a balloon, keeping its tail attached to where it pointed.
 *
 * A tail is drawn as a triangle from the balloon's edge to a tip. Moving the balloon moves
 * the base with it and keeps the tip pointing the same way, so the tail *follows* rather
 * than detaching. That is the criterion: moving a balloon preserves or updates the tail
 * without detaching it (plan §8). Length is preserved, so it neither collapses nor stretches.
 *
 * @param {{ x: number, y: number }} dxdy
 * @param {import('./geom.js').Rect} balloonFrameBefore
 * @param {{ base: { x: number, y: number }, tip: { x: number, y: number } }} tail
 * @returns {{ base: { x: number, y: number }, tip: { x: number, y: number } }}
 */
export function moveTail(dxdy, balloonFrameBefore, tail) {
  const cx = balloonFrameBefore.x + balloonFrameBefore.width / 2;
  const cy = balloonFrameBefore.y + balloonFrameBefore.height / 2;
  const mx = cx + dxdy.x;
  const my = cy + dxdy.y;

  // Where the tip points, relative to the balloon, in the balloon's own frame of reference.
  const vx = tail.tip.x - tail.base.x;
  const vy = tail.tip.y - tail.base.y;
  const len = Math.hypot(vx, vy);

  // The base sits on the balloon's edge along that direction, so it stays attached.
  const halfW = balloonFrameBefore.width / 2;
  const halfH = balloonFrameBefore.height / 2;
  let ux = 0, uy = 0;
  if (len > 1e-9) { ux = vx / len; uy = vy / len; }
  const scale = Math.min(
    Math.abs(ux) > 1e-9 ? halfW / Math.abs(ux) : Infinity,
    Math.abs(uy) > 1e-9 ? halfH / Math.abs(uy) : Infinity,
  );
  const edge = Number.isFinite(scale) ? scale : 0;

  return {
    base: { x: mx + ux * edge, y: my + uy * edge },
    tip: { x: mx + ux * (edge + len), y: my + uy * (edge + len) },
  };
}
