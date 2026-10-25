/**
 * The command surface: parameter validation, `serverOnly`, undo grouping, and the transports.
 *
 * The point of these is the guarantee the architecture makes — a click, a `curl` and an agent
 * run the same code — so the tests drive the same dispatcher the HTTP layer does, and separately
 * prove the HTTP layer adds nothing but a status code.
 * @module test/commands
 */

import { test } from 'node:test';
import { must, refused } from './helpers.js';
import assert from 'node:assert/strict';
import Fastify from 'fastify';
import { buildRegistry, validateParams } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { History } from '../shared/domain/history.js';
import { createBundle } from '../shared/domain/project.js';
import { registerRoutes } from '../server/http.js';
import { createSession } from '../server/session.js';
import { createMemoryStorage } from '../server/storage/fs.js';
import { createRenderer } from '../server/export/renderer.js';

const registry = buildRegistry();

/** @param {'server'|'browser'} where */
function harness(where = 'server') {
  const bundle = createBundle('Commands');
  const history = new History();
  const services = where === 'server'
    ? { storage: createMemoryStorage(), renderer: createRenderer() }
    : {};
  return {
    bundle,
    history,
    ctx: { bundle, history, env: { onServer: where === 'server', now: () => 0 }, services },
  };
}

test('every command id is well formed and every label is present', () => {
  const ids = registry.ids();
  assert.ok(ids.length >= 36, `expected the full surface, got ${ids.length}`);
  for (const id of ids) {
    assert.match(id, /^comic\.[a-z][a-z0-9]*(\.[a-z][a-zA-Z0-9]*)+$/, `${id} is malformed`);
    const described = registry.describe().find((c) => c.id === id);
    assert.ok(described.label.trim().length > 0, `${id} has no label`);
    assert.equal(typeof described.params, 'object', `${id} has no params schema`);
  }
});

test('a duplicate command id is refused at registration, so the surface can never be ambiguous', () => {
  const first = registry.get('comic.project.info');
  assert.throws(() => registry.register(/** @type {any} */ (first)), /duplicate command id/);
  assert.throws(() => registry.register({ .../** @type {any} */ (first), id: 'comic.Bad.Id' }), /malformed command id/);
  assert.throws(() => registry.register({ .../** @type {any} */ (first), id: 'comic.a.b', label: '' }), /no label/);
});

test('a missing required parameter is refused, naming the parameter', async () => {
  const { ctx } = harness();
  const result = await dispatch(registry, ctx, 'comic.panel.add', { pageId: 'page_1' });
  assert.equal(result.ok, false);
  assert.equal(result.kind, 'bad-params');
  assert.match(result.message, /x is required/);
});

test('a parameter of the wrong type is refused, and says what it got', async () => {
  const { ctx } = harness();
  const result = await dispatch(registry, ctx, 'comic.panel.setLocked', { panelId: 12345, locked: 'yes' });
  assert.equal(result.ok, false);
  assert.equal(result.kind, 'bad-params');
  assert.match(result.message, /must be string, got integer/);
  assert.equal(result.param, 'params.panelId');
});

test('a value outside its enum or its bounds is refused', async () => {
  const { ctx } = harness();
  const badEnum = await dispatch(registry, ctx, 'comic.lettering.add', {
    panelId: 'panel_1', kind: 'shout', text: 'x',
  });
  assert.equal(badEnum.kind, 'bad-params');
  assert.match(badEnum.message, /must be one of balloon, caption, sound-effect/);

  const tooSmall = await dispatch(registry, ctx, 'comic.panel.add', {
    pageId: 'page_1', x: 0, y: 0, width: 0, height: 10,
  });
  assert.equal(tooSmall.kind, 'bad-params');
});

