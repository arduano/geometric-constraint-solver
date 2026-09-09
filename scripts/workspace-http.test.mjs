// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { chmodSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { initProject, serveProject } from "./file-workspace.mjs";

async function setup(t,prepare) {
  const folder=mkdtempSync(resolve(tmpdir(),"geosolve-m98-http-"));
  initProject(folder);
  writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode:"editable"}));
  prepare?.(folder);
  const bridge=await serveProject(folder);
  t.after(async()=>{chmodSync(folder,0o700);await bridge.close();rmSync(folder,{recursive:true,force:true});});
  const rpc=async(method,input,state,operationId)=>{
    const response=await fetch(`${bridge.origin}/api/rpc`,{method:"POST",
      headers:{Authorization:`Bearer ${bridge.token}`,"Content-Type":"application/json"},
      body:JSON.stringify({method,input,clientId:"http-migration",authority:state?.authority,baseHash:state?.currentHash,operationId})});
    return {status:response.status,...await response.json()};
  };
  return {folder,bridge,rpc,read:()=>readFileSync(resolve(folder,"sketch.ts"),"utf8")};
}

async function eventStream(t,bridge) {
  const controller=new AbortController();
  const response=await fetch(`${bridge.origin}/api/events?token=${bridge.token}`,{signal:controller.signal});
  assert.equal(response.status,200);
  const events=[];
  const reader=response.body.getReader(),decoder=new TextDecoder();
  const reading=(async()=>{
    let pending="";
    for(;;) {
      const {done,value}=await reader.read();if(done)return;
      pending+=decoder.decode(value,{stream:true});
      let boundary;
      while((boundary=pending.indexOf("\n\n"))!==-1) {
        const message=pending.slice(0,boundary);pending=pending.slice(boundary+2);
        const lines=message.split("\n"),data=lines.find((line)=>line.startsWith("data: "))?.slice(6);
        if(data!==undefined)events.push({event:lines.find((line)=>line.startsWith("event: "))?.slice(7)??"message",data});
      }
    }
  })().catch((error)=>{if(!controller.signal.aborted)throw error;});
  t.after(async()=>{controller.abort();await reading;});
  return events;
}

async function waitFor(predicate,label) {
  const deadline=Date.now()+10000;
  while(!predicate()) {assert.ok(Date.now()<deadline,label);await delay(10);}
}

test("SSE reports background evaluation, reconnects while busy, and clears after rejection without idle scan chatter",{timeout:30000},async(t)=>{
  const source='import {sketch,mm} from "@geosolve/sketch-code";export default sketch(($)=>({port:$.geometry.centerRadiusCircle("port",{center:[0,0],radius:mm(6)})}));';
  const f=await setup(t,(folder)=>{
    writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode:"generator"}));
    writeFileSync(resolve(folder,"sketch.ts"),source);
  });
  const events=await eventStream(t,f.bridge);
  await waitFor(()=>events.some((event)=>event.event==="message"),"initial state event");
  assert.deepEqual(events.filter((event)=>event.event==="activity"),[{event:"activity",data:'{"busy":false}'}]);
  await delay(450);
  assert.equal(events.length,2,"unchanged watcher scans do not announce activity");
  const before=f.bridge.project.state();
  const pause='const until=Date.now()+1400;while(Date.now()<until){};';
  writeFileSync(resolve(f.folder,"sketch.ts"),pause+source.replace("mm(6)","mm(8)"));
  await waitFor(()=>events.some((event)=>event.event==="activity"&&JSON.parse(event.data).busy),"background evaluation announces busy");
  assert.equal(f.bridge.project.state().acceptedHash,before.acceptedHash,"old geometry remains accepted during evaluation");
  const reconnected=await eventStream(t,f.bridge);
  await waitFor(()=>reconnected.length>=2,"reconnected stream current state");
  assert.deepEqual(reconnected[0],{event:"activity",data:'{"busy":true}'});
  await waitFor(()=>f.bridge.project.state().acceptedHash!==before.acceptedHash,"external generator accepted");
  await waitFor(()=>events.at(-1)?.data==='{"busy":false}',"evaluation ends idle");
  const accepted=f.bridge.project.state(),start=events.length;
  writeFileSync(resolve(f.folder,"sketch.ts"),pause+'throw Error("deliberately rejected generator");'+source);
  await waitFor(()=>events.slice(start).some((event)=>event.event==="activity"&&JSON.parse(event.data).busy),"rejected evaluation announces busy");
  await waitFor(()=>f.bridge.project.state().status==="error","rejection recorded");
  await waitFor(()=>events.at(-1)?.data==='{"busy":false}',"rejection ends idle");
  assert.equal(f.bridge.project.state().acceptedHash,accepted.acceptedHash);
  assert.equal(events.filter((event)=>event.event==="message").length,3,"activity does not masquerade as a snapshot invalidation");
  assert.deepEqual(events.filter((event)=>event.event==="activity").map((event)=>JSON.parse(event.data).busy),[false,true,false,true,false]);
});

