// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { FolderWorkbenchAdapter, type FolderState } from "./folder-adapter";
import { assertWorkbenchSnapshot, type WorkbenchSnapshot } from "./adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";

async function harness(mode: "editable" | "generator" = "generator") {
  const fixture = await new MockWorkbenchAdapter().snapshot();
  fixture.parameters[0].metadata = { authority: "metadata", target: { kind: "parameter", id: "width" }, editable: true, canExtract: true };
  const state: FolderState = {
    mode, entry: "src/generator.ts", authority: { epoch: "epoch", lease: 1, revision: 1 },
    editor: { clientId: "tab", canEdit: true }, ok: true, sequence: 1, currentHash: "accepted-before", acceptedHash: "accepted-before", status: "saved",
    diagnostics: [], paths: { folder: "/project", source: "/project/src/generator.ts" },
    capabilities: { sourceEditing: mode === "editable", geometryEditing: mode === "editable", generatorInputs: mode === "generator" },
    inputDefinitions: { width: { type: "number", default: 12, label: "Channel width", unit: "mm", min: 8 } }, inputs: { width: 12 },
    sourceFiles: [ { path: "src/generator.ts", language: "typescript", contents: "export default generate;", readOnly: true },
      { path: "src/profile.ts", language: "typescript", contents: "export const width = 12;", readOnly: true } ],
  };
  const requests: Array<{ method: string; input?: { values?: Record<string, unknown>; command?: string }; baseHash: string; authority: FolderState["authority"] }> = [];
  let rejectInputs = false;
  vi.stubGlobal("fetch", vi.fn(async (_url, init) => {
    const request = JSON.parse(init.body); requests.push(request);
    if (request.method === "inputs.set") {
      if (rejectInputs) return { ok: false, status: 409, json: async () => ({ error: "Files changed since the input was prepared", state: structuredClone(state) }) };
      state.inputs = request.input.values; state.currentHash = state.acceptedHash = "accepted-input";
      state.authority!.revision++;
    }
    return { ok: true, json: async () => ({ result: structuredClone(fixture), state: structuredClone(state) }) };
  }));
  vi.stubGlobal("EventSource", class { close() {} });
  const adapter = new FolderWorkbenchAdapter("token");
  const snapshot = await adapter.construct(); adapter.installSnapshot(snapshot);
  return { adapter, snapshot, fixture, state, requests, rejectInputs: () => { rejectInputs = true; } };
}

afterEach(() => { vi.unstubAllGlobals(); sessionStorage.clear(); });

describe("generator workbench transport", () => {
  it("shows captured TypeScript read-only while preserving all accepted geometry", async () => {
    const { adapter, snapshot, fixture, requests } = await harness();
    expect(assertWorkbenchSnapshot(snapshot)).toBe(snapshot);
    expect(snapshot.source.files.map((file) => file.path)).toEqual(["src/generator.ts", "src/profile.ts"]);
    expect(snapshot.source.files.every((file) => file.readOnly)).toBe(true);
    expect(snapshot.source.selectedPath).toBe("src/generator.ts");
    expect(snapshot.frame).toEqual(fixture.frame);
    expect(snapshot.parameters.every((parameter) => !parameter.editable)).toBe(true);
    expect(snapshot.parameters[0].metadata).toMatchObject({ editable: false, canExtract: false });
    expect(snapshot.presentation.canUndo).toBe(false);
    expect(snapshot.explorer.every((row) => !row.capabilities.edit.enabled && !row.capabilities.delete.enabled)).toBe(true);
    const count = requests.length;
    const selected = await adapter.dispatch({ version: 2, command: "source.select", payload: { path: "src/profile.ts" } });
    expect(adapter.installSnapshot(selected)).toBe(true);
    expect(selected.source.selectedPath).toBe("src/profile.ts");
    expect(requests).toHaveLength(count);
    expect(() => adapter.installSnapshot(structuredClone(selected))).toThrow(/transport authority/);
    await expect(adapter.dispatch({ version: 2, command: "source.select", payload: { path: "absent.ts" } })).rejects.toThrow(/accepted generator/);
  });

  it("retains installed inputs and original draft basis across observation and takeover", async () => {
    const { adapter, state, requests, rejectInputs } = await harness();
    adapter.changeFieldEdit("width", "Channel width", "14");
    state.currentHash = "external-edit"; state.authority!.lease = 2; state.inputs = { width: 20 };
    const changed = await adapter.snapshot();
    expect(adapter.state?.inputs).toEqual({ width: 20 });
    expect(adapter.installedState?.inputs).toEqual({ width: 12 });
    expect(adapter.installSnapshot(changed)).toBe(false);
    const delivered = vi.fn((snapshot?: WorkbenchSnapshot) => { if (snapshot) adapter.installSnapshot(snapshot); });
    adapter.subscribe(delivered);
    await adapter.takeOver();
    expect(adapter.installedState?.inputs).toEqual({ width: 12 });
    rejectInputs();
    let submission!: Promise<WorkbenchSnapshot>;
    adapter.commitFieldEdit("width", () => { submission = adapter.setGeneratorInputs({ width: 14 }); });
    await expect(submission).rejects.toThrow(/Files changed/);
    expect(requests.at(-1)).toMatchObject({ method: "inputs.set", baseHash: "accepted-before", authority: { lease: 1 }, input: { values: { width: 14 } } });
    expect(adapter.pending).toContain("14");
    expect(adapter.installedState?.inputs).toEqual({ width: 12 });
  });

  it("publishes new inputs only when the authentic returned snapshot is installed", async () => {
    const { adapter } = await harness();
    adapter.changeFieldEdit("width", "Channel width", "14");
    let submission!: Promise<WorkbenchSnapshot>;
    adapter.commitFieldEdit("width", () => { submission = adapter.setGeneratorInputs({ width: 14 }); });
    const accepted = await submission;
    expect(adapter.installedState?.inputs).toEqual({ width: 12 });
    expect(adapter.installSnapshot(accepted)).toBe(true);
    expect(adapter.installedState?.inputs).toEqual({ width: 14 });
  });

  it("suppresses generator dragging and read-only-tab mutation transport", async () => {
    const { adapter, state, requests } = await harness();
    const count = requests.length;
    const pointer = { version: 2 as const, phase: "move" as const, pointerId: 1, x: 10, y: 20, buttons: 1, modifiers: { alt: false, ctrl: false, meta: false, shift: false } };
    expect(await adapter.pointer(pointer)).toBeNull();
    expect(requests).toHaveLength(count);
    expect(await adapter.pointer({ ...pointer, phase: "down" })).not.toBeNull();
    state.editor!.canEdit = false;
    await adapter.snapshot();
    const before = requests.length;
    expect(await adapter.resize({ version: 2, width: 900, height: 700, pixelRatio: 1 })).toBeNull();
    await expect(adapter.setGeneratorInputs({ width: 16 })).rejects.toThrow(/Another tab/);
    expect(requests).toHaveLength(before);
  });

  it("preserves ordinary editable snapshots and rejects generator-only inputs", async () => {
    const { adapter, snapshot, fixture } = await harness("editable");
    expect(snapshot).toEqual(fixture);
    expect(adapter.isGenerator).toBe(false);
    await expect(adapter.setGeneratorInputs({ width: 14 })).rejects.toThrow(/does not expose/);
  });
});
