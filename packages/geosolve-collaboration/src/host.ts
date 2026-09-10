// SPDX-License-Identifier: GPL-3.0-or-later
import { counter, unicode, encode, decode } from "./host-codec.js";
export { TrustedSemanticHost, createTrustedSemanticHost } from "./semantic.js";
export type { StatementPosition, CreationPosition, ObjectDescription, DependencyChange, ReorderChange, StructuralInverse, StructuralRecord, SemanticTarget, DeletionPlan, PropertyAddress, PropertyChange, SemanticHostConfiguration, SemanticCheckpoint, SemanticSnapshot, PreparedSemanticInverse, SemanticTargetReference, SemanticRecord, SemanticTransaction, SemanticStage, PersistSemanticStage, SemanticHostOptions } from "./semantic.js";
export { TrustedSourceHost, createTrustedSourceHost } from "./source.js";
export type { SourceHostOptions, SourceHostConfiguration, SourceSnapshot, AcceptedSource, SourceCapture, PreparedSource, SourceStage, SourcePatch, SourceEdit, SourceReconciliation, PersistSourceStage } from "./source.js";
/** Trusted Node/server host API. Never expose it through client command dispatch.
 * The host authenticates users and validates geometry independently of this ledger.
 */
export interface AuthorityConfiguration {
  readonly documentId: string;
  readonly documentEpoch: string;
  /** Fresh server-process epoch; do not reuse it after reconstruction. */
  readonly serverEpoch: string;
  /** Fingerprint of independently reconstructed initial accepted source/design. */
  readonly initialInput: string;
  readonly limits?: AuthorityLimits;
}
export interface AuthorityLimits {
  readonly maxSessions?: number;
  readonly maxPending?: number;
  readonly maxPendingPerClient?: number;
  readonly maxOperations?: number;
  readonly maxCommandBytes?: number;
  readonly maxLedgerBytes?: number;
  readonly maxResumeRecords?: number;
}
export type Role = "editor" | "viewer";
export interface HostPrincipal { readonly userId: string; readonly role: Role }
export interface Connection {
  readonly protocol: number;
  readonly documentId: string;
  readonly documentEpoch: string;
  readonly serverEpoch: string;
  readonly sessionId: string;
  readonly clientId: string;
  readonly userId: string;
  readonly role: Role;
}
export interface OperationId { readonly userId: string; readonly clientId: string; readonly requestId: string }
export interface Command {
  readonly kind: "semantic" | "apply" | "undo" | "redo" | "file";
  readonly basisRevision: number;
  readonly payload: unknown;
}
export interface Submit { readonly connection: Connection; readonly requestId: string; readonly command: Command }
export type Outcome =
  | { readonly status: "accepted"; readonly revision: number; readonly accepted_input: string; readonly summary: string }
  | { readonly status: "rejected"; readonly code: string; readonly message: string };
export interface Receipt { readonly operation: OperationId; readonly admission: number; readonly outcome: Outcome | null }
export type DocumentEvent =
  | { readonly event: "admitted"; readonly operation: OperationId; readonly admission: number; readonly command: Command; readonly digest: string }
  | { readonly event: "finished"; readonly operation: OperationId; readonly outcome: Outcome };
export interface JournalRecord {
  readonly protocol: number;
  readonly documentId: string;
  readonly documentEpoch: string;
  readonly sequence: number;
  readonly acceptedRevision: number;
  readonly acceptedInput: string;
  readonly previousDigest: string;
  readonly event: DocumentEvent;
  readonly digest: string;
}
export interface AuthoritySnapshot {
  readonly acceptedRevision: number;
  readonly acceptedInput: string;
  readonly latestSequence: number;
  readonly pendingCount: number;
  readonly ledgerBytes: number;
  readonly needsRecovery: boolean;
  readonly hasPendingStage: boolean;
}
export type Resume = { readonly status: "events"; readonly records: readonly JournalRecord[] }
  | { readonly status: "checkpoint_required"; readonly latest_sequence: number };
/** Descriptor of a native-held PreparedOperation. It cannot reconstruct the native ticket. */
export interface PreparedCommand {
  readonly ticket: string;
  readonly operation: OperationId;
  readonly command: Command;
  readonly acceptedRevision: number;
  readonly acceptedInput: string;
}
/** Construct only from the trusted domain worker after its independent validation. */
export type ValidatedCompletion =
  | { readonly status: "accepted"; readonly acceptedInput: string; readonly summary: string }
  | { readonly status: "rejected"; readonly code: string; readonly message: string };
/** This is a pending write, not an ACK. Persist exact recordJson before commit. */
export interface AuthorityStage {
  readonly status: "staged";
  readonly stageDigest: string;
  readonly recordJson: string;
}
export type AdmissionStage = AuthorityStage | { readonly status: "duplicate"; readonly receipt: Receipt };
/** Append and sync the record plus associated accepted source/design/model snapshot
 * in one recoverable transaction. Resolve only once its outcome is durable.
 */
export type PersistAuthorityStage = (stage: AuthorityStage) => Promise<void>;

export interface AuthorityNativeHandle {
  connect(userId: string, role: string, clientId: string, sessionId: string): string;
  disconnect(sessionId: string): void;
  snapshot(): string;
  checkpoint(): string;
  stageAdmission(requestJson: string): string;
  beginNext(): string | undefined;
  stageValidatedCompletion(ticket: string, completionJson: string): string;
  commitStage(stageDigest: string): string;
  failStage(stageDigest: string): void;
  receipt(connectionJson: string, requestId: string): string;
  resume(connectionJson: string, after: number): string;
  free(): void;
}
export interface AuthorityWasmModule {
  default(options: { module_or_path: Uint8Array | URL | Response | WebAssembly.Module }): Promise<unknown>;
  TrustedDocumentHost: {
    new(configurationJson: string): AuthorityNativeHandle;
    restore(configurationJson: string, recordsJson: string): AuthorityNativeHandle;
  };
}
export interface AuthorityHostOptions {
  readonly configuration: AuthorityConfiguration;
  readonly records?: readonly JournalRecord[];
  readonly wasmModule?: AuthorityWasmModule;
  readonly wasm?: Uint8Array | URL | Response | WebAssembly.Module;
}

