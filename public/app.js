/**
 * ComicCraft in the browser — the studio shell.
 *
 * The rule this file keeps: **no comic logic lives here**. Panels read project state from the
 * bundle and act by dispatching commands through the same registry the server and MCP use. A
 * click, a `curl` and an agent call therefore cannot disagree about what an edit means.
 *
 * What differs from the server is only the *services*: real text metrics here, a renderer and
 * a filesystem there. That is why `serverOnly` is a property of a command rather than a
 * second implementation of it.
 *
 * @module public/app
 */

import { buildRegistry, dispatch } from '../shared/commands/index.js';
import { History } from '../shared/domain/history.js';
import { createBundle } from '../shared/domain/project.js';
import { errorsOf, validateBundle, warningsOf } from '../shared/domain/validate.js';
import { serialize, deserialize, migrate } from '../shared/domain/bundle.js';
import { TEMPLATES, template as templateOf } from '../shared/domain/layout.js';
import { orderedPages } from '../shared/commands/commands/export.js';
import { catalogue } from '../shared/domain/fonts.js';
import { createMeasure } from './editor/measure.js';
import { clearImageCache, drawPage } from './editor/render.js';
import { nodeAt, toPageUnits } from './editor/hit-test.js';
import { saveBundle, loadBundle, listProjects } from './storage/local.js';

const canvas = /** @type {HTMLCanvasElement} */ (document.getElementById('canvas'));
const ctx = /** @type {CanvasRenderingContext2D} */ (canvas.getContext('2d'));
const measure = createMeasure(ctx);
const registry = buildRegistry();
const history = new History();

/**
 * The fonts this browser actually has. No font ships with ComicCraft, so on a machine with
 * neither face installed this list is empty and every lettering rule still works — with a
 * missing-font report, exactly as the fontless contract says.
 * @returns {{ families: string[] }}
 */
