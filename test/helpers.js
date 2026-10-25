/**
 * Test helpers.
 *
 * `must(result)` exists because `Result.value` is `unknown` — deliberately, since a command's
 * value is whatever that command returns — so a test that reads a field off it has to narrow
 * first. This is that narrowing, in one place, with a failure message that names the command.
 *
 * It returns `any` on purpose. A test is about to assert on the shape anyway, and a typed
 * return here would only push the same assertion into a cast.
 *
 * @module test/helpers
 */

import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createSession } from '../server/session.js';

/**
 * Assert a command succeeded and hand back its value.
 * @param {import('../shared/commands/registry.js').Result & { command?: string }} result
 * @returns {any}
 */
export function must(result) {
  assert.equal(
    result.ok,
    true,
    `${result.command ?? 'the command'} failed: ${result.ok ? '' : `${result.kind}: ${result.message}`}`,
  );
  return /** @type {any} */ (result).value;
}

/**
 * Assert a command failed, and return the failure so its kind and message can be checked.
 * @param {import('../shared/commands/registry.js').Result & { command?: string }} result
 * @returns {import('../shared/commands/registry.js').Failure & { command?: string }}
 */
export function refused(result) {
  assert.equal(
    result.ok,
    false,
    `${result.command ?? 'the command'} was expected to fail and did not`,
  );
  return /** @type {any} */ (result);
}


/**
 * A real session with a temporary storage root, so command tests exercise the same services
 * the server wires: filesystem storage, the renderer, providers and the job manager.
 * @param {import('node:test').TestContext} t
 * @param {{ now?: () => number, fonts?: { families: string[] } }} [options]
 * @returns {ReturnType<typeof createSession>}
 */
export function sessionHarness(t, options = {}) {
  const root = mkdtempSync(join(tmpdir(), 'comiccraft-test-'));
  const session = createSession({ root, now: options.now ?? (() => 1_700_000_000_000), ...(options.fonts ? { fonts: options.fonts } : {}) });
  t.after(() => rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 25 }));
  return session;
}
