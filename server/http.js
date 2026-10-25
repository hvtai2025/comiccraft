/**
 * The HTTP transport.
 *
 * Every comic operation is `POST /api/comic/<command>` with a JSON body of parameters. There
 * is no per-command route: the dispatcher already knows the command, and a second layer that
 * maps ids to handlers would be a second place for them to disagree.
 *
 * @module server/http
 */

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { join, extname, resolve, relative, isAbsolute } from 'node:path';
import { dispatch } from '../shared/commands/dispatch.js';
import { validateParams } from '../shared/commands/index.js';

/** @type {Record<string, string>} */
const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.map': 'application/json; charset=utf-8',
};

/**
 * @param {import('fastify').FastifyInstance} app
 * @param {ReturnType<import('./session.js').createSession>} session
 * @param {{ publicDir: string, sharedDir: string }} options
 */
export function registerRoutes(app, session, options) {
  const publicDir = resolve(options.publicDir);
  const sharedDir = resolve(options.sharedDir);

  app.get('/api/health', async () => ({
    ok: true,
    service: 'comiccraft',
    status: 'healthy',
    runtime: `node ${process.versions.node}`,
    architecture: 'Node.js backend + client-side HTML/JavaScript frontend',
    commands: session.registry.ids().length,
  }));

  /** The command catalogue: what the UI menus and an agent both build from. */
  app.get('/api/comic', async () => ({ commands: session.registry.describe() }));

  app.get('/api/comic/:command', async (request, reply) => {
    const { command } = /** @type {any} */ (request.params);
    const query = /** @type {any} */ (request.query) ?? {};
    return runCommand(session, command, query, reply);
  });

  app.post('/api/comic/:command', async (request, reply) => {
    const { command } = /** @type {any} */ (request.params);
    const body = request.body === undefined || request.body === null ? {} : request.body;
    return runCommand(session, command, body, reply);
  });

  /**
   * `shared/` is served to the browser verbatim, because the client imports the same modules
   * the server does. That is the load-bearing idea of this architecture: a rule is
   * implemented once, and the browser is just the runtime with a canvas in it.
   *
   * It is served from the repository, not copied into `public/`, precisely so that the two
   * cannot drift — there is one copy of the domain on disk.
   */
  app.get('/shared/*', async (request, reply) => {
    const url = /** @type {any} */ (request.params)['*'] ?? '';
    const abs = resolve(sharedDir, url);
    const rel = relative(sharedDir, abs);
    if (rel.startsWith('..') || isAbsolute(rel)) {
      reply.code(403).type('text/plain');
      return reply.send('forbidden');
    }
    if (!/\.[cm]?js$/.test(abs)) {
      // Only modules. The directory contains no secrets today, and this keeps it that way if
      // someone later drops a fixture or a note beside the code.
      reply.code(404).type('text/plain');
      return reply.send('not found');
    }
    return serve(reply, abs);
  });

  /** Static files. Anything that is not an API route or `shared/` is the client. */
  app.get('/*', async (request, reply) => {
    const url = /** @type {any} */ (request.params)['*'] ?? '';
    const wanted = url === '' ? 'index.html' : url;
    const abs = resolve(publicDir, wanted);
    const rel = relative(publicDir, abs);
    if (rel.startsWith('..') || isAbsolute(rel)) {
      reply.code(403).type('text/plain');
      return reply.send('forbidden');
    }
    try {
      const info = await stat(abs);
      if (info.isDirectory()) return serve(reply, join(abs, 'index.html'));
      return serve(reply, abs);
    } catch {
      // Unknown paths fall back to the app shell, so a client-side route deep-links.
      return serve(reply, join(publicDir, 'index.html'), true);
    }
  });
}

/**
 * @param {any} session @param {string} command @param {unknown} params @param {any} reply
 */
async function runCommand(session, command, params, reply) {
  const result = await session.run(command, params);
  const status = result.ok ? 200
    : result.kind === 'bad-params' ? 400
    : result.kind === 'not-found' ? 404
    : result.kind === 'conflict' ? 409
    : result.kind === 'disabled' ? 403
    : result.kind === 'internal' ? 500
    : 500;
  // A failed command is a normal outcome with a status code, not an exception with a stack.
  reply.code(status);
  return { ...result, history: session.history.depth() };
}

/**
 * @param {any} reply @param {string} abs @param {boolean} [spa]
 */
async function serve(reply, abs, spa = false) {
  try {
    const info = await stat(abs);
    if (!info.isFile()) throw new Error('not a file');
    reply.type(MIME[extname(abs)] ?? 'application/octet-stream');
    reply.header('content-length', String(info.size));
    reply.header('cache-control', spa ? 'no-store' : 'public, max-age=60');
    return reply.send(createReadStream(abs));
  } catch {
    reply.code(404).type('text/plain');
    return reply.send('not found');
  }
}

export { dispatch, validateParams };
