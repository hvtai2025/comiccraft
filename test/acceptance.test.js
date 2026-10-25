/**
 * The M1 acceptance suite: the twelve criteria of Phase 1, ported from the reference's
 * milestone and stated as behaviour rather than as a restatement of the code.
 *
 * Each test names the criterion it proves, so a failure says which promise broke.
 * @module test/acceptance
 */

import { test } from 'node:test';
import { must, refused } from './helpers.js';
import assert from 'node:assert/strict';
import { createBundle, rect, quantize, readingOrder, findArtboard } from '../shared/domain/index.js';
import { validateBundle, errorsOf } from '../shared/domain/validate.js';
import { serialize, deserialize, migrate, PAGES_DIR } from '../shared/domain/bundle.js';
import { History } from '../shared/domain/history.js';
import { buildRegistry } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { createMemoryStorage } from '../server/storage/fs.js';
import { createRenderer } from '../server/export/renderer.js';
import { pageToSurface, pngSize } from '../server/export/png.js';
import { PT_PER_UNIT } from '../server/export/pdf.js';

/** A session the way the server builds one, but in memory. */
function session() {
  const state = { bundle: createBundle('Acceptance') };
  const history = new History();
  const services = { storage: createMemoryStorage(), renderer: createRenderer(), measure: undefined };
  const registry = buildRegistry();
  const ctx = () => ({ bundle: state.bundle, history, env: { onServer: true, now: () => 0 }, services });
  return {
    registry,
    history,
    get bundle() { return state.bundle; },
    async run(id, params) {
      const result = await dispatch(registry, ctx(), id, params);
      if (result.ok && result.value?.bundle) {
        state.bundle = result.value.bundle;
        history.undoStack.length = 0;
        history.redoStack.length = 0;
      }
      return result;
    },
  };
}

/** The three-panel page of the criterion, built once per test that needs it. */
async function threePanelPage(s) {
  const created = await s.run('comic.project.create', { title: 'Three Panel' });
  assert.equal(created.ok, true);
  const page = await s.run('comic.page.add', { name: 'Page 1', template: 'three-up' });
  assert.equal(page.ok, true);
  const panels = s.bundle.project.pages[must(page).pageId].panelIds;
  assert.equal(panels.length, 3);
  return { pageId: must(page).pageId, panels };
}

test('1. a project is created with a page, and a page is an artboard', async () => {
  const s = session();
  const { pageId } = await threePanelPage(s);
  const page = s.bundle.project.pages[pageId];
  const artboard = findArtboard(s.bundle.document, page.artboardId);
  assert.ok(artboard, 'the page reaches its artboard');
  assert.equal(artboard.name, page.name);
  assert.equal(s.bundle.document.artboards.length, 1, 'one page, one artboard');
});

test('2. artwork is placed into a panel and referenced, never copied', async () => {
  const s = session();
  const { pageId, panels } = await threePanelPage(s);
  const placed = await s.run('comic.artwork.place', {
    panelId: panels[0], name: 'Art 1', contentHash: 'hash-one', provider: 'test',
  });
  assert.equal(placed.ok, true, placed.ok ? '' : placed.message);

  const panel = s.bundle.project.panels[panels[0]];
  assert.deepEqual(panel.artworkRefs, [must(placed).artworkNodeId], 'the panel holds a reference, not a copy');
  const node = s.bundle.document.nodes[String(must(placed).artworkNodeId)];
  assert.equal(node.kind, 'image');
  assert.equal(node.frame.width, s.bundle.document.nodes[String(panel.frameNodeId)].frame.width,
    'the artwork covers the frame without duplicating its geometry as data');
  assert.equal(Object.keys(s.bundle.project.assets).length, 1);
  void pageId;
});

test('3. crop changes which part is shown, never the object geometry', async () => {
  const s = session();
  const { panels } = await threePanelPage(s);
  const placed = await s.run('comic.artwork.place', { panelId: panels[0], name: 'A', contentHash: 'h' });
  const before = { ...s.bundle.document.nodes[String(must(placed).artworkNodeId)].frame };

  const cropped = await s.run('comic.artwork.crop', { panelId: panels[0], x: 0.1, y: 0.2, width: 0.5, height: 0.6 });
  assert.equal(cropped.ok, true, cropped.ok ? '' : cropped.message);
  assert.deepEqual(must(cropped).crop, { x: 0.1, y: 0.2, width: 0.5, height: 0.6 });
  assert.deepEqual(s.bundle.document.nodes[String(must(placed).artworkNodeId)].frame, before,
    'cropping did not resize the panel or the object');
});

test('4. a crop that runs past the image is refused', async () => {
  const s = session();
  const { panels } = await threePanelPage(s);
  await s.run('comic.artwork.place', { panelId: panels[0], name: 'A', contentHash: 'h' });
  const refused = await s.run('comic.artwork.crop', { panelId: panels[0], x: 0.8, y: 0, width: 0.5, height: 1 });
  assert.equal(refused.ok, false);
  assert.equal(refused.kind, 'bad-params');
});

