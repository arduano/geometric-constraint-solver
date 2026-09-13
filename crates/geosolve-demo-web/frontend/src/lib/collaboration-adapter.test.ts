// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, expect, it, vi } from "vitest";
import { CollaborativeWorkbenchAdapter, type CollaborativeTextClient, type CollaborativeDocument } from "./collaboration-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
import { createCollaborativeWorkbenchSession } from "./collaborative-workbench-session";
import type { LocalInteractionClient, LocalInteractionUpdate } from "./local-interaction-adapter";
import type { TextWorkerUpdate } from "./collaboration-text-adapter";
import type { PointerSample, WorkbenchSnapshot } from "./adapter";
import type { AuthoringModel, AuthoringPreview, AuthoringView, LocalAuthoringClient } from "./collaboration-authoring-adapter";
import type { AuthoringPointer } from "./local-interaction-adapter";
import type { LocalBrowsingClient } from "./collaboration-browsing-adapter";

const handles:CollaborativeWorkbenchAdapter[]=[];
afterEach(()=>{for(const handle of handles.splice(0))handle.dispose();});
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(yes=>{resolve=yes;});return{promise,resolve};}
async function fixture(fixtureOptions:{textResponse?:()=>Promise<Response>;browsing?:LocalBrowsingClient;navigation?:WorkbenchSnapshot["navigation"];authoring?:LocalAuthoringClient;onTextOpen?:()=>void}={}){
  const mock=new MockWorkbenchAdapter(),snapshot=await mock.snapshot(),toolCatalog=await mock.toolCatalog();
  snapshot.authoringDocument={authority:"server-inspector",title:"Sketch",description:"",areKeyConstraintsByDefault:false,editable:true};
  if(fixtureOptions.navigation)snapshot.navigation=fixtureOptions.navigation;
  const textSnapshot:TextWorkerUpdate={snapshot:{revision:{heads:[]},files:{"sketch.ts":"const a = 1;"}},changes:[],checkpoint:[],history:{undo:0,redo:0}};
  const text:CollaborativeTextClient={open:async()=>{fixtureOptions.onTextOpen?.();return textSnapshot;},edit:async()=>textSnapshot,receive:async()=>textSnapshot,undo:async()=>textSnapshot,redo:async()=>textSnapshot,snapshot:async()=>textSnapshot,dispose:()=>{}};
  let localCount=0;
  const view:LocalInteractionUpdate={frame:snapshot.frame,state:{},selectionChanged:false,serverFrameCompatible:false};
  const local:LocalInteractionClient={construct:async()=>view,replace:async()=>view,update:async()=>({ ...view,frame:{...view.frame,ariaLabel:`local ${++localCount}`}}),state:async()=>({}),dispose:()=>{}};
  const requests:string[]=[],held=deferred<Response>();
  const document={accepted:{modelRevision:0,acceptedInput:"a",files:textSnapshot.snapshot.files},working:textSnapshot.snapshot,model:{project:"{}",design:{},sourceDesignDigest:"digest"},textActor:[1],textCheckpoint:[],targets:{},inventory:{objects:[],properties:[]},pointTargets:[]} as unknown as CollaborativeDocument;
  const json=(value:unknown,headers?:Record<string,string>)=>new Response(JSON.stringify(value),{headers:{"Content-Type":"application/json",...headers}});
  const fetch=vi.fn(async(url:URL|RequestInfo,options?:RequestInit)=>{
    const route=new URL(String(url)).pathname.split("/").at(-1)!;requests.push(route);
    if(route==="join")return json({connection:{protocol:1,documentId:"doc",documentEpoch:"epoch",userId:"alice",clientId:"tab",role:"editor"},token:"a".repeat(64)});
    if(route==="state")return json({authority:{acceptedRevision:document.accepted.modelRevision,acceptedInput:"a",latestSequence:0},document,participants:[],presence:[]});
    if(route==="scene")return json({seed:{}},{"X-Geosolve-Revision":String(document.accepted.modelRevision)});
    if(route==="events")return new Response(new ReadableStream({start(controller){options?.signal?.addEventListener("abort",()=>{try{controller.close();}catch{}});}}),{headers:{"Content-Type":"text/event-stream"}});
    if(route==="commands")return held.promise;
    if(route==="text")return fixtureOptions.textResponse?fixtureOptions.textResponse():json({sourceSequence:1});
    if(route==="text-state")return json({sourceSequence:1,workingRevision:document.working.revision,changes:[],history:document.textHistory});
    throw Error(`Unexpected ${route}`);
  });
  const identity={documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"};
  const fallback:LocalBrowsingClient={
    replace:async model=>({kind:"ready",model}),
    present:async view=>({kind:"chrome",model:identity,view,chrome:{explorer:snapshot.explorer,navigation:snapshot.navigation,dimensions:snapshot.dimensions,parameters:snapshot.parameters,problems:snapshot.problems,selection:null,selectedGeometryRole:null,authoringDocument:snapshot.authoringDocument??null}}),
    navigate:vi.fn(),describe:vi.fn(),dispose:()=>{},
  };
  const browsing=fixtureOptions.browsing??fallback;
  browsing.initialize??=async(model,seed)=>{
    // Initial projection is independent of subsequent browsing refresh barriers.
    if(model.revision>0)await browsing.replace(model,seed);
    return {kind:"initialized",model,snapshot,seed,toolCatalog};
  };
  const adapter=new CollaborativeWorkbenchAdapter({baseUrl:"http://test/api/collaboration/",inviteToken:"invite",clientId:"tab",fetch:fetch as typeof globalThis.fetch},local,text,fixtureOptions.authoring??null,browsing);
  handles.push(adapter);await adapter.construct();
  return{adapter,requests,held,json,localCount:()=>localCount,snapshot,text,document,textSnapshot,local,view,browsing};
}

it("keeps the installed canvas responsive while local accepted initialization is held",async()=>{
  const f=await fixture(),entered=deferred<void>(),release=deferred<void>();
  const initialize=f.browsing.initialize!;
  f.browsing.initialize=async(model,seed)=>{entered.resolve();await release.promise;return initialize(model,seed);};
  Object.assign(f.document.accepted,{modelRevision:1});
  const refresh=f.adapter.refresh();await entered.promise;
  const requests=f.requests.length;
  const frame=await f.adapter.wheel({version:2,x:100,y:100,deltaX:0,deltaY:-20,ctrl:false});
  expect(frame?.frame.ariaLabel).toBe("local 1");expect(f.requests.length).toBe(requests);
  release.resolve();await refresh;
  expect((await f.adapter.snapshot()).project.status).toBe("accepted");
});

it("keeps local navigation and dimension browsing independent of a held semantic request",async()=>{
  const f=await fixture();
  const editing=f.adapter.submit({kind:"semantic",basisRevision:0,payload:{action:"values",writes:[]}});
  await vi.waitFor(()=>expect(f.requests).toContain("commands"));
  const requests=f.requests.length;
  const zoom=await f.adapter.wheel({version:2,x:100,y:100,deltaX:0,deltaY:-20,ctrl:false});
  const hover=await f.adapter.pointer({version:2,phase:"move",pointerId:1,x:10,y:10,buttons:0,modifiers:{alt:false,ctrl:false,meta:false,shift:false}});
  const dimensions=await f.adapter.dispatch({version:2,command:"dimensions.mode",payload:{mode:"hidden"}});
  expect(zoom?.frame.ariaLabel).toBe("local 1");expect(hover?.frame.ariaLabel).toBe("local 2");expect(dimensions.frame.ariaLabel).toBe("local 3");
  expect(f.requests.length).toBe(requests);expect(f.localCount()).toBe(3);
  const id=f.adapter.pending[0]!.requestId;
  f.held.resolve(f.json({receipt:{operation:{userId:"alice",clientId:"tab",requestId:id},admission:1,outcome:{status:"rejected",code:"fixture",message:"Held fixture completed"}}}));
  expect((await editing).outcome?.status).toBe("rejected");
  expect((await f.adapter.snapshot()).frame.ariaLabel).toBe("local 3");
  expect(f.adapter.activity.getPendingSnapshot()).toBe(false);
});

it("describes an initial parameter edit with the local Inspector authority after its worker finishes opening",async()=>{
  const mock=await new MockWorkbenchAdapter().snapshot(),ready=deferred<void>(),presented=deferred<void>();
  const identity={documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"};
  const browsing:LocalBrowsingClient={
    replace:async()=>{await ready.promise;return {kind:"ready",model:identity};},
    present:async(view)=>{await presented.promise;return {kind:"chrome",model:identity,view,chrome:{explorer:mock.explorer,navigation:mock.navigation,dimensions:mock.dimensions,parameters:mock.parameters,problems:mock.problems,
      selection:null,selectedGeometryRole:null,authoringDocument:{authority:"local-inspector",title:"Sketch",description:"",areKeyConstraintsByDefault:false,editable:true}}};},
    navigate:vi.fn(),describe:vi.fn(async(view,authority)=>{
      expect(authority).toBe("local-inspector");return {kind:"mutation" as const,model:identity,view,mutation:null};
    }),dispose:()=>{ready.resolve();presented.resolve();},
  };
  const f=await fixture({browsing});
  // Initial and refreshed read projections carry their own exact Inspector authority.
  const editing=f.adapter.dispatch({version:2,command:"parameter.edit",payload:{id:"channelWidth",value:11}});
  ready.resolve();presented.resolve();await editing;
  expect(browsing.describe).toHaveBeenCalledOnce();
});

it("keeps each tab's local frame independent and validates the genuine catalog boundary",async()=>{
  const a=await fixture(),b=await fixture();
  await a.adapter.wheel({version:2,x:0,y:0,deltaX:0,deltaY:20,ctrl:false});
  expect((await a.adapter.snapshot()).frame.ariaLabel).toBe("local 1");
  expect((await b.adapter.snapshot()).frame).toEqual(b.snapshot.frame);
  expect((await a.adapter.toolCatalog()).version).toBe(1);
});

it("maps an initial Explorer action to the ready local navigation authority",async()=>{
  const mock=await new MockWorkbenchAdapter().snapshot(),ready=deferred<void>();
  const identity={documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"};
  const chrome={explorer:mock.explorer,navigation:{authority:"local-navigation",selectionKey:"empty",rows:[],sources:[],itemCount:0,canNavigateSource:true},dimensions:mock.dimensions,parameters:mock.parameters,problems:mock.problems,selection:null,selectedGeometryRole:null,authoringDocument:null};
  const browsing:LocalBrowsingClient={
    replace:async()=>{await ready.promise;return {kind:"ready",model:identity};},
    present:async(view)=>{await ready.promise;return {kind:"chrome",model:identity,view,chrome};},
    navigate:vi.fn(async(view,command,payload)=>{
      expect(command).toBe("navigation.rows.select");
      expect(payload).toEqual({authority:"local-navigation",ids:["managed:planDepth"],mode:"replace"});
      return {kind:"navigation" as const,model:identity,view,chrome,state:view.state};
    }),describe:vi.fn(),dispose:()=>ready.resolve(),
  };
  const f=await fixture({browsing,navigation:{...chrome.navigation,authority:"server-navigation"}});
  await expect(f.adapter.dispatch({version:2,command:"navigation.rows.select",payload:{authority:"obsolete-navigation",ids:["managed:planDepth"],mode:"replace"}})).rejects.toThrow(/older source revision/);
  const navigating=f.adapter.dispatch({version:2,command:"navigation.rows.select",payload:{authority:"server-navigation",ids:["managed:planDepth"],mode:"replace"}});
  ready.resolve();await navigating;
  expect(browsing.navigate).toHaveBeenCalledOnce();expect(f.requests).not.toContain("commands");
});

it("keeps queued edits on their actually displayed branch even after a remote refresh was emitted",async()=>{
  const f=await fixture(),original=(await f.adapter.snapshot()).source.files.find(file=>file.path==="sketch.ts")!;
  const before="const a = 1;",remote="// Bob\n",first=before+"a",second=first+"b";
  f.text.receive=async()=>({...f.textSnapshot,snapshot:{revision:{heads:["remote"]},files:{"sketch.ts":remote+before}}});
  await f.adapter.refresh();
  expect((await f.adapter.snapshot()).source.files[0]!.contents).toBe(remote+before);
  // React has emitted that refresh but an event from the old displayed editor
  // can still be queued. Both local keystrokes use the old display identity.
  const started=deferred<void>(),release=deferred<void>();let count=0;
  const revisions:string[][]=[];
  f.text.edit=async(revision)=>{
    revisions.push([...revision.heads]);
    const step=++count;if(step===1){started.resolve();await release.promise;}
    return {...f.textSnapshot,changes:[[step]],localRevision:{heads:[`local${step}`]},snapshot:{revision:{heads:[`merged${step}`]},files:{"sketch.ts":remote+(step===1?first:second)}}};
  };
  const a=f.adapter.editSource("sketch.ts",{displayId:original.sharedRevision,before,after:first,changes:[{from:before.length,to:before.length,insert:"a"}]});
  await started.promise;
  const b=f.adapter.editSource("sketch.ts",{displayId:original.sharedRevision,before:first,after:second,changes:[{from:first.length,to:first.length,insert:"b"}]});
  release.resolve();await Promise.all([a,b]);
  expect(revisions).toEqual([[],["local1"]]);
  await vi.waitFor(()=>expect(f.adapter.pendingSourceEdits).toEqual([]));
  expect((await f.adapter.snapshot()).source.files[0]!.contents).toBe(remote+second);
  expect(f.adapter.pendingSourceEdits).toEqual([]);
});

it("continues native typing and navigation while the first text acknowledgement is held",async()=>{
  const held=deferred<Response>(),f=await fixture({textResponse:()=>held.promise.then(response=>response.clone())});
  const original=(await f.adapter.snapshot()).source.files[0]!;let count=0;
  f.text.edit=async()=>{
    const step=++count;
    return {...f.textSnapshot,changes:[[step]],localRevision:{heads:[`local${step}`]},snapshot:{revision:{heads:[`local${step}`]},files:{"sketch.ts":`const a = 1;${"x".repeat(step)}`}}};
  };
  await f.adapter.editSource("sketch.ts",{displayId:original.sharedRevision,before:"const a = 1;",after:"const a = 1;x",changes:[{from:12,to:12,insert:"x"}]});
  await f.adapter.editSource("sketch.ts",{displayId:original.sharedRevision,before:"const a = 1;x",after:"const a = 1;xx",changes:[{from:13,to:13,insert:"x"}]});
  expect(count).toBe(2);
  expect(f.adapter.pendingSourceEdits).toHaveLength(2);
  expect((await f.adapter.wheel({version:2,x:100,y:100,deltaX:0,deltaY:-90,ctrl:false}))?.frame.ariaLabel).toBe("local 1");
  held.resolve(f.json({sourceSequence:2}));
  await vi.waitFor(()=>expect(f.adapter.pendingSourceEdits).toEqual([]));
});


it("routes visibility, isolation and construction visibility locally while a server edit is held",async()=>{
  const f=await fixture();
  const editing=f.adapter.submit({kind:"semantic",basisRevision:0,payload:{action:"values",writes:[]}});
  await vi.waitFor(()=>expect(f.requests).toContain("commands"));
  const requests=f.requests.length;
  for(const [command,payload] of [["explorer.visibility.set",{id:"row",visible:false}],["explorer.visibility.isolate",{id:"group"}],["explorer.visibility.restore",{}],["view.construction.toggle",{}]] as const){
    await f.adapter.dispatch({version:2,command,payload});
  }
  expect(f.localCount()).toBe(4);expect(f.requests.length).toBe(requests);
  const requestId=f.adapter.pending[0]!.requestId;
  f.held.resolve(f.json({receipt:{operation:{userId:"alice",clientId:"tab",requestId},admission:1,outcome:{status:"rejected",code:"fixture",message:"done"}}}));
  await editing;
});

it("sends suppression through the guarded semantic source mutation gateway",async()=>{
  const mock=await new MockWorkbenchAdapter().snapshot(),identity={documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"};
  const browsing:LocalBrowsingClient={
    replace:async()=>({kind:"ready",model:identity}),
    present:async view=>({kind:"chrome",model:identity,view,chrome:{explorer:mock.explorer,navigation:mock.navigation,dimensions:mock.dimensions,parameters:mock.parameters,problems:mock.problems,
      selection:null,selectedGeometryRole:null,authoringDocument:{authority:"local-inspector",title:"Sketch",description:"",areKeyConstraintsByDefault:false,editable:true}}}),
    navigate:vi.fn(),describe:vi.fn(async(view,authority,command,payload)=>{
      expect(authority).toBe("local-inspector");expect(command).toBe("declaration.suppression.set");expect(payload).toEqual({id:"managed:segment1",suppressed:true});
      return {kind:"mutation",model:identity,view,mutation:{mutation:"set_suppressed",target:{target:"declaration",declaration:"segment1"},suppressed:true}} as const;
    }),dispose:()=>{},
  };
  const f=await fixture({browsing}),target={object:"object1",generation:0};
  Object.assign(f.adapter.state!.document,{targets:{object1:target},inventory:{objects:[{object:"object1",declaration:"segment1",dependencies:[]}],properties:[]}});
  const submit=vi.spyOn(f.adapter,"submit").mockResolvedValue({operation:{userId:"alice",clientId:"tab",requestId:"suppression"},admission:1,outcome:{status:"rejected",code:"fixture",message:"stop before refresh"}});
  await expect(f.adapter.dispatch({version:2,command:"declaration.suppression.set",payload:{id:"managed:segment1",suppressed:true}})).rejects.toThrow("stop before refresh");
  expect(submit).toHaveBeenCalledWith({kind:"semantic",basisRevision:0,payload:{action:"mutation",targets:[target],mutation:{mutation:"set_suppressed",target:{target:"declaration",declaration:"segment1"},suppressed:true}}});
});

it("preserves the full catalog and labels unavailable authoring capabilities",async()=>{
  const f=await fixture(),catalog=await f.adapter.toolCatalog(),native=await new MockWorkbenchAdapter().toolCatalog();
  expect(catalog.sections.flatMap(section=>section.commands).map(command=>command.toolId)).toEqual(native.sections.flatMap(section=>section.commands).map(command=>command.toolId));
  expect(catalog.select.unavailableReason).toBeUndefined();
  expect(catalog.sections.flatMap(section=>section.commands).every(command=>Boolean(command.unavailableReason))).toBe(true);
});


// The native authoring owner validates mathematics; this thin adapter witness
// records exactly which accepted/provisional coordinates it exposes to the UI.
async function dragFixture(){
  const base=(await new MockWorkbenchAdapter().snapshot()).frame;
  const frame=(x:number,authority:"accepted"|"provisional"):WorkbenchSnapshot["frame"]=>({...base,ariaLabel:`${authority} x=${x}`,scene:{...base.scene,provenance:{...base.scene.provenance,scene:authority},items:[{
    id:"moving-point",kind:"circle",center:[x,0],radius:3,layer:"geometry",semanticKey:"point",className:"",accessibleLabel:"Point",interactive:authority==="accepted",metadata:{},
    style:{fill:"white",stroke:null,strokeWidth:0,dash:[],opacity:1,lineCap:"round",lineJoin:"round",nonScalingStroke:true,textBaseline:"central",letterSpacing:0,fontFamily:"sans-serif",fontSize:12,fontWeight:400,textAnchor:"middle",shadow:null},
  }]}});
  const target={target:"point" as const,address:{project:"doc",owner:{address:{owner:"direct_declaration" as const,declaration:"circle"},allocation:1,generation:1},output:[],field:"point" as const}};
  const viewport={screen_size:[1000,700] as const,model_center:[0,0] as const,pixels_per_model_unit:10};
  let model!:AuthoringModel,lastView!:AuthoringView,x=0;
  const samples:{sequence:number;position:readonly[number,number]}[]=[];
  const preview=():AuthoringPreview=>({kind:"preview",model,view:lastView,frame:frame(x,"provisional"),presentation:JSON.stringify({opaqueCandidate:x})});
  const worker:LocalAuthoringClient={
    replace:vi.fn<LocalAuthoringClient["replace"]>(async next=>{model=next;return {kind:"ready",model};}),
    beginPoint:vi.fn(async input=>{lastView=input.view;return preview();}),
    advancePoint:vi.fn(async(sample,view)=>{samples.push(sample);x=sample.position[0];lastView=view;return preview();}),
    finishPoint:vi.fn<LocalAuthoringClient["finishPoint"]>(async()=>({kind:"point",model,terminal:{command:{basis:"digest",gesture_id:1,target,viewport,samples},accepted_position:[x,0]}})),
    beginConstruction:vi.fn(),advanceConstruction:vi.fn(),finishConstruction:vi.fn(),beginOperation:vi.fn(),advanceOperation:vi.fn(),pickOperationSelection:vi.fn(),finishOperation:vi.fn(),
    render:vi.fn(async view=>{lastView=view;return preview();}),cancel:vi.fn<LocalAuthoringClient["cancel"]>(async()=>({kind:"cancelled",model})),dispose:vi.fn(),
  };
  const f=await fixture({authoring:worker});
  f.view.frame=frame(0,"accepted");
  f.local.authoringPointer=async input=>({target,viewport,position:[input.x,0]} satisfies AuthoringPointer);
  Object.assign(f.adapter.state!.document,{targets:{circle:{object:"circle",generation:0}},inventory:{objects:[{object:"circle",declaration:"circle",dependencies:[]}],properties:[]}});
  const displayed:{authority:string;x:number}[]=[];
  const record=(snapshot?:WorkbenchSnapshot|null)=>{const circle=snapshot?.frame.scene.items.find(item=>item.id==="moving-point");if(circle?.kind==="circle")displayed.push({authority:snapshot!.frame.scene.provenance.scene,x:circle.center[0]});};
  f.adapter.subscribe(record);
  const pointer=async(phase:PointerSample["phase"],position:number)=>{
    record(await f.adapter.pointer({version:2,phase,pointerId:1,x:position,y:0,buttons:phase==="up"?0:1,modifiers:{alt:false,ctrl:false,meta:false,shift:false}}));
  };
  return {...f,worker,displayed,pointer,record,frame,viewport};
}

it("retains the displayed drag through pointer/presence frames and its pending terminal, then installs only its new accepted geometry",async()=>{
  const f=await dragFixture();
  await f.pointer("down",0);await f.pointer("move",5);
  await vi.waitFor(()=>expect(f.displayed.at(-1)).toEqual({authority:"provisional",x:5}));
  f.displayed.length=0;
  await f.pointer("move",10);await f.pointer("up",12);
  await vi.waitFor(()=>expect(f.requests).toContain("commands"));
  // Presence and other canvas-only frames still originate in accepted picking.
  f.record(await f.adapter.dispatch({version:2,command:"dimensions.hover.clear"}));
  f.view.state={viewport:{zoom:20}};
  f.record(await f.adapter.wheel({version:2,x:50,y:50,deltaX:0,deltaY:-20,ctrl:false}));
  await vi.waitFor(()=>expect(f.worker.render).toHaveBeenCalledWith(expect.objectContaining({state:f.view.state})));
  const pan={version:2 as const,pointerId:2,x:50,y:50,buttons:4,modifiers:{alt:false,ctrl:false,meta:false,shift:false}};
  await f.adapter.pointer({...pan,phase:"down"});
  f.view.state={viewport:{zoom:20,center:[2,1]}};
  f.record(await f.adapter.pointer({...pan,phase:"move",x:70,y:60}));
  await vi.waitFor(()=>expect(f.worker.render).toHaveBeenCalledWith(expect.objectContaining({state:f.view.state})));
  f.record(await f.adapter.pointer({...pan,phase:"up",x:70,y:60,buttons:0}));
  await new Promise(resolve=>setTimeout(resolve,125));
  expect(f.displayed.length).toBeGreaterThan(2);
  expect(f.displayed.every(frame=>frame.authority==="provisional"&&frame.x>=5)).toBe(true);
  expect(f.displayed.at(-1)).toEqual({authority:"provisional",x:12});
  const before=f.displayed.length,requestId=f.adapter.pending[0]!.requestId;
  Object.assign(f.document.accepted,{modelRevision:1});f.view.frame=f.frame(12,"accepted");f.snapshot.frame=f.view.frame;
  f.held.resolve(f.json({receipt:{operation:{userId:"alice",clientId:"tab",requestId},admission:1,outcome:{status:"accepted",revision:1,accepted_input:"new",summary:"Moved point"}}}));
  await vi.waitFor(()=>expect(f.displayed.at(-1)).toEqual({authority:"accepted",x:12}));
  expect(f.displayed.slice(before).every(frame=>frame.x===12)).toBe(true);
});

it("restores the complete accepted frame on cancellation and on a rejected shared drag",async()=>{
  for(const outcome of ["cancel","reject"]){
    const f=await dragFixture();await f.pointer("down",0);await f.pointer("move",6);
    await vi.waitFor(()=>expect(f.displayed.at(-1)).toEqual({authority:"provisional",x:6}));
    if(outcome==="cancel")f.record(await f.adapter.cancel({version:2,reason:"escape"}));
    else{
      await f.pointer("up",8);await vi.waitFor(()=>expect(f.requests).toContain("commands"));
      const requestId=f.adapter.pending[0]!.requestId;
      f.held.resolve(f.json({receipt:{operation:{userId:"alice",clientId:"tab",requestId},admission:1,outcome:{status:"rejected",code:"fixture",message:"Explicit branch rejected"}}}));
    }
    await vi.waitFor(()=>expect(f.displayed.at(-1)).toEqual({authority:"accepted",x:0}));
    expect((await f.adapter.snapshot()).frame.scene.items[0]!.interactive).toBe(true);
    if(outcome==="reject")await vi.waitFor(()=>expect(f.adapter.notice).toContain("Explicit branch rejected"));
  }
});

it("discards a superseded browsing request without surfacing its worker replacement notice",async()=>{
  const mock=await new MockWorkbenchAdapter().snapshot(),entered=deferred<void>();
  let reject!:(error:Error)=>void,count=0;
  let model={documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"};
  const chrome={explorer:mock.explorer,navigation:mock.navigation,dimensions:mock.dimensions,parameters:mock.parameters,problems:mock.problems,selection:null,selectedGeometryRole:null,authoringDocument:null};
  const browsing:LocalBrowsingClient={
    replace:vi.fn(async next=>{model=next;if(next.revision>0)reject(Error("Browsing model was replaced"));return {kind:"ready" as const,model};}),
    present:vi.fn(async view=>{if(++count===1){entered.resolve();await new Promise<void>((_,no)=>{reject=no;});}return {kind:"chrome" as const,model,view,chrome};}),
    navigate:vi.fn(),describe:vi.fn(),dispose:vi.fn(),
  };
  const f=await fixture({browsing});await entered.promise;
  Object.assign(f.document.accepted,{modelRevision:1});
  const notices:string[]=[];f.adapter.subscribe(()=>notices.push(f.adapter.notice));
  await f.adapter.refresh();await vi.waitFor(()=>expect(browsing.present).toHaveBeenCalledTimes(2));
  expect(notices.some(notice=>notice.includes("Browsing model was replaced"))).toBe(false);
  expect(f.adapter.notice).toBe("Shared document is up to date");
});

it("retires a held prediction when a peer replaces its accepted model",async()=>{
  const f=await dragFixture(),held=deferred<AuthoringPreview>();
  const old={kind:"preview" as const,model:vi.mocked(f.worker.replace).mock.calls[0]![0],view:{seed:{},state:{}},frame:f.frame(8,"provisional")};
  f.worker.beginPoint=vi.fn(()=>held.promise);
  await f.pointer("down",0);await f.pointer("move",8);
  await vi.waitFor(()=>expect(f.worker.beginPoint).toHaveBeenCalledOnce());
  Object.assign(f.document.accepted,{modelRevision:1});f.view.frame=f.frame(30,"accepted");f.snapshot.frame=f.view.frame;
  f.record(await f.adapter.refresh());const replaced=f.displayed.length;
  held.resolve(old);await f.pointer("up",10);await Promise.resolve();
  expect(f.displayed.at(-1)).toEqual({authority:"accepted",x:30});
  expect(f.displayed.slice(replaced).every(frame=>frame.authority==="accepted"&&frame.x===30)).toBe(true);
  expect(f.requests).not.toContain("commands");
});

it("reprojects the last native candidate locally while subsequent authoring compute is held",async()=>{
  const f=await dragFixture(),held=deferred<AuthoringPreview>();
  f.local.projectPrediction=vi.fn(async input=>({...f.view,state:input.view.state,frame:f.frame(JSON.parse(input.presentation).opaqueCandidate,"provisional")}));
  await f.pointer("down",0);await f.pointer("move",5);
  await vi.waitFor(()=>expect(f.displayed.at(-1)).toEqual({authority:"provisional",x:5}));
  const advance=f.worker.advancePoint;f.worker.advancePoint=vi.fn(()=>held.promise);
  await f.pointer("move",9);await vi.waitFor(()=>expect(f.worker.advancePoint).toHaveBeenCalledOnce());
  f.view.state={viewport:{zoom:30}};
  f.record(await f.adapter.wheel({version:2,x:50,y:50,deltaX:0,deltaY:-30,ctrl:false}));
  expect(f.local.projectPrediction).toHaveBeenCalledWith(expect.objectContaining({presentation:JSON.stringify({opaqueCandidate:5}),view:expect.objectContaining({state:f.view.state})}));
  expect(f.worker.render).not.toHaveBeenCalled();
  expect(f.displayed.at(-1)).toEqual({authority:"provisional",x:5});
  // Explicit cancellation owns the display even if held computation returns later.
  f.record(await f.adapter.cancel({version:2,reason:"escape"}));
  held.resolve(await advance({sequence:2,position:[9,0]},{seed:{},state:{}}));
  await Promise.resolve();
  expect(f.displayed.at(-1)).toEqual({authority:"accepted",x:0});
  expect(f.requests).not.toContain("commands");
});

it("installs simultaneous refreshes of the same accepted scene exactly once",async()=>{
  const f=await dragFixture(),entered=deferred<void>(),replacement=deferred<void>();Object.assign(f.document.accepted,{modelRevision:1});
  f.local.replace=vi.fn(async()=>{entered.resolve();await replacement.promise;return f.view;});
  const first=f.adapter.refresh();await entered.promise;const second=f.adapter.refresh();
  await vi.waitFor(()=>expect(f.requests.filter(route=>route==="scene")).toHaveLength(3));
  replacement.resolve();await Promise.all([first,second]);
  expect(f.worker.replace).toHaveBeenCalledTimes(2); // Initial model, then revision 1.
  expect(f.local.replace).toHaveBeenCalledOnce();
});

it("starts native authoring reconstruction before shared text and scene loading, without delaying navigation",async()=>{
  const ready=deferred<Awaited<ReturnType<LocalAuthoringClient["replace"]>>>();
  const authoring:LocalAuthoringClient={replace:vi.fn(()=>ready.promise),beginPoint:vi.fn(),advancePoint:vi.fn(),finishPoint:vi.fn(),beginConstruction:vi.fn(),advanceConstruction:vi.fn(),finishConstruction:vi.fn(),beginOperation:vi.fn(),advanceOperation:vi.fn(),pickOperationSelection:vi.fn(),finishOperation:vi.fn(),render:vi.fn(),cancel:vi.fn(),dispose:vi.fn()};
  const f=await fixture({authoring,onTextOpen:()=>expect(authoring.replace).toHaveBeenCalledOnce()});
  expect((await f.adapter.wheel({version:2,x:50,y:50,deltaX:0,deltaY:-30,ctrl:false}))?.frame.ariaLabel).toBe("local 1");
  expect(authoring.replace).toHaveBeenCalledOnce();
  ready.resolve({kind:"ready",model:vi.mocked(authoring.replace).mock.calls[0]![0]});
});

it("exposes the full native catalog and routes selected-role mutation through native operation prediction",async()=>{
  const f=await dragFixture();
  expect((await f.adapter.toolCatalog()).sections.flatMap(section=>section.commands)).toHaveLength(45);
  expect((await f.adapter.toolCatalog()).sections.flatMap(section=>section.commands).every(command=>command.unavailableReason===undefined)).toBe(true);
  const base=(await f.adapter.snapshot()).frame;
  const operation={sequence:0,completed:true,can_finish:false,has_pending:false,can_reset:false,can_step_back:false,diagnostic:null,pending:[],authoring_options:{tangent_orientation:"aligned" as const,curvature_relation:"signed" as const,continuity:{kind:"g1" as const},dimension_mode:"driving" as const,angle_orientation:"counter_clockwise" as const},fillet_options:{fillet_radius:2,flip_first_side:false,flip_second_side:false,alternate_arc:false},fillet_corner_count:0,fillet_corners:[],offset_distance:null};
  let captured!:Parameters<LocalAuthoringClient["beginOperation"]>[0];
  f.worker.beginOperation=vi.fn<LocalAuthoringClient["beginOperation"]>(async input=>{captured=input;return {kind:"preview",model:{documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"},view:input.view,frame:{...base,scene:{...base.scene,provenance:{...base.scene.provenance,scene:"provisional"}}},operation};});
  const gesture={basis:"digest",gesture_id:1,viewport:f.viewport,tool:"toggle_geometry_role" as const,selection:[],samples:[],expected_declarations:[]};
  f.worker.finishOperation=vi.fn<LocalAuthoringClient["finishOperation"]>(async()=>({kind:"operation",model:{documentEpoch:"epoch",revision:0,sourceDesignDigest:"digest"},command:gesture}));
  await f.adapter.dispatch({version:2,command:"geometry.role.toggle"});
  await vi.waitFor(()=>expect(f.requests).toContain("commands"));
  expect(captured.tool).toBe("toggle_geometry_role");expect(captured.selection).toBeUndefined();
  expect(f.adapter.pending[0]).toMatchObject({body:{command:{kind:"semantic",payload:{action:"tool_operation",gesture}}}});
  expect((await f.adapter.snapshot()).presentation.activeTool).toBe("select");
  const requestId=f.adapter.pending[0]!.requestId;
  f.held.resolve(f.json({receipt:{operation:{userId:"alice",clientId:"tab",requestId},admission:1,outcome:{status:"rejected",code:"fixture",message:"Completed role route witness"}}}));
});


it("exposes pending shared text independently of model saving and encodes recovery only on demand", async () => {
  const f = await fixture();
  const session = createCollaborativeWorkbenchSession(f.adapter);
  const checkpoint = vi.spyOn(f.adapter.client, "checkpoint");
  const persistence = vi.spyOn(f.adapter, "persistProject");
  const released = deferred<TextWorkerUpdate>();
  f.text.edit = async () => released.promise;
  const file = (await session.adapter.snapshot()).source.files[0]!;
  const edit = session.sharedText!.editSource(file.path, {
    displayId: file.sharedRevision, before: file.contents, after: file.contents + "a",
    changes: [{ from: file.contents.length, to: file.contents.length, insert: "a" }],
  });
  await vi.waitFor(() => expect(session.sharedText?.pendingCount).toBe(1));
  expect(session.persistence.automatic).toBe(false);
  expect(session.persistDraft).toBeUndefined();
  const banner = session.banner!;
  expect(banner.region).toBe("Shared document");
  expect(checkpoint).not.toHaveBeenCalled();
  const download = JSON.parse(banner.downloads![0]!.contents());
  expect(download.sourceEdits[0].edit.after).toBe(file.contents + "a");
  expect(checkpoint).toHaveBeenCalledOnce();
  expect(persistence).not.toHaveBeenCalled();
  released.resolve({ ...f.textSnapshot, localRevision: { heads: ["local"] } });
  await edit;
});
