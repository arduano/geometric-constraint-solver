// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { CollaborationAuthoringController, supportsCollaborativeConstruction, supportsCollaborativeTool } from "./collaboration-authoring-controller";
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
    beginOperation:vi.fn(async()=>preview),advanceOperation:vi.fn(async()=>preview),pickOperationSelection:vi.fn(async()=>preview),finishOperation:vi.fn(),
    render:vi.fn(async()=>preview),cancel:vi.fn<LocalAuthoringClient["cancel"]>(async()=>({kind:"cancelled",model})),dispose:vi.fn()};
  const callbacks={paint:vi.fn(),changed:vi.fn(),cleared:vi.fn(),error:vi.fn(),commit:vi.fn(async()=>{})};
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
  f.worker.advanceConstruction=vi.fn(async()=>({...f.preview,construction:{sequence:1,completed:true,can_finish:false,has_pending:false,can_reset:false,can_step_back:false,can_cycle_inference:false,can_flip_branch:false,stage:null,conic_options:{minor_axis_ratio:0.5,arc_start:0,arc_end:1,arc_sweep:"counter_clockwise" as const,middle_weight:1,trim_start:-1,trim_end:1,semi_conjugate:1,hyperbola_branch:"positive" as const},nurbs_options:{form:"clamped" as const,degree:3,weights:[],gauge_index:0},preview:null,inference_guides:[],adjusted_position:null,diagnostic:null}}));
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

it("advertises exactly the native collaborative construction tools and refuses unsupported selection",async()=>{
  const f=await fixture(),catalog=await new MockWorkbenchAdapter().toolCatalog();
  expect(catalog.sections.flatMap(section=>section.commands).filter(command=>supportsCollaborativeConstruction(command.toolId))).toHaveLength(25);
  expect(catalog.sections.flatMap(section=>section.commands).every(command=>supportsCollaborativeTool(command.toolId))).toBe(true);
  expect(()=>f.controller.select("unknown-tool")).toThrow(/not connected/);
  expect(f.controller.tool).toBe("select");f.controller.dispose();
});

it("reprojects the retained native terminal while publication is held and ignores new drawing until its outcome",async()=>{
  const f=await fixture(),publication=deferred<void>();f.callbacks.commit=vi.fn(()=>publication.promise);
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",5),projection,view);f.controller.pointer(pointer("up",8),projection,view);
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledOnce());
  const navigated={...view,state:{viewport:{zoom:20}}};
  f.controller.render(navigated);
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",5),projection,view);
  await vi.waitFor(()=>expect(f.worker.render).toHaveBeenCalledWith(navigated));
  expect(f.worker.beginPoint).toHaveBeenCalledOnce();
  expect(f.worker.cancel).not.toHaveBeenCalled();
  publication.resolve();await vi.waitFor(()=>expect(f.worker.cancel).toHaveBeenCalledOnce());
  expect(f.callbacks.cleared).toHaveBeenCalledTimes(2);f.controller.dispose();
});

it("does not let a cancelled in-flight prediction failure cancel the next gesture",async()=>{
  const f=await fixture();let reject!:(error:Error)=>void;
  f.worker.beginPoint=vi.fn().mockImplementationOnce(()=>new Promise<AuthoringPreview>((_,no)=>{reject=no;})).mockResolvedValue(f.preview);
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",5),projection,view);
  await vi.waitFor(()=>expect(f.worker.beginPoint).toHaveBeenCalledOnce());
  f.controller.cancel();
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",8),projection,view);f.controller.pointer(pointer("up",10),projection,view);
  reject(Error("Cancelled obsolete prediction"));
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledOnce());
  expect(f.callbacks.error).not.toHaveBeenCalled();f.controller.dispose();
});

it("batches queued point painting while preserving the exact terminal sequence",async()=>{
  const f=await fixture(),hold=deferred<AuthoringPreview>();f.worker.beginPoint=vi.fn(()=>hold.promise);
  f.controller.pointer(pointer("down",0),projection,view);
  for(let x=4;x<=24;x++)f.controller.pointer(pointer("move",x),{...projection,position:[x,0]},view);
  f.controller.pointer(pointer("up",25),{...projection,position:[25,0]},view);
  await vi.waitFor(()=>expect(f.worker.beginPoint).toHaveBeenCalledOnce());hold.resolve(f.preview);
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledOnce());
  expect(vi.mocked(f.worker.advancePoint).mock.calls.map(([sample])=>sample)).toEqual(Array.from({length:22},(_,index)=>({sequence:index+1,position:[index+4,0]})));
  expect(f.callbacks.paint).toHaveBeenCalledTimes(2); // Begin plus the batched final frame.
  f.controller.dispose();
});

