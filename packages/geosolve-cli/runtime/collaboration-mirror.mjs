// SPDX-License-Identifier: GPL-3.0-or-later
// Explicit external mirror. Caller holds the workspace lock and supplies the
// authenticated, durable text gateway; this module never invokes a model worker.
import { createHash, randomUUID } from "node:crypto";
import { constants } from "node:fs";
import { lstat, mkdir, open, readdir, realpath, rename, link, unlink } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { collaborationModuleUrl } from "./workspace-runtime-paths.mjs";
const { createSharedText } = await import(collaborationModuleUrl);

const FORMAT = "geosolve-collaboration-mirror-v1";
const MAX_FILE = 4 * 1024 * 1024, MAX_TREE = 16 * 1024 * 1024, MAX_CHECKPOINT = 64 * 1024 * 1024;
const MAX_MANIFEST = 40 * 1024 * 1024;
const MAX_PRESERVED_BYTES = 256 * 1024 * 1024, MAX_PRESERVED_FILES = 8192;
const ignored = new Set([".geosolve", ".git", "node_modules", "target", "dist"]);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const text = bytes => new TextDecoder("utf-8", { fatal: true }).decode(bytes);
const absent = error => error.code === "ENOENT";
function authored(path) {
  return typeof path === "string" && Buffer.byteLength(path) <= 512 && !/[\\:\u0000-\u001f\u007f]/u.test(path)
    && path.split("/").every(part => part && part !== "." && part !== ".." && !ignored.has(part))
    && (path === "geosolve.json" || /\.[cm]?[jt]s$/u.test(path));
}
function checkPath(path) { if (!authored(path)) throw Error(`Invalid authored mirror path: ${path}`); }
function checkIdentity(value) { if (typeof value !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/u.test(value)) throw Error("Invalid mirror identity"); }
async function syncDirectory(path) { const handle = await open(path, "r"); try { await handle.sync(); } finally { await handle.close(); } }
async function exists(path) { try { return await lstat(path); } catch (error) { if (absent(error)) return null; throw error; } }
async function readRegular(path, limit) {
  let handle;
  try { handle = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW); }
  catch (error) { if (absent(error)) return null; throw error; }
  try {
    const before = await handle.stat();
    if (!before.isFile() || before.size > limit) throw Error(`Invalid bounded regular file: ${path}`);
    const buffer = Buffer.alloc(before.size + 1); let length = 0;
    while (length < buffer.length) { const result = await handle.read(buffer, length, buffer.length - length, length); if (!result.bytesRead) break; length += result.bytesRead; }
    const bytes = buffer.subarray(0, length);
    const after = await handle.stat();
    const current = await lstat(path);
    if (bytes.length > limit || bytes.length !== before.size || before.size !== after.size || before.mtimeMs !== after.mtimeMs || before.ctimeMs !== after.ctimeMs
      || !current.isFile() || current.isSymbolicLink() || current.dev !== after.dev || current.ino !== after.ino) throw Error(`File changed while reading: ${path}`);
    return { bytes, blob: hash(bytes), device: String(after.dev), inode: String(after.ino) };
  } finally { await handle.close(); }
}
async function ensureDirectory(path) {
  const entry = await exists(path);
  if (entry) { if (!entry.isDirectory() || entry.isSymbolicLink()) throw Error(`Mirror directory is not a real directory: ${path}`); return; }
  await mkdir(path);
  await syncDirectory(dirname(path));
}
async function parents(root, relative, create = false) {
  let path = root;
  for (const part of relative.split("/").slice(0, -1)) {
    path = join(path, part);
    if (create) await ensureDirectory(path);
    else {
      const entry = await exists(path);
      if (!entry) return false;
      if (!entry.isDirectory() || entry.isSymbolicLink()) throw Error(`Mirror parent is not a real directory: ${path}`);
    }
  }
  return true;
}
async function writeExclusive(path, bytes) {
  const handle = await open(path, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
  try { await handle.writeFile(bytes); await handle.sync(); } finally { await handle.close(); }
}

// Scalar-aligned bounded Myers diff. If unusually broad edits exhaust the work
// budget, a conservative enclosing span may report a conflict; it cannot erase
// a concurrent change because native ownership still checks the whole span.
export function externalTextSplices(before, after) {
  if (before === after) return [];
  const a = Array.from(before), b = Array.from(after);
  let prefix = 0, suffix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) prefix++;
  while (suffix < a.length - prefix && suffix < b.length - prefix && a[a.length - 1 - suffix] === b[b.length - 1 - suffix]) suffix++;
  const left = a.slice(prefix, a.length - suffix), right = b.slice(prefix, b.length - suffix);
  const start = a.slice(0, prefix).join("").length;
  const fallback = () => [{ start, end: start + left.join("").length, insert: right.join("") }];
  let frontier = new Map([[1, 0]]), work = 0;
  const trace = [];
  for (let distance = 0; distance <= Math.min(left.length + right.length, 512); distance++) {
    trace.push(new Map(frontier));
    for (let diagonal = -distance; diagonal <= distance; diagonal += 2) {
      if (++work > 1_000_000) return fallback();
      let x = diagonal === -distance || (diagonal !== distance && (frontier.get(diagonal - 1) ?? -1) < (frontier.get(diagonal + 1) ?? -1))
        ? (frontier.get(diagonal + 1) ?? 0) : (frontier.get(diagonal - 1) ?? 0) + 1;
      let y = x - diagonal;
      while (x < left.length && y < right.length && left[x] === right[y]) { x++; y++; if (++work > 1_000_000) return fallback(); }
      frontier.set(diagonal, x);
      if (x < left.length || y < right.length) continue;
      const reversed = [];
      for (let d = distance; d >= 0; d--) {
        const v = trace[d], k = x - y;
        const previousK = k === -d || (k !== d && (v.get(k - 1) ?? -1) < (v.get(k + 1) ?? -1)) ? k + 1 : k - 1;
        const previousX = v.get(previousK) ?? 0, previousY = previousX - previousK;
        while (x > previousX && y > previousY) { reversed.push(["same", left[--x]]); --y; }
        if (d > 0) { if (x === previousX) reversed.push(["insert", right[--y]]); else reversed.push(["delete", left[--x]]); }
      }
      const edits = []; let offset = start, pending = null;
      for (const [kind, scalar] of reversed.reverse()) {
        if (kind === "same") { if (pending) edits.push(pending); pending = null; offset += scalar.length; }
        else {
          pending ??= { start: offset, end: offset, insert: "" };
          if (kind === "delete") { offset += scalar.length; pending.end = offset; } else pending.insert += scalar;
        }
      }
      if (pending) edits.push(pending);
      return edits;
    }
  }
  return fallback();
}

