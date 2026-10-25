/**
 * The browser's own bundle store.
 *
 * A project is a directory bundle on the server and an object here, and **the serializer is
 * the same module in both cases** — `shared/domain/bundle.js` — so a project saved locally
 * opens on the server unchanged. There is no separate "export format" to keep in sync, which
 * is the whole reason the browser can be a first-class editor rather than a preview.
 *
 * @module public/storage/local
 */

const DB_NAME = 'comiccraft';
const DB_VERSION = 1;
const STORE = 'projects';

/**
 * @returns {Promise<IDBDatabase>}
 */
function open() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE, { keyPath: 'path' });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB refused to open'));
  });
}

/**
 * @param {IDBDatabase} db
 * @param {IDBTransactionMode} mode
 * @returns {IDBObjectStore}
 */
const store = (db, mode) => db.transaction(STORE, mode).objectStore(STORE);

/**
 * @param {IDBDatabase} db
 * @param {IDBRequest} request
 * @returns {Promise<any>}
 */
const done = (db, request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
  void db;
});

/**
 * Write a serialized bundle. Each file is a row keyed by `bundlePath/filePath`, so a project
 * is a set of rows rather than one blob — which is what makes "save one page" possible later
 * without rewriting the book.
 * @param {string} root
 * @param {Map<string, string | Uint8Array>} files
 */
export async function saveBundle(root, files) {
  const db = await open();
  const tx = db.transaction(STORE, 'readwrite');
  const objectStore = tx.objectStore(STORE);
  objectStore.delete(IDBKeyRange.bound(`${root}/`, `${root}/\uffff`));
  for (const [path, data] of files) {
    objectStore.put({ path: `${root}/${path}`, root, data: typeof data === 'string' ? data : new TextDecoder().decode(data) });
  }
  await new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve(undefined);
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
  db.close();
  return files.size;
}

/**
 * @param {string} root
 * @returns {Promise<Map<string, string>>}
 */
export async function loadBundle(root) {
  const db = await open();
  const rows = await done(db, store(db, 'readonly').getAll());
  db.close();
  /** @type {Map<string, string>} */
  const files = new Map();
  for (const row of rows) {
    if (row.root !== root) continue;
    files.set(row.path.slice(root.length + 1), row.data);
  }
  return files;
}

/** @returns {Promise<string[]>} */
export async function listProjects() {
  const db = await open();
  const rows = await done(db, store(db, 'readonly').getAll());
  db.close();
  /** @type {Set<string>} */
  const roots = new Set();
  for (const row of rows) roots.add(row.root);
  return [...roots].sort();
}

/**
 * @param {string} root
 */
export async function deleteProject(root) {
  const db = await open();
  const tx = db.transaction(STORE, 'readwrite');
  tx.objectStore(STORE).delete(IDBKeyRange.bound(`${root}/`, `${root}/\uffff`));
  await new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve(undefined);
    tx.onerror = () => reject(tx.error);
  });
  db.close();
}
