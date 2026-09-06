// SPDX-License-Identifier: GPL-3.0-or-later
import type { PointerSample, WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { assertWorkbenchSnapshot } from "./adapter";
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
  return assertWorkbenchSnapshot(JSON.parse(json) as WorkbenchSnapshot);
}

function decodeOptionalSnapshot(json: string): WorkbenchSnapshot | null {
  if (json === "null") return null;
  return decodeSnapshot(json);
}

/** Promise-shaped browser adapter over the synchronous, instance-scoped Rust JSON boundary. */
export class WasmWorkbenchAdapter implements WorkbenchAdapter {
  private handle: JsonWorkbenchHandle | null = null;
  private catalog: ToolCatalog | null = null;

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
      const snapshot = decodeSnapshot(candidate.snapshot());
      const previous = this.handle;
      this.handle = candidate;
      this.catalog = catalog;
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
  async snapshot() { return decodeSnapshot(this.required().snapshot()); }
  async dispatch(input: { version: 2; command: string; payload?: unknown }) { return decodeSnapshot(this.required().dispatch(JSON.stringify(input))); }
  async managedCompilerContext() { return JSON.parse(this.required().managedCompilerContext()) as { version: 2; patches: Record<string, unknown> }; }
  async pointer(input: PointerSample) { return decodeOptionalSnapshot(this.required().pointer(JSON.stringify(input))); }
  async wheel(input: { version: 2; x: number; y: number; deltaX: number; deltaY: number; ctrl: boolean }) { return decodeOptionalSnapshot(this.required().wheel(JSON.stringify(input))); }
  async resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return decodeOptionalSnapshot(this.required().resize(JSON.stringify(input))); }
  async cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) { return decodeOptionalSnapshot(this.required().cancel(JSON.stringify(input))); }
  async exportProject() { return JSON.parse(this.required().exportProject()) as { version: 2; filename: string; contents: string }; }
  async persistProject() { return JSON.parse(this.required().persistProject()) as { version: 2; contents: string }; }
  async exportReproduction() { return JSON.parse(this.required().exportReproduction()) as { version: 2; filename: string; contents: string }; }
  async exportInteractionTrace() { return JSON.parse(this.required().exportInteractionTrace()) as { version: 2; filename: string; contents: string }; }

  applyIntentRpc(request: string) { return this.required().intentRpc(request); }
  applyCodeControlRpc(request: string) { return this.required().codeControlRpc(request); }

  private required() {
    if (!this.handle) throw new Error("Workbench adapter has not been constructed");
    return this.handle;
  }
}
