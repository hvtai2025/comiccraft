#!/usr/bin/env node
/**
 * The shared-boundary check: nothing in `shared/` may reach for a runtime.
 *
 * This is the rule the whole architecture rests on. `shared/` is imported by the browser and
 * by Node; the moment one module imports `node:fs` or touches `document`, one of the two
 * breaks — and the failure appears in the *other* runtime, which is the worst place to debug
 * it from. So it is a gate, not a convention.
 *
 * The check is deliberately textual **and** structural: it scans the source for a runtime
 * import or a DOM global, and then it actually imports every module in a bare Node context
 * with the browser globals absent. A file that passes the scan but throws on import is still
 * a failure — which is the case a scan alone would miss.
 *
 * Usage: node tools/check-boundary.js [--quiet]
 */

import { readFile, readdir } from 'node:fs/promises';
import { join, relative, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const root = resolve(import.meta.dirname, '..');
const sharedDir = join(root, 'shared');

/**
 * Globals a module in `shared/` must never reference.
 *
 * `document` is deliberately **absent**, and the reason is worth recording because the first
 * version of this check had it and was wrong. The domain names its visual document
 * `document` throughout — `const { project, document } = bundle`, `export * as document from
 * './document.js'` — so a textual search for it fires on correct code. It fired on seven
 * modules, every one of them a false positive.
 *
 * A check that fires wrongly is worse than no check, because it teaches people to ignore the
 * gate. Real DOM usage is caught where it can be caught reliably instead:
 *   - structurally, by importing every module below in a bare Node context;
 *   - behaviourally, by `test/boundary.test.js`, which replaces the browser globals with
 *     throwing accessors and then exercises the domain. A reference inside a function body
 *     throws there, which a text scan could not see anyway.
 */
const FORBIDDEN_GLOBALS = [
  'window', 'navigator', 'localStorage', 'indexedDB',
  'requestAnimationFrame', 'HTMLCanvasElement',
];

/** Modules a file in `shared/` must never import. */
const FORBIDDEN_MODULES = ['fs', 'path', 'os', 'http', 'https', 'child_process', 'worker_threads', 'zlib', 'crypto', 'stream', 'net'];

/** @param {string} dir @returns {Promise<string[]>} */
async function walk(dir) {
  /** @type {string[]} */
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...await walk(full));
    else if (entry.isFile() && entry.name.endsWith('.js')) out.push(full);
  }
  return out;
}

/**
 * Strip comments but keep string literals, so an import specifier is still visible.
 * @param {string} source
 * @returns {string}
 */
export function stripComments(source) {
  let out = '';
  let i = 0;
  while (i < source.length) {
    const ch = source[i];
    const next = source[i + 1];
    if (ch === '/' && next === '/') {
      while (i < source.length && source[i] !== '\n') i++;
      continue;
    }
    if (ch === '/' && next === '*') {
      i += 2;
      while (i < source.length && !(source[i] === '*' && source[i + 1] === '/')) i++;
      i += 2;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch;
      out += ch;
      i++;
      while (i < source.length && source[i] !== quote) {
        if (source[i] === '\\') { out += source[i]; i++; }
        out += source[i];
        i++;
      }
      out += quote;
      i++;
      continue;
    }
    out += ch;
    i++;
  }
  return out;
}

/**
 * Strip strings and comments before scanning globals, so that a mention of `document` inside
 * a documentation comment or a string is not a violation — the rule is about code, and a
 * check that fires on prose is a check people learn to ignore.
 * @param {string} source
 * @returns {string}
 */
export function stripNonCode(source) {
  let out = '';
  let i = 0;
  while (i < source.length) {
    const ch = source[i];
    const next = source[i + 1];
    if (ch === '/' && next === '/') {
      while (i < source.length && source[i] !== '\n') i++;
      continue;
    }
    if (ch === '/' && next === '*') {
      i += 2;
      while (i < source.length && !(source[i] === '*' && source[i + 1] === '/')) i++;
      i += 2;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      const quote = ch;
      out += ' '; // keep offsets readable but drop the contents
      i++;
      while (i < source.length && source[i] !== quote) {
        if (source[i] === '\\') i++;
        i++;
      }
      i++;
      continue;
    }
    out += ch;
    i++;
  }
  return out;
}

/**
 * @returns {Promise<{ problems: string[], modules: number }>}
 */
export async function checkBoundary() {
  /** @type {Set<string>} */
  const problems = new Set();
  /** @param {string} p */
  const report = (p) => problems.add(p);
  const files = await walk(sharedDir);

  for (const file of files) {
    const rel = relative(root, file);
    const source = await readFile(file, 'utf8');
    // Two views of the same file, because the two checks need opposite things. The import
    // scan needs the string literals; the global scan must not see a word mentioned in a
    // comment. Running both against one view is how the first version of this file silently
    // stopped checking imports at all — `stripNonCode` removed the specifiers it was looking
    // for, and only the structural half kept the gate honest.
    const code = stripNonCode(source);
    const imports = stripComments(source);

    // `node:`-prefixed imports, and bare builtin names.
    const importRe = /(?:import\s[^;]*?from\s*['"]([^'"]+)['"]|require\(\s*['"]([^'"]+)['"]\s*\)|import\(\s*['"]([^'"]+)['"]\s*\))/g;
    for (const match of imports.matchAll(importRe)) {
      const specifier = match[1] ?? match[2] ?? match[3];
      if (!specifier) continue;
      if (specifier.startsWith('node:')) {
        report(`${rel}: imports ${specifier}`);
        continue;
      }
      if (FORBIDDEN_MODULES.includes(specifier)) {
        report(`${rel}: imports the Node builtin '${specifier}'`);
      }
      if (specifier.startsWith('/') || /^[a-z@][^./]*$/.test(specifier)) {
        // A bare specifier resolves to a package. `shared/` may depend on nothing at all.
        report(`${rel}: imports the package '${specifier}', which a browser must fetch`);
      }
    }

    for (const global of FORBIDDEN_GLOBALS) {
      const re = new RegExp(`(?<![.\\w$])${global}\\b`);
      if (re.test(code)) report(`${rel}: references the browser global '${global}'`);
    }
    if (/(?<![.\w$])process\b/.test(code)) report(`${rel}: references 'process'`);
    if (/(?<![.\w$])fetch\s*\(/.test(code)) report(`${rel}: calls fetch()`);
  }

  // Now the structural half: import every module with the browser absent. A module that
  // passes the text scan and still fails to import is exactly the case a scan cannot catch.
  for (const file of files) {
    const rel = relative(root, file);
    try {
      await import(pathToFileURL(file).href);
    } catch (error) {
      report(`${rel}: failed to import in a bare Node context — ${/** @type {Error} */ (error).message}`);
    }
  }

  return { problems: [...problems], modules: files.length };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const { problems, modules } = await checkBoundary();
  if (problems.length === 0) {
    console.log(`boundary ok — ${modules} module(s) in shared/ import cleanly with no runtime`);
    process.exit(0);
  }
  console.error(`boundary violated (${problems.length} problem(s)):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
