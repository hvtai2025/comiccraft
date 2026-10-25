/**
 * Undo/redo as **transactions**, not as command inverses.
 *
 * A command snapshots the parts of the bundle it is about to change, mutates freely, and
 * commits. Undo restores the snapshot. That is deliberately dumb: writing an inverse for
 * every command is a place to make a mistake, and a wrong inverse corrupts a document
 * silently, whereas a snapshot cannot be wrong.
 *
 * The cost is memory — a snapshot of one page, not of the whole project, unless a command
 * declares it is project-wide.
 *
 * @module shared/domain/history
 */

/**
 * How much to snapshot.
 *
 * `page` takes the named project tables **and the whole visual document**. The first version
 * of this type let a command name the node ids it would touch — which cannot work, because a
 * command that *creates* a node does not know its id until after it has run. `comic.artwork.place`
 * declared `nodes: []`, snapshotted no nodes, and its undo left the image object behind. A test
 * caught it; this shape is what replaced it.
 *
 * The cost is that a one-word caption edit copies the document. A document is JSON and a page
 * is small, so this is the right trade for now; narrowing it would need the command to declare
 * the page it touches *before* running, which is a change to the command contract rather than
 * to this file.
 *
 * @typedef {{ tables: string[], projectWide: boolean }} Scope
 */

/**
 * @param {unknown} value
 * @returns {unknown} a deep copy, with stable key order
 */
export const clone = (value) => structuredClone(value);

/**
 * @typedef {object} Transaction
 * @property {string} label
 * @property {string} commandId
 * @property {unknown} before
 * @property {unknown} after
 * @property {Scope} scope
 */

export class History {
  /** @param {number} [limit] how many steps to keep */
  constructor(limit = 200) {
    /** @type {Transaction[]} */
    this.undoStack = [];
    /** @type {Transaction[]} */
    this.redoStack = [];
    this.limit = limit;
    /** @type {null | { label: string, commandId: string, snapshot: unknown, scope: Scope }} */
    this.pending = null;
  }

  /**
   * Snapshot before mutating. `scope` says what to snapshot: naming one page's tables and
   * nodes keeps a text edit from copying the whole book.
   * @param {string} label
   * @param {string} commandId
   * @param {any} bundle
   * @param {Scope} scope
   */
  begin(label, commandId, bundle, scope) {
    if (this.pending) throw new Error(`a transaction is already open: ${this.pending.label}`);
    this.pending = { label, commandId, snapshot: takeSnapshot(bundle, scope), scope };
  }

  /**
   * Close the open transaction. A transaction that changed nothing is **not recorded** —
   * an undo step that does nothing is worse than no step, because it makes the user press
   * undo twice to see a change.
   * @param {any} bundle
   * @returns {boolean} whether a step was recorded
   */
  commit(bundle) {
    const pending = this.pending;
    if (!pending) throw new Error('no transaction is open');
    this.pending = null;
    const after = takeSnapshot(bundle, pending.scope);
    if (JSON.stringify(after) === JSON.stringify(pending.snapshot)) return false;
    this.undoStack.push({
      label: pending.label,
      commandId: pending.commandId,
      before: pending.snapshot,
      after,
      scope: pending.scope,
    });
    if (this.undoStack.length > this.limit) this.undoStack.shift();
    this.redoStack.length = 0;
    return true;
  }

  /**
   * Restore the open transaction's snapshot and abandon it. This is what makes a thrown
   * handler or a returned failure leave no half-applied edit: the dispatcher calls it with
   * the bundle it was mutating.
   * @param {any} [bundle]
   */
  rollback(bundle) {
    const pending = this.pending;
    this.pending = null;
    if (!pending || !bundle) return;
    applySnapshot(bundle, pending.snapshot, pending.scope);
  }

  /** @returns {boolean} */
  canUndo() { return this.undoStack.length > 0; }

  /** @returns {boolean} */
  canRedo() { return this.redoStack.length > 0; }

  /**
   * @param {any} bundle
   * @returns {string | null} the label of the step undone
   */
  undo(bundle) {
    const step = this.undoStack.pop();
    if (!step) return null;
    applySnapshot(bundle, step.before, step.scope);
    this.redoStack.push(step);
    return step.label;
  }

  /**
   * @param {any} bundle
   * @returns {string | null} the label of the step redone
   */
  redo(bundle) {
    const step = this.redoStack.pop();
    if (!step) return null;
    applySnapshot(bundle, step.after, step.scope);
    this.undoStack.push(step);
    return step.label;
  }

  /** @returns {{ undo: number, redo: number, labels: string[] }} */
  depth() {
    return {
      undo: this.undoStack.length,
      redo: this.redoStack.length,
      labels: this.undoStack.map((s) => s.label),
    };
  }
}

