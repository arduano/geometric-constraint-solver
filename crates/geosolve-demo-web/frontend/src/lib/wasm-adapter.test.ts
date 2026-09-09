// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { MockWorkbenchAdapter } from "./mock-adapter";
import { WasmWorkbenchAdapter, type JsonWorkbenchHandle } from "./wasm-adapter";
import { getCanvasSnapshotSequence, isCanvasOnlySnapshot, type WorkbenchSnapshot } from "./adapter";

const icon = (key: string) => ({ key, svg: `<svg class="wb-palette-icon" viewBox="-10 -10 20 20" aria-hidden="true" focusable="false" data-icon-key="${key}"><path d="M-8 5L8-5"/></svg>` });
const tool = (stableId: string, toolId: string, label: string, key: string) => ({ stableId, toolId, label, group: label, icon: icon(key) });
const catalog = {
  version: 1,
  select: tool("sketch.select", "select", "Select", "geometry-select"),
  sections: [
    { id: "sketch", label: "Sketch", description: "Sketch tools", commands: [tool("sketch.variant.segment", "segment", "Segment", "geometry-segment")] },
    { id: "constraint", label: "Constraint", description: "Constraint tools", commands: [tool("constraint.coincident", "coincident", "Coincident", "coincident")] },
    { id: "dimension", label: "Dimension", description: "Dimension tools", commands: [tool("dimension.radius", "radius", "Radius", "radius")] },
    { id: "modify", label: "Modify", description: "Modify tools", commands: [tool("modify.fillet", "fillet", "Fillet", "feature-fillet")] },
  ],
  geometryRole: tool("inspector.geometry-role", "geometry-role", "Toggle Profile / Construction", "geometry-role-construction"),
};
const snapshot: WorkbenchSnapshot = { version: 2, revision: 0, project: { title: "Bridge", status: "accepted" }, presentation: { activeTool: "select", gridVisible: true, constructionVisible: true, visibilityRestoreAvailable: false, canUndo: false, canRedo: false, canFinish: false, geometryRole: "profile" }, frame: { scene: { format: "geosolve-draw-frame-v1", viewBox: [0, 0, 1000, 700], background: "#121617", provenance: { scene: "none" }, items: [] }, ariaLabel: "frame" }, source: { selectedPath: "", files: [], dirty: false }, explorer: [], parameters: [], problems: [] };

class FakeHandle implements JsonWorkbenchHandle {
  static requests: string[] = [];
  static catalogReads = 0;
  constructor(request: string) { FakeHandle.requests.push(request); }
  snapshot() { return JSON.stringify(snapshot); }
  toolCatalog() { FakeHandle.catalogReads += 1; return JSON.stringify(catalog); }
  managedCompilerContext() { return JSON.stringify({ version: 2, patches: {} }); }
  dispatch(request: string) { FakeHandle.requests.push(request); return JSON.stringify({ ...snapshot, revision: 1 }); }
  pointer() { return "null"; }
  wheel() { return "null"; }
  resize() { return "null"; }
  cancel() { return JSON.stringify(snapshot); }
  exportProject() { return JSON.stringify({ version: 2, filename: "project.json", contents: "{}" }); }
  persistProject() { return JSON.stringify({ version: 2, contents: "{}" }); }
  exportReproduction() { return JSON.stringify({ version: 2, filename: "repro.txt", contents: "repro" }); }
  exportInteractionTrace() { return JSON.stringify({ version: 2, filename: "trace.txt", contents: "trace" }); }
  intentRpc(request: string) { return request; }
  codeControlRpc(request: string) { return request; }
}

