#!/usr/bin/env node
/**
 * ComicCraft — one Node process serving the client and the command API.
 *
 * `node server.js` is the whole setup: there is no build step and nothing to compile.
 */

import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import Fastify from 'fastify';
import { createSession } from './server/session.js';
import { registerRoutes } from './server/http.js';

const here = dirname(fileURLToPath(import.meta.url));
const PORT = Number(process.env.PORT ?? 3000);
const HOST = process.env.HOST ?? '127.0.0.1';

const app = Fastify({
  logger: process.env.LOG === 'off' ? false : { level: process.env.LOG ?? 'info' },
  // A comic bundle is a directory of JSON; a page of artwork can be large, but nothing
  // unbounded. Capping the body is what stops one request from exhausting memory.
  bodyLimit: 32 * 1024 * 1024,
});

const session = createSession({ root: process.env.COMICCRAFT_ROOT });

registerRoutes(app, session, { publicDir: join(here, 'public'), sharedDir: join(here, 'shared') });

try {
  await app.listen({ port: PORT, host: HOST });
  app.log.info(`storage root: ${session.root}`);
  app.log.info(`${session.registry.ids().length} commands registered`);
} catch (error) {
  app.log.error(error);
  process.exit(1);
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, async () => {
    await app.close();
    process.exit(0);
  });
}

export { session };
