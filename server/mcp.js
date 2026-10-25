#!/usr/bin/env node
/**
 * The MCP server: the same command registry, spoken over stdio.
 *
 * There is no second automation protocol here. `tools/list` is generated from the registry's
 * parameter schemas, and `tools/call` goes through `session.run`, which is the dispatcher the
 * browser and HTTP use. A tool an agent sees is therefore a tool a person can click.
 *
 * ```sh
 * node server/mcp.js --headless          # newline-delimited JSON-RPC on stdin/stdout
 * node server/mcp.js --list              # the tool catalogue, for a gate or a human
 * node server/mcp.js --headless --project demo
 * ```
 *
 * Stdout is the protocol. Diagnostics go to stderr, never into a response.
 *
 * @module server/mcp
 */

import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { createInterface } from 'node:readline';
import { createSession } from './session.js';

const PROTOCOL_VERSION = '2024-11-05';

/** @returns {any} */
const pkg = () => {
  try {
    return JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  } catch {
    return { version: '0.1.0' };
  }
};

/**
 * The tool catalogue. `inputSchema` is the command's own parameters object — the same one
 * the HTTP endpoint validates against and the browser uses to build a form.
 * @param {ReturnType<typeof createSession>} session
 * @returns {any[]}
 */
export function toolList(session) {
  /** @type {any[]} */
  const tools = [];
  for (const command of session.registry.describe()) {
    const registered = session.registry.get(command.id);
    const retry = command.id.endsWith('.delete') || command.id.endsWith('.reject') ? 'idempotent' : 'state-changing';
    tools.push({
      name: command.id,
      description: `${command.label}.${command.serverOnly ? ' Server-only.' : ''} Retry: ${retry}.`,
      inputSchema: command.params,
      annotations: {
        title: command.label,
        readOnlyHint: /\.(info|list|get|validate|status|describe)$/.test(command.id),
        destructiveHint: /\.(delete|reject|cancel)$/.test(command.id),
      },
      ...(registered?.serverOnly ? { _meta: { serverOnly: true } } : {}),
    });
  }
  return tools;
}

/**
 * @param {unknown} value
 * @returns {any}
 */
function slimResult(value) {
  if (!value || typeof value !== 'object') return value;
  const v = /** @type {any} */ (value);
  if (!v.bundle) return v;
  return {
    ...v,
    bundle: {
      projectId: v.bundle.project.id,
      title: v.bundle.project.title,
      pages: Object.keys(v.bundle.project.pages).length,
      panels: Object.keys(v.bundle.project.panels).length,
      revision: v.bundle.project.revision,
    },
  };
}

/**
 * Handle one JSON-RPC message. Returns a response object, or null for a notification.
 * @param {any} session
 * @param {any} message
 * @returns {Promise<any | null>}
 */
export async function handleMessage(session, message) {
  if (!message || typeof message !== 'object') {
    return { jsonrpc: '2.0', id: null, error: { code: -32600, message: 'invalid request' } };
  }
  const { id, method, params } = message;
  const isNotification = id === undefined || id === null;
  /** @param {unknown} result */
  const reply = (result) => isNotification ? null : { jsonrpc: '2.0', id, result };
  /** @param {number} code @param {string} text */
  const error = (code, text) => isNotification ? null : { jsonrpc: '2.0', id, error: { code, message: text } };

  switch (method) {
    case 'initialize':
      return reply({
        protocolVersion: PROTOCOL_VERSION,
        capabilities: { tools: { listChanged: false } },
        serverInfo: { name: 'comiccraft', version: pkg().version ?? '0.1.0' },
        instructions: 'ComicCraft exposes its command registry as MCP tools. Open or create a project first; every tool is a normal command.',
      });

    case 'notifications/initialized':
    case 'notifications/cancelled':
    case 'notifications/progress':
      return null;

    case 'ping':
      return reply({});

    case 'tools/list':
      return reply({ tools: toolList(session) });

    case 'tools/call': {
      const name = params?.name;
      if (typeof name !== 'string' || name.length === 0) {
        return error(-32602, 'tools/call needs a tool name');
      }
      if (!session.registry.get(name)) {
        return error(-32602, `no tool ${name}`);
      }
      const args = params?.arguments ?? {};
      const result = await session.run(name, args);
      const payload = slimResult(result);
      return reply({
        content: [{ type: 'text', text: JSON.stringify(payload) }],
        structuredContent: payload,
        isError: result.ok !== true,
      });
    }

    case 'resources/list':
      return reply({ resources: [] });

    case 'shutdown':
      return reply({});

    default:
      return error(-32601, `method not found: ${String(method)}`);
  }
}

/**
 * @param {any} session
 * @returns {{ receive: (message: any) => Promise<any | null> }}
 */
export function createMcpServer(session) {
  return {
    receive: (message) => handleMessage(session, message),
  };
}

/**
 * @param {string[]} argv
 * @returns {Promise<number>} exit code
 */
export async function main(argv) {
  const args = new Set(argv);
  const projectIndex = argv.indexOf('--project');
  const projectPath = projectIndex === -1 ? null : argv[projectIndex + 1] ?? null;
  const session = createSession({ root: process.env.COMICCRAFT_ROOT });

  if (projectPath) {
    const opened = await session.run('comic.project.open', { path: projectPath });
    if (!opened.ok) {
      process.stderr.write(`could not open ${projectPath}: ${opened.kind}: ${opened.message}\n`);
      return 1;
    }
  }

  if (args.has('--list')) {
    process.stdout.write(`${JSON.stringify({ protocolVersion: PROTOCOL_VERSION, tools: toolList(session) }, null, 2)}\n`);
    return 0;
  }

  if (!args.has('--headless')) {
    process.stderr.write('ComicCraft MCP\n\n  node server/mcp.js --headless [--project <dir>]\n  node server/mcp.js --list\n');
    return 2;
  }

  const server = createMcpServer(session);
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of rl) {
    const trimmed = line.trim();
    if (trimmed === '') continue;
    let message;
    try {
      message = JSON.parse(trimmed);
    } catch {
      process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'parse error' } })}\n`);
      continue;
    }
    try {
      const response = await server.receive(message);
      if (response !== null) process.stdout.write(`${JSON.stringify(response)}\n`);
    } catch (fatal) {
      // A protocol handler must not take the process down: the client reports the error and
      // can keep the session alive.
      process.stderr.write(`mcp handler error: ${fatal instanceof Error ? fatal.stack ?? fatal.message : String(fatal)}\n`);
      if (message && message.id !== undefined) {
        process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id: message.id, error: { code: -32603, message: 'internal error' } })}\n`);
      }
    }
  }
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  process.exitCode = await main(process.argv.slice(2));
}
