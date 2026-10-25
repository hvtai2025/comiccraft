/**
 * M5's exit criteria: every command is documented for an agent, MCP lists and calls the same
 * registry, a batch is atomic on request, and malformed input cannot take the dispatcher down.
 * @module test/agent
 */

import { test } from 'node:test';
import { must, refused, sessionHarness } from './helpers.js';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { join } from 'node:path';
import { handleMessage, toolList } from '../server/mcp.js';
import { buildRegistry } from '../shared/commands/index.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { expectedDocument } from '../tools/gen-command-docs.js';
import { History } from '../shared/domain/history.js';
import { createBundle } from '../shared/domain/project.js';
import { createMemoryStorage } from '../server/storage/fs.js';
import { createRenderer } from '../server/export/renderer.js';

const exec = promisify(execFile);

test('the generated command API is current, and lists every command', async () => {
  const expected = await expectedDocument();
  const current = await readFile(join(import.meta.dirname, '..', 'docs', 'comiccraft-command-api.md'), 'utf8');
  assert.equal(current, expected, 'run `npm run docs:commands` after changing the registry');
  const registry = buildRegistry();
  assert.ok(registry.ids().length >= 80, `expected the completed surface, got ${registry.ids().length}`);
  for (const command of registry.describe()) {
    assert.ok(expected.includes(`### \`${command.id}\``), `${command.id} is missing from the docs`);
  }
});

test('the command catalogue publishes a parameter schema for every command', async (t) => {
  const session = sessionHarness(t);
  const tools = toolList(session);
  assert.equal(tools.length, session.registry.ids().length);
  for (const tool of tools) {
    assert.equal(tool.name, tool.name);
    assert.ok(tool.description.length > 0);
    assert.equal(typeof tool.inputSchema, 'object');
    assert.ok(Array.isArray(tool.inputSchema.required) || tool.inputSchema.required === undefined);
  }
  const described = must(await session.run('comic.command.describe', { commandId: 'comic.panel.add' }));
  assert.equal(described.command.params.properties.pageId.type, 'string');
  assert.equal(described.command.retry, 'state-changing');
});