function browserFonts() {
  // Only faces the document actually loaded count. `document.fonts.check` answers true for
  // an unknown family because a fallback can render it, so trusting it would claim a face
  // this build does not ship. No @font-face is declared here: the catalogue names faces a
  // deployment may provide, and an empty list is the honest answer on a bare machine.
  try {
    const loaded = [...document.fonts].map((face) => face.family.replace(/^["']|["']$/g, ''));
    return { families: catalogue().map((face) => face.name).filter((name) => loaded.includes(name)) };
  } catch {
    return { families: [] };
  }
}

/** @type {{ bundle: any, pageId: string | null, selectedId: number | null, scale: number, src: string | null, view: 'canvas'|'storyboard'|'review', publication: any, storyText: string }} */
const state = {
  bundle: createBundle('Untitled Comic'),
  pageId: null,
  selectedId: null,
  scale: 0.5,
  src: null,
  view: 'canvas',
  publication: null,
  storyText: '',
};

const el = (/** @type {string} */ id) => /** @type {HTMLElement} */ (document.getElementById(id));

/**
 * Adopt a different bundle and forget the previous project's history. A snapshot from another
 * project is not a step in this one: replaying it would splice pages into a document they do
 * not belong to.
 * @param {any} bundle
 */
function adoptBundle(bundle) {
  state.bundle = bundle;
  clearImageCache();
  history.undoStack.length = 0;
  history.redoStack.length = 0;
  const pages = Object.keys(bundle.project.pages);
  state.pageId = pages[0] ?? null;
  state.selectedId = null;
}

/** A filesystem-safe local project key. */
function saveKey() {
  const slug = String(state.bundle.project.title || 'comic')
    .toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40) || 'comic';
  return `comic-${slug}-${Date.now().toString(36)}`;
}
/** @param {unknown} value @returns {string} */
const esc = (value) => String(value ?? '')
  .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;').replace(/'/g, '&#39;');

/* ------------------------------------------------------------------ commands */

/**
 * @param {string} id @param {unknown} [params]
 * @returns {Promise<any>}
 */
async function run(id, params) {
  const result = await dispatch(registry, {
    bundle: state.bundle,
    history,
    // No renderer and no filesystem: those commands are `serverOnly` and report themselves
    // disabled rather than failing halfway through with a missing service.
    env: { onServer: false, now: Date.now },
    services: { measure, registry, fonts: browserFonts() },
  }, id, params);

  if (result.ok && result.value?.bundle) {
    adoptBundle(result.value.bundle);
  }
  if (!result.ok) toast(result.message, true);
  else if (result.changed) toast(id.split('.').slice(1).join('.'));
  return result;
}

/* ------------------------------------------------------------------ ui state */

/** @param {string} message @param {boolean} [bad] */
function toast(message, bad = false) {
  let node = document.querySelector('.toast');
  if (!node) {
    node = document.createElement('div');
    node.className = 'toast';
    document.body.append(node);
  }
  node.textContent = message;
  node.classList.toggle('bad', bad);
  node.classList.add('show');
  clearTimeout(/** @type {any} */ (toast)._timer);
  /** @type {any} */ (toast)._timer = setTimeout(() => node?.classList.remove('show'), 2200);
}


/**
 * An inline prompt. The studio must work in environments where a native browser prompt is disabled,
 * and a modal that is part of the document is easier to style and to test.
 * @param {{ title: string, label?: string, value?: string, multiline?: boolean, action?: string }} options
 * @returns {Promise<string|null>}
 */
function askText({ title, label = '', value = '', multiline = false, action = 'OK' }) {
  return new Promise((resolve) => {
    const dialog = document.createElement('dialog');
    dialog.className = 'dialog';
    dialog.innerHTML = `
      <form method="dialog">
        <h2>${esc(title)}</h2>
        ${label ? `<p class="muted small">${esc(label)}</p>` : ''}
        ${multiline
          ? `<textarea id="dialogInput" rows="8">${esc(value)}</textarea>`
          : `<input id="dialogInput" type="text" value="${esc(value)}">`}
        <div class="dialog-actions">
          <button value="cancel" class="ghost" type="submit">Cancel</button>
          <button value="ok" class="primary" type="submit">${esc(action)}</button>
        </div>
      </form>`;
    document.body.append(dialog);
    const input = /** @type {HTMLInputElement|HTMLTextAreaElement} */ (dialog.querySelector('#dialogInput'));
    dialog.addEventListener('close', () => {
      const answer = dialog.returnValue === 'ok' ? input.value : null;
      dialog.remove();
      resolve(answer);
    });
    dialog.showModal();
    input.focus();
    input.select?.();
  });
}

/**
 * @param {{ title: string, options: string[], value?: string, action?: string }} args
 * @returns {Promise<string|null>}
 */
function askChoice({ title, options, value, action = 'Open' }) {
  return new Promise((resolve) => {
    const dialog = document.createElement('dialog');
    dialog.className = 'dialog';
    dialog.innerHTML = `
      <form method="dialog">
        <h2>${esc(title)}</h2>
        <select id="dialogChoice">${options.map((option) => `<option ${option === value ? 'selected' : ''}>${esc(option)}</option>`).join('')}</select>
        <div class="dialog-actions">
          <button value="cancel" class="ghost" type="submit">Cancel</button>
          <button value="ok" class="primary" type="submit">${esc(action)}</button>
        </div>
      </form>`;
    document.body.append(dialog);
    dialog.addEventListener('close', () => {
      const choice = dialog.returnValue === 'ok' ? /** @type {HTMLSelectElement} */ (dialog.querySelector('#dialogChoice')).value : null;
      dialog.remove();
      resolve(choice);
    });
    dialog.showModal();
    dialog.querySelector('select')?.focus();
  });
}

function currentPage() {
  return state.pageId ? state.bundle.project.pages[state.pageId] ?? null : null;
}

/** @returns {any | null} */
function selectedNode() {
  return state.selectedId !== null ? state.bundle.document.nodes[String(state.selectedId)] ?? null : null;
}

/** @param {number|null} nodeId @returns {any | null} */
function panelOfNode(nodeId) {
  if (nodeId === null) return null;
  return Object.values(state.bundle.project.panels)
    .find((/** @type {any} */ panel) => panel.frameNodeId === nodeId) ?? null;
}

/** @param {number|null} nodeId @returns {any | null} */
function letteringOfNode(nodeId) {
  if (nodeId === null) return null;
  return Object.values(state.bundle.project.lettering)
    .find((/** @type {any} */ lettering) => lettering.nodeId === nodeId || lettering.tailNodeId === nodeId) ?? null;
}

function setView(/** @type {'canvas'|'storyboard'|'review'} */ view) {
  state.view = view;
  for (const [name, button, panel] of [
    ['canvas', 'viewCanvasBtn', 'canvasView'],
    ['storyboard', 'viewStoryboardBtn', 'storyboardView'],
    ['review', 'viewReviewBtn', 'reviewView'],
  ]) {
    const active = view === name;
    el(button).classList.toggle('current', active);
    el(button).setAttribute('aria-pressed', String(active));
    el(panel).toggleAttribute('hidden', !active);
  }
  render();
}

/**
 * The workflow stepper. It is derived from state, never stored: a proposal awaiting review is
 * the review step; pages without artwork are the edit/art steps; fully illustrated pages are
 * ready to publish. A user can always jump to a step.
 */
function renderWorkflow() {
  const pages = Object.values(state.bundle.project.pages);
  const panels = Object.values(state.bundle.project.panels);
  const proposals = Object.values(state.bundle.project.proposals);
  const proposed = proposals.some((proposal) => proposal.status === 'proposed');
  const painted = panels.filter((panel) => panel.artworkRefs.length > 0).length;
  let step = 'idea';
  let hint = 'Describe an idea, then let the comic director turn it into a storyboard.';
  if (proposed) {
    step = 'review';
    hint = 'Review the designed storyboard; accept all, or accept it one scene at a time.';
  } else if (pages.length > 0 && painted === 0) {
    step = 'edit';
    hint = 'Edit the pages and dialogue, then draw artwork for the panels.';
  } else if (pages.length > 0 && painted < panels.length) {
    step = 'art';
    hint = `${painted} of ${panels.length} panels have artwork. Draw the rest, or submit a server AI job.`;
  } else if (pages.length > 0) {
    step = 'publish';
    hint = 'Artwork is in place. Validate for print, then export from the server or package the project.';
  }
  const order = ['idea', 'design', 'review', 'edit', 'art', 'publish'];
  const active = order.indexOf(step);
  for (const node of el('workflow').querySelectorAll('.workflow-step')) {
    const name = /** @type {HTMLElement} */ (node).dataset.step ?? '';
    const index = order.indexOf(name);
    node.classList.toggle('active', name === step);
    node.classList.toggle('done', index !== -1 && index < active);
  }
  el('workflowHint').textContent = hint;
}

/**
 * The idea form. One dialog collects the brief and the production options; the command does
 * the design and stores an inert proposal.
 * @returns {Promise<null | { title: string, ideas: string, style: string, audience: string, characters: string[], maxPanelsPerPage: number }>}
 */
function askDesign() {
  return new Promise((resolve) => {
    const dialog = document.createElement('dialog');
    dialog.className = 'dialog design-dialog';
    dialog.innerHTML = `
      <form method="dialog">
        <h2>Design a comic</h2>
        <p class="muted small">Give the director an idea, a synopsis or a manuscript. It returns a structured storyboard you review before anything is accepted.</p>
        <label class="field"><span>Title</span><input id="designTitle" type="text" maxlength="200" placeholder="The Long Night"></label>
        <label class="field"><span>Story or idea</span><textarea id="designIdeas" rows="7" placeholder="A rooftop at night. Maya sees a shadow move behind her, and the city goes dark..."></textarea></label>
        <div class="grid-fields">
          <label class="field"><span>Style</span>
            <select id="designStyle">
              <option>black ink</option><option>manga</option><option>noir</option><option>watercolour</option><option>retro</option>
            </select>
          </label>
          <label class="field"><span>Audience</span>
            <select id="designAudience"><option>all ages</option><option>teen</option><option>mature</option></select>
          </label>
          <label class="field"><span>Panels per page</span>
            <select id="designPanels"><option>2</option><option selected>3</option><option>4</option><option>6</option></select>
          </label>
        </div>
        <label class="field"><span>Characters (one per line, optional)</span><textarea id="designCharacters" rows="3" placeholder="Maya: short black hair, red windbreaker&#10;Jonas: tall, grey coat"></textarea></label>
        <div class="dialog-actions">
          <button value="cancel" class="ghost" type="submit">Cancel</button>
          <button value="ok" class="primary" type="submit">Design storyboard</button>
        </div>
      </form>`;
    document.body.append(dialog);
    dialog.addEventListener('close', () => {
      const ok = dialog.returnValue === 'ok';
      const value = ok ? {
        title: /** @type {HTMLInputElement} */ (dialog.querySelector('#designTitle')).value.trim(),
        ideas: /** @type {HTMLTextAreaElement} */ (dialog.querySelector('#designIdeas')).value.trim(),
        style: /** @type {HTMLSelectElement} */ (dialog.querySelector('#designStyle')).value,
        audience: /** @type {HTMLSelectElement} */ (dialog.querySelector('#designAudience')).value,
        characters: /** @type {HTMLTextAreaElement} */ (dialog.querySelector('#designCharacters')).value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean),
        maxPanelsPerPage: Number(/** @type {HTMLSelectElement} */ (dialog.querySelector('#designPanels')).value),
      } : null;
      dialog.remove();
      resolve(value && value.ideas.length > 0 ? value : null);
    });
    dialog.showModal();
    dialog.querySelector('#designIdeas')?.focus();
  });
}

/* ------------------------------------------------------------------ render */

function render() {
  const page = currentPage();
  const artboard = page ? state.bundle.document.artboards.find((a) => a.id === page.artboardId) : null;
  el('projectTitle').textContent = state.bundle.project.title;

  if (artboard) {
    canvas.width = Math.round(artboard.size.width * state.scale);
    canvas.height = Math.round(artboard.size.height * state.scale);
  } else {
    canvas.width = 640;
    canvas.height = 400;
  }

  if (page) {
    drawPage(ctx, state.bundle, page, measure, { scale: state.scale, selectedId: state.selectedId });
  } else {
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = '#64748b';
    ctx.font = '14px system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('No page yet — add one to begin.', canvas.width / 2, 48);
    ctx.textAlign = 'start';
  }

  renderWorkflow();
  renderStoryNav();
  renderPageList();
  renderCharacters();
  renderAssets();
  renderInspector();
  renderTypography();
  renderReadingOrder();
  renderProblems();
  renderReview();
  if (state.view === 'storyboard') renderStoryboard();
  el('undoBtn').toggleAttribute('disabled', !history.canUndo());
  el('redoBtn').toggleAttribute('disabled', !history.canRedo());
  el('zoomLabel').textContent = `${Math.round(state.scale * 100)}%`;
  renderSelectionState();
}

