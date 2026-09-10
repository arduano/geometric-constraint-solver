// SPDX-License-Identifier: GPL-3.0-or-later
import { recordedSketch, type Sketch, type GeneratedSketchArtifact } from "@geosolve/sketch-code";

import { EditableSession, type EditableNativeHandle, type EditableDesign } from "./session.js";
export { EditableSession, type EditableSessionToken, type EditableSessionState, type EditableDesign, type EditableUpdate } from "./session.js";
export type { AuthoringAction, AuthoringReceipt, AuthoringUpdate, AuthoringValueChange, AuthoringValueWrite, PreparedAuthoring } from "./session.js";
export { RetainedPointGesture, type PointGestureCommand, type PointGestureFrame, type PointGestureHandle, type PointGestureSample, type PointGestureTarget, type PointGestureTerminal, type PointGestureViewport, type PointOwnerAddress, type PointWritableAddress, type PreparedPointGestureCommit } from "./point-gesture.js";

export interface EngineNativeHandle extends Partial<EditableNativeHandle> {
  compileProjectJson(json: string): string;
  evaluateGenerated(json: string): string;
  evaluateManaged(json: string): string;
  exportProfiles(resultId: string, chordErrorMm: number): string;
  exportProfilesForOutput?(resultId: string, chordErrorMm: number, output: string): string;
  releaseResult(resultId: string): boolean;
  free(): void;
}
export interface EngineWasmModule {
  default(options?: { module_or_path: Uint8Array | URL | Response | WebAssembly.Module }): Promise<unknown>;
  SketchEngine: new () => EngineNativeHandle;
}
export interface EngineOptions {
  /** Bundlers may supply their own imported WASM module and bytes. */
  wasmModule?: EngineWasmModule;
  wasm?: Uint8Array | URL | Response | WebAssembly.Module;
}
export interface AcceptedResult {
  readonly format: "geosolve-engine-result-v1";
  readonly status: "accepted";
  readonly result_id: string;
  readonly input_digest: string;
  readonly mode: "editable" | "generator";
  readonly capabilities: { readonly managed_source_edits: boolean; readonly reverse_geometry_edits: boolean; readonly profile_export: boolean };
  readonly document: { readonly title?: string; readonly description?: string };
  readonly validation: { readonly hard_residuals_validated: boolean; readonly all_active_features_current: boolean; readonly maximum_normalized_hard_residual?: number };
  readonly geometry: {
    readonly points: readonly { readonly id: unknown; readonly position: readonly [number, number] }[];
    readonly scalars: readonly { readonly id: unknown; readonly value: number }[];
    readonly curves: readonly Readonly<Record<string, unknown>>[];
    readonly computed_edges: readonly Readonly<Record<string, unknown>>[];
  };
  readonly named_outputs: Readonly<Record<string, unknown>>;
  readonly named_geometry: Readonly<Record<string, { readonly points: readonly unknown[]; readonly spans: readonly unknown[]; readonly computed_edges: readonly number[] }>>;
}
export interface EvaluationFailure {
  readonly status: "rejected" | "cancelled";
  readonly diagnostics: readonly { readonly code: string; readonly detail: string }[];
}
export type EvaluationResult = AcceptedResult | EvaluationFailure;
export interface ProfileExport {
  readonly format: "geosolve-baked-profile-v1";
  readonly units: "mm";
  readonly sampling: { readonly max_chord_error_mm: number };
  readonly regions: readonly { readonly id: string; readonly outer: readonly (readonly [number, number])[]; readonly holes: readonly (readonly (readonly [number, number])[])[] }[];
}
export type GeneratorRequest<Inputs, Result> = {
  readonly definition: (inputs: Inputs) => Sketch<Result>;
  readonly parameters: Inputs;
  readonly signal?: AbortSignal;
};

/** Hosts own loading and UI. Use a terminable worker for unbounded generator code. */
export class Engine {
  private current?: AcceptedResult;
  private readonly retained = new Set<AcceptedResult>();
  private disposed = false;
  private sequence = 0;
  constructor(private readonly native: EngineNativeHandle) {}
  get lastAccepted(): AcceptedResult | undefined { return this.current; }

