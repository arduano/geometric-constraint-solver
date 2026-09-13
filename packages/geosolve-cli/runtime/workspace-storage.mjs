// SPDX-License-Identifier: GPL-3.0-or-later
// Linux recoverable file publication. This is not atomic multi-file CAS: independent
// editors may retain writable old descriptors. Their displaced inodes remain inspectable.
import {
  closeSync, fchmodSync, fsyncSync, fstatSync, linkSync, lstatSync, mkdirSync, openSync,
  readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, unlinkSync, writeFileSync,
} from "node:fs";
import { createHash, randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";

const FILE_LIMIT = 16 * 1024 * 1024;
const TRANSACTION_LIMIT = 64 * 1024 * 1024;
const MAX_FILES = 256;
const heldLocks = new Map();
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const absent = (error) => error.code === "ENOENT";
const conflict = (message) => Object.assign(new Error(message), { code: "WORKSPACE_CONFLICT", conflict: true });
const inode = (stat) => `${stat.dev}:${stat.ino}`;

function syncDirectory(path) {
  const fd = openSync(path, "r");
  try { fsyncSync(fd); } finally { closeSync(fd); }
}

function syncFile(path) {
  const fd = openSync(path, "r");
  try { fsyncSync(fd); } finally { closeSync(fd); }
}

function ensureDirectory(path) {
  try { mkdirSync(path, { mode: 0o700 }); syncDirectory(dirname(path)); }
  catch (error) { if (error.code !== "EEXIST") throw error; }
  const stat = lstatSync(path);
  if (!stat.isDirectory() || stat.isSymbolicLink()) throw Error(`Expected local directory: ${path}`);
}

function writeDurable(path, bytes, mode = 0o600) {
  const fd = openSync(path, "wx", mode);
  try { writeFileSync(fd, bytes); fchmodSync(fd, mode); fsyncSync(fd); }
  finally { closeSync(fd); }
}

function writeJson(path, value) {
  const temporary = `${path}.${randomUUID()}.tmp`;
  try {
    writeDurable(temporary, JSON.stringify(value, null, 2) + "\n");
    renameSync(temporary, path);
    syncDirectory(dirname(path));
  } finally { try { unlinkSync(temporary); } catch (error) { if (!absent(error)) throw error; } }
}

function readRegular(path) {
  let stat;
  try { stat = lstatSync(path); } catch (error) { if (absent(error)) return null; throw error; }
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > FILE_LIMIT) throw Error(`Expected regular file <= ${FILE_LIMIT} bytes: ${path}`);
  const fd = openSync(path, "r");
  try {
    const before = fstatSync(fd);
    if (inode(before) !== inode(stat)) throw conflict(`File replaced during read: ${path}`);
    const bytes = readFileSync(fd);
    const after = fstatSync(fd);
    const current = lstatSync(path);
    if (inode(after) !== inode(current) || before.size !== after.size || before.mtimeMs !== after.mtimeMs
        || before.ctimeMs !== after.ctimeMs || bytes.length > FILE_LIMIT) throw conflict(`File changed during read: ${path}`);
    return { hash: digest(bytes), bytes, size: bytes.length, mode: after.mode & 0o777, inode: inode(after) };
  } finally { closeSync(fd); }
}

function summary(file) {
  if (!file) return { hash: null, size: 0, mode: null, inode: null };
  const { bytes: _bytes, ...result } = file;
  return result;
}

function processIdentity(pid) {
  try {
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    // comm (field 2) may itself contain spaces or parentheses.
    const fields = stat.slice(stat.lastIndexOf(")") + 2).trim().split(/\s+/);
    return { pid, startTicks: fields[19], bootId: readFileSync("/proc/sys/kernel/random/boot_id", "utf8").trim() };
  } catch (error) { if (absent(error) || error.code === "ESRCH") return null; throw error; }
}

function ownerAlive(owner) {
  if (!owner || !Number.isInteger(owner.pid) || owner.pid < 1) return false;
  const current = processIdentity(owner.pid);
  return !!current && current.startTicks === owner.startTicks && current.bootId === owner.bootId;
}