function renderSelectionState() {
  const node = selectedNode();
  if (!node) {
    el('selectionState').textContent = 'No selection';
    el('selectionState').className = 'pill';
    return;
  }
  const panel = panelOfNode(node.id);
  const lettering = letteringOfNode(node.id);
  const kind = lettering ? `${lettering.kind}${lettering.variant ? `/${lettering.variant}` : ''}` : panel ? 'panel' : node.kind;
  const locked = panel?.locked ? ' · locked' : '';
  const problems = validateBundle(state.bundle, { fonts: browserFonts(), measure, checkOverflow: true }).length;
  el('selectionState').textContent = `${kind}${locked}`;
  el('selectionState').className = `pill ${panel?.locked ? 'locked' : ''} ${problems > 0 ? 'has-warnings' : ''}`;
}

function renderStoryNav() {
  const box = el('storyNav');
  const chapters = Object.values(state.bundle.project.chapters);
  if (chapters.length === 0) {
    box.innerHTML = '<p class="muted small">No chapters yet.</p>';
    return;
  }
  const parts = [];
  for (const chapter of chapters) {
    const c = /** @type {any} */ (chapter);
    parts.push(`<div class="story-chapter"><span class="story-title">${esc(c.title)}</span>`);
    if (c.sceneIds.length === 0) parts.push('<p class="muted small">No scenes.</p>');
    for (const sceneId of c.sceneIds) {
      const scene = state.bundle.project.scenes[sceneId];
      if (!scene) continue;
      parts.push(`<div class="story-scene"><span class="story-title">${esc(scene.name)}</span>`);
      for (const pageId of scene.pageIds) {
        const page = state.bundle.project.pages[pageId];
        if (!page) continue;
        parts.push(`<button type="button" class="story-page ${pageId === state.pageId ? 'current' : ''}" data-page="${esc(pageId)}">${esc(page.name)} <span class="count">${page.panelIds.length}▭</span></button>`);
      }
      parts.push('</div>');
    }
    parts.push('</div>');
  }
  box.innerHTML = parts.join('');
  for (const button of box.querySelectorAll('[data-page]')) {
    button.addEventListener('click', () => {
      state.pageId = /** @type {HTMLElement} */ (button).dataset.page ?? null;
      state.selectedId = null;
      setView('canvas');
    });
  }
}

function renderPageList() {
  const list = el('pageList');
  list.textContent = '';
  const pages = orderedPages(state.bundle);
  if (pages.length === 0) {
    list.innerHTML = '<li class="muted">No pages.</li>';
    return;
  }
  for (const page of pages) {
    const li = document.createElement('li');
    li.setAttribute('aria-current', String(page.id === state.pageId));
    const status = page.status ?? 'draft';
    ioSet(li, `<span>${esc(page.name)}</span><span class="count">${page.panelIds.length}▭ · ${esc(status)}</span>`);
    li.addEventListener('click', () => { state.pageId = page.id; state.selectedId = null; setView('canvas'); });
    list.append(li);
  }
}

/** `innerHTML` is used with escaped text; this wrapper makes that intent obvious at each site. */
function ioSet(node, html) { node.innerHTML = html; }

function renderCharacters() {
  const list = el('characterList');
  list.textContent = '';
  const characters = Object.values(state.bundle.project.characters);
  if (characters.length === 0) {
    list.innerHTML = '<li class="muted">None yet.</li>';
    return;
  }
  for (const c of characters) {
    const record = /** @type {any} */ (c);
    const li = document.createElement('li');
    li.className = 'character';
    ioSet(li, `<span>${esc(record.name)}</span>${record.approvedDesignVersion ? '<span class="badge approved">approved</span>' : ''}`);
    list.append(li);
  }
}

function renderAssets() {
  const list = el('assetList');
  const assets = Object.values(state.bundle.project.assets);
  el('assetCount').textContent = String(assets.length);
  list.textContent = '';
  if (assets.length === 0) {
    list.innerHTML = '<li class="muted">No assets.</li>';
    return;
  }
  for (const asset of assets.slice(-12).reverse()) {
    const a = /** @type {any} */ (asset);
    const li = document.createElement('li');
    li.className = 'asset';
    li.classList.toggle('current', state.selectedAssetId === a.id);
    ioSet(li, `<span class="asset-name">${esc(a.name)}</span><span class="badge ${esc(a.approvalStatus ?? 'approved')}">${esc(a.approvalStatus ?? 'approved')}</span>`);
    li.addEventListener('click', () => {
      state.selectedAssetId = a.id;
      toast(`Selected asset ${a.name}`);
      render();
    });
    list.append(li);
  }
}

function renderInspector() {
  const box = el('inspector');
  box.textContent = '';
  const node = selectedNode();
  if (!node) {
    box.innerHTML = '<p class="muted">Nothing selected.</p>';
    return;
  }
  const panel = panelOfNode(node.id);
  const lettering = letteringOfNode(node.id);
  const rows = [
    ['Kind', lettering ? `${lettering.kind}${lettering.variant ? ` (${lettering.variant})` : ''}` : panel ? 'panel' : node.kind],
    ['Name', node.name],
  ];
  if (panel) rows.push(['Panel id', panel.id], ['Locked', panel.locked ? 'yes' : 'no']);
  if (lettering) rows.push(['Lettering id', lettering.id], ['Policy', lettering.overflowPolicy ?? 'warn']);
  if (node.kind === 'image' && node.crop) rows.push(['Crop', `${Math.round(node.crop.width * 100)}% × ${Math.round(node.crop.height * 100)}%`]);
  box.innerHTML = rows.map(([label, value]) => `<div class="row"><span>${esc(label)}</span><span>${esc(value)}</span></div>`).join('');

  const f = node.frame;
  box.insertAdjacentHTML('beforeend', `
    <div class="grid-fields">
      <label class="field"><span>X</span><input id="frameX" type="number" step="1" value="${Math.round(f.x)}"></label>
      <label class="field"><span>Y</span><input id="frameY" type="number" step="1" value="${Math.round(f.y)}"></label>
      <label class="field"><span>Width</span><input id="frameW" type="number" min="1" step="1" value="${Math.round(f.width)}"></label>
      <label class="field"><span>Height</span><input id="frameH" type="number" min="1" step="1" value="${Math.round(f.height)}"></label>
    </div>
    <button id="applyFrameBtn" class="wide" type="button">Apply frame</button>
  `);
  el('applyFrameBtn').addEventListener('click', async () => {
    const frame = {
      x: Number(/** @type {HTMLInputElement} */ (el('frameX')).value),
      y: Number(/** @type {HTMLInputElement} */ (el('frameY')).value),
      width: Number(/** @type {HTMLInputElement} */ (el('frameW')).value),
      height: Number(/** @type {HTMLInputElement} */ (el('frameH')).value),
    };
    if (!Number.isFinite(frame.x) || !Number.isFinite(frame.y) || !(frame.width > 0) || !(frame.height > 0)) {
      toast('Frame values must be finite and the size positive.', true);
      return;
    }
    if (panel) await run('comic.panel.setFrame', { panelId: panel.id, ...frame });
    else if (lettering) await run('comic.lettering.setFrame', { letteringId: lettering.id, ...frame });
    else toast('This object has no frame command in the browser.', true);
    render();
  });
}

