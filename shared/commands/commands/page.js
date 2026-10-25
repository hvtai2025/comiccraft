/**
 * `comic.page.*` and `comic.chapter.*`/`comic.scene.*` — narrative structure, and the page
 * as an artboard.
 * @module shared/commands/commands/page
 */

import { fail, ok } from '../registry.js';
import { allocate, readingOrder, unlinkAssetNodes } from '../../domain/project.js';
import { addNode, createArtboard, deleteArtboard, findArtboard, removeNode } from '../../domain/document.js';
import { gridFrames, template as templateOf } from '../../domain/layout.js';

/**
 * Every comic project has exactly one chapter and one scene until the user makes more.
 * Auto-creating them is what lets `comic.page.add` work on a fresh project without the
 * caller knowing the hierarchy exists.
 * @param {any} bundle
 * @returns {{ chapterId: string, sceneId: string }}
 */
export function ensureStructure(bundle) {
  const { project } = bundle;
  let chapterId = Object.keys(project.chapters)[0];
  if (!chapterId) {
    chapterId = allocate(bundle, 'chapter');
    project.chapters[chapterId] = { id: chapterId, title: 'Chapter 1', sceneIds: [] };
  }
  const chapter = project.chapters[chapterId];
  let sceneId = chapter.sceneIds[0];
  if (!sceneId || !project.scenes[sceneId]) {
    sceneId = allocate(bundle, 'scene');
    project.scenes[sceneId] = { id: sceneId, name: 'Scene 1', pageIds: [] };
    chapter.sceneIds.push(sceneId);
  }
  return { chapterId, sceneId };
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const pageCommands = [
  {
    id: 'comic.chapter.add',
    label: 'Add Chapter',
    params: {
      type: 'object',
      properties: { title: { type: 'string', minLength: 1, maxLength: 200 } },
      required: ['title'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: [] },
    run(ctx, params) {
      const id = allocate(ctx.bundle, 'chapter');
      ctx.bundle.project.chapters[id] = { id, title: params.title, sceneIds: [] };
      return ok({ chapterId: id, title: params.title });
    },
  },

  {
    id: 'comic.scene.add',
    label: 'Add Scene',
    params: {
      type: 'object',
      properties: {
        chapterId: { type: 'string', minLength: 1 },
        name: { type: 'string', minLength: 1, maxLength: 200 },
      },
      required: ['chapterId', 'name'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: [] },
    run(ctx, params) {
      const chapter = ctx.bundle.project.chapters[params.chapterId];
      if (!chapter) return fail('not-found', `no chapter ${params.chapterId}`, 'chapterId');
      const id = allocate(ctx.bundle, 'scene');
      ctx.bundle.project.scenes[id] = { id, name: params.name, pageIds: [] };
      chapter.sceneIds.push(id);
      return ok({ sceneId: id, name: params.name });
    },
  },

  {
    id: 'comic.page.add',
    label: 'Add Page',
    params: {
      type: 'object',
      properties: {
        name: { type: 'string', minLength: 1, maxLength: 200, description: 'Shown in the page navigator.' },
        sceneId: { type: 'string', minLength: 1, description: 'Defaults to the first scene, creating one if the project has none.' },
        template: { type: 'string', minLength: 1, description: 'Panel template applied on creation. Defaults to the project setting.' },
        width: { type: 'integer', minimum: 16, maximum: 20000 },
        height: { type: 'integer', minimum: 16, maximum: 20000 },
      },
      required: ['name'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['pages'] },
    run(ctx, params) {
      return pageAdd(ctx, params);
    },
  },

  {
    id: 'comic.page.setTemplate',
    label: 'Apply Page Template',
    params: {
      type: 'object',
      properties: {
        pageId: { type: 'string', minLength: 1 },
        template: { type: 'string', minLength: 1, description: 'Replaces every panel on the page.' },
      },
      required: ['pageId', 'template'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['pages', 'panels'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const page = bundle.project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');
      if (page.panelIds.length > 0) {
        return fail(
          'conflict',
          `page ${params.pageId} already has ${page.panelIds.length} panel(s); a template replaces the whole page. ` +
          'Remove the panels first, or use comic.panel.add for a single panel.',
          'template',
        );
      }
      const panels = applyGrid(ctx, page, templateOf(params.template));
      return ok({ pageId: params.pageId, panels, template: params.template });
    },
  },

  {
    id: 'comic.page.info',
    label: 'Page Info',
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
      const artboard = findArtboard(ctx.bundle.document, page.artboardId);
      return ok({
        pageId: page.id,
        name: page.name,
        artboardId: page.artboardId,
        size: artboard?.size ?? null,
        readingOrder: readingOrder(ctx.bundle.project, page.id).map((p) => p.id),
      });
    },
  },

  {
    id: 'comic.page.delete',
    label: 'Delete Page',
    params: {
      type: 'object',
      properties: { pageId: { type: 'string', minLength: 1 } },
      required: ['pageId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['pages', 'panels', 'dialogue', 'lettering'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const { project, document } = bundle;
      const page = project.pages[params.pageId];
      if (!page) return fail('not-found', `no page ${params.pageId}`, 'pageId');

      // Panels own the objects; deleting the artboard without them would leave nodes
      // reachable from the project and nothing drawing them.
      const nodeIds = new Set();
      for (const pid of page.panelIds) {
        const panel = project.panels[pid];
        if (!panel) continue;
        nodeIds.add(panel.frameNodeId);
        for (const ref of panel.artworkRefs) nodeIds.add(Number(ref));
        for (const lid of panel.letteringIds) {
          const l = project.lettering[lid];
          if (l) { nodeIds.add(l.nodeId); if (l.tailNodeId) nodeIds.add(l.tailNodeId); }
        }
        for (const did of panel.dialogueIds) delete project.dialogue[did];
        for (const lid of panel.letteringIds) delete project.lettering[lid];
        delete project.panels[pid];
      }
      deleteArtboard(document, page.artboardId);
      for (const id of nodeIds) removeNode(document, id);
      unlinkAssetNodes(project, nodeIds);
      delete project.pages[params.pageId];
      for (const scene of Object.values(project.scenes)) {
        const s = /** @type {any} */ (scene);
        const i = s.pageIds.indexOf(params.pageId);
        if (i !== -1) s.pageIds.splice(i, 1);
      }
      return ok({ pageId: params.pageId, panels: page.panelIds.length });
    },
  },
];

/**
 * Create a page and, when a template is named, lay its panels out. Shared by
 * `comic.page.add` and the design-named alias `comic.page.create`, so the two cannot drift.
 * @param {any} ctx
 * @param {any} params
 * @returns {import('../registry.js').Result}
 */
export function pageAdd(ctx, params) {
  const { bundle } = ctx;
  const { project, document } = bundle;
  const { sceneId: defaultSceneId } = ensureStructure(bundle);
  const sceneId = params.sceneId ?? defaultSceneId;
  const scene = project.scenes[sceneId];
  if (!scene) return fail('not-found', `no scene ${sceneId}`, 'sceneId');

  const size = {
    width: params.width ?? project.defaultPageSize.width,
    height: params.height ?? project.defaultPageSize.height,
  };
  const artboardId = createArtboard(document, params.name, size);
  const pageId = allocate(bundle, 'page');
  /** @type {any} */
  const page = { id: pageId, name: params.name, artboardId, panelIds: [] };
  project.pages[pageId] = page;
  scene.pageIds.push(pageId);

  const templateName = params.template ?? project.defaultTemplate ?? null;
  let panels = 0;
  if (templateName) {
    const t = templateOf(templateName);
    panels = applyGrid(ctx, page, t);
  }
  return ok({ pageId, artboardId, sceneId, size, panels, template: templateName });
}

/**
 * Lay a template out on a page and create the panels. Returns how many were made.
 * @param {any} ctx
 * @param {any} page
 * @param {import('../../domain/layout.js').GridTemplate} t
 * @returns {number}
 */
export function applyGrid(ctx, page, t) {
  const { bundle } = ctx;
  const { project, document } = bundle;
  const artboard = findArtboard(document, page.artboardId);
  if (!artboard) throw new Error(`page ${page.id} has no artboard`);

  const direction = project.metadata?.readingDirection === 'rtl' ? 'rtl' : 'ltr';
  const frames = gridFrames(artboard.size, t, direction);
  for (const frame of frames) {
    const nodeId = addNode(document, page.artboardId, {
      kind: 'frame', name: 'Panel', frame,
      fill: '#ffffff', stroke: '#111111', strokeWidth: 3,
    });
    const panelId = allocate(bundle, 'panel');
    project.panels[panelId] = {
      id: panelId,
      frameNodeId: nodeId,
      dialogueIds: [],
      letteringIds: [],
      artworkRefs: [],
      continuityIds: [],
      locked: false,
    };
    // Reading order is the order the template produced them: left to right, top to bottom —
    // or right to left, top to bottom on an rtl page. The coordinates follow the direction,
    // and the list remains the source of truth.
    page.panelIds.push(panelId);
  }
  return frames.length;
}

