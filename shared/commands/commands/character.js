/**
 * `comic.character.*` — the character bible.
 *
 * A reference is **immutable**: approving a new design appends a version and moves
 * `approvedDesignVersion`, so pages approved against an older design are not silently put
 * behind a new face. Nothing here deletes an approved reference.
 *
 * @module shared/commands/commands/character
 */

import { fail, ok } from '../registry.js';
import { allocate } from '../../domain/project.js';
import { reviewDecision } from '../../domain/jobs.js';

/** @param {any} ctx @param {string} characterId @returns {any | null} */
const characterOf = (ctx, characterId) => ctx.bundle.project.characters[characterId] ?? null;

/** @type {import('../registry.js').RegisteredCommand[]} */
export const characterCommands = [
  {
    id: 'comic.character.get',
    label: 'Get Character',
    params: {
      type: 'object',
      properties: { characterId: { type: 'string', minLength: 1 } },
      required: ['characterId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const character = characterOf(ctx, params.characterId);
      if (!character) return fail('not-found', `no character ${params.characterId}`, 'characterId');
      return ok({ character });
    },
  },

  {
    id: 'comic.character.list',
    label: 'List Characters',
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx) {
      const characters = Object.values(ctx.bundle.project.characters).map((/** @type {any} */ c) => ({
        characterId: c.id,
        name: c.name,
        description: c.description ?? '',
        canonicalDescription: c.canonicalDescription ?? '',
        referenceAssetIds: [...(c.referenceAssetIds ?? [])],
        approvedDesignVersion: c.approvedDesignVersion ?? null,
        continuityRules: [...(c.continuityRules ?? [])],
      }));
      return ok({ characters, count: characters.length });
    },
  },

  {
    id: 'comic.character.update',
    label: 'Update Character',
    params: {
      type: 'object',
      properties: {
        characterId: { type: 'string', minLength: 1 },
        name: { type: 'string', minLength: 1, maxLength: 200 },
        description: { type: 'string', maxLength: 2000 },
        canonicalDescription: { type: 'string', maxLength: 4000, description: 'The appearance every panel must carry.' },
        face: { type: 'string', maxLength: 1000 },
        body: { type: 'string', maxLength: 1000 },
        clothing: { type: 'string', maxLength: 1000 },
        palette: { type: 'string', maxLength: 1000 },
        continuityRules: { type: 'array', items: { type: 'string', maxLength: 500 }, maxItems: 50 },
        continuityExceptions: { type: 'array', items: { type: 'string', maxLength: 500 }, maxItems: 50 },
      },
      required: ['characterId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['characters'] },
    run(ctx, params) {
      const character = characterOf(ctx, params.characterId);
      if (!character) return fail('not-found', `no character ${params.characterId}`, 'characterId');
      if (params.name !== undefined) character.name = params.name;
      if (params.description !== undefined) character.description = params.description;
      if (params.canonicalDescription !== undefined) character.canonicalDescription = params.canonicalDescription;
      if (params.face !== undefined || params.body !== undefined || params.clothing !== undefined || params.palette !== undefined) {
        character.appearance = {
          ...(character.appearance ?? {}),
          ...(params.face !== undefined ? { face: params.face } : {}),
          ...(params.body !== undefined ? { body: params.body } : {}),
          ...(params.clothing !== undefined ? { clothing: params.clothing } : {}),
          ...(params.palette !== undefined ? { palette: params.palette } : {}),
        };
      }
      if (params.continuityRules !== undefined) character.continuityRules = [...params.continuityRules];
      if (params.continuityExceptions !== undefined) character.continuityExceptions = [...params.continuityExceptions];
      return ok({ characterId: character.id, character });
    },
  },

  {
    id: 'comic.character.reference.add',
    label: 'Add Character Reference',
    params: {
      type: 'object',
      properties: {
        characterId: { type: 'string', minLength: 1 },
        assetId: { type: 'string', minLength: 1 },
      },
      required: ['characterId', 'assetId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['characters'] },
    run(ctx, params) {
      const character = characterOf(ctx, params.characterId);
      if (!character) return fail('not-found', `no character ${params.characterId}`, 'characterId');
      const asset = /** @type {any} */ (ctx.bundle.project.assets[params.assetId]);
      if (!asset) return fail('not-found', `no asset ${params.assetId}`, 'assetId');
      if (!character.referenceAssetIds.includes(asset.id)) character.referenceAssetIds.push(asset.id);
      return ok({ characterId: character.id, referenceAssetIds: [...character.referenceAssetIds] });
    },
  },

  {
    id: 'comic.character.approveDesign',
    label: 'Approve Character Design',
    params: {
      type: 'object',
      properties: {
        characterId: { type: 'string', minLength: 1 },
        assetId: { type: 'string', minLength: 1 },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['characterId', 'assetId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['characters', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const character = characterOf(ctx, params.characterId);
      if (!character) return fail('not-found', `no character ${params.characterId}`, 'characterId');
      const asset = /** @type {any} */ (bundle.project.assets[params.assetId]);
      if (!asset) return fail('not-found', `no asset ${params.assetId}`, 'assetId');
      const now = ctx.env.now ? ctx.env.now() : 0;
      /** @type {any[]} */
      const versions = character.designVersions ?? [];
      const next = versionNumber(versions);
      const previous = character.approvedDesignVersion ?? null;
      versions.push({
        version: next,
        assetId: asset.id,
        description: character.canonicalDescription ?? character.description ?? '',
        approvedAt: now,
      });
      character.designVersions = versions;
      character.approvedDesignVersion = asset.id;
      if (!character.referenceAssetIds.includes(asset.id)) character.referenceAssetIds.push(asset.id);
      asset.approvalStatus = 'approved';

      const decisionId = allocate(bundle, 'decision');
      bundle.project.reviews[decisionId] = reviewDecision({
        id: decisionId,
        subject: 'character',
        subjectId: character.id,
        decision: 'approved',
        actor: 'user',
        timestamp: now,
        note: params.note ?? null,
        supersededAssetId: previous,
      });
      return ok({
        characterId: character.id,
        designVersion: next,
        approvedDesignVersion: asset.id,
        supersededAssetId: previous,
        decisionId,
      });
    },
  },
];

/** @param {any[]} versions @returns {number} */
function versionNumber(versions) {
  let max = 0;
  for (const version of versions) {
    if (Number.isInteger(version.version) && version.version > max) max = version.version;
  }
  return max + 1;
}