function renderTypography() {
  const box = el('typography');
  const lettering = letteringOfNode(state.selectedId);
  if (!lettering) {
    box.innerHTML = '<p class="muted">Select a balloon, caption or sound effect.</p>';
    return;
  }
  const node = state.bundle.document.nodes[String(lettering.nodeId)];
  const style = {
    family: lettering.fontFamily ?? state.bundle.project.defaultLetteringStyle.family,
    sizePt: lettering.fontSize ?? node?.fontSize ?? state.bundle.project.defaultLetteringStyle.sizePt,
    align: lettering.align ?? node?.align ?? 'left',
    padding: lettering.padding ?? 8,
  };
  const families = catalogue().map((face) => face.name);
  box.innerHTML = `
    <label class="field"><span>Text</span><textarea id="letteringText" rows="3">${esc(lettering.text ?? '')}</textarea></label>
    <label class="field"><span>Face</span><select id="letteringFamily">${families.map((name) => `<option ${name === style.family ? 'selected' : ''}>${esc(name)}</option>`).join('')}</select></label>
    <div class="grid-fields">
      <label class="field"><span>Size</span><input id="letteringSize" type="number" min="4" max="96" step="1" value="${Math.round(style.sizePt)}"></label>
      <label class="field"><span>Align</span><select id="letteringAlign">
        ${['left', 'center', 'right'].map((value) => `<option ${value === style.align ? 'selected' : ''}>${value}</option>`).join('')}
      </select></label>
    </div>
    ${lettering.kind === 'balloon' ? `<label class="field"><span>Variant</span><select id="letteringVariant">
      ${['speech', 'thought', 'shout', 'whisper'].map((value) => `<option ${value === (lettering.variant ?? 'speech') ? 'selected' : ''}>${value}</option>`).join('')}
    </select></label>` : ''}
    <label class="field"><span>Overflow</span><select id="letteringPolicy">
      ${['warn', 'shrink', 'resize-frame'].map((value) => `<option ${value === (lettering.overflowPolicy ?? 'warn') ? 'selected' : ''}>${value}</option>`).join('')}
    </select></label>
    <button id="applyTypographyBtn" class="wide" type="button">Apply typography</button>
  `;
  el('applyTypographyBtn').addEventListener('click', async () => {
    const text = /** @type {HTMLTextAreaElement} */ (el('letteringText')).value;
    const family = /** @type {HTMLSelectElement} */ (el('letteringFamily')).value;
    const sizePt = Number(/** @type {HTMLInputElement} */ (el('letteringSize')).value);
    const align = /** @type {HTMLSelectElement} */ (el('letteringAlign')).value;
    const policy = /** @type {HTMLSelectElement} */ (el('letteringPolicy')).value;
    const variant = /** @type {HTMLSelectElement|null} */ (document.getElementById('letteringVariant'));
    // One user action is one command, so it is one undo step.
    const result = await run('comic.lettering.edit', {
      letteringId: lettering.id,
      text, family, sizePt, align, policy,
      ...(variant ? { variant: variant.value } : {}),
    });
    if (result.ok) render();
  });
}

function renderReadingOrder() {
  const list = el('readingOrder');
  list.textContent = '';
  const page = currentPage();
  if (!page || page.panelIds.length === 0) {
    list.innerHTML = '<li class="muted">No panels.</li>';
    return;
  }
  page.panelIds.forEach((panelId, index) => {
    const panel = state.bundle.project.panels[panelId];
    const node = panel ? state.bundle.document.nodes[String(panel.frameNodeId)] : null;
    const li = document.createElement('li');
    li.classList.toggle('current', node?.id === state.selectedId);
    li.classList.toggle('locked', panel?.locked === true);
    ioSet(li, `<span>${index + 1}. ${esc(panelId)}${panel?.locked ? ' 🔒' : ''}</span><span class="count">${node ? `${Math.round(node.frame.width)}×${Math.round(node.frame.height)}` : '—'}</span>`);
    li.addEventListener('click', () => { state.selectedId = node?.id ?? null; render(); });
    const up = document.createElement('button');
    up.type = 'button'; up.className = 'tiny'; up.textContent = '↑';
    up.addEventListener('click', async (event) => {
      event.stopPropagation();
      if (index === 0) return;
      const order = [...page.panelIds];
      [order[index - 1], order[index]] = [order[index], order[index - 1]];
      await run('comic.reading.set', { pageId: page.id, order });
      render();
    });
    const down = document.createElement('button');
    down.type = 'button'; down.className = 'tiny'; down.textContent = '↓';
    down.addEventListener('click', async (event) => {
      event.stopPropagation();
      if (index === page.panelIds.length - 1) return;
      const order = [...page.panelIds];
      [order[index + 1], order[index]] = [order[index], order[index + 1]];
      await run('comic.reading.set', { pageId: page.id, order });
      render();
    });
    li.append(up, down);
    list.append(li);
  });
}

function renderProblems() {
  const box = el('problems');
  const problems = validateBundle(state.bundle, { fonts: browserFonts(), measure, checkOverflow: true });
  const errors = errorsOf(problems);
  const warnings = warningsOf(problems);
  if (problems.length === 0) {
    box.innerHTML = '<p class="muted">No problems found.</p>';
    return;
  }
  box.innerHTML = [...errors, ...warnings].slice(0, 20).map((problem) =>
    `<div class="problem ${problem.severity}">${esc(problem.message)}<br><code>${esc(problem.code)}</code></div>`,
  ).join('');
}

function renderReview() {
  renderProposals();
  renderJobs();
  renderAwaitingReview();
  const problems = validateBundle(state.bundle, { fonts: browserFonts(), measure, checkOverflow: true });
  const box = el('reviewProblems');
  box.innerHTML = problems.length === 0
    ? '<p class="muted">No findings.</p>'
    : problems.slice(0, 30).map((p) => `<div class="problem ${p.severity}">${esc(p.message)}<br><code>${esc(p.code)}</code></div>`).join('');

  const publication = state.publication;
  if (publication) {
    el('publication').innerHTML = `
      <p class="${publication.ready ? 'ok' : 'bad'}">${publication.ready ? 'Ready to export' : `${publication.errors} error(s) block export`}</p>
      ${publication.problems.slice(0, 20).map((p) => `<div class="problem ${p.severity}">${esc(p.message)}<br><code>${esc(p.code)}</code></div>`).join('')}
      <p class="muted small">Export is a server-side command; this browser can edit and save the bundle locally.</p>
    `;
  }
}

