/**
 * `comic.job.*` — submit, poll, cancel, apply and reject generation work.
 *
 * Submitting never blocks: it returns a job id before the provider runs. Status, cancellation
 * and application are separate commands, and **applying is the only one that may change the
 * document**. A result is an asset awaiting review until then.
 *
 * `comic.job.apply` reconciles `targetRevision` against the project and refuses a target that
 * changed while the job ran. The refusal is a value with the current revision in it, so an
 * agent can decide what to do instead of retrying blindly.
 *
 * @module shared/commands/commands/generation
 */

import { fail, ok } from '../registry.js';
import { allocate, unlinkAssetNodes } from '../../domain/project.js';
import { addNode, findArtboard, getNode, removeNode } from '../../domain/document.js';
import { canApply, jobView, reviewDecision } from '../../domain/jobs.js';

/** @param {any} ctx @returns {any} */
function managerOf(ctx) {
  const manager = /** @type {any} */ (ctx.services ?? {}).jobs;
  if (!manager) throw new Error('no job manager is configured');
  return manager;
}

/** @param {any} ctx @returns {any[]} */
function providersOf(ctx) {
  const providers = /** @type {any} */ (ctx.services ?? {}).providers;
  return providers && typeof providers.describe === 'function' ? providers.describe() : [];
}

