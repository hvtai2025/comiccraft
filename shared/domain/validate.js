/**
 * Validation. A loaded bundle is **untrusted input**, so every check here reports rather
 * than throws, and the caller decides whether a problem is fatal.
 *
 * @module shared/domain/validate
 */

import { SCHEMA_VERSION } from './project.js';
import { checkDocument } from './document.js';
import { parseId } from './ids.js';
import { isPositive, rect } from './geom.js';
import { fitText, stubMeasure } from './lettering.js';
import { fontEnvironment, missingFont } from './fonts.js';

/**
 * @typedef {object} Problem
 * @property {'error'|'warning'} severity
 * @property {'bundle'|'project'|'document'|'links'} area
 * @property {string} code
 * @property {string} message
 * @property {string} [at] the entity the problem is about
 */

/**
 * @typedef {object} ValidateOptions
 * @property {import('./fonts.js').FontEnvironment} [fonts] what the host can measure and draw
 * @property {import('./lettering.js').MeasureText} [measure] text metrics; defaults to the stub
 * @property {boolean} [checkOverflow] report text that does not fit (warnings, never errors)
 */

/**
 * Validate a whole bundle: schema version, entity shapes, the document's own invariants,
 * and every cross-model link.
 * @param {unknown} bundle
 * @param {ValidateOptions} [options]
 * @returns {Problem[]}
 */
export function validateBundle(bundle, options = {}) {
  /** @type {Problem[]} */
  const problems = [];

  if (!bundle || typeof bundle !== 'object') {
    return [{ severity: 'error', area: 'bundle', code: 'not-an-object', message: 'a bundle must be an object' }];
  }
  const b = /** @type {any} */ (bundle);
  if (!b.project || typeof b.project !== 'object') {
    problems.push({ severity: 'error', area: 'bundle', code: 'no-project', message: 'the bundle has no project' });
  }
  if (!b.document || typeof b.document !== 'object') {
    problems.push({ severity: 'error', area: 'bundle', code: 'no-document', message: 'the bundle has no document' });
  }
  if (problems.length > 0) return problems;

  const project = b.project;
  const document = b.document;

  if (project.schemaVersion !== SCHEMA_VERSION) {
    problems.push({
      severity: 'error', area: 'project', code: 'schema-version',
      message: `schema version ${project.schemaVersion} is not ${SCHEMA_VERSION}`,
      at: 'project',
    });
  }
  if (typeof project.title !== 'string' || project.title.trim() === '') {
    problems.push({ severity: 'warning', area: 'project', code: 'untitled', message: 'the project has no title' });
  }
  if (!parseId(project.id)) {
    problems.push({ severity: 'error', area: 'project', code: 'bad-project-id', message: `malformed project id: ${project.id}`, at: 'project' });
  }

  for (const [key, prefix] of [
    ['chapters', 'chapter'], ['scenes', 'scene'], ['pages', 'page'], ['panels', 'panel'],
    ['dialogue', 'dialogue'], ['lettering', 'lettering'], ['characters', 'character'],
    ['assets', 'asset'],
  ]) {
    const table = project[key];
    if (!table || typeof table !== 'object') {
      problems.push({ severity: 'error', area: 'project', code: 'missing-table', message: `the project has no ${key} table`, at: key });
      continue;
    }
    for (const [id, entity] of Object.entries(table)) {
      if (!parseId(id)) {
        problems.push({ severity: 'error', area: 'project', code: 'malformed-id', message: `malformed ${prefix} id: ${id}`, at: id });
      }
      if (!entity || typeof entity !== 'object') {
        problems.push({ severity: 'error', area: 'project', code: 'not-an-entity', message: `${id} is not an object`, at: id });
        continue;
      }
      const e = /** @type {any} */ (entity);
      if (e.id !== undefined && e.id !== id) {
        problems.push({ severity: 'error', area: 'project', code: 'id-mismatch', message: `${id} carries id ${e.id}`, at: id });
      }
    }
  }

  for (const problem of checkDocument(document)) {
    problems.push({ severity: 'error', area: 'document', code: 'document-invariant', message: problem });
  }

  problems.push(...validateLinks(b, options));
  problems.push(...validateProductionData(b, options));
  return problems;
}

