/**
 * The end-to-end product workflow: an idea becomes a designed storyboard, the user accepts it,
 * local artwork makes the pages readable, the result validates and exports, and every edit
 * remains editable.
 * @module test/workflow
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { validateBundle, errorsOf } from '../shared/domain/validate.js';
import { makeDrawing, ART_STYLES } from '../shared/domain/art.js';
import { designStoryboard } from '../shared/domain/story.js';
import { createStoryAgent } from '../server/ai/story.js';

const IDEA = `A rooftop at night. Maya looks over the sleeping city and sees a shadow move behind her.
"The city is quiet tonight," she says. A shape steps out of the dark.
She runs for the stairwell, and the door slams behind her. Somewhere below, an alarm starts.`;

test('an idea becomes a structured storyboard with a logline, style and characters', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Workflow' }));

  const designed = must(await session.run('comic.story.design', {
    ideas: IDEA,
    title: 'The Long Night',
    style: 'noir',
    audience: 'teen',
    characters: ['Maya: short black hair, red windbreaker'],
    maxPanelsPerPage: 3,
  }));

  assert.equal(designed.outline.title, 'The Long Night');
  assert.equal(designed.outline.style, 'noir');
  assert.equal(designed.design.provider, 'local-director');
  assert.equal(designed.design.model, 'deterministic-storyboard-v1');
  assert.ok(designed.outline.logline.length > 0);
  assert.ok(designed.outline.scenes.length >= 1);
  assert.ok(designed.outline.scenes.every((/** @type {any} */ scene) => scene.pages.length >= 1));

  const proposal = session.bundle.project.proposals[designed.proposalId];
  assert.equal(proposal.status, 'proposed');
  const panels = proposal.scenes.flatMap((/** @type {any} */ scene) => scene.pages.flatMap((/** @type {any} */ page) => page.panels));
  assert.ok(panels.length >= 3);
  for (const panel of panels) {
    assert.equal(typeof panel.shotDescription, 'string');
    assert.ok(panel.shotDescription.length > 0);
    assert.ok(panel.artworkPrompt.includes('noir'));
    assert.ok(Array.isArray(panel.lettering));
  }
  assert.ok(panels.some((/** @type {any} */ panel) => panel.lettering.some((/** @type {any} */ line) => line.kind === 'balloon')));
  assert.equal(Object.keys(session.bundle.project.pages).length, 0, 'design is inert until accepted');

  const quoted = designStoryboard('Maya sees a shape. "Who is there?" she asks.', { characters: ['Maya'] });
  const lines = quoted.proposal.scenes.flatMap((scene) => scene.pages.flatMap((page) => page.panels.flatMap((panel) => panel.lettering)));
  const balloon = lines.find((line) => line.kind === 'balloon');
  assert.ok(balloon, 'a question mark inside quotes does not break the dialogue into captions');
  assert.equal(balloon.text, 'Who is there?');
  assert.equal(balloon.character, 'Maya');
});

test('review can accept one scene, then the rest, without rewriting unrelated pages', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Scene review' }));
  const designed = must(await session.run('comic.story.design', {
    ideas: `A rooftop at night. Maya sees a shadow move.

An alley below. She runs past a neon sign.`,
    style: 'manga',
    characters: ['Maya'],
    maxPanelsPerPage: 2,
  }));
  const current = () => session.bundle.project.proposals[designed.proposalId];
  const proposal = current();
  assert.equal(proposal.scenes.length, 2);
  const [first, second] = proposal.scenes;

  const accepted = must(await session.run('comic.storyboard.accept', {
    proposalId: designed.proposalId, sceneKeys: [first.key],
  }));
  assert.equal(accepted.sceneCount, 1);
  assert.equal(current().status, 'proposed', 'a partial acceptance is not a finished review');
  assert.deepEqual(current().acceptedSceneKeys, [first.key]);
  assert.deepEqual(accepted.remainingSceneKeys, [second.key]);
  const firstSceneId = current().idMap.scenes[first.key];
  assert.equal(current().idMap.scenes[second.key], undefined, 'the second scene was not accepted');
  const firstPages = [...session.bundle.project.scenes[firstSceneId].pageIds];
  assert.ok(firstPages.length > 0);

  const unknown = refused(await session.run('comic.storyboard.accept', {
    proposalId: designed.proposalId, sceneKeys: ['nope'],
  }));
  assert.equal(unknown.kind, 'bad-params');

  const rest = must(await session.run('comic.storyboard.accept', {
    proposalId: designed.proposalId, sceneKeys: [second.key],
  }));
  assert.equal(rest.sceneCount, 1);
  assert.equal(current().status, 'approved');
  assert.deepEqual(current().acceptedSceneKeys, [first.key, second.key]);
  assert.deepEqual(session.bundle.project.scenes[firstSceneId].pageIds, firstPages, 'the accepted scene was untouched');
});

