/**
 * The story-design agent.
 *
 * **Local by default.** The deterministic director in `shared/domain/story.js` runs anywhere
 * and needs no key. When `COMICCRAFT_STORY_ENDPOINT` and `COMICCRAFT_STORY_API_KEY` are set, an
 * optional HTTP model can design the storyboard instead; its output is parsed, normalised and
 * run through the same `validateProposal` gate as any other input.
 *
 * Model output is **untrusted input**: sizes are capped, strings are truncated, missing keys
 * get explicit stable keys, and a proposal that fails validation is returned as a failure —
 * never stored as authoritative project data.
 *
 * @module server/ai/story
 */

import { designStoryboard, templateForCount, validateProposal } from '../../shared/domain/story.js';
import { TEMPLATES } from '../../shared/domain/layout.js';
import { errorsOf } from '../../shared/domain/validate.js';

/**
 * @typedef {object} StoryAgentOptions
 * @property {string} [endpoint]
 * @property {string} [apiKey]
 * @property {string} [model]
 * @property {number} [timeoutMs]
 */

/** @param {unknown} value @param {number} limit @returns {string} */
const clip = (value, limit) => String(value ?? '').slice(0, limit);

/** @param {unknown} value @param {number} limit @returns {unknown[]} */
const cap = (value, limit) => Array.isArray(value) ? value.slice(0, limit) : [];

/**
 * Normalise model JSON into the proposal shape. Keys are made explicit and deterministic when
 * the model omits them, so a regeneration still has something to map ids to.
 * @param {any} raw
 * @param {{ ideas: string, title?: string, style?: string, audience?: string, characters: string[], maxPanelsPerPage: number, now: number, baseRevision: number, model: string }} options
 * @returns {any}
 */
function normalize(raw, options) {
  const proposal = /** @type {any} */ (raw && typeof raw === 'object' ? raw : {});
  const scenes = cap(proposal.scenes, 100).map((scene, si) => {
    const s = /** @type {any} */ (scene && typeof scene === 'object' ? scene : {});
    const pages = cap(s.pages, 100).map((page, pi) => {
      const p = /** @type {any} */ (page && typeof page === 'object' ? page : {});
      const panels = cap(p.panels, 6).map((panel, ni) => {
        const n = /** @type {any} */ (panel && typeof panel === 'object' ? panel : {});
        const lettering = cap(n.lettering, 20).map((line, li) => {
          const l = /** @type {any} */ (line && typeof line === 'object' ? line : {});
          return {
            key: clip(l.key, 120) || `scene-${si + 1}-page-${pi + 1}-panel-${ni + 1}-lettering-${li + 1}`,
            text: clip(l.text, 4000),
            kind: ['balloon', 'caption', 'sound-effect'].includes(l.kind) ? l.kind : 'caption',
            variant: ['speech', 'thought', 'shout', 'whisper'].includes(l.variant) ? l.variant : null,
            character: l.character ? clip(l.character, 200) : null,
          };
        });
        return {
          key: clip(n.key, 120) || `scene-${si + 1}-page-${pi + 1}-panel-${ni + 1}`,
          shotDescription: clip(n.shotDescription, 500) || 'A silent panel.',
          camera: clip(n.camera, 200) || 'medium shot',
          characters: cap(n.characters, 20).map((/** @type {unknown} */ name) => clip(name, 200)),
          lettering,
          artworkPrompt: clip(n.artworkPrompt, 1000) || clip(n.shotDescription, 500) || 'A comic panel.',
        };
      });
      const declared = clip(p.template, 40);
      const declaredTemplate = TEMPLATES[declared];
      const template = declaredTemplate && declaredTemplate.rows * declaredTemplate.cols >= panels.length
        ? declared
        : templateForCount(panels.length);
      return {
        key: clip(p.key, 120) || `scene-${si + 1}-page-${pi + 1}`,
        name: clip(p.name, 200) || `Page ${pi + 1}`,
        template,
        panels,
      };
    });
    return {
      key: clip(s.key, 120) || `scene-${si + 1}`,
      name: clip(s.name, 200) || `Scene ${si + 1}`,
      synopsis: clip(s.synopsis, 500),
      pages,
    };
  });

  const style = clip(options.style ?? proposal.style, 80) || 'black ink';
  const audience = ['all ages', 'teen', 'mature'].includes(options.audience ?? proposal.audience)
    ? (options.audience ?? proposal.audience)
    : 'all ages';
  return {
    id: 'proposal-pending',
    status: 'proposed',
    title: clip(options.title ?? proposal.title, 200) || 'Untitled Comic',
    sourceText: clip(options.ideas, 200000),
    scenes,
    idMap: { scenes: {}, pages: {}, panels: {}, characters: {} },
    baseRevision: options.baseRevision,
    created: options.now,
    updated: options.now,
    acceptedAt: null,
    note: null,
    logline: clip(proposal.logline, 240) || clip(options.ideas, 240),
    style,
    audience,
    design: {
      provider: 'http',
      model: options.model,
      brief: clip(options.ideas, 2000),
      style,
      audience,
      panelsPerPage: options.maxPanelsPerPage,
      characters: [...options.characters],
      createdAt: options.now,
    },
  };
}

/**
 * @param {StoryAgentOptions} [options]
 */
