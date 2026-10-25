/**
 * `comic.project.*` — the lifecycle of a comic project: create it, open it, save it, and ask
 * it what is wrong with it.
 * @module shared/commands/commands/project
 */

import { fail, ok } from '../registry.js';
import { createBundle, DEFAULT_PAGE_SIZE } from '../../domain/project.js';
import { serialize, deserialize, migrate } from '../../domain/bundle.js';
import { errorsOf, validateBundle, warningsOf } from '../../domain/validate.js';
import { TEMPLATES } from '../../domain/layout.js';

/** @param {any} ctx @returns {any} */
const storageOf = (ctx) => {
  const s = /** @type {any} */ (ctx.services ?? {}).storage;
  if (!s) throw new Error('no storage service is configured');
  return s;
};

/** @type {import('../registry.js').RegisteredCommand[]} */
export const projectCommands = [
  {
    id: 'comic.project.create',
    label: 'New Comic',
    params: {
      type: 'object',
      properties: {
        title: { type: 'string', minLength: 1, maxLength: 200, description: 'The comic\u2019s title.' },
        pageWidth: { type: 'integer', minimum: 16, maximum: 20000, description: 'Default page width in page units.' },
        pageHeight: { type: 'integer', minimum: 16, maximum: 20000, description: 'Default page height in page units.' },
      },
      required: ['title'],
      additionalProperties: false,
    },
    run(ctx, params) {
      const bundle = createBundle(params.title);
      bundle.project.defaultPageSize = {
        width: params.pageWidth ?? DEFAULT_PAGE_SIZE.width,
        height: params.pageHeight ?? DEFAULT_PAGE_SIZE.height,
      };
      return ok({ bundle, projectId: bundle.project.id, title: bundle.project.title });
    },
  },

  {
    id: 'comic.project.info',
    label: 'Project Info',
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx) {
      const { project, document } = ctx.bundle;
      const problems = validateBundle(ctx.bundle);
      return ok({
        projectId: project.id,
        title: project.title,
        schemaVersion: project.schemaVersion,
        chapters: Object.keys(project.chapters).length,
        scenes: Object.keys(project.scenes).length,
        pages: Object.keys(project.pages).length,
        panels: Object.keys(project.panels).length,
        lettering: Object.keys(project.lettering).length,
        artboards: document.artboards.length,
        nodes: Object.keys(document.nodes).length,
        errors: errorsOf(problems).length,
        warnings: warningsOf(problems).length,
      });
    },
  },

  {
    id: 'comic.project.validate',
    label: 'Validate Comic',
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx) {
      const problems = validateBundle(ctx.bundle);
      return ok({ problems, errors: errorsOf(problems).length, warnings: warningsOf(problems).length });
    },
  },

  {
    id: 'comic.project.serialize',
    label: 'Serialize Project',
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx) {
      const result = serialize(ctx.bundle);
      if (!result.ok) {
        const first = result.problems.find((p) => p.severity === 'error');
        return fail('conflict', `the project is not valid, so it cannot be saved: ${first?.message ?? 'unknown problem'}`);
      }
      return ok({
        manifest: ctx.bundle.manifest ?? null,
        files: [...result.files.keys()].sort(),
      });
    },
  },

  {
    id: 'comic.project.save',
    label: 'Save Comic',
    serverOnly: true,
    params: {
      type: 'object',
      properties: { path: { type: 'string', minLength: 1, description: 'Project directory, relative to the storage root.' } },
      required: ['path'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const storage = storageOf(ctx);
      const result = serialize(ctx.bundle);
      if (!result.ok) {
        const first = result.problems.find((p) => p.severity === 'error');
        return fail('conflict', `refusing to save an invalid project: ${first?.message ?? 'unknown problem'}`);
      }
      await storage.ensureRoot(params.path);
      // Everything else first, the manifest last: it must never be readable while the data
      // it names is half-written.
      const entries = [...result.files.entries()];
      const manifest = entries.filter(([p]) => p === 'manifest.json');
      const rest = entries.filter(([p]) => p !== 'manifest.json');
      for (const [path, data] of [...rest, ...manifest]) {
        await storage.write(`${params.path}/${path}`, typeof data === 'string' ? data : new TextDecoder().decode(data));
      }
      return ok({ path: params.path, files: entries.length });
    },
  },

  {
    id: 'comic.project.open',
    label: 'Open Comic',
    serverOnly: true,
    params: {
      type: 'object',
      properties: { path: { type: 'string', minLength: 1, description: 'Project directory, relative to the storage root.' } },
      required: ['path'],
      additionalProperties: false,
    },
    run: async (ctx, params) => {
      const storage = storageOf(ctx);
      const paths = await storage.list(params.path);
      if (paths.length === 0) return fail('not-found', `no project at ${params.path}`);
      /** @type {Map<string, string>} */
      const files = new Map();
      for (const p of paths) {
        const data = await storage.read(p);
        if (data === undefined) continue;
        files.set(p.slice(params.path.length + 1), data);
      }
      const result = deserialize(files);
      if (!result.ok) {
        return fail('not-found', `the bundle is incomplete: missing ${result.missing.join(', ') || 'required files'}`);
      }
      let migrated;
      try {
        migrated = migrate(result.bundle);
      } catch (error) {
        return fail('conflict', /** @type {Error} */ (error).message);
      }
      const problems = validateBundle(migrated.bundle);
      return ok({
        bundle: migrated.bundle,
        migrated: migrated.migrated,
        problems,
        errors: errorsOf(problems).length,
        warnings: warningsOf(problems).length,
      });
    },
  },

  {
    id: 'comic.project.setDefaultTemplate',
    label: 'Set Default Template',
    params: {
      type: 'object',
      properties: {
        template: {
          type: ['string', 'null'],
          enum: [...Object.keys(TEMPLATES), null],
          description: 'Template used when a new page does not name one, or null to clear it.',
        },
      },
      required: ['template'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: [] },
    run(ctx, params) {
      ctx.bundle.project.defaultTemplate = params.template;
      return ok({ defaultTemplate: params.template });
    },
  },
];
