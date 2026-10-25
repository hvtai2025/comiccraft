#!/usr/bin/env node
/**
 * The brand gate: user-visible text must not name the vendors this project is clean-room
 * against.
 *
 * It scans the strings that reach a user or an agent — command labels, parameter
 * descriptions, UI copy, MCP tool definitions — rather than the whole tree, because a
 * citation in a design document is legitimate and a menu item that says a competitor's name
 * is not.
 *
 * Usage: node tools/check-brands.js
 */

import { readFile, readdir } from 'node:fs/promises';
import { join, relative, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const root = resolve(import.meta.dirname, '..');

/** Product names that must never appear in user-visible text. */
const VENDORS = [
  'Adobe', 'Illustrator', 'Photoshop', 'InDesign', 'Acrobat', 'FreeHand',
  'CorelDRAW', 'Affinity Designer', 'Sketch', 'Figma',
];

/** Where user-visible strings live. Deliberately not "everywhere". */
const SCAN = [
  'shared/commands/commands',
  'public',
  'shared/domain',
];

/**
 * @returns {Promise<string[]>}
 */
export async function checkBrands() {
  /** @type {string[]} */
  const problems = [];

  for (const area of SCAN) {
    const dir = join(root, area);
    /** @type {string[]} */
    let files;
    try {
      files = await walk(dir);
    } catch {
      continue;
    }
    for (const file of files) {
      const source = await readFile(file, 'utf8');
      const rel = relative(root, file);
      for (const vendor of VENDORS) {
        const re = new RegExp(`\\b${vendor}\\b`, 'i');
        const line = source.split('\n').findIndex((l) => re.test(l));
        if (line !== -1) {
          problems.push(`${rel}:${line + 1} names '${vendor}' in shipped text`);
        }
      }
    }
  }
  return problems;
}

/** @param {string} dir @returns {Promise<string[]>} */
async function walk(dir) {
  /** @type {string[]} */
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...await walk(full));
    else if (entry.isFile() && /\.(js|html|css)$/.test(entry.name)) out.push(full);
  }
  return out;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const problems = await checkBrands();
  if (problems.length === 0) {
    console.log('brands ok — no vendor name in shipped text');
    process.exit(0);
  }
  console.error(`brand violations (${problems.length}):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
