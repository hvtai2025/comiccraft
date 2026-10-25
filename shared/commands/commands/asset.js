/**
 * `comic.asset.*` — registering, listing and approving assets with provenance.
 *
 * An asset is content-addressed: the hash identifies the bytes, so registering the same file
 * twice returns the same record. Approved artwork is never overwritten — a replacement is a
 * new asset, and the review decision records what it superseded.
 *
 * @module shared/commands/commands/asset
 */

import { fail, ok } from '../registry.js';
import { allocate } from '../../domain/project.js';
import { reviewDecision } from '../../domain/jobs.js';

/** @param {any} bundle @param {string} contentHash @returns {any | null} */
const assetByHash = (bundle, contentHash) =>
  Object.values(bundle.project.assets).find((/** @type {any} */ a) => a.contentHash === contentHash) ?? null;

/** @type {import('../registry.js').RegisteredCommand[]} */
export const assetCommands = [
  {
    id: 'comic.asset.register',
    label: 'Register Asset',
    params: {
      type: 'object',
      properties: {
        name: { type: 'string', minLength: 1, maxLength: 200 },
        contentHash: { type: 'string', minLength: 1, maxLength: 200, description: 'Hash of the bytes; the identity of the asset.' },
        data: { type: 'string', description: 'The bytes as a data: URL or text payload. Omit when the asset lives outside the bundle.' },
        assetType: { type: 'string', enum: ['source', 'generated', 'edited', 'reference'] },
        mime: { type: 'string', maxLength: 100 },
        provider: { type: 'string', maxLength: 100 },
        model: { type: 'string', maxLength: 200 },
        request: { type: 'string', maxLength: 4000 },
        inputAssetIds: { type: 'array', items: { type: 'string', minLength: 1 }, maxItems: 32 },
        parentAssetIds: { type: 'array', items: { type: 'string', minLength: 1 }, maxItems: 32 },
      },
      required: ['name', 'contentHash'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['assets'] },
    run(ctx, params) {
      const { bundle } = ctx;
      for (const assetId of [...(params.inputAssetIds ?? []), ...(params.parentAssetIds ?? [])]) {
        if (!bundle.project.assets[assetId]) {
          return fail('bad-params', `no asset ${assetId} to name as an input or parent`, 'inputAssetIds');
        }
      }
      const existing = assetByHash(bundle, params.contentHash);
      if (existing) {
        return ok({ assetId: /** @type {any} */ (existing).id, deduplicated: true, contentHash: params.contentHash });
      }
      const assetId = allocate(bundle, 'asset');
      const now = ctx.env.now ? ctx.env.now() : 0;
      const type = params.assetType ?? 'source';
      bundle.project.assets[assetId] = {
        id: assetId,
        name: params.name,
        contentHash: params.contentHash,
        ...(params.data !== undefined ? { data: params.data } : {}),
        assetType: type,
        mime: params.mime ?? null,
        mediaMetadata: { mime: params.mime ?? null },
        provenance: {
          provider: params.provider ?? (type === 'generated' ? 'unknown' : 'local'),
          model: params.model ?? null,
          request: params.request ?? null,
          inputAssetIds: [...(params.inputAssetIds ?? [])],
          createdAt: now,
        },
        parentAssetIds: [...(params.parentAssetIds ?? [])],
        inputAssetIds: [...(params.inputAssetIds ?? [])],
        // A source or reference file is trusted as provided; generated work waits for review.
        approvalStatus: type === 'generated' || type === 'edited' ? 'pending' : 'approved',
        nodeIds: [],
      };
      return ok({ assetId, deduplicated: false, contentHash: params.contentHash, assetType: type });
    },
  },

  {
    id: 'comic.asset.list',
    label: 'List Assets',
    params: {
      type: 'object',
      properties: {
        assetType: { type: 'string', enum: ['source', 'generated', 'edited', 'reference'] },
        approvalStatus: { type: 'string', enum: ['pending', 'approved', 'rejected'] },
      },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      let assets = Object.values(ctx.bundle.project.assets).map((/** @type {any} */ asset) => ({
        assetId: asset.id,
        name: asset.name,
        contentHash: asset.contentHash,
        assetType: asset.assetType ?? 'source',
        approvalStatus: asset.approvalStatus ?? 'approved',
        hasData: asset.data !== undefined,
        provenance: asset.provenance ?? null,
        parentAssetIds: [...(asset.parentAssetIds ?? [])],
        nodeIds: [...(asset.nodeIds ?? [])],
      }));
      if (params.assetType) assets = assets.filter((a) => a.assetType === params.assetType);
      if (params.approvalStatus) assets = assets.filter((a) => a.approvalStatus === params.approvalStatus);
      return ok({ assets, count: assets.length });
    },
  },

  {
    id: 'comic.asset.approve',
    label: 'Approve Asset',
    params: {
      type: 'object',
      properties: {
        assetId: { type: 'string', minLength: 1 },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['assetId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['assets', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const asset = /** @type {any} */ (bundle.project.assets[params.assetId]);
      if (!asset) return fail('not-found', `no asset ${params.assetId}`, 'assetId');
      const now = ctx.env.now ? ctx.env.now() : 0;
      asset.approvalStatus = 'approved';
      asset.approvedAt = now;
      const decisionId = allocate(bundle, 'decision');
      bundle.project.reviews[decisionId] = reviewDecision({
        id: decisionId,
        subject: 'asset',
        subjectId: asset.id,
        decision: 'approved',
        actor: 'user',
        timestamp: now,
        note: params.note ?? null,
      });
      return ok({ assetId: asset.id, approvalStatus: asset.approvalStatus, decisionId });
    },
  },

  {
    id: 'comic.asset.reject',
    label: 'Reject Asset',
    params: {
      type: 'object',
      properties: {
        assetId: { type: 'string', minLength: 1 },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['assetId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['assets', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const asset = /** @type {any} */ (bundle.project.assets[params.assetId]);
      if (!asset) return fail('not-found', `no asset ${params.assetId}`, 'assetId');
      const now = ctx.env.now ? ctx.env.now() : 0;
      asset.approvalStatus = 'rejected';
      const decisionId = allocate(bundle, 'decision');
      bundle.project.reviews[decisionId] = reviewDecision({
        id: decisionId,
        subject: 'asset',
        subjectId: asset.id,
        decision: 'rejected',
        actor: 'user',
        timestamp: now,
        note: params.note ?? null,
      });
      // The bytes stay: a rejected asset may be used later, and deleting it would destroy
      // provenance. Rejection is a decision, not a deletion.
      return ok({ assetId: asset.id, approvalStatus: asset.approvalStatus, decisionId });
    },
  },

  {
    id: 'comic.asset.get',
    label: 'Get Asset',
    params: {
      type: 'object',
      properties: { assetId: { type: 'string', minLength: 1 } },
      required: ['assetId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const asset = /** @type {any} */ (ctx.bundle.project.assets[params.assetId]);
      if (!asset) return fail('not-found', `no asset ${params.assetId}`, 'assetId');
      return ok({ asset });
    },
  },
];
