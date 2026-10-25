#!/usr/bin/env node
/**
 * The fixture gate: every schema version has a fixture, and every fixture validates.
 *
 * This is the load-bearing check of the repository. A schema change without a fixture is how a
 * bundle stops opening six months later with nobody able to say which field moved — so the rule
 * is that a schema and its fixture change together, and the gate refuses to pass otherwise.
 *
 * It does **not** trust the fixture to be valid: each one is checked against its schema, and a
 * fixture that no longer matches fails. Two mutations are also run against a copy — a missing
 * required field and a wrong type — because a validator that accepts everything would pass
 * every fixture and prove nothing.
 *
 * Usage: node tools/check-fixtures.js
 */

import { readFile, readdir } from 'node:fs/promises';
import { join, resolve, basename } from 'node:path';
import { pathToFileURL } from 'node:url';
import { check, unsupportedKeyword } from '../shared/commands/validate-params.js';

const root = resolve(import.meta.dirname, '..');

/**
 * @returns {Promise<string[]>}
 */
export async function checkFixtures() {
  /** @type {string[]} */
  const problems = [];

  const schemaRoot = join(root, 'schema');
  /** @type {string[]} */
  let versions;
  try {
    versions = (await readdir(schemaRoot, { withFileTypes: true }))
      .filter((e) => e.isDirectory() && e.name.startsWith('v'))
      .map((e) => e.name)
      .sort();
  } catch {
    return ['schema/ is missing: no schema version exists, so no bundle can be trusted'];
  }
  if (versions.length === 0) return ['schema/ has no versioned directory'];

  for (const version of versions) {
    const schemas = (await readdir(join(schemaRoot, version))).filter((f) => f.endsWith('.schema.json'));
    if (schemas.length === 0) {
      problems.push(`schema/${version}/ has no schemas`);
      continue;
    }

    const fixtureDir = join(schemaRoot, 'fixtures', version);
    /** @type {string[]} */
    let fixtures;
    try {
      fixtures = (await readdir(fixtureDir)).filter((f) => f.endsWith('.json'));
    } catch {
      problems.push(`schema/${version}/ has ${schemas.length} schema(s) but schema/fixtures/${version}/ does not exist`);
      continue;
    }

    for (const file of schemas) {
      const entity = basename(file, '.schema.json');
      const schema = JSON.parse(await readFile(join(schemaRoot, version, file), 'utf8'));

      const unsupported = unsupportedKeyword(schema);
      if (unsupported) {
        problems.push(`schema/${version}/${file}: the checker cannot enforce '${unsupported}', so the schema is not verified — add support or remove the keyword`);
        continue;
      }

      if (!fixtures.includes(`${entity}.json`)) {
        problems.push(`schema/${version}/${file}: no fixture at schema/fixtures/${version}/${entity}.json`);
        continue;
      }

      const fixture = JSON.parse(await readFile(join(fixtureDir, `${entity}.json`), 'utf8'));
      const valid = check(schema, fixture, entity);
      if (valid) {
        problems.push(`schema/fixtures/${version}/${entity}.json does not satisfy its schema: ${valid.message}`);
        continue;
      }

      // A validator that accepts everything passes every fixture, so probe it. Both probes must
      // be *rejected*; if either is accepted, the schema is not enforcing what it claims.
      const required = schema.required ?? [];
      if (required.length > 0) {
        const broken = { ...fixture };
        delete broken[required[0]];
        if (!check(schema, broken, entity)) {
          problems.push(`schema/${version}/${file}: removing required field '${required[0]}' was accepted — the fixture gate is not enforcing it`);
        }
      }
      const typed = Object.entries(schema.properties ?? {}).find(([, s]) => s.type === 'string');
      if (typed) {
        const [name] = typed;
        const broken = { ...fixture, [name]: 12345 };
        if (typeof fixture[name] === 'string' && !check(schema, broken, entity)) {
          problems.push(`schema/${version}/${file}: a number in the string field '${name}' was accepted`);
        }
      }
    }

    for (const fixture of fixtures) {
      const entity = basename(fixture, '.json');
      if (!schemas.includes(`${entity}.schema.json`)) {
        problems.push(`schema/fixtures/${version}/${fixture}: no schema at schema/${version}/${entity}.schema.json`);
      }
    }
  }

  return problems;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const problems = await checkFixtures();
  if (problems.length === 0) {
    console.log('fixtures ok — every schema has a fixture, every fixture validates, and the check rejects a mutation');
    process.exit(0);
  }
  console.error(`fixture problems (${problems.length}):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
