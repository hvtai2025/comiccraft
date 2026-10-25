/**
 * The domain model: ids, geometry, the document, and the links between them.
 * @module test/domain
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  IdCounter, parseId, isIdOfKind, findArtboard,
  quantize, rect, union, intersection, inset, isPositive,
  createDocument, addNode, getNode, removeNode, createArtboard, deleteArtboard,
  repairNodeCounter, checkDocument, artboardNodes,
  createBundle, createProject, allocate, retire, danglingLinks,
} from '../shared/domain/index.js';
import { validateBundle, errorsOf, validateReadingOrder } from '../shared/domain/validate.js';

test('ids are typed, never reused, and reject malformed input', () => {
  const counter = new IdCounter();
  assert.equal(counter.next('page'), 'page_1');
  assert.equal(counter.next('page'), 'page_2');
  assert.equal(counter.next('panel'), 'panel_1', 'each prefix counts on its own');

  // A retired id must never be issued again: a comic record may still point at it, and a
  // reissued id would silently re-point that record at a different entity.
  counter.retire('page', 'page_99');
  assert.equal(counter.next('page'), 'page_100');

  assert.deepEqual(parseId('panel_12'), { prefix: 'panel', n: 12 });
  assert.equal(parseId('nope_1'), null, 'an unknown prefix is not an id');
  assert.equal(parseId('panel_0'), null, 'zero is not a valid counter value');
  assert.equal(parseId('panel'), null);
  assert.equal(parseId('panel_abc'), null);
  assert.equal(isIdOfKind('panel', 'panel_3'), true);
  assert.equal(isIdOfKind('page', 'panel_3'), false);
  assert.throws(() => counter.next('widget'), /unknown id prefix/);
});

test('geometry is quantized so a round trip cannot appear to change a document', () => {
  assert.equal(quantize(0.1 + 0.2), 0.3);
  assert.deepEqual(rect(3.0000001, 4, 5, 6).x, 3.0000001);
  const r = rect(0, 0, 100, 50);
  assert.deepEqual(union(r, rect(100, 0, 50, 50)), rect(0, 0, 150, 50));
  assert.deepEqual(intersection(r, rect(50, 25, 100, 100)), rect(50, 25, 50, 25));
  assert.equal(intersection(r, rect(200, 200, 10, 10)), null);
  assert.deepEqual(inset(r, 10), rect(10, 10, 80, 30));
  assert.equal(isPositive(rect(0, 0, 0, 10)), false);
  assert.equal(isPositive(rect(0, 0, NaN, 10)), false);
});

test('a node id is never reissued, even after the node is removed', () => {
  const doc = createDocument();
  // Artboards and nodes share one id space, as the reference format does, so no absolute
  // value is assumed here — the invariant is what this test is about.
  const board = createArtboard(doc, 'Page 1', { width: 100, height: 100 });
  const first = addNode(doc, board, { kind: 'frame', name: 'Panel', frame: rect(0, 0, 10, 10), fill: '#fff', stroke: null, strokeWidth: 1 });
  assert.ok(first > board, 'a node does not take its artboard\'s id');

  removeNode(doc, first);
  const second = addNode(doc, board, { kind: 'frame', name: 'Panel', frame: rect(0, 0, 10, 10), fill: '#fff', stroke: null, strokeWidth: 1 });
  assert.ok(second > first, 'the counter moved forward rather than backfilling the hole');
  assert.notEqual(second, first, 'the removed id is not reused');
  assert.equal(getNode(doc, first), undefined);

  const third = addNode(doc, board, { kind: 'frame', name: 'Panel', frame: rect(0, 0, 10, 10), fill: '#fff', stroke: null, strokeWidth: 1 });
  assert.equal(third, second + 1, 'ids are consecutive until something is removed');
});

test('a page artboard and its panels never collide on an id', () => {
  const doc = createDocument();
  const boards = [1, 2, 3].map((n) => createArtboard(doc, `Page ${n}`, { width: 50, height: 50 }));
  const nodeIds = boards.map((b) =>
    addNode(doc, b, { kind: 'frame', name: 'Panel', frame: rect(0, 0, 10, 10), fill: '#fff', stroke: null, strokeWidth: 1 }));
  const all = [...boards, ...nodeIds];
  assert.equal(new Set(all).size, all.length, 'every id in the document is distinct');
  assert.deepEqual(checkDocument(doc), []);
});

test('the node counter is repaired on load, because the reference format repairs nothing', () => {
  const doc = createDocument();
  doc.nodes['1'] = { id: 1, kind: 'frame', name: 'a', frame: rect(0, 0, 1, 1) };
  doc.nodes['7'] = { id: 7, kind: 'frame', name: 'b', frame: rect(0, 0, 1, 1) };
  doc.nextNodeId = 2; // a bundle claiming the counter is lower than an id it contains
  const stepped = repairNodeCounter(doc);
  assert.equal(doc.nextNodeId, 8, 'raised past the highest id in the document');
  assert.equal(stepped, 6);
  assert.deepEqual(checkDocument(doc).filter((p) => p.includes('nextNodeId')), []);
});

test('a retired artboard id is never reused, and reusing one is detected', () => {
  const doc = createDocument();
  const a = createArtboard(doc, 'Page 1', { width: 10, height: 10 });
  deleteArtboard(doc, a);
  assert.deepEqual(doc.retiredArtboards, [a]);

  const b = createArtboard(doc, 'Page 2', { width: 10, height: 10 });
  assert.notEqual(b, a, 'a fresh artboard does not take the retired id');

  // Force the collision the hazard describes and prove the structural check reports it.
  doc.artboards.push({ id: a, name: 'Reused', size: { width: 10, height: 10 }, nodes: [] });
  const problems = checkDocument(doc).filter((p) => p.includes('both live and retired'));
  assert.equal(problems.length, 1, 'the reuse is reported, not silently accepted');
});

test('a bundle round-trips its ids, and dangling links are reported rather than invented', () => {
  const bundle = createBundle('Link Probe');
  const pageId = allocate(bundle, 'page');
  const panelId = allocate(bundle, 'panel');
  const board = createArtboard(bundle.document, 'Page 1', { width: 100, height: 100 });
  const frameNode = addNode(bundle.document, board, {
    kind: 'frame', name: 'Panel', frame: rect(0, 0, 50, 50), fill: '#fff', stroke: null, strokeWidth: 1,
  });
  bundle.project.pages[pageId] = { id: pageId, name: 'Page 1', artboardId: board, panelIds: [panelId] };
  bundle.project.panels[panelId] = {
    id: panelId, frameNodeId: frameNode, dialogueIds: [], letteringIds: [], artworkRefs: [], continuityIds: [], locked: false,
  };

  assert.deepEqual(errorsOf(validateBundle(bundle)), [], 'a well-formed bundle validates');

  // Delete the object behind the panel's reference: the panel must report a hole rather than
  // invent a replacement.
  removeNode(bundle.document, frameNode);
  const errors = errorsOf(validateBundle(bundle));
  assert.equal(errors.length, 1);
  assert.equal(errors[0].code, 'missing-frame');
  assert.equal(danglingLinks(bundle).length, 1, 'the same hole is visible from the model');
});

test('one artboard cannot be claimed by two pages, because a page IS an artboard', () => {
  const bundle = createBundle('Shared Artboard');
  const board = createArtboard(bundle.document, 'Shared', { width: 10, height: 10 });
  const a = allocate(bundle, 'page');
  const b = allocate(bundle, 'page');
  bundle.project.pages[a] = { id: a, name: 'A', artboardId: board, panelIds: [] };
  bundle.project.pages[b] = { id: b, name: 'B', artboardId: board, panelIds: [] };
  const codes = validateBundle(bundle).map((p) => p.code);
  assert.ok(codes.includes('artboard-shared'), `expected artboard-shared, got ${codes.join(', ')}`);
});

test('retiring an id raises the persisted counter past it', () => {
  const bundle = createBundle('Retire');
  allocate(bundle, 'page');
  retire(bundle, 'page', 'page_50');
  assert.equal(bundle.project.counters.page, 50);
  assert.equal(allocate(bundle, 'page'), 'page_51');
});

test('reading order is a real list, and duplicates in it are detected', () => {
  const bundle = createBundle('Order');
  const pageId = allocate(bundle, 'page');
  const panelId = allocate(bundle, 'panel');
  bundle.project.pages[pageId] = { id: pageId, name: 'P', artboardId: 1, panelIds: [panelId, panelId] };
  const problems = validateReadingOrder(bundle.project, pageId);
  assert.equal(problems.length, 1);
  assert.equal(problems[0].code, 'duplicate-in-order');
});

test('artboardNodes expands groups depth-first, and a node belongs to exactly one place', () => {
  const doc = createDocument();
  const board = createArtboard(doc, 'Page', { width: 100, height: 100 });
  const group = addNode(doc, board, { kind: 'group', name: 'Group', frame: rect(0, 0, 10, 10) });
  const inner = addNode(doc, board, { kind: 'frame', name: 'Inner', frame: rect(0, 0, 5, 5), fill: '#fff', stroke: null, strokeWidth: 1 }, group);

  assert.deepEqual(artboardNodes(doc, board).map((n) => n.id), [group, inner]);
  assert.deepEqual(findArtboard(doc, board).nodes, [group], 'the group is on the artboard, its child is not');
  assert.deepEqual(getNode(doc, group).children, [inner]);

  // The rule the model now enforces, and which the first version of it did not.
  assert.throws(
    () => addNode(doc, board, { kind: 'group', name: 'Bad', frame: rect(0, 0, 1, 1), children: [inner] }),
    /created empty/,
  );
  assert.deepEqual(checkDocument(doc).filter((p) => p.includes('listed')), [], 'no node is reachable twice');

  removeNode(doc, inner);
  assert.deepEqual(artboardNodes(doc, board).map((n) => n.id), [group], 'a removed child is simply absent');
  assert.deepEqual(getNode(doc, group).children, [], 'and the group no longer lists it');
});

test('removing a group removes its children, because nothing else reaches them', () => {
  const doc = createDocument();
  const board = createArtboard(doc, 'Page', { width: 100, height: 100 });
  const group = addNode(doc, board, { kind: 'group', name: 'Group', frame: rect(0, 0, 10, 10) });
  const child = addNode(doc, board, { kind: 'frame', name: 'Child', frame: rect(0, 0, 5, 5), fill: '#fff', stroke: null, strokeWidth: 1 }, group);
  removeNode(doc, group);
  assert.equal(getNode(doc, child), undefined, 'the child went with the group');
  assert.deepEqual(checkDocument(doc), []);
});

test('createProject starts with a valid, empty, correctly versioned project', () => {
  const project = createProject('Fresh');
  assert.equal(project.title, 'Fresh');
  assert.equal(project.schemaVersion, 1);
  assert.ok(parseId(project.id), 'the project has a real id');
  assert.equal(Object.keys(project.pages).length, 0);
});
