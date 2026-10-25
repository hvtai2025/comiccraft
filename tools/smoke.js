#!/usr/bin/env node
/**
 * The smoke test: boot the real server, exercise the real HTTP surface, and prove the client
 * and the shared domain are actually served.
 *
 * This is the only check that runs the thing the user runs. Every other gate is a unit or a
 * static check, and a project can pass all of them with a server that does not start.
 *
 * Usage: node tools/smoke.js
 */

import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import Fastify from 'fastify';
import { createSession } from '../server/session.js';
import { registerRoutes } from '../server/http.js';

const PORT = 0; // let the OS choose, so the smoke test never collides with a running server

const root = await mkdtemp(join(tmpdir(), 'comiccraft-smoke-'));
const app = Fastify({ logger: false });
const session = createSession({ root });
registerRoutes(app, session, { publicDir: join(import.meta.dirname, '..', 'public'), sharedDir: join(import.meta.dirname, '..', 'shared') });

/** @type {string[]} */
const failures = [];
/** @param {string} what @param {boolean} ok @param {string} [detail] */
const check = (what, ok, detail = '') => {
  if (ok) {
    console.log(`  ok   ${what}`);
  } else {
    console.log(`  FAIL ${what}${detail ? ` — ${detail}` : ''}`);
    failures.push(what);
  }
};

