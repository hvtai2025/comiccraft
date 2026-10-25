/**
 * The visual document: artboards and the nodes inside them.
 *
 * **A page is an artboard.** One project is one visual document, and each page is one
 * artboard — so a page can be undone, exported and thumbnailed with the machinery that
 * already exists for artboards (design ADR-011, §5.1.1).
 *
 * Node ids are **numbers**, matching the reference format so a bundle round-trips without
 * renumbering. The next-node counter is persisted and repaired on load, because a deleted
 * node's id must never be reissued: a comic record may still point at it.
 *
 * @module shared/domain/document
 */

import { rect, quantize, quantizeRect, isPositive } from './geom.js';

/** @typedef {'frame'|'image'|'text'|'group'} NodeKind */

/**
 * @typedef {object} BaseNode
 * @property {number} id
 * @property {NodeKind} kind
 * @property {string} name
 * @property {{ x: number, y: number, width: number, height: number }} frame
 * @property {number} [rotation]
 */

/**
 * @typedef {BaseNode & { kind: 'frame', fill: string, stroke: string|null, strokeWidth: number }} FrameNode
 * @typedef {BaseNode & {
 *   kind: 'image',
 *   assetId: string,
 *   fit: 'cover'|'contain',
 *   clipToFrame: boolean,
 *   crop?: { x: number, y: number, width: number, height: number },
 *   clip?: { x: number, y: number, width: number, height: number },
 * }} ImageNode
 * @typedef {BaseNode & { kind: 'text', text: string, fontSize: number, lineHeight: number, align: 'left'|'center'|'right', color: string }} TextNode
 * @typedef {BaseNode & { kind: 'group', children: number[] }} GroupNode
 * @typedef {FrameNode|ImageNode|TextNode|GroupNode} DocNode
 */

/**
 * A node about to be added: a `DocNode` without its id, and **per member** rather than
 * `Omit<DocNode, 'id'>`. `Omit` over a union collapses it to the keys every member shares, so
 * `{ kind: 'frame', fill: … }` would be rejected for naming a property none of the others
 * have — which is every node a caller actually builds.
 *
 * @typedef {Omit<FrameNode,'id'>|Omit<ImageNode,'id'>|Omit<TextNode,'id'>|Omit<GroupNode,'id'>} NewNode
 */

/**
 * @typedef {object} Artboard
 * @property {number} id
 * @property {string} name
 * @property {{ width: number, height: number }} size
 * @property {number[]} nodes
 */

/**
 * @typedef {object} VisualDocument
 * @property {number} version
 * @property {Artboard[]} artboards
 * @property {Record<string, DocNode>} nodes  keyed by numeric id as a string
 * @property {number} nextNodeId
 * @property {number[]} retiredArtboards
 */

/** The document format version this build reads and writes. */
export const DOCUMENT_VERSION = 1;

/** @returns {VisualDocument} */
export function createDocument() {
  return { version: DOCUMENT_VERSION, artboards: [], nodes: {}, nextNodeId: 1, retiredArtboards: [] };
}

/**
 * The document's node counter is `max(id) + 1` in the reference format, which means a
 * deleted node's id *can* be reissued. That is silent when it breaks, so we repair the
 * counter on load instead of trusting it: raise it past every id the document mentions.
 * @param {VisualDocument} doc
 * @returns {number} how many nodes the repair stepped over
 */
export function repairNodeCounter(doc) {
  let max = 0;
  for (const key of Object.keys(doc.nodes)) {
    const n = Number(key);
    if (Number.isInteger(n) && n > max) max = n;
  }
  for (const a of doc.artboards) {
    if (Number.isInteger(a.id) && a.id > max) max = a.id;
    for (const n of a.nodes) if (Number.isInteger(n) && n > max) max = n;
  }
  const wanted = max + 1;
  if (doc.nextNodeId < wanted) {
    const stepped = wanted - doc.nextNodeId;
    doc.nextNodeId = wanted;
    return stepped;
  }
  return 0;
}

/**
 * @param {VisualDocument} doc
 * @param {string} name
 * @param {{ width: number, height: number }} size
 * @returns {number} the new artboard's id
 */
export function createArtboard(doc, name, size) {
  const id = Math.max(doc.nextNodeId, ...doc.artboards.map((a) => a.id + 1), 1);
  doc.nextNodeId = Math.max(doc.nextNodeId, id + 1);
  doc.artboards.push({ id, name, size: { ...size }, nodes: [] });
  return id;
}

/**
 * @param {VisualDocument} doc
 * @param {number} artboardId
 * @returns {Artboard | undefined}
 */
export const findArtboard = (doc, artboardId) => doc.artboards.find((a) => a.id === artboardId);

/**
 * Remove an artboard, recording its id as retired so it is never reissued.
 * @param {VisualDocument} doc
 * @param {number} artboardId
 * @returns {boolean} true when an artboard was removed
 */
