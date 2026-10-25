/**
 * `comic.story.*` and `comic.storyboard.*` — text in, reviewable structured storyboard out.
 *
 * Nothing here is authoritative until `comic.storyboard.accept`. An import stores a
 * proposal beside the book; accepting one scene writes only that scene's pages, so an edit to
 * one scene provably cannot rewrite another.
 *
 * @module shared/commands/commands/story
 */

import { fail, ok } from '../registry.js';
import { allocate } from '../../domain/project.js';
import {
  analyzeStory, applyProposal, carryIds, designStoryboard, proposalOutline, validateProposal, fingerprint,
} from '../../domain/story.js';
import { reviewDecision } from '../../domain/jobs.js';
import { errorsOf } from '../../domain/validate.js';

/** @param {any} ctx @param {boolean} [includeText] @returns {any[]} */
function proposalsOf(ctx, includeText = false) {
  return Object.values(ctx.bundle.project.proposals ?? {}).map((/** @type {any} */ proposal) => ({
    proposalId: proposal.id,
    status: proposal.status,
    title: proposal.title,
    created: proposal.created,
    updated: proposal.updated,
    baseRevision: proposal.baseRevision,
    scenes: proposal.scenes.length,
    pages: proposal.scenes.reduce((/** @type {number} */ n, /** @type {any} */ s) => n + s.pages.length, 0),
    ...(includeText ? { sourceText: proposal.sourceText, sourceFingerprint: fingerprint(proposal.sourceText ?? '') } : {}),
  }));
}

/** @param {any} ctx @param {any} proposal @returns {any} */
function storeProposal(ctx, proposal) {
  ctx.bundle.project.proposals[proposal.id] = proposal;
  return proposal;
}

/** @param {any} ctx @param {string} proposalId @returns {any | null} */
const proposalOf = (ctx, proposalId) => ctx.bundle.project.proposals?.[proposalId] ?? null;