it("keeps an identical accepted model open without invalidating its retained gesture",async()=>{
  const f=await fixture();
  f.controller.pointer(pointer("down",0),projection,view);f.controller.pointer(pointer("move",5),projection,view);
  await vi.waitFor(()=>expect(f.callbacks.paint).toHaveBeenCalled());
  f.controller.replace(structuredClone(model));
  f.controller.pointer(pointer("up",8),projection,view);
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledOnce());
  expect(f.worker.replace).toHaveBeenCalledOnce();
  f.controller.replace({...model,project:'{"changed":true}'});
  expect(f.worker.replace).toHaveBeenCalledTimes(2);
  f.controller.dispose();
});

it("starts all native construction variants and records correction/options samples in order",async()=>{
  const f=await fixture(),catalog=await new MockWorkbenchAdapter().toolCatalog();
  for(const tool of catalog.sections.find(section=>section.id==="sketch")!.commands){
    f.controller.select(tool.toolId);f.controller.pointer(pointer("move",0),projection,view);
    await vi.waitFor(()=>expect(f.worker.beginConstruction).toHaveBeenLastCalledWith(expect.objectContaining({tool:tool.toolId.replaceAll("-","_")})));
  }
  f.controller.constructionEvent({event:"flip_branch"});f.controller.constructionEvent({event:"cycle_inference"});f.controller.stepBack();
  const options={form:"periodic" as const,degree:2,weights:[1,2,1],gauge_index:1};
  f.controller.constructionEvent({event:"nurbs_options",options});f.controller.finish();
  await vi.waitFor(()=>expect(f.worker.advanceConstruction).toHaveBeenLastCalledWith({sequence:6,input:{event:"complete"}},view));
  expect(vi.mocked(f.worker.advanceConstruction).mock.calls.slice(-5).map(([sample])=>sample.input)).toEqual([{event:"flip_branch"},{event:"cycle_inference"},{event:"step_back"},{event:"nurbs_options",options},{event:"complete"}]);
  expect(f.callbacks.error).not.toHaveBeenCalled();f.controller.dispose();
});

const operationFrame={sequence:0,completed:false,can_finish:false,has_pending:false,can_reset:false,can_step_back:false,diagnostic:null,pending:[],authoring_options:{tangent_orientation:"aligned" as const,curvature_relation:"signed" as const,continuity:{kind:"g1" as const},dimension_mode:"driving" as const,angle_orientation:"counter_clockwise" as const},fillet_options:{fillet_radius:2,flip_first_side:false,flip_second_side:false,alternate_arc:false},fillet_corner_count:0,fillet_corners:[],offset_distance:null};
const operationCommand={basis:"basis",gesture_id:1,viewport:projection.viewport,tool:"horizontal" as const,selection:[],samples:[],expected_declarations:[]};
for(const tool of ["polyline","fillet"])it(`records the current camera before the next ${tool} pick without coupling navigation to authoring`,async()=>{
  const f=await fixture();f.controller.select(tool,{viewport:projection.viewport,view});
  const nextViewport={...projection.viewport,pixels_per_model_unit:40},navigated={...view,state:{viewport:nextViewport}};
  f.controller.render(navigated);
  await vi.waitFor(()=>expect(f.worker.render).toHaveBeenCalledWith(navigated));
  expect(f.worker.advanceConstruction).not.toHaveBeenCalled();expect(f.worker.advanceOperation).not.toHaveBeenCalled();
  f.controller.pointer(pointer("down",4),{...projection,viewport:nextViewport,position:[1,2]},navigated);
  const advance=tool==="fillet"?f.worker.advanceOperation:f.worker.advanceConstruction;
  await vi.waitFor(()=>expect(advance).toHaveBeenCalledTimes(2));
  const inputs=vi.mocked(advance).mock.calls.map(([sample])=>sample);
  expect(inputs[0]).toEqual({sequence:1,input:{event:"viewport",viewport:nextViewport}});
  expect(inputs[1]).toEqual({sequence:2,input:expect.objectContaining({event:"click",position:[1,2]})});
  expect(f.callbacks.error).not.toHaveBeenCalled();f.controller.dispose();
});
it("uses native preselection completion once and keeps terminal navigation independent from publication",async()=>{
  const f=await fixture(),publication=deferred<void>();
  f.callbacks.commit=vi.fn(()=>publication.promise);
  f.worker.beginOperation=vi.fn(async()=>({...f.preview,operation:{...operationFrame,completed:true}}));
  f.worker.finishOperation=vi.fn<LocalAuthoringClient["finishOperation"]>(async()=>({kind:"operation",model,command:operationCommand}));
  const selection=[{target:"binding" as const,symbol:"edge",binding:0,span:0,curve_parameter:0.5}];
  f.controller.select("horizontal",{viewport:projection.viewport,selection,view});
  f.controller.pointer(pointer("down",4),projection,view);
  await vi.waitFor(()=>expect(f.callbacks.commit).toHaveBeenCalledWith(model,operationCommand,"operation"));
  expect(f.worker.beginOperation).toHaveBeenCalledWith({tool:"horizontal",gestureId:1,viewport:projection.viewport,selection,view,options:undefined});
  expect(f.worker.advanceOperation).not.toHaveBeenCalled();
  const navigated={...view,state:{zoom:5}};f.controller.render(navigated);
  await vi.waitFor(()=>expect(f.worker.render).toHaveBeenCalledWith(navigated));
  expect(f.worker.cancel).toHaveBeenCalledTimes(1); // Selecting the tool retires previous state only.
  publication.resolve();await vi.waitFor(()=>expect(f.worker.cancel).toHaveBeenCalledTimes(2));
  f.controller.dispose();
});
it("keeps operation picks/options ordered and clears obsolete terminal responses",async()=>{
  const f=await fixture(),held=deferred<AuthoringPreview>();f.worker.beginOperation=vi.fn(()=>held.promise);
  f.worker.advanceOperation=vi.fn(async()=>({...f.preview,operation:operationFrame}));
  f.controller.select("fillet",{viewport:projection.viewport,view});
  f.controller.pointer(pointer("down",1),{...projection,position:[1,2]},view);
  f.controller.operationEvent({event:"fillet_radius",radius:4});f.controller.stepBack();f.controller.finish();
  await vi.waitFor(()=>expect(f.worker.beginOperation).toHaveBeenCalledOnce());held.resolve({...f.preview,operation:operationFrame});
  await vi.waitFor(()=>expect(f.worker.advanceOperation).toHaveBeenCalledTimes(4));
  expect(vi.mocked(f.worker.advanceOperation).mock.calls.map(([sample])=>sample)).toEqual([{sequence:1,input:{event:"click",position:[1,2]}},{sequence:2,input:{event:"fillet_radius",radius:4}},{sequence:3,input:{event:"step_back"}},{sequence:4,input:{event:"complete"}}]);
  f.controller.replace({...model,revision:2});expect(f.controller.operation).toBeUndefined();expect(f.callbacks.commit).not.toHaveBeenCalled();f.controller.dispose();
});

