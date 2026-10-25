/**
 * Publishing presets and the export gate.
 *
 * Screen, print and proofing are not the same page: a print export has bleed and marks, and a
 * proofing export is a review artefact at reduced resolution. The gate runs **before** any
 * bytes are written, so a missing asset or an unsupported page size is an actionable error
 * rather than a silently wrong file.
 *
 * The fontless contract still holds: a host with no font service exports with the standard
 * face and says so. A host that *has* fonts but not the project's face is a genuine missing
 * font and fails a strict (print) export.
 *
 * @module shared/domain/publish
 */

import { findArtboard } from './document.js';
import { validateBundle, errorsOf } from './validate.js';
import { fitText, stubMeasure } from './lettering.js';
import { missingFont, fontEnvironment } from './fonts.js';

/**
 * @typedef {object} ExportPreset
 * @property {string} id
 * @property {string} label
 * @property {number} bleed      page units of bleed on each edge
 * @property {number} markMargin page units reserved for printer's marks
 * @property {boolean} marks
 * @property {boolean} strictAssets
 * @property {boolean} strictFonts
 * @property {number} imageScale
 */

/** @type {Record<string, ExportPreset>} */
export const PRESETS = {
  screen: {
    id: 'screen',
    label: 'Screen',
    bleed: 0,
    markMargin: 0,
    marks: false,
    strictAssets: false,
    strictFonts: false,
    imageScale: 1,
  },
  print: {
    id: 'print',
    label: 'Print (bleed and marks)',
    // 3 mm at 300 ppi is 35.43 units; marks need another 5 mm.
    bleed: 36,
    markMargin: 60,
    marks: true,
    strictAssets: true,
    strictFonts: true,
    imageScale: 1,
  },
  proofing: {
    id: 'proofing',
    label: 'Proofing review',
    bleed: 18,
    markMargin: 36,
    marks: true,
    strictAssets: true,
    strictFonts: false,
    imageScale: 0.5,
  },
};

/**
 * @param {string|undefined} name
 * @returns {ExportPreset}
 */
export function presetOf(name) {
  const preset = PRESETS[name ?? 'screen'];
  if (!preset) throw new Error(`unknown export preset: ${name}`);
  return preset;
}

/**
 * @typedef {object} PublishProblem
 * @property {'error'|'warning'} severity
 * @property {string} code
 * @property {string} message
 * @property {string} [at]
 */

/**
 * Everything that must be true before bytes are written.
 *
 * @param {any} bundle
 * @param {object} [options]
 * @param {string} [options.preset]
 * @param {string[]} [options.pageIds]
 * @param {import('./fonts.js').FontEnvironment} [options.fonts]
 * @param {boolean} [options.allowMissingAssets] downgrade a missing asset to a warning
 * @param {boolean} [options.allowMissingFonts] downgrade a missing face to a warning
 * @param {boolean} [options.checkOverflow]
 * @returns {PublishProblem[]}
 */