test("M98-F015 rejected external generator retains the measured, zoomed and panned accepted canvas",{timeout:30000},async(t)=>{
  t.mock.timers.enable({apis:["setInterval"]});
  const source='import {sketch,mm} from "@geosolve/sketch-code";export default sketch(($)=>({port:$.geometry.centerRadiusCircle("port",{center:[10,20],radius:mm(6)})}));';
  const f=await setup(t,(folder)=>{
    writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode:"generator"}));
    writeFileSync(resolve(folder,"sketch.ts"),source);
  });
  const adapter=f.bridge.project.adapter;
  await adapter.resize({version:2,width:969,height:876,pixelRatio:2});
  await adapter.wheel({version:2,x:127,y:283,deltaX:0,deltaY:-160,ctrl:false});
  for(const [phase,x,y,buttons] of [["down",480,320,4],["move",540,345,4],["up",540,345,0]]) {
    await adapter.pointer({version:2,phase,pointerId:17,x,y,buttons,modifiers:{alt:false,ctrl:false,meta:false,shift:false}});
  }
  const before=await adapter.snapshot(),state=f.bridge.project.state(),persisted=await adapter.persistProject();
  const rejected='throw Error("external rejection retains host view");'+source;
  writeFileSync(resolve(f.folder,"sketch.ts"),rejected);
  await f.bridge.project.scan(true);
  const after=await adapter.snapshot();
  assert.equal(f.bridge.project.state().status,"error");
  assert.equal(f.bridge.project.state().acceptedHash,state.acceptedHash);
  assert.equal(f.read(),rejected);
  assert.deepEqual(after.frame.scene.viewBox,before.frame.scene.viewBox);
  const geometry=(snapshot)=>snapshot.frame.scene.items.filter((item)=>item.layer==="geometry");
  assert.deepEqual(geometry(after),geometry(before));
  assert.deepEqual(await adapter.persistProject(),persisted);
  await adapter.resize({version:2,width:800,height:600,pixelRatio:1});
  await adapter.resize({version:2,width:969,height:876,pixelRatio:2});
  assert.deepEqual(geometry(await adapter.snapshot()),geometry(before),"subsequent resize retains the already-measured camera instead of fitting again");
});

test("init refuses existing authored files without replacing source or manifest",()=>{
  const folder=mkdtempSync(resolve(tmpdir(),"geosolve-m98-init-"));
  try {
    initProject(folder);
    writeFileSync(resolve(folder,"sketch.ts"),"authored source that must survive");
    const source=readFileSync(resolve(folder,"sketch.ts")),manifest=readFileSync(resolve(folder,"geosolve.json"));
    assert.throws(()=>initProject(folder),/Refusing to overwrite/);
    assert.deepEqual(readFileSync(resolve(folder,"sketch.ts")),source);
    assert.deepEqual(readFileSync(resolve(folder,"geosolve.json")),manifest);
  } finally {rmSync(folder,{recursive:true,force:true});}
});

