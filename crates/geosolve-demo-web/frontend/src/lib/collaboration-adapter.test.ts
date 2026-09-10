// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, expect, it, vi } from "vitest";
import { CollaborativeWorkbenchAdapter, type CollaborativeTextClient, type CollaborativeDocument } from "./collaboration-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
import type { LocalInteractionClient, LocalInteractionUpdate } from "./local-interaction-adapter";
import type { TextWorkerUpdate } from "./collaboration-text-adapter";
import type { WorkbenchSnapshot } from "./adapter";
import type { LocalBrowsingClient } from "./collaboration-browsing-adapter";

const handles:CollaborativeWorkbenchAdapter[]=[];
afterEach(()=>{for(const handle of handles.splice(0))handle.dispose();});
function deferred<T>(){let resolve!:(value:T)=>void;const promise=new Promise<T>(yes=>{resolve=yes;});return{promise,resolve};}
async function fixture(fixtureOptions:{textResponse?:()=>Promise<Response>;browsing?:LocalBrowsingClient;navigation?:WorkbenchSnapshot["navigation"]}={}){
  const mock=new MockWorkbenchAdapter(),snapshot=await mock.snapshot(),toolCatalog=await mock.toolCatalog();
  snapshot.authoringDocument={authority:"server-inspector",title:"Sketch",description:"",areKeyConstraintsByDefault:false,editable:true};
  if(fixtureOptions.navigation)snapshot.navigation=fixtureOptions.navigation;
  const textSnapshot:TextWorkerUpdate={snapshot:{revision:{heads:[]},files:{"sketch.ts":"const a = 1;"}},changes:[],checkpoint:[],history:{undo:0,redo:0}};
  const text:CollaborativeTextClient={open:async()=>textSnapshot,edit:async()=>textSnapshot,receive:async()=>textSnapshot,undo:async()=>textSnapshot,redo:async()=>textSnapshot,snapshot:async()=>textSnapshot,dispose:()=>{}};
  let localCount=0;
  const view:LocalInteractionUpdate={frame:snapshot.frame,state:{},selectionChanged:false,serverFrameCompatible:false};
  const local:LocalInteractionClient={construct:async()=>view,replace:async()=>view,update:async()=>({ ...view,frame:{...view.frame,ariaLabel:`local ${++localCount}`}}),state:async()=>({}),dispose:()=>{}};
  const requests:string[]=[],held=deferred<Response>();
  const document={accepted:{modelRevision:0,acceptedInput:"a",files:textSnapshot.snapshot.files},working:textSnapshot.snapshot,model:{project:"{}",design:{},sourceDesignDigest:"digest"},textActor:[1],textCheckpoint:[],targets:{},inventory:{objects:[],properties:[]},pointTargets:[]} as unknown as CollaborativeDocument;
  const json=(value:unknown,headers?:Record<string,string>)=>new Response(JSON.stringify(value),{headers:{"Content-Type":"application/json",...headers}});
  const fetch=vi.fn(async(url:URL|RequestInfo,options?:RequestInit)=>{
    const route=new URL(String(url)).pathname.split("/").at(-1)!;requests.push(route);
    if(route==="join")return json({connection:{protocol:1,documentId:"doc",documentEpoch:"epoch",userId:"alice",clientId:"tab",role:"editor"},token:"a".repeat(64)});
    if(route==="state")return json({authority:{acceptedRevision:0,acceptedInput:"a",latestSequence:0},document,participants:[],presence:[]});
    if(route==="scene")return json({snapshot,seed:{},toolCatalog},{"X-Geosolve-Revision":"0"});
    if(route==="events")return new Response(new ReadableStream({start(controller){options?.signal?.addEventListener("abort",()=>{try{controller.close();}catch{}});}}),{headers:{"Content-Type":"text/event-stream"}});
    if(route==="commands")return held.promise;
    if(route==="text")return fixtureOptions.textResponse?fixtureOptions.textResponse():json({sourceSequence:1});
    if(route==="text-state")return json({sourceSequence:1,workingRevision:document.working.revision,changes:[],history:document.textHistory});
    throw Error(`Unexpected ${route}`);
  });
  const adapter=new CollaborativeWorkbenchAdapter({baseUrl:"http://test/api/collaboration/",inviteToken:"invite",clientId:"tab",fetch:fetch as typeof globalThis.fetch},local,text,null,fixtureOptions.browsing??null);
  handles.push(adapter);await adapter.construct();
  return{adapter,requests,held,json,localCount:()=>localCount,snapshot,text,document,textSnapshot};
}

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
  // The server's initial chrome has a different native allocation namespace.
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
    present:async(view)=>({kind:"chrome",model:identity,view,chrome}),
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
