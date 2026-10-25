#!/usr/bin/env node
/**
 * Generate `docs/comiccraft-command-api.md` from the registry.
 *
 * The registry's parameter schemas are the documentation MCP publishes, so a hand-written
 * command reference drifts the moment a command changes. This generates the reference from
 * the same objects and `--check` fails when the committed file is stale.
 *
 * Usage: node tools/gen-command-docs.js [--check]
 */

import { readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { buildRegistry } from '../shared/commands/index.js';

const root = resolve(import.meta.dirname, '..');
const OUTPUT = join(root, 'docs', 'comiccraft-command-api.md');

/**
 * @param {any} schema
 * @param {string} path
 * @returns {string[]}
 */
function paramsOf(schema, path = '') {
  /** @type {string[]} */
  const out = [];
  const required = new Set(schema.required ?? []);
  for (const [name, sub] of Object.entries(schema.properties ?? {})) {
    const s = /** @type {any} */ (sub);
    const type = Array.isArray(s.type) ? s.type.join(' | ') : (s.type ?? 'any');
    const bits = [`\`${path}${name}\`: ${type}`];
    if (required.has(name)) bits.push('required');
    if (s.enum) bits.push(`one of ${s.enum.map((/** @type {any} */ v) => JSON.stringify(v)).join(', ')}`);
    if (s.minimum !== undefined) bits.push(`min ${s.minimum}`);
    if (s.maximum !== undefined) bits.push(`max ${s.maximum}`);
    if (s.description) bits.push(s.description);
    out.push(`- ${bits.join(' — ')}`);
    if (s.type === 'object' && s.properties) out.push(...paramsOf(s, `${path}${name}.`));
    if (s.items?.properties) out.push(...paramsOf(s.items, `${path}${name}[].`));
  }
  return out;
}

/**
 * @returns {Promise<string>}
 */
export async function expectedDocument() {
  const registry = buildRegistry();
  /** @type {Map<string, any[]>} */
  const groups = new Map();
  for (const id of registry.ids()) {
    const command = /** @type {any} */ (registry.get(id));
    const group = id.split('.')[1];
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group)?.push(command);
  }

  /** @type {string[]} */
  const lines = [];
  lines.push('# ComicCraft command API');
  lines.push('');
  lines.push('> Generated from `shared/commands` by `npm run docs:commands`. Do not edit by hand.');
  lines.push('> The parameter schema shown here is the same object `tools/list` publishes over MCP');
  lines.push('> and the HTTP endpoint validates against, so a click, a `curl` and an agent call cannot disagree.');
  lines.push('');
  lines.push(`**${registry.ids().length} commands.** Every command is enabled only on an open comic project unless stated otherwise.`);
  lines.push('');
  lines.push('Retry classes: **safe** commands are read-only; **idempotent** commands may be replayed with the same result; **state-changing** commands write project state and should be retried deliberately.');
  lines.push('');

  for (const [group, commands] of [...groups.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    lines.push(`## \`comic.${group}.*\``);
    lines.push('');
    for (const command of commands.sort((a, b) => a.id.localeCompare(b.id))) {
      const retry = command.id.endsWith('.delete') || command.id.endsWith('.reject') ? 'idempotent'
        : /\.(info|list|get|validate|status|describe)$/.test(command.id) ? 'safe' : 'state-changing';
      lines.push(`### \`${command.id}\``);
      lines.push('');
      lines.push(`**${command.label}**${command.serverOnly ? ' · server only' : ''} · retry: ${retry}${command.undo ? ` · undo: ${command.undo.scope}` : ''}`);
      lines.push('');
      const params = paramsOf(command.params);
      if (params.length === 0) lines.push('No parameters.');
      else lines.push(...params);
      lines.push('');
    }
  }

  lines.push('## Transports');
  lines.push('');
  lines.push('- **HTTP:** `POST /api/comic/<command>` with a JSON body; `GET` for read-only calls.');
  lines.push('- **MCP:** `node server/mcp.js --headless`; `tools/list` and `tools/call` are generated from this registry.');
  lines.push('- **CLI:** `node server/cli.js run --cmd <id> --params \'<json>\'`.');
  lines.push('- **Browser:** the same dispatch function in `public/app.js`.');
  lines.push('');
  return `${lines.join('\n')}`;
}

async function main() {
  const text = await expectedDocument();
  if (process.argv.includes('--check')) {
    let current = '';
    try {
      current = await readFile(OUTPUT, 'utf8');
    } catch {
      current = '';
    }
    if (current !== text) {
      process.stderr.write('docs/comiccraft-command-api.md is stale — run `npm run docs:commands`\n');
      return 1;
    }
    process.stdout.write('command docs ok — every registered command is documented\n');
    return 0;
  }
  await writeFile(OUTPUT, text, 'utf8');
  process.stdout.write(`wrote ${OUTPUT}\n`);
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  process.exitCode = await main();
}