  async evaluate<Inputs, Result>(request: GeneratorRequest<Inputs, Result>): Promise<EvaluationResult> {
    return this.run(() => this.native.evaluateGenerated(JSON.stringify(recordedSketch(request.definition(request.parameters)))), request.signal);
  }
  async evaluateGenerated(artifact: GeneratedSketchArtifact, options: { signal?: AbortSignal } = {}): Promise<EvaluationResult> {
    return this.run(() => this.native.evaluateGenerated(JSON.stringify(artifact)), options.signal);
  }
  /** Validates local compiler output and exact patch pins into the existing project wire. */
  compileProject(input: { project: string; compiled: unknown; customFiles: Readonly<Record<string, unknown>>; artifacts: Readonly<Record<string, unknown>>; lock: unknown }): string {
    this.assertLive();
    return this.native.compileProjectJson(JSON.stringify(input));
  }
  /** Open managed history using authentic source and an optional semantic design sidecar. */
  openEditableSession(project: unknown, options: { design?: EditableDesign | string } = {}): EditableSession {
    this.assertLive();
    return new EditableSession({ native: this.native, isLive: () => !this.disposed,
      accept: (result) => { this.admit(result); ++this.sequence; return result; },
    }, project, options);
  }
  /** Existing authenticated CodeProject wire. No synthetic compiler receipts. */
  async evaluateEditable(project: unknown, options: { signal?: AbortSignal } = {}): Promise<EvaluationResult> {
    return this.run(() => this.native.evaluateManaged(typeof project === "string" ? project : JSON.stringify(project)), options.signal);
  }
  async exportProfiles(result: AcceptedResult, options: { chordErrorMm: number; output?: string }): Promise<ProfileExport> {
    this.assertLive();
    if (!this.retained.has(result)) throw Error("Result is released or belongs to another engine");
    if (!Number.isFinite(options.chordErrorMm) || options.chordErrorMm <= 0) throw Error("chordErrorMm must be finite and positive");
    if (options.output !== undefined && !this.native.exportProfilesForOutput) throw Error("This engine build does not support named profile selection");
    const json = options.output === undefined ? this.native.exportProfiles(result.result_id, options.chordErrorMm)
      : this.native.exportProfilesForOutput!(result.result_id, options.chordErrorMm, options.output);
    return freeze(JSON.parse(json)) as ProfileExport;
  }
  release(result: AcceptedResult): void {
    this.assertLive();
    if (!this.retained.delete(result)) return;
    // Equivalent evaluations can share native IDs; retain it until its last JS owner releases.
    if (![...this.retained].some((other) => other.result_id === result.result_id)) this.native.releaseResult(result.result_id);
    if (this.current === result) this.current = undefined;
  }
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.sequence++;
    this.retained.clear();
    this.current = undefined;
    this.native.free();
  }
  private admit(result: AcceptedResult): void {
    if (result.format !== "geosolve-engine-result-v1" || result.status !== "accepted" || !result.validation.hard_residuals_validated || !result.validation.all_active_features_current) throw Error("Native result lacks independent acceptance evidence");
    freeze(result);
    this.retained.add(result);
    this.current = result;
  }
  private assertLive() { if (this.disposed) throw Error("Engine has been disposed"); }
  private async run(action: () => string, signal?: AbortSignal): Promise<EvaluationResult> {
    this.assertLive();
    const sequence = ++this.sequence;
    // A queued newer request or AbortSignal can cancel before any generator runs.
    await Promise.resolve();
    const cancelled = () => this.disposed || signal?.aborted || sequence !== this.sequence;
    if (cancelled()) return failure("cancelled", "Evaluation was cancelled or superseded");
    try {
      const result = JSON.parse(action()) as AcceptedResult;
      if (result.format !== "geosolve-engine-result-v1" || result.status !== "accepted" || !result.validation.hard_residuals_validated || !result.validation.all_active_features_current) throw Error("Native result lacks independent acceptance evidence");
      if (cancelled()) {
        if (![...this.retained].some((other) => other.result_id === result.result_id)) this.native.releaseResult(result.result_id);
        return failure("cancelled", "Evaluation was cancelled or superseded");
      }
      this.admit(result);
      return result;
    } catch (error) {
      return failure(cancelled() ? "cancelled" : "rejected", String(error));
    }
  }
}
function failure(status: "cancelled" | "rejected", detail: string): EvaluationFailure {
  return freeze({ status, diagnostics: [{ code: status, detail }] }) as EvaluationFailure;
}
function freeze(value: unknown): unknown {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze); Object.freeze(value);
  }
  return value;
}

export async function createEngine(options: EngineOptions = {}): Promise<Engine> {
  const path = "./wasm/geosolve_sketch_engine_wasm.js";
  const module: EngineWasmModule = options.wasmModule ?? await import(path);
  let bytes = options.wasm;
  if (!bytes) {
    const url = new URL("./wasm/geosolve_sketch_engine_wasm_bg.wasm", import.meta.url);
    if (url.protocol === "file:") {
      // Loaded only by Node; browser bundles have no filesystem dependency at runtime.
      const nodeFs = "node:fs/promises";
      bytes = await (await import(nodeFs)).readFile(url);
    } else bytes = url;
  }
  await module.default({ module_or_path: bytes! });
  return new Engine(new module.SketchEngine());
}
