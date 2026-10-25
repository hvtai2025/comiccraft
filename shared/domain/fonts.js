/**
 * Fonts are **referenced, never shipped**.
 *
 * No font file lives in this repository. Lettering names a family, and the host says which
 * families it can actually measure and draw. A project whose face is missing must still
 * open, wrap, fit and export — with a *reported* missing font, never a silent substitution.
 * That is why this module resolves a style to a report rather than to bytes.
 *
 * @module shared/domain/fonts
 */

/**
 * @typedef {object} FontFace
 * @property {string} family
 * @property {string} role          what the face is for
 * @property {string[]} styles
 * @property {string} source        where a deployment obtains it; never a path in this repo
 * @property {string} licence
 */

/** The two OFL-1.1 faces ComicCraft names as defaults (design ADR-017). */
export const FONT_CATALOG = /** @type {Record<string, FontFace>} */ ({
  'Comic Neue': {
    family: 'Comic Neue',
    role: 'dialogue, captions and narration',
    styles: ['regular', 'bold', 'italic'],
    source: 'craft-fonts',
    licence: 'OFL-1.1',
  },
  'Bangers': {
    family: 'Bangers',
    role: 'display: sound effects, titles and cover text',
    styles: ['regular'],
    source: 'craft-fonts',
    licence: 'OFL-1.1',
  },
});

/** The default pair, named by settings rather than hard-coded into a command. */
export const DEFAULT_FONT = 'Comic Neue';
export const DISPLAY_FONT = 'Bangers';

/**
 * @typedef {object} FontEnvironment
 * @property {string[]} families the families this host can measure and draw; empty means none
 */

/** @type {FontEnvironment} */
export const NO_FONTS = { families: [] };

/**
 * Normalise a host's font list into an environment. A host that passes nothing gets the
 * tested, fontless contract rather than an accidental assumption that a system font exists.
 * @param {unknown} value
 * @returns {FontEnvironment}
 */
export function fontEnvironment(value) {
  if (value && typeof value === 'object' && Array.isArray(/** @type {any} */ (value).families)) {
    return { families: /** @type {any} */ (value).families.filter((/** @type {any} */ f) => typeof f === 'string' && f.length > 0) };
  }
  return { ...NO_FONTS };
}

/**
 * @param {{ family: string, sizePt?: number, bold?: boolean, italic?: boolean }} style
 * @param {FontEnvironment} [environment]
 * @returns {{ family: string, sizePt: number, bold: boolean, italic: boolean } | null}
 *   the face that will be used, or null when the project's family is not available
 */
export function resolveFace(style, environment = NO_FONTS) {
  const family = typeof style.family === 'string' && style.family.length > 0 ? style.family : DEFAULT_FONT;
  if (!environment.families.some((f) => f.toLowerCase() === family.toLowerCase())) return null;
  return {
    family,
    sizePt: typeof style.sizePt === 'number' && Number.isFinite(style.sizePt) ? style.sizePt : 12,
    bold: style.bold === true,
    italic: style.italic === true,
  };
}

/**
 * The missing-font report a command and the validator share. It is a value, not a warning
 * written into a log: the caller can show it, and a test can assert it.
 * @param {{ family: string }} style
 * @param {FontEnvironment} [environment]
 * @returns {{ code: 'missing-font', family: string, message: string } | null}
 */
export function missingFont(style, environment = NO_FONTS) {
  const family = typeof style.family === 'string' && style.family.length > 0 ? style.family : DEFAULT_FONT;
  if (environment.families.some((f) => f.toLowerCase() === family.toLowerCase())) return null;
  return {
    code: 'missing-font',
    family,
    message: `the font '${family}' is not available on this host; text is measured with the deterministic fallback and exported with a standard face`,
  };
}

/**
 * The family a lettering style should use for a kind. Sound effects get the display face;
 * everything spoken or narrated gets the reading face. Both are defaults a project can
 * override, not constants a renderer enforces.
 * @param {string} kind
 * @param {{ family: string, sizePt: number }} defaults
 * @returns {{ family: string, sizePt: number }}
 */
export function styleForKind(kind, defaults) {
  if (kind === 'sound-effect') {
    return { family: DISPLAY_FONT, sizePt: Math.max(defaults.sizePt, 14) };
  }
  return { family: defaults.family, sizePt: defaults.sizePt };
}

/**
 * @returns {{ name: string, role: string, styles: string[], licence: string, source: string }[]}
 */
export function catalogue() {
  return Object.values(FONT_CATALOG).map((f) => ({
    name: f.family,
    role: f.role,
    styles: [...f.styles],
    licence: f.licence,
    source: f.source,
  }));
}