export function deleteArtboard(doc, artboardId) {
  const i = doc.artboards.findIndex((a) => a.id === artboardId);
  if (i === -1) return false;
  const [removed] = doc.artboards.splice(i, 1);
  doc.retiredArtboards.push(removed.id);
  for (const nodeId of removed.nodes) delete doc.nodes[String(nodeId)];
  return true;
}

/**
 * Add a node to an artboard, or to a group inside it.
 *
 * A node is reachable from **exactly one** place: either the artboard's list or a group's
 * `children`. It is never both — a node listed twice is drawn twice and counted twice by every
 * walk of the tree, which is what `checkDocument` now catches. Passing `parentId` is how a
 * caller says which of the two it means.
 *
 * @param {VisualDocument} doc
 * @param {number} artboardId
 * @param {NewNode} node
 * @param {number} [parentId] a group to place it in; omit for the artboard itself
 * @returns {number} the new node's id
 */
export function addNode(doc, artboardId, node, parentId) {
  const artboard = findArtboard(doc, artboardId);
  if (!artboard) throw new Error(`no artboard ${artboardId}`);

  if (parentId !== undefined) {
    const parent = getNode(doc, parentId);
    if (!parent) throw new Error(`no node ${parentId} to add into`);
    if (parent.kind !== 'group') throw new Error(`node ${parentId} is a ${parent.kind}, not a group`);
    for (const a of doc.artboards) {
      if (a.id !== artboardId && a.nodes.includes(parentId)) {
        throw new Error(`node ${parentId} is on artboard ${a.id}, not ${artboardId}`);
      }
    }
  }

  if (node.kind === 'group' && /** @type {GroupNode} */ (node).children !== undefined) {
    // A group is born empty; its children are added with `parentId`, which is what keeps the
    // "reachable from exactly one place" rule in one writer instead of every caller.
    throw new Error('a group is created empty; add its children with the parentId argument');
  }

  const id = doc.nextNodeId;
  doc.nextNodeId = id + 1;
  /** @type {any} */
  const stored = node.kind === 'group'
    ? { ...node, id, frame: quantizeRect(node.frame), children: [] }
    : { ...node, id, frame: quantizeRect(node.frame) };
  doc.nodes[String(id)] = stored;

  if (parentId === undefined) artboard.nodes.push(id);
  else /** @type {any} */ (getNode(doc, parentId)).children.push(id);
  return id;
}

/**
 * @param {VisualDocument} doc
 * @param {number} nodeId
 * @returns {DocNode | undefined}
 */
export const getNode = (doc, nodeId) => doc.nodes[String(nodeId)];

/**
 * Type guards for the node union, so a filtered list is **narrowed** rather than left as a
 * union the caller has to cast. Both renderers filter by kind, and without these the type
 * check cannot see that `n.kind === 'frame'` implies the frame's own fields exist.
 * @param {DocNode | undefined} node
 * @returns {node is FrameNode}
 */
export const isFrame = (node) => node !== undefined && node.kind === 'frame';

/** @param {DocNode | undefined} node @returns {node is ImageNode} */
export const isImage = (node) => node !== undefined && node.kind === 'image';

/** @param {DocNode | undefined} node @returns {node is TextNode} */
export const isText = (node) => node !== undefined && node.kind === 'text';

/** @param {DocNode | undefined} node @returns {node is GroupNode} */
export const isGroup = (node) => node !== undefined && node.kind === 'group';

/**
 * Remove a node from its artboard. The node id is **not** reissued: `nextNodeId` never
 * moves backwards, so a dangling comic reference is detectable rather than dangerous.
 * @param {VisualDocument} doc
 * @param {number} nodeId
 * @returns {boolean}
 */
export function removeNode(doc, nodeId) {
  const key = String(nodeId);
  if (!(key in doc.nodes)) return false;
  for (const a of doc.artboards) {
    const i = a.nodes.indexOf(nodeId);
    if (i !== -1) a.nodes.splice(i, 1);
  }
  // A node can be inside a group instead of directly on the artboard, so every group is
  // checked too — a group that kept a reference to a removed child would walk into nothing.
  for (const candidate of Object.values(doc.nodes)) {
    if (candidate.kind !== 'group') continue;
    const i = candidate.children.indexOf(nodeId);
    if (i !== -1) candidate.children.splice(i, 1);
  }
  // Removing a group takes its children with it: they are reachable from nothing else.
  const node = doc.nodes[key];
  if (node && node.kind === 'group') {
    for (const child of [...node.children]) removeNode(doc, child);
  }
  delete doc.nodes[key];
  return true;
}

/**
 * @param {VisualDocument} doc
 * @param {number} nodeId
 * @param {Record<string, any>} patch
 * @returns {DocNode} the patched node
 */
export function updateNode(doc, nodeId, patch) {
  const node = getNode(doc, nodeId);
  if (!node) throw new Error(`no node ${nodeId}`);
  if (patch.frame) patch = { ...patch, frame: quantizeRect(patch.frame) };
  Object.assign(node, patch);
  return node;
}

