/**
 * Deterministic local artwork drawings.
 *
 * This is **not** a model. It paints a panel from its prompt and style with shapes, gradients
 * and a caption, using no network, no key and no random source. It exists so the whole
 * workflow — idea → storyboard → edit → artwork → publish — can be exercised on a laptop with
 * no provider configured. A real provider is an optional adapter in `server/ai/`.
 *
 * The output is an SVG data URL, which the browser displays directly and the package stores as
 * text. The raster exporter cannot decode it, so it deliberately draws its labelled placeholder
 * instead of pretending to have rendered the artwork.
 *
 * @module shared/domain/art
 */

export const ART_STYLES = /** @type {const} */ ([
  'black ink', 'manga', 'noir', 'watercolour', 'retro',
]);

/** @typedef {'black ink'|'manga'|'noir'|'watercolour'|'retro'} ArtStyle */

/** @param {string} value @returns {string} */
function escapeSvg(value) {
  return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** @param {unknown} value @returns {number} */
function hashOf(value) {
  let h = 2166136261;
  const text = String(value ?? '');
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h;
}

/**
 * @param {unknown} value
 * @returns {string}
 */
export const drawingFingerprint = (value) => hashOf(value).toString(16).padStart(8, '0');

/**
 * A stable palette per style and seed.
 * @param {ArtStyle} style
 * @param {number} seed
 */
function palette(style, seed) {
  const hue = seed % 360;
  switch (style) {
    case 'manga':
      return { sky: '#f7f7f2', back: '#d9d9d2', mid: '#8e8e86', front: '#1b1b19', accent: '#ffffff', ink: '#101010', caption: '#ffffff' };
    case 'noir':
      return { sky: `hsl(${220 + (seed % 20)} 25% 18%)`, back: `hsl(${225 + (seed % 12)} 30% 26%)`, mid: `hsl(${215 + (seed % 18)} 28% 42%)`, front: '#0b0e14', accent: `hsl(${42} 85% 62%)`, ink: '#f3f0e8', caption: '#f3f0e8' };
    case 'watercolour':
      return { sky: `hsl(${(hue + 30) % 360} 70% 88%)`, back: `hsl(${(hue + 90) % 360} 45% 72%)`, mid: `hsl(${(hue + 150) % 360} 38% 54%)`, front: `hsl(${(hue + 210) % 360} 32% 30%)`, accent: '#fff7d6', ink: '#2a2f38', caption: '#ffffff' };
    case 'retro':
      return { sky: '#f4e3b2', back: '#d68f5a', mid: '#8c4a3c', front: '#2f2a3a', accent: '#ffd166', ink: '#1f1b24', caption: '#f4e3b2' };
    default:
      return { sky: '#ffffff', back: '#dfe6ef', mid: '#8fa3bb', front: '#1b1b19', accent: '#2f5d9e', ink: '#111111', caption: '#ffffff' };
  }
}

/**
 * @param {object} args
 * @param {string} args.prompt
 * @param {ArtStyle} [args.style]
 * @param {number} [args.width]
 * @param {number} [args.height]
 * @param {string} [args.seed] extra seed material (usually the panel id)
 * @returns {{ data: string, mime: string, width: number, height: number, model: string, provider: string, prompt: string, style: ArtStyle }}
 */
export function makeDrawing({ prompt, style = 'black ink', width = 800, height = 600, seed = '' }) {
  const chosen = ART_STYLES.includes(/** @type {any} */ (style)) ? style : 'black ink';
  const hash = hashOf(`${prompt}|${chosen}|${seed}`);
  const p = palette(chosen, hash);
  const w = Math.max(64, Math.min(4000, Math.round(width)));
  const h = Math.max(64, Math.min(4000, Math.round(height)));
  const caption = String(prompt || 'Untitled panel').replace(/\s+/g, ' ').trim().slice(0, 140);
  const horizon = Math.round(h * (0.55 + ((hash >>> 5) % 20) / 100));
  const sunX = Math.round(w * (0.2 + ((hash >>> 9) % 60) / 100));
  const sunY = Math.round(h * 0.24);
  const sunR = Math.round(Math.min(w, h) * (0.08 + ((hash >>> 13) % 6) / 100));
  const figureX = Math.round(w * (0.55 + ((hash >>> 17) % 25) / 100));
  const figures = 1 + (hash % 3);

  /** @type {string[]} */
  const shapes = [];
  shapes.push(`<rect width="${w}" height="${h}" fill="${p.sky}"/>`);
  shapes.push(`<circle cx="${sunX}" cy="${sunY}" r="${sunR}" fill="${p.accent}" opacity="0.9"/>`);
  if (chosen === 'manga') {
    for (let i = 0; i < 18; i++) {
      const y = Math.round((i / 18) * h);
      shapes.push(`<line x1="0" y1="${y}" x2="${w}" y2="${y - 30}" stroke="#111" stroke-width="1" opacity="0.18"/>`);
    }
  }
  if (chosen === 'retro') {
    shapes.push(`<defs><pattern id="dots" width="10" height="10" patternUnits="userSpaceOnUse"><circle cx="2" cy="2" r="1.5" fill="#2f2a3a" opacity="0.25"/></pattern></defs>`);
    shapes.push(`<rect width="${w}" height="${h}" fill="url(#dots)"/>`);
  }
  shapes.push(`<path d="M0 ${horizon} L${Math.round(w * 0.18)} ${Math.round(horizon - h * 0.12)} L${Math.round(w * 0.36)} ${horizon - Math.round(h * 0.05)} L${Math.round(w * 0.55)} ${horizon - Math.round(h * 0.16)} L${Math.round(w * 0.78)} ${horizon - Math.round(h * 0.04)} L${w} ${horizon - Math.round(h * 0.1)} L${w} ${h} L0 ${h} Z" fill="${p.back}"/>`);
  shapes.push(`<rect x="0" y="${horizon}" width="${w}" height="${h - horizon}" fill="${p.mid}" opacity="0.55"/>`);
  for (let i = 0; i < figures; i++) {
    const x = figureX + i * Math.round(w * 0.08);
    const base = h - Math.round(h * (0.08 + (i % 2) * 0.03));
    const scale = Math.min(w, h) / 600;
    shapes.push(`<g fill="${p.front}">`);
    shapes.push(`<circle cx="${x}" cy="${base - Math.round(120 * scale)}" r="${Math.round(26 * scale)}"/>`);
    shapes.push(`<path d="M${x - 36 * scale} ${base} L${x - 24 * scale} ${base - 110 * scale} L${x + 24 * scale} ${base - 110 * scale} L${x + 36 * scale} ${base} Z"/>`);
    shapes.push('</g>');
  }
  shapes.push(`<rect x="8" y="8" width="${w - 16}" height="${h - 16}" fill="none" stroke="${p.ink}" stroke-width="6"/>`);
  shapes.push(`<rect x="18" y="${h - 54}" width="${w - 36}" height="36" fill="${chosen === 'noir' ? '#0b0e14' : '#ffffff'}" opacity="0.86"/>`);
  shapes.push(`<text x="30" y="${h - 28}" font-family="sans-serif" font-size="${Math.max(12, Math.round(Math.min(w, h) / 26))}" fill="${chosen === 'noir' ? p.caption : '#111'}">${escapeSvg(caption)}</text>`);
  if (chosen === 'watercolour') {
    shapes.push(`<rect width="${w}" height="${h}" fill="${p.accent}" opacity="0.06"/>`);
  }

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${shapes.join('')}</svg>`;
  return {
    data: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`,
    mime: 'image/svg+xml',
    width: w,
    height: h,
    model: 'deterministic-drawing-v1',
    provider: 'local-vector',
    prompt: caption,
    style: chosen,
  };
}
