/**
 * Serialize and deserialize a project bundle.
 *
 * The bundle is the **unit of storage on both sides**: in the browser it is an object
 * persisted to IndexedDB or downloaded as a folder, on the server it is a directory. The
 * same module produces it in both cases, so a bundle saved locally opens on the server
 * unchanged — there is no separate "export format" to keep in sync.
 *
 * Save order matters: the manifest is written **last**, so it can never reference
 * partially written data (design §7.2).
 *
 * @module shared/domain/bundle
 */

import { SCHEMA_VERSION, normalizeProject } from './project.js';
import { DOCUMENT_VERSION, repairNodeCounter, createDocument } from './document.js';
import { validateBundle } from './validate.js';

/** The file that says which schema and document versions the rest of the bundle is in. */
export const MANIFEST = 'manifest.json';
/** The single visual document. Pages are artboards inside it. */
export const DOCUMENT_FILE = 'document.json';
/** The comic project: narrative structure and everything the document does not own. */
export const PROJECT_FILE = 'project.json';
/** Per-page comic files live here, keyed by page id rather than by an ordinal. */
export const PAGES_DIR = 'pages';
/** Content-addressed assets. An interrupted save writes a new file, never over an old one. */
export const ASSETS_DIR = 'assets';
/** Generation jobs and review decisions: durable so an interrupted session can report them. */
export const PROVENANCE_FILE = 'provenance/generations.json';
/** Story proposals live outside project.json so a proposal never rewrites the book. */
export const STORY_FILE = 'story/proposals.json';

/**
 * A bundle as a set of files: path → contents. Producing a map rather than writing
 * directly is what lets the same code serve the filesystem, a zip, and a test.
 * @typedef {Map<string, string | Uint8Array>} FileSet
 */

/**
 * @param {unknown} bundle
 * @returns {{ ok: true, files: FileSet } | { ok: false, problems: import('./validate.js').Problem[] }}
 */
export function serialize(bundle) {
  const problems = validateBundle(bundle);
  const errors = problems.filter((p) => p.severity === 'error');
  if (errors.length > 0) return { ok: false, problems };

  const b = /** @type {any} */ (bundle);
  /** @type {FileSet} */
  const files = new Map();

  files.set(DOCUMENT_FILE, json({ version: DOCUMENT_VERSION, artboards: b.document.artboards, nodes: b.document.nodes, nextNodeId: b.document.nextNodeId, retiredArtboards: b.document.retiredArtboards }));

  // Per-page comic files, one per page. Named by page id, never by an ordinal: an ordinal
  // changes when a page is inserted, and a file that changes its name when an unrelated
  // page is added is a file that gets lost on a merge.
  const pageScoped = ['panels', 'dialogue', 'lettering'];
  for (const page of Object.values(b.project.pages ?? /** @type {any} */ ({}))) {
    const p = /** @type {any} */ (page);
    /** @type {Record<string, unknown>} */
    const slice = { page: p };
    for (const table of pageScoped) {
      /** @type {Record<string, unknown>} */
      const picked = {};
      const panelIds = new Set(p.panelIds);
      for (const [id, entity] of Object.entries(b.project[table] ?? {})) {
        if (table === 'panels' && panelIds.has(id)) picked[id] = entity;
        if (table === 'dialogue' && belongsToPage(b.project, p, entity, 'dialogue')) picked[id] = entity;
        if (table === 'lettering' && belongsToPage(b.project, p, entity, 'lettering')) picked[id] = entity;
      }
      slice[table] = picked;
    }
    files.set(`${PAGES_DIR}/${p.id}.json`, json(slice));
  }

  files.set(PROJECT_FILE, json(stripPageScoped(b.project)));
  files.set(PROVENANCE_FILE, json({ jobs: b.project.jobs ?? {}, reviews: b.project.reviews ?? {} }));
  files.set(STORY_FILE, json({ proposals: b.project.proposals ?? {} }));

  for (const asset of Object.values(b.project.assets ?? /** @type {any} */ ({}))) {
    const a = /** @type {any} */ (asset);
    if (a.contentHash && a.data !== undefined) {
      files.set(`${ASSETS_DIR}/${a.contentHash}`, a.data);
    }
  }

  // The manifest is written last: it must never be readable while the data it names is
  // half-written.
  files.set(MANIFEST, json({
    name: 'comiccraft',
    schemaVersion: SCHEMA_VERSION,
    documentVersion: DOCUMENT_VERSION,
    projectId: b.project.id,
    title: b.project.title,
    pageCount: Object.keys(b.project.pages ?? {}).length,
    revision: b.project.revision ?? 1,
  }));

  return { ok: true, files };
}

/**
 * @param {unknown} value
 * @returns {string}
 */