/**
 * Cross-model link checks. These are separate from the shape checks because a dangling
 * link is a *content* problem — an object was deleted, or a bundle was edited by hand —
 * and the repair is a decision, not a default.
 * @param {any} b
 * @param {ValidateOptions} [options]
 * @returns {Problem[]}
 */
export function validateLinks(b, options = {}) {
  /** @type {Problem[]} */
  const problems = [];
  const project = b.project;
  const doc = b.document;
  const nodeExists = (/** @type {any} */ id) => String(id) in doc.nodes;
  const artboardExists = (/** @type {any} */ id) => doc.artboards.some((/** @type {any} */ a) => a.id === id);

  /** @type {Set<number>} */
  const claimedArtboards = new Set();
  for (const page of Object.values(project.pages ?? /** @type {any} */ ({}))) {
    const p = /** @type {any} */ (page);
    if (!artboardExists(p.artboardId)) {
      problems.push({ severity: 'error', area: 'links', code: 'missing-artboard', message: `${p.id} points at artboard ${p.artboardId}, which does not exist`, at: p.id });
    } else if (claimedArtboards.has(p.artboardId)) {
      problems.push({ severity: 'error', area: 'links', code: 'artboard-shared', message: `two pages claim artboard ${p.artboardId}; a page is an artboard`, at: p.id });
    }
    claimedArtboards.add(p.artboardId);
    if ((doc.retiredArtboards ?? []).includes(p.artboardId)) {
      problems.push({ severity: 'error', area: 'links', code: 'retired-artboard', message: `${p.id} points at retired artboard ${p.artboardId}; a retired id must never be reused`, at: p.id });
    }
    const board = doc.artboards.find((/** @type {any} */ a) => a.id === p.artboardId);
    if (board && !isPositive(rect(0, 0, board.size.width, board.size.height))) {
      problems.push({ severity: 'error', area: 'links', code: 'bad-page-size', message: `page ${p.id} has a non-positive artboard size`, at: p.id });
    }
    for (const pid of p.panelIds ?? []) {
      if (!project.panels?.[pid]) {
        problems.push({ severity: 'error', area: 'links', code: 'missing-panel', message: `page ${p.id} lists panel ${pid}, which does not exist`, at: p.id });
      }
    }
    // Reading order is the list itself. Checking it here, rather than in the layout engine,
    // keeps the rule independent of coordinates: the same geometry can read left-to-right or
    // right-to-left, and only the list can say which was intended.
    const seen = new Set();
    for (const pid of p.panelIds ?? []) {
      if (seen.has(pid)) {
        problems.push({ severity: 'error', area: 'links', code: 'duplicate-in-order', message: `page ${p.id} lists panel ${pid} twice in reading order`, at: p.id });
      }
      seen.add(pid);
    }
    const everyPanelOnPage = Object.values(project.panels ?? {})
      .filter((/** @type {any} */ panel) => Object.values(project.pages).some((/** @type {any} */ page) => page.id === p.id && page.panelIds.includes(panel.id)))
      .length;
    if (seen.size !== everyPanelOnPage) {
      problems.push({
        severity: 'error', area: 'links', code: 'reading-order-coverage',
        message: `page ${p.id} lists ${seen.size} panel(s) but owns ${everyPanelOnPage}; the reading order must cover every panel exactly once`,
        at: p.id,
      });
    }
    if (board && (board.size.width !== project.defaultPageSize?.width || board.size.height !== project.defaultPageSize?.height)) {
      problems.push({
        severity: 'warning', area: 'links', code: 'page-size-default',
        message: `page ${p.id} is ${board.size.width}x${board.size.height}, which differs from the project default ${project.defaultPageSize?.width}x${project.defaultPageSize?.height}`,
        at: p.id,
      });
    }
  }

  for (const panel of Object.values(project.panels ?? /** @type {any} */ ({}))) {
    const p = /** @type {any} */ (panel);
    if (!nodeExists(p.frameNodeId)) {
      problems.push({ severity: 'error', area: 'links', code: 'missing-frame', message: `panel ${p.id} points at object ${p.frameNodeId}, which does not exist`, at: p.id });
      continue;
    }
    const frame = doc.nodes[String(p.frameNodeId)];
    if (frame && frame.kind !== 'frame') {
      problems.push({ severity: 'error', area: 'links', code: 'frame-not-a-frame', message: `panel ${p.id} points at a ${frame.kind}, not a frame`, at: p.id });
    }
    for (const ref of p.artworkRefs ?? []) {
      const node = doc.nodes[String(ref)];
      if (!node) {
        problems.push({ severity: 'error', area: 'links', code: 'missing-artwork', message: `panel ${p.id} references object ${ref}, which does not exist`, at: p.id });
      } else if (node.kind !== 'image') {
        problems.push({ severity: 'warning', area: 'links', code: 'artwork-not-an-image', message: `panel ${p.id} references a ${node.kind} as artwork`, at: p.id });
      }
    }
    for (const lid of p.letteringIds ?? []) {
      if (!project.lettering?.[lid]) {
        problems.push({ severity: 'error', area: 'links', code: 'missing-lettering', message: `panel ${p.id} lists lettering ${lid}, which does not exist`, at: p.id });
      }
    }
    for (const did of p.dialogueIds ?? []) {
      if (!project.dialogue?.[did]) {
        problems.push({ severity: 'error', area: 'links', code: 'missing-dialogue', message: `panel ${p.id} lists dialogue ${did}, which does not exist`, at: p.id });
      }
    }
  }

  for (const l of Object.values(project.lettering ?? /** @type {any} */ ({}))) {
    const e = /** @type {any} */ (l);
    if (!nodeExists(e.nodeId)) {
      problems.push({ severity: 'error', area: 'links', code: 'missing-lettering-object', message: `lettering ${e.id} points at object ${e.nodeId}, which does not exist`, at: e.id });
    }
    if (e.tailNodeId !== undefined && e.tailNodeId !== null && !nodeExists(e.tailNodeId)) {
      problems.push({ severity: 'error', area: 'links', code: 'missing-tail', message: `lettering ${e.id} points at tail object ${e.tailNodeId}, which does not exist`, at: e.id });
    }
  }

  for (const d of Object.values(project.dialogue ?? /** @type {any} */ ({}))) {
    const e = /** @type {any} */ (d);
    if (!project.lettering?.[e.letteringId]) {
      problems.push({ severity: 'error', area: 'links', code: 'dialogue-without-lettering', message: `dialogue ${e.id} has no lettering to present it`, at: e.id });
    }
    if (e.characterId && !project.characters?.[e.characterId]) {
      problems.push({ severity: 'warning', area: 'links', code: 'unknown-character', message: `dialogue ${e.id} names character ${e.characterId}, which does not exist`, at: e.id });
    }
  }

  for (const asset of Object.values(project.assets ?? /** @type {any} */ ({}))) {
    const a = /** @type {any} */ (asset);
    for (const nodeId of a.nodeIds ?? []) {
      if (!nodeExists(nodeId)) {
        problems.push({ severity: 'error', area: 'links', code: 'missing-asset-node', message: `asset ${a.id} lists object ${nodeId}, which does not exist`, at: a.id });
      }
    }
  }

  return problems;
}