/** @param {any} bundle @param {string} panelId @returns {any | null} */
function pageOfPanel(bundle, panelId) {
  for (const page of Object.values(bundle.project.pages)) {
    if (/** @type {any} */ (page).panelIds.includes(panelId)) return page;
  }
  return null;
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const generationCommands = [
  {
    id: 'comic.provider.list',
    label: 'List Providers',
    params: { type: 'object', properties: {}, additionalProperties: false },
    run(ctx) {
      return ok({
        providers: providersOf(ctx),
        defaultProvider: ctx.services?.providers?.defaultId?.() ?? null,
        configured: providersOf(ctx).filter((p) => p.configured).map((p) => p.id),
      });
    },
  },

  {
    id: 'comic.job.submit',
    label: 'Submit Generation Job',
    serverOnly: true,
    params: {
      type: 'object',
      properties: {
        operation: { type: 'string', enum: ['text-to-image', 'reference-to-image', 'image-to-image', 'inpaint', 'variation'] },
        prompt: { type: 'string', minLength: 1, maxLength: 4000 },
        panelId: { type: 'string', minLength: 1, description: 'The panel the result is for. The job never writes to it.' },
        inputAssetIds: { type: 'array', items: { type: 'string', minLength: 1 }, maxItems: 32 },
        provider: { type: 'string', minLength: 1, maxLength: 100 },
        model: { type: 'string', minLength: 1, maxLength: 200 },
        parameters: { type: 'object', description: 'Provider parameters, recorded verbatim in provenance.' },
      },
      required: ['operation', 'prompt'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    // Submitting creates only a job record and starts no undoable document change. A result
    // is applied later, as its own undoable step.
    run(ctx, params) {
      const result = managerOf(ctx).submit({
        operation: params.operation,
        prompt: params.prompt,
        panelId: params.panelId ?? null,
        inputAssetIds: params.inputAssetIds ?? [],
        provider: params.provider,
        model: params.model,
        parameters: params.parameters ?? {},
      });
      if (!result.ok) return fail('bad-params', result.message);
      return ok({ jobId: result.job.id, status: result.job.status, targetRevision: result.job.targetRevision });
    },
  },

  {
    id: 'comic.job.status',
    label: 'Generation Job Status',
    params: {
      type: 'object',
      properties: { jobId: { type: 'string', minLength: 1 } },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      // Status is read from the bundle, not from the manager, so it works in the browser and
      // after a reopen: a job record is durable project data.
      const jobs = ctx.bundle.project.jobs ?? {};
      if (params.jobId) {
        const job = jobs[params.jobId];
        if (!job) return fail('not-found', `no job ${params.jobId}`, 'jobId');
        return ok({ job: jobView(job) });
      }
      return ok({ jobs: Object.values(jobs).map((job) => jobView(job)) });
    },
  },

  {
    id: 'comic.job.list',
    label: 'List Generation Jobs',
    params: {
      type: 'object',
      properties: { status: { type: 'string', enum: ['queued', 'running', 'completed', 'failed', 'awaiting_review', 'cancelled'] } },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      let jobs = Object.values(ctx.bundle.project.jobs ?? {}).map((/** @type {any} */ job) => jobView(job));
      if (params.status) jobs = jobs.filter((j) => j.status === params.status);
      return ok({ jobs, count: jobs.length });
    },
  },

  {
    id: 'comic.job.cancel',
    label: 'Cancel Generation Job',
    serverOnly: true,
    params: {
      type: 'object',
      properties: { jobId: { type: 'string', minLength: 1 } },
      required: ['jobId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run: async (ctx, params) => {
      const result = await managerOf(ctx).cancel(params.jobId);
      if (!result.ok) {
        return fail(result.message.startsWith('no job') ? 'not-found' : 'conflict', result.message, 'jobId');
      }
      return ok({ jobId: params.jobId, status: result.job.status });
    },
  },

  {
    id: 'comic.job.apply',
    label: 'Apply Generation Result',
    params: {
      type: 'object',
      properties: {
        jobId: { type: 'string', minLength: 1 },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['jobId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['panels', 'assets', 'jobs', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const job = bundle.project.jobs?.[params.jobId];
      if (!job) return fail('not-found', `no job ${params.jobId}`, 'jobId');
      if (job.appliedAt !== undefined && job.appliedAt !== null) {
        return fail('conflict', `job ${job.id} was already applied at revision ${job.appliedRevision}`, 'jobId');
      }
      const allowed = canApply(job, bundle.project);
      if (!allowed.ok) {
        return fail('conflict', allowed.reason, 'jobId');
      }
      if (!job.targetPanelId) {
        return fail('conflict', `job ${job.id} has no target panel to apply to`, 'jobId');
      }
      const panel = bundle.project.panels[job.targetPanelId];
      const frameNode = panel ? getNode(bundle.document, panel.frameNodeId) : null;
      if (!panel || !frameNode) {
        return fail('not-found', `job ${job.id} targets panel ${job.targetPanelId}, which does not exist`, 'jobId');
      }
      if (panel.locked) {
        return fail('conflict', `panel ${panel.id} is locked; unlock it before applying generated artwork`, 'jobId');
      }
      const page = pageOfPanel(bundle, panel.id);
      if (!page) return fail('conflict', `panel ${panel.id} is not on any page`, 'jobId');

      const asset = bundle.project.assets[job.resultAssetIds[0]];
      if (!asset) return fail('conflict', `job ${job.id} names an asset that is not in the project`, 'jobId');

      const supersededNode = panel.artworkRefs[0] ? getNode(bundle.document, Number(panel.artworkRefs[0])) : null;
      const supersededAssetId = supersededNode && 'assetId' in supersededNode ? supersededNode.assetId : null;
      const oldNodeIds = [...panel.artworkRefs];

      const nodeId = addNode(bundle.document, /** @type {any} */ (page).artboardId, {
        kind: 'image',
        name: asset.name,
        frame: { ...frameNode.frame },
        assetId: asset.id,
        fit: 'cover',
        clipToFrame: true,
      });
      for (const ref of oldNodeIds) removeNode(bundle.document, Number(ref));
      unlinkAssetNodes(bundle.project, oldNodeIds);
      panel.artworkRefs = [nodeId];
      const a = /** @type {any} */ (asset);
      a.nodeIds = [...(a.nodeIds ?? []), nodeId];
      a.approvalStatus = 'approved';
      a.approvedAt = ctx.env.now ? ctx.env.now() : 0;

      const now = ctx.env.now ? ctx.env.now() : 0;
      job.appliedAt = now;
      job.appliedRevision = bundle.project.revision ?? 1;
      job.updated = now;

      const decisionId = allocate(bundle, 'decision');
      bundle.project.reviews[decisionId] = reviewDecision({
        id: decisionId,
        subject: 'asset',
        subjectId: asset.id,
        decision: 'approved',
        actor: 'user',
        timestamp: now,
        note: params.note ?? null,
        supersededAssetId,
      });

      return ok({
        jobId: job.id,
        panelId: panel.id,
        artworkNodeId: nodeId,
        assetId: asset.id,
        supersededAssetId,
        decisionId,
      });
    },
  },

  {
    id: 'comic.job.reject',
    label: 'Reject Generation Result',
    params: {
      type: 'object',
      properties: {
        jobId: { type: 'string', minLength: 1 },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['jobId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'page', tables: ['assets', 'jobs', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const job = bundle.project.jobs?.[params.jobId];
      if (!job) return fail('not-found', `no job ${params.jobId}`, 'jobId');
      if (job.appliedAt !== undefined && job.appliedAt !== null) {
        return fail('conflict', `job ${job.id} was already applied; apply another revision instead of rejecting it`, 'jobId');
      }
      if (job.rejectedAt !== undefined && job.rejectedAt !== null) {
        return fail('conflict', `job ${job.id} was already rejected`, 'jobId');
      }
      const now = ctx.env.now ? ctx.env.now() : 0;
      job.rejectedAt = now;
      job.updated = now;
      for (const assetId of job.resultAssetIds) {
        const asset = /** @type {any} */ (bundle.project.assets[assetId]);
        if (asset) asset.approvalStatus = 'rejected';
      }
      const decisionId = allocate(bundle, 'decision');
      bundle.project.reviews[decisionId] = reviewDecision({
        id: decisionId,
        subject: 'asset',
        subjectId: job.resultAssetIds[0] ?? job.id,
        decision: 'rejected',
        actor: 'user',
        timestamp: now,
        note: params.note ?? null,
      });
      return ok({
        jobId: job.id,
        status: job.status,
        rejectedAssetIds: [...job.resultAssetIds],
        decisionId,
      });
    },
  },
];
