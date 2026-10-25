/**
 * The command registry and dispatcher.
 *
 * **Everything is a command.** The browser UI, the HTTP endpoint and MCP all reach the same
 * dispatcher, so a click, a `curl` and an agent run the same code and cannot disagree. This
 * is the property the whole architecture is built around (design §3, Layer E).
 *
 * A command declares:
 *  - `id`          stable, `comic.<group>.<verb>`; agents address it by id
 *  - `label`       user-visible; scanned for vendor names
 *  - `params`      a JSON Schema — this *is* the documentation MCP publishes
 *  - `enabled`     honest: false without an open project
 *  - `serverOnly`  true when the handler must run on the server (filesystem, provider)
 *  - `scope`       what undo must snapshot
 *  - `run`         the handler
 *
 * @module shared/commands/registry
 */

/**
 * @typedef {'ok'|'bad-params'|'not-found'|'conflict'|'disabled'|'internal'} FailureKind
 */

/**
 * A failure is a **returned value**, not a throw. A command that throws is a bug; a command
 * that returns a failure is a normal outcome the caller must handle.
 * @typedef {{ ok: false, kind: FailureKind, message: string, param?: string }} Failure
 * @typedef {{ ok: true, value: unknown }} Success
 * @typedef {Success | Failure} Result
 */

/** @param {unknown} value @returns {Success} */
export const ok = (value) => ({ ok: true, value });

/** @param {FailureKind} kind @param {string} message @param {string} [param] @returns {Failure} */
export const fail = (kind, message, param) => ({ ok: false, kind, message, ...(param ? { param } : {}) });

/**
 * @typedef {object} CommandSpec
 * @property {string} id
 * @property {string} label
 * @property {object} params
 * @property {(ctx: any) => boolean} [enabled]
 * @property {boolean} [serverOnly]
 * @property {{ scope: 'page'|'project', tables?: string[], nodes?: number[], artboards?: number[] }} [undo]
 */

/**
 * @typedef {CommandSpec & {
 *   run: (ctx: import('./dispatch.js').DispatchContext, params: any) => Result | Promise<Result>
 * }} RegisteredCommand
 */

/**
 * Command ids are `comic.<group>.<verb>`. Groups are lowercase single words; the verb is
 * lowercase-initial camelCase, because `comic.page.setTemplate` reads better than
 * `comic.page.set_template` and MCP publishes the id verbatim to agents.
 */
const COMMAND_ID = /^comic\.[a-z][a-z0-9]*(\.[a-z][a-zA-Z0-9]*)+$/;

export class Registry {
  constructor() {
    /** @type {Map<string, RegisteredCommand>} */
    this.commands = new Map();
  }

  /**
   * @param {RegisteredCommand} command
   * @returns {Registry}
   */
  register(command) {
    if (this.commands.has(command.id)) {
      throw new Error(`duplicate command id: ${command.id}`);
    }
    if (!COMMAND_ID.test(command.id)) {
      throw new Error(`malformed command id: ${command.id} — expected comic.<group>.<verb>`);
    }
    if (typeof command.label !== 'string' || command.label.trim() === '') {
      throw new Error(`${command.id} has no label`);
    }
    if (typeof command.run !== 'function') throw new Error(`${command.id} has no run`);
    if (!command.params || typeof command.params !== 'object') {
      throw new Error(`${command.id} has no params schema`);
    }
    this.commands.set(command.id, command);
    return this;
  }

  /**
   * @param {RegisteredCommand[]} commands
   * @returns {Registry}
   */
  registerAll(commands) {
    for (const c of commands) this.register(c);
    return this;
  }

  /** @returns {string[]} */
  ids() {
    return [...this.commands.keys()].sort();
  }

  /**
   * The machine-readable catalogue MCP publishes and the UI builds its menus from.
   * @returns {{ id: string, label: string, params: object, serverOnly: boolean }[]}
   */
  describe() {
    return this.ids().map((id) => {
      const c = /** @type {RegisteredCommand} */ (this.commands.get(id));
      return { id: c.id, label: c.label, params: c.params, serverOnly: c.serverOnly === true };
    });
  }

  /** @param {string} id @returns {RegisteredCommand | undefined} */
  get(id) { return this.commands.get(id); }
}
