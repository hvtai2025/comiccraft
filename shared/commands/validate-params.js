/**
 * Parameter validation against a command's declared schema.
 *
 * This runs **before** the handler, which is what makes "a malformed call cannot half-apply
 * an edit" true rather than aspirational. It implements the subset of JSON Schema the
 * commands use — `type`, `properties`, `required`, `enum`, `minimum`, `maximum`,
 * `minLength`, `maxLength`, `items`, `minItems`, `maxItems`, `additionalProperties` — and
 * **refuses a schema it does not understand** rather than silently passing everything.
 *
 * @module shared/commands/validate-params
 */

import { fail } from './registry.js';

/**
 * Keywords this checker knows how to enforce, plus the purely declarative ones.
 *
 * `$schema`, `title`, `description` and `default` say something *about* a schema rather than
 * constraining an instance, so accepting them is not the same as ignoring a rule. Everything
 * else is either enforced below or refused by `unsupportedKeyword` — because a schema that is
 * silently not enforced reports success on a value it never checked.
 */
const KNOWN_KEYWORDS = new Set([
  'type', 'properties', 'required', 'enum', 'const', 'minimum', 'maximum',
  'exclusiveMinimum', 'exclusiveMaximum',
  'minLength', 'maxLength', 'items', 'minItems', 'maxItems',
  'additionalProperties',
  '$schema', 'title', 'description', 'default', 'examples',
]);

/**
 * @param {object} schema
 * @returns {string | null} the name of an unsupported keyword, or null
 */
export function unsupportedKeyword(schema) {
  for (const key of Object.keys(schema)) {
    if (!KNOWN_KEYWORDS.has(key)) return key;
  }
  return null;
}

/**
 * @param {object} schema
 * @param {unknown} value
 * @param {string} [path]
 * @returns {import('./registry.js').Failure | null} null when the value is valid
 */
export function check(schema, value, path = '') {
  const s = /** @type {any} */ (schema);
  const at = path === '' ? 'params' : path;

  const unsupported = unsupportedKeyword(s);
  if (unsupported !== null) {
    // A schema we cannot enforce is worse than no schema: it would report success on a
    // check it never performed. Refuse loudly at validation time instead.
    return fail('internal', `the schema for ${at} uses an unsupported keyword: ${unsupported}`);
  }

  if (s.type) {
    const types = Array.isArray(s.type) ? s.type : [s.type];
    const actual = jsonType(value);
    if (!types.includes(actual) && !(types.includes('number') && actual === 'integer')) {
      return fail('bad-params', `${at} must be ${types.join(' or ')}, got ${actual}`, at);
    }
    // `integer` must actually be one; JSON has no separate integer type.
    if (types.includes('integer') && actual === 'number' && !Number.isInteger(value)) {
      return fail('bad-params', `${at} must be an integer, got ${value}`, at);
    }
  }

  if (s.enum && !s.enum.includes(value)) {
    return fail('bad-params', `${at} must be one of ${s.enum.join(', ')}, got ${JSON.stringify(value)}`, at);
  }

  if (s.const !== undefined && JSON.stringify(value) !== JSON.stringify(s.const)) {
    return fail('bad-params', `${at} must be ${JSON.stringify(s.const)}, got ${JSON.stringify(value)}`, at);
  }

  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return fail('bad-params', `${at} must be a finite number`, at);
    if (s.minimum !== undefined && value < s.minimum) {
      return fail('bad-params', `${at} must be at least ${s.minimum}, got ${value}`, at);
    }
    if (s.maximum !== undefined && value > s.maximum) {
      return fail('bad-params', `${at} must be at most ${s.maximum}, got ${value}`, at);
    }
    if (s.exclusiveMinimum !== undefined && value <= s.exclusiveMinimum) {
      return fail('bad-params', `${at} must be greater than ${s.exclusiveMinimum}, got ${value}`, at);
    }
    if (s.exclusiveMaximum !== undefined && value >= s.exclusiveMaximum) {
      return fail('bad-params', `${at} must be less than ${s.exclusiveMaximum}, got ${value}`, at);
    }
  }

  if (typeof value === 'string') {
    if (s.minLength !== undefined && value.length < s.minLength) {
      return fail('bad-params', `${at} must be at least ${s.minLength} characters`, at);
    }
    if (s.maxLength !== undefined && value.length > s.maxLength) {
      return fail('bad-params', `${at} must be at most ${s.maxLength} characters`, at);
    }
  }

  if (Array.isArray(value)) {
    if (s.minItems !== undefined && value.length < s.minItems) {
      return fail('bad-params', `${at} must have at least ${s.minItems} item(s)`, at);
    }
    if (s.maxItems !== undefined && value.length > s.maxItems) {
      return fail('bad-params', `${at} must have at most ${s.maxItems} item(s)`, at);
    }
    if (s.items) {
      for (let i = 0; i < value.length; i++) {
        const problem = check(s.items, value[i], `${at}[${i}]`);
        if (problem) return problem;
      }
    }
  }

  if (s.type === 'object' || s.properties || s.required) {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) {
      return fail('bad-params', `${at} must be an object`, at);
    }
    const obj = /** @type {Record<string, unknown>} */ (value);
    for (const name of s.required ?? []) {
      if (!(name in obj) || obj[name] === undefined) {
        return fail('bad-params', `${at}.${name} is required`, `${at}.${name}`);
      }
    }
    for (const [name, sub] of Object.entries(s.properties ?? {})) {
      if (name in obj && obj[name] !== undefined) {
        const problem = check(/** @type {object} */ (sub), obj[name], `${at}.${name}`);
        if (problem) return problem;
      }
    }
  }

  return null;
}

/**
 * @param {unknown} value
 * @returns {string} the JSON type name, or the JavaScript one for a value JSON cannot carry
 */
function jsonType(value) {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  if (typeof value === 'number') return Number.isInteger(value) ? 'integer' : 'number';
  // A symbol or a function is not JSON at all; naming it as itself makes the failure message
  // say what it actually is rather than claiming 'object'.
  return typeof value;
}