export function validateForExport(bundle, options = {}) {
  const preset = presetOf(options.preset);
  const fonts = fontEnvironment(options.fonts);
  /** @type {PublishProblem[]} */
  const problems = [];

  const base = validateBundle(bundle, { fonts, checkOverflow: options.checkOverflow === true });
  for (const problem of base) {
    // The bundle checks already include link errors and geometry; keep their codes so a
    // caller can react to them, and downgrade the production warnings that a preset allows.
    if (problem.code === 'missing-font' && (options.allowMissingFonts === true || fonts.families.length === 0)) {
      problems.push({ severity: 'warning', code: problem.code, message: problem.message, ...(problem.at ? { at: problem.at } : {}) });
      continue;
    }
    if (problem.code === 'missing-asset-bytes') {
      const strict = preset.strictAssets && options.allowMissingAssets !== true;
      problems.push({
        severity: strict ? 'error' : 'warning',
        code: problem.code,
        message: strict
          ? `${problem.message}; restore the asset bytes or pass allowMissingAssets to export a marked placeholder`
          : problem.message,
        ...(problem.at ? { at: problem.at } : {}),
      });
      continue;
    }
    problems.push({
      severity: problem.severity,
      code: problem.code,
      message: problem.message,
      ...(problem.at ? { at: problem.at } : {}),
    });
  }

  const pages = options.pageIds
    ? options.pageIds.map((id) => bundle.project.pages[id]).filter(Boolean)
    : Object.values(bundle.project.pages);

  for (const page of pages) {
    const p = /** @type {any} */ (page);
    const artboard = findArtboard(bundle.document, p.artboardId);
    if (!artboard) continue;
    const { width, height } = artboard.size;
    if (width <= preset.bleed * 2 || height <= preset.bleed * 2) {
      problems.push({
        severity: 'error', code: 'page-too-small-for-bleed',
        message: `page ${p.id} is ${width}x${height}, too small for a ${preset.bleed}-unit bleed on each edge`,
        at: p.id,
      });
    }
    if (preset.id === 'print' && (width < 600 || height < 600 || width > 14400 || height > 14400)) {
      problems.push({
        severity: 'error', code: 'unsupported-print-dimensions',
        message: `page ${p.id} is ${width}x${height} units; a print page must be between 600 and 14400 units on each side at 300 ppi`,
        at: p.id,
      });
    }
  }

  // Overflow is a warning everywhere: the policy may legitimately be `warn`, and the marked
  // red box in the output is the user's signal rather than a refused export.
  if (options.checkOverflow !== false) {
    const measure = stubMeasure(0.5);
    for (const lettering of Object.values(bundle.project.lettering ?? {})) {
      const l = /** @type {any} */ (lettering);
      const node = bundle.document.nodes[String(l.nodeId)];
      if (!node || node.kind !== 'text') continue;
      const fit = fitText({
        text: l.text ?? '',
        frame: node.frame,
        style: { family: l.fontFamily ?? bundle.project.defaultLetteringStyle.family, sizePt: node.fontSize ?? bundle.project.defaultLetteringStyle.sizePt },
        measure,
        policy: l.overflowPolicy ?? 'warn',
      });
      if (fit.overflowed && l.overflowPolicy !== 'shrink') {
        problems.push({
          severity: 'warning', code: 'lettering-overflow',
          message: `lettering ${l.id} overflows its frame and will be marked in the export`,
          at: l.id,
        });
      }
    }
  }

  for (const family of new Set(Object.values(bundle.project.lettering ?? {}).map((/** @type {any} */ l) => l.fontFamily ?? bundle.project.defaultLetteringStyle.family))) {
    const missing = missingFont({ family }, fonts);
    if (!missing) continue;
    const strict = preset.strictFonts && fonts.families.length > 0 && options.allowMissingFonts !== true;
    problems.push({
      severity: strict ? 'error' : 'warning',
      code: 'missing-font',
      message: strict
        ? `missing font '${family}': the print preset will not substitute silently`
        : missing.message,
      at: family,
    });
  }

  return dedupe(problems);
}

/** @param {PublishProblem[]} problems @returns {PublishProblem[]} */
export function dedupe(problems) {
  const seen = new Set();
  /** @type {PublishProblem[]} */
  const out = [];
  for (const problem of problems) {
    const key = `${problem.severity}|${problem.code}|${problem.message}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(problem);
  }
  return out;
}

/** @param {PublishProblem[]} problems @returns {PublishProblem[]} */
export const publishErrors = (problems) => problems.filter((p) => p.severity === 'error');

/**
 * The geometry a PDF page must carry for a preset. Kept as a pure function so a test can
 * assert it without parsing a PDF, and the PDF writer uses the same numbers.
 * @param {{ width: number, height: number }} pageSize
 * @param {ExportPreset} preset
 * @returns {{ media: { width: number, height: number }, trim: { x: number, y: number, width: number, height: number }, bleed: { x: number, y: number, width: number, height: number }, contentOffset: { x: number, y: number }, marks: boolean }}
 */
export function presetGeometry(pageSize, preset) {
  const offset = preset.bleed + preset.markMargin;
  return {
    media: { width: pageSize.width + offset * 2, height: pageSize.height + offset * 2 },
    trim: { x: offset, y: offset, width: pageSize.width, height: pageSize.height },
    bleed: {
      x: preset.markMargin, y: preset.markMargin,
      width: pageSize.width + preset.bleed * 2,
      height: pageSize.height + preset.bleed * 2,
    },
    contentOffset: { x: offset, y: offset },
    marks: preset.marks,
  };
}

/**
 * @param {any} bundle
 * @param {string[]} [pageIds]
 * @returns {PublishProblem[]}
 */
export function validatePublication(bundle, pageIds) {
  const problems = validateForExport(bundle, { preset: 'print', pageIds });
  return problems.filter((p) => p.severity === 'error' || p.code === 'lettering-overflow');
}

/** @param {any} bundle @returns {number} */
export const pageCount = (bundle) => Object.keys(bundle.project.pages).length;

/** @param {any} bundle @returns {number} */
export const errorCount = (bundle) => errorsOf(validateBundle(bundle)).length;
