// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { CollaborationAuthoringController } from "./collaboration-authoring-controller";
import type { AuthoringModel, AuthoringPreview, AuthoringView, LocalAuthoringClient } from "./collaboration-authoring-adapter";
import type { AuthoringPointer } from "./local-interaction-adapter";
import type { PointerSample } from "./adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
const model={documentEpoch:"doc",revision:1,sourceDesignDigest:"basis",project:"{}",design:{format:"geosolve-design-v1",project:"doc",generated:{},overrides:{}}} as unknown as AuthoringModel;
const view:AuthoringView={seed:{scene_key:"a"},state:{viewport:{}}};
const target={target:"point" as const,address:{project:"doc",owner:{address:{owner:"direct_declaration" as const,declaration:"point"},allocation:1,generation:1},output:[],field:"point" as const}};
const projection:AuthoringPointer={viewport:{screen_size:[800,600],model_center:[0,0],pixels_per_model_unit:10},position:[0,0],target};
const pointer=(phase:PointerSample["phase"],x:number):PointerSample=>({version:2,phase,pointerId:1,x,y:0,buttons:phase==="up"?0:1,modifiers:{alt:false,ctrl:false,meta:false,shift:false}});
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(yes=>{resolve=yes;});return {promise,resolve};}
async function fixture(){
  const frame=structuredClone((await new MockWorkbenchAdapter().snapshot()).frame);
  frame.scene.provenance.scene="provisional";frame.scene.items.forEach(item=>{item.interactive=false;});
  const preview:AuthoringPreview={kind:"preview",model,view,frame};
  const worker:LocalAuthoringClient={replace:vi.fn<LocalAuthoringClient["replace"]>(async()=>({kind:"ready",model})),beginPoint:vi.fn(async()=>preview),advancePoint:vi.fn(async()=>preview),
    finishPoint:vi.fn<LocalAuthoringClient["finishPoint"]>(async()=>({kind:"point",model,terminal:{command:{basis:"basis",gesture_id:1,target,viewport:projection.viewport,samples:[]},accepted_position:[2,0]}})),
    beginConstruction:vi.fn(async()=>preview),advanceConstruction:vi.fn(async()=>preview),finishConstruction:vi.fn<LocalAuthoringClient["finishConstruction"]>(async()=>({kind:"construction",model,command:{basis:"basis",gesture_id:1,viewport:projection.viewport,tool:"segment",role:"profile",samples:[],expected_declarations:[]}})),
    render:vi.fn(async()=>preview),cancel:vi.fn<LocalAuthoringClient["cancel"]>(async()=>({kind:"cancelled",model})),dispose:vi.fn()};
  const callbacks={paint:vi.fn(),changed:vi.fn(),error:vi.fn(),commit:vi.fn(async()=>{})};
  const controller=new CollaborationAuthoringController(worker,callbacks);controller.replace(model);
  return {controller,worker,callbacks,preview};
}
it("returns from pointer routing while prediction is held and preserves terminal sample order",async()=>{
  const f=await fixture(),hold=deferred<AuthoringPreview>();f.worker.beginPoint=vi.fn(()=>hold.promise);
  f.controller.pointer(pointer("down",0),projection,view);
  expect(f.controller.pointer(pointer("move",4),{...projection,position:[1,0]},view)).toBeUndefined();
  expect(f.controller.pointer(pointer("move",8),{...projection,position:[2,0]},view)).toBeUndefined();
  expect(f.controller.pointer(pointer("up",12),{...projection,position:[3,0]},view)).toBeUndefined();
  await vi.waitFor(()=>expect(f.worker.beginPoint).toHaveBeenCalledOnce());
  expect(f.worker.advancePoint).not.toHaveBeenCalled();expect(f.callbacks.commit).not.toHaveBeenCalled();
  hold.resolve(f.preview);
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledOnce());
  expect(vi.mocked(f.worker.advancePoint).mock.calls.map(([sample])=>sample)).toEqual([{sequence:1,position:[1,0]},{sequence:2,position:[2,0]},{sequence:3,position:[3,0]}]);
  expect(f.worker.finishPoint).toHaveBeenCalledOnce();expect(f.callbacks.error).not.toHaveBeenCalled();f.controller.dispose();
});
it("uses native construction completion and retires queued events from its finished gesture",async()=>{
  const f=await fixture();
  f.worker.advanceConstruction=vi.fn(async()=>({...f.preview,construction:{sequence:1,completed:true,can_finish:false,preview:null,inference_guides:[],adjusted_position:null,diagnostic:null}}));
  f.controller.select("segment");
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",8),projection,view);
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledOnce());
  expect(f.worker.advanceConstruction).toHaveBeenCalledOnce();expect(f.worker.finishConstruction).toHaveBeenCalledOnce();
  expect(f.callbacks.commit).toHaveBeenCalledWith(model,expect.any(Object),"construction");f.controller.dispose();
});
it("does not publish prediction or a terminal after the accepted model changes",async()=>{
  const f=await fixture(),hold=deferred<AuthoringPreview>();f.worker.beginPoint=vi.fn(()=>hold.promise);
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",5),projection,view);f.controller.pointer(pointer("up",6),projection,view);
  await vi.waitFor(()=>expect(f.worker.beginPoint).toHaveBeenCalledOnce());
  f.controller.replace({...model,revision:2});hold.resolve(f.preview);
  await vi.waitFor(()=>expect(f.worker.replace).toHaveBeenCalledTimes(2));
  await Promise.resolve();expect(f.callbacks.paint).not.toHaveBeenCalled();expect(f.callbacks.commit).not.toHaveBeenCalled();f.controller.dispose();
});