function renderProposals() {
  const proposals = Object.values(state.bundle.project.proposals);
  el('proposalCount').textContent = String(proposals.length);
  const box = el('proposalList');
  if (proposals.length === 0) {
    box.innerHTML = '<p class="muted">No story proposals. Import a story from the Story panel.</p>';
    return;
  }
  box.innerHTML = proposals.map((proposal) => `
    <article class="card ${esc(proposal.status)}">
      <header><strong>${esc(proposal.title)}</strong><span class="badge ${esc(proposal.status)}">${esc(proposal.status)}</span></header>
      <p class="small">${esc(proposal.logline || '')}</p>
      <p class="muted small">${esc(proposal.style || 'black ink')} · ${esc(proposal.audience || 'all ages')} · ${proposal.scenes.length} scene(s) · ${proposal.scenes.reduce((n, scene) => n + scene.pages.length, 0)} page(s) · revision ${proposal.baseRevision}</p>
      <p class="muted small">Designed by ${esc(proposal.design?.provider || 'local-director')} / ${esc(proposal.design?.model || 'deterministic-storyboard-v1')}</p>
      <ol class="proposal-scenes">
        ${proposal.scenes.map((scene) => {
          const taken = (proposal.acceptedSceneKeys ?? []).includes(scene.key);
          return `<li><span>${esc(scene.name)}</span><span class="muted small">${scene.pages.length} page(s)</span>${taken
            ? '<span class="badge approved">accepted</span>'
            : proposal.status === 'proposed'
              ? `<button type="button" class="tiny" data-proposal="${esc(proposal.id)}" data-scene="${esc(scene.key)}">Accept scene</button>`
              : ''}</li>`;
        }).join('')}
      </ol>
      <div class="card-actions">
        ${proposal.status === 'proposed' ? `<button type="button" data-accept="${esc(proposal.id)}">Accept all</button><button type="button" class="ghost" data-reject="${esc(proposal.id)}">Reject</button>` : ''}
      </div>
    </article>
  `).join('');
  for (const button of box.querySelectorAll('[data-accept]')) {
    button.addEventListener('click', async () => {
      const result = await run('comic.storyboard.accept', { proposalId: /** @type {HTMLElement} */ (button).dataset.accept });
      if (result.ok) {
        toast(`Accepted ${result.value.pages.length} page(s). ${result.value.remainingSceneKeys.length > 0 ? `${result.value.remainingSceneKeys.length} scene(s) still await review.` : 'Edit, then draw artwork.'}`);
        if (result.value.pages[0]) state.pageId = result.value.pages[0];
        state.selectedId = null;
        if (result.value.remainingSceneKeys.length > 0) setView('review'); else setView('canvas');
      } else render();
    });
  }
  for (const button of box.querySelectorAll('[data-scene]')) {
    button.addEventListener('click', async () => {
      const node = /** @type {HTMLElement} */ (button);
      const result = await run('comic.storyboard.accept', {
        proposalId: node.dataset.proposal, sceneKeys: [node.dataset.scene],
      });
      if (result.ok) {
        toast(`Accepted ${result.value.pages.length} page(s) from that scene. ${result.value.remainingSceneKeys.length > 0 ? `${result.value.remainingSceneKeys.length} scene(s) still await review.` : 'Edit, then draw artwork.'}`);
        if (result.value.pages[0]) state.pageId = result.value.pages[0];
        state.selectedId = null;
        if (result.value.remainingSceneKeys.length > 0) setView('review'); else setView('canvas');
      } else render();
    });
  }
  for (const button of box.querySelectorAll('[data-reject]')) {
    button.addEventListener('click', async () => {
      await run('comic.storyboard.reject', { proposalId: /** @type {HTMLElement} */ (button).dataset.reject });
      render();
    });
  }
}

function renderJobs() {
  const jobs = Object.values(state.bundle.project.jobs);
  el('jobCount').textContent = String(jobs.length);
  const box = el('jobList');
  if (jobs.length === 0) {
    box.innerHTML = '<p class="muted">No generation jobs.</p>';
    return;
  }
  box.innerHTML = jobs.slice().reverse().slice(0, 12).map((job) => `
    <article class="card job ${esc(job.status)}">
      <header><strong>${esc(job.operation)}</strong><span class="badge ${esc(job.status)}">${esc(job.status)}</span></header>
      <p class="muted small">${esc(job.provider)} / ${esc(job.model)} · target revision ${job.targetRevision}${job.targetPanelId ? ` · ${esc(job.targetPanelId)}` : ''}</p>
      ${job.error ? `<p class="bad">${esc(job.error.message)}</p>` : ''}
      ${job.status === 'queued' || job.status === 'running' ? `<button type="button" data-cancel="${esc(job.id)}">Cancel</button>` : ''}
      ${job.status === 'awaiting_review' ? `
        <div class="card-actions">
          <button type="button" data-apply="${esc(job.id)}">Apply</button>
          <button type="button" class="ghost" data-rejectjob="${esc(job.id)}">Reject</button>
        </div>` : ''}
    </article>
  `).join('');
  for (const button of box.querySelectorAll('[data-apply]')) {
    button.addEventListener('click', async () => {
      await run('comic.job.apply', { jobId: /** @type {HTMLElement} */ (button).dataset.apply });
      render();
    });
  }
  for (const button of box.querySelectorAll('[data-rejectjob]')) {
    button.addEventListener('click', async () => {
      await run('comic.job.reject', { jobId: /** @type {HTMLElement} */ (button).dataset.rejectjob });
      render();
    });
  }
  for (const button of box.querySelectorAll('[data-cancel]')) {
    button.addEventListener('click', async () => {
      await run('comic.job.cancel', { jobId: /** @type {HTMLElement} */ (button).dataset.cancel });
      render();
    });
  }
  const pending = jobs.some((job) => job.status === 'queued' || job.status === 'running');
  ensureJobPoll(pending);
}

function renderAwaitingReview() {
  const assets = Object.values(state.bundle.project.assets).filter((asset) => asset.approvalStatus === 'pending');
  el('awaitingCount').textContent = String(assets.length);
  const box = el('reviewQueue');
  if (assets.length === 0) {
    box.innerHTML = '<p class="muted">Nothing awaiting review.</p>';
    return;
  }
  box.innerHTML = assets.map((asset) => `
    <article class="card pending">
      <header><strong>${esc(asset.name)}</strong><span class="badge pending">pending</span></header>
      <p class="muted small">${esc(asset.provenance?.provider ?? 'unknown')} / ${esc(asset.provenance?.model ?? 'unknown')}</p>
      <div class="card-actions">
        <button type="button" data-approve="${esc(asset.id)}">Approve</button>
        <button type="button" class="ghost" data-rejectasset="${esc(asset.id)}">Reject</button>
      </div>
    </article>
  `).join('');
  for (const button of box.querySelectorAll('[data-approve]')) {
    button.addEventListener('click', async () => {
      await run('comic.asset.approve', { assetId: /** @type {HTMLElement} */ (button).dataset.approve });
      render();
    });
  }
  for (const button of box.querySelectorAll('[data-rejectasset]')) {
    button.addEventListener('click', async () => {
      await run('comic.asset.reject', { assetId: /** @type {HTMLElement} */ (button).dataset.rejectasset });
      render();
    });
  }
}

let jobTimer = /** @type {any} */ (null);
/** @param {boolean} active */
function ensureJobPoll(active) {
  if (active && !jobTimer) {
    jobTimer = setInterval(() => { renderJobs(); }, 1500);
  } else if (!active && jobTimer) {
    clearInterval(jobTimer);
    jobTimer = null;
  }
}

