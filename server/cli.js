#!/usr/bin/env node
/**
 * ComicCraft from the command line — the same commands an agent drives over MCP or a user
 * clicks in the browser.
 *
 * ```sh
 * node server/cli.js commands
 * node server/cli.js run --cmd comic.project.create --params '{"title":"My Comic"}'
 * node server/cli.js render --page page_1 --out out/page-1.png
 * ```
 *
 * @module server/cli
 */

import { parseArgs } from 'node:util';
import { readFile } from 'node:fs/promises';
import { createSession } from './session.js';
import { orderedPages } from '../shared/commands/commands/export.js';
import { errorsOf } from '../shared/domain/validate.js';

/**
 * @param {string[]} argv
 * @returns {Promise<number>} the exit code
 */
export async function main(argv) {
  const [subcommand, ...rest] = argv;

  if (!subcommand || subcommand === 'help' || subcommand === '--help') {
    process.stdout.write(usage());
    return 0;
  }

  const session = createSession({ root: process.env.COMICCRAFT_ROOT });

  if (subcommand === 'commands') {
    const { values } = parseArgs({ args: rest, options: { json: { type: 'boolean' } }, allowPositionals: true });
    const described = session.registry.describe();
    if (values.json) {
      process.stdout.write(`${JSON.stringify(described, null, 2)}\n`);
      return 0;
    }
    for (const c of described) {
      process.stdout.write(`${c.serverOnly ? '[server]' : '        '} ${c.id.padEnd(34)} ${c.label}\n`);
    }
    process.stdout.write(`\n${described.length} command(s)\n`);
    return 0;
  }

  if (subcommand === 'run') {
    const { values } = parseArgs({
      args: rest,
      options: {
        cmd: { type: 'string', short: 'c' },
        params: { type: 'string', short: 'p' },
        'params-file': { type: 'string' },
        project: { type: 'string' },
      },
      allowPositionals: true,
    });
    if (!values.cmd) {
      process.stderr.write('run needs --cmd\n');
      return 2;
    }
    let params = {};
    if (values['params-file']) params = JSON.parse(await readFile(values['params-file'], 'utf8'));
    if (values.params) params = JSON.parse(values.params);

    if (values.project) {
      const opened = await session.run('comic.project.open', { path: values.project });
      if (!opened.ok) {
        process.stderr.write(`${opened.kind}: ${opened.message}\n`);
        return 1;
      }
    }

    const result = await session.run(values.cmd, params);
    // The bundle is large and is not what a caller wants to read; report what changed.
    const printable = { ...result };
    if (printable.ok && printable.value && typeof printable.value === 'object' && 'bundle' in printable.value) {
      const bundle = printable.value.bundle;
      printable.value = {
        .../** @type {Record<string, unknown>} */ (printable.value),
        bundle: {
          projectId: bundle.project.id,
          title: bundle.project.title,
          pages: Object.keys(bundle.project.pages).length,
          artboards: bundle.document.artboards.length,
        },
      };
    }
    process.stdout.write(`${JSON.stringify(printable, null, 2)}\n`);
    return result.ok ? 0 : 1;
  }

  if (subcommand === 'render') {
    const { values } = parseArgs({
      args: rest,
      options: {
        page: { type: 'string' },
        out: { type: 'string' },
        scale: { type: 'string' },
        project: { type: 'string' },
        format: { type: 'string' },
      },
      allowPositionals: true,
    });
    if (values.project) {
      const opened = await session.run('comic.project.open', { path: values.project });
      if (!opened.ok) {
        process.stderr.write(`${opened.kind}: ${opened.message}\n`);
        return 1;
      }
    }
    if (!values.page) {
      process.stderr.write('render needs --page (or --format pdf for the whole book)\n');
      return 2;
    }
    const format = values.format ?? (values.out?.endsWith('.pdf') ? 'pdf' : values.out?.endsWith('.svg') ? 'svg' : 'png');
    const out = values.out ?? `out/${values.page}.${format}`;

    const result = format === 'svg'
      ? await session.run('comic.export.svg', { pageId: values.page, path: out })
      : format === 'pdf'
        ? await session.run('comic.export.pdf', { path: out, pageIds: [values.page] })
        : await session.run('comic.export.png', { pageId: values.page, path: out, scale: values.scale ? Number(values.scale) : 1 });

    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return result.ok ? 0 : 1;
  }

  if (subcommand === 'demo') {
    return demo(session);
  }

  process.stderr.write(`unknown subcommand: ${subcommand}\n\n${usage()}`);
  return 2;
}

/**
 * Build a three-panel page end to end. This is the M1 vertical slice as one command, and it
 * is what the smoke test and the README both point at.
 * @param {any} session
 * @returns {Promise<number>}
 */