export function createStoryAgent(options = {}) {
  const endpoint = options.endpoint ?? process.env.COMICCRAFT_STORY_ENDPOINT ?? '';
  const apiKey = options.apiKey ?? process.env.COMICCRAFT_STORY_API_KEY ?? '';
  const model = options.model ?? process.env.COMICCRAFT_STORY_MODEL ?? 'story-director-1';
  const timeoutMs = options.timeoutMs ?? 60_000;
  const configured = endpoint.length > 0 && apiKey.length > 0;

  return {
    /** @returns {any[]} */
    describe() {
      return [
        { id: 'local', label: 'Local comic director', model: 'deterministic-storyboard-v1', configured: true, requiresNetwork: false },
        { id: 'http', label: 'Configured story model', model, configured, requiresNetwork: true },
      ];
    },
    defaultId() { return configured ? 'http' : 'local'; },

    /**
     * @param {string} ideas
     * @param {{ title?: string, style?: string, audience?: string, characters?: string[], maxPanelsPerPage?: number, now?: number, baseRevision?: number, provider?: 'local'|'http' }} [opts]
     * @returns {Promise<{ ok: true, proposal: any, problems: any[] } | { ok: false, kind: string, message: string }>}
     */
    async design(ideas, opts = {}) {
      const maxPanels = Math.max(1, Math.min(6, opts.maxPanelsPerPage ?? 3));
      const localOptions = {
        title: opts.title,
        style: opts.style,
        audience: opts.audience,
        characters: opts.characters ?? [],
        maxPanelsPerPage: maxPanels,
        now: opts.now ?? 0,
        baseRevision: opts.baseRevision ?? 1,
      };
      if (!configured || opts.provider === 'local') {
        const local = designStoryboard(ideas, localOptions);
        return { ok: true, proposal: local.proposal, problems: local.problems };
      }

      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      try {
        const response = await fetch(endpoint, {
          method: 'POST',
          headers: { 'content-type': 'application/json', authorization: `Bearer ${apiKey}` },
          body: JSON.stringify({
            model,
            temperature: 0.4,
            response_format: { type: 'json_object' },
            messages: [
              {
                role: 'system',
                content: 'You are a comic director. Return only JSON with this shape: '
                  + '{"title":string,"logline":string,"scenes":[{"key":string,"name":string,"synopsis":string,'
                  + '"pages":[{"key":string,"name":string,"template":"single|two-up|three-up|two-by-two|three-by-two|five-grid",'
                  + '"panels":[{"key":string,"shotDescription":string,"camera":string,"characters":string[],'
                  + '"artworkPrompt":string,"lettering":[{"key":string,"text":string,"kind":"balloon|caption|sound-effect",'
                  + '"variant":"speech|thought|shout|whisper"|null,"character":string|null}]}]}]}]}. '
                  + `Use at most ${maxPanels} panels per page, keep every key unique and stable, and never write prose outside the JSON.`,
              },
              {
                role: 'user',
                content: JSON.stringify({
                  ideas,
                  title: opts.title,
                  style: opts.style ?? 'black ink',
                  audience: opts.audience ?? 'all ages',
                  characters: opts.characters ?? [],
                }),
              },
            ],
          }),
          signal: controller.signal,
        });
        if (!response.ok) {
          return { ok: false, kind: 'conflict', message: `the story model answered ${response.status}: ${clip(await response.text(), 300)}` };
        }
        const body = /** @type {any} */ (await response.json());
        const text = extractText(body);
        const raw = parseJson(text);
        if (!raw) return { ok: false, kind: 'conflict', message: 'the story model returned no parsable JSON storyboard' };
        const proposal = normalize(raw, { ideas, ...localOptions, model });
        const problems = validateProposal(proposal);
        const errors = errorsOf(problems);
        if (errors.length > 0) {
          return { ok: false, kind: 'conflict', message: `the story model produced an invalid storyboard: ${errors[0].message}` };
        }
        return { ok: true, proposal, problems };
      } catch (error) {
        const message = error instanceof Error && error.name === 'AbortError'
          ? `the story model did not answer within ${Math.round(timeoutMs / 1000)}s`
          : error instanceof Error ? error.message : String(error);
        return { ok: false, kind: 'conflict', message: `the story model failed: ${message}` };
      } finally {
        clearTimeout(timer);
      }
    },
  };
}

/** @param {any} body @returns {string} */
function extractText(body) {
  if (typeof body?.output_text === 'string') return body.output_text;
  if (typeof body?.content === 'string') return body.content;
  if (Array.isArray(body?.content)) return body.content.map((/** @type {any} */ part) => part?.text ?? '').join('');
  const choice = body?.choices?.[0];
  if (typeof choice?.message?.content === 'string') return choice.message.content;
  if (Array.isArray(choice?.message?.content)) return choice.message.content.map((/** @type {any} */ part) => part?.text ?? '').join('');
  return JSON.stringify(body ?? {});
}

/** @param {string} text @returns {any | null} */
function parseJson(text) {
  const cleaned = String(text ?? '').replace(/^```(?:json)?\s*/i, '').replace(/```\s*$/i, '').trim();
  try {
    return JSON.parse(cleaned);
  } catch {
    const start = cleaned.indexOf('{');
    const end = cleaned.lastIndexOf('}');
    if (start === -1 || end <= start) return null;
    try {
      return JSON.parse(cleaned.slice(start, end + 1));
    } catch {
      return null;
    }
  }
}
