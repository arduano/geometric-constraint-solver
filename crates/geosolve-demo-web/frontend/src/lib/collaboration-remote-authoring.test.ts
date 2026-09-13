// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { createRemoteAuthoringClient, type PreviewRpc } from "./collaboration-remote-authoring";
import type { RemotePaintRequest } from "./collaboration-remote-authoring-renderer";
import type { AuthoringModel, AuthoringView } from "./collaboration-authoring-worker";
import type { PointGestureTarget, PointGestureTerminal } from "../../../../../packages/geosolve-engine/src/index";
import { MockWorkbenchAdapter } from "./mock-adapter";

const model: AuthoringModel = { documentEpoch: "document", revision: 1, sourceDesignDigest: "digest", project: "private project bytes", design: { format: "geosolve-design-v1", project: "private design bytes", generated: {}, overrides: {} } };
const basis = { documentEpoch: model.documentEpoch, revision: model.revision, sourceDesignDigest: model.sourceDesignDigest };
const view: AuthoringView = { seed: { opaque: "local scene" }, state: { opaque: "personal selection" } };
const viewport = { screen_size: [1000, 700] as const, model_center: [0, 0] as const, pixels_per_model_unit: 10 };
// Transport fixture only: native target/terminal semantics have separate actual-WASM tests.
const target = { fixture: "target" } as unknown as PointGestureTarget;
const terminal = { fixture: "terminal" } as unknown as PointGestureTerminal;
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes; }); return { promise, resolve }; }
type Reply = Awaited<ReturnType<PreviewRpc>>;
const preview = (ticket = "a".repeat(64), identity = basis): Reply => ({ kind: "preview", basis: identity, ticket, presentation: "native presentation" });

async function fixture(handler?: (body: Record<string, unknown>, signal?: AbortSignal) => Promise<Reply>) {
  const frame = structuredClone((await new MockWorkbenchAdapter().snapshot()).frame);
  frame.scene.provenance.scene = "provisional";
  frame.scene.items.forEach(item => { item.interactive = false; });
  class Renderer extends EventTarget {
    terminate = vi.fn();
    postMessage = vi.fn((request: RemotePaintRequest) => {
      queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data: { id: request.id, generation: request.generation, result: { ...request.result, frame: structuredClone(frame) } } })));
    });
  }
  const renderer = new Renderer();
  const rpc = vi.fn<PreviewRpc>(async (request, signal) => {
    const body = request as Record<string, unknown>;
    if (body.action === "cancel") return { kind: "cancelled", basis };
    if (handler) return handler(body, signal);
    if (body.action === "finish") return { kind: "point", basis, terminal };
    return preview();
  });
  const client = createRemoteAuthoringClient(rpc, renderer as unknown as Worker);
  await client.replace(model);
  return { client, rpc, renderer };
}

