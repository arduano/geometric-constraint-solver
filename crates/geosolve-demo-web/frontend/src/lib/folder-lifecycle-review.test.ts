// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { FolderWorkbenchAdapter } from "./folder-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";

async function harness() {
  const fixture = await new MockWorkbenchAdapter().snapshot();
  let revision = 1;
  let disk = "disk-before";
  let resolveNext: (() => void) | undefined;
  const requests: Array<{ method: string; input?: { command: string }; baseHash: string; authority: { revision: number } }> = [];
  vi.stubGlobal("fetch", vi.fn(async (_url, init) => {
    const request = JSON.parse(init.body); requests.push(request);
    if (request.input?.command === "dimensions.focus") revision++;
    if (request.input?.command === "dimensions.edit" && resolveNext) await new Promise<void>((resolve) => { resolveNext = resolve; });
    return { ok: true, json: async () => ({ result: structuredClone(fixture), state: {
      authority: { epoch: "epoch", lease: 1, revision }, editor: { clientId: "client", canEdit: true },
      ok: true, sequence: revision, currentHash: disk, acceptedHash: disk, status: "saved",
      diagnostics: [], paths: { folder: "/project", source: "/project/sketch.ts" },
    } }) };
  }));
  vi.stubGlobal("EventSource", class { close() {} });
  const adapter = new FolderWorkbenchAdapter("test-token");
  adapter.installSnapshot(await adapter.construct());
  return { adapter, requests, diskChanged: () => { disk = "disk-after"; },
    delayNextEdit: () => { resolveNext = () => {}; }, resolveEdit: () => resolveNext?.() };
}

afterEach(() => { vi.unstubAllGlobals(); sessionStorage.clear(); });

describe("folder lifecycle review", () => {
  it("refresh cannot silently rebase a surviving unchanged-value field draft", async () => {
    const { adapter, requests, diskChanged } = await harness();
    const unsubscribe = adapter.subscribe((snapshot) => { if (snapshot) adapter.installSnapshot(snapshot); });
    adapter.changeFieldEdit("width", "Width", "14");
    diskChanged();
    await adapter.refresh(true);
    let submitted!: Promise<unknown>;
    adapter.commitFieldEdit("width", () => { submitted = adapter.dispatch({ version: 2, command: "dimensions.edit", payload: { value: "14" } }); });
    await submitted;
    expect(requests.at(-1)?.baseHash).toBe("disk-before");
    unsubscribe();
  });

  it("focus response records a new revision and an already started draft stays stale", async () => {
    const { adapter, requests } = await harness();
    const focus = adapter.dispatch({ version: 2, command: "dimensions.focus" });
    adapter.changeFieldEdit("width", "Width", "14");
    adapter.installSnapshot(await focus);
    let submitted!: Promise<unknown>;
    adapter.commitFieldEdit("width", () => { submitted = adapter.dispatch({ version: 2, command: "dimensions.edit" }); });
    await submitted;
    expect(adapter.state?.authority?.revision).toBe(2);
    expect(requests.at(-1)?.authority.revision).toBe(1);
  });
});