test("HTTP rejects missing or foreign tokens, hostile origins and unsupported write methods",async(t)=>{
  const f=await setup(t),before=f.read(),initial=await f.rpc("session.join");
  const design=readFileSync(resolve(f.folder,".geosolve/design.json"));
  for(const [path,options] of [
    ["/api/status",{}],
    ["/api/status",{headers:{Authorization:"Bearer foreign-token"}}],
    ["/api/status",{headers:{Authorization:`Bearer ${f.bridge.token}`,Origin:"https://example.com"}}],
    ["/api/rpc",{method:"POST",headers:{"Content-Type":"application/json"},body:"{}"}],
    ["/api/rpc",{method:"POST",headers:{Authorization:`Bearer ${f.bridge.token}`,Origin:"https://example.com","Content-Type":"application/json"},body:"{}"}],
  ]) {
    const response=await fetch(`${f.bridge.origin}${path}`,options);
    assert.equal(response.status,403,`${path}: ${await response.text()}`);
  }
  const unsupported=await f.rpc("writeFile",{path:"../outside.ts",contents:"overwrite"},initial.state,"forbidden-write");
  assert.equal(unsupported.status,400,unsupported.error);
  const internalRestore=await f.rpc("dispatch",{version:2,command:"workspace.checkpoint.restore",payload:{contents:(await f.bridge.project.adapter.persistProject()).contents}},initial.state,"forbidden-restore");
  assert.equal(internalRestore.status,400,internalRestore.error);
  assert.match(internalRestore.error,/Folder mode keeps/);
  assert.equal(f.read(),before);
  assert.deepEqual(readFileSync(resolve(f.folder,".geosolve/design.json")),design);
  assert.equal(f.bridge.project.state().writes,initial.state.writes);
});

test("an external rename before any watcher scan rejects a v2 edit with the old complete revision",async(t)=>{
  // Freeze the host's interval callbacks only. The native worker and HTTP loop
  // remain real; this makes the absence of a preceding watcher scan explicit.
  t.mock.timers.enable({apis:["setInterval"]});
  const f=await setup(t),initial=await f.rpc("session.join"),before=f.read();
  const external=before.replace("value: mm(10)","value: mm(16)");
  writeFileSync(resolve(f.folder,"sketch.ts.external"),external);
  renameSync(resolve(f.folder,"sketch.ts.external"),resolve(f.folder,"sketch.ts"));
  assert.equal(f.bridge.project.state().currentHash,initial.state.currentHash);
  assert.equal(f.bridge.project.state().externalApplies,initial.state.externalApplies);
  const response=await f.rpc("dispatch",{version:2,command:"source.prepare",payload:{path:"sketch.ts",contents:before.replace("value: mm(10)","value: mm(99)")}},initial.state,"stale-before-watch");
  assert.equal(response.status,409,response.error);
  assert.match(response.error,/Conflict/);
  assert.equal(f.read(),external);
  assert.notEqual(response.state.currentHash,initial.state.currentHash);
});

test("a real publication failure retains source, semantic design and recoverable pending source",async(t)=>{
  const f=await setup(t),initial=await f.rpc("session.join"),before=f.read();
  const design=readFileSync(resolve(f.folder,".geosolve/design.json"));
  await f.bridge.project.adapter.resize({version:2,width:969,height:876,pixelRatio:2});
  await f.bridge.project.adapter.wheel({version:2,x:127,y:283,deltaX:0,deltaY:-160,ctrl:false});
  const frame=(await f.bridge.project.adapter.snapshot()).frame.scene;
  const candidate=before.replace("value: mm(10)","value: mm(20)");
  chmodSync(f.folder,0o500);
  try {
    const response=await f.rpc("dispatch",{version:2,command:"source.prepare",payload:{path:"sketch.ts",contents:candidate}},initial.state,"publication-denied");
    assert.equal(response.status,400,response.error);
    assert.match(response.error,/EACCES/);
    assert.match(response.pendingSource,/value: mm\(20\)/);
    assert.equal(f.read(),before);
    assert.deepEqual(readFileSync(resolve(f.folder,".geosolve/design.json")),design);
    assert.equal(response.state.writes,initial.state.writes);
    const retained=(await f.bridge.project.adapter.snapshot()).frame.scene;
    assert.deepEqual(retained.viewBox,frame.viewBox);
    assert.deepEqual(retained.items.filter((item)=>item.layer==="geometry"),frame.items.filter((item)=>item.layer==="geometry"));
  } finally {chmodSync(f.folder,0o700);}
});
