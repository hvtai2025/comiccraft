/**
 * The shared boundary, proved behaviourally.
 *
 * `tools/check-boundary.js` proves it statically and by import. This proves it the way a
 * browser would notice: it **replaces the browser globals with throwing accessors**, then
 * exercises the domain, so a reference inside a function body throws here even though a text
 * scan cannot see it.
 *
 * The poisoning happens *inside* the test, not at module scope. An earlier version installed it
 * at the top level and deleted it at the bottom, which removed the accessors before
 * `node:test` ran the callbacks — so every assertion passed while nothing was poisoned. The
 * last test in this file exists to make that failure mode impossible to reintroduce.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

const POISONED = ['document', 'window', 'navigator', 'localStorage', 'indexedDB', 'fetch', 'requestAnimationFrame'];

/**
 * Run `body` with the browser globals replaced by throwing accessors, and return which of them
 * `body` tripped.
 *
 * Returning the list rather than asserting on it is deliberate: the first test *wants* to trip
 * two of them, to prove the mechanism works, and a helper that insisted on an empty list would
 * make that test impossible to write.
 * @param {() => Promise<void> | void} body
 * @returns {Promise<string[]>} the globals that were touched, in order
 */
async function withPoisonedGlobals(body) {
  /** @type {string[]} */
  const tripped = [];
  /** @type {Record<string, PropertyDescriptor | undefined>} */
  const saved = {};

  for (const name of POISONED) {
    saved[name] = Object.getOwnPropertyDescriptor(globalThis, name);
    Object.defineProperty(globalThis, name, {
      configurable: true,
      get() {
        tripped.push(name);
        throw new Error(`shared/ reached for the browser global '${name}'`);
      },
      set() {
        tripped.push(name);
        throw new Error(`shared/ assigned the browser global '${name}'`);
      },
    });
  }

  try {
    await body();
  } finally {
    for (const name of POISONED) {
      // `delete` then restore: the descriptor may have been absent, and reinstating a
      // getter-only property with `undefined` would be a different global.
      delete /** @type {any} */ (globalThis)[name];
      if (saved[name]) Object.defineProperty(globalThis, name, saved[name]);
    }
  }
  return tripped;
}

test('the poisoning mechanism actually works', async () => {
  // Without this, a mistake in `withPoisonedGlobals` would make every test below pass
  // vacuously — which is precisely what happened the first time this file was written.
  const tripped = await withPoisonedGlobals(() => {
    assert.throws(() => globalThis.document, /browser global 'document'/);
    assert.throws(() => globalThis.window, /browser global 'window'/);
  });
  assert.deepEqual(tripped, ['document', 'window'], 'both accessors fired');
  assert.equal(typeof globalThis.document, 'undefined', 'the globals were restored');
  assert.equal(typeof globalThis.fetch, 'function', 'and fetch came back as it was');
});

test('the domain runs a real command with every browser global poisoned', async () => {
  const domain = await import('../shared/domain/index.js');
  const commands = await import('../shared/commands/index.js');

  const tripped = await withPoisonedGlobals(async () => {
    const bundle = domain.createBundle('Boundary Probe');
    const registry = commands.buildRegistry();
    const ctx = {
      bundle,
      history: new domain.History(),
      env: { onServer: false, now: Date.now },
      services: { measure: domain.stubMeasure(0.5) },
    };

    const added = await commands.dispatch(registry, ctx, 'comic.page.add', { name: 'Page 1', template: 'two-up' });
    assert.equal(added.ok, true, added.ok ? '' : added.message);
    assert.equal(added.value.panels, 2, 'the template produced two panels');

    const panelId = bundle.project.pages[added.value.pageId].panelIds[0];
    const lettered = await commands.dispatch(registry, ctx, 'comic.lettering.add', {
      panelId, kind: 'balloon', text: 'ONE TWO THREE FOUR FIVE', overflowPolicy: 'shrink',
    });
    assert.equal(lettered.ok, true, lettered.ok ? '' : lettered.message);

    const validated = await commands.dispatch(registry, ctx, 'comic.project.validate', {});
    assert.equal(validated.value.errors, 0);
  });
  assert.deepEqual(tripped, [], `the domain touched: ${tripped.join(', ')}`);
});

test('geometry, layout and lettering are pure arithmetic', async () => {
  const domain = await import('../shared/domain/index.js');

  const tripped = await withPoisonedGlobals(() => {
    const frames = domain.gridFrames({ width: 900, height: 600 }, domain.template('three-up'));
    assert.equal(frames.length, 3);
    assert.equal(frames[0].y, frames[1].y, 'a one-row template gives every panel the same y');
    assert.ok(frames[0].x < frames[1].x, 'reading order runs left to right');

    const fits = domain.fitText({
      text: 'ONE TWO THREE FOUR FIVE SIX SEVEN',
      frame: domain.rect(0, 0, 120, 60),
      style: { family: 'Comic Neue', sizePt: 12 },
      measure: domain.stubMeasure(0.5),
      policy: 'warn',
    });
    assert.ok(fits.lines.length > 1, 'the text wrapped');
    assert.equal(fits.overflowed, true, 'and it overflowed, which the caller must be told');
  });
  assert.deepEqual(tripped, [], `the domain touched: ${tripped.join(', ')}`);
});
