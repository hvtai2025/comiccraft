/**
 * Provider abstractions for artwork generation.
 *
 * This is the **only** place a provider may be reached from, and it is server-side by
 * construction: `shared/` and `public/` are forbidden `fetch` by the boundary gate, so no key
 * path can reach the browser. A provider advertises what it can actually do; a request is
 * refused when the operation is not in that list rather than being sent and failing.
 *
 * The default provider is **offline and deterministic**. It produces real SVG bytes from a
 * prompt, which is enough to exercise the whole job lifecycle — submit, poll, cancel, apply —
 * on a laptop with no credentials and no network. A configured HTTP provider is optional.
 *
 * @module server/ai/providers
 */

import { createHash } from 'node:crypto';

/**
 * @typedef {'text-to-image'|'reference-to-image'|'image-to-image'|'inpaint'|'variation'} Capability
 * @typedef {object} ProviderDescriptor
 * @property {string} id
 * @property {string} label
 * @property {Capability[]} capabilities
 * @property {string[]} models
 * @property {boolean} configured
 * @property {boolean} requiresNetwork
 */

/**
 * @typedef {object} GenerationRequest
 * @property {Capability} operation
 * @property {string} prompt
 * @property {string|null} panelId
 * @property {string[]} inputAssetIds
 * @property {Record<string, unknown>} parameters
 * @property {AbortSignal} signal
 */

/**
 * @typedef {object} GenerationResult
 * @property {string} model
 * @property {string} mime
 * @property {string} data          a data: URL or base64 payload stored on the asset
 * @property {number} width
 * @property {number} height
 * @property {Record<string, unknown>} request
 */

/** @param {string} value @returns {string} */
const sha256 = (value) => createHash('sha256').update(value).digest('hex');

/** @param {string} value @returns {number} */
function hueOf(value) {
  let h = 0;
  for (let i = 0; i < value.length; i++) h = (h * 31 + value.charCodeAt(i)) >>> 0;
  return h % 360;
}

/** @param {string} value @param {number} limit @returns {string} */
const truncate = (value, limit) => value.length <= limit ? value : `${value.slice(0, limit - 1)}…`;

/**
 * A deterministic offline painter. It is an honest provider: it advertises vector artwork,
 * not a model it does not have.
 * @returns {{ descriptor: ProviderDescriptor, generate: (request: GenerationRequest) => Promise<GenerationResult> }}
 */
export function createLocalProvider() {
  return {
    descriptor: {
      id: 'local-vector',
      label: 'Local vector painter',
      capabilities: ['text-to-image', 'reference-to-image', 'image-to-image', 'variation'],
      models: ['deterministic-vector-v1'],
      configured: true,
      requiresNetwork: false,
    },
    async generate(request) {
      if (request.signal.aborted) throw new Error('cancelled before the provider ran');
      const hue = hueOf(`${request.prompt}|${request.panelId ?? ''}`);
      const caption = truncate(request.prompt.replace(/\s+/g, ' ').trim() || 'Untitled panel', 120);
      const svg = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 800 600">',
        `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl(${hue} 45% 82%)"/><stop offset="1" stop-color="hsl(${(hue + 48) % 360} 40% 58%)"/></linearGradient></defs>`,
        '<rect width="800" height="600" fill="url(#g)"/>',
        `<path d="M0 470 L250 300 L470 430 L640 320 L800 420 L800 600 L0 600 Z" fill="hsl(${(hue + 18) % 360} 25% 22% 0.65)"/>`,
        `<circle cx="${180 + (hue % 220)}" cy="190" r="70" fill="hsl(${(hue + 90) % 360} 60% 92% 0.7)"/>`,
        '<rect x="24" y="24" width="752" height="552" fill="none" stroke="#111" stroke-width="6"/>',
        `<text x="40" y="552" font-family="sans-serif" font-size="26" fill="#111">${escapeSvg(caption)}</text>`,
        '</svg>',
      ].join('');
      return {
        model: 'deterministic-vector-v1',
        mime: 'image/svg+xml',
        data: `data:image/svg+xml;base64,${Buffer.from(svg, 'utf8').toString('base64')}`,
        width: 800,
        height: 600,
        request: {
          operation: request.operation,
          prompt: request.prompt,
          parameters: { ...request.parameters },
          inputAssetIds: [...request.inputAssetIds],
          promptHash: sha256(request.prompt),
        },
      };
    },
  };
}