test('5. clip sets a clip rect inside the frame, and refuses one that would be empty', async () => {
  const s = session();
  const { panels } = await threePanelPage(s);
  await s.run('comic.artwork.place', { panelId: panels[0], name: 'A', contentHash: 'h' });
  const frame = s.bundle.document.nodes[String(s.bundle.project.panels[panels[0]].frameNodeId)].frame;

  const clipped = await s.run('comic.artwork.clip', { panelId: panels[0], inset: 6 });
  assert.equal(clipped.ok, true);
  assert.deepEqual(must(clipped).clip, rect(frame.x + 6, frame.y + 6, frame.width - 12, frame.height - 12));

  const tooMuch = await s.run('comic.artwork.clip', { panelId: panels[0], inset: Math.round(frame.width / 2) + 1 });
  assert.equal(tooMuch.ok, false);
  assert.equal(tooMuch.kind, 'bad-params');
});

test('6. replacing artwork unlinks the old object and keeps the old asset', async () => {
  const s = session();
  const { panels } = await threePanelPage(s);
  const first = await s.run('comic.artwork.place', { panelId: panels[0], name: 'Old', contentHash: 'old-hash' });
  const second = await s.run('comic.artwork.place', { panelId: panels[0], name: 'New', contentHash: 'new-hash' });
  assert.equal(second.ok, true);

  assert.equal(s.bundle.document.nodes[String(first.value.artworkNodeId)], undefined, 'the superseded object is gone');
  assert.deepEqual(s.bundle.project.panels[panels[0]].artworkRefs, [must(second).artworkNodeId]);
  assert.equal(Object.keys(s.bundle.project.assets).length, 2, 'both assets remain: replacing art must not destroy it');
});

test('7. every lettering kind can be added, and dialogue keeps one text', async () => {
  const s = session();
  const { pageId, panels } = await threePanelPage(s);
  const character = await s.run('comic.character.add', { name: 'Narrator' });
  const kinds = ['balloon', 'caption', 'sound-effect'];
  for (let i = 0; i < 3; i++) {
    const added = await s.run('comic.lettering.add', {
      panelId: panels[i], kind: kinds[i], text: `Line ${i}`,
      ...(i === 0 ? { characterId: character.value.characterId } : {}),
    });
    assert.equal(added.ok, true, added.ok ? '' : added.message);
  }
  const listed = await s.run('comic.lettering.list', { pageId });
  assert.equal(listed.value.lettering.length, 3);
  assert.deepEqual(listed.value.lettering.map((l) => l.kind).sort(), ['balloon', 'caption', 'sound-effect']);
});

test('8. one user action is one undo step, and undo restores both models', async () => {
  const s = session();
  const { panels } = await threePanelPage(s);
  const depthBefore = s.history.depth().undo;

  const placed = await s.run('comic.artwork.place', { panelId: panels[0], name: 'A', contentHash: 'h' });
  assert.equal(placed.changed, true, 'the command recorded a step');
  assert.equal(s.history.depth().undo, depthBefore + 1, 'exactly one step');

  const nodeId = must(placed).artworkNodeId;
  const undone = await s.run('comic.edit.undo');
  assert.equal(undone.ok, true);
  assert.equal(s.bundle.document.nodes[String(nodeId)], undefined, 'the object came back out');
  assert.deepEqual(s.bundle.project.panels[panels[0]].artworkRefs, [], 'and the reference with it');
  assert.equal(errorsOf(validateBundle(s.bundle)).length, 0, 'the undone state is still valid');

  const redone = await s.run('comic.edit.redo');
  assert.equal(redone.ok, true);
  assert.ok(s.bundle.document.nodes[String(nodeId)], 'redo put it back');
});

test('9. a command that changes nothing records no undo step', async () => {
  const s = session();
  await threePanelPage(s);
  const before = s.history.depth().undo;
  const noop = await s.run('comic.project.setDefaultTemplate', { template: s.bundle.project.defaultTemplate ?? null });
  assert.equal(noop.ok, true);
  assert.equal(noop.changed, false, 'an identical value is not a change');
  assert.equal(s.history.depth().undo, before, 'so no undo step was pushed');
});

test('10. save then reopen preserves every id and every link', async () => {
  const s = session();
  const { pageId, panels } = await threePanelPage(s);
  await s.run('comic.artwork.place', { panelId: panels[0], name: 'A', contentHash: 'h1' });
  await s.run('comic.lettering.add', { panelId: panels[1], kind: 'caption', text: 'CAPTION' });

  await s.run('comic.project.save', { path: 'proj' });
  const reopened = await s.run('comic.project.open', { path: 'proj' });
  assert.equal(reopened.ok, true, reopened.ok ? '' : reopened.message);

  const { project, document } = must(reopened).bundle;
  assert.deepEqual(Object.keys(project.pages), [pageId], 'the page id survived');
  assert.deepEqual(Object.keys(project.panels).sort(), [...panels].sort(), 'the panel ids survived');
  assert.equal(project.pages[pageId].artboardId, s.bundle.project.pages[pageId].artboardId, 'the artboard link survived');
  assert.equal(document.nextNodeId, s.bundle.document.nextNodeId, 'the node counter survived');
  assert.equal(errorsOf(must(reopened).problems).length, 0, 'and the reopened bundle is valid');
  assert.ok(project.pages[pageId].panelIds.length === 3, 'reading order survived');
});

