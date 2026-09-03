// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { WasmWorkbenchAdapter, type JsonWorkbenchHandle } from "./wasm-adapter";
import type { WorkbenchSnapshot } from "./adapter";

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
const snapshot: WorkbenchSnapshot = { version: 1, revision: 0, project: { title: "Bridge", status: "accepted" }, presentation: { activeTool: "select", gridVisible: true, canUndo: false, canRedo: false, canFinish: false, geometryRole: "profile" }, frame: { svg: "<svg/>", ariaLabel: "frame" }, source: { selectedPath: "", files: [], dirty: false }, explorer: [], parameters: [], problems: [] };

class FakeHandle implements JsonWorkbenchHandle {
  static requests: string[] = [];
  static catalogReads = 0;
  constructor(request: string) { FakeHandle.requests.push(request); }
  snapshot() { return JSON.stringify(snapshot); }
  toolCatalog() { FakeHandle.catalogReads += 1; return JSON.stringify(catalog); }
  managedCompilerContext() { return JSON.stringify({ version: 1, patches: {} }); }
  dispatch(request: string) { FakeHandle.requests.push(request); return JSON.stringify({ ...snapshot, revision: 1 }); }
  pointer() { return "null"; }
  wheel() { return "null"; }
  resize() { return "null"; }
  cancel() { return JSON.stringify(snapshot); }
  exportProject() { return JSON.stringify({ version: 1, filename: "project.json", contents: "{}" }); }
  persistProject() { return JSON.stringify({ version: 1, contents: "{}" }); }
  exportReproduction() { return JSON.stringify({ version: 1, filename: "repro.txt", contents: "repro" }); }
  exportInteractionTrace() { return JSON.stringify({ version: 1, filename: "trace.txt", contents: "trace" }); }
  intentRpc(request: string) { return request; }
  codeControlRpc(request: string) { return request; }
}

describe("WasmWorkbenchAdapter", () => {
  it("keeps the wasm bridge instance-scoped and forwards versioned JSON", async () => {
    FakeHandle.requests = [];
    FakeHandle.catalogReads = 0;
    const adapter = new WasmWorkbenchAdapter(FakeHandle);
    expect((await adapter.construct({ version: 1 })).project.title).toBe("Bridge");
    expect((await adapter.snapshot()).presentation.canFinish).toBe(false);
    expect((await adapter.toolCatalog()).sections).toHaveLength(4);
    expect((await adapter.toolCatalog()).sections[0]?.commands[0]?.icon.key).toBe("geometry-segment");
    expect((await adapter.managedCompilerContext()).patches).toEqual({});
    expect(FakeHandle.catalogReads).toBe(1);
    expect((await adapter.dispatch({ version: 1, command: "project.new" })).revision).toBe(1);
    expect(await adapter.resize({ version: 1, width: 1024, height: 720, pixelRatio: 1 })).toBeNull();
    expect(FakeHandle.requests.map((request) => JSON.parse(request))).toEqual([{ version: 1 }, { version: 1, command: "project.new" }]);
  });
});