test('a silent designed character still enters the bible, and regenerating keeps the design brief', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Silent cast' }));
  const designed = must(await session.run('comic.story.design', {
    ideas: 'A rooftop at night. Maya waits alone while the city sleeps.',
    title: 'Quiet', style: 'noir', audience: 'teen',
    characters: ['Maya: short black hair, red windbreaker'],
    maxPanelsPerPage: 3,
  }));
  must(await session.run('comic.storyboard.accept', { proposalId: designed.proposalId }));
  const proposal = session.bundle.project.proposals[designed.proposalId];
  const maya = Object.values(session.bundle.project.characters).find((/** @type {any} */ c) => c.name === 'Maya');
  assert.ok(maya, 'the silent character was created from the cast');
  assert.match(/** @type {any} */ (maya).canonicalDescription, /red windbreaker/);
  assert.equal(proposal.idMap.characters.Maya, /** @type {any} */ (maya).id);

  const pagesBefore = Object.keys(session.bundle.project.pages).length;
  const regenerated = must(await session.run('comic.storyboard.generate', { proposalId: designed.proposalId }));
  assert.equal(regenerated.outline.style, 'noir');
  assert.equal(regenerated.outline.audience, 'teen');
  const next = session.bundle.project.proposals[designed.proposalId];
  assert.deepEqual(next.design.characters, ['Maya: short black hair, red windbreaker']);
  assert.ok(next.scenes.reduce((n, scene) => n + scene.pages.length, 0) >= pagesBefore, 'the designed structure was not collapsed');
});

test('the accepted comic can be drawn locally, edited, validated, exported and packaged', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Produce' }));
  const designed = must(await session.run('comic.story.design', {
    ideas: IDEA, style: 'watercolour', characters: ['Maya: short black hair, red windbreaker'], maxPanelsPerPage: 3,
  }));
  must(await session.run('comic.storyboard.accept', { proposalId: designed.proposalId }));
  assert.ok(Object.keys(session.bundle.project.pages).length > 0);
  const maya = Object.values(session.bundle.project.characters).find((/** @type {any} */ character) => character.name === 'Maya');
  assert.ok(maya, 'the named character entered the bible');
  assert.match(/** @type {any} */ (maya).canonicalDescription, /red windbreaker|short black hair/);

  const panelIds = Object.keys(session.bundle.project.panels);
  const painted = must(await session.run('comic.batch.run', {
    commands: panelIds.map((panelId) => ({
      command: 'comic.artwork.paint',
      params: { panelId, style: 'watercolour' },
    })),
  }));
  assert.equal(painted.failed, 0);
  for (const panelId of panelIds) {
    const panel = session.bundle.project.panels[panelId];
    assert.equal(panel.artworkRefs.length, 1, `panel ${panelId} has artwork`);
    const node = session.bundle.document.nodes[String(panel.artworkRefs[0])];
    assert.equal(node.kind, 'image');
    const asset = session.bundle.project.assets[node.assetId];
    assert.ok(asset.data.startsWith('data:image/svg+xml'), 'local art is a real, editable asset');
    assert.equal(asset.provenance.provider, 'local-vector');
  }

  // Edit a line and move it; the workflow must leave ordinary editing in charge.
  const letteringId = Object.keys(session.bundle.project.lettering)[0];
  must(await session.run('comic.lettering.setText', { letteringId, text: 'A REWRITTEN LINE.' }));
  must(await session.run('comic.lettering.move', { letteringId, dx: 4, dy: 2 }));
  must(await session.run('comic.edit.undo', {}));

  const validated = must(await session.run('comic.project.validate', {}));
  assert.equal(validated.errors, 0, `the comic has errors: ${JSON.stringify(errorsOf(validateBundle(session.bundle)))}`);

  const exported = must(await session.run('comic.book.export', {
    path: 'out/workflow', format: 'images', preset: 'proofing', scale: 0.2,
  }));
  assert.equal(exported.pages, Object.keys(session.bundle.project.pages).length);
  assert.ok(exported.files.some((/** @type {any} */ file) => file.path.endsWith('contact-sheet.png')));

  must(await session.run('comic.project.package', { path: 'pkg/workflow.comiccraft' }));
  must(await session.run('comic.project.unpack', { archivePath: 'pkg/workflow.comiccraft', path: 'reopened' }));
  const pageIds = Object.keys(session.bundle.project.pages).sort();
  must(await session.run('comic.project.open', { path: 'reopened' }));
  assert.deepEqual(Object.keys(session.bundle.project.pages).sort(), pageIds);
  assert.equal(session.bundle.project.assets[Object.keys(session.bundle.project.assets)[0]].data.startsWith('data:image/svg+xml'), true);
});

