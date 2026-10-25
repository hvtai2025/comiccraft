/**
 * Deterministic single-file project package.
 *
 * A package is an **export**, not the working format: it contains the same bundle and the
 * same manifest, plus `archiveVersion`. It is deterministic — entries sorted, fixed DOS
 * timestamps, no extra fields — so packaging the same project twice produces identical bytes,
 * which is both a useful property and how the round trip is tested.
 *
 * Stored (uncompressed) entries are deliberate: the bundle is mostly JSON that is already
 * small, and a store-only archive has no compressor version to vary between platforms. The
 * reader accepts deflate as well, so an archive produced elsewhere still opens.
 *
 * @module server/export/package
 */

import { deflateRawSync, inflateRawSync } from 'node:zlib';
import { serialize, deserialize, MANIFEST } from '../../shared/domain/bundle.js';

export const ARCHIVE_VERSION = '1.0';

/** @type {Uint32Array | null} */
let crcTable = null;

/** @returns {Uint32Array} */
function crc32Table() {
  if (crcTable) return crcTable;
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  crcTable = table;
  return table;
}

/** @param {Uint8Array} bytes @returns {number} */
export function crc32(bytes) {
  const table = crc32Table();
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = table[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** @param {unknown} value @returns {Uint8Array} */
const bytesOf = (value) => typeof value === 'string' ? new TextEncoder().encode(value) : /** @type {Uint8Array} */ (value);

/* 1980-01-01 00:00:00, the DOS epoch. A fixed timestamp is what makes the archive stable. */
const DOS_TIME = 0;
const DOS_DATE = 0x0021;

/**
 * @param {Map<string, string|Uint8Array>} files
 * @param {{ compress?: boolean }} [options]
 * @returns {Uint8Array}
 */
export function zipFiles(files, options = {}) {
  const entries = [...files.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  /** @type {Uint8Array[]} */
  const locals = [];
  /** @type {{ name: Uint8Array, crc: number, size: number, compressed: number, method: number, offset: number }[]} */
  const central = [];
  let offset = 0;

  for (const [name, value] of entries) {
    const nameBytes = new TextEncoder().encode(name);
    const data = bytesOf(value);
    const crc = crc32(data);
    const tryCompress = options.compress !== false && data.length > 64;
    let method = 0;
    let payload = data;
    if (tryCompress) {
      const compressed = new Uint8Array(deflateRawSync(data));
      if (compressed.length < data.length) {
        method = 8;
        payload = compressed;
      }
    }

    const local = new Uint8Array(30 + nameBytes.length + payload.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 20, true);
    lv.setUint16(6, 0x0800, true);
    lv.setUint16(8, method, true);
    lv.setUint16(10, DOS_TIME, true);
    lv.setUint16(12, DOS_DATE, true);
    lv.setUint32(14, crc, true);
    lv.setUint32(18, payload.length, true);
    lv.setUint32(22, data.length, true);
    lv.setUint16(26, nameBytes.length, true);
    lv.setUint16(28, 0, true);
    local.set(nameBytes, 30);
    local.set(payload, 30 + nameBytes.length);
    locals.push(local);
    central.push({ name: nameBytes, crc, size: data.length, compressed: payload.length, method, offset });
    offset += local.length;
  }

  const centralStart = offset;
  let centralSize = 0;
  /** @type {Uint8Array[]} */
  const headers = [];
  for (const entry of central) {
    const header = new Uint8Array(46 + entry.name.length);
    const hv = new DataView(header.buffer);
    hv.setUint32(0, 0x02014b50, true);
    hv.setUint16(4, 20, true);
    hv.setUint16(6, 20, true);
    hv.setUint16(8, 0x0800, true);
    hv.setUint16(10, entry.method, true);
    hv.setUint16(12, DOS_TIME, true);
    hv.setUint16(14, DOS_DATE, true);
    hv.setUint32(16, entry.crc, true);
    hv.setUint32(20, entry.compressed, true);
    hv.setUint32(24, entry.size, true);
    hv.setUint16(28, entry.name.length, true);
    hv.setUint16(30, 0, true);
    hv.setUint16(32, 0, true);
    hv.setUint16(34, 0, true);
    hv.setUint16(36, 0, true);
    hv.setUint32(38, 0, true);
    hv.setUint32(42, entry.offset, true);
    header.set(entry.name, 46);
    headers.push(header);
    centralSize += header.length;
  }

  const eocd = new Uint8Array(22);
  const ev = new DataView(eocd.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(4, 0, true);
  ev.setUint16(6, 0, true);
  ev.setUint16(8, central.length, true);
  ev.setUint16(10, central.length, true);
  ev.setUint32(12, centralSize, true);
  ev.setUint32(16, centralStart, true);
  ev.setUint16(20, 0, true);

  const total = new Uint8Array(centralStart + centralSize + eocd.length);
  let at = 0;
  for (const local of locals) { total.set(local, at); at += local.length; }
  for (const header of headers) { total.set(header, at); at += header.length; }
  total.set(eocd, at);
  return total;
}

/**
 * @param {Uint8Array} archive
 * @returns {{ ok: true, files: Map<string, Uint8Array> } | { ok: false, problems: string[] }}
 */
export function unzipFiles(archive) {
  /** @type {string[]} */
  const problems = [];
  if (!(archive instanceof Uint8Array) || archive.length < 22) {
    return { ok: false, problems: ['archive is too short to be a package'] };
  }
  const view = new DataView(archive.buffer, archive.byteOffset, archive.byteLength);
  let eocd = -1;
  for (let i = archive.length - 22; i >= 0; i--) {
    if (view.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd === -1) return { ok: false, problems: ['no end-of-central-directory record; this is not a package'] };

  const count = view.getUint16(eocd + 10, true);
  const centralSize = view.getUint32(eocd + 12, true);
  const centralStart = view.getUint32(eocd + 16, true);
  if (count > 5000) return { ok: false, problems: [`archive declares ${count} entries, over the 5000 limit`] };
  if (centralStart + centralSize > archive.length) return { ok: false, problems: ['the central directory runs past the archive'] };

  /** @type {Map<string, Uint8Array>} */
  const files = new Map();
  let cursor = centralStart;
  let total = 0;
  for (let i = 0; i < count; i++) {
    if (view.getUint32(cursor, true) !== 0x02014b50) {
      return { ok: false, problems: [`central entry ${i} has a bad signature`] };
    }
    const method = view.getUint16(cursor + 10, true);
    const crc = view.getUint32(cursor + 16, true);
    const compressedSize = view.getUint32(cursor + 20, true);
    const size = view.getUint32(cursor + 24, true);
    const nameLength = view.getUint16(cursor + 28, true);
    const extraLength = view.getUint16(cursor + 30, true);
    const commentLength = view.getUint16(cursor + 32, true);
    const localOffset = view.getUint32(cursor + 42, true);
    const name = new TextDecoder().decode(archive.subarray(cursor + 46, cursor + 46 + nameLength));
    cursor += 46 + nameLength + extraLength + commentLength;

    if (name.length === 0 || name.includes('\0') || name.startsWith('/') || name.split('/').includes('..')) {
      problems.push(`entry '${name}' is not a safe bundle path`);
      continue;
    }
    total += size;
    if (total > 256 * 1024 * 1024) return { ok: false, problems: ['archive expands past the 256 MB limit'] };
    if (view.getUint32(localOffset, true) !== 0x04034b50) {
      problems.push(`entry '${name}' has a bad local header`);
      continue;
    }
    const localName = view.getUint16(localOffset + 26, true);
    const localExtra = view.getUint16(localOffset + 28, true);
    const start = localOffset + 30 + localName + localExtra;
    const payload = archive.subarray(start, start + compressedSize);
    let data;
    try {
      data = method === 0 ? new Uint8Array(payload) : method === 8 ? new Uint8Array(inflateRawSync(payload)) : null;
    } catch (error) {
      problems.push(`entry '${name}' could not be decompressed: ${error instanceof Error ? error.message : String(error)}`);
      continue;
    }
    if (!data) {
      problems.push(`entry '${name}' uses unsupported compression method ${method}`);
      continue;
    }
    if (data.length !== size || crc32(data) !== crc) {
      problems.push(`entry '${name}' failed its size or checksum`);
      continue;
    }
    files.set(name, data);
  }
  if (problems.length > 0) return { ok: false, problems };
  return { ok: true, files };
}

/**
 * Package a project. Deterministic: the same bundle produces the same bytes.
 * @param {any} bundle
 * @returns {{ ok: true, bytes: Uint8Array, files: Map<string, string|Uint8Array> } | { ok: false, problems: import('../../shared/domain/validate.js').Problem[] }}
 */
export function packageProject(bundle) {
  const serialized = serialize(bundle);
  if (!serialized.ok) return serialized;
  const manifest = JSON.parse(/** @type {string} */ (serialized.files.get(MANIFEST)));
  manifest.archiveVersion = ARCHIVE_VERSION;
  serialized.files.set(MANIFEST, `${JSON.stringify(manifest, null, 2)}\n`);
  return { ok: true, bytes: zipFiles(serialized.files), files: serialized.files };
}

/**
 * Unpack a package into the file set a bundle reader consumes.
 * @param {Uint8Array} bytes
 * @returns {{ ok: true, files: Map<string, string> } | { ok: false, problems: string[] }}
 */
export function unpackProject(bytes) {
  const unzipped = unzipFiles(bytes);
  if (!unzipped.ok) return unzipped;
  /** @type {Map<string, string>} */
  const files = new Map();
  const decoder = new TextDecoder();
  for (const [name, data] of unzipped.files) {
    // Asset bytes are copied separately by the caller when they need to stay binary; the
    // bundle reader only wants the JSON files, and assets are referenced by hash.
    files.set(name, decoder.decode(data));
  }
  const parsed = deserialize(files);
  if (!parsed.ok) {
    return { ok: false, problems: [`the package is not a comic bundle: missing ${parsed.missing.join(', ')}`] };
  }
  return { ok: true, files };
}
