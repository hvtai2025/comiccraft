/**
 * Geometry primitives. Pure: no DOM, no Node, no globals beyond Math.
 * @module shared/domain/geom
 */

/**
 * @typedef {{ x: number, y: number }} Point
 * @typedef {{ x: number, y: number, width: number, height: number }} Rect
 */

/** @param {number} x @param {number} y @returns {Point} */
export const point = (x, y) => ({ x, y });

/** @param {number} x @param {number} y @param {number} width @param {number} height @returns {Rect} */
export const rect = (x, y, width, height) => ({ x, y, width, height });

/** @param {Rect} r @returns {number} */
export const right = (r) => r.x + r.width;

/** @param {Rect} r @returns {number} */
export const bottom = (r) => r.y + r.height;

/** @param {Rect} r @returns {Point} */
export const center = (r) => point(r.x + r.width / 2, r.y + r.height / 2);

/** @param {Rect} r @returns {number} */
export const area = (r) => r.width * r.height;

/**
 * @param {Rect} r
 * @returns {boolean} true when width and height are positive and finite
 */
export const isPositive = (r) =>
  Number.isFinite(r.x) && Number.isFinite(r.y) &&
  Number.isFinite(r.width) && Number.isFinite(r.height) &&
  r.width > 0 && r.height > 0;

/**
 * @param {Rect} a @param {Rect} b
 * @returns {boolean} true when the two rects share any interior area
 */
export const intersects = (a, b) =>
  a.x < right(b) && b.x < right(a) && a.y < bottom(b) && b.y < bottom(a);

/**
 * @param {Rect} a @param {Rect} b
 * @returns {Rect | null} the overlap, or null when they do not intersect
 */
export function intersection(a, b) {
  if (!intersects(a, b)) return null;
  const x = Math.max(a.x, b.x);
  const y = Math.max(a.y, b.y);
  return rect(x, y, Math.min(right(a), right(b)) - x, Math.min(bottom(a), bottom(b)) - y);
}

/**
 * @param {Rect} a @param {Rect} b
 * @returns {Rect} the smallest rect containing both
 */
export const union = (a, b) => {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return rect(x, y, Math.max(right(a), right(b)) - x, Math.max(bottom(a), bottom(b)) - y);
};

/** @param {Rect} r @param {number} dx @param {number} dy @returns {Rect} */
export const translate = (r, dx, dy) => rect(r.x + dx, r.y + dy, r.width, r.height);

/** @param {Rect} outer @param {Rect} inner @returns {boolean} */
export const contains = (outer, inner) =>
  inner.x >= outer.x && inner.y >= outer.y &&
  right(inner) <= right(outer) && bottom(inner) <= bottom(outer);

/** @param {Rect} r @param {number} amount @returns {Rect} negative grows inward */
export const inset = (r, amount) =>
  rect(r.x + amount, r.y + amount, r.width - amount * 2, r.height - amount * 2);

/**
 * Round to a fixed number of decimals, so a value read from JSON and the same value
 * computed in memory compare equal. Every geometric comparison in the domain goes
 * through this; comparing raw floats is how a round trip appears to change a document.
 * @param {number} n @param {number} [decimals]
 * @returns {number}
 */
export const quantize = (n, decimals = 6) => {
  const f = 10 ** decimals;
  return Math.round(n * f) / f;
};

/** @param {Rect} r @returns {Rect} */
export const quantizeRect = (r) =>
  rect(quantize(r.x), quantize(r.y), quantize(r.width), quantize(r.height));
