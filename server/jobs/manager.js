/**
 * The in-process job manager.
 *
 * Jobs are **pollable**, because the transport accepts but ignores progress notifications.
 * `submit` returns a job id before any provider work starts; the run happens on the next
 * turn of the event loop. A completed result is an **asset awaiting review** — it never
 * touches a panel, so a failed, cancelled or superseded job cannot corrupt the comic.
 *
 * Job metadata is written to `jobs/<id>.json` when storage is configured, so an interrupted
 * session can say what was in flight. That write is metadata only; the authoritative project
 * is saved through the ordinary transactional save.
 *
 * @module server/jobs/manager
 */

import { createHash } from 'node:crypto';
import { allocate } from '../../shared/domain/project.js';
import { createJob, jobView, isTerminal } from '../../shared/domain/jobs.js';

/**
 * @param {object} options
 * @param {() => any} options.getBundle
 * @param {ReturnType<import('../ai/providers.js').createProviders>} options.providers
 * @param {any} [options.storage]
 * @param {() => number} [options.now]
 * @param {(ms: number, fn: () => void) => any} [options.schedule]
 */
export function createJobManager({ getBundle, providers, storage, now = Date.now, schedule }) {
  /** @type {Map<string, AbortController>} */
  const controllers = new Map();
  /** @type {Map<string, any>} */
  const bundles = new Map();
  const enqueue = schedule ?? ((ms, fn) => setTimeout(fn, ms));

  /** @param {any} bundle @param {any} job */
  async function persist(bundle, job) {
    if (!storage || typeof storage.write !== 'function') return;
    try {
      await storage.write(`jobs/${job.id}.json`, `${JSON.stringify(job, null, 2)}\n`);
    } catch {
      // Job metadata is a convenience for reporting an interrupted session. It is not the
      // authoritative project, so a storage failure must not turn a successful job into a
      // failed one — the in-memory record is still pollable.
      void bundle;
    }
  }

  /**
   * @param {object} args
   * @param {import('../../shared/domain/jobs.js').JobOperation} args.operation
   * @param {string} args.prompt
   * @param {string|null} [args.panelId]
   * @param {string[]} [args.inputAssetIds]
   * @param {string} [args.provider]
   * @param {string} [args.model]
   * @param {Record<string, unknown>} [args.parameters]
   * @returns {{ ok: true, job: any } | { ok: false, message: string }}
   */
  function submit({ operation, prompt, panelId = null, inputAssetIds = [], provider, model, parameters = {} }) {
    const bundle = getBundle();
    if (!bundle) return { ok: false, message: 'no open comic project' };
    const providerId = provider ?? providers.defaultId();
    const resolved = providers.resolve(providerId, operation);
    if (!resolved.ok) return { ok: false, message: resolved.message };
    if (panelId && !bundle.project.panels[panelId]) {
      return { ok: false, message: `no panel ${panelId}` };
    }
    for (const assetId of inputAssetIds) {
      if (!bundle.project.assets[assetId]) return { ok: false, message: `no input asset ${assetId}` };
    }

    const jobId = allocate(bundle, 'job');
    const targetRevision = bundle.project.revision ?? 1;
    const job = createJob({
      id: jobId,
      operation,
      provider: providerId,
      model: model ?? resolved.model,
      request: { prompt, parameters: { ...parameters }, inputAssetIds: [...inputAssetIds] },
      targetRevision,
      now: now(),
      targetPanelId: panelId,
      inputAssetIds,
    });
    bundle.project.jobs[jobId] = job;
    bundles.set(jobId, bundle);
    const controller = new AbortController();
    controllers.set(jobId, controller);
    void persist(bundle, job);

    enqueue(0, () => { void run(jobId, bundle, controller.signal); });
    return { ok: true, job: jobView(job) };
  }

  /**
   * @param {string} jobId
   * @param {any} bundle
   * @param {AbortSignal} signal
   */
  async function run(jobId, bundle, signal) {
    const job = bundle.project.jobs?.[jobId];
    if (!job || isTerminal(job.status)) return;
    if (getBundle() !== bundle) {
      // The user opened another project while this job was queued. Its result is discarded:
      // applying it to a different book is exactly the corruption this design forbids.
      job.status = 'failed';
      job.error = { code: 'project-changed', message: 'the project was replaced before the job ran' };
      job.updated = now();
      return;
    }

    job.status = 'running';
    job.progress = { done: 0, total: 1 };
    job.updated = now();
    await persist(bundle, job);

    const provider = providers.get(job.provider);
    if (!provider) {
      failJob(bundle, job, 'provider-missing', `provider ${job.provider} is not known`);
      return;
    }

    try {
      const result = await provider.generate({
        operation: job.operation,
        prompt: String(job.request.prompt ?? ''),
        panelId: job.targetPanelId,
        inputAssetIds: [...job.inputAssetIds],
        parameters: /** @type {Record<string, unknown>} */ (job.request.parameters ?? {}),
        signal,
      });
      if (job.status === 'cancelled') {
        job.discardedResult = { model: result.model, mime: result.mime, request: result.request };
        job.updated = now();
        await persist(bundle, job);
        return;
      }
      if (signal.aborted) {
        job.status = 'cancelled';
        job.updated = now();
        await persist(bundle, job);
        return;
      }

      const assetId = allocate(bundle, 'asset');
      const contentHash = createHash('sha256').update(result.data).digest('hex');
      const existing = Object.values(bundle.project.assets)
        .find((/** @type {any} */ a) => a.contentHash === contentHash);
      const finalAssetId = existing ? /** @type {any} */ (existing).id : assetId;
      if (!existing) {
        bundle.project.assets[finalAssetId] = {
          id: finalAssetId,
          name: `Generated art for ${job.targetPanelId ?? job.id}`,
          contentHash,
          data: result.data,
          assetType: 'generated',
          mime: result.mime,
          mediaMetadata: { mime: result.mime, width: result.width, height: result.height },
          provenance: {
            provider: job.provider,
            model: result.model,
            request: result.request,
            inputAssetIds: [...job.inputAssetIds],
            createdAt: now(),
          },
          parentAssetIds: [...job.inputAssetIds],
          inputAssetIds: [...job.inputAssetIds],
          approvalStatus: 'pending',
          nodeIds: [],
        };
      }
      job.status = 'awaiting_review';
      job.progress = { done: 1, total: 1 };
      job.resultAssetIds = [finalAssetId];
      job.updated = now();
      await persist(bundle, job);
    } catch (error) {
      if (job.status === 'cancelled') {
        job.updated = now();
        await persist(bundle, job);
        return;
      }
      failJob(bundle, job, 'provider-failed', error instanceof Error ? error.message : String(error));
    }
  }

  /**
   * @param {any} bundle @param {any} job @param {string} code @param {string} message
   */
  function failJob(bundle, job, code, message) {
    job.status = 'failed';
    job.error = { code, message };
    job.updated = now();
    void persist(bundle, job);
  }

  return {
    submit,
    /**
     * @param {string} jobId
     * @returns {Promise<{ ok: true, job: any } | { ok: false, message: string }>}
     */
    async cancel(jobId) {
      const bundle = getBundle();
      const job = bundle?.project.jobs?.[jobId];
      if (!job) return { ok: false, message: `no job ${jobId}` };
      if (isTerminal(job.status)) return { ok: false, message: `job ${jobId} is ${job.status}, so it cannot be cancelled` };
      job.status = 'cancelled';
      job.updated = now();
      controllers.get(jobId)?.abort();
      await persist(bundle, job);
      return { ok: true, job: jobView(job) };
    },

    /** @param {string|undefined} jobId @returns {any} */
    status(jobId) {
      const bundle = getBundle();
      if (!bundle) return { jobs: [] };
      if (jobId) {
        const job = bundle.project.jobs?.[jobId];
        return job ? { job: jobView(job) } : { job: null };
      }
      return { jobs: Object.values(bundle.project.jobs ?? {}).map((/** @type {any} */ job) => jobView(job)) };
    },

    /** Wait for one job to settle. Tests use this; the command layer never blocks on it. */
    /** @param {string} jobId */
    async settled(jobId) {
      const bundle = getBundle();
      for (let i = 0; i < 200; i++) {
        const job = bundle?.project.jobs?.[jobId];
        if (!job || isTerminal(job.status)) return job ?? null;
        await new Promise((resolve) => setTimeout(resolve, 5));
      }
      return bundle?.project.jobs?.[jobId] ?? null;
    },
  };
}