/**
 * Keep placed artwork with its panel when the panel frame moves or resizes.
 *
 * Artwork stores its own geometry (a renderer draws the image, not the panel), so a panel
 * edit must retarget it. Scaling the image frame by the panel's scale is what makes
 * `fit: cover` cover the new frame instead of the old one.
 *
 * @param {VisualDocument} doc
 * @param {Iterable<number|string>} nodeIds
 * @param {{ x: number, y: number, width: number, height: number }} before
 * @param {{ x: number, y: number, width: number, height: number }} after
 */
export function retargetArtwork(doc, nodeIds, before, after) {
  if (before.width <= 0 || before.height <= 0) return;
  const sx = after.width / before.width;
  const sy = after.height / before.height;
  /** @param {{ x: number, y: number, width: number, height: number }} r */
  const map = (r) => ({
    x: quantize(after.x + (r.x - before.x) * sx),
    y: quantize(after.y + (r.y - before.y) * sy),
    width: quantize(r.width * sx),
    height: quantize(r.height * sy),
  });
  for (const id of nodeIds) {
    const node = getNode(doc, Number(id));
    if (!node || node.kind !== 'image') continue;
    node.frame = map(node.frame);
    if (node.clip) node.clip = map(node.clip);
  }
}

/**
 * Every node in an artboard, depth-first, groups expanded.
 * @param {VisualDocument} doc
 * @param {number} artboardId
 * @returns {DocNode[]}
 */
export function artboardNodes(doc, artboardId) {
  const artboard = findArtboard(doc, artboardId);
  if (!artboard) return [];
  /** @type {DocNode[]} */
  const out = [];
  /** @param {number[]} ids */
  const walk = (ids) => {
    for (const id of ids) {
      const node = getNode(doc, id);
      if (!node) continue;
      out.push(node);
      if (node.kind === 'group') walk(node.children);
    }
  };
  walk(artboard.nodes);
  return out;
}

/**
 * @param {VisualDocument} doc
 * @param {number} artboardId
 * @returns {number}
 */
export function artboardArea(doc, artboardId) {
  const artboard = findArtboard(doc, artboardId);
  return artboard ? artboard.size.width * artboard.size.height : 0;
}

/**
 * Structural check of the document against its own invariants. Returns problems, never
 * throws: a loaded bundle is untrusted input and the caller decides what to do.
 * @param {VisualDocument} doc
 * @returns {string[]}
 */
export function checkDocument(doc) {
  /** @type {string[]} */
  const problems = [];
  if (doc.version !== DOCUMENT_VERSION) problems.push(`document version ${doc.version} is not ${DOCUMENT_VERSION}`);

  const boardIds = new Set();
  for (const a of doc.artboards) {
    if (boardIds.has(a.id)) problems.push(`duplicate artboard id ${a.id}`);
    boardIds.add(a.id);
    if (!isPositive(rect(0, 0, a.size.width, a.size.height))) {
      problems.push(`artboard ${a.id} has a non-positive size`);
    }
    for (const n of a.nodes) {
      if (!(String(n) in doc.nodes)) problems.push(`artboard ${a.id} lists node ${n}, which does not exist`);
    }
  }
  for (const retired of doc.retiredArtboards) {
    if (boardIds.has(retired)) {
      problems.push(`artboard ${retired} is both live and retired — a retired id must never be reused`);
    }
  }
  // A node must be reachable from exactly one place. Twice means it is drawn twice and every
  // tree walk double-counts it, which is silent until something measures the page.
  /** @type {Map<number, number>} */
  const reachEdges = new Map();
  /** @param {number} id */
  const countReach = (id) => reachEdges.set(id, (reachEdges.get(id) ?? 0) + 1);
  for (const a of doc.artboards) for (const n of a.nodes) countReach(n);
  for (const node of Object.values(doc.nodes)) {
    if (node.kind === 'group') for (const c of node.children) countReach(c);
  }
  for (const [id, count] of reachEdges) {
    if (count > 1) problems.push(`node ${id} is listed ${count} times; a node belongs to one artboard or one group`);
  }

  for (const [key, node] of Object.entries(doc.nodes)) {
    const id = Number(key);
    if (!Number.isInteger(id) || id < 1) problems.push(`node key ${key} is not a positive integer`);
    if (node.id !== id) problems.push(`node ${key} carries id ${node.id}`);
    if (!isPositive(node.frame)) problems.push(`node ${id} has a non-positive frame`);
    if (node.kind === 'group') {
      for (const c of node.children) {
        if (!(String(c) in doc.nodes)) problems.push(`group ${id} lists child ${c}, which does not exist`);
      }
    }
    if (id >= doc.nextNodeId) problems.push(`node ${id} is at or past nextNodeId ${doc.nextNodeId}`);
  }
  return problems;
}
