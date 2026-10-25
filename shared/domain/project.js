/**
 * The comic project: the authoritative semantic model.
 *
 * This owns **meaning** — what a page is for, which panel follows which, who speaks, what
 * they say — and the visual document owns geometry. They are linked by id, never by a
 * duplicated field, because two copies of the same geometry is the failure this
 * architecture exists to prevent (design §6.1).
 *
 * @module shared/domain/project
 */

import { IdCounter } from './ids.js';
import { createDocument } from './document.js';

/** The comic schema version written into the manifest. */
export const SCHEMA_VERSION = 1;

/**
 * @typedef {'balloon'|'caption'|'sound-effect'} LetteringKind
 * @typedef {'warn'|'shrink'|'resize-frame'} OverflowPolicy
 */

/**
 * @typedef {object} Lettering
 * @property {string} id
 * @property {LetteringKind} kind
 * @property {number} nodeId        the document object that draws it
 * @property {string} text
 * @property {OverflowPolicy} overflowPolicy
 * @property {number} [tailNodeId]  the tail's own object, for a balloon
 * @property {boolean} [tailAttached] false once a move detaches it deliberately
 */

/**
 * @typedef {object} Dialogue
 * @property {string} id
 * @property {string|null} characterId
 * @property {string} letteringId   the lettering that presents this line
 * @property {string} text
 */

/**
 * @typedef {object} Panel
 * @property {string} id
 * @property {number} frameNodeId   the document object that owns the geometry
 * @property {string} [shotDescription]
 * @property {string} [camera]
 * @property {string} [artworkPrompt]
 * @property {string[]} dialogueIds
 * @property {string[]} letteringIds
 * @property {number[]} artworkRefs
 * @property {Record<string, 'primary'|'background'|'overlay'>} [artworkRoles]
 * @property {string[]} continuityIds
 * @property {boolean} locked
 */

/**
 * @typedef {object} Page
 * @property {string} id
 * @property {string} name
 * @property {number} artboardId    the artboard that *is* this page
 * @property {string[]} panelIds    in reading order — explicit, never derived from x/y
 * @property {'draft'|'generated'|'lettered'|'approved'|'published'} [status]
 */

/**
 * @typedef {object} Scene
 * @property {string} id
 * @property {string} name
 * @property {string} [summary]
 * @property {string[]} pageIds
 */

/**
 * @typedef {object} Chapter
 * @property {string} id
 * @property {string} title
 * @property {string[]} sceneIds
 */

/**
 * @typedef {object} ComicProject
 * @property {number} schemaVersion
 * @property {string} id
 * @property {string} title
 * @property {Record<string, number>} counters
 * @property {Record<string, Chapter>} chapters
 * @property {Record<string, Scene>} scenes
 * @property {Record<string, Page>} pages
 * @property {Record<string, Panel>} panels
 * @property {Record<string, Dialogue>} dialogue
 * @property {Record<string, Lettering>} lettering
 * @property {Record<string, CharacterRecord>} characters
 * @property {Record<string, object>} locations
 * @property {Record<string, object>} styles
 * @property {Record<string, AssetRecord>} assets
 * @property {Record<string, import('./jobs.js').GenerationJob>} jobs
 * @property {Record<string, import('./story.js').StoryProposal>} proposals
 * @property {Record<string, import('./jobs.js').ReviewDecision>} reviews
 * @property {{ author?: string, language?: string, genre?: string, readingDirection: 'ltr'|'rtl', created: number, updated: number }} metadata
 * @property {{ width: number, height: number }} defaultPageSize
 * @property {string|null} defaultTemplate
 * @property {{ family: string, sizePt: number, align?: 'left'|'center'|'right', padding?: number, bold?: boolean, italic?: boolean }} defaultLetteringStyle
 * @property {number} revision  monotonic; optimistic concurrency for generation jobs
 */

/**
 * @typedef {object} CharacterRecord
 * @property {string} id
 * @property {string} name
 * @property {string} description
 * @property {string} [canonicalDescription]
 * @property {string[]} referenceAssetIds
 * @property {string|null} [approvedDesignVersion]
 * @property {string[]} [continuityRules]
 * @property {string[]} [continuityExceptions]
 * @property {{ description: string, assetId: string, approvedAt: number }[]} [designVersions]
 * @property {{ face?: string, body?: string, clothing?: string, palette?: string }} [appearance]
 *
 * @typedef {object} AssetRecord
 * @property {string} id
 * @property {string} name
 * @property {string} contentHash
 * @property {string} [data]
 * @property {string} [storageReference]
 * @property {'source'|'generated'|'edited'|'reference'} [assetType]
 * @property {string|null} [mime]
 * @property {{ mime?: string|null, width?: number, height?: number }} [mediaMetadata]
 * @property {{ provider: string, model?: string|null, request?: unknown, inputAssetIds?: string[], createdAt?: number, placedAt?: number|null }} [provenance]
 * @property {string[]} [parentAssetIds]
 * @property {string[]} [inputAssetIds]
 * @property {'pending'|'approved'|'rejected'} [approvalStatus]
 * @property {number} [approvedAt]
 * @property {number[]} nodeIds
 *
 * @typedef {object} Storage
 * @property {(path: string) => Promise<string | undefined>} read
 * @property {(path: string, data: string) => Promise<void>} write
 * @property {(path: string) => Promise<Uint8Array | undefined>} [readBytes]
 * @property {(path: string, data: Uint8Array) => Promise<void>} [writeBytes]
 * @property {(prefix: string) => Promise<string[]>} list
 * @property {(prefix: string) => Promise<void>} remove
 * @property {(root: string) => Promise<void>} ensureRoot
 */