function renderStoryboard() {
  const box = el('storyboard');
  const pages = orderedPages(state.bundle);
  if (pages.length === 0) {
    box.innerHTML = '<p class="muted">No pages yet.</p>';
    return;
  }
  box.textContent = '';
  for (const page of pages) {
    const article = document.createElement('article');
    article.className = 'storyboard-page';
    article.dataset.page = page.id;
    const thumb = document.createElement('canvas');
    const artboard = state.bundle.document.artboards.find((a) => a.id === page.artboardId);
    const width = 220;
    const scale = artboard ? width / artboard.size.width : 0.2;
    thumb.width = width;
    thumb.height = artboard ? Math.round(artboard.size.height * scale) : 140;
    const thumbCtx = thumb.getContext('2d');
    if (thumbCtx && artboard) drawPage(thumbCtx, state.bundle, page, measure, { scale, selectedId: null, showHandles: false });
    const caption = document.createElement('div');
    caption.className = 'storyboard-caption';
    ioSet(caption, `<strong>${esc(page.name)}</strong><span class="muted small">${page.panelIds.length} panel(s)</span>`);
    const sequence = document.createElement('ol');
    sequence.className = 'sequence';
    for (const panelId of page.panelIds) {
      const panel = state.bundle.project.panels[panelId];
      if (!panel) continue;
      const li = document.createElement('li');
      const lettering = panel.letteringIds.map((id) => state.bundle.project.lettering[id]?.text ?? '').filter(Boolean);
      ioSet(li, `<span class="small">${esc(panel.shotDescription ?? panel.camera ?? 'Panel')}</span>${lettering.length ? `<span class="muted small">“${esc(lettering[0])}”</span>` : ''}`);
      sequence.append(li);
    }
    article.append(thumb, caption, sequence);
    article.addEventListener('click', () => {
      state.pageId = page.id;
      state.selectedId = null;
      setView('canvas');
    });
    box.append(article);
  }
}

/* ------------------------------------------------------------------ interaction */

let suppressClick = false;
/**
 * The object a click should actually select. Artwork covers its panel and a tail is part of
 * its balloon; selecting the image or the 12x12 tail would make the panel tools unreachable.
 * @param {any} node
 * @returns {number|null}
 */
function selectableId(node) {
  if (!node) return null;
  const lettering = Object.values(state.bundle.project.lettering)
    .find((/** @type {any} */ l) => l.nodeId === node.id || l.tailNodeId === node.id);
  if (lettering) return lettering.nodeId;
  const panel = Object.values(state.bundle.project.panels)
    .find((/** @type {any} */ p) => p.artworkRefs.map(Number).includes(node.id));
  if (panel) return panel.frameNodeId;
  return node.id;
}

canvas.addEventListener('click', (event) => {
  if (suppressClick) { suppressClick = false; return; }
  const page = currentPage();
  if (!page) return;
  const at = toPageUnits(canvas, event, state.scale);
  const hit = nodeAt(state.bundle, page, at);
  state.selectedId = selectableId(hit);
  render();
});

let dragging = null;
canvas.addEventListener('pointerdown', (event) => {
  const page = currentPage();
  if (!page || state.selectedId === null) return;
  const node = state.bundle.document.nodes[String(state.selectedId)];
  if (!node) return;
  const panel = panelOfNode(node.id);
  if (panel?.locked) { toast('This panel is locked.', true); return; }
  const lettering = Object.values(state.bundle.project.lettering)
    .find((/** @type {any} */ l) => l.nodeId === node.id || l.tailNodeId === node.id);
  const at = toPageUnits(canvas, event, state.scale);
  dragging = {
    node,
    panel: panel?.id ?? null,
    lettering: lettering?.id ?? null,
    start: at,
    last: at,
    origin: { x: node.frame.x, y: node.frame.y },
    before: { ...node.frame },
    moved: false,
  };
  canvas.setPointerCapture(event.pointerId);
});

canvas.addEventListener('pointermove', (event) => {
  if (!dragging) return;
  const at = toPageUnits(canvas, event, state.scale);
  const dx = at.x - dragging.start.x;
  const dy = at.y - dragging.start.y;
  if (Math.abs(dx) < 1 && Math.abs(dy) < 1) return;
  dragging.last = at;
  dragging.moved = true;
  // Preview by moving the object directly; the bundle is not committed until pointerup, so a
  // drag is exactly one undoable command.
  dragging.node.frame = {
    x: dragging.before.x + dx,
    y: dragging.before.y + dy,
    width: dragging.before.width,
    height: dragging.before.height,
  };
  render();
});

canvas.addEventListener('pointerup', async (event) => {
  if (!dragging) return;
  const drag = dragging;
  dragging = null;
  if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
  if (!drag.moved) {
    drag.node.frame = drag.before;
    render();
    return;
  }
  suppressClick = true;
  const at = toPageUnits(canvas, event, state.scale);
  const dx = at.x - drag.start.x;
  const dy = at.y - drag.start.y;
  const x = drag.before.x + dx;
  const y = drag.before.y + dy;
  drag.node.frame = drag.before;
  if (drag.lettering) {
    // A lettering drag is `move`, which keeps the tail attached and preserves its length.
    await run('comic.lettering.move', { letteringId: drag.lettering, dx, dy });
  } else if (drag.panel) {
    await run('comic.panel.setFrame', {
      panelId: drag.panel, x, y, width: drag.before.width, height: drag.before.height,
    });
  }
  render();
});

canvas.addEventListener('pointercancel', (event) => {
  if (!dragging) return;
  dragging.node.frame = dragging.before;
  dragging = null;
  if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
  render();
});

/* ------------------------------------------------------------------ controls */

el('undoBtn').addEventListener('click', async () => { await run('comic.edit.undo'); render(); });
el('redoBtn').addEventListener('click', async () => { await run('comic.edit.redo'); render(); });
el('viewCanvasBtn').addEventListener('click', () => setView('canvas'));
el('viewStoryboardBtn').addEventListener('click', () => setView('storyboard'));
el('viewReviewBtn').addEventListener('click', () => setView('review'));

async function designComic() {
  const design = await askDesign();
  if (!design) return;
  const result = await run('comic.story.design', design);
  if (result.ok) {
    // Carry the chosen visual direction into the local drawing tool.
    const style = /** @type {HTMLSelectElement} */ (el('artStyle'));
    if ([...style.options].some((option) => option.value === result.value.design.style)) style.value = result.value.design.style;
    toast(`Storyboard designed: ${result.value.outline.scenes.length} scene(s), ${result.value.outline.scenes.reduce((n, scene) => n + scene.pages.length, 0)} page(s).`);
    setView('review');
  }
}

el('designBtn').addEventListener('click', () => { void designComic(); });

el('newBtn').addEventListener('click', async () => {
  const title = await askText({ title: 'New comic', value: 'Untitled Comic', action: 'Create' });
  if (!title) return;
  await run('comic.project.create', { title });
  state.pageId = null;
  state.selectedId = null;
  state.src = null;
  render();
});

el('openBtn').addEventListener('click', async () => {
  try {
    const saved = await listProjects();
    if (saved.length === 0) { toast('No locally saved projects.', true); return; }
    const choice = await askChoice({ title: 'Open a locally saved comic', options: saved, value: saved[saved.length - 1] });
    if (!choice) return;
    const files = await loadBundle(choice);
    const parsed = deserialize(files);
    if (!parsed.ok) { toast('That project could not be reopened.', true); return; }
    const { bundle } = migrate(parsed.bundle);
    adoptBundle(bundle);
    state.src = choice;
    toast(`Reopened ${bundle.project.title}`);
    render();
  } catch (error) {
    toast(`Local storage unavailable: ${/** @type {Error} */ (error).message}`, true);
  }
});

