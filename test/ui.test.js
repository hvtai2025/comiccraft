/**
 * The M6 UI check, runnable in the gate.
 *
 * The browser is not launched in CI (AGENTS.md: the gate must never need a browser), so this
 * checks the properties that make the studio honest without one: every M1 operation has a
 * control, panels act only through commands, no browser prompt is required, and the client
 * imports no server module. The interactive walkthrough is recorded in the milestone report.
 * @module test/ui
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const html = await readFile(join(root, 'public', 'index.html'), 'utf8');
const app = await readFile(join(root, 'public', 'app.js'), 'utf8');
const css = await readFile(join(root, 'public', 'styles', 'app.css'), 'utf8');

const REQUIRED_IDS = [
  'canvas', 'projectTitle', 'undoBtn', 'redoBtn', 'saveBtn', 'openBtn', 'newBtn',
  'viewCanvasBtn', 'viewStoryboardBtn', 'viewReviewBtn',
  'workflow', 'workflowHint', 'designBtn', 'artStyle', 'artPanelBtn', 'artAllBtn',
  'storyNav', 'addChapterBtn', 'addSceneBtn', 'pageList', 'addPageBtn', 'templateSelect',
  'characterList', 'addCharacterBtn', 'assetList', 'uploadAssetBtn',
  'panelAddBtn', 'balloonAddBtn', 'thoughtAddBtn', 'captionAddBtn', 'sfxAddBtn',
  'artworkPlaceBtn', 'cropBtn', 'clipBtn', 'lockBtn', 'deleteBtn',
  'inspector', 'typography', 'readingOrder', 'storyboard',
  'proposalList', 'jobList', 'reviewQueue', 'reviewProblems',
  'generatePrompt', 'generateBtn', 'publicationPreset', 'validatePublicationBtn', 'publication',
  'problems', 'zoom', 'zoomLabel',
];

test('every workspace panel and tool the milestone promises exists in the shell', () => {
  for (const id of REQUIRED_IDS) {
    assert.ok(html.includes(`id="${id}"`), `the shell is missing #${id}`);
  }
  assert.ok(html.includes('role="toolbar"'), 'the canvas tools are a toolbar');
  assert.equal((html.match(/class="workflow-step"/g) ?? []).length, 6, 'the workflow has six steps');
  assert.ok(css.includes('.storyboard-page'), 'the storyboard has a presentation');
  assert.ok(css.includes('.card.pending'), 'pending work is visually distinct');
});

test('every M1 operation and every M2–M8 workspace action is dispatched as a command', () => {
  const commands = [
    'comic.project.create',
    'comic.page.add', 'comic.panel.add', 'comic.panel.setFrame', 'comic.panel.delete',
    'comic.panel.setLocked', 'comic.artwork.place', 'comic.artwork.crop', 'comic.artwork.clip',
    'comic.lettering.add', 'comic.lettering.edit', 'comic.lettering.setFrame',
    'comic.edit.undo', 'comic.edit.redo',
    'comic.story.design', 'comic.story.import', 'comic.storyboard.accept', 'comic.storyboard.reject',
    'comic.asset.register', 'comic.asset.approve', 'comic.asset.reject',
    'comic.job.submit', 'comic.job.apply', 'comic.job.reject', 'comic.job.cancel',
    'comic.reading.set', 'comic.chapter.add', 'comic.scene.add', 'comic.character.add',
    'comic.book.validate',
  ];
  for (const id of commands) {
    assert.ok(app.includes(`'${id}'`), `the UI never dispatches ${id}`);
  }
  assert.ok(app.includes('dispatch(registry'), 'the studio uses the shared dispatcher');
  assert.ok(app.includes('serialize(state.bundle)'), 'local save uses the same serializer as the server');
  assert.ok(app.includes('adoptBundle('), 'adopting a project clears the previous undo history');
  assert.ok(app.includes('saveKey('), 'a new project gets a unique local save key instead of overwriting comic-project_1');
  assert.ok(!/from '\.\.\/server\//.test(app), 'the client imports no server module');
  assert.ok(!html.includes('onclick='), 'no inline comic logic in the shell');
});

test('the studio never blocks on a browser prompt and never ships a provider key path', () => {
  assert.ok(!app.includes('window.prompt'), 'an inline dialog replaced window.prompt');
  assert.ok(app.includes('askText('), 'the inline dialog is present');
  assert.ok(app.includes('askDesign('), 'the idea-to-storyboard form is present');
  assert.ok(!/COMICCRAFT_AI_(KEY|ENDPOINT)/.test(app), 'credentials are not reachable from the client');
  assert.ok(!app.includes('fetch('), 'the client does not call a provider');
});
