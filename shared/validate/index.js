/**
 * Bundle validation, re-exported where the plan says it lives.
 *
 * The implementation is in `shared/domain/validate.js` because it is part of the domain —
 * the browser validates a project before saving and the server validates it before writing,
 * using the same code. This module exists so that both addresses resolve to one
 * implementation rather than two.
 *
 * @module shared/validate
 */

export {
  errorsOf,
  validateBundle,
  validateLinks,
  validateReadingOrder,
  warningsOf,
} from '../domain/validate.js';
