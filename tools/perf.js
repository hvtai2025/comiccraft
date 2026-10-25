#!/usr/bin/env node
/**
 * A comic-shaped performance check.
 *
 * The reference harness benchmarks a synthetic 50k-path illustration, which is not what a
 * comic is. This builds a book of many artboards, raster panels and lettering objects, then
 * times the things a creator waits for: build, save, open, validate and render.
 *
 * The budgets are regression alarms, not benchmarks. `PERF_STRICT=1` tightens them for a
 * release run; `--json` prints the numbers for a CI artifact.
 *
 * Usage: node tools/perf.js [--json]
 */

import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createSession } from '../server/session.js';
import { packageProject } from '../server/export/package.js';

const PAGES = Number(process.env.PERF_PAGES ?? (process.env.PERF_STRICT === '1' ? 40 : 24));
const PANELS_PER_PAGE = 4;

/**
 * @param {number} ms
 * @param {string} label
 * @param {number} budget
 * @param {string[]} failures
 */
function check(ms, label, budget, failures) {
  const ok = ms <= budget;
  console.log(`  ${ok ? 'ok  ' : 'SLOW'} ${label.padEnd(22)} ${ms.toFixed(1).padStart(8)} ms  (budget ${budget} ms)`);
  if (!ok) failures.push(`${label}: ${ms.toFixed(1)} ms > ${budget} ms`);
}

async function main() {
  const root = await mkdtemp(join(tmpdir(), 'comiccraft-perf-'));
  const session = createSession({ root });
  /** @type {string[]} */
  const failures = [];
  /** @type {Record<string, number>} */
  const timings = {};

  const time = async (/** @type {string} */ label, /** @type {() => Promise<any>|any} */ fn) => {
    const start = performance.now();
    const value = await fn();
    timings[label] = performance.now() - start;
    return value;
  };

  await time('project.create', async () => {
    const result = await session.run('comic.project.create', { title: 'Performance Book' });
    if (!result.ok) throw new Error(result.message);
  });

  await time('build', async () => {
    for (let page = 0; page < PAGES; page++) {
      const created = await session.run('comic.page.add', {
        name: `Page ${page + 1}`, template: 'two-by-two',
        ...(page % 7 === 0 ? { width: 1275, height: 2550 } : {}),
      });
      if (!created.ok) throw new Error(created.message);
      const pageId = created.value.pageId;
      const panelIds = session.bundle.project.pages[pageId].panelIds.slice(0, PANELS_PER_PAGE);
      for (let i = 0; i < panelIds.length; i++) {
        const panelId = panelIds[i];
        await session.run('comic.artwork.place', {
          panelId, name: `Art ${page}-${i}`, contentHash: `perf-${page}-${i}`,
          data: 'data:image/svg+xml;base64,PHN2Zy8+',
        });
        await session.run('comic.artwork.clip', { panelId, inset: 6 });
        await session.run('comic.lettering.add', {
          panelId,
          kind: i === 0 ? 'caption' : 'balloon',
          variant: i === 0 ? undefined : 'speech',
          text: `PAGE ${page + 1} PANEL ${i + 1}: a line of dialogue that has to wrap inside its balloon.`,
          overflowPolicy: 'shrink',
        });
      }
    }
  });

  const save = await time('save', () => session.run('comic.project.save', { path: 'book' }));
  if (!save.ok) throw new Error(save.message);
  const open = await time('open', () => session.run('comic.project.open', { path: 'book' }));
  if (!open.ok) throw new Error(open.message);

  const validate = await time('validate', () => session.run('comic.book.validate', { preset: 'proofing' }));
  if (!validate.ok) throw new Error(validate.message);

  const firstPageId = Object.keys(session.bundle.project.pages)[0];
  await time('render.png', () => session.run('comic.page.render', { pageId: firstPageId, format: 'png', scale: 0.35 }));

  const packaged = await time('package', () => packageProject(session.bundle));
  if (!packaged.ok) throw new Error('the book could not be packaged');
  const archiveBytes = packaged.ok ? packaged.bytes.length : 0;

  const strict = process.env.PERF_STRICT === '1';
  const budgets = {
    build: strict ? 15_000 : 30_000,
    save: strict ? 2_000 : 4_000,
    open: strict ? 3_000 : 5_000,
    validate: strict ? 2_000 : 4_000,
    'render.png': strict ? 2_000 : 4_000,
    package: strict ? 2_000 : 4_000,
  };
  for (const [label, budget] of Object.entries(budgets)) {
    check(timings[label] ?? 0, label, budget, failures);
  }

  const summary = {
    pages: Object.keys(session.bundle.project.pages).length,
    panels: Object.keys(session.bundle.project.panels).length,
    lettering: Object.keys(session.bundle.project.lettering).length,
    archiveBytes,
    timings,
    budgets,
    failures,
  };
  if (process.argv.includes('--json')) process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  else {
    console.log(`\n${summary.pages} page(s), ${summary.panels} panel(s), ${summary.lettering} lettering object(s), package ${Math.round(archiveBytes / 1024)} KB`);
  }

  await rm(root, { recursive: true, force: true });
  if (failures.length > 0) {
    console.error(`\nperformance failed: ${failures.length} budget(s) exceeded`);
    return 1;
  }
  console.log('\nperformance ok — the comic-shaped book fits its budgets');
  return 0;
}

process.exitCode = await main();