/** @type {import('../registry.js').RegisteredCommand[]} */
export const storyCommands = [
  {
    id: 'comic.story.import',
    label: 'Import Story',
    params: {
      type: 'object',
      properties: {
        text: { type: 'string', minLength: 1, maxLength: 200000, description: 'A prompt, synopsis or manuscript. Headings, NAME: lines, CAPTION: and SFX: are recognised.' },
        title: { type: 'string', maxLength: 200 },
        maxPanelsPerPage: { type: 'integer', minimum: 1, maximum: 6, description: 'Default 4.' },
      },
      required: ['text'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['proposals'] },
    run(ctx, params) {
      const { proposal, problems } = analyzeStory(params.text, {
        title: params.title,
        maxPanelsPerPage: params.maxPanelsPerPage,
        now: ctx.env.now ? ctx.env.now() : 0,
        baseRevision: ctx.bundle.project.revision ?? 1,
      });
      proposal.id = allocate(ctx.bundle, 'proposal');
      storeProposal(ctx, proposal);
      const errors = errorsOf(problems);
      if (errors.length > 0) {
        return fail('conflict', `the story could not be structured: ${errors[0].message}`);
      }
      return ok({
        proposalId: proposal.id,
        outline: proposalOutline(proposal),
        problems,
        status: proposal.status,
        sourceFingerprint: fingerprint(params.text),
      });
    },
  },

  {
    id: 'comic.story.design',
    label: 'Design Comic',
    params: {
      type: 'object',
      properties: {
        ideas: { type: 'string', minLength: 1, maxLength: 200000, description: 'The story, idea, outline or manuscript to turn into a comic.' },
        title: { type: 'string', maxLength: 200 },
        style: { type: 'string', maxLength: 80, description: 'Visual direction, e.g. black ink, manga, watercolour, noir.' },
        audience: { type: 'string', enum: ['all ages', 'teen', 'mature'] },
        characters: { type: 'array', items: { type: 'string', minLength: 1, maxLength: 400 }, maxItems: 24, description: 'Names, optionally Name: appearance.' },
        maxPanelsPerPage: { type: 'integer', minimum: 1, maximum: 6, description: 'Default 3.' },
        provider: { type: 'string', enum: ['local', 'http'], description: 'local runs the deterministic director in any runtime; http is server-only and needs configuration.' },
      },
      required: ['ideas'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    // No undo declaration on purpose: the HTTP director may take a minute, and an open undo
    // transaction for that whole time would block every other command. The proposal is inert
    // until accepted and can be rejected, so a design does not need an undo step.
    async run(ctx, params) {
      const now = ctx.env.now ? ctx.env.now() : 0;
      const baseRevision = ctx.bundle.project.revision ?? 1;
      let proposal;
      /** @type {import('../../domain/validate.js').Problem[]} */
      let problems = [];
      const provider = params.provider ?? /** @type {any} */ (ctx.services?.storyAgent)?.defaultId?.() ?? 'local';
      if (provider === 'http') {
        const agent = /** @type {any} */ (ctx.services?.storyAgent);
        if (!agent) {
          return fail('disabled', 'the HTTP story agent runs on the server, where its credentials live; choose the local director instead', 'provider');
        }
        const designed = await agent.design(params.ideas, {
          title: params.title,
          style: params.style,
          audience: params.audience,
          characters: params.characters ?? [],
          maxPanelsPerPage: params.maxPanelsPerPage,
          now,
          baseRevision,
        });
        if (!designed.ok) return fail(designed.kind ?? 'conflict', designed.message);
        proposal = designed.proposal;
        problems = designed.problems ?? [];
      } else {
        const designed = designStoryboard(params.ideas, {
          title: params.title,
          style: params.style,
          audience: params.audience,
          characters: params.characters ?? [],
          maxPanelsPerPage: params.maxPanelsPerPage,
          now,
          baseRevision,
        });
        proposal = designed.proposal;
        problems = designed.problems;
      }
      const errors = errorsOf(problems);
      if (errors.length > 0) {
        return fail('conflict', `the design could not be structured: ${errors[0].message}`);
      }
      proposal.id = allocate(ctx.bundle, 'proposal');
      proposal.baseRevision = baseRevision;
      storeProposal(ctx, proposal);
      return ok({
        proposalId: proposal.id,
        outline: proposalOutline(proposal),
        problems,
        status: proposal.status,
        design: proposal.design,
        sourceFingerprint: fingerprint(params.ideas),
      });
    },
  },

  {
    id: 'comic.story.providers',
    label: 'Story Design Providers',
    params: { type: 'object', properties: {}, additionalProperties: false },
    run(ctx) {
      const agent = /** @type {any} */ (ctx.services?.storyAgent);
      return ok({
        providers: agent?.describe?.() ?? [{
          id: 'local', label: 'Local comic director', model: 'deterministic-storyboard-v1',
          configured: true, requiresNetwork: false,
        }],
        defaultProvider: agent?.defaultId?.() ?? 'local',
      });
    },
  },

  {
    id: 'comic.story.get',
    label: 'Get Story Proposal',
    params: {
      type: 'object',
      properties: { proposalId: { type: 'string', minLength: 1 } },
      required: ['proposalId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const proposal = proposalOf(ctx, params.proposalId);
      if (!proposal) return fail('not-found', `no story proposal ${params.proposalId}`, 'proposalId');
      return ok({ proposal });
    },
  },

  {
    id: 'comic.story.validate',
    label: 'Validate Story Proposal',
    params: {
      type: 'object',
      properties: { proposalId: { type: 'string', minLength: 1 } },
      required: ['proposalId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      const proposal = proposalOf(ctx, params.proposalId);
      if (!proposal) return fail('not-found', `no story proposal ${params.proposalId}`, 'proposalId');
      const problems = validateProposal(proposal);
      return ok({ proposalId: proposal.id, problems, errors: errorsOf(problems).length });
    },
  },

  {
    id: 'comic.storyboard.generate',
    label: 'Generate Storyboard',
    params: {
      type: 'object',
      properties: {
        proposalId: { type: 'string', minLength: 1, description: 'Regenerate this proposal from its stored source, carrying its ids.' },
        text: { type: 'string', minLength: 1, maxLength: 200000, description: 'New source instead of a stored proposal.' },
        title: { type: 'string', maxLength: 200 },
        maxPanelsPerPage: { type: 'integer', minimum: 1, maximum: 6 },
      },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['proposals'] },
    run(ctx, params) {
      const previous = params.proposalId ? proposalOf(ctx, params.proposalId) : null;
      if (params.proposalId && !previous) {
        return fail('not-found', `no story proposal ${params.proposalId}`, 'proposalId');
      }
      const text = params.text ?? previous?.sourceText;
      if (typeof text !== 'string' || text.trim() === '') {
        return fail('bad-params', 'storyboard generation needs text or a proposal with a source', 'text');
      }
      const analyzed = previous?.design
        ? designStoryboard(text, {
            title: params.title ?? previous.title,
            style: previous.style,
            audience: previous.audience,
            characters: previous.design?.characters ?? [],
            maxPanelsPerPage: params.maxPanelsPerPage ?? previous.design?.panelsPerPage,
            now: ctx.env.now ? ctx.env.now() : 0,
            baseRevision: ctx.bundle.project.revision ?? 1,
          })
        : analyzeStory(text, {
            title: params.title ?? previous?.title,
            maxPanelsPerPage: params.maxPanelsPerPage,
            now: ctx.env.now ? ctx.env.now() : 0,
            baseRevision: ctx.bundle.project.revision ?? 1,
          });
      let proposal = analyzed.proposal;
      // A regenerate carries the existing keys' ids, so accepting it reuses the pages and
      // panels already in the book rather than renumbering them.
      proposal = carryIds(proposal, ctx.bundle, previous);
      proposal.id = previous ? previous.id : allocate(ctx.bundle, 'proposal');
      proposal.created = previous?.created ?? proposal.created;
      storeProposal(ctx, proposal);
      return ok({
        proposalId: proposal.id,
        outline: proposalOutline(proposal),
        problems: analyzed.problems,
        carriedIds: proposal.idMap,
        sourceFingerprint: fingerprint(text),
      });
    },
  },

  {
    id: 'comic.storyboard.list',
    label: 'List Storyboards',
    params: {
      type: 'object',
      properties: {
        status: { type: 'string', enum: ['proposed', 'approved', 'rejected', 'superseded'] },
        includeText: { type: 'boolean' },
      },
      required: [],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    run(ctx, params) {
      let list = proposalsOf(ctx, params.includeText === true);
      if (params.status) list = list.filter((p) => p.status === params.status);
      return ok({ proposals: list, count: list.length });
    },
  },

  {
    id: 'comic.storyboard.accept',
    label: 'Accept Storyboard',
    params: {
      type: 'object',
      properties: {
        proposalId: { type: 'string', minLength: 1 },
        sceneKeys: { type: 'array', items: { type: 'string', minLength: 1 }, description: 'Scenes to accept. Omit for every scene. Unlisted scenes are left completely untouched.' },
        chapterId: { type: 'string', minLength: 1, description: 'Chapter to place new scenes in. Defaults to the first chapter.' },
        preserveExisting: { type: 'boolean', description: 'Keep approved content on a page that already exists and refresh only descriptions. Default true.' },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['proposalId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['chapters', 'scenes', 'pages', 'panels', 'dialogue', 'lettering', 'characters', 'proposals', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const proposal = proposalOf(ctx, params.proposalId);
      if (!proposal) return fail('not-found', `no story proposal ${params.proposalId}`, 'proposalId');
      if (proposal.status === 'rejected') {
        return fail('conflict', `proposal ${proposal.id} was rejected; generate a new one before accepting`, 'proposalId');
      }
      const problem = validateProposal(proposal);
      const errors = errorsOf(problem);
      if (errors.length > 0) {
        return fail('conflict', `proposal ${proposal.id} is not valid: ${errors[0].message}`, 'proposalId');
      }
      if (params.chapterId && !bundle.project.chapters[params.chapterId]) {
        return fail('not-found', `no chapter ${params.chapterId}`, 'chapterId');
      }
      const allSceneKeys = proposal.scenes.map((/** @type {any} */ scene) => scene.key);
      const requested = params.sceneKeys ?? allSceneKeys;
      const unknown = requested.filter((/** @type {string} */ key) => !allSceneKeys.includes(key));
      if (unknown.length > 0) {
        return fail('bad-params', `no scene ${unknown[0]} in proposal ${proposal.id}`, 'sceneKeys');
      }
      const uniqueRequested = [...new Set(requested)];
      if (uniqueRequested.length === 0) {
        return fail('bad-params', 'accept needs at least one scene', 'sceneKeys');
      }

      const applied = applyProposal(bundle, proposal, {
        sceneKeys: params.sceneKeys,
        chapterId: params.chapterId,
        now: ctx.env.now ? ctx.env.now() : 0,
        preserveExisting: params.preserveExisting !== false,
      });
      const now = ctx.env.now ? ctx.env.now() : 0;
      const accepted = new Set([...(proposal.acceptedSceneKeys ?? []), ...uniqueRequested]);
      proposal.acceptedSceneKeys = allSceneKeys.filter((/** @type {string} */ key) => accepted.has(key));
      const complete = proposal.acceptedSceneKeys.length === allSceneKeys.length;
      proposal.status = complete ? 'approved' : 'proposed';
      proposal.acceptedAt = complete ? now : null;
      proposal.updated = now;
      if (params.note !== undefined) proposal.note = params.note;

      let decisionId = null;
      if (complete) {
        decisionId = allocate(bundle, 'decision');
        bundle.project.reviews[decisionId] = reviewDecision({
          id: decisionId,
          subject: 'story',
          subjectId: proposal.id,
          decision: 'approved',
          actor: 'user',
          timestamp: now,
          note: params.note ?? null,
        });
      }

      return ok({
        proposalId: proposal.id,
        status: proposal.status,
        ...applied,
        idMap: proposal.idMap,
        decisionId,
        sceneCount: applied.scenes.length,
        acceptedSceneKeys: [...proposal.acceptedSceneKeys],
        remainingSceneKeys: allSceneKeys.filter((/** @type {string} */ key) => !accepted.has(key)),
      });
    },
  },

  {
    id: 'comic.storyboard.reject',
    label: 'Reject Storyboard',
    params: {
      type: 'object',
      properties: {
        proposalId: { type: 'string', minLength: 1 },
        note: { type: 'string', maxLength: 1000 },
      },
      required: ['proposalId'],
      additionalProperties: false,
    },
    enabled: (ctx) => Boolean(ctx.bundle),
    undo: { scope: 'project', tables: ['proposals', 'reviews'] },
    run(ctx, params) {
      const { bundle } = ctx;
      const proposal = proposalOf(ctx, params.proposalId);
      if (!proposal) return fail('not-found', `no story proposal ${params.proposalId}`, 'proposalId');
      if (proposal.status === 'approved') {
        return fail('conflict', `proposal ${proposal.id} was already accepted; rejecting it now would not remove the pages it created`, 'proposalId');
      }
      const now = ctx.env.now ? ctx.env.now() : 0;
      proposal.status = 'rejected';
      proposal.updated = now;
      if (params.note !== undefined) proposal.note = params.note;
      const decisionId = allocate(bundle, 'decision');
      bundle.project.reviews[decisionId] = reviewDecision({
        id: decisionId,
        subject: 'story',
        subjectId: proposal.id,
        decision: 'rejected',
        actor: 'user',
        timestamp: now,
        note: params.note ?? null,
      });
      return ok({ proposalId: proposal.id, status: proposal.status, decisionId });
    },
  },
];
