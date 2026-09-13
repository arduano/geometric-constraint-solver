// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash, randomBytes } from "node:crypto";
import { constants } from "node:fs";
import { lstat, mkdir, open, realpath, rename, unlink } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

const format = "geosolve-collaboration-storage-v1";
const magic = Buffer.from("GSCADJ01\n");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const defaultLimits = Object.freeze({ maxBlobBytes: 64 * 1024 * 1024, maxRecordBytes: 1024 * 1024, maxJournalBytes: 64 * 1024 * 1024, maxRecords: 100_000, maxAttachments: 32 });
function encode(value) { return Buffer.from(JSON.stringify(value)); }
function digest(value) { if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) throw Error("Invalid collaboration content digest"); return value; }
async function syncDirectory(path) { const directory = await open(path, constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW); try { await directory.sync(); } finally { await directory.close(); } }
async function safeDirectory(path) {
  let created = false;
  try { await mkdir(path, { mode: 0o700 }); created = true; } catch (error) { if (error.code !== "EEXIST") throw error; }
  const stat = await lstat(path);
  if (!stat.isDirectory() || stat.isSymbolicLink()) throw Error("Collaboration storage must use regular local directories");
  if (created) await syncDirectory(dirname(path));
}
async function readBounded(path, limit) {
  const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const stat = await file.stat();
    if (!stat.isFile() || stat.size > limit) throw Error("Collaboration file exceeds its byte limit or is not regular");
    const bytes = Buffer.alloc(stat.size);
    let offset = 0;
    while (offset < bytes.length) {
      const { bytesRead } = await file.read(bytes, offset, bytes.length-offset, offset);
      if (!bytesRead) throw Error("Collaboration file changed during bounded read");
      offset += bytesRead;
    }
    const extra = Buffer.alloc(1);
    if ((await file.read(extra, 0, 1, offset)).bytesRead) throw Error("Collaboration file grew during bounded read");
    return bytes;
  } finally { await file.close(); }
}
function frozen(value) { if (value && typeof value === "object") { Object.values(value).forEach(frozen); Object.freeze(value); } return value; }
function limitsOf(overrides) {
  const limits = { ...defaultLimits, ...overrides };
  if (Object.keys(limits).some((key) => !Object.hasOwn(defaultLimits, key)) || Object.values(limits).some((value) => !Number.isSafeInteger(value) || value < 1)) throw Error("Invalid collaboration storage limits");
  return limits;
}

/** Reference durable transport storage. Caller holds the existing exclusive
 * canonical folder lock for its whole lifetime. Native core/engine replay owns
 * semantic authentication; this layer only preserves complete bytes and order.
 */