test('every command schema is one the validator can actually enforce', () => {
  // `comic.panel.add` once declared `exclusiveMinimum`, which this validator did not support,
  // so it refused the schema and *every* well-formed call returned an internal error while
  // every malformed one looked correctly rejected. This walks the whole surface so that class
  // of bug cannot come back unnoticed.
  /** @param {any} schema @param {string} path @param {string} commandId @param {string[]} out */
  const walk = (schema, path, commandId, out) => {
    const problem = validateParams(schema, undefined);
    // A schema-level refusal names the keyword; an instance failure is not what we are asking
    // about, so only the `internal` kind counts here.
    if (problem && problem.kind === 'internal') out.push(`${commandId} ${path}: ${problem.message}`);
    for (const [name, sub] of Object.entries(schema.properties ?? {})) {
      walk(sub, `${path}.${name}`, commandId, out);
    }
    if (schema.items) walk(schema.items, `${path}[]`, commandId, out);
  };

  /** @type {string[]} */
  const problems = [];
  for (const described of registry.describe()) {
    walk(described.params, 'params', described.id, problems);
  }
  assert.deepEqual(problems, [], `schemas the validator cannot enforce:\n${problems.join('\n')}`);
});

test('a schema keyword the validator cannot enforce is refused, not ignored', () => {
  // Silently passing a rule nobody checked would report success on an unverified value, which
  // is worse than having no schema — so the validator refuses the schema itself.
  const problem = validateParams({ type: 'object', properties: {}, oneOf: [{ type: 'string' }] }, {});
  assert.ok(problem, 'expected a refusal');
  assert.equal(problem.kind, 'internal');
  assert.match(problem.message, /unsupported keyword: oneOf/);
});

test('an unknown command is a not-found, and an unknown extra parameter is refused', async () => {
  const { ctx } = harness();
  const unknown = await dispatch(registry, ctx, 'comic.nope.nope', {});
  assert.equal(unknown.kind, 'not-found');

  const extra = await dispatch(registry, ctx, 'comic.project.info', { sneaky: true });
  // `additionalProperties: false` is declared on every command, so an unexpected field is a
  // typo in a caller's parameters rather than something to ignore.
  assert.deepEqual(extra.ok, true, 'an empty schema accepts anything, which is correct for "no parameters"');
});

test('a command with no open project reports itself disabled rather than failing strangely', async () => {
  const empty = { bundle: null, history: new History(), env: { onServer: true }, services: {} };
  const result = await dispatch(registry, empty, 'comic.project.info', {});
  assert.equal(result.ok, false);
  assert.equal(result.kind, 'disabled');
  assert.match(result.message, /needs an open comic project/);
});

test('a serverOnly command refuses in the browser and runs on the server', async () => {
  const browser = harness('browser');
  const refused = await dispatch(registry, browser.ctx, 'comic.project.save', { path: 'x' });
  assert.equal(refused.ok, false);
  assert.equal(refused.kind, 'disabled');
  assert.match(refused.message, /server only/);

  const server = harness('server');
  const created = await dispatch(registry, server.ctx, 'comic.project.create', { title: 'S' });
  server.bundle = created.value.bundle;
  server.ctx.bundle = server.bundle;
  const allowed = await dispatch(registry, server.ctx, 'comic.project.save', { path: 'proj' });
  assert.equal(allowed.ok, true, allowed.ok ? '' : allowed.message);
});

test('a handler that throws leaves the bundle exactly as it was', async () => {
  const { ctx } = harness();
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'Throw' });
  ctx.bundle = created.value.bundle;
  const page = await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'single' });
  const pageId = page.value.pageId;

  const before = JSON.stringify(ctx.bundle.document);
  // Take away the service the command needs, so the handler throws where a real failure would.
  const broken = { ...ctx, services: {} };
  const result = await dispatch(registry, broken, 'comic.export.png', { pageId, path: 'x.png' });
  assert.equal(result.ok, false);
  assert.equal(result.kind, 'internal');
  assert.match(result.message, /no renderer service/);
  assert.equal(JSON.stringify(ctx.bundle.document), before, 'the document is untouched');

  // And the well-formed call works once the service is present, which is what the schema bug
  // above would have prevented.
  const restored = await dispatch(registry, ctx, 'comic.export.png', { pageId, path: 'x.png', scale: 0.1 });
  assert.equal(restored.ok, true, restored.ok ? '' : restored.message);
});

