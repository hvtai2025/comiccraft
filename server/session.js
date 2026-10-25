/**
 * A server session: the bundle, its undo history, and the services a command may need.
 *
 * **Commands run on the server through the same dispatcher the browser uses.** The only
 * difference is which services are present — filesystem and renderer here, canvas metrics
 * there — which is why `serverOnly` is a property of a command rather than a second
 * implementation of it.
 *
 * @module server/session
 */

import { resolve } from 'node:path';
import { History } from '../shared/domain/history.js';
import { buildRegistry } from '../shared/commands/index.js';
import { createBundle } from '../shared/domain/project.js';
import { dispatch } from '../shared/commands/dispatch.js';
import { createFileStorage } from './storage/fs.js';
import { createRenderer } from './export/renderer.js';
import { createProviders } from './ai/providers.js';
import { createStoryAgent } from './ai/story.js';
import { createJobManager } from './jobs/manager.js';
import { contactSheet, pagePreviews } from './export/contact.js';
import { packageProject, unpackProject } from './export/package.js';

/**
 * @param {{ root?: string, measure?: any, now?: () => number, fonts?: { families: string[] } }} [options]
 */
export function createSession(options = {}) {
  const root = resolve(options.root ?? process.env.COMICCRAFT_ROOT ?? 'comiccraft-projects');
  const history = new History();
  const registry = buildRegistry();

  /** @type {{ bundle: any }} */
  const state = { bundle: createBundle('Untitled Comic') };
  const storage = createFileStorage(root);
  const providers = createProviders();
  const storyAgent = createStoryAgent();
  const now = options.now ?? Date.now;
  const jobs = createJobManager({
    getBundle: () => state.bundle,
    providers,
    storage,
    now,
  });
  const services = {
    storage,
    renderer: createRenderer(options.measure ? { measure: options.measure } : {}),
    providers,
    storyAgent,
    jobs,
    registry,
    contactSheet,
    pagePreviews,
    packageProject,
    unpackProject,
    ...(options.measure ? { measure: options.measure } : {}),
    ...(options.fonts ? { fonts: options.fonts } : {}),
  };

  const ctx = () => ({
    bundle: state.bundle,
    history,
    env: { onServer: true, now },
    services,
  });

  /** One command at a time. The server has one bundle and one undo stack, so two commands
   * interleaving could open two transactions or adopt a project mid-edit. The queue makes the
   * session a single-writer surface without a lock layer. */
  let queue = Promise.resolve();

  return {
    root,
    registry,
    history,
    storage,
    providers,
    storyAgent,
    jobs,
    get bundle() { return state.bundle; },

    /**
     * @param {string} id @param {unknown} [params]
     * @returns {Promise<any>}
     */
    async run(id, params) {
      const task = queue.then(async () => {
        const result = await dispatch(registry, ctx(), id, params);
        // `comic.project.create` and `comic.project.open` return a whole new bundle. Adopting
        // it here — not inside the command — is what keeps the command free of any knowledge
        // of who is holding the session.
        if (result.ok && result.value && typeof result.value === 'object' && 'bundle' in result.value) {
          state.bundle = /** @type {any} */ (result.value).bundle;
          history.undoStack.length = 0;
          history.redoStack.length = 0;
        }
        return result;
      });
      // Keep the queue alive after a failed command; the failure is the caller's result.
      queue = task.then(() => undefined, () => undefined);
      return task;
    },

    /** @param {any} bundle */
    adopt(bundle) {
      if (history.pending) {
        throw new Error('cannot adopt a project while a command transaction is open');
      }
      state.bundle = bundle;
      history.undoStack.length = 0;
      history.redoStack.length = 0;
    },
  };
}
