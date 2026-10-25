/**
 * Turning a pointer position into a node.
 *
 * Hit-testing runs in **page units**, not device pixels, so a hit is the same whether the
 * user is zoomed to 20% or 140% — and so the test for it does not need a canvas.
 *
 * @module public/editor/hit-test
 */

import { artboardNodes } from '../../shared/domain/document.js';
import { contains, point } from '../../shared/domain/geom.js';

/**
 * The topmost node at a page-space point. Later nodes are drawn on top, so the search runs
 * back to front — the opposite order from drawing, which is the mistake this function exists
 * to make once rather than in every caller.
 * @param {any} bundle
 * @param {any} page
 * @param {{ x: number, y: number }} at in page units
 * @returns {any | null}
 */
export function nodeAt(bundle, page, at) {
  const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  if (!artboard) return null;
  const nodes = artboardNodes(bundle.document, artboard.id);
  for (let i = nodes.length - 1; i >= 0; i--) {
    const node = nodes[i];
    if (!node.frame) continue;
    if (contains(node.frame, { x: at.x, y: at.y, width: 0, height: 0 })) return node;
  }
  return null;
}

/**
 * The panel whose frame contains the point, if any. Panels are frames, so this is a subset
 * of `nodeAt` — but it is what the UI needs for "place artwork in the panel I clicked".
 * @param {any} bundle
 * @param {any} page
 * @param {{ x: number, y: number }} at
 * @returns {any | null}
 */
export function panelAt(bundle, page, at) {
  const artboard = bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  if (!artboard) return null;
  const nodes = artboardNodes(bundle.document, artboard.id);
  for (let i = nodes.length - 1; i >= 0; i--) {
    const node = nodes[i];
    if (node.kind !== 'frame' || node.name === 'Balloon Tail') continue;
    if (contains(node.frame, { x: at.x, y: at.y, width: 0, height: 0 })) {
      for (const panelId of page.panelIds) {
        const panel = bundle.project.panels[panelId];
        if (panel && panel.frameNodeId === node.id) return { panel, node };
      }
    }
  }
  return null;
}

/**
 * Convert a pointer event on a canvas element into page units.
 * @param {HTMLCanvasElement} canvas
 * @param {MouseEvent} event
 * @param {number} scale
 * @returns {{ x: number, y: number }}
 */
export function toPageUnits(canvas, event, scale) {
  const rect = canvas.getBoundingClientRect();
  return point(
    (event.clientX - rect.left) / scale,
    (event.clientY - rect.top) / scale,
  );
}
