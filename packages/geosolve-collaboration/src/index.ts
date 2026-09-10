// SPDX-License-Identifier: GPL-3.0-or-later
/** Causal heads identify raw draft text, never an accepted geometric result. */
export interface TextRevision { readonly heads: readonly string[] }
export interface TextSnapshot { readonly revision: TextRevision; readonly files: Readonly<Record<string, string>> }
export interface HistoricalTextEdit {
  readonly snapshot: TextSnapshot;
  /** Local displayed branch after this edit, before unseen remote merging. */
  readonly localRevision: TextRevision;
}
export interface TextLimits {
  readonly max_files?: number;
  readonly max_file_bytes?: number;
  readonly max_total_bytes?: number;
  readonly max_history_bytes?: number;
  readonly max_changes?: number;
  readonly max_operations?: number;
  readonly max_sync_bytes?: number;
  readonly max_peers?: number;
}
export type TextEdit =
  | { readonly kind: "create_file"; readonly path: string; readonly text: string }
  | { readonly kind: "splice"; readonly path: string; readonly start_utf16: number; readonly delete_utf16: number; readonly insert: string }
  | { readonly kind: "remove_file"; readonly path: string }
  | { readonly kind: "rename_file"; readonly path: string; readonly new_path: string };
/** Stable cursor encoding is owned by the pinned Rust protocol. Treat it as opaque. */
export interface TextCursor { readonly path: string; readonly object: string; readonly encoded: readonly number[] }
export interface TextLocation { readonly path: string; readonly utf16: number }
export interface TextRange { readonly path: string; readonly start_utf16: number; readonly end_utf16: number }
export interface TextRangeAnchor { readonly start: TextCursor; readonly end: TextCursor; readonly expected: string; readonly ownership: readonly number[] }

export interface CollaborationNativeHandle {
  save(): Uint8Array;
  capture(): string;
  fork(actor: Uint8Array): CollaborationNativeHandle;
  edit(revisionJson: string, editsJson: string): string;
  editFromRevision(revisionJson: string, editsJson: string): string;
  generateSyncMessage(peer: string): Uint8Array | undefined;
  receiveSyncMessage(peer: string, bytes: Uint8Array): string;
  receiveSyncMessageFrom(peer: string, bytes: Uint8Array, actor: Uint8Array): string;
  forgetPeer(peer: string): void;
  changesSince(revisionJson: string): string;
  applyServerChanges(changesJson: string): string;
  applyChangesFrom(changesJson: string, actor: Uint8Array): string;
  cursor(path: string, utf16: number, after: boolean): string;
  resolveCursor(cursorJson: string): string;
  fileId(path: string): string;
  anchorRange(revisionJson: string, path: string, start: number, end: number): string;
  resolveRange(anchorJson: string): string;
  replaceRange(anchorJson: string, insert: string): string;
  undo(): string;
  redo(): string;
  undoCount(): number;
  redoCount(): number;
  free(): void;
}
export interface CollaborationWasmModule {
  default(options: { module_or_path: Uint8Array | URL | Response | WebAssembly.Module }): Promise<unknown>;
  SharedTextReplica: {
    new(actor: Uint8Array, limitsJson: string): CollaborationNativeHandle;
    load(bytes: Uint8Array, actor: Uint8Array, limitsJson: string): CollaborationNativeHandle;
  };
}
export interface CollaborationOptions {
  /** Server-issued, connection-authenticated unique actor. Never infer it from a display name. */
  readonly actor: Uint8Array;
  readonly checkpoint?: Uint8Array;
  readonly limits?: TextLimits;
  readonly wasmModule?: CollaborationWasmModule;
  readonly wasm?: Uint8Array | URL | Response | WebAssembly.Module;
}

/** Synchronous thin owner intended for a worker independent of compiler/solver jobs.
 * Transport, persistence-before-ACK and authenticated actor assignment belong to the host.
 */