el('addChapterBtn').addEventListener('click', async () => {
  const title = await askText({ title: 'New chapter', value: `Chapter ${Object.keys(state.bundle.project.chapters).length + 1}`, action: 'Add' });
  if (!title) return;
  await run('comic.chapter.add', { title });
  render();
});

el('addSceneBtn').addEventListener('click', async () => {
  const chapterId = Object.keys(state.bundle.project.chapters)[0];
  if (!chapterId) { toast('Add a chapter first.', true); return; }
  const name = await askText({ title: 'New scene', value: `Scene ${Object.keys(state.bundle.project.scenes).length + 1}`, action: 'Add' });
  if (!name) return;
  await run('comic.scene.add', { chapterId, name });
  render();
});

el('addPageBtn').addEventListener('click', async () => {
  const template = /** @type {HTMLSelectElement} */ (el('templateSelect')).value || undefined;
  const index = Object.keys(state.bundle.project.pages).length + 1;
  const result = await run('comic.page.add', { name: `Page ${index}`, ...(template ? { template } : {}) });
  if (result.ok && result.value?.pageId) state.pageId = result.value.pageId;
  render();
});

el('addCharacterBtn').addEventListener('click', async () => {
  const name = await askText({ title: 'New character', label: 'A name is enough; the bible fills in detail later.', action: 'Add' });
  if (!name) return;
  await run('comic.character.add', { name });
  render();
});

el('uploadAssetBtn').addEventListener('click', () => /** @type {HTMLInputElement} */ (el('assetFile')).click());
el('assetFile').addEventListener('change', async (event) => {
  const input = /** @type {HTMLInputElement} */ (event.target);
  const file = input.files?.[0];
  if (!file) return;
  const bytes = new Uint8Array(await file.arrayBuffer());
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  const hash = [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
  const data = await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error('The file could not be read'));
    reader.readAsDataURL(file);
  });
  await run('comic.asset.register', {
    name: file.name, contentHash: hash, data, assetType: 'source', mime: file.type || 'application/octet-stream',
  });
  input.value = '';
  render();
});

el('panelAddBtn').addEventListener('click', async () => {
  const page = currentPage();
  if (!page) { toast('Add a page first.', true); return; }
  const artboard = state.bundle.document.artboards.find((a) => a.id === page.artboardId);
  if (!artboard) return;
  const count = page.panelIds.length;
  const width = Math.round(artboard.size.width - 80);
  const height = Math.round((artboard.size.height - 80) / 4);
  const y = 40 + (count % 4) * (height + 16);
  const result = await run('comic.panel.add', {
    pageId: page.id, x: 40, y: Math.min(y, artboard.size.height - height - 40), width, height,
  });
  if (result.ok && result.value?.frameNodeId) state.selectedId = result.value.frameNodeId;
  render();
});

/**
 * @param {'balloon'|'thought'|'caption'|'sound-effect'} kind
 * @param {'speech'|'thought'} [variant]
 */
async function addLettering(kind, variant) {
  const page = currentPage();
  if (!page) { toast('Add a page first.', true); return; }
  const selected = panelOfNode(state.selectedId);
  const panelId = selected?.id ?? page.panelIds[0];
  if (!panelId) { toast('Add a panel first.', true); return; }
  const label = kind === 'caption' ? 'Caption text' : kind === 'sound-effect' ? 'Sound effect' : 'Dialogue';
  const text = await askText({
    title: `Add ${kind}`,
    label,
    value: kind === 'sound-effect' ? 'KRAKOOM!' : 'A line of dialogue',
    multiline: kind !== 'sound-effect',
    action: 'Add',
  });
  if (!text) return;
  const panel = state.bundle.project.panels[panelId];
  const frameNode = state.bundle.document.nodes[String(panel.frameNodeId)];
  const frame = frameNode.frame;
  const result = await run('comic.lettering.add', {
    panelId, kind, text,
    ...(variant ? { variant } : {}),
    x: Math.round(frame.x + frame.width * 0.18),
    y: Math.round(frame.y + frame.height * 0.55),
    width: Math.round(frame.width * 0.64),
    height: Math.round(Math.max(50, frame.height * 0.2)),
    overflowPolicy: 'shrink',
    ...(kind === 'balloon' ? { tailToX: Math.round(frame.x + frame.width * 0.5), tailToY: Math.round(frame.y + frame.height * 0.95) } : {}),
  });
  if (result.ok && result.value?.nodeId) state.selectedId = result.value.nodeId;
  render();
}

el('balloonAddBtn').addEventListener('click', () => addLettering('balloon', 'speech'));
el('thoughtAddBtn').addEventListener('click', () => addLettering('balloon', 'thought'));
el('captionAddBtn').addEventListener('click', () => addLettering('caption'));
el('sfxAddBtn').addEventListener('click', () => addLettering('sound-effect'));

el('lockBtn').addEventListener('click', async () => {
  const panel = panelOfNode(state.selectedId);
  if (!panel) { toast('Select a panel first.', true); return; }
  await run('comic.panel.setLocked', { panelId: panel.id, locked: !panel.locked });
  render();
});

el('deleteBtn').addEventListener('click', async () => {
  const lettering = letteringOfNode(state.selectedId);
  const panel = panelOfNode(state.selectedId);
  if (lettering) await run('comic.lettering.delete', { letteringId: lettering.id });
  else if (panel) await run('comic.panel.delete', { panelId: panel.id });
  else { toast('Select a panel or lettering first.', true); return; }
  state.selectedId = null;
  render();
});

el('generateBtn').addEventListener('click', async () => {
  const prompt = /** @type {HTMLTextAreaElement} */ (el('generatePrompt')).value.trim();
  if (!prompt) { toast('Write a prompt first.', true); return; }
  const panel = panelOfNode(state.selectedId) ?? (currentPage()?.panelIds[0] ? state.bundle.project.panels[currentPage()?.panelIds[0]] : null);
  if (!panel) { toast('Add a panel first.', true); return; }
  const result = await run('comic.job.submit', { operation: 'text-to-image', prompt, panelId: panel.id });
  if (result.ok) {
    el('generateStatus').textContent = `Job ${result.value.jobId} submitted; it will wait for review.`;
    setView('review');
  } else if (result.kind === 'disabled') {
    el('generateStatus').textContent = 'Generation runs on the server, where credentials live; this browser can review results but cannot call a provider.';
  }
});

/**
 * The asset a person picked in the library. Placement uses its content hash, so the existing
 * asset record is reused rather than copied.
 * @type {string|null}
 */
state.selectedAssetId = null;

el('artworkPlaceBtn').addEventListener('click', async () => {
  const panel = panelOfNode(state.selectedId);
  if (!panel) { toast('Select a panel first.', true); return; }
  let asset = state.selectedAssetId ? state.bundle.project.assets[state.selectedAssetId] : null;
  if (!asset) {
    const name = await askText({ title: 'Place artwork', label: 'A name for this artwork.', value: 'Panel art' });
    if (!name) return;
    asset = {
      name,
      contentHash: `manual-${Date.now()}`,
      data: 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=',
    };
  }
  const result = await run('comic.artwork.place', {
    panelId: panel.id,
    name: asset.name,
    contentHash: asset.contentHash,
    ...(asset.data ? { data: asset.data } : {}),
    ...(asset.assetType === 'generated' ? { provider: asset.provenance?.provider, model: asset.provenance?.model } : {}),
  });
  if (result.ok) state.selectedId = panel.frameNodeId;
  render();
});