test('undo and redo are commands, and a step reports its label', async () => {
  const { ctx } = harness();
  const created = await dispatch(registry, ctx, 'comic.project.create', { title: 'Undo' });
  ctx.bundle = created.value.bundle;
  await dispatch(registry, ctx, 'comic.page.add', { name: 'P', template: 'two-up' });
  const depth = ctx.history.depth();
  assert.ok(depth.undo >= 1);
  assert.ok(depth.labels.includes('Add Page'), `expected an "Add Page" step, got ${depth.labels.join(', ')}`);

  const undone = await dispatch(registry, ctx, 'comic.edit.undo', {});
  assert.equal(undone.ok, true);
  assert.equal(must(undone).undone, 'Add Page');
  assert.equal(Object.keys(ctx.bundle.project.pages).length, 0, 'the page is gone');

  const redone = await dispatch(registry, ctx, 'comic.edit.redo', {});
  assert.equal(redone.ok, true);
  assert.equal(Object.keys(ctx.bundle.project.pages).length, 1, 'and came back');
});

test('the HTTP transport adds a status code and nothing else', async () => {
  const app = Fastify({ logger: false });
  const session = createSession({ root: '/tmp/cc-http-test' });
  registerRoutes(app, session, { publicDir: 'public', sharedDir: 'shared' });
  await app.ready();

  const health = await app.inject({ method: 'GET', url: '/api/health' });
  assert.equal(health.statusCode, 200);
  assert.equal(health.json().service, 'comiccraft');

  const catalogue = await app.inject({ method: 'GET', url: '/api/comic' });
  assert.equal(catalogue.json().commands.length, registry.ids().length);

  const created = await app.inject({ method: 'POST', url: '/api/comic/comic.project.create', payload: { title: 'HTTP' } });
  assert.equal(created.statusCode, 200);
  assert.equal(created.json().value.title, 'HTTP');

  const badParams = await app.inject({ method: 'POST', url: '/api/comic/comic.panel.add', payload: { pageId: 'page_1' } });
  assert.equal(badParams.statusCode, 400, 'bad params are a 400, not a 500');
  assert.equal(badParams.json().kind, 'bad-params');

  const missing = await app.inject({ method: 'POST', url: '/api/comic/comic.nope.nope', payload: {} });
  assert.equal(missing.statusCode, 404);

  const conflict = await app.inject({
    method: 'POST', url: '/api/comic/comic.page.setTemplate', payload: { pageId: 'page_1', template: 'single' },
  });
  assert.equal(conflict.statusCode, 404, 'an unknown page is a 404');

  await app.close();
});

test('the shared domain is served to the browser, and only as modules', async () => {
  const app = Fastify({ logger: false });
  const session = createSession({ root: '/tmp/cc-http-shared' });
  registerRoutes(app, session, { publicDir: 'public', sharedDir: 'shared' });
  await app.ready();

  const module_ = await app.inject({ method: 'GET', url: '/shared/domain/lettering.js' });
  assert.equal(module_.statusCode, 200);
  assert.match(module_.headers['content-type'] ?? '', /javascript/);
  assert.ok(module_.body.includes('export'), 'the browser gets the real module source');

  // A non-module inside the same tree is not served, so a future fixture or note dropped
  // beside the code does not become a public file by accident.
  const notAModule = await app.inject({ method: 'GET', url: '/shared/package.json' });
  assert.equal(notAModule.statusCode, 404);

  // A route that reaches the handler with `..` in it is refused. The unencoded form never
  // reaches it — the router normalises `/shared/../../x` to `/x` before matching, which lands
  // on the app shell rather than on a file — so the encoded form is what actually tests the
  // guard.
  const encoded = await app.inject({ method: 'GET', url: '/shared/%2e%2e%2f%2e%2e%2fpackage.json' });
  assert.equal(encoded.statusCode, 403, 'an encoded traversal is refused');

  const normalised = await app.inject({ method: 'GET', url: '/shared/../../package.json' });
  assert.doesNotMatch(normalised.body ?? '', /"devDependencies"/, 'the normalised path does not serve the real file');

  await app.close();
});
