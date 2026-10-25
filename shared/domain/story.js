/**
 * Story import and storyboard proposals.
 *
 * A story is turned into **structured, schema-validated data**, never prose the editor has
 * to parse. A proposal is inert until accepted: it lives beside the project, it can be
 * reviewed page by page, and accepting it is one ordinary command that writes through the
 * same model as a hand-drawn page.
 *
 * Two properties are load-bearing and tested:
 *
 *  - **Stable ids.** Every scene, page and panel in a proposal has a stable key. Regenerating
 *    a description keeps the key, and accepting a regenerated proposal reuses the ids it
 *    already has, so approving new text cannot renumber the book.
 *  - **Scenes are isolated.** Accepting one scene writes only that scene's pages. A change to
 *    one scene provably leaves the others byte-for-byte identical.
 *
 * @module shared/domain/story
 */

import { allocate, unlinkAssetNodes } from './project.js';
import { addNode, createArtboard, findArtboard, removeNode } from './document.js';
import { gridFrames, template as templateOf, TEMPLATES } from './layout.js';
import { rect } from './geom.js';

/**
 * @typedef {object} StoryDialogue
 * @property {string} key
 * @property {string} text
 * @property {'balloon'|'caption'|'sound-effect'} kind
 * @property {'speech'|'thought'|'shout'|'whisper'|null} variant
 * @property {string|null} character
 */

/**
 * @typedef {object} StoryPanel
 * @property {string} key
 * @property {string} shotDescription
 * @property {string} camera
 * @property {string[]} characters
 * @property {StoryDialogue[]} lettering
 * @property {string} artworkPrompt
 */

/**
 * @typedef {object} StoryPage
 * @property {string} key
 * @property {string} name
 * @property {string} template
 * @property {StoryPanel[]} panels
 */

/**
 * @typedef {object} StoryScene
 * @property {string} key
 * @property {string} name
 * @property {string} synopsis
 * @property {StoryPage[]} pages
 */

/**
 * @typedef {object} StoryProposal
 * @property {string} id
 * @property {'proposed'|'approved'|'rejected'|'superseded'} status
 * @property {string} title
 * @property {string} sourceText
 * @property {StoryScene[]} scenes
 * @property {{ scenes: Record<string,string>, pages: Record<string,string>, panels: Record<string,string>, characters: Record<string,string> }} idMap
 * @property {number} baseRevision
 * @property {number} created
 * @property {number} updated
 * @property {number|null} acceptedAt
 * @property {string|null} note
 * @property {string[]} [acceptedSceneKeys]
 * @property {string} logline
 * @property {string} style
 * @property {string} audience
 * @property {{ provider: string, model: string, brief: string, style: string, audience: string, panelsPerPage: number, characters: string[], createdAt: number }} design
 */

const MAX_SOURCE = 200_000;
const MAX_PANELS_PER_PAGE = 6;

/** @param {unknown} value @param {number} limit @returns {string} */
const clipText = (value, limit) => String(value ?? '').slice(0, limit);

/**
 * A stable, non-cryptographic content hash. Proposals use it to say whether a regenerated
 * description is materially different; ids never depend on it, so it is safe to compute here
 * without a runtime dependency.
 * @param {string} text
 * @returns {string}
 */
