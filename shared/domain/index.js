/**
 * The ComicCraft domain. **Pure**: no DOM, no `node:*`, no `fetch`, no provider SDK.
 * Everything it needs from a runtime — a clock, an id counter, text measurement, storage —
 * is injected by the caller. That is what lets the browser and the server import this
 * directory unchanged, and it is enforced by a test, not by convention.
 *
 * @module shared/domain
 */

export * as geom from './geom.js';
export * as ids from './ids.js';
export * as document from './document.js';
export * as project from './project.js';
export * as layout from './layout.js';
export * as lettering from './lettering.js';
export * as validate from './validate.js';
export * as bundle from './bundle.js';
export * as history from './history.js';

export { DEFAULT_PAGE_SIZE, SCHEMA_VERSION, allocate, createBundle, createProject, counterOf, danglingLinks, readingOrder, retire, storeCounter } from './project.js';
export { DOCUMENT_VERSION, addNode, artboardNodes, checkDocument, createArtboard, createDocument, deleteArtboard, findArtboard, getNode, isFrame, isGroup, isImage, isText, removeNode, repairNodeCounter, updateNode } from './document.js';
export { History, clone } from './history.js';
export { IdCounter, isIdOfKind, parseId } from './ids.js';
export { TEMPLATES, degenerateFrames, findOverlaps, framesOutsidePage, gridFrames, template } from './layout.js';
export { fitText, moveTail, requiredFrame, stubMeasure, tailGeometry, wrap } from './lettering.js';
export { errorsOf, validateBundle, validateLinks, validateReadingOrder, warningsOf } from './validate.js';
export { ASSETS_DIR, DOCUMENT_FILE, MANIFEST, PAGES_DIR, PROJECT_FILE, deserialize, migrate, serialize } from './bundle.js';
export * from './geom.js';