export class SharedText {
  private disposed = false;
  constructor(private readonly native: CollaborationNativeHandle) {}
  capture(): TextSnapshot { this.live(); return decode(this.native.capture()); }
  save(): Uint8Array { this.live(); return this.native.save(); }
  fork(actor: Uint8Array): SharedText { this.live(); checkActor(actor); return new SharedText(this.native.fork(actor)); }
  /** Always pass the editor's basis revision when applying asynchronously prepared offsets. */
  edit(edits: readonly TextEdit[], expected: TextRevision = this.capture().revision): TextSnapshot {
    this.live(); return decode(this.native.edit(encode(expected), encode(edits)));
  }
  /** Existing-file typing against actual displayed heads, merged by native CRDT.
   * Unseen local-actor edits reject. Use server personal history for Undo. */
  editFromRevision(edits: readonly TextEdit[], displayed: TextRevision): HistoricalTextEdit {
    this.live(); return decode(this.native.editFromRevision(encode(displayed), encode(edits)));
  }
  generateSyncMessage(peer: string): Uint8Array | undefined {
    this.live(); unicode(peer); return this.native.generateSyncMessage(peer);
  }
  /** Client receives authoritative server history through this method. */
  receiveSyncMessage(peer: string, bytes: Uint8Array): TextSnapshot {
    this.live(); unicode(peer); return decode(this.native.receiveSyncMessage(peer, bytes));
  }
  /** Server ingress authenticates every novel change using its session-bound actor. */
  receiveSyncMessageFrom(peer: string, bytes: Uint8Array, actor: Uint8Array): TextSnapshot {
    this.live(); unicode(peer); checkActor(actor); return decode(this.native.receiveSyncMessageFrom(peer, bytes, actor));
  }
  /** Reset both ends when reconnecting or recovering from a dropped generated message. */
  forgetPeer(peer: string): void { this.live(); unicode(peer); this.native.forgetPeer(peer); }
  changesSince(revision: TextRevision): readonly Uint8Array[] {
    this.live(); return (decode(this.native.changesSince(encode(revision))) as readonly number[][]).map((bytes) => Uint8Array.from(bytes));
  }
  applyChangesFrom(changes: readonly Uint8Array[], actor: Uint8Array): TextSnapshot {
    this.live(); checkActor(actor);
    return decode(this.native.applyChangesFrom(encode(changes.map((bytes) => Array.from(bytes))), actor));
  }
  /** Client-only receive from authenticated authority. Server ingress must use
   * applyChangesFrom to authenticate each novel writer. Preserves local edits. */
  applyServerChanges(changes: readonly Uint8Array[]): TextSnapshot {
    this.live(); return decode(this.native.applyServerChanges(encode(changes.map((bytes) => Array.from(bytes)))));
  }
  /** Bias controls surviving-neighbor fallback after deletion, not insertion affinity. */
  cursor(path: string, utf16: number, deletionBias: "before" | "after" = "after"): TextCursor {
    this.live(); unicode(path); offset(utf16);
    if (deletionBias !== "before" && deletionBias !== "after") throw Error("Invalid cursor deletion bias");
    return decode(this.native.cursor(path, utf16, deletionBias === "after"));
  }
  resolveCursor(cursor: TextCursor): TextLocation { this.live(); return decode(this.native.resolveCursor(encode(cursor))); }
  fileId(path: string): string { this.live(); unicode(path); return this.native.fileId(path); }
  anchorRange(path: string, start: number, end: number, expected: TextRevision = this.capture().revision): TextRangeAnchor {
    this.live(); unicode(path); offset(start); offset(end);
    return decode(this.native.anchorRange(encode(expected), path, start, end));
  }
  /** Fails when overlapping edits destroyed the original range's character ownership. */
  resolveRange(anchor: TextRangeAnchor): TextRange {
    this.live(); return decode(this.native.resolveRange(encode(anchor)));
  }
  /** Fails when overlapping edits destroyed the original range's character ownership. */
  replaceRange(anchor: TextRangeAnchor, insert: string): TextSnapshot {
    this.live(); unicode(insert); return decode(this.native.replaceRange(encode(anchor), insert));
  }
  undo(): TextSnapshot { this.live(); return decode(this.native.undo()); }
  redo(): TextSnapshot { this.live(); return decode(this.native.redo()); }
  /** Counts are advisory: a later overlapping contribution can make an inverse unavailable. */
  get history(): { readonly undo: number; readonly redo: number } {
    this.live(); return Object.freeze({ undo: this.native.undoCount(), redo: this.native.redoCount() });
  }
  dispose(): void { if (!this.disposed) { this.disposed = true; this.native.free(); } }
  private live(): void { if (this.disposed) throw Error("Shared text replica has been disposed"); }
}

/** Loads the same Rust WASM in browsers and Node; bundlers may inject module and bytes. */
export async function createSharedText(options: CollaborationOptions): Promise<SharedText> {
  checkActor(options.actor);
  const limits = encode(options.limits ?? {});
  const path = "./wasm/geosolve_collaboration_wasm.js";
  const module: CollaborationWasmModule = options.wasmModule ?? await import(path);
  let bytes = options.wasm;
  if (!bytes) {
    const url = new URL("./wasm/geosolve_collaboration_wasm_bg.wasm", import.meta.url);
    if (url.protocol === "file:") {
      const nodeFs = "node:fs/promises";
      bytes = await (await import(nodeFs)).readFile(url);
    } else bytes = url;
  }
  await module.default({ module_or_path: bytes! });
  return new SharedText(options.checkpoint
    ? module.SharedTextReplica.load(options.checkpoint, options.actor, limits)
    : new module.SharedTextReplica(options.actor, limits));
}
function checkActor(actor: Uint8Array): void {
  if (!(actor instanceof Uint8Array) || !actor.length || actor.length > 64) throw Error("Actor must contain 1..64 bytes");
}
function offset(value: number): void {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffffffff) throw Error("Offset must be a nonnegative WASM-sized integer");
}
/** wasm-bindgen replaces unpaired UTF-16 surrogates; refuse before crossing that boundary. */
function unicode(value: string): void {
  if (typeof value !== "string") throw Error("Expected text");
  for (let index = 0; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(++index);
      if (!(next >= 0xdc00 && next <= 0xdfff)) throw Error("Text contains an unpaired UTF-16 surrogate");
    } else if (code >= 0xdc00 && code <= 0xdfff) throw Error("Text contains an unpaired UTF-16 surrogate");
  }
}
function encode(value: unknown): string {
  const inspect = (item: unknown): void => {
    if (typeof item === "string") unicode(item);
    else if (typeof item === "number" && (!Number.isSafeInteger(item) || item < 0)) throw Error("Expected a nonnegative safe integer");
    else if (item !== null && typeof item === "object") {
      for (const [key, child] of Object.entries(item)) { unicode(key); inspect(child); }
    }
  };
  inspect(value);
  return JSON.stringify(value);
}
function decode<T>(text: string): T {
  const freeze = (value: unknown): unknown => {
    if (value !== null && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); }
    return value;
  };
  return freeze(JSON.parse(text)) as T;
}
