/**
 * Generation jobs and review decisions.
 *
 * The domain owns the **shape and the rules**, not the network. A provider is a server-side
 * adapter; a job is a record that can be polled, cancelled and applied. Nothing here fetches,
 * reads a key or writes a file, so the same rules run in the browser and in `node --test`.
 *
 * **Reconciliation is mandatory.** A job records the revision it was built against. Applying
 * a result whose target has moved is a conflict the caller must see, never a blind overwrite:
 * while a job runs the user keeps editing, and their newer work wins.
 *
 * @module shared/domain/jobs
 */

import { revisionOf } from './project.js';

/** @typedef {'queued'|'running'|'completed'|'failed'|'awaiting_review'|'cancelled'} JobStatus */
/** @typedef {'text-to-image'|'reference-to-image'|'image-to-image'|'inpaint'|'variation'} JobOperation */

export const JOB_STATUSES = /** @type {const} */ ([
  'queued', 'running', 'completed', 'failed', 'awaiting_review', 'cancelled',
]);

export const JOB_OPERATIONS = /** @type {const} */ ([
  'text-to-image', 'reference-to-image', 'image-to-image', 'inpaint', 'variation',
]);

/** Statuses from which no further provider work happens. */
export const TERMINAL_STATUSES = /** @type {const} */ (['completed', 'failed', 'awaiting_review', 'cancelled']);

/**
 * @typedef {object} GenerationJob
 * @property {string} id
 * @property {JobOperation} operation
 * @property {string|null} targetPanelId
 * @property {string[]} inputAssetIds
 * @property {string} provider
 * @property {string} model
 * @property {Record<string, unknown>} request
 * @property {JobStatus} status
 * @property {{ done: number, total: number }|null} progress
 * @property {{ code: string, message: string }|null} error
 * @property {string[]} resultAssetIds
 * @property {number} targetRevision
 * @property {number} created
 * @property {number} updated
 * @property {number} [appliedAt]
 * @property {number} [appliedRevision]
 * @property {number} [rejectedAt]
 * @property {{ model: string, mime: string, request: Record<string, unknown> }} [discardedResult]
 * @property {{ provider: string, model: string, request: Record<string, unknown>, createdAt: number }} provenance
 */

/**
 * @typedef {object} ReviewDecision
 * @property {string} id
 * @property {'asset'|'story'|'character'|'batch'|'publication'} subject
 * @property {string} subjectId
 * @property {'approved'|'rejected'|'superseded'} decision
 * @property {string} actor
 * @property {number} timestamp
 * @property {string|null} note
 * @property {string|null} supersededAssetId
 */

/**
 * @param {object} args
 * @param {string} args.id
 * @param {JobOperation} args.operation
 * @param {string} args.provider
 * @param {string} args.model
 * @param {Record<string, unknown>} args.request
 * @param {number} args.targetRevision
 * @param {number} [args.now]
 * @param {string|null} [args.targetPanelId]
 * @param {string[]} [args.inputAssetIds]
 * @returns {GenerationJob}
 */
export function createJob({
  id, operation, provider, model, request, targetRevision, now = 0,
  targetPanelId = null, inputAssetIds = [],
}) {
  return {
    id,
    operation,
    targetPanelId,
    inputAssetIds: [...inputAssetIds],
    provider,
    model,
    request: { ...request },
    status: 'queued',
    progress: null,
    error: null,
    resultAssetIds: [],
    targetRevision,
    created: now,
    updated: now,
    provenance: { provider, model, request: { ...request }, createdAt: now },
  };
}

/**
 * @param {object} args
 * @param {string} args.id
 * @param {'asset'|'story'|'character'|'batch'|'publication'} args.subject
 * @param {string} args.subjectId
 * @param {'approved'|'rejected'|'superseded'} args.decision
 * @param {string} [args.actor]
 * @param {number} [args.timestamp]
 * @param {string|null} [args.note]
 * @param {string|null} [args.supersededAssetId]
 * @returns {ReviewDecision}
 */
export function reviewDecision({
  id, subject, subjectId, decision, actor = 'user', timestamp = 0, note = null, supersededAssetId = null,
}) {
  return { id, subject, subjectId, decision, actor, timestamp, note, supersededAssetId };
}

/**
 * @param {string} status
 * @returns {boolean}
 */
export const isTerminal = (status) => TERMINAL_STATUSES.includes(/** @type {any} */ (status));

/**
 * Whether a job may be applied to the project as it stands. This is the rule the command
 * uses; putting it here means a test can call it without a registry.
 *
 * @param {GenerationJob} job
 * @param {import('./project.js').ComicProject} project
 * @returns {{ ok: true, revision: number } | { ok: false, reason: string, currentRevision: number }}
 */
export function canApply(job, project) {
  const currentRevision = project.revision ?? 1;
  if (job.status !== 'awaiting_review' && job.status !== 'completed') {
    return { ok: false, reason: `job ${job.id} is ${job.status}, not awaiting review`, currentRevision };
  }
  if (job.targetRevision !== currentRevision) {
    return {
      ok: false,
      reason: `job ${job.id} was built against revision ${job.targetRevision}, but the project is at ${currentRevision}; the target changed while the job ran`,
      currentRevision,
    };
  }
  if (job.targetPanelId && !project.panels[job.targetPanelId]) {
    return { ok: false, reason: `job ${job.id} targets panel ${job.targetPanelId}, which no longer exists`, currentRevision };
  }
  if (job.resultAssetIds.length === 0) {
    return { ok: false, reason: `job ${job.id} has no result asset to apply`, currentRevision };
  }
  for (const assetId of job.resultAssetIds) {
    if (!project.assets[assetId]) {
      return { ok: false, reason: `job ${job.id} names asset ${assetId}, which is not in the project`, currentRevision };
    }
  }
  return { ok: true, revision: currentRevision };
}

/**
 * @param {GenerationJob} job
 * @returns {GenerationJob} a shallow public copy without provider credentials (there are none)
 */
export function jobView(job) {
  return {
    ...job,
    request: { ...job.request },
    inputAssetIds: [...job.inputAssetIds],
    resultAssetIds: [...job.resultAssetIds],
    provenance: { ...job.provenance, request: { ...job.provenance.request } },
  };
}

/** @param {any} bundle @returns {number} */
export const revisionOfBundle = (bundle) => revisionOf(bundle);
