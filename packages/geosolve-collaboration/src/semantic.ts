// SPDX-License-Identifier: GPL-3.0-or-later
import { counter, unicode, encode, decode } from "./host-codec.js";
import type { OperationId } from "./host.js";

/** Canonical names and dependencies come from the trusted compiler, never scene labels. */
export interface SemanticTarget { readonly object: string; readonly generation: number }
export interface DeletionPlan { readonly roots: readonly SemanticTarget[]; readonly closure: readonly SemanticTarget[] }
export interface PropertyAddress { readonly target: SemanticTarget; readonly property: string }
export interface StatementPosition { readonly previous: SemanticTarget | null; readonly next: SemanticTarget | null }
export interface ObjectDescription {
  readonly target: SemanticTarget;
  readonly dependencies: readonly SemanticTarget[];
  readonly payload: unknown;
  readonly position: StatementPosition;
}
export interface DependencyChange { readonly target: SemanticTarget; readonly before: readonly SemanticTarget[]; readonly after: readonly SemanticTarget[] }
export interface ReorderChange { readonly target: SemanticTarget; readonly before: StatementPosition; readonly after: StatementPosition }
export interface StructuralInverse {
  readonly create: readonly ObjectDescription[];
  readonly delete: DeletionPlan | null;
  readonly dependencies: readonly DependencyChange[];
  readonly reorders: readonly ReorderChange[];
  readonly restored: readonly { readonly before: SemanticTarget; readonly after: SemanticTarget }[];
}
export interface CreationPosition {
  readonly previous: SemanticTarget | SemanticTargetReference | null;
  readonly next: SemanticTarget | SemanticTargetReference | null;
}
export interface StructuralRecord {
  readonly created?: readonly { readonly object: string; readonly payload: unknown; readonly position: CreationPosition }[];
  readonly deleted?: readonly { readonly target: SemanticTarget; readonly payload: unknown; readonly position: StatementPosition }[];
  readonly reorders?: readonly ReorderChange[];
}
export interface PropertyChange { readonly address: PropertyAddress; readonly before: unknown; readonly after: unknown }
export interface SemanticHostConfiguration {
  readonly documentEpoch: string;
  /** Fresh process epoch after durable reconstruction. */
  readonly serverEpoch: string;
  readonly objects?: readonly { readonly object: string; readonly dependencies?: readonly string[] }[];
  readonly limits?: {
    readonly maxObjectsIncludingTombstones?: number;
    readonly maxDependenciesPerObject?: number;
    readonly maxTotalDependencies?: number;
  };
}
/** Store both native JSON strings unchanged with the authenticated model revision. */
export interface SemanticCheckpoint {
  readonly documentEpoch: string;
  readonly revision: number;
  readonly targetsJson: string;
  readonly historyJson: string;
}
export interface SemanticSnapshot {
  readonly revision: number;
  readonly historyRevision: number;
  readonly highWater: number;
  readonly dependencyCount: number;
  readonly hasPendingStage: boolean;
  readonly needsRecovery: boolean;
}
/** Descriptor only; the checked InversePlan remains owned by native Rust. */
export interface PreparedSemanticInverse {
  readonly ticket: string;
  readonly basisRevision: number;
  readonly contribution: OperationId;
  readonly direction: "undo" | "redo";
  readonly structural: StructuralInverse;
  readonly changes: readonly PropertyChange[];
}
export type SemanticTargetReference = { readonly kind: "existing"; readonly target: SemanticTarget }
  | { readonly kind: "created"; readonly object: string };
export interface SemanticRecord {
  readonly basisRevision: number;
  readonly revision: number;
  readonly operation: OperationId;
  readonly changes: readonly PropertyChange[];
}
/** Validated compiler inventory updates. Deletions retain exactly reviewed closures;
 * all created names are allocated before forward dependency references are resolved.
 * Supply record.structural with trusted compiler observations to put lifecycle,
 * dependency and reorder contributions in the same per-user Undo timeline.
 */