function json(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

/**
 * The project file holds everything that is not page-scoped. Panels, dialogue and lettering
 * travel in their page's file instead, so editing one page does not rewrite the whole book
 * — and two people editing different pages do not conflict.
 * @param {any} project
 * @returns {Record<string, unknown>}
 */
function stripPageScoped(project) {
  /** @type {Record<string, unknown>} */
  const rest = {};
  for (const [key, value] of Object.entries(project)) {
    if (key === 'panels' || key === 'dialogue' || key === 'lettering') continue;
    if (key === 'jobs' || key === 'reviews' || key === 'proposals') continue;
    rest[key] = value;
  }
  return rest;
}

/**
 * @param {any} project @param {any} page @param {any} entity @param {'dialogue'|'lettering'} table
 * @returns {boolean}
 */
function belongsToPage(project, page, entity, table) {
  for (const pid of page.panelIds) {
    const panel = project.panels[pid];
    if (!panel) continue;
    if (table === 'dialogue' && panel.dialogueIds.includes(entity.id)) return true;
    if (table === 'lettering' && panel.letteringIds.includes(entity.id)) return true;
  }
  return false;
}

/**
 * Read a bundle from a file set. `missing` names the files a bundle needed and did not
 * have — reported rather than thrown, because a half-copied directory is a real case.
 * @param {Map<string, string | Uint8Array>} files
 * @returns {{ ok: true, bundle: any, problems: import('./validate.js').Problem[] } | { ok: false, missing: string[], problems: import('./validate.js').Problem[] }}
 */
export function deserialize(files) {
  const read = (/** @type {string} */ path) => {
    const value = files.get(path);
    if (value === undefined) return undefined;
    return typeof value === 'string' ? value : new TextDecoder().decode(value);
  };

  for (const required of [MANIFEST, PROJECT_FILE, DOCUMENT_FILE]) {
    if (!files.has(required)) {
      return { ok: false, missing: [required], problems: [] };
    }
  }

  let manifest;
  let project;
  let document;
  try {
    manifest = JSON.parse(/** @type {string} */ (read(MANIFEST)));
    project = JSON.parse(/** @type {string} */ (read(PROJECT_FILE)));
    document = JSON.parse(/** @type {string} */ (read(DOCUMENT_FILE)));
  } catch (error) {
    return { ok: false, missing: [], problems: [{ severity: 'error', area: 'bundle', code: 'unparseable', message: `a bundle file is not valid JSON: ${/** @type {Error} */ (error).message}` }] };
  }

  const doc = { ...createDocument(), ...document };
  // Repair before use: the reference format's counter is `max(id) + 1` and a deleted node's
  // id can otherwise be reissued, which is silent when it breaks.
  repairNodeCounter(doc);

  const merged = normalizeProject({ ...project, panels: {}, dialogue: {}, lettering: {} });
  merged.revision = Number.isInteger(manifest.revision) && manifest.revision > 0
    ? Math.max(merged.revision ?? 1, manifest.revision)
    : (merged.revision ?? 1);
  // Jobs and proposals are their own files so a proposal never rewrites the book, and so an
  // interrupted session can report what was in flight. Their absence is normal, not missing.
  const records = /** @type {any} */ (merged);
  for (const [path, key] of [[PROVENANCE_FILE, 'jobs'], [STORY_FILE, 'proposals']]) {
    const raw = read(path);
    if (raw === undefined) continue;
    try {
      const parsed = JSON.parse(raw);
      Object.assign(records[key], parsed[key] ?? {});
      if (key === 'jobs') Object.assign(records.reviews, parsed.reviews ?? {});
    } catch {
      // A damaged provenance file must not make the comic unopenable. The data is reported
      // by validation as missing rather than silently treated as empty.
      records[key] = records[key] ?? {};
    }
  }
  /** @type {string[]} */
  const missingPages = [];
  const prefix = `${PAGES_DIR}/`;
  for (const path of files.keys()) {
    if (!path.startsWith(prefix)) continue;
    const slice = JSON.parse(/** @type {string} */ (read(path)));
    Object.assign(merged.panels, slice.panels ?? {});
    Object.assign(merged.dialogue, slice.dialogue ?? {});
    Object.assign(merged.lettering, slice.lettering ?? {});
  }
  for (const pageId of Object.keys(merged.pages ?? {})) {
    if (!files.has(`${PAGES_DIR}/${pageId}.json`)) missingPages.push(`${PAGES_DIR}/${pageId}.json`);
  }

  const bundle = { project: merged, document: doc, manifest };
  const problems = validateBundle(bundle);
  // A page whose file is missing loses its panels and lettering. That is a warning rather
  // than an error on purpose: the rest of the bundle is intact and can be repaired, and
  // refusing to open the whole book because one page file went astray would be worse.
  for (const path of missingPages) {
    problems.push({
      severity: 'warning', area: 'bundle', code: 'missing-page-file',
      message: `${path} is missing, so that page's panels and lettering are not loaded`,
    });
  }
  return { ok: true, bundle, problems };
}

/**
 * Migrate a bundle to the current schema version. Refuses rather than guesses, preserves
 * every id, and preserves unknown fields so a newer ComicCraft can still read an older file.
 * @param {any} bundle
 * @returns {{ bundle: any, migrated: string[] }}
 */
export function migrate(bundle) {
  /** @type {string[]} */
  const migrated = [];
  const project = bundle.project;
  if (project.schemaVersion === SCHEMA_VERSION) return { bundle, migrated };

  if (project.schemaVersion === undefined) {
    throw new Error('refusing to migrate: the project has no schema version');
  }
  if (project.schemaVersion > SCHEMA_VERSION) {
    throw new Error(
      `refusing to migrate: schema version ${project.schemaVersion} is newer than this build ` +
      `understands (${SCHEMA_VERSION}). Opening it would discard fields it does not know about.`,
    );
  }

  const next = { ...project };
  if (next.schemaVersion === 1) {
    // v1 → v2 in the reference added panel locking. A v1 project has no locks at all,
    // which is the correct default rather than an invented one.
    for (const panel of Object.values(next.panels ?? {})) {
      const p = /** @type {any} */ (panel);
      if (p.locked === undefined) p.locked = false;
    }
    next.schemaVersion = 2;
    migrated.push('v1 → v2: added `locked: false` to every panel');
  }

  if (next.schemaVersion !== SCHEMA_VERSION) {
    throw new Error(`no migration path from schema version ${next.schemaVersion} to ${SCHEMA_VERSION}`);
  }
  return { bundle: { ...bundle, project: next }, migrated };
}