describe("WasmWorkbenchAdapter", () => {
  it("installs interaction export/apply snapshots as complete sequenced bases while forwarding opaque state", async () => {
    const exported = { ...snapshot, revision: 7, project: { title: "Exported", status: "accepted" as const } };
    const applied = { ...snapshot, revision: 8, project: { title: "Applied", status: "accepted" as const } };
    const seed = { format: "opaque-rust-seed", scene: "do not parse", state: { arbitrary: [1, "token"] } };
    let exportResponse = JSON.stringify({ snapshot: exported, seed });
    let applyResponse = JSON.stringify(applied);
    let frameRevision = 7;
    const calls: string[] = [];
    class InteractionHandle extends FakeHandle {
      interactionSnapshot() { return exportResponse; }
      interactionApply(input: string) { calls.push(input); return applyResponse; }
      override wheel() { return JSON.stringify({ version: 2, kind: "frame", revision: frameRevision, frame: snapshot.frame }); }
    }
    const adapter = new WasmWorkbenchAdapter(InteractionHandle);
    const original = await adapter.construct({ version: 2 });
    const result = await adapter.interactionSnapshot();
    expect(result.seed).toEqual(seed);
    expect(result.snapshot).toEqual(exported);
    expect(getCanvasSnapshotSequence(result.snapshot)).toBe(getCanvasSnapshotSequence(original)! + 1);
    expect(isCanvasOnlySnapshot(result.snapshot)).toBe(false);
    expect(Object.isFrozen(result.snapshot.frame.scene.items)).toBe(true);
    const afterExport = (await adapter.wheelBatch([]))!;
    expect(afterExport.project).toBe(result.snapshot.project);
    expect(isCanvasOnlySnapshot(afterExport)).toBe(true);

    const state = { sceneKey: "opaque identity", selection: ["native target"], view: [11, 12] };
    const updated = await adapter.interactionApply(state);
    expect(calls).toEqual([JSON.stringify(state)]);
    expect(updated).toEqual(applied);
    expect(isCanvasOnlySnapshot(updated)).toBe(false);
    expect(getCanvasSnapshotSequence(updated)).toBe(getCanvasSnapshotSequence(afterExport)! + 1);
    frameRevision = 8;
    expect((await adapter.wheelBatch([]))!.project).toBe(updated.project);

    for (const invalid of [{ snapshot: exported }, { snapshot: exported, seed: null }, { snapshot: exported, seed: [] }, { snapshot: {}, seed }, { snapshot: exported, seed, extra: true }]) {
      exportResponse = JSON.stringify(invalid);
      await expect(adapter.interactionSnapshot()).rejects.toThrow();
    }
    applyResponse = JSON.stringify({ ...applied, frame: { ...applied.frame, scene: { ...applied.frame.scene, viewBox: [0, 0, null, 700] } } });
    await expect(adapter.interactionApply(state)).rejects.toThrow("finite typed primitives");
    const afterRejection = (await adapter.wheelBatch([]))!;
    expect(afterRejection.project).toBe(updated.project);
    expect(result.snapshot.project.title).toBe("Exported");
    expect(original.project.title).toBe("Bridge");
  });

  it("requires explicit native interaction support without replacing the existing snapshot", async () => {
    const adapter = new WasmWorkbenchAdapter(FakeHandle);
    await adapter.construct({ version: 2 });
    await expect(adapter.interactionSnapshot()).rejects.toThrow("current WASM build");
    await expect(adapter.interactionApply({})).rejects.toThrow("current WASM build");
    expect((await adapter.snapshot()).project.title).toBe("Bridge");
  });

  it("keeps the wasm bridge instance-scoped and forwards versioned JSON", async () => {
    FakeHandle.requests = [];
    FakeHandle.catalogReads = 0;
    const adapter = new WasmWorkbenchAdapter(FakeHandle);
    expect((await adapter.construct({ version: 2 })).project.title).toBe("Bridge");
    expect((await adapter.snapshot()).presentation.canFinish).toBe(false);
    expect((await adapter.toolCatalog()).sections).toHaveLength(4);
    expect((await adapter.toolCatalog()).sections[0]?.commands[0]?.icon.key).toBe("geometry-segment");
    expect((await adapter.managedCompilerContext()).patches).toEqual({});
    expect(FakeHandle.catalogReads).toBe(1);
    expect((await adapter.dispatch({ version: 2, command: "project.new" })).revision).toBe(1);
    expect(await adapter.resize({ version: 2, width: 1024, height: 720, pixelRatio: 1 })).toBeNull();
    expect(FakeHandle.requests.map((request) => JSON.parse(request))).toEqual([{ version: 2 }, { version: 2, command: "project.new" }]);
  });
  it("recovers legacy persistence from unchanged raw bytes and replaces only a validated handle", async () => {
    const freed: string[] = [];
    const recovered: string[] = [];
    class RecoveringHandle extends FakeHandle {
      constructor(private readonly name: string) {
        super(name);
        if (name.includes("persistedProject")) throw new Error("ordinary request too large");
      }
      override snapshot() {
        return this.name === "invalid candidate" ? "{}" : super.snapshot();
      }
      free() { freed.push(this.name); }
      static restoreLegacyCodeWorkbench(raw: string) {
        recovered.push(raw);
        if (raw === "bad legacy") throw new Error("legacy validation rejected");
        return new RecoveringHandle(raw === "bad snapshot" ? "invalid candidate" : "recovered");
      }
    }
    const adapter = new WasmWorkbenchAdapter(RecoveringHandle);
    await adapter.construct({ version: 2 });
    await expect(adapter.construct({ version: 2, persistedProject: "bad legacy" })).rejects.toThrow("legacy validation rejected");
    expect(freed).toEqual([]);
    expect((await adapter.snapshot()).project.title).toBe("Bridge");
    await expect(adapter.construct({ version: 2, persistedProject: "bad snapshot" })).rejects.toThrow();
    expect(freed).toEqual(["invalid candidate"]);
    expect((await adapter.snapshot()).project.title).toBe("Bridge");
    // Duplicate keys are intentionally retained for Rust to reject; JS must not
    // normalize the incoming string before forwarding it.
    const original = '{"format":"legacy","format":"duplicate"}';
    await adapter.construct({ version: 2, persistedProject: original });
    expect(recovered).toEqual(["bad legacy", "bad snapshot", original]);
    expect(freed).toEqual(["invalid candidate", '{"version":2}']);
  });

  it("accepts finite canvas-only updates against the current settled revision and retains UI identities", async () => {
    let response = "null";
    class FrameHandle extends FakeHandle {
      override wheel() { return response; }
    }
    const adapter = new WasmWorkbenchAdapter(FrameHandle);
    const base = await adapter.construct({ version: 2 });
    const frame = { ...snapshot.frame, scene: { ...snapshot.frame.scene, viewBox: [0, 0, 900, 800] } };
    response = JSON.stringify({ version: 2, kind: "frame", revision: 0, frame });
    const next = (await adapter.wheel({ version: 2, x: 100, y: 100, deltaX: 0, deltaY: 1, ctrl: false }))!;
    expect(isCanvasOnlySnapshot(next)).toBe(true);
    expect(getCanvasSnapshotSequence(next)).toBeGreaterThan(getCanvasSnapshotSequence(base)!);
    expect(next.frame).toEqual(frame);
    for (const key of ["source", "explorer", "parameters", "presentation", "project", "problems"] as const) expect(next[key]).toBe(base[key]);
    expect(Object.isFrozen(next.frame.scene.items)).toBe(true);
    response = JSON.stringify({ version: 2, kind: "frame", revision: 1, frame });
    await expect(adapter.wheelBatch([])).rejects.toThrow("matching settled");
    response = JSON.stringify({ version: 2, kind: "frame", revision: 0, frame, explorer: [] });
    await expect(adapter.wheelBatch([])).rejects.toThrow("matching settled");
    response = JSON.stringify({ version: 2, kind: "frame", revision: 0, frame: { ...frame, scene: { ...frame.scene, viewBox: [0, 0, null, 800] } } });
    await expect(adapter.wheelBatch([])).rejects.toThrow("finite typed primitives");
    const full = await adapter.dispatch({ version: 2, command: "project.new" });
    expect(isCanvasOnlySnapshot(full)).toBe(false);
    expect(getCanvasSnapshotSequence(full)).toBeGreaterThan(getCanvasSnapshotSequence(next)!);
    response = JSON.stringify({ version: 2, kind: "frame", revision: 1, frame });
    expect((await adapter.wheelBatch([]))?.source).toBe(full.source);
  });

  it("forwards every ordered wheel anchor and clamp input in a single Rust batch", async () => {
    let request = "";
    class BatchHandle extends FakeHandle {
      override wheel(input?: string) { request = input ?? ""; return "null"; }
    }
    const adapter = new WasmWorkbenchAdapter(BatchHandle);
    await adapter.construct({ version: 2 });
    const samples = [
      { version: 2 as const, x: 20, y: 30, deltaX: 0, deltaY: -5000, ctrl: false },
      { version: 2 as const, x: 40, y: 50, deltaX: 0, deltaY: 60, ctrl: true },
    ];
    await adapter.wheelBatch(samples);
    expect(JSON.parse(request)).toEqual({ version: 2, samples });
  });

  it("retains the current snapshot when native defers late dimension cleanup", async () => {
    class CleanupHandle extends FakeHandle { override dispatch() { return "null"; } }
    const adapter = new WasmWorkbenchAdapter(CleanupHandle);
    const before = await adapter.construct({ version: 2 });
    for (const command of ["dimensions.navigation.end", "dimensions.hover.clear"]) {
      const after = await adapter.dispatch({ version: 2, command });
      expect(after).toEqual(before);
      expect(after.frame).toBe(before.frame);
      expect(after.pendingManagedMutation).toBe(before.pendingManagedMutation);
      expect(isCanvasOnlySnapshot(after)).toBe(true);
    }
    await expect(adapter.dispatch({ version: 2, command: "dimensions.edit" })).rejects.toThrow("no snapshot");
  });

  it("publishes selection deltas while retaining source and durable presentation references", async () => {
    const fixture = await new MockWorkbenchAdapter().snapshot();
    fixture.navigation = { authority: "accepted-source-scene", selectionKey: "old-selection", rows: [], sources: [], itemCount: 0, canNavigateSource: true };
    let response = "null";
    class SelectionHandle extends FakeHandle {
      override snapshot() { return JSON.stringify(fixture); }
      override dispatch() { return response; }
      override pointer() { return response; }
      override wheel() { return response; }
    }
    const adapter = new WasmWorkbenchAdapter(SelectionHandle);
    const base = await adapter.construct({ version: 2 });
    response = JSON.stringify({ version: 2, kind: "frame", revision: fixture.revision, frame: fixture.frame });
    const canvasOnly = (await adapter.wheelBatch([]))!;
    expect(isCanvasOnlySnapshot(canvasOnly)).toBe(true);
    const delta = {
      version: 2, kind: "selection", revision: fixture.revision,
      frame: { ...fixture.frame, scene: { ...fixture.frame.scene, viewBox: [0, 0, 900, 800] } },
      navigation: { ...fixture.navigation, selectionKey: "line", rows: [{ id: "line-1", state: "selected" }], sources: [{ path: "sketch.ts", from: 10, to: 20 }], itemCount: 3 },
      selectedDeclarations: ["line-1"],
      selection: { id: "line-1", label: "Line 1", kind: "Geometry", source: { path: "sketch.ts", from: 10, to: 20 }, metadata: { authority: "accepted-properties", target: { kind: "declaration", id: "line-1" }, label: "Line 1", editable: true } },
      selectedGeometryRole: "profile",
    };
    response = JSON.stringify(delta);
    const next = await adapter.dispatch({ version: 2, command: "navigation.rows.select" });
    expect(isCanvasOnlySnapshot(next)).toBe(false);
    expect(next.selection?.metadata).toEqual(delta.selection.metadata);
    expect(getCanvasSnapshotSequence(next)).toBe(getCanvasSnapshotSequence(canvasOnly)! + 1);
    for (const key of ["source", "parameters", "project", "problems"] as const) expect(next[key]).toBe(base[key]);
    expect(next.presentation).toEqual({ ...base.presentation, selectedGeometryRole: "profile" });
    expect(next.explorer[0].children.find((row) => row.id === "line-1")?.selected).toBe(true);
    expect(next.explorer[0].children.find((row) => row.id === "fillet-1")?.selected).toBe(false);
    expect(base.explorer[0].children.find((row) => row.id === "fillet-1")?.selected).toBe(true);
    expect(next.explorer[0].children.find((row) => row.id === "origin")).toBe(base.explorer[0].children.find((row) => row.id === "origin"));
    expect(next.explorer[0].children[1].source).toBe(base.explorer[0].children[1].source);
    expect(Object.isFrozen(next.frame.scene.items)).toBe(true);
    response = JSON.stringify({ ...delta, navigation: { ...delta.navigation, selectionKey: "empty", rows: [], sources: [], itemCount: 0 }, selectedDeclarations: [], selection: null, selectedGeometryRole: null });
    const empty = (await adapter.pointer({ version: 2, phase: "down", pointerId: 1, x: 10, y: 10, buttons: 1, modifiers: { alt: false, ctrl: false, meta: false, shift: false } }))!;
    expect(empty.source).toBe(base.source);
    expect(empty.selection).toBeUndefined();
    expect(empty.presentation.selectedGeometryRole).toBeUndefined();
    expect(empty.explorer[0].children.every((row) => !row.selected)).toBe(true);
    expect(getCanvasSnapshotSequence(empty)).toBe(getCanvasSnapshotSequence(next)! + 1);
    expect(isCanvasOnlySnapshot(empty)).toBe(false);
    response = JSON.stringify({ version: 2, kind: "frame", revision: fixture.revision, frame: fixture.frame });
    const panned = (await adapter.wheelBatch([]))!;
    expect(isCanvasOnlySnapshot(panned)).toBe(true);
    expect(getCanvasSnapshotSequence(panned)).toBe(getCanvasSnapshotSequence(empty)! + 1);
    expect(panned.navigation).toBe(empty.navigation);
    expect(panned.explorer).toBe(empty.explorer);
    expect(panned.presentation).toBe(empty.presentation);
  });

  it("rejects stale and malformed selection deltas without replacing the accepted base", async () => {
    const fixture = await new MockWorkbenchAdapter().snapshot();
    fixture.navigation = { authority: "accepted-source-scene", selectionKey: "base", rows: [], sources: [], itemCount: 0, canNavigateSource: true };
    let response = "null";
    class SelectionHandle extends FakeHandle {
      override snapshot() { return JSON.stringify(fixture); }
      override dispatch() { return response; }
    }
    const adapter = new WasmWorkbenchAdapter(SelectionHandle);
    const base = await adapter.construct({ version: 2 });
    const delta = { version: 2, kind: "selection", revision: fixture.revision, frame: fixture.frame, navigation: fixture.navigation, selectedDeclarations: [], selection: null, selectedGeometryRole: null };
    const invalid = [
      { revision: fixture.revision + 1 },
      { revision: fixture.revision - 1 },
      { revision: fixture.revision + 0.5 },
      { version: 1 },
      { navigation: { ...fixture.navigation, authority: "old-source" } },
      { navigation: { ...fixture.navigation, canNavigateSource: false } },
      { navigation: { ...fixture.navigation, unavailableReason: "different source authority" } },
      { source: { files: [] } },
      { selectedDeclarations: [42] },
      { selectedDeclarations: ["line-1", "line-1"] },
      { selectedDeclarations: ["group:sketch"] },
      { selectedGeometryRole: "invalid" },
      { navigation: { ...fixture.navigation, rows: [{ id: "line-1", state: "mixed" }] } },
      { navigation: { ...fixture.navigation, rows: [{ id: "absent-row", state: "selected" }] } },
      { navigation: { ...fixture.navigation, rows: [{ id: "line-1", state: "selected" }, { id: "line-1", state: "partial" }] } },
      { frame: { ...fixture.frame, scene: { ...fixture.frame.scene, viewBox: [0, 0, null, 800] } } },
      { frame: { scene: fixture.frame.scene } },
      { selectedDeclarations: ["absent-owner"] },
      { selection: {} },
      { selection: "line-1" },
      { selection: [] },
      { selection: { id: 1, label: "Line 1", kind: "Geometry" } },
      { selection: { id: "line-1", kind: "Geometry" } },
      { selection: { id: "line-1", label: "Line 1" } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", ownership: 1 } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", metadata: { authority: "source", target: { kind: "declaration", id: "line-1" }, editable: "yes" } } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", source: null } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", source: { path: "sketch.ts", from: 9, to: 2 } } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", source: { path: "sketch.ts", from: -1, to: 2 } } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", source: { path: "sketch.ts", from: 1.5, to: 2 } } },
      { selection: { id: "line-1", label: "Line 1", kind: "Geometry", source: { path: "sketch.ts", from: 1, to: Number.MAX_SAFE_INTEGER + 1 } } },
    ];
    let sequence = getCanvasSnapshotSequence(base)!;
    for (const changes of invalid) {
      response = JSON.stringify({ ...delta, ...changes });
      await expect(adapter.dispatch({ version: 2, command: "navigation.rows.select" }), JSON.stringify(changes)).rejects.toThrow();
      response = JSON.stringify(delta);
      const current = await adapter.dispatch({ version: 2, command: "navigation.rows.select" });
      expect(current.source).toBe(base.source);
      expect(current.revision).toBe(base.revision);
      expect(current.navigation?.authority).toBe(base.navigation?.authority);
      expect(getCanvasSnapshotSequence(current)).toBe(++sequence);
    }
    for (const key of Object.keys(delta)) {
      // Null is the explicit transport spelling for an absent Inspector or
      // geometry role. Missing fields must not silently clear accepted state.
      const missing: Record<string, unknown> = { ...delta };
      delete missing[key];
      response = JSON.stringify(missing);
      await expect(adapter.dispatch({ version: 2, command: "navigation.rows.select" }), `missing ${key}`).rejects.toThrow();
      response = JSON.stringify(delta);
      const current = await adapter.dispatch({ version: 2, command: "navigation.rows.select" });
      expect(getCanvasSnapshotSequence(current)).toBe(++sequence);
      for (const retained of ["source", "project", "parameters", "problems"] as const) expect(current[retained]).toBe(base[retained]);
    }
  });

  it("refreshes contextual dimension metadata with selection and rejects missing or malformed replacements", async () => {
    const fixture = await new MockWorkbenchAdapter().snapshot();
    fixture.navigation = { authority: "accepted-source-scene", selectionKey: "base", rows: [], sources: [], itemCount: 0, canNavigateSource: true };
    fixture.dimensions = { mode: "focused", entries: [], parameters: [], pinCount: 0 };
    let response = "null";
    class DimensionHandle extends FakeHandle {
      override snapshot() { return JSON.stringify(fixture); }
      override dispatch() { return response; }
    }
    const adapter = new WasmWorkbenchAdapter(DimensionHandle);
    const base = await adapter.construct({ version: 2 });
    const dimensions: NonNullable<WorkbenchSnapshot["dimensions"]> = { ...fixture.dimensions, entries: [{ id: "dimension:1", label: "Width", value: "12", unit: "mm", kind: "Distance", reference: false, generated: false, pinned: false, focused: false, visible: true, editable: true }] };
    const delta = { version: 2, kind: "selection", revision: fixture.revision, frame: fixture.frame, navigation: fixture.navigation, selectedDeclarations: [], selection: null, selectedGeometryRole: null, dimensions };
    response = JSON.stringify(delta);
    const next = await adapter.dispatch({ version: 2, command: "navigation.rows.select" });
    expect(next.dimensions).toEqual(dimensions);
    expect(next.source).toBe(base.source);
    for (const replacement of [undefined, null, { ...dimensions, pinCount: 5 }, { ...dimensions, entries: [{ ...dimensions.entries[0], visible: 1 }] }]) {
      response = JSON.stringify({ ...delta, dimensions: replacement });
      await expect(adapter.dispatch({ version: 2, command: "navigation.rows.select" })).rejects.toThrow();
    }
    response = JSON.stringify({ version: 2, kind: "frame", revision: fixture.revision, frame: fixture.frame });
    const hovered = await adapter.dispatch({ version: 2, command: "dimensions.hover", payload: { x: 20, y: 30 } });
    expect(isCanvasOnlySnapshot(hovered)).toBe(true);
    expect(hovered.dimensions).toBe(next.dimensions);
    expect(hovered.source).toBe(base.source);
    const settledDimensions = { ...dimensions, entries: dimensions.entries.map((entry) => ({ ...entry, visible: false })) };
    const settledDelta = { version: 2, kind: "frame", revision: fixture.revision, frame: fixture.frame, dimensions: settledDimensions };
    response = JSON.stringify(settledDelta);
    const settled = await adapter.dispatch({ version: 2, command: "dimensions.navigation.end" });
    expect(isCanvasOnlySnapshot(settled)).toBe(false);
    expect(settled.dimensions).toEqual(settledDimensions);
    expect(settled.source).toBe(base.source);
    expect(settled.revision).toBe(base.revision);
    const unchanged = await adapter.dispatch({ version: 2, command: "dimensions.navigation.end" });
    expect(isCanvasOnlySnapshot(unchanged)).toBe(true);
    for (const replacement of [null, { ...settledDimensions, pinCount: 5 }, { ...settledDimensions, entries: [{ ...dimensions.entries[0], visible: 1 }] }]) {
      response = JSON.stringify({ ...settledDelta, dimensions: replacement });
      await expect(adapter.dispatch({ version: 2, command: "dimensions.navigation.end" })).rejects.toThrow();
    }
  });

  it("stamps synchronous selection and camera responses in decode order before promises settle", async () => {
    const fixture = await new MockWorkbenchAdapter().snapshot();
    fixture.navigation = { authority: "accepted-order", selectionKey: "start", rows: [], sources: [], itemCount: 0, canNavigateSource: true };
    let response = "null";
    class OrderedHandle extends FakeHandle {
      override snapshot() { return JSON.stringify(fixture); }
      override dispatch() { return response; }
      override wheel() { return response; }
    }
    const adapter = new WasmWorkbenchAdapter(OrderedHandle);
    const base = await adapter.construct({ version: 2 });
    response = JSON.stringify({ version: 2, kind: "selection", revision: fixture.revision, frame: fixture.frame,
      navigation: { ...fixture.navigation, selectionKey: "group", rows: [{ id: "group:sketch", state: "partial" }, { id: "line-1", state: "selected" }], itemCount: 1 },
      selectedDeclarations: ["line-1"], selection: null, selectedGeometryRole: "mixed" });
    const selecting = adapter.dispatch({ version: 2, command: "navigation.rows.select" });
    response = JSON.stringify({ version: 2, kind: "frame", revision: fixture.revision, frame: fixture.frame });
    const panning = adapter.wheelBatch([]);
    const [selected, panned] = await Promise.all([selecting, panning]);
    expect(getCanvasSnapshotSequence(selected)).toBe(getCanvasSnapshotSequence(base)! + 1);
    expect(getCanvasSnapshotSequence(panned!)).toBe(getCanvasSnapshotSequence(selected)! + 1);
    expect(isCanvasOnlySnapshot(selected)).toBe(false);
    expect(isCanvasOnlySnapshot(panned!)).toBe(true);
    expect(panned!.navigation).toBe(selected.navigation);
    expect(panned!.explorer).toBe(selected.explorer);
    expect(panned!.presentation.selectedGeometryRole).toBe("mixed");
    expect(selected.parameters).toBe(base.parameters);
  });

});