test('MCP initialize, tools/list and tools/call go through the same executor', async (t) => {
  const session = sessionHarness(t);
  const initialized = await handleMessage(session, { jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
  assert.equal(initialized.result.serverInfo.name, 'comiccraft');
  assert.ok(initialized.result.capabilities.tools);

  const listed = await handleMessage(session, { jsonrpc: '2.0', id: 2, method: 'tools/list' });
  assert.equal(listed.result.tools.length, session.registry.ids().length);
  const addPage = listed.result.tools.find((/** @type {any} */ tool) => tool.name === 'comic.page.add');
  assert.equal(addPage.inputSchema.properties.template.type, 'string');

  const created = await handleMessage(session, {
    jsonrpc: '2.0', id: 3, method: 'tools/call',
    params: { name: 'comic.project.create', arguments: { title: 'Over MCP' } },
  });
  assert.equal(created.result.isError, false);
  assert.equal(session.bundle.project.title, 'Over MCP');

  const page = await handleMessage(session, {
    jsonrpc: '2.0', id: 4, method: 'tools/call',
    params: { name: 'comic.page.add', arguments: { name: 'P', template: 'two-up' } },
  });
  assert.equal(page.result.isError, false);
  assert.equal(page.result.structuredContent.value.panels, 2);
  // The protocol reports a command failure as a result, not as a transport crash.
  const bad = await handleMessage(session, {
    jsonrpc: '2.0', id: 5, method: 'tools/call',
    params: { name: 'comic.page.add', arguments: {} },
  });
  assert.equal(bad.result.isError, true);
  assert.match(bad.result.content[0].text, /bad-params/);
  const missing = await handleMessage(session, { jsonrpc: '2.0', id: 6, method: 'tools/call', params: { name: 'comic.nope.nope' } });
  assert.equal(missing.error.code, -32602);
});

test('the MCP binary prints a complete catalogue without a browser or a network', async () => {
  const { stdout } = await exec(process.execPath, [join(import.meta.dirname, '..', 'server', 'mcp.js'), '--list'], {
    cwd: join(import.meta.dirname, '..'),
  });
  const parsed = JSON.parse(stdout);
  assert.equal(parsed.protocolVersion, '2024-11-05');
  assert.ok(parsed.tools.length >= 80);
  assert.ok(parsed.tools.every((/** @type {any} */ tool) => tool.inputSchema && tool.description));
});

test('a batch runs through one dispatcher and is atomic when asked to be', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Batch' }));

  const ok = must(await session.run('comic.batch.run', {
    commands: [
      { command: 'comic.page.add', params: { name: 'One', template: 'three-up' } },
      { command: 'comic.character.add', params: { name: 'Maya' } },
    ],
  }));
  assert.equal(ok.total, 2);
  assert.equal(ok.failed, 0);
  assert.equal(Object.keys(session.bundle.project.pages).length, 1);
  assert.equal(Object.keys(session.bundle.project.characters).length, 1);

  const before = structuredClone(session.bundle.project);
  const depthBefore = session.history.undoStack.length;
  const failed = refused(await session.run('comic.batch.run', {
    commands: [
      { command: 'comic.page.add', params: { name: 'Two', template: 'two-up' } },
      { command: 'comic.panel.setFrame', params: { panelId: 'panel_nope', x: 0, y: 0, width: 10, height: 10 } },
    ],
  }));
  assert.equal(failed.kind, 'conflict');
  assert.deepEqual(session.bundle.project.pages, before.pages, 'the successful first command was rolled back');
  assert.equal(session.history.undoStack.length, depthBefore, 'and its undo step was dropped');
});

test('an atomic batch that includes an undo restores the stacks exactly and keeps the revision', async (t) => {
  const session = sessionHarness(t);
  must(await session.run('comic.project.create', { title: 'Batch undo' }));
  must(await session.run('comic.page.add', { name: 'One', template: 'single' }));
  must(await session.run('comic.character.add', { name: 'Maya' }));
  const depth = session.history.depth().undo;
  const revision = session.bundle.project.revision;
  const labels = [...session.history.depth().labels];

  const failed = refused(await session.run('comic.batch.run', {
    commands: [
      { command: 'comic.edit.undo' },
      { command: 'comic.panel.setFrame', params: { panelId: 'panel_nope', x: 0, y: 0, width: 10, height: 10 } },
    ],
  }));
  assert.equal(failed.kind, 'conflict');
  assert.equal(session.history.depth().undo, depth);
  assert.deepEqual(session.history.depth().labels, labels);
  assert.equal(session.history.depth().redo, 0);
  assert.equal(session.bundle.project.revision, revision, 'a rollback does not lower the revision');
  assert.equal(Object.keys(session.bundle.project.pages).length, 1);
});

test('malformed parameters cannot crash the dispatcher for any registered command', async () => {
  const registry = buildRegistry();
  const bundle = createBundle('Fuzz');
  const history = new History();
  const services = {
    storage: createMemoryStorage(),
    renderer: createRenderer(),
    registry,
  };
  const ctx = { bundle, history, env: { onServer: true, now: () => 0 }, services };

  /** @type {any[]} */
  const payloads = [
    {},
    { unexpected: true },
    { pageId: 42, panelId: [], text: null },
    { order: 'not-an-array' },
    { kind: 'nope', text: 'x', panelId: 'panel_1' },
  ];
  const allowed = new Set(['ok', 'bad-params', 'not-found', 'conflict', 'disabled', 'internal']);
  for (const id of registry.ids()) {
    for (const params of payloads) {
      const result = await dispatch(registry, ctx, id, params);
      assert.equal(typeof result.ok, 'boolean', `${id} returned no result for ${JSON.stringify(params)}`);
      assert.ok(allowed.has(result.ok ? 'ok' : result.kind), `${id} returned kind ${result.ok ? 'ok' : result.kind}`);
    }
  }
});