function cacheDirectory(folder) {
  const canonical = realpathSync(folder);
  const cache = join(canonical, ".geosolve");
  ensureDirectory(cache);
  return { canonical, cache };
}

/** Linux advisory bridge ownership. The never-unlinked flock inode is held by this
 * process, not a helper daemon; process death releases it even if owner JSON is stale.
 * Requires the standard util-linux `flock` executable. Never signals another process. */
export function acquireWorkspaceLock(folder) {
  if (process.platform !== "linux") throw Error("Local workspace locking currently requires Linux");
  const { canonical, cache } = cacheDirectory(folder);
  if (heldLocks.has(canonical)) throw Object.assign(Error("This process already owns the workspace bridge"), { code: "WORKSPACE_LOCKED" });
  const lockPath = join(cache, "bridge.lock");
  try { if (!lstatSync(lockPath).isFile()) throw Error("Bridge lock must be a regular file"); }
  catch (error) { if (!absent(error)) throw error; }
  const fd = openSync(lockPath, "a+", 0o600);
  const ownerPath = join(cache, "bridge-owner.json");
  let previous = null;
  try {
    try { previous = JSON.parse(readRegular(ownerPath)?.bytes.toString("utf8") ?? "null"); } catch { /* Derived owner metadata is advisory. */ }
    const result = spawnSync("flock", ["--exclusive", "--nonblock", "3"], { stdio: ["ignore", "pipe", "pipe", fd] });
    if (result.error) throw Error(`Linux workspace locking needs util-linux flock: ${result.error.message}`);
    if (result.status !== 0) throw Object.assign(Error("Workspace bridge already has an owner"), { code: "WORKSPACE_LOCKED", owner: previous });
    const identity = { ...processIdentity(process.pid), token: randomUUID(), folder: canonical };
    const owner = { ...identity, acquiredAt: new Date().toISOString() };
    writeJson(ownerPath, owner);
    const lock = {
      folder: canonical, owner, previousOwner: previous,
      staleOwnerRecovered: !!previous && !ownerAlive(previous),
      assertHeld() {
        if (heldLocks.get(canonical) !== lock) throw Error("Workspace lock has been released");
        if (inode(fstatSync(fd)) !== inode(lstatSync(lockPath))) throw Error("Workspace lock file was replaced; restart the bridge");
      },
      release() {
        if (heldLocks.get(canonical) !== lock) return;
        try {
          let current;
          try { current = JSON.parse(readRegular(ownerPath)?.bytes.toString("utf8") ?? "null"); } catch { /* Preserve malformed competing metadata. */ }
          if (current?.token === identity.token) { unlinkSync(ownerPath); syncDirectory(cache); }
        } finally { heldLocks.delete(canonical); closeSync(fd); }
      },
    };
    heldLocks.set(canonical, lock);
    return lock;
  } catch (error) { closeSync(fd); throw error; }
}

function projectPath(folder, relative) {
  if (typeof relative !== "string" || !relative || relative.includes("\\") || relative.startsWith("/")
      || relative.split("/").some((part) => !part || part === "." || part === "..")
      || (relative.startsWith(".geosolve/") && relative !== ".geosolve/design.json" && relative !== ".geosolve/inputs.json")) {
    throw Error(`Invalid project publication path: ${relative}`);
  }
  const path = resolve(folder, relative);
  let parent = dirname(path);
  while (parent !== folder) {
    const stat = lstatSync(parent);
    if (!stat.isDirectory() || stat.isSymbolicLink()) throw Error(`Project parent must be a local directory: ${parent}`);
    parent = dirname(parent);
  }
  return path;
}

function validateId(id) {
  if (typeof id !== "string" || !/^[A-Za-z0-9_-]{1,100}$/.test(id)) throw Error("Operation ID must have 1–100 ASCII letters, numbers, underscores or hyphens");
}

/** Check same-user editors before pruning: a displaced inode may still have an
 * external writable FD. An inaccessible same-user /proc scan conservatively retains
 * bytes. Cross-user/privileged writers are outside this local-user retention check. */