try {
  await app.listen({ port: PORT, host: '127.0.0.1' });
  const base = `http://127.0.0.1:${/** @type {any} */ (app.server.address()).port}`;

  /** `json()` is `unknown`, because a response body is untrusted until something checks it.
   *  This tool is the thing doing the checking, so it asserts the shape as it goes. */
  const json = async (/** @type {string} */ url) =>
    /** @type {any} */ (await fetch(url).then((r) => r.json()));

  const health = await json(`${base}/api/health`);
  check('the server answers /api/health', health.ok === true && health.service === 'comiccraft');

  const shell = await fetch(`${base}/`);
  check('the app shell is served', shell.ok && (await shell.text()).includes('<canvas'),
    `status ${shell.status}`);

  const domain = await fetch(`${base}/shared/domain/index.js`);
  const domainSource = domain.ok ? await domain.text() : '';
  check('the shared domain is served to the browser', domain.ok && domainSource.includes('export'),
    `status ${domain.status}`);

  const catalogue = await json(`${base}/api/comic`);
  check(`the command catalogue lists ${catalogue.commands?.length ?? 0} commands`,
    Array.isArray(catalogue.commands) && catalogue.commands.length >= 36);

  /** @param {string} cmd @param {unknown} [params] */
  const run = async (cmd, params = {}) => {
    const response = await fetch(`${base}/api/comic/${cmd}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(params),
    });
    return { status: response.status, body: /** @type {any} */ (await response.json()) };
  };

  const created = await run('comic.project.create', { title: 'Smoke' });
  check('a project is created over HTTP', created.body.ok === true, created.body.message);

  const page = await run('comic.page.add', { name: 'Page 1', template: 'three-up' });
  check('a three-panel page is created', page.body.ok === true && page.body.value.panels === 3, page.body.message);

  const pageId = page.body.value?.pageId;
  const panels = session.bundle.project.pages[pageId].panelIds;

  const placed = await run('comic.artwork.place', { panelId: panels[0], name: 'Art', contentHash: 'smoke-1' });
  check('artwork is placed', placed.body.ok === true, placed.body.message);

  const cropped = await run('comic.artwork.crop', { panelId: panels[0], x: 0.1, y: 0.1, width: 0.8, height: 0.8 });
  check('artwork is cropped', cropped.body.ok === true, cropped.body.message);

  const clipped = await run('comic.artwork.clip', { panelId: panels[0], inset: 5 });
  check('artwork is clipped to the panel', clipped.body.ok === true, clipped.body.message);

  const lettered = await run('comic.lettering.add', {
    panelId: panels[0], kind: 'balloon', text: 'A LINE OF DIALOGUE', overflowPolicy: 'shrink',
  });
  check('lettering is added', lettered.body.ok === true, lettered.body.message);

  const validated = await run('comic.project.validate');
  check('the project validates', validated.body.value?.errors === 0,
    `${validated.body.value?.errors} error(s)`);

  const png = await run('comic.export.png', { pageId, path: 'page-1.png', scale: 0.2 });
  check('PNG export reports a size', png.body.ok === true && png.body.value.width > 0, png.body.message);

  const pdf = await run('comic.export.pdf', { path: 'book.pdf' });
  check('PDF export reports a page count', pdf.body.ok === true && pdf.body.value.pages === 1, pdf.body.message);

  const saved = await run('comic.project.save', { path: 'smoke' });
  check('the project saves', saved.body.ok === true, saved.body.message);

  const reopened = await run('comic.project.open', { path: 'smoke' });
  check('the project reopens with no errors', reopened.body.ok === true && reopened.body.value.errors === 0,
    reopened.body.message);

  const providers = await run('comic.provider.list');
  check('providers declare their capabilities', providers.body.ok === true && providers.body.value.providers.length >= 2);

  const submitted = await run('comic.job.submit', {
    operation: 'text-to-image', prompt: 'A smoke-test rooftop', panelId: panels[0],
  });
  check('a generation job is submitted without blocking', submitted.body.ok === true && submitted.body.value.status === 'queued',
    submitted.body.message);
  const jobId = submitted.body.value?.jobId;
  const settled = jobId ? await session.jobs.settled(jobId) : null;
  check('the job settles as a result awaiting review',
    settled && settled.status === 'awaiting_review' && settled.resultAssetIds.length === 1,
    settled ? settled.status : 'no job');
  const applied = jobId ? await run('comic.job.apply', { jobId }) : { body: {} };
  check('a reviewed result can be applied as one step', applied.body.ok === true, applied.body.message);

  const batch = await run('comic.batch.run', {
    commands: [
      { command: 'comic.character.add', params: { name: 'Smoke Character' } },
      { command: 'comic.reading.validate', params: { pageId } },
    ],
  });
  check('a batch runs through the same dispatcher', batch.body.ok === true, batch.body.message);

  const imported = await run('comic.story.import', { text: 'SCENE: Rooftop\nMAYA: The city is quiet.\nSFX: KRAKOOM!' });
  check('a story import produces a proposal', imported.body.ok === true, imported.body.message);
  const accepted = await run('comic.storyboard.accept', { proposalId: imported.body.value?.proposalId });
  check('a proposal is only authoritative once accepted', accepted.body.ok === true, accepted.body.message);

  const designed = await run('comic.story.design', {
    ideas: 'A rooftop at night. Maya sees a shadow move. She runs for the stairwell.',
    style: 'noir',
    characters: ['Maya: red windbreaker'],
    maxPanelsPerPage: 3,
  });
  check('an idea is designed into a reviewable storyboard', designed.body.ok === true && designed.body.value.outline.scenes.length >= 1,
    designed.body.message);
  const acceptedDesign = await run('comic.storyboard.accept', { proposalId: designed.body.value?.proposalId });
  check('the designed storyboard is accepted into pages', acceptedDesign.body.ok === true && acceptedDesign.body.value.pages.length > 0,
    acceptedDesign.body.message);
  const designPanel = acceptedDesign.body.value?.panels?.[0];
  const painted = await run('comic.artwork.paint', { panelId: designPanel, style: 'noir' });
  check('a local drawing is placed without a provider', painted.body.ok === true, painted.body.message);

  const publication = await run('comic.book.validate', { preset: 'screen' });
  check('the publication gate reports a verdict', publication.body.ok === true && typeof publication.body.value.ready === 'boolean',
    publication.body.message);

  const packaged = await run('comic.project.package', { path: 'smoke.comiccraft' });
  check('the project packages into one deterministic archive', packaged.body.ok === true, packaged.body.message);

  const malformed = await run('comic.panel.add', { pageId });
  check('a malformed call is a 400, not a 500', malformed.status === 400, `status ${malformed.status}`);

  const unknown = await run('comic.nope.nope');
  check('an unknown command is a 404', unknown.status === 404, `status ${unknown.status}`);
} catch (error) {
  console.log(`  FAIL the server could not start — ${/** @type {Error} */ (error).message}`);
  failures.push('server start');
} finally {
  await app.close();
  await rm(root, { recursive: true, force: true });
}

if (failures.length > 0) {
  console.error(`\nsmoke failed: ${failures.length} check(s)`);
  process.exit(1);
}
console.log('\nsmoke ok — the server boots, serves the client and the domain, and runs the slice');