export function fingerprint(text) {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

/** @param {string} word @returns {string} */
const titleCase = (word) => word.length === 0 ? word : word[0].toUpperCase() + word.slice(1).toLowerCase();

/** True when a line carries an explicit speaker, caption or sound-effect marker. @param {string} line @returns {boolean} */
const isMarkedLine = (line) =>
  /^(sfx|sound|effect|caption|narration|narrator)\s*[:\-]/i.test(line.trim()) ||
  /^[A-Za-z][A-Za-z0-9 .'\-]{0,40}\s*:\s*\S/.test(line.trim());

/** @param {string} line @returns {boolean} */
const looksLikeHeading = (line) =>
  /^(chapter|scene|part|act)\b/i.test(line.trim()) || /^#{1,3}\s+/.test(line.trim());

/**
 * A heading's own words, with the marker removed, so "CHAPTER 1: The City" becomes "The City".
 * @param {string} line
 * @returns {{ level: 'chapter'|'scene', name: string }}
 */
function parseHeading(line) {
  const clean = line.replace(/^#{1,3}\s+/, '').trim();
  const match = /^(chapter|scene|part|act)\s*([0-9ivx]+)?\s*[:\-–—]?\s*(.*)$/i.exec(clean);
  const level = /^scene/i.test(match?.[1] ?? '') ? 'scene' : 'chapter';
  const name = (match?.[3] ?? '').trim() || clean;
  return { level, name: name || 'Untitled' };
}

/**
 * Classify a written line into lettering. The rules are deliberately simple and stated:
 * an explicit `NAME:` prefix is speech, `CAPTION:`/`NARRATION:` is narration, `SFX:` is a
 * sound effect, and a line in parentheses with no other marker is a thought. A line longer
 * than a balloon without a marker becomes a caption, because guessing a speaker invents
 * content the writer did not provide.
 *
 * @param {string} line
 * @returns {{ kind: 'balloon'|'caption'|'sound-effect', variant: 'speech'|'thought'|'shout'|'whisper'|null, character: string|null, text: string }}
 */
export function classifyLine(line) {
  const raw = line.trim();
  const sfx = /^(sfx|sound|effect)\s*[:\-]\s*(.+)$/i.exec(raw);
  if (sfx) return { kind: 'sound-effect', variant: null, character: null, text: sfx[2].trim() };

  const caption = /^(caption|narration|narrator)\s*[:\-]\s*(.+)$/i.exec(raw);
  if (caption) return { kind: 'caption', variant: null, character: null, text: caption[2].trim() };

  const spoken = /^([A-Za-z][A-Za-z0-9 .'\-]{0,40})\s*:\s*(.+)$/.exec(raw);
  if (spoken) {
    const name = spoken[1].trim();
    const text = spoken[2].trim();
    const variant = /^\(.*\)$/.test(text) ? 'thought'
      : /[!]{1,}/.test(text) && text === text.toUpperCase() ? 'shout'
      : /^\(.*\)$/.test(raw) ? 'whisper'
      : 'speech';
    return { kind: 'balloon', variant, character: name, text };
  }

  if (/^\(.+\)$/.test(raw)) {
    return { kind: 'balloon', variant: 'thought', character: null, text: raw.slice(1, -1).trim() };
  }
  if (raw === raw.toUpperCase() && raw.length <= 40 && /[!]{1,}/.test(raw)) {
    return { kind: 'sound-effect', variant: null, character: null, text: raw };
  }
  return { kind: 'caption', variant: null, character: null, text: raw };
}

/**
 * The template that lays out `n` panels on one page. A count with no exact template uses the
 * nearest grid and reports the remainder rather than inventing panels.
 * @param {number} n
 * @returns {string}
 */
export function templateForCount(n) {
  if (n <= 1) return 'single';
  if (n === 2) return 'two-up';
  if (n === 3) return 'three-up';
  if (n === 4) return 'two-by-two';
  if (n === 5) return 'five-grid';
  return 'three-by-two';
}

/**
 * Analyse a manuscript. Deterministic: the same text always produces the same proposal keys,
 * which is what makes "regenerate and accept" testable without comparing prose.
 *
 * @param {string} text
 * @param {object} [options]
 * @param {string} [options.title]
 * @param {number} [options.maxPanelsPerPage]
 * @param {number} [options.now]
 * @param {string} [options.id]
 * @param {number} [options.baseRevision]
 * @param {string} [options.style]
 * @param {string} [options.audience]
 * @param {string[]} [options.characters]
 * @param {string} [options.provider]
 * @param {string} [options.model]
 * @returns {{ proposal: StoryProposal, problems: import('./validate.js').Problem[] }}
 */
export function analyzeStory(text, options = {}) {
  const source = typeof text === 'string' ? text.slice(0, MAX_SOURCE) : '';
  const maxPanels = Math.max(1, Math.min(MAX_PANELS_PER_PAGE, options.maxPanelsPerPage ?? 4));
  const title = (options.title ?? firstHeading(source) ?? 'Untitled Story').slice(0, 200);

  /** @type {{ key: string, name: string, lines: string[] }[]} */
  const sceneBlocks = [];
  let current = { key: 'scene-1', name: 'Scene 1', lines: /** @type {string[]} */ ([]) };

  for (const raw of source.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.length === 0) {
      if (current.lines.length > 0) current.lines.push('');
      continue;
    }
    if (looksLikeHeading(line)) {
      const heading = parseHeading(line);
      if (heading.level === 'scene' || sceneBlocks.length > 0 || current.lines.some((l) => l.length > 0)) {
        if (current.lines.some((l) => l.length > 0)) sceneBlocks.push(current);
        current = { key: `scene-${sceneBlocks.length + 1}`, name: clipText(heading.name, 200), lines: [] };
      } else {
        current.name = clipText(heading.name, 200);
      }
      continue;
    }
    current.lines.push(line);
  }
  if (current.lines.some((l) => l.length > 0) || sceneBlocks.length === 0) sceneBlocks.push(current);

  /** @type {StoryScene[]} */
  const scenes = [];
  let pageNumberTotal = 0;
  for (const block of sceneBlocks) {
    const beats = block.lines
      .join('\n')
      .split(/\n\s*\n/)
      .map((b) => b.split('\n').map((l) => l.trim()).filter(Boolean))
      .filter((b) => b.length > 0);

    /** @type {StoryPage[]} */
    const pages = [];
    for (let i = 0; i < Math.max(1, beats.length); i += maxPanels) {
      const slice = beats.slice(i, i + maxPanels);
      const pageIndex = pages.length + 1;
      pageNumberTotal += 1;
      const pageKey = `${block.key}-page-${pageIndex}`;
      /** @type {StoryPanel[]} */
      const panels = slice.map((lines, j) => {
        const panelKey = `${pageKey}-panel-${j + 1}`;
        // A line with a marker is lettering; a plain line is the panel's description. The
        // distinction is what stops an action beat being invented as a caption.
        const lettering = lines.filter(isMarkedLine).slice(0, 20).map((line, k) => {
          const classified = classifyLine(line);
          return {
            key: `${panelKey}-lettering-${k + 1}`,
            text: clipText(classified.text, 4000),
            kind: classified.kind,
            variant: classified.variant,
            character: classified.character ? clipText(classified.character, 200) : null,
          };
        });
        const characters = [...new Set(lettering.map((l) => l.character).filter((/** @type {string|null} */ c) => typeof c === 'string'))];
        const prose = lines.filter((line) => !isMarkedLine(line));
        const description = (prose.join(' ') || lettering.map((l) => l.text).join(' ') || 'A silent panel.').slice(0, 500);
        return {
          key: panelKey,
          shotDescription: description,
          camera: cameraFor(lines.length, j),
          characters: characters.map((name) => clipText(name, 200)),
          lettering,
          artworkPrompt: artworkPromptFor(description, characters),
        };
      });
      pages.push({
        key: pageKey,
        name: clipText(`Page ${pageNumberTotal}`, 200),
        template: clipText(templateForCount(panels.length), 40),
        panels,
      });
    }
    scenes.push({
      key: block.key,
      name: clipText(block.name, 200),
      synopsis: clipText(block.lines.join(' '), 500),
      pages,
    });
  }

  const logline = firstProse(source) ?? title;
  const style = options.style ?? 'black ink';
  const audience = options.audience ?? 'all ages';
  const proposal = newProposal({
    id: options.id ?? 'proposal-pending',
    title,
    sourceText: source,
    scenes,
    now: options.now ?? 0,
    baseRevision: options.baseRevision ?? 1,
    logline,
    style,
    audience,
    design: {
      provider: options.provider ?? 'local-director',
      model: options.model ?? 'deterministic-storyboard-v1',
      brief: source.slice(0, 2000),
      style,
      audience,
      panelsPerPage: maxPanels,
      characters: [...(options.characters ?? [])],
      createdAt: options.now ?? 0,
    },
  });
  return { proposal, problems: validateProposal(proposal) };
}

/**
 * A camera note derived from the beat, not from a model. It gives the artwork step a
 * concrete instruction while remaining obviously deterministic.
 * @param {number} lineCount
 * @param {number} index
 * @returns {string}
 */
function cameraFor(lineCount, index) {
  if (lineCount >= 3) return 'medium shot, dialogue-heavy';
  if (index === 0) return 'establishing shot';
  return lineCount === 1 ? 'reaction shot' : 'medium shot';
}

/**
 * @param {string} description
 * @param {string[]} characters
 * @returns {string}
 */
function artworkPromptFor(description, characters) {
  const cast = characters.length > 0 ? ` featuring ${characters.join(' and ')}` : '';
  return `${description}${cast}; black ink, clean linework, comic panel`.slice(0, 1000);
}

/** @param {string} text @returns {string | null} */
function firstProse(text) {
  for (const line of text.split(/\r?\n/)) {
    const clean = line.trim();
    if (clean.length === 0 || looksLikeHeading(clean) || isMarkedLine(clean)) continue;
    return clean.slice(0, 240);
  }
  return null;
}

/** @param {string} text @returns {string | null} */
function firstHeading(text) {
  for (const line of text.split(/\r?\n/)) {
    if (looksLikeHeading(line)) return parseHeading(line).name;
    if (line.trim().length > 0) return line.trim().slice(0, 80);
  }
  return null;
}

/**
 * @param {{ id: string, title: string, sourceText: string, scenes: StoryScene[], now: number, baseRevision: number, logline: string, style: string, audience: string, design: any }} args
 * @returns {StoryProposal}
 */
function newProposal({ id, title, sourceText, scenes, now, baseRevision, logline, style, audience, design }) {
  return {
    id,
    status: 'proposed',
    title,
    sourceText,
    scenes,
    idMap: { scenes: {}, pages: {}, panels: {}, characters: {} },
    baseRevision,
    created: now,
    updated: now,
    acceptedAt: null,
    note: null,
    acceptedSceneKeys: [],
    logline,
    style,
    audience,
    design,
  };
}

/**
 * Validate a proposal before it can be stored or accepted. Prose is refused at the door:
 * every panel has a structured description, every lettering has a kind, and every key is
 * unique. `problems` are values, so a caller can show them without catching a throw.
 * @param {unknown} proposal
 * @returns {import('./validate.js').Problem[]}
 */
export function validateProposal(proposal) {
  /** @type {import('./validate.js').Problem[]} */
  const problems = [];
  const p = /** @type {any} */ (proposal);
  if (!p || typeof p !== 'object') {
    return [{ severity: 'error', area: 'project', code: 'not-a-proposal', message: 'a story proposal must be an object' }];
  }
  if (typeof p.title !== 'string' || p.title.trim() === '') {
    problems.push({ severity: 'warning', area: 'project', code: 'untitled-proposal', message: 'the proposal has no title' });
  } else if (p.title.length > 200) {
    problems.push({ severity: 'error', area: 'project', code: 'title-too-long', message: 'the proposal title is longer than 200 characters' });
  }
  if (p.logline !== undefined && String(p.logline).length > 240) {
    problems.push({ severity: 'error', area: 'project', code: 'logline-too-long', message: 'the logline is longer than 240 characters' });
  }
  if (!Array.isArray(p.scenes) || p.scenes.length === 0) {
    problems.push({ severity: 'error', area: 'project', code: 'no-scenes', message: 'a proposal needs at least one scene' });
    return problems;
  }
  const keys = new Set();
  /** @param {string} key @param {string} at */
  const unique = (key, at) => {
    if (typeof key !== 'string' || key.length === 0) {
      problems.push({ severity: 'error', area: 'project', code: 'missing-key', message: `${at} has no stable key`, at });
      return;
    }
    if (keys.has(key)) {
      problems.push({ severity: 'error', area: 'project', code: 'duplicate-key', message: `${at} reuses key ${key}`, at });
    }
    keys.add(key);
  };

  p.scenes.forEach((/** @type {any} */ scene, /** @type {number} */ si) => {
    unique(scene.key, `scene ${si}`);
    if (typeof scene.name !== 'string' || scene.name.trim() === '') {
      problems.push({ severity: 'error', area: 'project', code: 'scene-name', message: `scene ${scene.key} has no name`, at: scene.key });
    } else if (scene.name.length > 200) {
      problems.push({ severity: 'error', area: 'project', code: 'scene-name-too-long', message: `scene ${scene.key} has a name longer than 200 characters`, at: scene.key });
    }
    if (typeof scene.synopsis === 'string' && scene.synopsis.length > 500) {
      problems.push({ severity: 'error', area: 'project', code: 'synopsis-too-long', message: `scene ${scene.key} has a synopsis longer than 500 characters`, at: scene.key });
    }
    if (!Array.isArray(scene.pages) || scene.pages.length === 0) {
      problems.push({ severity: 'error', area: 'project', code: 'scene-no-pages', message: `scene ${scene.key} has no pages`, at: scene.key });
      return;
    }
    for (const page of scene.pages) {
      unique(page.key, `page ${page.key}`);
      if (typeof page.name !== 'string' || page.name.length === 0 || page.name.length > 200) {
        problems.push({ severity: 'error', area: 'project', code: 'page-name', message: `page ${page.key} needs a name of at most 200 characters`, at: page.key });
      }
      if (!TEMPLATES[page.template]) {
        problems.push({ severity: 'error', area: 'project', code: 'unknown-template', message: `page ${page.key} names template ${page.template}`, at: page.key });
      }
      if (!Array.isArray(page.panels) || page.panels.length === 0) {
        problems.push({ severity: 'error', area: 'project', code: 'page-no-panels', message: `page ${page.key} has no panels`, at: page.key });
        continue;
      }
      const expected = templateOf(page.template);
      if (expected.rows * expected.cols < page.panels.length) {
        problems.push({
          severity: 'error', area: 'project', code: 'template-count',
          message: `page ${page.key} has ${page.panels.length} panel(s) but template ${page.template} lays out only ${expected.rows * expected.cols}`,
          at: page.key,
        });
      } else if (expected.rows * expected.cols !== page.panels.length) {
        problems.push({
          severity: 'warning', area: 'project', code: 'template-count',
          message: `page ${page.key} has ${page.panels.length} panel(s) but template ${page.template} lays out ${expected.rows * expected.cols}`,
          at: page.key,
        });
      }
      for (const panel of page.panels) {
        unique(panel.key, `panel ${panel.key}`);
        if (typeof panel.shotDescription !== 'string' || panel.shotDescription.trim() === '') {
          problems.push({ severity: 'error', area: 'project', code: 'panel-description', message: `panel ${panel.key} has no description`, at: panel.key });
        } else if (panel.shotDescription.length > 500) {
          problems.push({ severity: 'error', area: 'project', code: 'panel-description-too-long', message: `panel ${panel.key} has a description longer than 500 characters`, at: panel.key });
        }
        if (typeof panel.artworkPrompt === 'string' && panel.artworkPrompt.length > 1000) {
          problems.push({ severity: 'error', area: 'project', code: 'artwork-prompt-too-long', message: `panel ${panel.key} has an artwork prompt longer than 1000 characters`, at: panel.key });
        }
        if (panel.lettering !== undefined && !Array.isArray(panel.lettering)) {
          problems.push({ severity: 'error', area: 'project', code: 'lettering-not-a-list', message: `panel ${panel.key} has a lettering value that is not a list`, at: panel.key });
        }
        if (Array.isArray(panel.lettering) && panel.lettering.length > 20) {
          problems.push({ severity: 'error', area: 'project', code: 'too-many-lines', message: `panel ${panel.key} has more than 20 lettering entries`, at: panel.key });
        }
        for (const line of panel.lettering ?? []) {
          if (!['balloon', 'caption', 'sound-effect'].includes(line.kind)) {
            problems.push({ severity: 'error', area: 'project', code: 'lettering-kind', message: `lettering ${line.key} has kind ${line.kind}`, at: line.key });
          }
          if (typeof line.text !== 'string' || line.text.trim() === '') {
            problems.push({ severity: 'error', area: 'project', code: 'lettering-text', message: `lettering ${line.key} has no text`, at: line.key });
          } else if (line.text.length > 4000) {
            problems.push({ severity: 'error', area: 'project', code: 'lettering-text-too-long', message: `lettering ${line.key} is longer than 4000 characters`, at: line.key });
          }
        }
      }
    }
  });
  return problems;
}

/**
 * Give a regenerated proposal the ids the project already assigned.
 *
 * The **proposal's stable keys** are the identity, not display names: two scenes may legally
 * be named "Rooftop", and a rename must not duplicate a scene. When a previous proposal is
 * supplied its `idMap` is the authority; name matching is only a fallback, and only when the
 * name is unambiguous.
 *
 * @param {StoryProposal} proposal
 * @param {import('./project.js').Bundle} bundle
 * @param {StoryProposal|null} [previous]
 * @returns {StoryProposal}
 */
export function carryIds(proposal, bundle, previous = null) {
  const next = structuredClone(proposal);
  const { project } = bundle;
  next.idMap = previous
    ? {
        scenes: { ...previous.idMap.scenes },
        pages: { ...previous.idMap.pages },
        panels: { ...previous.idMap.panels },
        characters: { ...previous.idMap.characters },
      }
    : { scenes: {}, pages: {}, panels: {}, characters: {} };

  /** @param {any[]} records @param {string} name @returns {any[]} */
  const byName = (records, name) => records.filter((record) => record.name === name);
  for (const record of Object.values(project.characters)) {
    const character = /** @type {any} */ (record);
    if (next.idMap.characters[character.name] === undefined) next.idMap.characters[character.name] = character.id;
  }

  for (const scene of next.scenes) {
    if (!next.idMap.scenes[scene.key]) {
      const matches = byName(Object.values(project.scenes), scene.name);
      if (matches.length === 1) next.idMap.scenes[scene.key] = matches[0].id;
    }
    const sceneId = next.idMap.scenes[scene.key];
    for (const page of scene.pages) {
      if (!next.idMap.pages[page.key] && sceneId) {
        const scene = project.scenes[sceneId];
        const siblings = (scene?.pageIds ?? []).map((id) => project.pages[id]).filter(Boolean);
        const matches = byName(siblings, page.name);
        if (matches.length === 1) next.idMap.pages[page.key] = matches[0].id;
      }
      const pageId = next.idMap.pages[page.key];
      if (!pageId) continue;
      const panelIds = project.pages[pageId]?.panelIds ?? [];
      page.panels.forEach((panel, index) => {
        if (!next.idMap.panels[panel.key] && panelIds[index]) next.idMap.panels[panel.key] = panelIds[index];
      });
    }
  }
  const sceneKeys = new Set(next.scenes.map((scene) => scene.key));
  const pageKeys = new Set(next.scenes.flatMap((scene) => scene.pages.map((page) => page.key)));
  const panelKeys = new Set(next.scenes.flatMap((scene) => scene.pages.flatMap((page) => page.panels.map((panel) => panel.key))));
  for (const key of Object.keys(next.idMap.scenes)) if (!sceneKeys.has(key)) delete next.idMap.scenes[key];
  for (const key of Object.keys(next.idMap.pages)) if (!pageKeys.has(key)) delete next.idMap.pages[key];
  for (const key of Object.keys(next.idMap.panels)) if (!panelKeys.has(key)) delete next.idMap.panels[key];
  // A partially accepted proposal keeps the scenes the user already took.
  next.acceptedSceneKeys = (previous?.acceptedSceneKeys ?? []).filter((/** @type {string} */ key) => sceneKeys.has(key));
  next.baseRevision = bundle.project.revision ?? 1;
  return next;
}

/**
 * @param {import('./project.js').ComicProject} project
 * @param {string} pageId
 * @returns {string | null}
 */
function parentSceneOf(project, pageId) {
  for (const scene of Object.values(project.scenes)) {
    if (/** @type {any} */ (scene).pageIds.includes(pageId)) return /** @type {any} */ (scene).id;
  }
  return null;
}

/**
 * The scene keys a caller may accept. `all` means every scene in the proposal.
 * @param {StoryProposal} proposal
 * @param {string[]|undefined} sceneKeys
 * @returns {string[]}
 */
export function selectedSceneKeys(proposal, sceneKeys) {
  if (!sceneKeys || sceneKeys.length === 0) return proposal.scenes.map((s) => s.key);
  return proposal.scenes.filter((s) => sceneKeys.includes(s.key)).map((s) => s.key);
}

/**
 * Write the selected scenes of a proposal into the project.
 *
 * For a **new** page the full page is built. For a page that already exists (matched by the
 * carried id map) only descriptions of unlocked panels are refreshed: approved artwork,
 * lettering and geometry are never replaced by a regenerate. That asymmetry is deliberate —
 * a storyboard revision must not silently discard approved content.
 *
 * @param {import('./project.js').Bundle} bundle
 * @param {StoryProposal} proposal
 * @param {object} [options]
 * @param {string[]} [options.sceneKeys]
 * @param {string} [options.chapterId]
 * @param {number} [options.now]
 * @param {boolean} [options.preserveExisting]
 * @returns {{ scenes: string[], pages: string[], panels: string[], characters: string[], updatedPanels: string[], skippedPanels: string[] }}
 */
export function applyProposal(bundle, proposal, options = {}) {
  const { project, document } = bundle;
  const now = options.now ?? 0;
  const preserveExisting = options.preserveExisting !== false;
  const wanted = selectedSceneKeys(proposal, options.sceneKeys);
  const result = {
    scenes: /** @type {string[]} */ ([]),
    pages: /** @type {string[]} */ ([]),
    panels: /** @type {string[]} */ ([]),
    characters: /** @type {string[]} */ ([]),
    updatedPanels: /** @type {string[]} */ ([]),
    skippedPanels: /** @type {string[]} */ ([]),
  };

  const chapterId = options.chapterId ?? ensureChapter(bundle);
  const designCharacters = proposal.design?.characters ?? [];

  // The cast exists even when nobody speaks in the storyboard: a silent character still needs
  // a bible entry and a stable id.
  for (const entry of designCharacters) {
    const name = String(entry).split(':')[0].trim();
    if (name) ensureCharacter(bundle, name, [], result, designCharacters);
  }

  for (const scene of proposal.scenes) {
    if (!wanted.includes(scene.key)) continue;
    let sceneId = proposal.idMap.scenes[scene.key];
    if (!sceneId || !project.scenes[sceneId]) {
      sceneId = allocate(bundle, 'scene');
      project.scenes[sceneId] = { id: sceneId, name: scene.name, summary: scene.synopsis, pageIds: [] };
      project.chapters[chapterId].sceneIds.push(sceneId);
      proposal.idMap.scenes[scene.key] = sceneId;
    }
    result.scenes.push(sceneId);

    let pageNumber = project.scenes[sceneId].pageIds.length;
    for (const page of scene.pages) {
      pageNumber += 1;
      const existingPageId = proposal.idMap.pages[page.key];
      const existingPage = existingPageId ? project.pages[existingPageId] : undefined;

      if (existingPage) {
        if (preserveExisting) {
          // Refresh descriptions only. Approved content stays; locked panels are not even
          // touched by a description update.
          page.panels.forEach((panel, i) => {
            const panelId = existingPage.panelIds[i];
            const record = panelId ? project.panels[panelId] : undefined;
            if (!record) return;
            if (record.locked) { result.skippedPanels.push(record.id); return; }
            record.shotDescription = panel.shotDescription;
            result.updatedPanels.push(record.id);
          });
          result.pages.push(existingPage.id);
          continue;
        }
        // An explicit replace is allowed to rebuild the page, but still keeps its id.
        removePageContent(bundle, existingPage);
      }

      const pageId = existingPage ? existingPage.id : allocate(bundle, 'page');
      let artboardId = existingPage ? existingPage.artboardId : 0;
      if (!existingPage) {
        artboardId = createArtboard(document, page.name || `Page ${pageNumber}`, project.defaultPageSize);
        /** @type {any} */
        const record = {
          id: pageId,
          name: page.name || `Page ${pageNumber}`,
          artboardId,
          panelIds: [],
          status: 'generated',
        };
        project.pages[pageId] = record;
        project.scenes[sceneId].pageIds.push(pageId);
      } else {
        project.pages[pageId].name = page.name || project.pages[pageId].name;
        project.pages[pageId].status = 'generated';
      }
      proposal.idMap.pages[page.key] = pageId;
      result.pages.push(pageId);

      const declared = templateOf(TEMPLATES[page.template] ? page.template : templateForCount(page.panels.length));
      // A template that cannot hold every panel is a structural error, not a reason to stack
      // the extra panels at the page origin. Choose the smallest template that fits.
      const template = declared.rows * declared.cols >= page.panels.length
        ? declared
        : templateOf(templateForCount(page.panels.length));
      const artboard = findArtboard(document, artboardId);
      if (!artboard) throw new Error(`page ${pageId} has no artboard`);
      const direction = project.metadata?.readingDirection === 'rtl' ? 'rtl' : 'ltr';
      const frames = gridFrames(artboard.size, template, direction).slice(0, page.panels.length);

      page.panels.forEach((panel, i) => {
        const frame = frames[i] ?? rect(0, 0, 100, 100);
        const nodeId = addNode(document, artboardId, {
          kind: 'frame', name: 'Panel', frame,
          fill: '#ffffff', stroke: '#111111', strokeWidth: 3,
        });
        const panelId = allocate(bundle, 'panel');
        project.panels[panelId] = {
          id: panelId,
          frameNodeId: nodeId,
          shotDescription: panel.shotDescription,
          camera: panel.camera,
          artworkPrompt: panel.artworkPrompt,
          dialogueIds: [],
          letteringIds: [],
          artworkRefs: [],
          continuityIds: [],
          locked: false,
        };
        project.pages[pageId].panelIds.push(panelId);
        proposal.idMap.panels[panel.key] = panelId;
        result.panels.push(panelId);

        for (const name of panel.characters ?? []) {
          if (name) ensureCharacter(bundle, name, panel.characters, result, designCharacters);
        }
        for (const line of panel.lettering) {
          const characterId = line.character ? ensureCharacter(bundle, line.character, panel.characters, result, designCharacters) : null;
          const letteringId = allocate(bundle, 'lettering');
          const kind = line.kind;
          const style = kind === 'sound-effect'
            ? { fontFamily: 'Bangers' }
            : {};
          const textNodeId = addNode(document, artboardId, {
            kind: 'text',
            name: kind === 'caption' ? 'Caption' : kind === 'sound-effect' ? 'Sound Effect' : 'Balloon',
            frame: insetFrame(frame, 8),
            text: line.text,
            fontSize: kind === 'sound-effect' ? 18 : project.defaultLetteringStyle.sizePt,
            lineHeight: 1.25,
            align: kind === 'sound-effect' ? 'center' : 'left',
            color: '#111111',
          });
          /** @type {any} */
          const lettering = {
            id: letteringId,
            kind,
            variant: line.variant ?? undefined,
            nodeId: textNodeId,
            text: line.text,
            overflowPolicy: 'shrink',
            ...style,
          };
          if (kind === 'balloon') {
            const tailTip = { x: frame.x + frame.width * 0.3, y: frame.y + frame.height - 10 };
            const tailNodeId = addNode(document, artboardId, {
              kind: 'frame', name: 'Balloon Tail',
              frame: rect(quantizeTail(tailTip.x), quantizeTail(tailTip.y), 12, 12),
              fill: '#ffffff', stroke: '#111111', strokeWidth: 2,
            });
            lettering.tailNodeId = tailNodeId;
            lettering.tailTip = tailTip;
            lettering.tailAttached = true;
          }
          project.lettering[letteringId] = lettering;
          project.panels[panelId].letteringIds.push(letteringId);

          if (kind !== 'sound-effect') {
            const dialogueId = allocate(bundle, 'dialogue');
            project.dialogue[dialogueId] = {
              id: dialogueId,
              characterId,
              letteringId,
              text: line.text,
            };
            project.panels[panelId].dialogueIds.push(dialogueId);
          }
        }
      });
      void now;
    }
  }
  return result;
}

/** @param {number} n @returns {number} */
const quantizeTail = (n) => Math.round(n * 1e6) / 1e6;

/**
 * @param {import('./geom.js').Rect} frame
 * @param {number} inset
 * @returns {import('./geom.js').Rect}
 */
function insetFrame(frame, inset) {
  return rect(frame.x + inset, frame.y + inset, Math.max(1, frame.width - inset * 2), Math.max(1, frame.height - inset * 2));
}

/**
 * @param {import('./project.js').Bundle} bundle
 * @returns {string}
 */
function ensureChapter(bundle) {
  const { project } = bundle;
  const existing = Object.keys(project.chapters)[0];
  if (existing) return existing;
  const id = allocate(bundle, 'chapter');
  project.chapters[id] = { id, title: 'Chapter 1', sceneIds: [] };
  return id;
}

/**
 * @param {import('./project.js').Bundle} bundle
 * @param {string} name
 * @param {string[]} descriptions
 * @param {{ characters: string[] }} result
 * @param {string[]} [designCharacters]
 * @returns {string}
 */
function ensureCharacter(bundle, name, descriptions, result, designCharacters = []) {
  const { project } = bundle;
  const existing = Object.values(project.characters).find((c) => /** @type {any} */ (c).name.toLowerCase() === name.toLowerCase());
  if (existing) {
    const id = /** @type {any} */ (existing).id;
    proposalCharacterMap(bundle, name, id);
    return id;
  }
  const id = allocate(bundle, 'character');
  const source = designCharacters.find((entry) => entry.split(':')[0].trim().toLowerCase() === name.toLowerCase());
  const canonical = source && source.includes(':') ? source.slice(source.indexOf(':') + 1).trim() : '';
  project.characters[id] = {
    id,
    name,
    description: canonical || (descriptions.length > 0 ? `${name} appears in the storyboard.` : ''),
    canonicalDescription: canonical,
    referenceAssetIds: [],
    approvedDesignVersion: null,
    continuityRules: [],
    designVersions: [],
  };
  result.characters.push(id);
  proposalCharacterMap(bundle, name, id);
  return id;
}

/**
 * Record the name→id mapping on every proposal in the bundle that names this character. The
 * map belongs to the proposal (the caller has it in hand), but doing it here means a silent
 * design character and a speaking one are recorded the same way.
 * @param {import('./project.js').Bundle} bundle @param {string} name @param {string} id
 */
function proposalCharacterMap(bundle, name, id) {
  for (const proposal of Object.values(bundle.project.proposals ?? {})) {
    const p = /** @type {any} */ (proposal);
    if (p.idMap?.characters) p.idMap.characters[name] = id;
  }
}

/**
 * Remove a page's panels, lettering and dialogue before rebuilding it. Assets and character
 * records are never removed: provenance outlives the page that used it.
 * @param {import('./project.js').Bundle} bundle
 * @param {any} page
 */
function removePageContent(bundle, page) {
  const { project, document } = bundle;
  for (const panelId of [...page.panelIds]) {
    const panel = project.panels[panelId];
    if (!panel) continue;
    for (const lid of panel.letteringIds) {
      const l = project.lettering[lid];
      if (l) {
        removeNode(document, l.nodeId);
        if (l.tailNodeId) removeNode(document, l.tailNodeId);
      }
      delete project.lettering[lid];
    }
    for (const did of panel.dialogueIds) delete project.dialogue[did];
    unlinkAssetNodes(project, panel.artworkRefs);
    removeNode(document, panel.frameNodeId);
    delete project.panels[panelId];
  }
  page.panelIds = [];
}

/**
 * The local **comic director**: turn an idea or a prose brief into a structured storyboard.
 *
 * This is deliberately not a language model. It is a deterministic planner that reads
 * sentences as beats, groups them into pages, chooses a camera progression, recognises quoted
 * dialogue and sound effects, and names scenes from the places it can see. It is honest about
 * being a planner — the proposal records `provider: local-director` and its model — and it
 * gives the review step something real to accept or reject. A deployment may replace it with
 * an LLM through `server/ai/story.js`; the proposal shape does not change.
 *
 * @param {string} ideas
 * @param {object} [options]
 * @param {string} [options.title]
 * @param {string} [options.style]
 * @param {string} [options.audience]
 * @param {string[]} [options.characters] names, optionally `Name: appearance`
 * @param {number} [options.maxPanelsPerPage]
 * @param {number} [options.now]
 * @param {number} [options.baseRevision]
 * @param {string} [options.id]
 * @returns {{ proposal: StoryProposal, problems: import('./validate.js').Problem[] }}
 */
export function designStoryboard(ideas, options = {}) {
  const source = typeof ideas === 'string' ? ideas.slice(0, MAX_SOURCE) : '';
  const style = options.style ?? 'black ink';
  const audience = options.audience ?? 'all ages';
  const maxPanels = Math.max(1, Math.min(MAX_PANELS_PER_PAGE, options.maxPanelsPerPage ?? 3));
  const characterEntries = options.characters ?? [];
  const characters = characterEntries.map((entry) => entry.split(':')[0].trim()).filter(Boolean);

  // A manuscript with explicit SCENE:/NAME: markers already has a structure; use the parser
  // that understands those markers rather than re-inventing beats over it.
  const marked = /^\s*(scene|chapter|part|act)\b/im.test(source) || /^\s*[A-Za-z][A-Za-z0-9 .'\-]{0,40}\s*:\s*\S/m.test(source);
  if (marked) {
    return analyzeStory(source, {
      title: options.title,
      style,
      audience,
      characters: characterEntries,
      maxPanelsPerPage: maxPanels,
      now: options.now,
      baseRevision: options.baseRevision,
      id: options.id,
      provider: 'local-director',
      model: 'deterministic-storyboard-v1',
    });
  }

  const paragraphs = source.split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean);
  const blocks = paragraphs.length > 0 ? paragraphs : [source.trim() || 'A silent page.'];
  /** @type {StoryScene[]} */
  const scenes = [];
  let pageNumber = 0;

  blocks.forEach((block, blockIndex) => {
    const sentences = splitSentences(block);
    const chunks = [];
    for (let i = 0; i < sentences.length; i += maxPanels) chunks.push(sentences.slice(i, i + maxPanels));
    if (chunks.length === 0) chunks.push(['A silent page.']);

    /** @type {StoryPage[]} */
    const pages = chunks.map((chunk, chunkIndex) => {
      pageNumber += 1;
      const pageKey = `scene-${blockIndex + 1}-page-${chunkIndex + 1}`;
      /** @type {StoryPanel[]} */
      const panels = chunk.map((sentence, panelIndex) => {
        const panelKey = `${pageKey}-panel-${panelIndex + 1}`;
        const quoted = [...sentence.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
        /** @type {StoryDialogue[]} */
        const lettering = quoted.length > 0
          ? quoted.map((text, i) => ({
              key: `${panelKey}-lettering-${i + 1}`,
              text,
              kind: /** @type {'balloon'} */ ('balloon'),
              variant: /** @type {'speech'} */ ('speech'),
              character: characters[0] ?? null,
            }))
          : [{
              key: `${panelKey}-lettering-1`,
              text: sentence.replace(/"/g, ''),
              kind: isSfxSentence(sentence) ? 'sound-effect' : 'caption',
              variant: null,
              character: null,
            }];
        const cast = lettering.map((line) => line.character).filter((/** @type {string|null} */ name) => typeof name === 'string');
        return {
          key: panelKey,
          shotDescription: clipText(sentence, 500),
          camera: cameraFor(sentences.length, panelIndex),
          characters: [...new Set(cast)],
          lettering,
          artworkPrompt: clipText(`${sentence}; ${style}; clean linework, comic panel`, 1000),
        };
      });
      return {
        key: pageKey,
        name: clipText(`Page ${pageNumber}`, 200),
        template: clipText(templateForCount(panels.length), 40),
        panels,
      };
    });

    scenes.push({
      key: `scene-${blockIndex + 1}`,
      name: clipText(locationOf(block) ?? `Scene ${blockIndex + 1}`, 200),
      synopsis: clipText(sentences.slice(0, 3).join(' '), 500),
      pages,
    });
  });

  const logline = clipText(sentencesOf(source)[0] ?? options.title ?? 'A comic from an idea', 240);
  const proposal = newProposal({
    id: options.id ?? 'proposal-pending',
    title: options.title ?? logline.slice(0, 80),
    sourceText: source,
    scenes,
    now: options.now ?? 0,
    baseRevision: options.baseRevision ?? 1,
    logline,
    style,
    audience,
    design: {
      provider: 'local-director',
      model: 'deterministic-storyboard-v1',
      brief: source.slice(0, 2000),
      style,
      audience,
      panelsPerPage: maxPanels,
      characters: characterEntries,
      createdAt: options.now ?? 0,
    },
  });
  return { proposal, problems: validateProposal(proposal) };
}

/**
 * Split prose into sentences without breaking inside a quotation. A question mark inside
 * `"Who is there?"` is part of the dialogue, not the end of the narrative sentence.
 * @param {string} text
 * @returns {string[]}
 */
function splitSentences(text) {
  /** @type {string[]} */
  const out = [];
  let buffer = '';
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    buffer += ch;
    if (ch === '"') quoted = !quoted;
    if (!quoted && (ch === '.' || ch === '!' || ch === '?')) {
      const next = text[i + 1] ?? '';
      if (next === '' || /\s/.test(next)) {
        const sentence = buffer.trim();
        if (sentence) out.push(sentence);
        buffer = '';
      }
    }
  }
  const last = buffer.trim();
  if (last) out.push(last);
  return out;
}

/** @param {string} source @returns {string[]} */
function sentencesOf(source) {
  return splitSentences(source);
}

/** @param {string} sentence @returns {boolean} */
function isSfxSentence(sentence) {
  return /\b(boom|bang|crash|krakoom|thud|whoosh|clang|splash|zap)\b/i.test(sentence) || /^[A-Z!? ]{4,}$/.test(sentence.trim());
}

/** @param {string} block @returns {string | null} */
function locationOf(block) {
  const places = ['rooftop', 'roof', 'alley', 'street', 'city', 'forest', 'room', 'office', 'school', 'beach', 'space', 'station', 'harbour', 'harbor', 'desert', 'mountain', 'lab', 'library'];
  const lower = block.toLowerCase();
  const found = places.find((place) => lower.includes(place));
  return found ? found[0].toUpperCase() + found.slice(1) : null;
}

/**
 * A summary an agent or the storyboard UI can read without loading the whole proposal.
 * @param {StoryProposal} proposal
 * @returns {object}
 */
export function proposalOutline(proposal) {
  return {
    proposalId: proposal.id,
    status: proposal.status,
    title: proposal.title,
    created: proposal.created,
    updated: proposal.updated,
    baseRevision: proposal.baseRevision,
    logline: proposal.logline,
    style: proposal.style,
    audience: proposal.audience,
    design: proposal.design,
    scenes: proposal.scenes.map((scene) => ({
      key: scene.key,
      name: scene.name,
      synopsis: scene.synopsis,
      pages: scene.pages.map((page) => ({
        key: page.key,
        name: page.name,
        template: page.template,
        panels: page.panels.length,
        lettering: page.panels.reduce((n, p) => n + p.lettering.length, 0),
      })),
    })),
  };
}

/** @param {string} text @returns {string} */
export const sourceFingerprint = (text) => fingerprint(text);
