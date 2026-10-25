/**
 * Id allocation.
 *
 * Two kinds of id, deliberately different:
 *
 *  - **Comic ids are typed strings** (`page_7`), allocated from a counter persisted in the
 *    manifest, so a deleted entity's id is never reused. Never random, never an array index.
 *  - **Document node ids are numbers**, because the visual document round-trips through the
 *    reference format unchanged and that format numbers its nodes.
 *
 * @module shared/domain/ids
 */

/** @typedef {string} EntityId */

const ID_PATTERN = /^([a-z]+)_([0-9]+)$/;

/** The prefix of every comic entity id. Ordered for stable error messages. */
export const PREFIXES = /** @type {const} */ ([
  'project', 'chapter', 'scene', 'page', 'panel', 'dialogue', 'lettering',
  'character', 'location', 'style', 'asset', 'job', 'proposal', 'decision', 'ref',
]);

/**
 * @param {string} prefix
 * @param {number} n
 * @returns {EntityId}
 */
function format(prefix, n) {
  if (!PREFIXES.includes(/** @type {any} */ (prefix))) {
    throw new Error(`unknown id prefix: ${prefix}`);
  }
  if (!Number.isInteger(n) || n < 1) {
    throw new Error(`id counter must be a positive integer, got ${n}`);
  }
  return `${prefix}_${n}`;
}

/**
 * @param {EntityId} id
 * @returns {{ prefix: string, n: number } | null}
 */
export function parseId(id) {
  if (typeof id !== 'string') return null;
  const m = ID_PATTERN.exec(id);
  if (!m) return null;
  const prefix = m[1];
  const n = Number(m[2]);
  if (!PREFIXES.includes(/** @type {any} */ (prefix)) || !Number.isInteger(n) || n < 1) return null;
  return { prefix, n };
}

/**
 * @param {string} prefix
 * @param {EntityId} id
 * @returns {boolean}
 */
export function isIdOfKind(prefix, id) {
  const parsed = parseId(id);
  return parsed !== null && parsed.prefix === prefix;
}

/**
 * A counter per prefix, persisted in the manifest. `next` never returns a value it has
 * returned before, even after the entity is deleted — that is the whole point.
 */
export class IdCounter {
  /** @param {Record<string, number>} [counters] highest issued number per prefix */
  constructor(counters = {}) {
    /** @type {Record<string, number>} */
    this.counters = { ...counters };
  }

  /**
   * @param {string} prefix
   * @returns {EntityId}
   */
  next(prefix) {
    const current = this.counters[prefix] ?? 0;
    const n = current + 1;
    this.counters[prefix] = n;
    return format(prefix, n);
  }

  /**
   * Raise the counter for `prefix` past `id`, so a re-opened bundle cannot reissue it.
   * Used by migrations and by `retire`.
   * @param {string} prefix
   * @param {EntityId} id
   */
  retire(prefix, id) {
    const parsed = parseId(id);
    if (!parsed) throw new Error(`cannot retire a malformed id: ${id}`);
    if (parsed.prefix !== prefix) {
      throw new Error(`cannot retire ${id} as a ${prefix}: the prefix does not match`);
    }
    if (parsed.n > (this.counters[prefix] ?? 0)) this.counters[prefix] = parsed.n;
  }

  /** @returns {Record<string, number>} */
  toJSON() {
    return { ...this.counters };
  }
}
