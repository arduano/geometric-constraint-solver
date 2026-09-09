// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { FolderWorkbenchAdapter } from "./folder-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";

async function harness() {
  const fixture = await new MockWorkbenchAdapter().snapshot();
  let disk = "radius-10";
  const requests: Array<{ method: string; input?: { command: string }; baseHash: string }> = [];
  vi.stubGlobal("fetch", vi.fn(async (_url, init) => {
    const request = JSON.parse(init.body);
    requests.push(request);
    return { ok: true, json: async () => ({ result: structuredClone(fixture), state: {
      ok: true, sequence: 1, currentHash: disk, acceptedHash: disk, status: "saved",
      diagnostics: [], paths: { folder: "/project", source: "/project/sketch.ts" },
    } }) };
  }));
  vi.stubGlobal("EventSource", class { close() {} });
  const adapter = new FolderWorkbenchAdapter("test-token");
  adapter.installSnapshot(await adapter.construct());
  return { adapter, requests, changeDisk: () => { disk = "radius-15"; } };
}

afterEach(() => { vi.unstubAllGlobals(); sessionStorage.clear(); });

describe("folder installed source authority", () => {
  it("M98-F001: hidden disk refresh cannot authorize Undo against a snapshot the UI never installed", async () => {
    const { adapter, requests, changeDisk } = await harness();
    const delivered = vi.fn();
    const unsubscribe = adapter.subscribe(delivered);
    adapter.changeFieldEdit("radius", "Radius", "12");
    changeDisk();
    await adapter.refresh(false);
    expect(delivered.mock.calls.every(([snapshot]) => snapshot === undefined)).toBe(true);
    await adapter.dispatch({ version: 2, command: "history.undo" });
    expect(requests.at(-1)?.baseHash).toBe("radius-10");
    unsubscribe();
  });

  it("background fetch cannot rebase a pending source draft", async () => {
    const { adapter, requests, changeDisk } = await harness();
    adapter.draftChanged(true);
    changeDisk();
    await adapter.refresh(false);
    await adapter.dispatch({ version: 2, command: "source.prepare", payload: { contents: "pending" } });
    expect(requests.at(-1)?.baseHash).toBe("radius-10");
  });
  it("observing snapshots only advances status; explicit installation advances edit authority", async () => {
    const { adapter, requests, changeDisk } = await harness();
    const old = await adapter.snapshot();
    changeDisk();
    const observed = await adapter.snapshot();
    expect(adapter.state?.currentHash).toBe("radius-15");
    await adapter.dispatch({ version: 2, command: "authoring.parameter.extract" });
    expect(requests.at(-1)?.baseHash).toBe("radius-10");
    expect(adapter.installSnapshot(observed)).toBe(true);
    expect(adapter.installSnapshot(old)).toBe(false);
    await adapter.dispatch({ version: 2, command: "history.redo" });
    expect(requests.at(-1)?.baseHash).toBe("radius-15");
  });

  it("navigation responses cannot replace a pending metadata textarea's source", async () => {
    const { adapter, requests, changeDisk } = await harness();
    adapter.changeFieldEdit("description", "Document description", "Two\nlines");
    changeDisk();
    const newer = (await adapter.wheelBatch([]))!;
    expect(adapter.installSnapshot(newer)).toBe(false);
    let committed!: Promise<unknown>;
    adapter.commitFieldEdit("description", () => {
      committed = adapter.dispatch({ version: 2, command: "authoring.metadata.set", payload: { description: "Two\nlines" } });
    });
    await committed;
    expect(requests.at(-1)?.baseHash).toBe("radius-10");
  });

  it("a field changed again during an outstanding commit retains its original basis", async () => {
    const { adapter, requests, changeDisk } = await harness();
    adapter.changeFieldEdit("radius", "Radius", "12");
    let committed!: Promise<unknown>;
    adapter.commitFieldEdit("radius", () => { committed = adapter.dispatch({ version: 2, command: "dimensions.edit" }); });
    adapter.changeFieldEdit("radius", "Radius", "14");
    await committed;
    changeDisk();
    const newer = await adapter.snapshot();
    expect(adapter.installSnapshot(newer)).toBe(false);
    await adapter.dispatch({ version: 2, command: "history.undo" });
    expect(requests.at(-1)?.baseHash).toBe("radius-10");
    adapter.cancelFieldEdit("radius");
    expect(adapter.installSnapshot(newer)).toBe(true);
  });

});