/** @param {unknown} value @returns {string} */
function escapeSvg(value) {
  return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/**
 * An optional HTTP adapter, disabled unless credentials are configured. Credentials are read
 * from the environment at call time and **never** written to a job, an asset or a bundle:
 * `descriptor` reports only whether they exist.
 *
 * @param {{ endpoint?: string, apiKey?: string, model?: string }} options
 * @returns {{ descriptor: ProviderDescriptor, generate: (request: GenerationRequest) => Promise<GenerationResult> }}
 */
export function createHttpProvider(options = {}) {
  const endpoint = options.endpoint ?? process.env.COMICCRAFT_AI_ENDPOINT ?? '';
  const apiKey = options.apiKey ?? process.env.COMICCRAFT_AI_API_KEY ?? '';
  const model = options.model ?? process.env.COMICCRAFT_AI_MODEL ?? 'image-1';
  const configured = endpoint.length > 0 && apiKey.length > 0;

  return {
    descriptor: {
      id: 'http',
      label: 'Configured image endpoint',
      capabilities: ['text-to-image', 'reference-to-image', 'image-to-image', 'variation'],
      models: [model],
      configured,
      requiresNetwork: true,
    },
    async generate(request) {
      if (!configured) throw new Error('the HTTP provider is not configured: set COMICCRAFT_AI_ENDPOINT and COMICCRAFT_AI_API_KEY');
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: { 'content-type': 'application/json', authorization: `Bearer ${apiKey}` },
        body: JSON.stringify({
          model,
          prompt: request.prompt,
          operation: request.operation,
          input_assets: request.inputAssetIds,
          ...request.parameters,
        }),
        signal: request.signal,
      });
      if (!response.ok) {
        throw new Error(`the image endpoint answered ${response.status}: ${truncate(await response.text(), 300)}`);
      }
      const body = /** @type {any} */ (await response.json());
      const data = typeof body.data === 'string' ? body.data
        : typeof body.image === 'string' ? body.image
        : typeof body.url === 'string' ? body.url
        : null;
      if (!data) throw new Error('the image endpoint returned no image data');
      return {
        model: typeof body.model === 'string' ? body.model : model,
        mime: typeof body.mime === 'string' ? body.mime : 'image/png',
        data: data.startsWith('data:') || data.startsWith('http') ? data : `data:image/png;base64,${data}`,
        width: Number.isInteger(body.width) ? body.width : 1024,
        height: Number.isInteger(body.height) ? body.height : 1024,
        request: {
          operation: request.operation,
          prompt: request.prompt,
          parameters: { ...request.parameters },
          inputAssetIds: [...request.inputAssetIds],
          promptHash: sha256(request.prompt),
        },
      };
    },
  };
}

/**
 * @param {{ env?: Record<string, string|undefined> }} [options]
 */
export function createProviders(options = {}) {
  void options;
  const local = createLocalProvider();
  const http = createHttpProvider({});
  const providers = new Map([[local.descriptor.id, local], [http.descriptor.id, http]]);

  return {
    /** @returns {ProviderDescriptor[]} */
    describe() {
      return [...providers.values()].map((p) => ({ ...p.descriptor }));
    },
    /** @param {string} id */
    get(id) {
      return providers.get(id) ?? null;
    },
    /** @returns {string} the provider used when a request names none */
    defaultId() {
      return http.descriptor.configured ? 'http' : 'local-vector';
    },
    /**
     * @param {string} id
     * @param {Capability} operation
     * @returns {{ ok: true, provider: any, model: string } | { ok: false, message: string }}
     */
    resolve(id, operation) {
      const provider = providers.get(id);
      if (!provider) return { ok: false, message: `no provider ${id}; available: ${[...providers.keys()].join(', ')}` };
      if (!provider.descriptor.capabilities.includes(operation)) {
        return { ok: false, message: `provider ${id} does not support ${operation}` };
      }
      if (!provider.descriptor.configured) {
        return { ok: false, message: `provider ${id} is not configured` };
      }
      return { ok: true, provider, model: provider.descriptor.models[0] ?? 'unknown' };
    },
  };
}