/** Durable ordering for a trusted host, kept out of the client package entrypoint.
 * One pending stage blocks mutations; committed reads remain available throughout.
 */
export class DocumentAuthorityHost {
  private disposed = false;
  private pending?: AuthorityStage;
  private ticket?: PreparedCommand;
  constructor(private readonly native: AuthorityNativeHandle) {}
  connect(principal: HostPrincipal, clientId: string, sessionId: string): Connection {
    this.live();
    unicode(principal.userId); unicode(principal.role); unicode(clientId); unicode(sessionId);
    return decode(this.native.connect(principal.userId, principal.role, clientId, sessionId));
  }
  disconnect(sessionId: string): void { this.live(); unicode(sessionId); this.native.disconnect(sessionId); }
  snapshot(): AuthoritySnapshot { this.live(); return decode(this.native.snapshot()); }
  checkpoint(): readonly JournalRecord[] { this.live(); return decode(this.native.checkpoint()); }
  receipt(connection: Connection, requestId: string): Receipt | null {
    this.live(); unicode(requestId); return decode(this.native.receipt(encode(connection), requestId));
  }
  resume(connection: Connection, after: number): Resume {
    this.live(); counter(after); return decode(this.native.resume(encode(connection), after));
  }
  stageAdmission(request: Submit): AdmissionStage {
    this.live(); counter(request.command.basisRevision);
    const stage = decode<AdmissionStage>(this.native.stageAdmission(encode(request)));
    if (stage.status === "staged") this.pending = stage;
    return stage;
  }
  beginNext(): PreparedCommand | undefined {
    this.live();
    const result = this.native.beginNext();
    if (result === undefined) return undefined;
    this.ticket = decode<PreparedCommand>(result);
    return this.ticket;
  }
  stageValidatedCompletion(prepared: PreparedCommand, completion: ValidatedCompletion): AuthorityStage {
    this.live();
    if (prepared !== this.ticket) throw Error("Prepared command belongs to another or stale host ticket");
    const stage = decode<AuthorityStage>(this.native.stageValidatedCompletion(prepared.ticket, encode(completion)));
    this.pending = stage;
    return stage;
  }
  /** Call only after exact record and associated model snapshot have been synced. */
  commitStage(stage: AuthorityStage): Receipt {
    this.owns(stage);
    const receipt = decode<Receipt>(this.native.commitStage(stage.stageDigest));
    this.pending = undefined;
    if (receipt.outcome) this.ticket = undefined;
    return receipt;
  }
  /** Any uncertain write outcome requires reconstruction from the actual journal. */
  failStage(stage: AuthorityStage): void {
    this.owns(stage);
    this.native.failStage(stage.stageDigest);
    this.pending = undefined;
    this.ticket = undefined;
  }
  /** Returns a new ACK only after the async durable append callback resolves.
   * A known duplicate returns its original receipt without invoking persistence.
   */
  async admit(request: Submit, persist: PersistAuthorityStage): Promise<Receipt> {
    const stage = this.stageAdmission(request);
    if (stage.status === "duplicate") return stage.receipt;
    return this.persist(stage, persist);
  }
  /** Worker must independently validate before calling. The callback persists
   * accepted source/design/model with the terminal record before this returns.
   */
  async complete(prepared: PreparedCommand, completion: ValidatedCompletion, persist: PersistAuthorityStage): Promise<Receipt> {
    return this.persist(this.stageValidatedCompletion(prepared, completion), persist);
  }
  dispose(): void {
    if (!this.disposed) {
      if (this.pending) throw Error("Resolve pending authority persistence before disposing the host");
      this.disposed = true; this.ticket = undefined; this.native.free();
    }
  }
  private async persist(stage: AuthorityStage, persist: PersistAuthorityStage): Promise<Receipt> {
    try { await persist(stage); }
    catch (failure) { this.failStage(stage); throw failure; }
    return this.commitStage(stage);
  }
  private owns(stage: AuthorityStage): void {
    this.live(); if (stage !== this.pending) throw Error("Unknown or stale authority stage");
  }
  private live(): void { if (this.disposed) throw Error("Authority host has been disposed"); }
}

/** Create only in a trusted server process after host authentication/model rebuild. */
export async function createDocumentAuthorityHost(options: AuthorityHostOptions): Promise<DocumentAuthorityHost> {
  const configuration = encode(options.configuration);
  const records = options.records === undefined ? undefined : encode(options.records);
  const path = "./wasm/geosolve_collaboration_wasm.js";
  const module: AuthorityWasmModule = options.wasmModule ?? await import(path);
  let bytes = options.wasm;
  if (!bytes) {
    const url = new URL("./wasm/geosolve_collaboration_wasm_bg.wasm", import.meta.url);
    if (url.protocol === "file:") {
      const nodeFs = "node:fs/promises";
      bytes = await (await import(nodeFs)).readFile(url);
    } else bytes = url;
  }
  await module.default({ module_or_path: bytes! });
  return new DocumentAuthorityHost(records === undefined
    ? new module.TrustedDocumentHost(configuration)
    : module.TrustedDocumentHost.restore(configuration, records));
}