/**
 * readCommitted(): {checkpoint:Uint8Array,snapshot:{working:{revision,files},fileIds}}
 * admitWorkingEdits({operation,expectedRevision,edits}): durable
 *   {status:"committed"|"rejected",reason?}; an exception means retry exact bytes.
 * Caller owns the exclusive workspace lock and authenticates configured identity.
 */
export async function createCollaborationMirror(options) {
  const { documentId, documentEpoch, clientId, userId, readCommitted, admitWorkingEdits } = options;
  for (const value of [documentId, documentEpoch, clientId, userId]) checkIdentity(value);
  if (typeof readCommitted !== "function" || typeof admitWorkingEdits !== "function") throw Error("Mirror requires committed source and durable admission callbacks");
  const folder = resolve(options.folder);
  if ((await realpath(folder)) !== folder || !(await lstat(folder)).isDirectory()) throw Error("Mirror folder must be a real canonical directory");
  const metadata = join(folder, ".geosolve", "collaboration-mirror");
  await ensureDirectory(join(folder, ".geosolve")); await ensureDirectory(metadata);
  for (const name of ["blobs", "checkpoints", "moves", "staging"]) await ensureDirectory(join(metadata, name));
  const manifestPath = join(metadata, "manifest.json");
  const identity = { documentId, documentEpoch, clientId, userId };
  const actor = createHash("sha256").update(JSON.stringify(identity)).digest();
  let queue = Promise.resolve();
  let preservedBudget;
  const phase = async (name, detail = {}) => options.onPhase?.(name, detail);
  async function blob(bytes, kind = "blobs") {
    if (bytes.length > MAX_CHECKPOINT) throw Error("Mirror blob exceeds bounds");
    const digest = hash(bytes), path = join(metadata, kind, digest);
    if (kind === "blobs") {
      if (!preservedBudget) {
        const entries = await readdir(join(metadata, kind)); let bytes = 0;
        for (const name of entries) {
          const entry = await lstat(join(metadata, kind, name));
          if (!/^[a-f0-9]{64}$/u.test(name) || !entry.isFile() || entry.isSymbolicLink()) throw Error("Invalid mirror preserved storage; explicit recovery required");
          bytes += entry.size;
        }
        preservedBudget = { bytes, files: entries.length };
      }
      if (!await exists(path) && (preservedBudget.bytes + bytes.length > MAX_PRESERVED_BYTES || preservedBudget.files >= MAX_PRESERVED_FILES)) throw Error("Mirror preserved bytes reached retention bounds; explicit archival is required");
    }
    try { await writeExclusive(path, bytes); await syncDirectory(dirname(path)); if (kind === "blobs") { preservedBudget.bytes += bytes.length; preservedBudget.files++; } }
    catch (error) { if (error.code !== "EEXIST") throw error; const saved = await readRegular(path, MAX_CHECKPOINT); if (!saved || saved.blob !== digest) throw Error("Corrupt mirror blob; explicit recovery required"); }
    return digest;
  }
  async function getBlob(digest, limit = MAX_CHECKPOINT, kind = "blobs") {
    if (!/^[a-f0-9]{64}$/u.test(digest)) throw Error("Invalid mirror blob reference");
    const saved = await readRegular(join(metadata, kind, digest), limit);
    if (!saved || saved.blob !== digest) throw Error("Corrupt or absent mirror blob; explicit recovery required");
    return saved.bytes;
  }
  async function load() {
    const saved = await readRegular(manifestPath, MAX_MANIFEST);
    if (!saved) return { format: FORMAT, ...identity, generation: 0, basis: null, pending: null, notices: [] };
    const { integrity, ...state } = JSON.parse(text(saved.bytes));
    if (integrity !== hash(Buffer.from(JSON.stringify(state)))) throw Error("Corrupt mirror manifest checksum; explicit recovery required");
    if (state.format !== FORMAT || Object.entries(identity).some(([key, value]) => state[key] !== value)
      || !Number.isSafeInteger(state.generation) || state.generation < 0 || !Array.isArray(state.notices)) throw Error("Foreign or corrupt mirror manifest; explicit recovery required");
    for (const files of [state.basis?.disk, state.pending?.observed, state.pending?.desired]) {
      if (!files) continue;
      if (Object.keys(files).length > 512) throw Error("Mirror manifest file limit");
      for (const [path, entry] of Object.entries(files)) { checkPath(path); if (!/^[a-f0-9]{64}$/u.test(entry.blob)) throw Error("Invalid mirror file reference"); }
    }
    if (state.pending && !["admission", "export"].includes(state.pending.kind)) throw Error("Invalid mirror pending phase");
    if (state.basis && !/^[a-f0-9]{64}$/u.test(state.basis.checkpoint)) throw Error("Invalid mirror basis checkpoint");
    if (state.pending?.kind === "export" && (!/^[a-f0-9-]{36}$/u.test(state.pending.id)
      || !/^[a-f0-9]{64}$/u.test(state.pending.checkpoint) || !Array.isArray(state.pending.completed)
      || state.pending.completed.length > 1024 || state.pending.completed.some(path => !authored(path)))) throw Error("Invalid mirror export plan");
    if (state.pending?.kind === "admission") {
      const request = state.pending.request;
      if (!request || request.operation?.userId !== userId || request.operation?.clientId !== clientId
        || !/^mirror-[a-f0-9-]{36}$/u.test(request.operation?.requestId)
        || !Array.isArray(request.edits) || !request.edits.length || request.edits.length > 256
        || request.edits.some(edit => !authored(edit.path) || (edit.kind === "rename_file" && !authored(edit.new_path)))) throw Error("Invalid mirror admission plan");
    }
    return state;
  }
  async function save(state) {
    const encoded = JSON.stringify(state), bytes = Buffer.from(JSON.stringify({ ...state, integrity: hash(Buffer.from(encoded)) }));
    if (bytes.length > MAX_MANIFEST) throw Error("Mirror manifest exceeds bounds");
    const previous = await readRegular(manifestPath, MAX_MANIFEST);
    if (previous && previous.bytes.equals(bytes)) return;
    const temporary = join(metadata, "staging", `manifest-${randomUUID()}`);
    await writeExclusive(temporary, bytes); await rename(temporary, manifestPath); await syncDirectory(metadata);
  }
  async function scan() {
    const files = {}; let total = 0, visited = 0;
    async function visit(relative, depth) {
      if (depth > 32) throw Error("Mirror directory depth exceeds bounds");
      for (const entry of (await readdir(join(folder, relative), { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name, "en"))) {
        if (++visited > 8192) throw Error("Mirror tree entry limit");
        if (ignored.has(entry.name)) continue;
        const path = relative ? `${relative}/${entry.name}` : entry.name;
        if (entry.isSymbolicLink()) throw Error(`Authored mirror cannot contain symlinks: ${path}`);
        if (entry.isDirectory()) { await visit(path, depth + 1); continue; }
        if (!authored(path)) continue;
        await parents(folder, path);
        const value = await readRegular(join(folder, path), MAX_FILE);
        if (!value) throw Error(`Authored file disappeared while reading: ${path}`);
        text(value.bytes); // Preserve invalid source syntax, reject invalid UTF-8.
        total += value.bytes.length;
        if (total > MAX_TREE || Object.keys(files).length >= 512) throw Error("Authored mirror tree exceeds bounds");
        const digest = await blob(value.bytes);
        files[path] = { blob: digest, device: value.device, inode: value.inode };
      }
    }
    await visit("", 0); return files;
  }
  async function committed() {
    const value = await readCommitted();
    const checkpoint = Buffer.from(value.checkpoint);
    if (checkpoint.length > MAX_CHECKPOINT) throw Error("Mirror native checkpoint exceeds bounds");
    const replica = await createSharedText({ actor, checkpoint });
    try {
      const captured = replica.capture(), files = captured.files, ids = {};
      if (!same(captured.revision, value.snapshot.working.revision) || !same(files, value.snapshot.working.files)) throw Error("Incoherent committed mirror snapshot");
      for (const path of Object.keys(files)) { checkPath(path); ids[path] = replica.fileId(path); }
      if (!same(ids, value.snapshot.fileIds)) throw Error("Incoherent committed mirror file identities");
      return { checkpoint: await blob(checkpoint, "checkpoints"), snapshot: { working: captured, fileIds: ids }, replica };
    } catch (error) { replica.dispose(); throw error; }
  }
  const image = files => Object.fromEntries(Object.entries(files).sort(([a], [b]) => a.localeCompare(b, "en")).map(([path, entry]) => [path, entry.blob]));
  async function desiredFiles(current) {
    const files = {};
    for (const [path, source] of Object.entries(current.snapshot.working.files)) files[path] = { blob: await blob(Buffer.from(source)), id: current.snapshot.fileIds[path] };
    return files;
  }
  async function notice(state, notices) { state.notices = notices; await save(state); return statusValue(state); }
  function statusValue(state) {
    return { status: state.notices.length ? "reconciliation_pending" : state.pending ? "pending" : state.basis ? "synchronized" : "uninitialized",
      generation: state.generation, pendingPhase: state.pending?.kind ?? null, notices: structuredClone(state.notices), manifestPath };
  }

  async function planImport(state, disk, current) {
    const baseline = await createSharedText({ actor, checkpoint: await getBlob(state.basis.checkpoint, MAX_CHECKPOINT, "checkpoints") });
    const candidate = current.replica;
    const edits = [], notices = [];
    // Resolve bounded adjacent native ownership ranges independently against one
    // immutable current checkpoint. Large deleted spans therefore keep the same
    // ownership checks without constructing a client inverse or mutating a probe.
    function resolveSpan(path, source, start, end) {
      const ranges = []; let offset = start, chunkStart = start, bytes = 0;
      for (const scalar of source.slice(start, end)) {
        const size = Buffer.byteLength(scalar);
        if (bytes + size > 64 * 1024) { ranges.push(candidate.resolveRange(baseline.anchorRange(path, chunkStart, offset))); chunkStart = offset; bytes = 0; }
        bytes += size; offset += scalar.length;
      }
      ranges.push(candidate.resolveRange(baseline.anchorRange(path, chunkStart, end)));
      for (let index = 1; index < ranges.length; index++) {
        if (ranges[index].path !== ranges[0].path || ranges[index - 1].end_utf16 !== ranges[index].start_utf16) throw Error("Shared insertion overlaps the external range");
      }
      return { path: ranges[0].path, start_utf16: ranges[0].start_utf16, end_utf16: ranges.at(-1).end_utf16 };
    }
    try {
      const oldFiles = baseline.capture().files, currentFiles = current.snapshot.working.files;
      const oldIds = Object.fromEntries(Object.keys(oldFiles).map(path => [path, baseline.fileId(path)]));
      const sharedById = new Map(Object.entries(current.snapshot.fileIds).map(([path, id]) => [id, path]));
      const oldPaths = new Set(Object.keys(oldFiles)), additions = new Set(Object.keys(disk));
      const renames = new Map(), missing = new Set(oldPaths);
      const assign = (oldPath, path) => { renames.set(oldPath, path); additions.delete(path); missing.delete(oldPath); };
      const sameInode = (oldPath, path) => disk[path].device === state.basis.disk[oldPath].device && disk[path].inode === state.basis.disk[oldPath].inode;
      // Stable paths first disambiguate unchanged hard-linked siblings. Remaining
      // inode correspondence detects actual renames, including occupied-path swaps.
      for (const oldPath of oldPaths) if (disk[oldPath] && sameInode(oldPath, oldPath)) assign(oldPath, oldPath);
      for (const byInode of [true, false]) {
        if (!byInode) {
          // An editor's atomic save replaces its inode while retaining logical
          // path identity. Apply this fallback only after actual moved inodes.
          for (const oldPath of [...missing]) if (additions.has(oldPath)) assign(oldPath, oldPath);
        }
        const matchesByOldPath = new Map();
        for (const oldPath of missing) {
          matchesByOldPath.set(oldPath, [...additions].filter(path => byInode
            ? sameInode(oldPath, path) : disk[path].blob === state.basis.disk[oldPath].blob));
        }
        for (const [oldPath, matches] of matchesByOldPath) {
          if (matches.length === 1 && [...matchesByOldPath.values()].filter(values => values.includes(matches[0])).length === 1) assign(oldPath, matches[0]);
          else if (matches.length) notices.push({ path: oldPath, reason: "External rename is ambiguous; original bytes are preserved" });
        }
        if (notices.length) return { edits: [], notices };
      }
      const removals = [], moves = [], replacements = [];
      for (const oldPath of oldPaths) {
        const externalPath = renames.get(oldPath) ?? oldPath, external = renames.has(oldPath) ? disk[externalPath] : null, sharedPath = sharedById.get(oldIds[oldPath]);
        const old = oldFiles[oldPath], raw = external ? text(await getBlob(external.blob, MAX_FILE)) : null;
        if (!external) {
          if (!sharedPath) continue;
          if (currentFiles[sharedPath] !== old || sharedPath !== oldPath) notices.push({ path: oldPath, reason: "External deletion overlaps shared file changes" });
          else {
            try {
              // Equal bytes do not establish unchanged ownership: another editor
              // may have deliberately replaced a range with the same value.
              resolveSpan(oldPath, old, 0, old.length);
              removals.push({ kind: "remove_file", path: sharedPath });
            } catch (error) { notices.push({ path: oldPath, reason: `External deletion overlaps shared ownership: ${error.message ?? error}` }); }
          }
          continue;
        }
        if (!sharedPath) {
          if (raw !== old || externalPath !== oldPath) notices.push({ path: externalPath, reason: "External edit targets a deleted or recreated shared file", externalBlob: external.blob });
          continue;
        }
        if (externalPath !== oldPath && sharedPath !== oldPath && externalPath !== sharedPath) { notices.push({ path: externalPath, reason: "External and shared file renames compete", externalBlob: external.blob }); continue; }
        if (externalPath !== oldPath && externalPath !== sharedPath) moves.push({ kind: "rename_file", path: sharedPath, new_path: externalPath });
        if (raw === old || raw === currentFiles[sharedPath]) continue;
        for (const splice of externalTextSplices(old, raw)) {
          try { const range = resolveSpan(oldPath, old, splice.start, splice.end); replacements.push({ kind: "splice", path: range.path, start_utf16: range.start_utf16, delete_utf16: range.end_utf16 - range.start_utf16, insert: splice.insert }); }
          catch (error) { notices.push({ path: externalPath, reason: String(error.message ?? error), externalBlob: external.blob }); }
        }
      }
      if (notices.length) return { edits: [], notices };
      // Native checked anchors own the mapping. Descending offsets compose their
      // exact edits atomically at the host's immutable expected revision.
      replacements.sort((a, b) => a.path.localeCompare(b.path, "en") || b.start_utf16 - a.start_utf16);
      for (let index = 1; index < replacements.length; index++) {
        const previous = replacements[index - 1], next = replacements[index];
        if (previous.path === next.path && next.start_utf16 + next.delete_utf16 > previous.start_utf16) return { edits: [], notices: [{ path: next.path, reason: "External changes mapped to overlapping shared ranges" }] };
      }
      edits.push(...replacements);
      edits.push(...removals);
      // Atomic batches still execute ordered paths. Temporarily vacate rename
      // sources so swaps never overwrite or allocate a new native file identity.
      const occupied = new Set(Object.keys(currentFiles).filter(path => !removals.some(edit => edit.path === path)));
      const temporaryMoves = [];
      for (const [index, move] of moves.entries()) {
        const temporary = `__geosolve_mirror_${state.generation}_${index}.ts`;
        if (occupied.has(temporary) || disk[temporary]) { notices.push({ path: temporary, reason: "Mirror temporary rename path is occupied" }); break; }
        edits.push({ kind: "rename_file", path: move.path, new_path: temporary }); occupied.delete(move.path); occupied.add(temporary);
        temporaryMoves.push({ ...move, path: temporary });
      }
      for (const move of temporaryMoves) {
        if (occupied.has(move.new_path)) { notices.push({ path: move.new_path, reason: "External rename destination is occupied by shared source" }); break; }
        edits.push(move); occupied.delete(move.path); occupied.add(move.new_path);
      }
      for (const path of additions) {
        const source = text(await getBlob(disk[path].blob, MAX_FILE));
        if (occupied.has(path)) {
          if (currentFiles[path] !== source) notices.push({ path, reason: "External and shared file creations compete", externalBlob: disk[path].blob });
        } else { edits.push({ kind: "create_file", path, text: source }); occupied.add(path); }
      }
      return { edits: notices.length ? [] : edits, notices };
    } finally { baseline.dispose(); }
  }

  async function prepareExport(state, observed) {
    const current = await committed();
    try {
      const desired = await desiredFiles(current);
      state.pending = { kind: "export", id: randomUUID(), checkpoint: current.checkpoint, desired, observed, completed: [] };
      state.notices = []; await save(state); await phase("export-planned", { state: structuredClone(state) });
    } finally { current.replica.dispose(); }
  }
  async function publish(state) {
    const plan = state.pending, paths = [...new Set([...Object.keys(plan.observed), ...Object.keys(plan.desired)])].sort();
    const conflicts = [];
    async function restoreMoved(savedPath, target, path) {
      await parents(folder, path);
      try { await link(savedPath, target); await syncDirectory(dirname(target)); }
      catch (error) { if (error.code !== "EEXIST") throw error; }
    }
    for (const [index, path] of paths.entries()) {
      checkPath(path); await parents(folder, path, true);
      const target = join(folder, path), expected = plan.observed[path]?.blob ?? null, desired = plan.desired[path]?.blob ?? null;
      const savedPath = join(metadata, "moves", `${plan.id}-${index}`);
      let current = await readRegular(target, MAX_FILE);
      if ((current?.blob ?? null) === desired) { if (!plan.completed.includes(path)) { plan.completed.push(path); await save(state); } continue; }
      if (plan.completed.includes(path)) { if (current) await blob(current.bytes); conflicts.push({ path, reason: "External bytes changed after mirror installation", externalBlob: current?.blob ?? null }); continue; }
      let moved;
      try { moved = await readRegular(savedPath, MAX_FILE); }
      catch (error) { if (await exists(savedPath)) await restoreMoved(savedPath, target, path); throw error; }
      if (moved) {
        if (moved.blob !== expected || current) {
          if (!current) await restoreMoved(savedPath, target, path);
          await blob((current ?? moved).bytes);
          conflicts.push({ path, reason: "Interrupted mirror publication has competing external bytes", externalBlob: current?.blob ?? moved.blob }); continue;
        }
      } else {
        if ((current?.blob ?? null) !== expected) { if (current) await blob(current.bytes); conflicts.push({ path, reason: "External bytes changed before mirror publication", externalBlob: current?.blob ?? null }); continue; }
        if (current) {
          await phase("before-preserve", { path });
          await parents(folder, path);
          await rename(target, savedPath); await syncDirectory(dirname(target)); await syncDirectory(dirname(savedPath));
          await phase("after-move", { path });
          try { moved = await readRegular(savedPath, MAX_FILE); }
          catch (error) { await restoreMoved(savedPath, target, path); throw error; }
          if (!moved || moved.blob !== expected) {
            if (moved) { await restoreMoved(savedPath, target, path); await blob(moved.bytes); }
            conflicts.push({ path, reason: "External bytes changed during mirror publication", externalBlob: moved?.blob ?? null }); continue;
          }
          await phase("after-preserve", { path });
        }
      }
      if (desired !== null) {
        const temporary = join(metadata, "staging", `export-${randomUUID()}`);
        await writeExclusive(temporary, await getBlob(desired, MAX_FILE));
        try {
          await parents(folder, path); await link(temporary, target); await syncDirectory(dirname(target));
        } catch (error) { if (error.code !== "EEXIST") throw error; conflicts.push({ path, reason: "External bytes appeared during mirror publication" }); }
        finally { await unlink(temporary); }
      }
      await phase("after-install", { path });
      current = await readRegular(target, MAX_FILE);
      if ((current?.blob ?? null) !== desired) { if (current) await blob(current.bytes); conflicts.push({ path, reason: "External bytes compete with installed mirror", externalBlob: current?.blob ?? null }); continue; }
      plan.completed.push(path); await save(state);
    }
    const disk = await scan();
    if (!same(image(disk), image(plan.desired))) {
      if (!conflicts.length) conflicts.push({ path: null, reason: "External tree changed during mirror publication" });
      return notice(state, conflicts);
    }
    state.basis = { checkpoint: plan.checkpoint, disk };
    state.pending = null; state.notices = []; state.generation++;
    await phase("before-manifest", {}); await save(state);
    return statusValue(state);
  }
  async function run() {
    const state = await load();
    if (state.pending?.kind === "export") return publish(state);
    if (state.pending?.kind === "admission") {
      const result = await admitWorkingEdits(structuredClone(state.pending.request));
      if (result?.status === "rejected") {
        state.refusal = { expectedRevision: state.pending.request.expectedRevision, observed: image(state.pending.observed) };
        state.pending = null; return notice(state, [{ path: null, reason: `External contribution rejected: ${result.reason ?? "stale or invalid basis"}` }]);
      }
      if (result?.status !== "committed") throw Error("Mirror gateway returned no durable terminal outcome");
      const observed = state.pending.observed;
      await phase("after-admission", { request: structuredClone(state.pending.request) });
      await prepareExport(state, observed); return publish(state);
    }
    const disk = await scan(), current = await committed();
    try {
      if (!state.basis) {
        const desired = await desiredFiles(current);
        if (!same(image(disk), image(desired))) return notice(state, [{ path: null, reason: "First mirror attachment differs from shared source; external bytes are preserved for explicit reconciliation" }]);
        state.basis = { checkpoint: current.checkpoint, disk }; state.notices = []; await save(state); return statusValue(state);
      }
      if (state.refusal && same(state.refusal.expectedRevision, current.snapshot.working.revision) && same(state.refusal.observed, image(disk))) return statusValue(state);
      delete state.refusal;
      const planned = await planImport(state, disk, current);
      if (planned.notices.length) return notice(state, planned.notices);
      if (planned.edits.length) {
        if (planned.edits.length > 256) return notice(state, [{ path: null, reason: "External change exceeds the atomic 256-edit gateway limit; split or reconcile the edit explicitly" }]);
        const request = { operation: { userId, clientId, requestId: `mirror-${randomUUID()}` }, expectedRevision: current.snapshot.working.revision, edits: planned.edits };
        state.pending = { kind: "admission", request, observed: disk }; state.notices = [];
        await save(state); await phase("admission-planned", { request: structuredClone(request) });
        // Resume the persisted request through the same code path as a restart.
      } else {
        const desired = await desiredFiles(current);
        if (same(image(disk), image(desired))) { state.basis = { checkpoint: current.checkpoint, disk }; state.notices = []; await save(state); return statusValue(state); }
        await prepareExport(state, disk); return publish(state);
      }
    } finally { current.replica.dispose(); }
    return run();
  }
  async function collectCheckpoints() {
    const state = await load(), retained = new Set([state.basis?.checkpoint, state.pending?.checkpoint]);
    let removedCheckpoint = false, removedMove = false, removedStaging = false;
    for (const name of await readdir(join(metadata, "checkpoints"))) {
      if (!/^[a-f0-9]{64}$/u.test(name)) throw Error("Unexpected mirror checkpoint entry; explicit recovery required");
      if (!retained.has(name)) { await unlink(join(metadata, "checkpoints", name)); removedCheckpoint = true; }
    }
    if (removedCheckpoint) await syncDirectory(join(metadata, "checkpoints"));
    // Source files moved aside during publication are already protected by
    // content-addressed blobs. Keep live crash-plan moves only, avoiding a second
    // accumulating copy of every ordinary exported file.
    for (const name of await readdir(join(metadata, "moves"))) {
      if (!/^[a-f0-9-]{36}-\d+$/u.test(name)) throw Error("Unexpected mirror preserved entry; explicit recovery required");
      if (state.pending?.kind === "export" && name.startsWith(`${state.pending.id}-`)) continue;
      const path = join(metadata, "moves", name), value = await readRegular(path, MAX_FILE);
      if (!value) continue;
      await blob(value.bytes); await unlink(path); removedMove = true;
    }
    if (removedMove) await syncDirectory(join(metadata, "moves"));
    for (const name of await readdir(join(metadata, "staging"))) {
      if (!/^(?:manifest|export)-[a-f0-9-]{36}$/u.test(name)) throw Error("Unexpected mirror staging entry; explicit recovery required");
      const path = join(metadata, "staging", name), entry = await lstat(path);
      if (!entry.isFile() || entry.isSymbolicLink()) throw Error("Invalid mirror staging file; explicit recovery required");
      await unlink(path); removedStaging = true;
    }
    if (removedStaging) await syncDirectory(join(metadata, "staging"));
  }
  return {
    reconcile() { const result = queue.then(async () => { await collectCheckpoints(); const value = await run(); await collectCheckpoints(); return value; }); queue = result.catch(() => {}); return result; },
    async status() { await queue; return statusValue(await load()); },
  };
}