/**
 * @param {any} bundle
 * @param {Scope} scope
 * @returns {unknown}
 */
function takeSnapshot(bundle, scope) {
  if (scope.projectWide) {
    // The revision is **time**, not content: undo restores what the user sees, never the
    // monotonic counter a generation job was reconciled against.
    const project = /** @type {any} */ (clone(bundle.project));
    delete project.revision;
    // Jobs are created by a deliberately non-undoable command, so they are not part of an
    // undo snapshot. Proposals and review decisions *are* editing content: undoing an accept
    // must put the proposal back to `proposed`, so they stay in the snapshot.
    delete project.jobs;
    return { project, document: clone(bundle.document) };
  }
  /** @type {Record<string, unknown>} */
  const project = { counters: clone(bundle.project.counters), id: bundle.project.id };
  for (const table of scope.tables) project[table] = clone(bundle.project[table]);
  return { project, document: clone(bundle.document) };
}

/**
 * @param {any} bundle
 * @param {any} snapshot
 * @param {Scope} scope
 */
function applySnapshot(bundle, snapshot, scope) {
  const isGenerated = (/** @type {any} */ asset) =>
    asset && (asset.assetType === 'generated' || asset.assetType === 'edited');

  if (scope.projectWide) {
    const revision = bundle.project.revision;
    const current = /** @type {any} */ (bundle.project);
    const jobs = current.jobs ?? {};
    const currentAssets = current.assets ?? {};
    /** @type {Record<string, number>} */
    const counters = { ...(current.counters ?? {}) };
    bundle.project = clone(snapshot.project);
    for (const [prefix, value] of Object.entries(/** @type {any} */ (snapshot.project.counters ?? {}))) {
      counters[prefix] = Math.max(Number(counters[prefix] ?? 0), Number(value ?? 0));
    }
    bundle.project.counters = counters;
    if (revision !== undefined) bundle.project.revision = revision;
    // A job is not undoable content: keep every job, including one submitted after the
    // snapshot. Its generated asset is provenance too, so keep assets that the snapshot
    // predates when they were produced by a job/derivative. A *registered source* asset is
    // ordinary content and is undone with its command.
    bundle.project.jobs = { ...(bundle.project.jobs ?? {}), ...jobs };
    const restoredAssets = /** @type {any} */ (bundle.project.assets ?? {});
    for (const [id, asset] of Object.entries(currentAssets)) {
      if (restoredAssets[id] === undefined && isGenerated(asset)) restoredAssets[id] = asset;
    }
    bundle.project.assets = restoredAssets;
    bundle.document = clone(snapshot.document);
    pruneAssetNodes(bundle);
    return;
  }

  /** @type {Set<string>} */
  const versioned = new Set(['assets', 'jobs', 'reviews', 'proposals']);
  const current = /** @type {any} */ (bundle.project);
  for (const table of scope.tables) {
    if (!versioned.has(table)) {
      bundle.project[table] = clone(snapshot.project[table]);
      continue;
    }
    const snapshotTable = /** @type {Record<string, any>} */ (snapshot.project[table] ?? {});
    /** @type {Record<string, any>} */
    const merged = { ...(current[table] ?? {}), .../** @type {Record<string, any>} */ (clone(snapshotTable)) };
    if (table === 'assets') {
      // Keep generated derivatives created after the snapshot; a registered source asset is
      // restored/removed by the undo like any other content.
      for (const [id, asset] of Object.entries(current.assets ?? {})) {
        if (merged[id] === undefined && isGenerated(asset)) merged[id] = asset;
      }
    }
    bundle.project[table] = merged;
  }
  // Counters are monotonic. An id allocated by a non-undoable job (or any later command) must
  // never be reissued because an earlier page-scoped step was undone.
  const counters = /** @type {any} */ (bundle.project.counters ?? {});
  const before = /** @type {any} */ (snapshot.project.counters ?? {});
  for (const [prefix, value] of Object.entries(before)) {
    counters[prefix] = Math.max(Number(counters[prefix] ?? 0), Number(value ?? 0));
  }
  bundle.project.counters = counters;
  // The document is restored whole for a page-scoped step, for the reason on `Scope` above.
  bundle.document = clone(snapshot.document);
  pruneAssetNodes(bundle);
}

/**
 * An asset may list nodes that the restored document no longer has. The record stays (it is
 * provenance), but the dangling link is removed so validation tells the truth.
 * @param {any} bundle
 */
function pruneAssetNodes(bundle) {
  const liveNodes = new Set(Object.keys(bundle.document.nodes));
  for (const asset of Object.values(bundle.project.assets ?? {})) {
    const a = /** @type {any} */ (asset);
    if (Array.isArray(a.nodeIds)) a.nodeIds = a.nodeIds.filter((/** @type {any} */ id) => liveNodes.has(String(id)));
  }
}
