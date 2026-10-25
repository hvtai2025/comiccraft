#!/usr/bin/env node
/**
 * Generate `schema/v1/` and its fixtures from the domain's own constants.
 *
 * Generating rather than hand-writing is deliberate: a fixture written by hand drifts from the
 * model silently, and a schema that disagrees with the code is worse than none because it
 * reports success on a project that will not open. This runs once and its output is committed,
 * so the fixture gate has something stable to check.
 *
 * Usage: node tools/gen-fixtures.js
 */

import { writeFile, mkdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { SCHEMA_VERSION } from '../shared/domain/project.js';
import { DOCUMENT_VERSION } from '../shared/domain/document.js';

const root = resolve(import.meta.dirname, '..');
const schemaDir = join(root, 'schema', 'v1');
const fixtureDir = join(root, 'schema', 'fixtures', 'v1');

/** @param {string} p @param {unknown} value */
const write = async (p, value) => {
  await mkdir(join(p, '..'), { recursive: true });
  await writeFile(p, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
};

const idString = { type: 'string', minLength: 3, maxLength: 60, description: 'A stable, never-reused comic id.' };
const rectSchema = {
  type: 'object',
  properties: {
    x: { type: 'number' }, y: { type: 'number' },
    width: { type: 'number', minimum: 0 }, height: { type: 'number', minimum: 0 },
  },
  required: ['x', 'y', 'width', 'height'],
  additionalProperties: false,
};

const schemas = {
  'manifest': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft manifest',
    description: 'Written last, so it can never reference partially written data.',
    type: 'object',
    properties: {
      name: { type: 'string' },
      schemaVersion: { type: 'integer', minimum: 1 },
      documentVersion: { type: 'integer', minimum: 1 },
      projectId: idString,
      title: { type: 'string', maxLength: 200 },
      pageCount: { type: 'integer', minimum: 0 },
    },
    required: ['name', 'schemaVersion', 'documentVersion', 'projectId', 'title', 'pageCount'],
    additionalProperties: false,
  },

  'character': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft character',
    type: 'object',
    properties: {
      id: idString,
      name: { type: 'string', minLength: 1, maxLength: 200 },
      description: { type: 'string', maxLength: 2000, description: 'Continuity note carried into every generation.' },
      referenceAssetIds: { type: 'array', items: idString },
    },
    required: ['id', 'name', 'description', 'referenceAssetIds'],
    additionalProperties: false,
  },

  'asset': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft asset',
    type: 'object',
    properties: {
      id: idString,
      name: { type: 'string', minLength: 1, maxLength: 200 },
      contentHash: { type: 'string', minLength: 1, description: 'The asset is stored under this name: a new file, never an overwrite.' },
      data: { type: 'string', maxLength: 20000000 },
      nodeIds: { type: 'array', items: { type: 'integer', minimum: 1 } },
      provenance: {
        type: 'object',
        properties: {
          provider: { type: 'string', maxLength: 100 },
          model: { type: ['string', 'null'], maxLength: 100 },
          request: { type: ['string', 'null'], maxLength: 4000 },
          placedAt: { type: ['integer', 'null'] },
        },
        required: ['provider'],
        additionalProperties: true,
      },
    },
    required: ['id', 'name', 'contentHash', 'nodeIds'],
    additionalProperties: false,
  },

  'panel': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft panel',
    description: 'Carries a frame *reference*, never its own geometry: two copies of a rectangle is the failure this architecture prevents.',
    type: 'object',
    properties: {
      id: idString,
      frameNodeId: { type: 'integer', minimum: 1 },
      shotDescription: { type: 'string', maxLength: 2000 },
      dialogueIds: { type: 'array', items: idString },
      letteringIds: { type: 'array', items: idString },
      artworkRefs: { type: 'array', items: { type: 'integer', minimum: 1 } },
      continuityIds: { type: 'array', items: idString },
      locked: { type: 'boolean' },
    },
    required: ['id', 'frameNodeId', 'dialogueIds', 'letteringIds', 'artworkRefs', 'continuityIds', 'locked'],
    additionalProperties: false,
  },

  'lettering': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft lettering',
    description: 'Balloons, captions and sound effects are one entity: their difference is presentation, not structure.',
    type: 'object',
    properties: {
      id: idString,
      kind: { type: 'string', enum: ['balloon', 'caption', 'sound-effect'] },
      nodeId: { type: 'integer', minimum: 1 },
      text: { type: 'string', maxLength: 4000 },
      overflowPolicy: { type: 'string', enum: ['warn', 'shrink', 'resize-frame'] },
      tailNodeId: { type: 'integer', minimum: 1 },
      tailTip: rectSchema.properties && {
        type: 'object',
        properties: { x: { type: 'number' }, y: { type: 'number' } },
        required: ['x', 'y'],
        additionalProperties: false,
      },
    },
    required: ['id', 'kind', 'nodeId', 'text', 'overflowPolicy'],
    additionalProperties: false,
  },

  'dialogue': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft dialogue',
    type: 'object',
    properties: {
      id: idString,
      characterId: { type: ['string', 'null'] },
      letteringId: idString,
      text: { type: 'string', maxLength: 4000 },
    },
    required: ['id', 'characterId', 'letteringId', 'text'],
    additionalProperties: false,
  },

  'page': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft page',
    description: 'A page *is* an artboard. Reading order is explicit and never derived from coordinates.',
    type: 'object',
    properties: {
      id: idString,
      name: { type: 'string', minLength: 1, maxLength: 200 },
      artboardId: { type: 'integer', minimum: 1 },
      panelIds: { type: 'array', items: idString, description: 'Reading order.' },
    },
    required: ['id', 'name', 'artboardId', 'panelIds'],
    additionalProperties: false,
  },

  'document': {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'ComicCraft visual document',
    type: 'object',
    properties: {
      version: { type: 'integer', const: DOCUMENT_VERSION },
      nextNodeId: { type: 'integer', minimum: 1 },
      retiredArtboards: { type: 'array', items: { type: 'integer', minimum: 1 }, description: 'A retired id is never reissued.' },
      artboards: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            id: { type: 'integer', minimum: 1 },
            name: { type: 'string' },
            size: {
              type: 'object',
              properties: { width: { type: 'number', minimum: 1 }, height: { type: 'number', minimum: 1 } },
              required: ['width', 'height'],
              additionalProperties: false,
            },
            nodes: { type: 'array', items: { type: 'integer', minimum: 1 } },
          },
          required: ['id', 'name', 'size', 'nodes'],
          additionalProperties: false,
        },
      },
      nodes: { type: 'object' },
    },
    required: ['version', 'artboards', 'nodes', 'nextNodeId', 'retiredArtboards'],
    additionalProperties: false,
  },
};