/** A4 at 300 ppi is the print default; a comic page is taller than it is wide. */
export const DEFAULT_PAGE_SIZE = { width: 1275, height: 1950 };

/** @returns {ComicProject} */
export function createProject(title = 'Untitled Comic') {
  const counters = new IdCounter();
  return {
    schemaVersion: SCHEMA_VERSION,
    id: counters.next('project'),
    title,
    counters: counters.toJSON(),
    chapters: {}, scenes: {}, pages: {}, panels: {}, dialogue: {}, lettering: {},
    characters: {}, locations: {}, styles: {}, assets: {},
    jobs: {}, proposals: {}, reviews: {},
    metadata: {
      language: 'en',
      readingDirection: 'ltr',
      created: 0,
      updated: 0,
    },
    defaultPageSize: { ...DEFAULT_PAGE_SIZE },
    defaultTemplate: null,
    defaultLetteringStyle: { family: 'Comic Neue', sizePt: 12, align: 'left', padding: 8, bold: false, italic: false },
    revision: 1,
  };
}

/**
 * Jobs and proposals are additive collections. A bundle written before they existed must
 * still open, so reading fills them in rather than making every reader check for undefined.
 * @param {ComicProject} project
 * @returns {ComicProject}
 */
export function normalizeProject(project) {
  if (!project.jobs) project.jobs = {};
  if (!project.proposals) project.proposals = {};
  if (!project.reviews) project.reviews = {};
  if (!project.metadata) project.metadata = { language: 'en', readingDirection: 'ltr', created: 0, updated: 0 };
  if (project.metadata.readingDirection !== 'rtl') project.metadata.readingDirection = 'ltr';
  if (!Number.isInteger(project.revision) || project.revision < 1) project.revision = 1;
  return project;
}

/** @param {Bundle} bundle @returns {number} */
export const revisionOf = (bundle) => bundle.project.revision ?? 1;

/**
 * A revision is **monotonic**. Undo restores content, not time: rolling the number back
 * would let a job built against an older document apply over newer approved state.
 * @param {Bundle} bundle
 * @returns {number} the new revision
 */
export function bumpRevision(bundle) {
  bundle.project.revision = revisionOf(bundle) + 1;
  return bundle.project.revision;
}

/**
 * A project always travels with its document. Keeping them in one object is what stops a
 * caller from mutating the document without the model noticing, or the reverse.
 *
 * @typedef {object} Bundle
 * @property {ComicProject} project
 * @property {import('./document.js').VisualDocument} document
 * @property {object} [manifest] present once the bundle has been written or read
 */

/**
 * @param {string} [title]
 * @returns {Bundle}
 */
export function createBundle(title) {
  return { project: createProject(title), document: createDocument() };
}

/** @param {Bundle} bundle @returns {IdCounter} */
export const counterOf = (bundle) => new IdCounter(bundle.project.counters);

/** @param {Bundle} bundle @param {IdCounter} counter */
export function storeCounter(bundle, counter) {
  bundle.project.counters = counter.toJSON();
}

/**
 * Allocate an id and persist the advanced counter in the same step, so a crash between the
 * two cannot reissue an id.
 * @param {Bundle} bundle
 * @param {string} prefix
 * @returns {string}
 */
export function allocate(bundle, prefix) {
  const counter = counterOf(bundle);
  const id = counter.next(prefix);
  storeCounter(bundle, counter);
  return id;
}

/**
 * Raise the counter past an id, so a loaded bundle cannot reissue it.
 * @param {Bundle} bundle @param {string} prefix @param {string} id
 */
export function retire(bundle, prefix, id) {
  const counter = counterOf(bundle);
  counter.retire(prefix, id);
  storeCounter(bundle, counter);
}

