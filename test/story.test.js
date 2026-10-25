/**
 * M3's exit criteria: a story becomes structured data with stable ids, every proposal is
 * reviewable and reversible, accepting one scene leaves the others untouched, and
 * regenerating descriptions preserves approved content.
 * @module test/story
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { validateBundle } from '../shared/domain/validate.js';
import { analyzeStory, classifyLine, fingerprint } from '../shared/domain/story.js';

const STORY = `# The Long Night

SCENE: Rooftop
Maya looks over the sleeping city.
MAYA: The city is quiet tonight.
CAPTION: Too quiet.

A shadow moves behind her.
MAYA: Who is there?

SCENE: Alley
MAYA: Run!
SFX: KRAKOOM!
`;

test('the analyser returns structured, validated data — never prose the editor must parse', () => {
  const { proposal, problems } = analyzeStory(STORY, { now: 1, baseRevision: 3 });
  assert.equal(problems.filter((p) => p.severity === 'error').length, 0);
  assert.equal(proposal.scenes.length, 2);
  assert.equal(proposal.scenes[0].name, 'Rooftop');
  assert.equal(proposal.scenes[1].name, 'Alley');
  const panels = proposal.scenes.flatMap((s) => s.pages.flatMap((p) => p.panels));
  assert.equal(panels.length, 3);
  for (const panel of panels) {
    assert.equal(typeof panel.shotDescription, 'string');
    assert.ok(panel.key.length > 0);
    assert.ok(Array.isArray(panel.lettering));
    for (const line of panel.lettering) {
      assert.ok(['balloon', 'caption', 'sound-effect'].includes(line.kind));
      assert.equal(typeof line.text, 'string');
    }
  }
  assert.equal(fingerprint('abc'), fingerprint('abc'));
  assert.notEqual(fingerprint('abc'), fingerprint('abd'));
  assert.deepEqual(classifyLine('MAYA: LOOK OUT!'), { kind: 'balloon', variant: 'shout', character: 'MAYA', text: 'LOOK OUT!' });
  assert.equal(classifyLine('CAPTION: Later.').kind, 'caption');
  assert.equal(classifyLine('SFX: BOOM').kind, 'sound-effect');
  assert.equal(classifyLine('(She remembers.)').variant, 'thought');
});

test('import stores a proposal and creates no pages; acceptance is the only authoritative step', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Story' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  assert.equal(imported.outline.scenes.length, 2);
  assert.equal(Object.keys(session.bundle.project.pages).length, 0, 'review comes before authority');
  assert.equal(Object.keys(session.bundle.project.scenes).length, 0);
  assert.equal(session.bundle.project.proposals[imported.proposalId].status, 'proposed');

  const accepted = must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));
  assert.equal(accepted.sceneCount, 2);
  assert.equal(Object.keys(session.bundle.project.pages).length, 2);
  assert.equal(Object.keys(session.bundle.project.panels).length, 3);
  assert.ok(accepted.characters.length >= 1, 'Maya is created once');
  assert.equal(session.bundle.project.proposals[imported.proposalId].status, 'approved');
  assert.deepEqual(validateBundle(session.bundle).filter((p) => p.severity === 'error'), []);
});

test('a scene is isolated: accepting one leaves the other page byte-for-byte identical', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Isolation' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));

  const proposal = session.bundle.project.proposals[imported.proposalId];
  const rooftopKey = proposal.scenes[0].key;
  const alleyKey = proposal.scenes[1].key;
  const rooftopSceneId = proposal.idMap.scenes[rooftopKey];
  const rooftopPageIds = session.bundle.project.scenes[rooftopSceneId].pageIds;
  const before = structuredClone(rooftopPageIds.map((/** @type {string} */ id) => ({
    page: session.bundle.project.pages[id],
    panels: session.bundle.project.pages[id].panelIds.map((/** @type {string} */ p) => session.bundle.project.panels[p]),
  })));

  // Change the *other* scene in a regenerated proposal and accept only that scene.
  proposal.scenes[1].pages[0].panels[0].shotDescription = 'A very different alley.';
  const accepted = must(await session.run('comic.storyboard.accept', {
    proposalId: imported.proposalId, sceneKeys: [alleyKey],
  }));
  assert.equal(accepted.sceneCount, 1);

  const after = rooftopPageIds.map((/** @type {string} */ id) => ({
    page: session.bundle.project.pages[id],
    panels: session.bundle.project.pages[id].panelIds.map((/** @type {string} */ p) => session.bundle.project.panels[p]),
  }));
  assert.deepEqual(after, before, 'the unselected scene was not rewritten');
});

test('regenerate-and-accept keeps every page and panel id', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Stable ids' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));

  const pageIds = Object.keys(session.bundle.project.pages).sort();
  const panelIds = Object.keys(session.bundle.project.panels).sort();

  const regenerated = must(await session.run('comic.storyboard.generate', { proposalId: imported.proposalId }));
  assert.deepEqual(Object.values(regenerated.carriedIds.pages).sort(), pageIds);
  assert.deepEqual(Object.values(regenerated.carriedIds.panels).sort(), panelIds);

  const accepted = must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));
  assert.deepEqual(Object.keys(session.bundle.project.pages).sort(), pageIds, 'no page was renumbered');
  assert.deepEqual(Object.keys(session.bundle.project.panels).sort(), panelIds, 'no panel was renumbered');
  assert.ok(accepted.updatedPanels.length > 0, 'descriptions were refreshed');
});