async function demo(session) {
  const steps = [
    ['comic.project.create', { title: 'Three Panel Demo' }],
    ['comic.page.add', { name: 'Page 1', template: 'three-up' }],
    ['comic.character.add', { name: 'Narrator' }],
  ];

  for (const [cmd, params] of steps) {
    const result = await session.run(cmd, params);
    if (!result.ok) {
      process.stderr.write(`${cmd} failed: ${result.message}\n`);
      return 1;
    }
    process.stdout.write(`✓ ${cmd}\n`);
  }

  const pageId = Object.keys(session.bundle.project.pages)[0];
  const page = session.bundle.project.pages[pageId];
  const panels = page.panelIds;

  const characterId = Object.keys(session.bundle.project.characters)[0];
  const lines = [
    'THE CITY WOKE SLOWLY THAT MORNING,',
    'AND NOTHING IN IT KNEW WHAT WAS COMING.',
    'KRAKOOM!',
  ];
  const kinds = ['balloon', 'caption', 'sound-effect'];

  for (let i = 0; i < panels.length; i++) {
    const placed = await session.run('comic.artwork.place', {
      panelId: panels[i],
      name: `Panel ${i + 1} art`,
      contentHash: `demo-panel-${i + 1}`,
      provider: 'demo',
    });
    if (!placed.ok) {
      process.stderr.write(`place failed: ${placed.message}\n`);
      return 1;
    }
    await session.run('comic.artwork.crop', { panelId: panels[i], x: 0.05, y: 0.05, width: 0.9, height: 0.9 });
    await session.run('comic.artwork.clip', { panelId: panels[i], inset: 6 });

    const lettered = await session.run('comic.lettering.add', {
      panelId: panels[i],
      kind: kinds[i],
      text: lines[i],
      ...(kinds[i] === 'balloon' ? { characterId } : {}),
      overflowPolicy: i === 0 ? 'shrink' : 'warn',
    });
    if (!lettered.ok) {
      process.stderr.write(`lettering failed: ${lettered.message}\n`);
      return 1;
    }
    process.stdout.write(`✓ panel ${i + 1}: artwork placed, cropped, clipped, ${kinds[i]} added\n`);
  }

  const problems = await session.run('comic.project.validate', {});
  process.stdout.write(`✓ validate: ${problems.value.errors} error(s), ${problems.value.warnings} warning(s)\n`);

  const first = panels[0];
  const moved = await session.run('comic.lettering.move', {
    letteringId: session.bundle.project.panels[first].letteringIds[0],
    dx: 20, dy: -10,
  });
  process.stdout.write(`✓ moved a balloon; tail attached: ${Boolean(moved.value.tail?.attached)}\n`);

  const undone = await session.run('comic.edit.undo', {});
  process.stdout.write(`✓ undo: ${undone.value?.undone ?? 'nothing'} (${undone.value?.undoDepth ?? 0} step(s) left)\n`);

  const pages = orderedPages(session.bundle);
  /** @type {[string, string, Record<string, unknown>][]} */
  const formats = [
    ['svg', 'comic.export.svg', {}],
    ['png', 'comic.export.png', { scale: 0.25 }],
  ];
  for (const [format, cmd, extra] of formats) {
    const out = `out/demo-page-1.${format}`;
    const result = await session.run(cmd, { pageId: pageId, path: out, ...extra });
    if (!result.ok) {
      process.stderr.write(`${format} export failed: ${result.message}\n`);
      return 1;
    }
    process.stdout.write(`✓ ${format} → ${out} (${result.value.bytes} bytes)\n`);
  }

  const pdf = await session.run('comic.export.pdf', { path: 'out/demo.pdf' });
  if (!pdf.ok) {
    process.stderr.write(`pdf export failed: ${pdf.message}\n`);
    return 1;
  }
  process.stdout.write(`✓ pdf → out/demo.pdf (${pdf.value.pages} page(s), ${pdf.value.bytes} bytes)\n`);

  const saved = await session.run('comic.project.save', { path: 'demo' });
  if (!saved.ok) {
    process.stderr.write(`save failed: ${saved.message}\n`);
    return 1;
  }
  process.stdout.write(`✓ saved ${saved.value.files} file(s) to demo/\n`);

  const reopened = await session.run('comic.project.open', { path: 'demo' });
  if (!reopened.ok) {
    process.stderr.write(`reopen failed: ${reopened.message}\n`);
    return 1;
  }
  const errors = errorsOf(reopened.value.problems);
  process.stdout.write(
    `✓ reopened: ${Object.keys(reopened.value.bundle.project.pages).length} page(s), ` +
    `${Object.keys(reopened.value.bundle.project.panels).length} panel(s), ` +
    `${errors.length} error(s)\n`,
  );
  void pages;
  return errors.length === 0 ? 0 : 1;
}

/** @returns {string} */
function usage() {
  return `ComicCraft

  node server/cli.js commands [--json]
  node server/cli.js run --cmd <id> [--params '<json>' | --params-file <path>] [--project <dir>]
  node server/cli.js render --page <pageId> --out <file> [--scale 1] [--format png|svg|pdf] [--project <dir>]
  node server/cli.js demo

The storage root comes from COMICCRAFT_ROOT, and defaults to ./comiccraft-projects.
`;
}

process.exitCode = await main(process.argv.slice(2));