export interface SemanticTransaction {
  readonly basisRevision: number;
  readonly revision: number;
  readonly create?: readonly { readonly object: string; readonly dependencies?: readonly SemanticTargetReference[] }[];
  readonly deletions?: readonly DeletionPlan[];
  readonly dependencies?: readonly { readonly target: SemanticTargetReference; readonly dependencies: readonly SemanticTargetReference[] }[];
  readonly record?: { readonly operation: OperationId; readonly changes: readonly PropertyChange[]; readonly structural?: StructuralRecord };
}
/** Persist exact strings with accepted model/source/authority outcome before commit. */
export interface SemanticStage {
  readonly status: "staged";
  readonly stageId: string;
  readonly basisRevision: number;
  readonly revision: number;
  readonly targetsJson: string;
  readonly historyJson: string;
  readonly created: readonly SemanticTarget[];
}
export type PersistSemanticStage = (stage: SemanticStage) => Promise<void>;
export interface SemanticNativeHandle {
  snapshot(): string;
  checkpoint(): string;
  current(object: string): string;
  authenticate(targetJson: string): void;
  planDelete(rootsJson: string): string;
  authenticateDelete(planJson: string): void;
  propertyOwner(addressJson: string): string;
  prepareUndo(userId: string): string;
  prepareRedo(userId: string): string;
  release(ticket: string): void;
  stageValidatedRecord(requestJson: string): string;
  stageValidatedTransaction(requestJson: string): string;
  stageValidatedInverse(ticket: string, operationJson: string, revisionJson: string): string;
  commitStage(stageId: string): string;
  failStage(stageId: string): void;
  discardUnpersistedStage(stageId: string): void;
  free(): void;
}
export interface SemanticWasmModule {
  default(options: { module_or_path: Uint8Array | URL | Response | WebAssembly.Module }): Promise<unknown>;
  TrustedSemanticHost: {
    new(configurationJson: string): SemanticNativeHandle;
    restore(configurationJson: string, checkpointJson: string): SemanticNativeHandle;
  };
}
export interface SemanticHostOptions {
  readonly configuration: SemanticHostConfiguration;
  readonly checkpoint?: SemanticCheckpoint;
  readonly wasmModule?: SemanticWasmModule;
  readonly wasm?: Uint8Array | URL | Response | WebAssembly.Module;
}
/** Trusted Node host adapter over actual Rust TargetLedger + ContributionHistory.
 * Synchronous staging stays separate from asynchronous durable publication.
 */