test('regenerating a description never overwrites approved lettering or geometry', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Preserve' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));

  const letteringId = Object.keys(session.bundle.project.lettering)[0];
  must(await session.run('comic.lettering.setText', { letteringId, text: 'APPROVED NEW LINE' }));
  const panelId = Object.values(session.bundle.project.panels)
    .find((/** @type {any} */ p) => p.letteringIds.includes(letteringId)).id;
  const panel = session.bundle.project.panels[panelId];
  const frameBefore = { ...session.bundle.document.nodes[String(panel.frameNodeId)].frame };

  const regenerated = must(await session.run('comic.storyboard.generate', { proposalId: imported.proposalId }));
  assert.ok(regenerated.proposalId);
  must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));

  assert.equal(session.bundle.project.lettering[letteringId].text, 'APPROVED NEW LINE');
  assert.deepEqual(session.bundle.document.nodes[String(panel.frameNodeId)].frame, frameBefore);
});

test('duplicate scene names keep distinct ids across regenerate-and-accept', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Duplicate names' }));
  const duplicated = `SCENE: Rooftop\nMAYA: First roof.\n\nSCENE: Rooftop\nMAYA: Second roof.\n`;
  const imported = must(await session.run('comic.story.import', { text: duplicated }));
  must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));
  const proposal = session.bundle.project.proposals[imported.proposalId];
  const [firstKey, secondKey] = proposal.scenes.map((/** @type {any} */ s) => s.key);
  const firstId = proposal.idMap.scenes[firstKey];
  const secondId = proposal.idMap.scenes[secondKey];
  assert.notEqual(firstId, secondId, 'the two identical names are two scenes');

  const regenerated = must(await session.run('comic.storyboard.generate', { proposalId: imported.proposalId }));
  assert.equal(regenerated.carriedIds.scenes[firstKey], firstId);
  assert.equal(regenerated.carriedIds.scenes[secondKey], secondId, 'the second scene did not map to the first');

  const firstPages = [...session.bundle.project.scenes[firstId].pageIds];
  const accepted = must(await session.run('comic.storyboard.accept', {
    proposalId: imported.proposalId, sceneKeys: [secondKey],
  }));
  assert.deepEqual(accepted.scenes, [secondId]);
  assert.deepEqual(session.bundle.project.scenes[firstId].pageIds, firstPages, 'the first scene was untouched');
});

test('rebuilding a page keeps its artboard size and reading direction', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Rebuild' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  must(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));
  const pageId = Object.keys(session.bundle.project.pages)[0];
  must(await session.run('comic.page.setSize', { pageId, width: 500, height: 700 }));
  must(await session.run('comic.reading.setDirection', { direction: 'rtl' }));

  must(await session.run('comic.storyboard.generate', { proposalId: imported.proposalId }));
  must(await session.run('comic.storyboard.accept', {
    proposalId: imported.proposalId, preserveExisting: false,
  }));
  const page = session.bundle.project.pages[pageId];
  const artboard = session.bundle.document.artboards.find((/** @type {any} */ a) => a.id === page.artboardId);
  assert.deepEqual(artboard.size, { width: 500, height: 700 });
  for (const panelId of page.panelIds) {
    const frame = session.bundle.document.nodes[String(session.bundle.project.panels[panelId].frameNodeId)].frame;
    assert.ok(frame.x >= 0 && frame.y >= 0 && frame.x + frame.width <= 500.0001 && frame.y + frame.height <= 700.0001,
      `panel ${panelId} is inside the page: ${JSON.stringify(frame)}`);
  }
  const frames = page.panelIds.map((/** @type {string} */ id) => session.bundle.document.nodes[String(session.bundle.project.panels[id].frameNodeId)].frame);
  if (frames.length > 1) assert.ok(frames[0].x > frames[1].x, 'the first panel is on the right in an rtl page');
});

test('undoing import removes the proposal, and undoing accept restores its proposed state', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Undo story' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  assert.ok(session.bundle.project.proposals[imported.proposalId]);
  must(await session.run('comic.edit.undo', {}));
  assert.equal(session.bundle.project.proposals[imported.proposalId], undefined, 'an import is undoable');

  const again = must(await session.run('comic.story.import', { text: STORY }));
  must(await session.run('comic.storyboard.accept', { proposalId: again.proposalId }));
  assert.equal(session.bundle.project.proposals[again.proposalId].status, 'approved');
  must(await session.run('comic.edit.undo', {}));
  assert.equal(Object.keys(session.bundle.project.pages).length, 0, 'the accepted pages are removed');
  assert.equal(session.bundle.project.proposals[again.proposalId].status, 'proposed', 'the proposal is reviewable again');
});

test('a rejected proposal creates nothing and can be replaced by a fresh one', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Reject' }));
  const imported = must(await session.run('comic.story.import', { text: STORY }));
  const rejected = must(await session.run('comic.storyboard.reject', { proposalId: imported.proposalId, note: 'not yet' }));
  assert.equal(rejected.status, 'rejected');
  assert.equal(Object.keys(session.bundle.project.pages).length, 0);

  const refusedAccept = refused(await session.run('comic.storyboard.accept', { proposalId: imported.proposalId }));
  assert.equal(refusedAccept.kind, 'conflict');

  const second = must(await session.run('comic.story.import', { text: STORY }));
  assert.notEqual(second.proposalId, imported.proposalId, 'a fresh proposal gets a fresh id');
  must(await session.run('comic.storyboard.accept', { proposalId: second.proposalId }));
  assert.equal(Object.keys(session.bundle.project.pages).length, 2);
});