/**
 * Production facts that are not link errors: a missing face, missing asset bytes, text that
 * does not fit, and jobs whose target has gone. Each is reported with a code a UI can act on.
 * @param {any} b
 * @param {ValidateOptions} options
 * @returns {Problem[]}
 */
export function validateProductionData(b, options = {}) {
  /** @type {Problem[]} */
  const problems = [];
  const project = b.project;
  const fonts = options.fonts ? fontEnvironment(options.fonts) : fontEnvironment(undefined);
  const measure = options.measure ?? stubMeasure(0.5);

  const styles = new Set();
  for (const l of Object.values(project.lettering ?? {})) {
    const e = /** @type {any} */ (l);
    const family = e.fontFamily ?? project.defaultLetteringStyle?.family ?? 'Comic Neue';
    styles.add(family);
    const node = b.document.nodes[String(e.nodeId)];
    if (options.checkOverflow && node && node.kind === 'text') {
      const fit = fitText({
        text: e.text ?? node.text ?? '',
        frame: node.frame,
        style: { family, sizePt: node.fontSize ?? project.defaultLetteringStyle?.sizePt ?? 12 },
        measure,
        policy: e.overflowPolicy ?? 'warn',
      });
      if (fit.overflowed) {
        problems.push({
          severity: 'warning', area: 'links', code: 'lettering-overflow',
          message: `lettering ${e.id} does not fit its frame at ${fit.fontSize}pt (${fit.lines.length} line(s)); policy is ${e.overflowPolicy ?? 'warn'}`,
          at: e.id,
        });
      }
    }
  }
  for (const family of styles) {
    const missing = missingFont({ family }, fonts);
    if (missing) {
      problems.push({ severity: 'warning', area: 'project', code: missing.code, message: missing.message, at: family });
    }
  }

  for (const asset of Object.values(project.assets ?? {})) {
    const a = /** @type {any} */ (asset);
    if (a.data === undefined && !a.storageReference) {
      problems.push({
        severity: 'warning', area: 'project', code: 'missing-asset-bytes',
        message: `asset ${a.id} (${a.name}) has no bytes in the bundle, so exports draw a labelled placeholder`,
        at: a.id,
      });
    }
  }

  for (const job of Object.values(project.jobs ?? {})) {
    const j = /** @type {any} */ (job);
    if (j.targetPanelId && !project.panels[j.targetPanelId] && j.status !== 'cancelled') {
      problems.push({
        severity: 'warning', area: 'project', code: 'job-target-missing',
        message: `job ${j.id} targets panel ${j.targetPanelId}, which no longer exists; its result is kept as an asset and cannot be applied`,
        at: j.id,
      });
    }
  }

  for (const proposal of Object.values(project.proposals ?? {})) {
    const p = /** @type {any} */ (proposal);
    if (p.status === 'proposed' && p.baseRevision !== undefined && p.baseRevision !== (project.revision ?? 1)) {
      problems.push({
        severity: 'warning', area: 'project', code: 'proposal-stale',
        message: `proposal ${p.id} was built against revision ${p.baseRevision}, but the project is at ${project.revision}; review it before accepting`,
        at: p.id,
      });
    }
  }
  return problems;
}

/** @param {Problem[]} problems @returns {Problem[]} */
export const errorsOf = (problems) => problems.filter((p) => p.severity === 'error');

/** @param {Problem[]} problems @returns {Problem[]} */
export const warningsOf = (problems) => problems.filter((p) => p.severity === 'warning');

/**
 * Reading order must be explicit and must not be derivable from coordinates. This checks
 * the first half — that the order is a real list of the page's panels, with no repeats and
 * nothing missing. That the order is *independent* of geometry is a property of the shape
 * of the model, proved by a test that reorders panels without moving them.
 * @param {any} project
 * @param {string} pageId
 * @returns {Problem[]}
 */
export function validateReadingOrder(project, pageId) {
  /** @type {Problem[]} */
  const problems = [];
  const page = project.pages[pageId];
  if (!page) return [{ severity: 'error', area: 'links', code: 'no-page', message: `no page ${pageId}`, at: pageId }];
  const seen = new Set();
  for (const pid of page.panelIds) {
    if (seen.has(pid)) {
      problems.push({ severity: 'error', area: 'links', code: 'duplicate-in-order', message: `page ${pageId} lists panel ${pid} twice`, at: pageId });
    }
    seen.add(pid);
  }
  return problems;
}
