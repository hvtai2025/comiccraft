/**
 * The one route through which every comic operation runs.
 *
 * The dispatcher owns three guarantees, and each exists because its absence is silent:
 *
 *  1. **Parameters are checked before the handler.** A malformed call cannot half-apply an
 *     edit — it never reaches the handler at all.
 *  2. **Undo is a transaction.** The dispatcher snapshots, runs, and commits; on a thrown
 *     error it rolls the snapshot back, so a bug cannot leave a half-applied edit either.
 *  3. **The status is checked.** An async handler that reports a failure *after* awaiting
 *     still had its mutation rolled back, because the rollback is driven by the result and
 *     not by an exception.
 *
 * @module shared/commands/dispatch
 */

import { fail, ok } from './registry.js';
import { check } from './validate-params.js';
import { bumpRevision } from '../domain/project.js';

/**
 * What a command is handed.
 *
 * `bundle` is typed rather than `any` on purpose: with `any` here, every callback a command
 * writes loses its contextual type and the type check fills with `implicitly has an 'any'
 * type` — which is how a strict type check stops being read.
 *
 * It is typed non-null because the dispatcher **guarantees** it: a command that declares
 * `enabled` is only run when `enabled` returned true, and the conventional predicate is
 * `(ctx) => Boolean(ctx.bundle)`. Typing it nullable would put a null check in every command
 * to restate a guarantee the caller already made.
 *
 * @typedef {object} DispatchContext
 * @property {import('../domain/project.js').Bundle} bundle
 * @property {import('../domain/history.js').History} history
 * @property {{ onServer: boolean, now?: () => number, roots?: string[] }} env
 * @property {Record<string, any>} [services]
 */

/**
 * @param {import('./registry.js').Registry} registry
 * @param {DispatchContext} ctx
 * @param {string} id
 * @param {unknown} [params]
 * @returns {Promise<import('./registry.js').Result & { command: string, label?: string, changed?: boolean }>}
 */
export async function dispatch(registry, ctx, id, params = {}) {
  const command = registry.get(id);
  if (!command) {
    return { ...fail('not-found', `no such command: ${id}`), command: id };
  }

  if (command.enabled && !command.enabled(ctx)) {
    return { ...fail('disabled', `${command.label} needs an open comic project`), command: id };
  }

  if (command.serverOnly && !ctx.env.onServer) {
    return {
      ...fail('disabled', `${command.label} runs on the server only, and this is the browser`),
      command: id,
    };
  }

  const paramProblem = check(command.params, params ?? {});
  if (paramProblem) {
    return { ...paramProblem, command: id, label: command.label };
  }

  const undo = command.undo;
  /** @type {import('../domain/history.js').Scope} */
  const scope = {
    projectWide: undo?.scope !== 'page',
    tables: undo?.tables ?? [],
  };

  let opened = false;
  if (undo) {
    try {
      ctx.history.begin(command.label, command.id, ctx.bundle, scope);
      opened = true;
    } catch (error) {
      return { ...fail('internal', `could not open an undo step: ${/** @type {Error} */ (error).message}`), command: id };
    }
  }

  /** @type {import('./registry.js').Result} */
  let result;
  try {
    result = await command.run(ctx, params ?? {});
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (opened) ctx.history.rollback(ctx.bundle);
    return { ...fail('internal', `${command.label} failed: ${message}`), command: id, label: command.label };
  }

  if (!result || typeof result !== 'object' || typeof (/** @type {any} */ (result).ok) !== 'boolean') {
    if (opened) ctx.history.rollback(ctx.bundle);
    return { ...fail('internal', `${command.label} returned no result`), command: id, label: command.label };
  }

  let changed = false;
  if (opened) {
    if (result.ok) {
      changed = ctx.history.commit(ctx.bundle);
      // A successful edit advances the revision, so a job built against older state can say
      // so explicitly instead of overwriting newer work. `changed` is false for a no-op, and
      // an undo is not itself a content edit, so neither moves the counter.
      if (changed) bumpRevision(ctx.bundle);
    } else {
      // The handler reported a failure after mutating; put the bundle back.
      ctx.history.rollback(ctx.bundle);
      changed = false;
    }
  }

  return { ...result, command: id, label: command.label, changed };
}

/** @param {object} schema @param {unknown} value @returns {import('./registry.js').Failure | null} */
export { check as validateParams };

/** Re-exported so callers do not need two imports to build a result. */
export { fail, ok };