export class TrustedSemanticHost {
  private disposed = false;
  private pending?: SemanticStage;
  private readonly prepared = new Set<PreparedSemanticInverse>();
  constructor(private readonly native: SemanticNativeHandle) {}
  snapshot(): SemanticSnapshot { this.live(); return decode(this.native.snapshot()); }
  checkpoint(): SemanticCheckpoint { this.live(); return decode(this.native.checkpoint()); }
  current(object: string): SemanticTarget | null { this.live(); unicode(object); return decode(this.native.current(object)); }
  authenticate(target: SemanticTarget): void { this.live(); counter(target.generation); this.native.authenticate(encode(target)); }
  planDelete(roots: readonly SemanticTarget[]): DeletionPlan { this.live(); return decode(this.native.planDelete(encode(roots))); }
  authenticateDelete(plan: DeletionPlan): void { this.live(); this.native.authenticateDelete(encode(plan)); }
  propertyOwner(address: PropertyAddress): OperationId | null { this.live(); return decode(this.native.propertyOwner(encode(address))); }
  prepareUndo(userId: string): PreparedSemanticInverse {
    this.live(); unicode(userId); return this.retain(this.native.prepareUndo(userId));
  }
  prepareRedo(userId: string): PreparedSemanticInverse {
    this.live(); unicode(userId); return this.retain(this.native.prepareRedo(userId));
  }
  release(prepared: PreparedSemanticInverse): void {
    this.ownsPrepared(prepared); this.native.release(prepared.ticket); this.prepared.delete(prepared);
  }
  stageValidatedRecord(request: SemanticRecord): SemanticStage {
    this.live(); counter(request.basisRevision); counter(request.revision);
    return this.stage(this.native.stageValidatedRecord(encode(request)));
  }
  stageValidatedTransaction(request: SemanticTransaction): SemanticStage {
    this.live(); counter(request.basisRevision); counter(request.revision);
    return this.stage(this.native.stageValidatedTransaction(encode(request)));
  }
  stageValidatedInverse(prepared: PreparedSemanticInverse, operation: OperationId, revision: number): SemanticStage {
    this.ownsPrepared(prepared); counter(revision);
    return this.stage(this.native.stageValidatedInverse(prepared.ticket, encode(operation), encode(revision)));
  }
  /** Call only after the exact stage/model/source/authority envelope is durable. */
  commitStage(stage: SemanticStage): SemanticSnapshot {
    this.owns(stage);
    const result = decode<SemanticSnapshot>(this.native.commitStage(stage.stageId));
    this.pending = undefined; this.prepared.clear(); return result;
  }
  /** Only before any persistence has begun; uncertain writes require failStage. */
  discardUnpersistedStage(stage: SemanticStage): void {
    this.owns(stage); this.native.discardUnpersistedStage(stage.stageId); this.pending = undefined;
  }
  failStage(stage: SemanticStage): void {
    this.owns(stage); this.native.failStage(stage.stageId); this.pending = undefined; this.prepared.clear();
  }
  async record(request: SemanticRecord, persist: PersistSemanticStage): Promise<SemanticSnapshot> {
    return this.persist(this.stageValidatedRecord(request), persist);
  }
  async transact(request: SemanticTransaction, persist: PersistSemanticStage): Promise<SemanticSnapshot> {
    return this.persist(this.stageValidatedTransaction(request), persist);
  }
  async inverse(prepared: PreparedSemanticInverse, operation: OperationId, revision: number, persist: PersistSemanticStage): Promise<SemanticSnapshot> {
    return this.persist(this.stageValidatedInverse(prepared, operation, revision), persist);
  }
  dispose(): void {
    if (!this.disposed) {
      if (this.pending) throw Error("Resolve pending semantic persistence before disposing the host");
      this.disposed = true; this.prepared.clear(); this.native.free();
    }
  }
  private retain(text: string): PreparedSemanticInverse {
    const result = decode<PreparedSemanticInverse>(text); this.prepared.add(result); return result;
  }
  private stage(text: string): SemanticStage {
    const result = decode<SemanticStage>(text); this.pending = result; return result;
  }
  private owns(stage: SemanticStage): void {
    this.live(); if (stage !== this.pending) throw Error("Unknown or stale semantic stage");
  }
  private ownsPrepared(prepared: PreparedSemanticInverse): void {
    this.live(); if (!this.prepared.has(prepared)) throw Error("Unknown or stale semantic inverse ticket");
  }
  private async persist(stage: SemanticStage, persist: PersistSemanticStage): Promise<SemanticSnapshot> {
    try { await persist(stage); }
    catch (failure) { this.failStage(stage); throw failure; }
    return this.commitStage(stage);
  }
  private live(): void { if (this.disposed) throw Error("Semantic host has been disposed"); }
}

export async function createTrustedSemanticHost(options: SemanticHostOptions): Promise<TrustedSemanticHost> {
  const configuration = encode(options.configuration);
  if (options.checkpoint) counter(options.checkpoint.revision);
  const checkpoint = options.checkpoint === undefined ? undefined : encode(options.checkpoint);
  const path = "./wasm/geosolve_collaboration_wasm.js";
  const module: SemanticWasmModule = options.wasmModule ?? await import(path);
  let bytes = options.wasm;
  if (!bytes) {
    const url = new URL("./wasm/geosolve_collaboration_wasm_bg.wasm", import.meta.url);
    if (url.protocol === "file:") {
      const nodeFs = "node:fs/promises";
      bytes = await (await import(nodeFs)).readFile(url);
    } else bytes = url;
  }
  await module.default({ module_or_path: bytes! });
  return new TrustedSemanticHost(checkpoint === undefined
    ? new module.TrustedSemanticHost(configuration)
    : module.TrustedSemanticHost.restore(configuration, checkpoint));
}