/**
 * Register a page against an artboard that already exists. Creating the artboard is the
 * caller's job (it belongs to the document), which is what keeps this function free of any
 * dependency on the document module.
 * @param {Bundle} bundle
 * @param {Scene} scene
 * @param {{ name: string, artboardId: number }} args
 * @returns {Page}
 */
export function addPage(bundle, scene, { name, artboardId }) {
  if (!Number.isInteger(artboardId)) throw new Error('artboardId must be an integer');
  const id = allocate(bundle, 'page');
  /** @type {Page} */
  const page = { id, name, artboardId, panelIds: [] };
  bundle.project.pages[id] = page;
  scene.pageIds.push(id);
  return page;
}

/**
 * Reading order is an explicit list, validated on its own terms. It is **never** derived
 * from panel coordinates: a right-to-left page and a left-to-right page can have identical
 * geometry, so geometry cannot be the source of truth for order.
 * @param {ComicProject} project
 * @param {string} pageId
 * @returns {Panel[]} the panels in reading order, skipping any id that no longer resolves
 */
export function readingOrder(project, pageId) {
  const page = project.pages[pageId];
  if (!page) return [];
  return page.panelIds.map((id) => project.panels[id]).filter(Boolean);
}

/**
 * Remove placed-object ids from every asset that named them. Assets are **not** deleted:
 * provenance outlives the object that used it, and a validation error about a missing node
 * would be false once the panel that held it is gone.
 * @param {ComicProject} project
 * @param {Iterable<number|string>} nodeIds
 */
export function unlinkAssetNodes(project, nodeIds) {
  const gone = new Set([...nodeIds].map((id) => Number(id)));
  for (const asset of Object.values(project.assets)) {
    const a = /** @type {any} */ (asset);
    if (!Array.isArray(a.nodeIds)) continue;
    a.nodeIds = a.nodeIds.filter((/** @type {any} */ id) => !gone.has(Number(id)));
  }
}

/**
 * Every comedic link that dangles. A dangling link is reported, never repaired silently:
 * inventing content to fill a hole is how a document loses meaning.
 * @param {Bundle} bundle
 * @returns {{ ref: string, from: string, field: string, reason: string }[]}
 */
export function danglingLinks(bundle) {
  const { project, document: doc } = bundle;
  /** @type {{ ref: string, from: string, field: string, reason: string }[]} */
  const out = [];
  const hasNode = (/** @type {number} */ id) => String(id) in doc.nodes;
  const hasArtboard = (/** @type {number} */ id) => doc.artboards.some((a) => a.id === id);

  for (const page of Object.values(project.pages)) {
    if (!hasArtboard(page.artboardId)) {
      out.push({ ref: String(page.artboardId), from: page.id, field: 'artboardId', reason: 'artboard does not exist' });
    }
    for (const pid of page.panelIds) {
      if (!project.panels[pid]) {
        out.push({ ref: pid, from: page.id, field: 'panelIds', reason: 'panel does not exist' });
      }
    }
  }
  for (const panel of Object.values(project.panels)) {
    if (!hasNode(panel.frameNodeId)) {
      out.push({ ref: String(panel.frameNodeId), from: panel.id, field: 'frameNodeId', reason: 'frame object does not exist' });
    }
    for (const ref of panel.artworkRefs) {
      if (!hasNode(Number(ref))) {
        out.push({ ref: String(ref), from: panel.id, field: 'artworkRefs', reason: 'artwork object does not exist' });
      }
    }
    for (const lid of panel.letteringIds) {
      if (!project.lettering[lid]) {
        out.push({ ref: lid, from: panel.id, field: 'letteringIds', reason: 'lettering does not exist' });
      }
    }
    for (const did of panel.dialogueIds) {
      if (!project.dialogue[did]) {
        out.push({ ref: did, from: panel.id, field: 'dialogueIds', reason: 'dialogue does not exist' });
      }
    }
  }
  for (const l of Object.values(project.lettering)) {
    if (!hasNode(l.nodeId)) {
      out.push({ ref: String(l.nodeId), from: l.id, field: 'nodeId', reason: 'lettering object does not exist' });
    }
  }
  for (const d of Object.values(project.dialogue)) {
    if (d.characterId && !project.characters[d.characterId]) {
      out.push({ ref: d.characterId, from: d.id, field: 'characterId', reason: 'character does not exist' });
    }
    if (!project.lettering[d.letteringId]) {
      out.push({ ref: d.letteringId, from: d.id, field: 'letteringId', reason: 'lettering does not exist' });
    }
  }
  for (const asset of Object.values(project.assets)) {
    const a = /** @type {any} */ (asset);
    for (const nodeId of a.nodeIds ?? []) {
      if (!hasNode(nodeId)) {
        out.push({ ref: String(nodeId), from: a.id, field: 'nodeIds', reason: 'placed object does not exist' });
      }
    }
  }
  return out;
}