function writableDescriptors(inodes) {
  const held = new Set();
  let complete = true;
  for (const processName of readdirSync("/proc").filter((name) => /^\d+$/.test(name))) {
    let descriptors;
    try {
      if (statSync(`/proc/${processName}`).uid !== process.getuid()) continue;
      descriptors = readdirSync(`/proc/${processName}/fd`);
    }
    catch (error) { if (!absent(error)) complete = false; continue; }
    for (const name of descriptors) {
      try {
        const key = inode(statSync(`/proc/${processName}/fd/${name}`));
        if (!inodes.has(key)) continue;
        const info = readFileSync(`/proc/${processName}/fdinfo/${name}`, "utf8");
        const flags = Number.parseInt(/^flags:\s+([0-7]+)$/m.exec(info)?.[1] ?? "0", 8);
        if ((flags & 3) !== 0) held.add(key);
      } catch (error) { if (!absent(error)) complete = false; }
    }
  }
  return { held, complete };
}

/** Recoverable project publication with explicit acknowledgment. Caller must own the
 * canonical bridge lock. All methods are synchronous, giving each Node request one
 * ordered publication boundary. `fault(point, context)` exists for deterministic tests. */
export function createWorkspaceStorage(folder, { lock, historyLimit = 32, fault = () => {} } = {}) {
  const { canonical, cache } = cacheDirectory(folder);
  if (!lock || lock.folder !== canonical) throw Error("Workspace storage requires its canonical bridge lock");
  if (!Number.isSafeInteger(historyLimit) || historyLimit < 0 || historyLimit > 10000) throw Error("Invalid history limit");
  lock.assertHeld();
  const operations = join(cache, "operations");
  ensureDirectory(operations);
  const operationDirectory = (id) => { validateId(id); return join(operations, id); };
  const save = (record) => writeJson(join(operationDirectory(record.operationId), "manifest.json"), record);
  const load = (id) => {
    const directory = operationDirectory(id);
    try { if (!lstatSync(directory).isDirectory() || lstatSync(directory).isSymbolicLink()) throw Error("Journal must be a local directory"); }
    catch (error) { if (absent(error)) return null; throw error; }
    const raw = readRegular(join(directory, "manifest.json"));
    if (!raw) return null;
    const record = JSON.parse(raw.bytes.toString("utf8"));
    if (record.format !== "geosolve-publication-v1" || record.operationId !== id || !Array.isArray(record.files)
        || record.files.length > MAX_FILES || (!record.files.length && !record.pruned)
        || !["staging", "staged", "published", "acknowledged", "conflict", "resolved"].includes(record.state)
        || !Array.isArray(record.expectedInputs) || record.expectedInputs.length > MAX_FILES) throw Error(`Invalid publication journal: ${id}`);
    for (const file of [...record.files, ...record.expectedInputs]) {
      projectPath(canonical, file.path);
      if (file.expectedHash !== null && !/^[a-f0-9]{64}$/.test(file.expectedHash ?? "")) throw Error(`Invalid expected hash in journal: ${id}`);
    }
    for (const file of record.files) {
      if (file.candidateHash !== null && !/^[a-f0-9]{64}$/.test(file.candidateHash ?? "")) throw Error(`Invalid candidate hash in journal: ${id}`);
    }
    return record;
  };
  const paths = (record, index) => ({
    current: projectPath(canonical, record.files[index].path),
    candidate: join(operationDirectory(record.operationId), `candidate-${index}`),
    publication: join(operationDirectory(record.operationId), `publication-${index}`),
    original: join(operationDirectory(record.operationId), `original-${index}`),
    before: join(operationDirectory(record.operationId), `before-${index}`),
  });
  const inspectRecord = (record) => ({
    ...record,
    files: record.files.map((file, index) => {
      const locations = paths(record, index);
      return { ...file, locations, current: summary(readRegular(locations.current)),
        before: summary(readRegular(locations.before)), candidate: summary(readRegular(locations.candidate)),
        original: summary(readRegular(locations.original)), publication: summary(readRegular(locations.publication)) };
    }),
  });
  const allPublished = (inspection) => inspection.files.every((file) => file.current.hash === file.candidateHash
    && (file.expectedHash === null || file.before.hash === file.expectedHash)
    && (file.candidateHash === null || file.candidate.hash === file.candidateHash))
    && inspection.expectedInputs.every((input) => {
      const edited = inspection.files.find((file) => file.path === input.path);
      return (readRegular(projectPath(canonical, input.path))?.hash ?? null) === (edited ? edited.candidateHash : input.expectedHash);
    });
  const inspect = (id) => { lock.assertHeld(); const record = load(id); return record && inspectRecord(record); };
  const finishPruning = (record) => {
    for (const name of readdirSync(operationDirectory(record.operationId))) {
      if (name !== "manifest.json") rmSync(join(operationDirectory(record.operationId), name), { recursive: true });
    }
    syncDirectory(operationDirectory(record.operationId));
  };
  const list = () => {
    lock.assertHeld();
    return readdirSync(operations).sort().flatMap((id) => {
      try { const record = load(id); return record ? [inspectRecord(record)] : [{ operationId: id, state: "unreadable", error: "Missing manifest; retained for manual inspection" }]; }
      catch (error) { return [{ operationId: id, state: "unreadable", error: String(error) }]; }
    });
  };
  const reconcile = () => {
    lock.assertHeld();
    for (const item of list()) {
      if (item.state === "unreadable") continue;
      const record = load(item.operationId);
      if (record.pruned) { finishPruning(record); continue; }
      if (record.state === "acknowledged" || record.state === "resolved" || record.state === "conflict") continue;
      if (allPublished(item) && record.state !== "staging") {
        record.state = "published";
        record.recovered = true;
      } else {
        // No overwritten live pathname, even if some siblings were already published.
        // Only restore missing originals via an exclusive link to the retained inode.
        for (let i = 0; i < record.files.length; i++) {
          const locations = paths(record, i);
          if (readRegular(locations.current) === null && readRegular(locations.before) !== null) {
            try { linkSync(locations.before, locations.current); syncDirectory(dirname(locations.current)); }
            catch (error) { if (error.code !== "EEXIST") throw error; }
          }
        }
        record.state = "conflict";
        record.error = "Interrupted publication retained; inspect files and resolve explicitly";
      }
      save(record);
    }
    return list();
  };
  const publish = ({ operationId, files, expectedInputs = [], requestDigest }) => {
    lock.assertHeld();
    validateId(operationId);
    if (requestDigest !== undefined && !/^[a-f0-9]{64}$/.test(requestDigest)) throw Error("Invalid operation request digest");
    if (!Array.isArray(files) || !files.length || files.length > MAX_FILES) throw Error(`Expected 1–${MAX_FILES} publication files`);
    const prepared = files.map((file) => {
      projectPath(canonical, file.path);
      if (file.expectedHash !== null && !/^[a-f0-9]{64}$/.test(file.expectedHash ?? "")) throw Error("Every file needs an explicit expectedHash (null means absent)");
      if (file.bytes !== null && typeof file.bytes !== "string" && !(file.bytes instanceof Uint8Array)) throw Error("Publication bytes must be text, Uint8Array, or null for deletion");
      const bytes = file.bytes === null ? null : Buffer.from(file.bytes);
      if (bytes && bytes.length > FILE_LIMIT) throw Error("Publication file exceeds byte limit");
      return { path: file.path, expectedHash: file.expectedHash, bytes, candidateHash: bytes === null ? null : digest(bytes) };
    }).sort((a, b) => a.path.localeCompare(b.path));
    if (prepared.reduce((size, file) => size + (file.bytes?.length ?? 0), 0) > TRANSACTION_LIMIT) throw Error("Publication exceeds 64 MiB transaction limit");
    if (new Set(prepared.map((file) => file.path)).size !== prepared.length) throw Error("Duplicate publication path");
    if (!Array.isArray(expectedInputs) || expectedInputs.length > MAX_FILES) throw Error("Invalid expected project inputs");
    const dependencies = expectedInputs.map(({ path, expectedHash }) => {
      projectPath(canonical, path);
      if (expectedHash !== null && !/^[a-f0-9]{64}$/.test(expectedHash ?? "")) throw Error("Every input needs an explicit expectedHash");
      return { path, expectedHash };
    }).sort((a, b) => a.path.localeCompare(b.path));
    if (new Set(dependencies.map((file) => file.path)).size !== dependencies.length) throw Error("Duplicate expected input path");
    for (const input of dependencies) {
      const edited = prepared.find((file) => file.path === input.path);
      if (edited && edited.expectedHash !== input.expectedHash) throw Error("Inconsistent edited input hash");
    }
    const checkDependencies = (afterPublication = false) => {
      for (const file of dependencies) {
        const edited = afterPublication && prepared.find((edited) => edited.path === file.path);
        if ((readRegular(projectPath(canonical, file.path))?.hash ?? null) !== (edited ? edited.candidateHash : file.expectedHash)) throw conflict(`Stale project input: ${file.path}`);
      }
    };
    const requestHash = digest(JSON.stringify({ files: prepared.map(({ bytes: _bytes, ...file }) => file), expectedInputs: dependencies, requestDigest }));
    const existing = load(operationId);
    if (existing) {
      if (existing.requestHash !== requestHash) throw conflict("Operation ID was already used for different input");
      return inspectRecord(existing);
    }
    const record = { format: "geosolve-publication-v1", operationId, requestHash, requestDigest,
      state: "staging", createdAt: new Date().toISOString(),
      expectedInputs: dependencies,
      files: prepared.map(({ bytes: _bytes, ...file }) => ({ ...file, mode: 0o600 })) };
    const directory = operationDirectory(operationId);
    mkdirSync(directory, { mode: 0o700 }); syncDirectory(operations);
    save(record);
    try {
      checkDependencies();
      for (let i = 0; i < prepared.length; i++) {
        const locations = paths(record, i);
        const current = readRegular(locations.current);
        if ((current?.hash ?? null) !== prepared[i].expectedHash) throw conflict(`Stale input: ${prepared[i].path}`);
        record.files[i].mode = current?.mode ?? 0o600;
        if (current) writeDurable(locations.original, current.bytes, current.mode);
        if (prepared[i].bytes !== null) {
          // Keep the intended bytes independent of the live publication inode: an
          // external in-place save must not destroy the staged candidate evidence.
          writeDurable(locations.candidate, prepared[i].bytes, record.files[i].mode);
          writeDurable(locations.publication, prepared[i].bytes, record.files[i].mode);
        }
        fault("staging", { operationId, index: i, ...locations });
      }
      syncDirectory(directory);
      record.state = "staged"; save(record);
      fault("staged", { operationId });
      checkDependencies();
      // Check the complete input set again before the first displacement.
      for (const file of prepared) if ((readRegular(projectPath(canonical, file.path))?.hash ?? null) !== file.expectedHash) throw conflict(`Stale input: ${file.path}`);
      for (let i = 0; i < prepared.length; i++) {
        const locations = paths(record, i);
        const file = prepared[i];
        if ((readRegular(locations.current)?.hash ?? null) !== file.expectedHash) throw conflict(`Stale input: ${file.path}`);
        fault("before-displace", { operationId, index: i, ...locations });
        if (file.expectedHash !== null) {
          renameSync(locations.current, locations.before);
          syncFile(locations.before);
          syncDirectory(dirname(locations.current)); syncDirectory(directory);
          fault("displaced", { operationId, index: i, ...locations });
          if (readRegular(locations.before)?.hash !== file.expectedHash) throw conflict(`External write during displacement: ${file.path}`);
        }
        if (file.bytes !== null) linkSync(locations.publication, locations.current);
        syncDirectory(dirname(locations.current));
        fault("published-file", { operationId, index: i, ...locations });
      }
      if (!allPublished(inspectRecord(record))) throw conflict("External write during publication; all alternatives retained");
      checkDependencies(true);
      record.state = "published"; save(record);
      fault("published", { operationId });
      return inspectRecord(record);
    } catch (error) {
      // Faults intentionally model abrupt death and leave the last durable state.
      if (error.simulateCrash) throw error;
      record.state = "conflict"; record.error = String(error); save(record);
      for (let i = 0; i < record.files.length; i++) {
        const locations = paths(record, i);
        if (readRegular(locations.current) === null && readRegular(locations.before) !== null) {
          try { linkSync(locations.before, locations.current); syncDirectory(dirname(locations.current)); }
          catch (restoreError) { if (restoreError.code !== "EEXIST") record.restoreError = String(restoreError); }
        }
      }
      save(record);
      throw Object.assign(error, { operationId, recovery: inspectRecord(record) });
    }
  };
  const acknowledge = (operationId) => {
    lock.assertHeld();
    const record = load(operationId);
    if (!record) throw Error("Unknown operation ID");
    if (record.state === "acknowledged") return inspectRecord(record);
    if (record.state !== "published" || !allPublished(inspectRecord(record))) throw conflict("Publication changed or is incomplete; inspect and resolve before acknowledgment");
    fault("before-acknowledge", { operationId });
    if (!allPublished(inspectRecord(record))) throw conflict("Publication changed before acknowledgment; all alternatives retained");
    record.state = "acknowledged"; record.acknowledgedAt = new Date().toISOString(); save(record);
    fault("acknowledged", { operationId });
    return inspectRecord(record);
  };
  const resolveRecovery = ({ operationId, expectedHashes, action = "keep-current" }) => {
    lock.assertHeld();
    if (action !== "keep-current") throw Error("Recovery resolution supports keep-current; publish inspected alternatives as a new revision-checked operation");
    const record = load(operationId);
    if (!record) throw Error("Unknown operation ID");
    const inspection = inspectRecord(record);
    if (!expectedHashes || Object.keys(expectedHashes).length !== record.files.length
        || inspection.files.some((file) => !Object.hasOwn(expectedHashes, file.path) || expectedHashes[file.path] !== file.current.hash)) throw conflict("Recovery resolution requires the exact current file hashes");
    record.state = "resolved"; record.resolvedAt = new Date().toISOString(); record.resolution = "keep-current";
    record.retainedHashes = inspection.files.map((file) => ({ before: file.before.hash, candidate: file.candidate.hash,
      original: file.original.hash, publication: file.publication.hash }));
    save(record);
    return inspectRecord(record);
  };
  const prune = ({ allowUnverifiedDescriptors = false } = {}) => {
    lock.assertHeld();
    const terminal = list().filter((item) => !item.pruned && ["acknowledged", "resolved"].includes(item.state))
      .sort((a, b) => (a.acknowledgedAt ?? a.resolvedAt).localeCompare(b.acknowledgedAt ?? b.resolvedAt));
    const candidates = terminal.slice(0, Math.max(0, terminal.length - historyLimit));
    const scan = writableDescriptors(new Set(candidates.flatMap((item) => item.files.flatMap((file) => [file.before.inode, file.candidate.inode, file.publication.inode, file.original.inode]).filter(Boolean))));
    const removed = [], deferred = [];
    for (const item of candidates) {
      const changed = item.files.some((file, i) => {
        const expected = item.retainedHashes?.[i] ?? { before: file.expectedHash, candidate: file.candidateHash,
          original: file.expectedHash, publication: file.candidateHash };
        return ["before", "candidate", "original", "publication"].some((name) => file[name].hash !== expected[name]);
      });
      const held = item.files.some((file) => ["before", "candidate", "original", "publication"].some((name) => scan.held.has(file[name].inode)));
      if (changed || held || (!scan.complete && !allowUnverifiedDescriptors)) {
        deferred.push({ operationId: item.operationId, reason: changed ? "retained-bytes-changed" : held ? "writable-descriptor-open" : "descriptor-scan-incomplete" });
        continue;
      }
      // Keep a small durable idempotency receipt forever: pruning large file history
      // must never allow a delayed retry to execute an acknowledged operation again.
      const record = load(item.operationId);
      record.publishedHashes = Object.fromEntries(item.files.map((file) => [file.path, file.candidateHash]));
      record.files = []; record.pruned = true; save(record);
      fault("pruned-receipt", { operationId: item.operationId });
      finishPruning(record);
      removed.push(item.operationId);
    }
    return { removed, deferred, descriptorScanComplete: scan.complete };
  };
  return { folder: canonical, operations, publish, acknowledge, inspect, list, reconcile,
    outcome: inspect, resolve: resolveRecovery, prune,
    read: (path) => { lock.assertHeld(); return readRegular(projectPath(canonical, path)); } };
}