export async function openCollaborationStorage(folder, { create = false, limits: overrides, fault = async () => {} } = {}) {
  const limits = limitsOf(overrides);
  const canonical = await realpath(folder);
  if (resolve(folder) !== canonical) throw Error("Collaboration storage requires its canonical locked folder");
  const metadata = join(canonical, ".geosolve");
  const directory = join(metadata, "collaboration");
  const blobs = join(directory, "blobs");
  if (create) { await safeDirectory(metadata); await safeDirectory(directory); await safeDirectory(blobs); }
  for (const path of [metadata, directory, blobs]) {
    const stat = await lstat(path);
    if (!stat.isDirectory() || stat.isSymbolicLink()) throw Error("Collaboration storage directory is invalid");
  }
  const journalPath = join(directory, "operations.journal");
  if (create) {
    let created;
    try { created = await open(journalPath, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600); }
    catch (error) { if (error.code !== "EEXIST") throw error; }
    if (created) {
      try { await created.writeFile(magic); await created.sync(); } finally { await created.close(); }
      await syncDirectory(directory);
    }
  }
  const bytes = await readBounded(journalPath, limits.maxJournalBytes);
  if (!bytes.subarray(0, magic.length).equals(magic)) throw Error("Corrupt collaboration journal header; explicit recovery required");
  const records = [];
  let offset = magic.length;
  let previousDigest = "";
  const verifiedBlobs = new Set();
  async function readBlob(key) {
    const bytes = await readBounded(join(blobs, `${digest(key)}.blob`), limits.maxBlobBytes);
    if (hash(bytes) !== key) throw Error("Corrupt collaboration checkpoint; explicit recovery required");
    return bytes;
  }
  while (offset < bytes.length) {
    if (bytes.length-offset < 4) throw Error("Partial collaboration journal length; explicit recovery required");
    const length = bytes.readUInt32LE(offset); offset += 4;
    if (!length || length > limits.maxRecordBytes || length > bytes.length-offset) throw Error("Partial or oversized collaboration journal record; explicit recovery required");
    let record;
    try { record = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(offset, offset+length))); }
    catch { throw Error("Corrupt collaboration journal JSON; explicit recovery required"); }
    offset += length;
    if (records.length >= limits.maxRecords || !record || Object.keys(record).sort().join(",") !== "attachments,digest,format,payload,previousDigest,sequence" || record.format !== format || record.sequence !== records.length+1 || record.previousDigest !== previousDigest || !record.attachments || Array.isArray(record.attachments) || typeof record.attachments !== "object" || Object.keys(record.attachments).length > limits.maxAttachments) throw Error("Corrupt collaboration record chain; explicit recovery required");
    const { digest: recordedDigest, ...body } = record;
    if (hash(encode(body)) !== recordedDigest) throw Error("Corrupt collaboration record digest; explicit recovery required");
    for (const [name, key] of Object.entries(record.attachments)) {
      validateName(name); digest(key);
      if (!verifiedBlobs.has(key)) { await readBlob(key); verifiedBlobs.add(key); }
    }
    previousDigest = recordedDigest;
    records.push(frozen(record));
  }
  const file = await open(journalPath, constants.O_WRONLY | constants.O_APPEND | constants.O_NOFOLLOW);
  let totalBytes = bytes.length;
  let poisoned = false;
  let writing = false;
  let closed = false;
  async function writeBlob(value) {
    if (!(value instanceof Uint8Array)) throw Error("Collaboration checkpoint must contain exact bytes");
    if (value.byteLength > limits.maxBlobBytes) throw Error("Collaboration checkpoint exceeds byte limit");
    const bytes = Buffer.from(value); const key = hash(bytes); const destination = join(blobs, `${key}.blob`);
    if (verifiedBlobs.has(key)) return key;
    try { await readBlob(key); verifiedBlobs.add(key); return key; } catch (error) { if (error.code !== "ENOENT") throw error; }
    const temporary = join(blobs, `.stage-${randomBytes(16).toString("hex")}`);
    const handle = await open(temporary, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
    try {
      await handle.writeFile(bytes); await fault("blob-written"); await handle.sync();
      await fault("blob-synced");
      await handle.close();
      // Caller has exclusive document lock; concurrent external bytes are still
      // never accepted without content verification during restore/read.
      await rename(temporary, destination); await syncDirectory(blobs);
      verifiedBlobs.add(key); return key;
    } finally { await handle.close().catch(() => {}); await unlink(temporary).catch((error) => { if (error.code !== "ENOENT") throw error; }); }
  }
  return {
    directory,
    records: () => records.slice(),
    get latestSequence() { return records.length; },
    get needsRecovery() { return poisoned; },
    readBlob,
    /** Flush immutable attachments before appending the envelope. Any uncertain
     * write prevents further appends until explicit reopen/replay. No ACK escapes
     * this method before fsync, and no truncation is performed on errors.
     */
    async append(payload, attachments = {}) {
      if (writing) throw Error("Only one collaboration journal append may be staged at a time");
      if (closed || poisoned) throw Error("Collaboration storage needs reopen/recovery");
      if (records.length >= limits.maxRecords || !attachments || Array.isArray(attachments) || Object.keys(attachments).length > limits.maxAttachments) throw Error("Collaboration storage record/attachment limit");
      const capturedPayload = JSON.parse(JSON.stringify(payload));
      if (encode(capturedPayload).length > limits.maxRecordBytes) throw Error("Collaboration payload exceeds record limit");
      const captured = Object.entries(attachments).map(([name, value]) => { validateName(name); if (!(value instanceof Uint8Array) || value.byteLength > limits.maxBlobBytes) throw Error("Invalid collaboration attachment"); return [name, Buffer.from(value)]; });
      writing = true;
      try {
        const references = {};
        for (const [name, value] of captured) references[name] = await writeBlob(value);
        const body = { format, sequence: records.length+1, previousDigest, payload: capturedPayload, attachments: references };
        const record = { ...body, digest: hash(encode(body)) };
        const bytes = encode(record);
        if (bytes.length > limits.maxRecordBytes || totalBytes+4+bytes.length > limits.maxJournalBytes) throw Error("Collaboration journal byte limit");
        const header = Buffer.alloc(4); header.writeUInt32LE(bytes.length);
        await fault("before-append");
        await file.writeFile(Buffer.concat([header, bytes]));
        await fault("journal-written");
        await file.sync();
        await fault("journal-synced");
        totalBytes += 4+bytes.length; previousDigest = record.digest;
        records.push(frozen(record)); poisoned = false;
        return records.at(-1);
      } catch (error) { poisoned = true; throw error; }
      finally { writing = false; }
    },
    async close() { if (writing) throw Error("Await pending collaboration append before closing storage"); if (!closed) { closed = true; await file.close(); } },
  };
}
function validateName(name) { if (typeof name !== "string" || !/^[a-z][a-zA-Z0-9_-]{0,63}$/u.test(name)) throw Error("Invalid checkpoint attachment name"); }
