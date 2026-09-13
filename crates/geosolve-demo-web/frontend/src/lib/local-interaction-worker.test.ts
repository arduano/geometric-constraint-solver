// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { createLocalInteractionHandler, type InteractionHandle, type LocalInteractionRequest, type LocalInteractionResponse } from "./local-interaction-worker";
import { LocalInteractionWorker } from "./local-interaction-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";

class FakeWorker extends EventTarget {
  postMessage = vi.fn();
  terminate = vi.fn();
  reply(response: LocalInteractionResponse) { this.dispatchEvent(new MessageEvent("message", { data: response })); }
}

describe("detached interaction worker transport", () => {
  it("keeps construction and pointer commands in order and offers no editing or solving method", async () => {
    const calls: string[] = [];
    const reply = vi.fn();
    let release!: (value: new (seed: string) => InteractionHandle) => void;
    const ready = new Promise<new (seed: string) => InteractionHandle>((resolve) => { release = resolve; });
    class Handle implements InteractionHandle {
      constructor(seed: string) { calls.push(`construct:${seed}`); }
      replace(value: string) { calls.push(`replace:${value}`); return '{"frame":{},"state":{},"selectionChanged":false}'; }
      pointer(value: string) { calls.push(`pointer:${value}`); return '{"frame":{},"state":{},"selectionChanged":true}'; }
      dispatch = this.pointer; wheel = this.pointer; resize = this.pointer; cancel = this.pointer;
      authoringPointer() { return '{}'; }
      restoreSelection = this.pointer;
      presence = this.pointer;
      state() { return '{"opaque":"state"}'; }
      free() { calls.push("free"); }
    }
    const handle = createLocalInteractionHandler(ready, reply);
    const send = (request: LocalInteractionRequest) => handle(new MessageEvent("message", { data: request }));
    send({ id: 1, method: "construct", input: { sceneKey: "scene" } });
    send({ id: 2, method: "pointer", input: { x: 12 } });
    expect(calls).toEqual([]);
    release(Handle);
    await vi.waitFor(() => expect(reply).toHaveBeenCalledTimes(2));
    expect(calls).toEqual(['construct:{"sceneKey":"scene"}', 'replace:{"seed":{"sceneKey":"scene"},"preserveSelection":false}', 'pointer:{"x":12}']);
    send({ id: 3, method: "edit" as LocalInteractionRequest["method"] });
    await vi.waitFor(() => expect(reply).toHaveBeenLastCalledWith({ id: 3, error: "Unsupported local interaction operation" }));
  });

  it("exports personal presentation through the native codec without exposing mutation", async () => {
    const presentation = { hiddenRows: ["managed:edge"], constructionVisible: false, dimensions: { mode: "all" as const, pins: ["persistent-dimension-key"] } };
    const reply = vi.fn();
    class Handle implements InteractionHandle {
      replace() { return "{}"; } dispatch = this.replace; pointer = this.replace; wheel = this.replace;
      resize = this.replace; cancel = this.replace; authoringPointer = this.replace; restoreSelection = this.replace; presence = this.replace;
      state() { return "{}"; } exportPresentation() { return JSON.stringify(presentation); } free() {}
    }
    const handle = createLocalInteractionHandler(Promise.resolve(Handle), reply);
    handle(new MessageEvent("message", { data: { id: 1, method: "construct", input: {} } }));
    handle(new MessageEvent("message", { data: { id: 2, method: "exportPresentation" } }));
    await vi.waitFor(() => expect(reply).toHaveBeenLastCalledWith({ id: 2, result: presentation }));
    const worker = new FakeWorker(), adapter = new LocalInteractionWorker(worker as unknown as Worker);
    const saved = adapter.exportPresentation();
    expect(worker.postMessage).toHaveBeenCalledWith({ id: 1, method: "exportPresentation", input: undefined });
    worker.reply({ id: 1, result: presentation });
    expect(await saved).toEqual(presentation); adapter.dispose();
  });

  it("freezes locally composed frames and rejects pending requests when its worker fails", async () => {
    const worker = new FakeWorker(), adapter = new LocalInteractionWorker(worker as unknown as Worker);
    const frame = (await new MockWorkbenchAdapter().snapshot()).frame;
    const constructed = adapter.construct({ opaque: "seed" });
    worker.reply({ id: 1, result: { frame, state: { opaque: "state" }, selectionChanged: false, serverFrameCompatible: false } });
    expect(Object.isFrozen((await constructed).frame.scene.items)).toBe(true);
    const first = expect(adapter.update("wheel", { opaque: "wheel" })).rejects.toThrow("Local interaction response could not be read");
    const second = expect(adapter.state()).rejects.toThrow("Local interaction response could not be read");
    worker.dispatchEvent(new Event("messageerror"));
    await Promise.all([first, second]);
    expect(worker.terminate).toHaveBeenCalledOnce();
    await expect(adapter.state()).rejects.toThrow("Local interaction response could not be read");
  });
});
