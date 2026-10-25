/**
 * Filesystem storage for project bundles.
 *
 * Every path is resolved against a configured root and **refused if it escapes it**. A project
 * bundle is a directory a user can point at, so "list the project" is a path-traversal surface
 * — a `../../etc/passwd` in a bundle manifest must be rejected, not resolved.
 *
 * @module server/storage/fs
 */

import { mkdir, readFile, writeFile, readdir, rm, stat } from 'node:fs/promises';
import { dirname, join, resolve, relative, sep, isAbsolute } from 'node:path';

export class PathEscapeError extends Error {
  /** @param {string} path @param {string} root */
  constructor(path, root) {
    super(`refusing to touch ${path}: it resolves outside the storage root (${root})`);
    this.name = 'PathEscapeError';
  }
}

/**
 * @param {string} root
 * @returns {{ root: string, resolve: (p: string) => string }}
 */
function guard(root) {
  const absRoot = resolve(root);
  /**
   * @param {string} p
   * @returns {string}
   */
  const resolveSafe = (p) => {
    if (typeof p !== 'string' || p.length === 0) throw new PathEscapeError(String(p), absRoot);
    if (p.includes('\0')) throw new PathEscapeError(p, absRoot);
    if (isAbsolute(p)) throw new PathEscapeError(p, absRoot);

    const abs = resolve(absRoot, p);
    const rel = relative(absRoot, abs);
    if (rel === '') return absRoot;
    if (rel.startsWith('..') || isAbsolute(rel)) throw new PathEscapeError(p, absRoot);
    return abs;
  };
  return { root: absRoot, resolve: resolveSafe };
}

/**
 * @param {string} root
 * @returns {import('../../shared/domain/project.js').Storage}
 */
export function createFileStorage(root) {
  const g = guard(root);

  return {
    /** @param {string} path */
    async read(path) {
      const abs = g.resolve(path);
      try {
        return await readFile(abs, 'utf8');
      } catch (error) {
        if (/** @type {any} */ (error).code === 'ENOENT') return undefined;
        throw error;
      }
    },

    async write(path, data) {
      const abs = g.resolve(path);
      await mkdir(dirname(abs), { recursive: true });
      await writeFile(abs, data, 'utf8');
    },

    /** Binary read, for an archive or a generated image. */
    async readBytes(path) {
      const abs = g.resolve(path);
      try {
        const data = await readFile(abs);
        return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      } catch (error) {
        if (/** @type {any} */ (error).code === 'ENOENT') return undefined;
        throw error;
      }
    },

    /** Binary write. The path is confined by the same guard as every other operation. */
    async writeBytes(path, data) {
      const abs = g.resolve(path);
      await mkdir(dirname(abs), { recursive: true });
      await writeFile(abs, data);
    },

    async list(prefix) {
      const abs = g.resolve(prefix);
      let info;
      try {
        info = await stat(abs);
      } catch (error) {
        if (/** @type {any} */ (error).code === 'ENOENT') return [];
        throw error;
      }
      if (!info.isDirectory()) return [];

      /** @type {string[]} */
      const out = [];
      /** @param {string} dir */
      const walk = async (dir) => {
        for (const entry of await readdir(dir, { withFileTypes: true })) {
          const full = join(dir, entry.name);
          if (entry.isDirectory()) await walk(full);
          else if (entry.isFile()) out.push(relative(g.root, full).split(sep).join('/'));
        }
      };
      await walk(abs);
      return out.sort();
    },

    async remove(prefix) {
      const abs = g.resolve(prefix);
      await rm(abs, { recursive: true, force: true });
    },

    async ensureRoot(path) {
      await mkdir(g.resolve(path), { recursive: true });
    },
  };
}

/**
 * In-memory storage with the same contract, for tests and for the browser's own bundle store.
 * @returns {import('../../shared/domain/project.js').Storage}
 */
export function createMemoryStorage() {
  /** @type {Map<string, string | Uint8Array>} */
  const files = new Map();
  return {
    /** @param {string} path */
    async read(path) {
      const value = files.get(path);
      return value === undefined ? undefined : typeof value === 'string' ? value : new TextDecoder().decode(value);
    },
    async readBytes(path) {
      const value = files.get(path);
      if (value === undefined) return undefined;
      return typeof value === 'string' ? new TextEncoder().encode(value) : value;
    },
    async write(path, data) { files.set(path, data); },
    async writeBytes(path, data) { files.set(path, data); },
    async list(prefix) {
      return [...files.keys()].filter((p) => p === prefix || p.startsWith(`${prefix}/`)).sort();
    },
    async remove(prefix) {
      for (const p of [...files.keys()]) {
        if (p === prefix || p.startsWith(`${prefix}/`)) files.delete(p);
      }
    },
    async ensureRoot() { /* nothing to do in memory */ },
  };
}