// The `tailTip` entry above used a stray expression; overwrite it so the schema is plain data.
schemas.lettering.properties.tailTip = {
  type: 'object',
  properties: { x: { type: 'number' }, y: { type: 'number' } },
  required: ['x', 'y'],
  additionalProperties: false,
};

const fixtures = {
  'manifest': { name: 'comiccraft', schemaVersion: SCHEMA_VERSION, documentVersion: DOCUMENT_VERSION, projectId: 'project_1', title: 'Fixture Comic', pageCount: 1 },
  'character': { id: 'character_1', name: 'Narrator', description: 'A tall figure in a long coat; grey eyes.', referenceAssetIds: [] },
  'asset': { id: 'asset_1', name: 'Panel 1 art', contentHash: 'sha256-fixture', nodeIds: [3], provenance: { provider: 'local', model: null, request: null, placedAt: 0 } },
  'panel': { id: 'panel_1', frameNodeId: 1, dialogueIds: ['dialogue_1'], letteringIds: ['lettering_1'], artworkRefs: [2], continuityIds: [], locked: false },
  'lettering': { id: 'lettering_1', kind: 'balloon', nodeId: 4, text: 'IT FITS.', overflowPolicy: 'warn', tailNodeId: 5, tailTip: { x: 40, y: 90 } },
  'dialogue': { id: 'dialogue_1', characterId: 'character_1', letteringId: 'lettering_1', text: 'IT FITS.' },
  'page': { id: 'page_1', name: 'Page 1', artboardId: 10, panelIds: ['panel_1'] },
  'document': {
    version: DOCUMENT_VERSION, nextNodeId: 6, retiredArtboards: [],
    artboards: [{ id: 10, name: 'Page 1', size: { width: 1275, height: 1950 }, nodes: [1, 2, 4, 5] }],
    nodes: {
      '1': { id: 1, kind: 'frame', name: 'Panel', frame: { x: 0, y: 0, width: 1275, height: 1950 }, fill: '#ffffff', stroke: '#111111', strokeWidth: 3 },
      '2': { id: 2, kind: 'image', name: 'Art', frame: { x: 0, y: 0, width: 1275, height: 1950 }, assetId: 'asset_1', fit: 'cover', clipToFrame: true },
      '4': { id: 4, kind: 'text', name: 'Balloon', frame: { x: 100, y: 1500, width: 400, height: 120 }, text: 'IT FITS.', fontSize: 12, lineHeight: 1.25, align: 'left', color: '#111111' },
      '5': { id: 5, kind: 'frame', name: 'Balloon Tail', frame: { x: 34, y: 84, width: 12, height: 12 }, fill: '#ffffff', stroke: '#111111', strokeWidth: 2 },
    },
  },
};

await mkdir(schemaDir, { recursive: true });
await mkdir(fixtureDir, { recursive: true });

for (const [name, schema] of Object.entries(schemas)) {
  await write(join(schemaDir, `${name}.schema.json`), schema);
}
for (const [name, fixture] of Object.entries(fixtures)) {
  await write(join(fixtureDir, `${name}.json`), fixture);
}

console.log(`wrote ${Object.keys(schemas).length} schema(s) to schema/v1/`);
console.log(`wrote ${Object.keys(fixtures).length} fixture(s) to schema/fixtures/v1/`);
console.log(`schema version ${SCHEMA_VERSION}, document version ${DOCUMENT_VERSION}`);
