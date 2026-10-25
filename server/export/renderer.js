/**
 * The renderer service: one object the export commands call, so that the renderers
 * themselves stay pure functions of a bundle and a page.
 * @module server/export/renderer
 */

import { pageToSvg } from './svg.js';
import { pageToPng } from './png.js';
import { bookToPdf } from './pdf.js';

/**
 * @param {{ measure?: import('../../shared/domain/lettering.js').MeasureText }} [options]
 * @returns {any}
 */
export function createRenderer(options = {}) {
  return {
    /** @param {any} bundle @param {any} page @returns {string} */
    pageToSvg: (bundle, page) => pageToSvg(bundle, page, options),
    /** @param {any} bundle @param {any} page @param {number} scale @returns {Buffer} */
    pageToPng: (bundle, page, scale) => pageToPng(bundle, page, scale, options),
    /** @param {any} bundle @param {any[]} pages @param {{ preset?: string }} [callOptions] @returns {Buffer} */
    bookToPdf: (bundle, pages, callOptions = {}) => bookToPdf(bundle, pages, { ...options, ...callOptions }),
  };
}
