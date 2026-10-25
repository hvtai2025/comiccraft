/**
 * Real text metrics, from the canvas the browser already has.
 *
 * This is the **other half of the injected-measurement contract**: the domain's lettering
 * rules call a function it was given, and here that function is the browser's own text
 * measurement. In `node --test` the same rules run against a deterministic stub, which is why
 * the wrapping and overflow policy are testable without a font file.
 *
 * @module public/editor/measure
 */

/** @type {Record<string, number>} */
const cache = new Map();

/**
 * @param {CanvasRenderingContext2D} ctx
 * @param {number} [resolution] font size used for measuring, in pixels
 * @returns {import('../../shared/domain/lettering.js').MeasureText}
 */
export function createMeasure(ctx, resolution = 100) {
  return (text, style) => {
    if (!text) return 0;
    const key = `${style.family}|${resolution}|${text}`;
    const hit = cache.get(key);
    if (hit !== undefined) return (hit / resolution) * style.sizePt;

    // Measure at a fixed size and scale, so the cache does not grow with every font size a
    // user drags through, and so a measurement is stable as the size changes.
    ctx.save();
    ctx.font = `${resolution}px ${style.family}, sans-serif`;
    const width = ctx.measureText(text).width;
    ctx.restore();
    cache.set(key, width);
    if (cache.size > 4000) cache.clear();
    return (width / resolution) * style.sizePt;
  };
}
