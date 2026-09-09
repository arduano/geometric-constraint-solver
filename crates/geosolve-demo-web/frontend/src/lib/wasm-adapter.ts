// SPDX-License-Identifier: GPL-3.0-or-later
import type { PointerSample, WheelSample, WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { assertWorkbenchSnapshot, markCanvasOnlySnapshot, stampCanvasSnapshot } from "./adapter";
import { freezeDrawFrame } from "./canvas-scene";
import type { ToolCatalog } from "./tool-catalog";
import { assertToolCatalog } from "./tool-catalog";

/** Structural view of the generated wasm-bindgen handle, kept out of generated source. */
export interface JsonWorkbenchHandle {
  snapshot(): string;
  interactionSnapshot?(): string;
  interactionApply?(request: string): string;
  toolCatalog(): string;
  dispatch(request: string): string;
  managedCompilerContext(): string;
  pointer(request: string): string;
  wheel(request: string): string;
  resize(request: string): string;
  cancel(request: string): string;
  exportWorkspaceDesign?(): string;
  exportProject(): string;
  bakeProfile?(maxChordErrorMm: number): string;
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

const selectionUpdateFields = ["version", "kind", "revision", "frame", "navigation", "selectedDeclarations", "selection", "selectedGeometryRole"] as const;

function validSelection(value: unknown): value is WorkbenchSnapshot["selection"] | null {
  if (value === null) return true;
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const selection = value as Record<string, unknown>;
  if (typeof selection.id !== "string" || typeof selection.label !== "string" || typeof selection.kind !== "string"
    || (selection.ownership !== undefined && typeof selection.ownership !== "string")
    || Object.keys(selection).some((key) => !["id", "label", "kind", "ownership", "source", "metadata"].includes(key))) return false;
  if (selection.source === undefined) return true;
  if (!selection.source || typeof selection.source !== "object" || Array.isArray(selection.source)) return false;
  const source = selection.source as Record<string, unknown>;
  return typeof source.path === "string" && typeof source.from === "number" && typeof source.to === "number"
    && Number.isSafeInteger(source.from) && source.from >= 0 && Number.isSafeInteger(source.to) && source.to >= source.from
    && Object.keys(source).every((key) => ["path", "from", "to"].includes(key));
}

function explorerRows(values: WorkbenchSnapshot["explorer"]): Map<string, WorkbenchSnapshot["explorer"][number]> {
  const rows = new Map<string, WorkbenchSnapshot["explorer"][number]>();
  function visit(children: WorkbenchSnapshot["explorer"]) {
    for (const row of children) {
      rows.set(row.id, row);
      visit(row.children);
    }
  }
  visit(values);
  return rows;
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
  async interactionSnapshot(): Promise<{ snapshot: WorkbenchSnapshot; seed: Record<string, unknown> }> {
    const handle = this.required();
    if (!handle.interactionSnapshot) throw Error("Local interaction export requires the current WASM build");
    const exported = JSON.parse(handle.interactionSnapshot()) as { snapshot?: WorkbenchSnapshot; seed?: unknown } | null;
    if (!exported || typeof exported !== "object" || Array.isArray(exported)
      || Object.keys(exported).some((key) => !["snapshot", "seed"].includes(key))
      || !exported.seed || typeof exported.seed !== "object" || Array.isArray(exported.seed)
      || !exported.snapshot) throw Error("Malformed native interaction export");
    // Native captures the seed and chrome together. Keep seed contents opaque;
    // install the full validated chrome as the next incremental-frame base.
    const snapshot = assertWorkbenchSnapshot(exported.snapshot);
    freezeDrawFrame(snapshot.frame.scene);
    this.current = stampCanvasSnapshot(snapshot, ++this.sequence);
    return { snapshot: this.current, seed: exported.seed as Record<string, unknown> };
  }
  async interactionApply(input: Record<string, unknown>): Promise<WorkbenchSnapshot> {
    const handle = this.required();
    if (!handle.interactionApply) throw Error("Local interaction apply requires the current WASM build");
    return this.decodeFull(handle.interactionApply(JSON.stringify(input)));
  }
  async dispatch(input: { version: 2; command: string; payload?: unknown }) {
    const next = this.decodeUpdate(this.required().dispatch(JSON.stringify(input)));
    if (!next && this.current && ["dimensions.navigation.end", "dimensions.hover.clear"].includes(input.command)) {
      // Native can defer paint-only cleanup while a compiler ticket is pending.
      // Retain that exact snapshot without starting another UI/compiler update.
      this.current = markCanvasOnlySnapshot(stampCanvasSnapshot({ ...this.current }, ++this.sequence));
      return this.current;
    }
    if (!next) throw new Error("Workbench command returned no snapshot");
    return next;
  }
  async managedCompilerContext() { return JSON.parse(this.required().managedCompilerContext()) as { version: 2; patches: Record<string, unknown> }; }
  async pointer(input: PointerSample) { return this.decodeUpdate(this.required().pointer(JSON.stringify(input))); }
  async wheel(input: WheelSample) { return this.decodeUpdate(this.required().wheel(JSON.stringify(input))); }
  async wheelBatch(inputs: WheelSample[]) { return this.decodeUpdate(this.required().wheel(JSON.stringify({ version: 2, samples: inputs }))); }
  async resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return this.decodeUpdate(this.required().resize(JSON.stringify(input))); }
  async cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) { return this.decodeUpdate(this.required().cancel(JSON.stringify(input))); }
  async exportWorkspaceDesign(): Promise<Record<string, unknown> | null> {
    const handle = this.required();
    if (!handle.exportWorkspaceDesign) throw Error("Workspace design export requires the current WASM build");
    return JSON.parse(handle.exportWorkspaceDesign()) as Record<string, unknown> | null;
  }
  async exportProject() { return JSON.parse(this.required().exportProject()) as { version: 2; filename: string; contents: string }; }
  async bakeProfile(maxChordErrorMm: number): Promise<Record<string, unknown>> {
    const handle = this.required();
    if (!handle.bakeProfile) throw new Error("Rebuild the M98-B WASM package to enable bakeProfile");
    return JSON.parse(handle.bakeProfile(maxChordErrorMm)) as Record<string, unknown>;
  }
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
    const value = JSON.parse(json) as { kind?: string; version?: number; revision?: number; frame?: WorkbenchSnapshot["frame"]; dimensions?: WorkbenchSnapshot["dimensions"] };
    if (value?.kind === "selection") {
      const update = value as typeof value & {
        navigation: NonNullable<WorkbenchSnapshot["navigation"]>;
        selectedDeclarations: string[];
        selection: WorkbenchSnapshot["selection"] | null;
        selectedGeometryRole: WorkbenchSnapshot["presentation"]["selectedGeometryRole"] | null;
        dimensions?: WorkbenchSnapshot["dimensions"];
      };
      const base = this.current;
      if (!base?.navigation || update.version !== 2 || !Number.isSafeInteger(update.revision) || update.revision !== base.revision
        || base.pendingManagedMutation || update.navigation?.authority !== base.navigation.authority
        || update.navigation.canNavigateSource !== base.navigation.canNavigateSource
        || update.navigation.unavailableReason !== base.navigation.unavailableReason
        || !update.frame || !Array.isArray(update.selectedDeclarations)
        || !update.selectedDeclarations.every((id) => typeof id === "string")
        || !validSelection(update.selection)
        || (update.selectedGeometryRole !== null && !["profile", "construction", "mixed"].includes(update.selectedGeometryRole ?? ""))
        || selectionUpdateFields.some((key) => !Object.hasOwn(update, key))
        || (base.dimensions !== undefined && update.dimensions === undefined) || update.dimensions === null
        || Object.keys(update).some((key) => key !== "dimensions" && !selectionUpdateFields.includes(key as typeof selectionUpdateFields[number]))) {
        throw new Error("Invalid selection update: no matching accepted workbench authority");
      }
      const knownRows = explorerRows(base.explorer);
      const selected = new Set(update.selectedDeclarations);
      if (selected.size !== update.selectedDeclarations.length || update.selectedDeclarations.some((id) => {
        const row = knownRows.get(id);
        return !row || row.rowKind === "group";
      })) {
        throw new Error("Invalid selection update: unknown or repeated declaration identity");
      }
      freezeDrawFrame(update.frame.scene);
      const rows = (values: WorkbenchSnapshot["explorer"]): WorkbenchSnapshot["explorer"] => {
        const next = values.map((row) => {
          const children = rows(row.children);
          const isSelected = selected.has(row.id);
          return row.selected === isSelected && children === row.children ? row : { ...row, selected: isSelected, children };
        });
        return next.every((row, index) => row === values[index]) ? values : next;
      };
      const next = assertWorkbenchSnapshot({ ...base, frame: update.frame, navigation: update.navigation,
        explorer: rows(base.explorer), selection: update.selection ?? undefined,
        dimensions: update.dimensions ?? base.dimensions,
        presentation: { ...base.presentation, selectedGeometryRole: update.selectedGeometryRole ?? undefined },
      });
      if (next.navigation!.rows.some((row) => !knownRows.has(row.id))
        || new Set(next.navigation!.rows.map((row) => row.id)).size !== next.navigation!.rows.length) {
        throw new Error("Invalid selection update: unknown or repeated navigation row");
      }
      this.current = stampCanvasSnapshot(next, ++this.sequence);
      return this.current;
    }
    if (value?.kind === "frame") {
      const base = this.current;
      if (!base || value.version !== 2 || !Number.isSafeInteger(value.revision) || value.revision !== base.revision
        || base.pendingManagedMutation || !value.frame || typeof value.frame.ariaLabel !== "string"
        || Object.keys(value).some((key) => !["version", "kind", "revision", "frame", "dimensions"].includes(key))
        || value.dimensions === null) {
        throw new Error("Invalid canvas update: no matching settled workbench snapshot");
      }
      freezeDrawFrame(value.frame.scene);
      const dimensionsChanged = value.dimensions !== undefined && JSON.stringify(value.dimensions) !== JSON.stringify(base.dimensions);
      const next = stampCanvasSnapshot(value.dimensions === undefined ? { ...base, frame: value.frame }
        : assertWorkbenchSnapshot({ ...base, frame: value.frame, dimensions: value.dimensions }), ++this.sequence);
      // Only a settled visibility change updates Inspector React state. Ordinary
      // wheel, pan and hover frames retain the existing canvas-only fast path.
      if (!dimensionsChanged) markCanvasOnlySnapshot(next);
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
