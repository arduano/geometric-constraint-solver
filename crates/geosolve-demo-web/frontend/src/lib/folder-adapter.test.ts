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
  vi.stubGlobal("EventSource", class extends EventTarget { close() {} });
  const adapter = new FolderWorkbenchAdapter("test-token");
  adapter.installSnapshot(await adapter.construct());
  return { adapter, requests, changeDisk: () => { disk = "radius-15"; } };
}

afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); sessionStorage.clear(); });

it("tracks queued folder work through completion and failed transport retries", async () => {
  const { adapter } = await harness();
  vi.useFakeTimers();
  const successful = vi.mocked(fetch).getMockImplementation()!;
  let release!: () => void;
  vi.stubGlobal("fetch", vi.fn(async (...args: Parameters<typeof fetch>) => {
    await new Promise<void>((resolve) => { release = resolve; });
    return successful(...args);
  }));
  const first = adapter.dispatch({ version: 2, command: "view.fit" });
  const second = adapter.snapshot();
  await vi.advanceTimersByTimeAsync(499);
  expect(adapter.activity.getSnapshot()).toBe(false);
  await vi.advanceTimersByTimeAsync(1);
  expect(adapter.activity.getSnapshot()).toBe(true);
  release(); await first;
  await vi.advanceTimersByTimeAsync(0);
  expect(adapter.activity.getSnapshot()).toBe(true);
  release(); await second;
  expect(adapter.activity.getSnapshot()).toBe(false);

  vi.stubGlobal("fetch", vi.fn(async () => {
    await new Promise<void>((resolve) => { release = resolve; });
    throw Error("offline");
  }));
  const failure = expect(adapter.snapshot()).rejects.toThrow("offline");
  await vi.advanceTimersByTimeAsync(500);
  expect(adapter.activity.getSnapshot()).toBe(true);
  release(); await vi.advanceTimersByTimeAsync(0);
  expect(adapter.activity.getSnapshot()).toBe(true);
  release(); await failure;
  expect(adapter.activity.getSnapshot()).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
});

it("tracks external activity without fetching or installing state and retires it on disconnect/unsubscribe", async () => {
  const { adapter, requests } = await harness();
  vi.useFakeTimers();
  let events!: EventTarget & { onerror?: () => void };
  vi.stubGlobal("EventSource", class extends EventTarget {
    constructor() { super(); events = this; }
    close() {}
  });
  const listener = vi.fn();
  const stop = adapter.subscribe(listener);
  const notify = (data: string) => events.dispatchEvent(new MessageEvent("activity", { data }));
  const baseline = requests.length;
  notify('{"busy":true}');
  await vi.advanceTimersByTimeAsync(300);
  notify('{"busy":true}');
  await vi.advanceTimersByTimeAsync(200);
  expect(adapter.activity.getSnapshot()).toBe(true);
  expect(listener).not.toHaveBeenCalled();
  expect(requests).toHaveLength(baseline);
  notify("invalid");
  expect(adapter.activity.getSnapshot()).toBe(true);
  notify('{"busy":false}');
  expect(adapter.activity.getSnapshot()).toBe(false);
  notify('{"busy":true}');
  await vi.advanceTimersByTimeAsync(500);
  events.onerror?.();
  expect(adapter.activity.getSnapshot()).toBe(false);
  notify('{"busy":true}');
  await vi.advanceTimersByTimeAsync(500);
  stop();
  expect(adapter.activity.getSnapshot()).toBe(false);
  notify('{"busy":true}');
  expect(vi.getTimerCount()).toBe(0);
});

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

it("a lost save response retries the identical operation ID and retains it if disconnected", async () => {
  const { adapter } = await harness();
  const successful = vi.mocked(fetch).getMockImplementation()!;
  const bodies: string[] = [];
  let attempt = 0;
  vi.stubGlobal("fetch", vi.fn(async (...args: Parameters<typeof fetch>) => {
    bodies.push(String(args[1]?.body));
    if (++attempt === 1) throw Error("response lost after server commit");
    return successful(...args);
  }));
  await adapter.dispatch({ version: 2, command: "history.undo" });
  expect(bodies).toHaveLength(2);
  expect(bodies[0]).toBe(bodies[1]);
  expect(JSON.parse(bodies[0]).operationId).toBeTruthy();
  vi.stubGlobal("fetch", vi.fn(async () => { throw Error("disconnected"); }));
  await expect(adapter.dispatch({ version: 2, command: "history.undo" })).rejects.toThrow("disconnected");
  expect(adapter.pendingOperationId).toBeTruthy();
  expect(JSON.parse(adapter.pending).baseHash).toBe("radius-10");
  expect(adapter.notice).toContain("status is unknown");
});

it("disconnected navigation is not replayed and status checks retain the saved intent", async () => {
  const { adapter } = await harness();
  adapter.pending = JSON.stringify({ operationId: "saved-edit", input: "original" });
  const original = adapter.pending;
  const disconnected = vi.fn(async () => { throw Error("disconnected"); });
  vi.stubGlobal("fetch", disconnected);
  await expect(adapter.wheel({ version: 2, x: 100, y: 100, deltaX: 0, deltaY: -20, ctrl: false })).rejects.toThrow("disconnected");
  expect(disconnected).toHaveBeenCalledTimes(1);
  expect(adapter.pending).toBe(original);
  disconnected.mockClear();
  await expect(adapter.checkPendingOperation()).rejects.toThrow("disconnected");
  expect(disconnected).toHaveBeenCalledTimes(2);
  expect(adapter.pending).toBe(original);
});

it("successful status and background observations preserve a disconnected save's operation identity", async () => {
  const { adapter, changeDisk } = await harness();
  const connected = vi.mocked(fetch).getMockImplementation()!;
  adapter.changeFieldEdit("radius", "Radius", "12");
  vi.stubGlobal("fetch", vi.fn(async () => { throw Error("response lost"); }));
  let saving!: Promise<unknown>;
  adapter.commitFieldEdit("radius", () => { saving = adapter.dispatch({ version: 2, command: "dimensions.edit" }); });
  await expect(saving).rejects.toThrow("response lost");
  const request = adapter.pending;
  const operationId = adapter.pendingOperationId;
  expect(operationId).toBeTruthy();
  changeDisk();
  vi.stubGlobal("fetch", vi.fn(async (...args: Parameters<typeof fetch>) => {
    const response = await connected(...args);
    const body = await response.json();
    const request = JSON.parse(String(args[1]?.body));
    return { ...response, json: async () => ({ ...body, result: request.method === "operation.outcome" ? { state: "acknowledged" } : body.result }) };
  }));
  await adapter.checkPendingOperation();
  expect(adapter.notice).toContain("This edit was saved");
  expect(adapter.pendingOperationId).toBe(operationId);
  expect(adapter.pending).toBe(request);
  await adapter.refresh(false);
  expect(adapter.pending).toBe(request);
  expect(sessionStorage.getItem("geosolve.folder.pending")).toBe(request);
});
