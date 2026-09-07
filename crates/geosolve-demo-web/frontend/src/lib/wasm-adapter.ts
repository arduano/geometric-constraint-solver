// SPDX-License-Identifier: GPL-3.0-or-later
import type { PointerSample, WheelSample, WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { assertWorkbenchSnapshot, markCanvasOnlySnapshot, stampCanvasSnapshot } from "./adapter";
import { freezeDrawFrame } from "./canvas-scene";
import type { ToolCatalog } from "./tool-catalog";
import { assertToolCatalog } from "./tool-catalog";

/** Structural view of the generated wasm-bindgen handle, kept out of generated source. */
export interface JsonWorkbenchHandle {
  snapshot(): string;
  toolCatalog(): string;
  dispatch(request: string): string;
  managedCompilerContext(): string;
  pointer(request: string): string;
  wheel(request: string): string;
  resize(request: string): string;
  cancel(request: string): string;
  exportProject(): string;
  persistProject(): string;
  exportReproduction(): string;
  exportInteractionTrace(): string;
  intentRpc(request: string): string;
  codeControlRpc(request: string): string;
  free?(): void;
}

export interface JsonWorkbenchHandleConstructor {
  new (request: string): JsonWorkbenchHandle;
  restoreLegacyCodeWorkbench?(persisted: string): JsonWorkbenchHandle;
}

function decodeSnapshot(json: string): WorkbenchSnapshot {
  const value = JSON.parse(json) as WorkbenchSnapshot;
  if (value?.frame) freezeDrawFrame(value.frame.scene);
  return assertWorkbenchSnapshot(value);
}

/** Promise-shaped browser adapter over the synchronous, instance-scoped Rust JSON boundary. */
export class WasmWorkbenchAdapter implements WorkbenchAdapter {
  private handle: JsonWorkbenchHandle | null = null;
  private catalog: ToolCatalog | null = null;
  private current: WorkbenchSnapshot | null = null;
  private sequence = 0;

  constructor(private readonly Handle: JsonWorkbenchHandleConstructor) {}

  async construct(input: { version: 2; persistedProject?: string }): Promise<WorkbenchSnapshot> {
    let candidate: JsonWorkbenchHandle;
    try {
      candidate = new this.Handle(JSON.stringify(input));
    } catch (error) {
      if (!input.persistedProject || !this.Handle.restoreLegacyCodeWorkbench) throw error;
      // Pass the original bytes to Rust. Parsing and rewriting here could erase
      // duplicate fields before the strict legacy/authority decoders see them.
      candidate = this.Handle.restoreLegacyCodeWorkbench(input.persistedProject);
    }
    try {
      const catalog = assertToolCatalog(JSON.parse(candidate.toolCatalog()) as ToolCatalog);
      const snapshot = stampCanvasSnapshot(decodeSnapshot(candidate.snapshot()), ++this.sequence);
      const previous = this.handle;
      this.handle = candidate;
      this.catalog = catalog;
      this.current = snapshot;
      previous?.free?.();
      return snapshot;
    } catch (error) {
      candidate.free?.();
      throw error;
    }
  }

  async toolCatalog() {
    if (!this.catalog) throw new Error("Workbench tool catalog is unavailable before construction");
    return this.catalog;
  }
  async snapshot() { return this.decodeFull(this.required().snapshot()); }
  async dispatch(input: { version: 2; command: string; payload?: unknown }) { return this.decodeFull(this.required().dispatch(JSON.stringify(input))); }
  async managedCompilerContext() { return JSON.parse(this.required().managedCompilerContext()) as { version: 2; patches: Record<string, unknown> }; }
  async pointer(input: PointerSample) { return this.decodeUpdate(this.required().pointer(JSON.stringify(input))); }
  async wheel(input: WheelSample) { return this.decodeUpdate(this.required().wheel(JSON.stringify(input))); }
  async wheelBatch(inputs: WheelSample[]) { return this.decodeUpdate(this.required().wheel(JSON.stringify({ version: 2, samples: inputs }))); }
  async resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return this.decodeUpdate(this.required().resize(JSON.stringify(input))); }
  async cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) { return this.decodeUpdate(this.required().cancel(JSON.stringify(input))); }
  async exportProject() { return JSON.parse(this.required().exportProject()) as { version: 2; filename: string; contents: string }; }
  async persistProject() { return JSON.parse(this.required().persistProject()) as { version: 2; contents: string }; }
  async exportReproduction() { return JSON.parse(this.required().exportReproduction()) as { version: 2; filename: string; contents: string }; }
  async exportInteractionTrace() { return JSON.parse(this.required().exportInteractionTrace()) as { version: 2; filename: string; contents: string }; }

  applyIntentRpc(request: string) { return this.required().intentRpc(request); }
  applyCodeControlRpc(request: string) { return this.required().codeControlRpc(request); }

  private decodeFull(json: string): WorkbenchSnapshot {
    const snapshot = stampCanvasSnapshot(decodeSnapshot(json), ++this.sequence);
    this.current = snapshot;
    return snapshot;
  }

  private decodeUpdate(json: string): WorkbenchSnapshot | null {
    if (json === "null") return null;
    const value = JSON.parse(json) as { kind?: string; version?: number; revision?: number; frame?: WorkbenchSnapshot["frame"] };
    if (value?.kind === "frame") {
      const base = this.current;
      if (!base || value.version !== 2 || !Number.isSafeInteger(value.revision) || value.revision !== base.revision
        || base.pendingManagedMutation || !value.frame || typeof value.frame.ariaLabel !== "string"
        || Object.keys(value).some((key) => !["version", "kind", "revision", "frame"].includes(key))) {
        throw new Error("Invalid canvas update: no matching settled workbench snapshot");
      }
      freezeDrawFrame(value.frame.scene);
      const next = markCanvasOnlySnapshot(stampCanvasSnapshot({ ...base, frame: value.frame }, ++this.sequence));
      this.current = next;
      return next;
    }
    // Full interaction results still pass every workbench/managed-ticket check.
    const next = value as WorkbenchSnapshot;
    if (next?.frame) freezeDrawFrame(next.frame.scene);
    this.current = stampCanvasSnapshot(assertWorkbenchSnapshot(next), ++this.sequence);
    return this.current;
  }

  private required() {
    if (!this.handle) throw new Error("Workbench adapter has not been constructed");
    return this.handle;
  }
}