test('local design clamps every field the committed proposal schema caps', async () => {
  const { designStoryboard, validateProposal } = await import('../shared/domain/story.js');
  const { check } = await import('../shared/commands/validate-params.js');
  const { readFile } = await import('node:fs/promises');
  const schema = JSON.parse(await readFile(new URL('../schema/v1/proposal.schema.json', import.meta.url), 'utf8'));
  const long = designStoryboard(`SCENE: ${'X'.repeat(300)}\nMAYA: ${'Y'.repeat(5000)}`, {
    title: 'T'.repeat(300), characters: ['Maya: red coat'], maxPanelsPerPage: 3,
  });
  assert.equal(validateProposal(long.proposal).filter((p) => p.severity === 'error').length, 0);
  assert.equal(check(schema, long.proposal, 'proposal'), null, 'the proposal satisfies the committed schema');
});

test('local drawings are deterministic and every advertised style is distinct', () => {
  const first = makeDrawing({ prompt: 'A rooftop at night', style: 'noir', width: 400, height: 300, seed: 'panel_1' });
  const second = makeDrawing({ prompt: 'A rooftop at night', style: 'noir', width: 400, height: 300, seed: 'panel_1' });
  assert.equal(first.data, second.data);
  const hashes = new Set(ART_STYLES.map((style) => makeDrawing({ prompt: 'A rooftop', style, seed: 'panel_1' }).data));
  assert.equal(hashes.size, ART_STYLES.length, 'the styles do not collapse into one palette');
});

test('the optional HTTP story agent is untrusted input: it normalises and validates, or refuses', async (t) => {
  const originalFetch = globalThis.fetch;
  t.after(() => { globalThis.fetch = originalFetch; });

  globalThis.fetch = (async () => ({
    ok: true,
    json: async () => ({
      choices: [{ message: { content: JSON.stringify({
        title: 'Model Comic',
        logline: 'A model-designed story.',
        scenes: [{
          key: 's1', name: 'Rooftop', synopsis: 'Maya waits.',
          pages: [{ key: 'p1', name: 'Page 1', template: 'single', panels: [{
            key: 'pn1', shotDescription: 'Maya on a roof.', camera: 'wide', characters: ['Maya'],
            artworkPrompt: 'Maya on a roof, noir',
            lettering: [{ key: 'l1', text: 'Quiet.', kind: 'caption', variant: null, character: null }],
          }] }],
        }],
      }) } }],
    }),
  }));

  const agent = createStoryAgent({ endpoint: 'https://example.invalid/story', apiKey: 'test-key', model: 'fake-model' });
  const designed = await agent.design('A rooftop at night.', {
    provider: 'http', style: 'noir', characters: ['Maya'], maxPanelsPerPage: 3, now: 5, baseRevision: 2,
  });
  assert.equal(designed.ok, true);
  if (!designed.ok) return;
  assert.equal(designed.proposal.design.provider, 'http');
  assert.equal(designed.proposal.design.model, 'fake-model');
  assert.equal(designed.proposal.scenes[0].pages[0].panels[0].lettering[0].kind, 'caption');

  globalThis.fetch = (async () => ({
    ok: true,
    json: async () => ({ choices: [{ message: { content: 'not json at all' } }] }),
  }));
  const refusedResult = await agent.design('A rooftop.', { provider: 'http' });
  assert.equal(refusedResult.ok, false);
  assert.match(/** @type {any} */ (refusedResult).message, /parsable JSON|invalid/);
  // The failure is a value, not a throw, and no proposal was created.
  assert.equal(refusedResult.ok, false);
});
