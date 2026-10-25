/**
 * `comic.edit.*` — undo and redo, exposed as commands so an agent can drive history the way
 * a user does. Discrete undo steps are a user-facing behaviour, not an implementation detail.
 * @module shared/commands/commands/edit
 */

import { fail, ok } from '../registry.js';

/** @type {import('../registry.js').RegisteredCommand[]} */
export const editCommands = [
  {
    id: 'comic.edit.undo',
    label: 'Undo',
    // Undo is not itself undoable: snapshotting the history stack to make it so would let a
    // user undo an undo forever.
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle) && ctx.history.canUndo(),
    run(ctx) {
      const label = ctx.history.undo(ctx.bundle);
      if (label === null) return fail('disabled', 'there is nothing to undo');
      const depth = ctx.history.depth();
      return ok({ undone: label, undoDepth: depth.undo, redoDepth: depth.redo });
    },
  },

  {
    id: 'comic.edit.redo',
    label: 'Redo',
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle) && ctx.history.canRedo(),
    run(ctx) {
      const label = ctx.history.redo(ctx.bundle);
      if (label === null) return fail('disabled', 'there is nothing to redo');
      const depth = ctx.history.depth();
      return ok({ redone: label, undoDepth: depth.undo, redoDepth: depth.redo });
    },
  },

  {
    id: 'comic.edit.history',
    label: 'History',
    params: { type: 'object', properties: {}, additionalProperties: false },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx) {
      const depth = ctx.history.depth();
      return ok({ canUndo: ctx.history.canUndo(), canRedo: ctx.history.canRedo(), ...depth });
    },
  },
];
