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

export type JsonWorkbenchHandleConstructor = new (request: string) => JsonWorkbenchHandle;

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

  async construct(input: { version: 1; persistedProject?: string }): Promise<WorkbenchSnapshot> {
    this.handle?.free?.();
    this.handle = new this.Handle(JSON.stringify(input));
    this.catalog = assertToolCatalog(JSON.parse(this.handle.toolCatalog()) as ToolCatalog);
    return decodeSnapshot(this.handle.snapshot());
  }

  async toolCatalog() {
    if (!this.catalog) throw new Error("Workbench tool catalog is unavailable before construction");
    return this.catalog;
  }
  async snapshot() { return decodeSnapshot(this.required().snapshot()); }
  async dispatch(input: { version: 1; command: string; payload?: unknown }) { return decodeSnapshot(this.required().dispatch(JSON.stringify(input))); }
  async managedCompilerContext() { return JSON.parse(this.required().managedCompilerContext()) as { version: 1; patches: Record<string, unknown> }; }
  async pointer(input: PointerSample) { return decodeOptionalSnapshot(this.required().pointer(JSON.stringify(input))); }
  async wheel(input: { version: 1; x: number; y: number; deltaX: number; deltaY: number; ctrl: boolean }) { return decodeOptionalSnapshot(this.required().wheel(JSON.stringify(input))); }
  async resize(input: { version: 1; width: number; height: number; pixelRatio: number }) { return decodeOptionalSnapshot(this.required().resize(JSON.stringify(input))); }
  async cancel(input: { version: 1; reason: "escape" | "lost-capture" | "blur" }) { return decodeOptionalSnapshot(this.required().cancel(JSON.stringify(input))); }
  async exportProject() { return JSON.parse(this.required().exportProject()) as { version: 1; filename: string; contents: string }; }
  async persistProject() { return JSON.parse(this.required().persistProject()) as { version: 1; contents: string }; }
  async exportReproduction() { return JSON.parse(this.required().exportReproduction()) as { version: 1; filename: string; contents: string }; }
  async exportInteractionTrace() { return JSON.parse(this.required().exportInteractionTrace()) as { version: 1; filename: string; contents: string }; }

  applyIntentRpc(request: string) { return this.required().intentRpc(request); }
  applyCodeControlRpc(request: string) { return this.required().codeControlRpc(request); }

  private required() {
    if (!this.handle) throw new Error("Workbench adapter has not been constructed");
    return this.handle;
  }
}