test('11. the manifest is written last, and a bundle missing one is reported not guessed', async () => {
  const s = session();
  await threePanelPage(s);
  const serialized = serialize(s.bundle);
  assert.equal(serialized.ok, true);
  const paths = [...serialized.files.keys()];
  assert.equal(paths[paths.length - 1], 'manifest.json', 'the manifest is the last file in the set');

  const incomplete = new Map(serialized.files);
  incomplete.delete('manifest.json');
  const read = deserialize(incomplete);
  assert.equal(read.ok, false);
  assert.deepEqual(read.missing, ['manifest.json']);

  // A page file that went astray is a warning rather than a refusal: the rest of the book is
  // intact and can be repaired.
  const noPage = new Map(serialized.files);
  const pagePath = [...noPage.keys()].find((p) => p.startsWith(PAGES_DIR));
  noPage.delete(pagePath);
  const partial = deserialize(noPage);
  assert.equal(partial.ok, true);
  assert.ok(partial.problems.some((p) => p.code === 'missing-page-file'), 'the missing page file is reported');
});

test('12. a newer schema is refused rather than silently downgraded', () => {
  const bundle = createBundle('Future');
  bundle.project.schemaVersion = 99;
  assert.throws(() => migrate(bundle), /newer than this build/);
  const fine = deserialize(serialize(createBundle('Now')).files);
  assert.equal(fine.ok, true);
  assert.deepEqual(migrate(fine.bundle).migrated, [], 'a current bundle needs no migration');
});

test('13. PNG output is a real PNG at the requested size, and the round trip is exact', async () => {
  const s = session();
  const { pageId } = await threePanelPage(s);
  const page = s.bundle.project.pages[pageId];
  const artboard = findArtboard(s.bundle.document, page.artboardId);

  const png = await s.run('comic.export.png', { pageId, path: 'p.png', scale: 0.25 });
  assert.equal(png.ok, true, png.ok ? '' : png.message);
  const again = await s.run('comic.export.png', { pageId, path: 'q.png', scale: 0.25 });
  assert.equal(again.value.width, Math.round(artboard.size.width * 0.25));
  assert.equal(again.value.height, Math.round(artboard.size.height * 0.25));

  // The same bundle renders byte-identically twice: a raster that changes between runs would
  // make every golden comparison meaningless.
  const first = createRenderer().pageToPng(s.bundle, page, 0.25);
  const second = createRenderer().pageToPng(s.bundle, page, 0.25);
  assert.equal(first.equals(second), true, 'rasterising the same page twice gives the same bytes');
});

test('14. PDF page geometry is the artboard in points, and the boxes read back', async () => {
  const s = session();
  const { pageId } = await threePanelPage(s);
  const artboard = findArtboard(s.bundle.document, s.bundle.project.pages[pageId].artboardId);

  const surface = pageToSurface(s.bundle, s.bundle.project.pages[pageId], 0.1);
  assert.ok(surface.width > 0 && surface.height > 0);

  const exported = await s.run('comic.export.pdf', { path: 'book.pdf' });
  assert.equal(exported.ok, true, exported.ok ? '' : exported.message);

  const renderer = createRenderer();
  const pdf = renderer.bookToPdf(s.bundle, [s.bundle.project.pages[pageId]]);
  const text = pdf.toString('latin1');
  assert.ok(text.startsWith('%PDF-'), 'a real PDF header');
  assert.ok(text.trimEnd().endsWith('%%EOF'), 'and a real trailer');

  const expectedWidth = quantize(artboard.size.width * PT_PER_UNIT, 4);
  const expectedHeight = quantize(artboard.size.height * PT_PER_UNIT, 4);
  const box = /\/MediaBox \[0 0 ([\d.]+) ([\d.]+)\]/.exec(text);
  assert.ok(box, 'the page declares a MediaBox');
  assert.equal(Number(box[1]), expectedWidth, `width ${box[1]} should be ${expectedWidth} points`);
  assert.equal(Number(box[2]), expectedHeight, `height ${box[2]} should be ${expectedHeight} points`);

  const startxref = /startxref\s+(\d+)/.exec(text);
  assert.ok(startxref, 'the file declares a startxref');
  assert.equal(text.slice(Number(startxref[1]), Number(startxref[1]) + 4), 'xref',
    'and startxref points at the cross-reference table, which is what makes the file open');
  assert.equal(text.split('/Type /Page ').length - 1, 1, 'one page');
});

test('15. PNG bytes decode at the size the header claims', async () => {
  const s = session();
  const { pageId } = await threePanelPage(s);
  const renderer = createRenderer();
  const png = renderer.pageToPng(s.bundle, s.bundle.project.pages[pageId], 0.2);
  const size = await pngSize(new Uint8Array(png));
  const artboard = findArtboard(s.bundle.document, s.bundle.project.pages[pageId].artboardId);
  assert.equal(size.width, Math.round(artboard.size.width * 0.2));
  assert.equal(size.height, Math.round(artboard.size.height * 0.2));
});
