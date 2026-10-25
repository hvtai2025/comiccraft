/**
 * `comic.page.*` tools that complete the design-named page surface without changing the M1
 * commands: create, reorder, resize, apply a template, set status, and choose a reading
 * direction.
 *
 * @module shared/commands/commands/page-tools
 */

import { fail, ok } from '../registry.js';
import { findArtboard, removeNode } from '../../domain/document.js';
import { unlinkAssetNodes } from '../../domain/project.js';
import { TEMPLATES, template as templateOf, templateWarnings } from '../../domain/layout.js';
import { validateReadingOrder } from '../../domain/validate.js';
import { fontEnvironment, missingFont } from '../../domain/fonts.js';
import { applyGrid, pageAdd } from './page.js';
import { geometryWarnings } from './panel.js';

/**
 * Remove every panel on a page, with the objects and records it owns. Assets and character
 * records survive: provenance outlives the panel that used it.
 * @param {any} bundle
 * @param {any} page
 */
function clearPanels(bundle, page) {
  const { project, document } = bundle;
  for (const panelId of [...page.panelIds]) {
    const panel = project.panels[panelId];
    if (!panel) continue;
    for (const lid of panel.letteringIds) {
      const lettering = project.lettering[lid];
      if (lettering) {
        removeNode(document, lettering.nodeId);
        if (lettering.tailNodeId !== undefined && lettering.tailNodeId !== null) removeNode(document, lettering.tailNodeId);
      }
      delete project.lettering[lid];
    }
    for (const did of panel.dialogueIds) delete project.dialogue[did];
    for (const ref of panel.artworkRefs) removeNode(document, Number(ref));
    unlinkAssetNodes(project, panel.artworkRefs);
    removeNode(document, panel.frameNodeId);
    delete project.panels[panelId];
  }
  page.panelIds = [];
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const pageToolCommands = [
  {
    id: 'comic.page.create',
    label: 'Create Page',
    params: {
      type: 'object',
      properties: {
        name: { type: 'string', minLength: 1, maxLength: 200 },
        sceneId: { type: 'string', minLength: 1 },
        template: { type: 'string', minLength: 1 },
        width: { type: 'integer', minimum: 16, maximum: 20000 },
        height: { type: 'integer', minimum: 16, maximum: 20000 },
      },
      required: ['name'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['pages'] },
    run: (ctx, params) => pageAdd(ctx, params),
  },

  {
    id: 'comic.page.reorder',
    label: 'Reorder Pages',
    params: {
      type: 'object',
      properties: {
        sceneId: { type: 'string', minLength: 1 },
        order: { type: 'array', minItems: 1, items: { type: 'string', minLength: 1 }, description: 'Every page id in the scene, in the intended order.' },
      },
      required: ['sceneId', 'order'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: [] },
    run(ctx, params) {
      const scene = ctx.bundle.project.scenes[params.sceneId];
      if (!scene) return fail('not-found', `no scene ${params.sceneId}`, 'sceneId');
      const current = [...scene.pageIds].sort();
      const proposed = [...params.order].sort();
      if (current.length !== proposed.length || current.some((id, i) => id !== proposed[i])) {
        return fail(
          'bad-params',
          `the order must list exactly the ${current.length} page(s) in scene ${params.sceneId}, each once`,
          'order',
        );
      }
      scene.pageIds = [...params.order];
      return ok({ sceneId: params.sceneId, order: [...scene.pageIds] });
    },
  },

  {
    id: 'comic.page.setSize',
    label: 'Set Page Size',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        width: { type: 'integer', minimum: 16, maximum: 20000 },
        height: { type: 'integer', minimum: 16, maximum: 20000 },
      },
      required: ['pageId', 'width', 'height'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages'] },
    run(ctx, params) {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const artboard = findArtboard(ctx.bundle.document, page.artboardId);
      if (!artboard) return fail('conflict', `page ${params.pageId} has no artboard`, 'pageId');
      artboard.size = { width: params.width, height: params.height };
      return ok({ pageId: params.pageId, size: { ...artboard.size }, warnings: geometryWarnings(ctx.bundle, page) });
    },
  },

  {
    id: 'comic.page.template.apply',
    label: 'Apply Page Template',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        template: { type: 'string', minLength: 1 },
        replace: { type: 'boolean', description: 'Replace every panel on the page. Default false, which refuses a page that already has panels.' },
      },
      required: ['pageId', 'template'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages', 'panels', 'lettering', 'dialogue', 'assets'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const page = bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      if (!TEMPLATES[params.template]) {
        return fail('bad-params', `no template ${params.template}; known templates: ${Object.keys(TEMPLATES).join(', ')}`, 'template');
      }
      if (page.panelIds.length > 0 && params.replace !== true) {
        return fail(
          'conflict',
          `page ${params.pageId} already has ${page.panelIds.length} panel(s); pass replace: true to rebuild the page`,
          'replace',
        );
      }
      const warnings = [];
      if (page.panelIds.length > 0) {
        clearPanels(bundle, page);
        warnings.push({ code: 'panels-replaced', message: `replaced every panel on page ${params.pageId}; the previous panels and their lettering were removed in this undoable step` });
      }
      const t = templateOf(params.template);
      const artboard = findArtboard(bundle.document, page.artboardId);
      if (artboard) warnings.push(...templateWarnings(artboard.size, t));
      const panels = applyGrid(ctx, page, t);
      return ok({ pageId: params.pageId, template: params.template, panels, warnings });
    },
  },

  {
    id: 'comic.page.setStatus',
    label: 'Set Page Status',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        status: { type: 'string', enum: ['draft', 'generated', 'lettered', 'approved', 'published'] },
      },
      required: ['pageId', 'status'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['pages'] },
    run(ctx, params) {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const previous = page.status ?? 'draft';
      page.status = params.status;
      return ok({ pageId: params.pageId, status: params.status, previous });
    },
  },

  {
    id: 'comic.reading.setDirection',
    label: 'Set Reading Direction',
    params: {
      type: 'object',
      properties: { direction: { type: 'string', enum: ['ltr', 'rtl'] } },
      required: ['direction'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: [] },
    run(ctx, params) {
      const previous = ctx.bundle.project.metadata.readingDirection ?? 'ltr';
      ctx.bundle.project.metadata.readingDirection = params.direction;
      return ok({ direction: params.direction, previous });
    },
  },

  {
    id: 'comic.reading.set',
    label: 'Set Reading Order',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        order: { type: 'array', minItems: 1, items: { type: 'string', minLength: 1 } },
      },
      required: ['pageId', 'order'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages'] },
    run(ctx, params) {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const current = [...page.panelIds].sort();
      const proposed = [...params.order].sort();
      if (current.length !== proposed.length || current.some((id, i) => id !== proposed[i])) {
        return fail('bad-params', `the order must list exactly the ${current.length} panel(s) on page ${params.pageId}, each once`, 'order');
      }
      page.panelIds = [...params.order];
      return ok({ pageId: params.pageId, order: [...page.panelIds] });
    },
  },

  {
    id: 'comic.reading.validate',
    label: 'Validate Reading Order',
    params: {
      type: 'object',
      properties: { pageId: { type: 'string', minLength: 1 } },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const page = ctx.bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      const problems = validateReadingOrder(ctx.bundle.project, params.pageId);
      return ok({
        pageId: params.pageId,
        order: [...page.panelIds],
        direction: ctx.bundle.project.metadata.readingDirection ?? 'ltr',
        valid: problems.length === 0,
        problems,
      });
    },
  },

  {
    id: 'comic.project.setLetteringDefault',
    label: 'Set Default Lettering Style',
    params: {
      type: 'object',
      properties: {
        family: { type: 'string', minLength: 1, maxLength: 100 },
        sizePt: { type: 'number', minimum: 4, maximum: 96 },
        align: { type: 'string', enum: ['left', 'center', 'right'] },
        padding: { type: 'number', minimum: 0, maximum: 80 },
        bold: { type: 'boolean' },
        italic: { type: 'boolean' },
      },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: [] },
    run(ctx, params) {
      const style = ctx.bundle.project.defaultLetteringStyle;
      if (params.family !== undefined) style.family = params.family;
      if (params.sizePt !== undefined) style.sizePt = params.sizePt;
      if (params.align !== undefined) style.align = params.align;
      if (params.padding !== undefined) style.padding = params.padding;
      if (params.bold !== undefined) style.bold = params.bold;
      if (params.italic !== undefined) style.italic = params.italic;
      const missing = missingFont({ family: style.family }, fontEnvironment(ctx.services?.fonts));
      return ok({ style: { ...style }, missingFont: missing });
    },
  },

  {
    id: 'comic.scene.list',
    label: 'List Scenes',
    params: {
      type: 'object',
      properties: { chapterId: { type: 'string', minLength: 1 } },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const { project } = ctx.bundle;
      const chapters = params.chapterId ? [project.chapters[params.chapterId]].filter(Boolean) : Object.values(project.chapters);
      const scenes = [];
      for (const chapter of chapters) {
        const c = /** @type {any} */ (chapter);
        for (const sceneId of c.sceneIds) {
          const scene = project.scenes[sceneId];
          if (!scene) continue;
          scenes.push({
            sceneId,
            chapterId: c.id,
            name: scene.name,
            summary: scene.summary ?? '',
            pageIds: [...scene.pageIds],
          });
        }
      }
      return ok({ scenes });
    },
  },
];
