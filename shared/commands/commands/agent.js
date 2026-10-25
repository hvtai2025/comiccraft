/**
 * `comic.batch.*` and `comic.command.*` — the agent-facing conveniences.
 *
 * A batch is a sequence of ordinary commands through the same dispatcher. It is **atomic on
 * request**: a failure restores the bundle snapshot and drops the history steps the batch
 * created, so an agent can retry a planned edit without half of it landing. Nested batches
 * are refused, because a batch inside a batch has no meaning and doubles the failure modes.
 *
 * @module shared/commands/commands/agent
 */

import { fail, ok } from '../registry.js';
import { dispatch } from '../dispatch.js';

/** Commands that are safe to retry: they either change nothing or are idempotent deletes. */
const RETRY_SAFE = new Set([
  'comic.project.info', 'comic.project.validate', 'comic.project.serialize',
  'comic.page.info', 'comic.page.list', 'comic.panel.list', 'comic.panel.readingOrder',
  'comic.lettering.list', 'comic.reading.validate', 'comic.asset.list', 'comic.asset.get',
  'comic.character.list', 'comic.character.get', 'comic.job.status', 'comic.job.list',
  'comic.provider.list', 'comic.fonts.list', 'comic.command.list', 'comic.command.describe',
  'comic.story.get', 'comic.story.validate', 'comic.storyboard.list',
]);

/**
 * @param {string} id
 * @returns {'safe'|'idempotent'|'state-changing'}
 */
function retryClass(id) {
  if (RETRY_SAFE.has(id)) return 'safe';
  if (id.endsWith('.delete') || id.endsWith('.reject')) return 'idempotent';
  return 'state-changing';
}

/** @param {import('../registry.js').Registry} registry */
/**
 * @param {import('../registry.js').Registry} registry
 * @param {string} id
 */
function describeOne(registry, id) {
  const command = registry.get(id);
  if (!command) return null;
  return {
    id: command.id,
    label: command.label,
    params: command.params,
    serverOnly: command.serverOnly === true,
    undo: command.undo ?? null,
    retry: retryClass(command.id),
  };
}

/** @type {import('../registry.js').RegisteredCommand[]} */
export const agentCommands = [
  {
    id: 'comic.command.list',
    label: 'List Commands',
    params: {
      type: 'object',
      properties: { prefix: { type: 'string', maxLength: 100, description: 'Only ids starting with this prefix, e.g. comic.panel.' } },
      required: [],
      additionalProperties: false,
    },
    async run(ctx, params) {
      const registry = ctx.services?.registry;
      if (!registry) return fail('disabled', 'this host has no command registry');
      const commands = registry.describe()
        .filter((/** @type {any} */ command) => !params.prefix || command.id.startsWith(params.prefix))
        .map((/** @type {any} */ command) => describeOne(registry, command.id))
        .filter((/** @type {any} */ command) => command !== null);
      return ok({ commands, count: commands.length });
    },
  },

  {
    id: 'comic.command.describe',
    label: 'Describe Command',
    params: {
      type: 'object',
      properties: { commandId: { type: 'string', minLength: 1 } },
      required: ['commandId'],
      additionalProperties: false,
    },
    async run(ctx, params) {
      const registry = ctx.services?.registry;
      if (!registry) return fail('disabled', 'this host has no command registry');
      const described = describeOne(registry, params.commandId);
      if (!described) return fail('not-found', `no command ${params.commandId}`, 'commandId');
      return ok({ command: described });
    },
  },

  {
    id: 'comic.batch.run',
    label: 'Run Command Batch',
    params: {
      type: 'object',
      properties: {
        commands: {
          type: 'array',
          minItems: 1,
          maxItems: 200,
          description: 'An ordered list of { command, params } objects. Commands run through the same dispatcher as a click or a curl.',
          items: {
            type: 'object',
            properties: {
              command: { type: 'string', minLength: 1, maxLength: 120 },
              params: { type: 'object' },
            },
            required: ['command'],
            additionalProperties: false,
          },
        },
        atomic: { type: 'boolean', description: 'Restore the whole bundle if any command fails. Default true.' },
        stopOnError: { type: 'boolean', description: 'Stop at the first failure. Default true.' },
      },
      required: ['commands'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    async run(ctx, params) {
      const registry = ctx.services?.registry;
      if (!registry) return fail('disabled', 'this host has no command registry');
      for (const entry of params.commands) {
        if (entry.command === 'comic.batch.run') {
          return fail('bad-params', 'a batch cannot contain another batch', 'commands');
        }
      }

      const atomic = params.atomic !== false;
      const stopOnError = params.stopOnError !== false;
      const snapshot = atomic
        ? { project: structuredClone(ctx.bundle.project), document: structuredClone(ctx.bundle.document) }
        : null;
      const undoSnapshot = [...ctx.history.undoStack];
      const redoSnapshot = [...ctx.history.redoStack];
      const revision = ctx.bundle.project.revision ?? 1;
      const results = [];
      let failed = 0;

      for (const entry of params.commands) {
        const settled = await dispatch(registry, ctx, entry.command, entry.params ?? {});
        results.push({ command: entry.command, ok: settled.ok, kind: settled.ok ? 'ok' : settled.kind, message: settled.ok ? undefined : settled.message });
        if (!settled.ok) {
          failed += 1;
          if (stopOnError) break;
        }
      }

      if (failed > 0 && atomic && snapshot) {
        ctx.bundle.project = snapshot.project;
        // The revision is time, not content: a rolled-back batch must not lower it, or a job
        // built against the newer state would suddenly look applicable again.
        ctx.bundle.project.revision = revision;
        ctx.bundle.document = snapshot.document;
        // Restore the stacks exactly. A batch may contain `comic.edit.undo`, so the stack can
        // be *shorter* than it was; setting a length would pad it with empty slots.
        ctx.history.undoStack.splice(0, ctx.history.undoStack.length, ...undoSnapshot);
        ctx.history.redoStack.splice(0, ctx.history.redoStack.length, ...redoSnapshot);
      }
      const out = {
        results,
        total: results.length,
        failed,
        atomic,
        rolledBack: failed > 0 && atomic,
      };
      if (failed > 0) return fail('conflict', `${failed} command(s) failed in the batch`, 'commands');
      return ok(out);
    },
  },
];