describe("server prediction with local presentation", () => {
  it("sends identity and ordered samples while keeping project, scene and navigation local", async () => {
    const f = await fixture();
    try {
      expect(f.rpc).not.toHaveBeenCalled();
      const first = await f.client.beginPoint({ target, gestureId: 7, viewport, view });
      expect(first.presentation).toBe("native presentation");
      expect(f.rpc.mock.calls[0][0]).toEqual({ action: "begin", basis, kind: "point", target, gestureId: 7, viewport });
      const a = f.client.advancePoint({ sequence: 1, position: [1, 2] }, view);
      const b = f.client.advancePoint({ sequence: 2, position: [3, 4] }, view);
      await Promise.all([a, b]);
      expect(f.rpc.mock.calls[1][0]).toEqual({ action: "advance", ticket: "a".repeat(64), samples: [{ sequence: 1, position: [1, 2] }, { sequence: 2, position: [3, 4] }] });
      const camera = { ...view, state: { opaque: "zoomed local camera" } };
      expect((await f.client.render(camera)).view).toEqual(camera);
      expect(f.rpc).toHaveBeenCalledTimes(2);
      expect(f.renderer.postMessage.mock.lastCall?.[0].result.view).toEqual(camera);
      expect((await f.client.finishPoint()).terminal).toEqual(terminal);
      expect(JSON.stringify(f.rpc.mock.calls)).not.toMatch(/private project|private design|personal selection|local scene|zoomed local/);
    } finally { f.client.dispose(); }
  });

  it("aborts replacement work and cancels a late ticket without painting an obsolete model", async () => {
    const hold = deferred<Reply>(), started = deferred<AbortSignal | undefined>();
    const f = await fixture(async (_body, signal) => { started.resolve(signal); return hold.promise; });
    try {
      const begin = f.client.beginPoint({ target, gestureId: 8, viewport, view });
      const rejection = expect(begin).rejects.toThrow("replaced");
      const signal = await started.promise;
      await f.client.replace({ ...model, revision: 2 }); await rejection;
      expect(signal?.aborted).toBe(true);
      hold.resolve(preview());
      await vi.waitFor(() => expect(f.rpc.mock.calls.some(([body]) => (body as { action: string }).action === "cancel")).toBe(true));
      expect(f.renderer.postMessage).not.toHaveBeenCalled();
    } finally { hold.resolve(preview()); f.client.dispose(); }
  });

  it("an obsolete finish cannot retire the newer model's active ticket", async () => {
    const hold = deferred<Reply>(), started = deferred<void>();
    const f = await fixture(async body => {
      if (body.action === "finish") { started.resolve(); return hold.promise; }
      return preview((body.basis as typeof basis).revision === 1 ? "a".repeat(64) : "b".repeat(64), body.basis as typeof basis);
    });
    try {
      await f.client.beginPoint({ target, gestureId: 9, viewport, view });
      const finish = f.client.finishPoint(), rejected = expect(finish).rejects.toThrow("replaced");
      await started.promise;
      await f.client.replace({ ...model, revision: 2 }); await rejected;
      await f.client.beginPoint({ target, gestureId: 10, viewport, view });
      hold.resolve({ kind: "point", basis, terminal });
      await new Promise(resolve => setTimeout(resolve, 0));
      expect((await f.client.render(view)).model.revision).toBe(2);
      await f.client.cancel();
      expect(f.rpc.mock.calls.at(-1)?.[0]).toEqual({ action: "cancel", ticket: "b".repeat(64) });
    } finally { hold.resolve({ kind: "point", basis, terminal }); f.client.dispose(); }
  });

  it("rejects a foreign basis, releases its ticket and permits a new gesture", async () => {
    let attempts = 0;
    const f = await fixture(async () => preview("a".repeat(64), ++attempts === 1 ? { ...basis, revision: 999 } : basis));
    try {
      await expect(f.client.beginPoint({ target, gestureId: 11, viewport, view })).rejects.toThrow("foreign basis");
      expect(f.rpc.mock.calls[1][0]).toEqual({ action: "cancel", ticket: "a".repeat(64) });
      await f.client.beginPoint({ target, gestureId: 12, viewport, view });
      await f.client.cancel();
      await expect(f.client.render(view)).rejects.toThrow("No authoring preview");
    } finally { f.client.dispose(); }
    expect(f.renderer.terminate).toHaveBeenCalledOnce();
  });

  it("disposal aborts the active HTTP request and releases a late response", async () => {
    const hold = deferred<Reply>(), started = deferred<AbortSignal | undefined>();
    const f = await fixture(async (_body, signal) => { started.resolve(signal); return hold.promise; });
    const begin = f.client.beginConstruction({ tool: "segment", gestureId: 13, viewport, view });
    const rejection = expect(begin).rejects.toThrow("disposed");
    const signal = await started.promise;
    f.client.dispose(); await rejection;
    expect(signal?.aborted).toBe(true);
    hold.resolve(preview());
    await vi.waitFor(() => expect(f.rpc.mock.calls.at(-1)?.[0]).toEqual({ action: "cancel", ticket: "a".repeat(64) }));
    expect(f.renderer.postMessage).not.toHaveBeenCalled();
  });
});