el('cropBtn').addEventListener('click', async () => {
  const panel = panelOfNode(state.selectedId);
  if (!panel) { toast('Select a panel first.', true); return; }
  const raw = await askText({ title: 'Crop artwork', label: 'x, y, width, height as fractions of the image (0–1)', value: '0.05,0.05,0.9,0.9', action: 'Crop' });
  if (!raw) return;
  const [x, y, width, height] = raw.split(',').map((value) => Number(value.trim()));
  if ([x, y, width, height].some((value) => !Number.isFinite(value))) { toast('Crop needs four numbers.', true); return; }
  await run('comic.artwork.crop', { panelId: panel.id, x, y, width, height });
  render();
});

el('clipBtn').addEventListener('click', async () => {
  const panel = panelOfNode(state.selectedId);
  if (!panel) { toast('Select a panel first.', true); return; }
  const raw = await askText({ title: 'Clip artwork', label: 'Inset from the panel edge, in page units', value: '6', action: 'Clip' });
  if (raw === null) return;
  await run('comic.artwork.clip', { panelId: panel.id, inset: Number(raw) || 0 });
  render();
});

el('validatePublicationBtn').addEventListener('click', async () => {
  const preset = /** @type {HTMLSelectElement} */ (el('publicationPreset')).value;
  const result = await run('comic.book.validate', { preset });
  if (result.ok) {
    state.publication = result.value;
    render();
  }
});

/**
 * Paint local drawings through the ordinary dispatcher. The batch is atomic, so a failure
 * leaves no half-illustrated page.
 * @param {string[]} panelIds
 * @param {string} [singlePrompt]
 */
async function paintPanels(panelIds, singlePrompt) {
  if (panelIds.length === 0) { toast('There are no panels to drawing.', true); return; }
  const style = /** @type {HTMLSelectElement} */ (el('artStyle')).value;
  let painted = 0;
  for (let i = 0; i < panelIds.length; i += 80) {
    const slice = panelIds.slice(i, i + 80);
    const commands = slice.map((panelId) => {
      const panel = state.bundle.project.panels[panelId];
      const prompt = singlePrompt || panel?.artworkPrompt || panel?.shotDescription || `Panel ${panelId}`;
      return { command: 'comic.artwork.paint', params: { panelId, style, prompt } };
    });
    const result = await run('comic.batch.run', { commands });
    if (!result.ok) { render(); return; }
    painted += slice.length;
  }
  toast(`Drew ${painted} panel(s) in ${style}.`);
  setView('canvas');
}

el('artPanelBtn').addEventListener('click', async () => {
  const page = currentPage();
  const panel = panelOfNode(state.selectedId) ?? (page?.panelIds[0] ? state.bundle.project.panels[page.panelIds[0]] : null);
  if (!panel) { toast('Add a panel first.', true); return; }
  const prompt = /** @type {HTMLTextAreaElement} */ (el('generatePrompt')).value.trim() || undefined;
  await paintPanels([panel.id], prompt);
});

el('artAllBtn').addEventListener('click', async () => {
  const panelIds = Object.values(state.bundle.project.pages)
    .flatMap((page) => page.panelIds)
    .filter((panelId) => state.bundle.project.panels[panelId]);
  if (panelIds.length === 0) { toast('Add a page first.', true); return; }
  await paintPanels(panelIds);
});

let assetRenderTimer = /** @type {any} */ (null);
window.addEventListener('comiccraft:asset-loaded', () => {
  clearTimeout(assetRenderTimer);
  assetRenderTimer = setTimeout(() => render(), 60);
});

el('zoom').addEventListener('input', (event) => {
  state.scale = Number(/** @type {HTMLInputElement} */ (event.target).value);
  render();
});

el('saveBtn').addEventListener('click', async () => {
  const serialized = serialize(state.bundle);
  if (!serialized.ok) {
    toast('The project is not valid, so it was not saved.', true);
    return;
  }
  const root = state.src ?? saveKey();
  try {
    const count = await saveBundle(root, serialized.files);
    state.src = root;
    toast(`Saved ${count} file(s) locally.`);
  } catch (error) {
    toast(`Local save unavailable: ${/** @type {Error} */ (error).message}`, true);
  }
});

/* ------------------------------------------------------------------ boot */

async function boot() {
  const select = /** @type {HTMLSelectElement} */ (el('templateSelect'));
  for (const name of Object.keys(TEMPLATES).sort()) {
    const option = document.createElement('option');
    option.value = name;
    option.textContent = name;
    select.append(option);
  }
  if (TEMPLATES['three-up']) select.value = 'three-up';

  // A story import lives in the Story panel so the M3 workflow is reachable without an agent.
  const storySection = el('storyNav').parentElement;
  if (storySection) {
    const button = document.createElement('button');
    button.id = 'importStoryBtn';
    button.type = 'button';
    button.className = 'ghost wide';
    button.textContent = 'Import story…';
    button.addEventListener('click', async () => {
      const text = await askText({
        title: 'Import a story',
        label: 'Paste a story, synopsis or prompt. NAME: lines become dialogue; CAPTION: and SFX: are recognised.',
        multiline: true,
        action: 'Import',
      });
      if (!text) return;
      const result = await run('comic.story.import', { text });
      if (result.ok) { toast(`Proposal ${result.value.proposalId} is ready for review.`); setView('review'); }
    });
    storySection.append(button);
  }

  try {
    const saved = await listProjects();
    if (saved.length > 0) {
      const files = await loadBundle(saved[saved.length - 1]);
      const parsed = deserialize(files);
      if (parsed.ok) {
        const { bundle } = migrate(parsed.bundle);
        adoptBundle(bundle);
        state.src = saved[saved.length - 1];
        toast(`Reopened ${bundle.project.title}`);
      } else {
        toast('A saved project could not be reopened.', true);
      }
    }
  } catch (error) {
    // Storage is a convenience, not a requirement: a browser with IndexedDB disabled must
    // still edit, it just cannot save locally.
    toast(`Local storage unavailable: ${/** @type {Error} */ (error).message}`, true);
  }

  document.addEventListener('keydown', async (event) => {
    const mod = event.ctrlKey || event.metaKey;
    if (!mod) return;
    if (event.key === 'z' && !event.shiftKey) { event.preventDefault(); await run('comic.edit.undo'); render(); }
    if ((event.key === 'z' && event.shiftKey) || event.key === 'y') { event.preventDefault(); await run('comic.edit.redo'); render(); }
    if (event.key === 's') { event.preventDefault(); el('saveBtn').click(); }
  });

  for (const step of el('workflow').querySelectorAll('.workflow-step')) {
    step.addEventListener('click', async () => {
      const name = /** @type {HTMLElement} */ (step).dataset.step;
      if (name === 'idea' || name === 'design') { await designComic(); return; }
      if (name === 'review') { setView('review'); return; }
      if (name === 'edit' || name === 'art') { setView('canvas'); return; }
      setView('canvas');
      el('validatePublicationBtn').click();
    });
  }

  render();
}

await boot();

// Exposed for the browser console and for an end-to-end check; not used by the UI.
globalThis.ComicCraft = { state, registry, run, render, drawPage, measure };