it("remembers native operation options across accepted replacements before applying new preselection",async()=>{
  const f=await fixture();
  const remembered={...operationFrame,authoring_options:{...operationFrame.authoring_options,dimension_mode:"reference" as const}};
  f.worker.beginOperation=vi.fn(async()=>({...f.preview,operation:remembered}));
  f.controller.select("radius",{viewport:projection.viewport,view});
  await vi.waitFor(()=>expect(f.controller.operation?.authoring_options.dimension_mode).toBe("reference"));
  f.controller.replace({...model,revision:2});
  f.controller.select("radius",{viewport:projection.viewport,view,selection:[]});
  await vi.waitFor(()=>expect(f.worker.beginOperation).toHaveBeenCalledTimes(2));
  expect(vi.mocked(f.worker.beginOperation).mock.lastCall?.[0].options).toEqual({authoring_options:remembered.authoring_options,fillet_options:remembered.fillet_options,offset_distance:remembered.offset_distance});
  f.controller.dispose();
});
it("routes native Explorer selection as one ordered operation sample",async()=>{
  const f=await fixture();f.worker.beginOperation=vi.fn(async()=>({...f.preview,operation:operationFrame}));
  f.worker.advanceOperation=vi.fn(async()=>({...f.preview,operation:operationFrame}));
  f.worker.pickOperationSelection=vi.fn(async()=>({...f.preview,operation:operationFrame}));
  f.controller.select("parallel",{viewport:projection.viewport,view});
  f.controller.pointer(pointer("down",5),projection,view);
  const picked={...view,state:{nativeSelection:"second curve occurrence"}};f.controller.pickSelection(picked);
  f.controller.finish();
  await vi.waitFor(()=>expect(f.worker.pickOperationSelection).toHaveBeenCalledWith(2,picked));
  expect(f.worker.advanceOperation).toHaveBeenLastCalledWith({sequence:3,input:{event:"complete"}},picked);
  f.controller.dispose();
});

it("tracks held native authoring independently from navigation and retires activity after cancellation",async()=>{
  const f=await fixture(),held=deferred<AuthoringPreview>(),finished=vi.fn(),activity=vi.fn(()=>finished);
  const controller=new CollaborationAuthoringController(f.worker,{...f.callbacks,activity});controller.replace(model);
  f.worker.beginOperation=vi.fn(()=>held.promise);controller.select("fillet",{viewport:projection.viewport,view});
  await vi.waitFor(()=>expect(activity).toHaveBeenCalledOnce());expect(finished).not.toHaveBeenCalled();
  controller.cancel();held.resolve(f.preview);await vi.waitFor(()=>expect(finished).toHaveBeenCalledOnce());
  controller.dispose();
});