it("sends operation preselection for native mapping once and keeps subsequent navigation local",async()=>{
  const operation={sequence:0,completed:false,can_finish:false,has_pending:false,can_reset:false,can_step_back:false,diagnostic:null,pending:[],authoring_options:{tangent_orientation:"aligned" as const,curvature_relation:"signed" as const,continuity:{kind:"g1" as const},dimension_mode:"driving" as const,angle_orientation:"counter_clockwise" as const},fillet_options:{fillet_radius:2,flip_first_side:false,flip_second_side:false,alternate_arc:false},fillet_corner_count:0,fillet_corners:[],offset_distance:null};
  const command={basis:"digest",gesture_id:16,viewport,tool:"fillet" as const,selection:[],samples:[],expected_declarations:[]};
  const f=await fixture(async body=>body.action==="finish"?{kind:"operation",basis,command}:{...preview(),kind:"preview",basis,ticket:"a".repeat(64),presentation:"native operation",operation});
  try{
    expect((await f.client.beginOperation({tool:"fillet",gestureId:16,viewport,view})).operation).toEqual(operation);
    expect(f.rpc.mock.calls[0][0]).toEqual({action:"begin",basis,kind:"operation",tool:"fillet",gestureId:16,viewport,view:{state:view.state},selection:undefined,options:undefined});
    const first=f.client.advanceOperation({sequence:1,input:{event:"fillet_radius",radius:4}},view);
    const second=f.client.advanceOperation({sequence:2,input:{event:"complete"}},view);
    await Promise.all([first,second]);
    expect(f.rpc.mock.calls[1][0]).toEqual({action:"advance",ticket:"a".repeat(64),samples:[{sequence:1,input:{event:"fillet_radius",radius:4}},{sequence:2,input:{event:"complete"}}]});
    const count=f.rpc.mock.calls.length;
    await f.client.render({...view,state:{camera:"new camera"}});expect(f.rpc).toHaveBeenCalledTimes(count);
    expect((await f.client.finishOperation()).command).toEqual(command);
    expect(JSON.stringify(f.rpc.mock.calls)).not.toMatch(/private project|private design|new camera/);
  }finally{f.client.dispose();}
});

it("maps Explorer picks on the server from personal state and preserves option choices before activation",async()=>{
  const f=await fixture();
  try{
    const options={offset_distance:3};
    await f.client.beginOperation({tool:"offset",gestureId:19,viewport,view,options});
    expect(f.rpc.mock.calls[0][0]).toMatchObject({action:"begin",kind:"operation",options,view:{state:view.state}});
    const selected={...view,state:{selection:"native curve occurrence"}};
    await f.client.pickOperationSelection(1,selected);
    expect(f.rpc.mock.calls[1][0]).toEqual({action:"pick_selection",ticket:"a".repeat(64),sequence:1,view:{state:selected.state}});
    expect(JSON.stringify(f.rpc.mock.calls)).not.toContain("local scene");
  }finally{f.client.dispose();}
});

it("ignores a late local paint after replacement without retiring the new server ticket", async () => {
  const f = await fixture(async body => preview("b".repeat(64), body.basis as typeof basis));
  const paint = f.renderer.postMessage.getMockImplementation()!;
  f.renderer.postMessage.mockImplementation(() => {});
  try {
    const old = expect(f.client.beginPoint({ target, gestureId: 19, viewport, view })).rejects.toThrow("replaced");
    await vi.waitFor(() => expect(f.renderer.postMessage).toHaveBeenCalledOnce());
    const stale = f.renderer.postMessage.mock.calls[0][0];
    await f.client.replace({ ...model, revision: 2 }); await old;
    f.renderer.postMessage.mockImplementation(paint);
    expect((await f.client.beginPoint({ target, gestureId: 20, viewport, view })).model.revision).toBe(2);
    paint(stale);
    expect((await f.client.render(view)).model.revision).toBe(2);
    await f.client.cancel();
    expect(f.rpc.mock.calls.at(-1)?.[0]).toEqual({ action: "cancel", ticket: "b".repeat(64) });
  } finally { f.client.dispose(); }
});
