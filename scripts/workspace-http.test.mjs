// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { chmodSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { initProject, serveProject } from "./file-workspace.mjs";

async function setup(t) {
  const folder=mkdtempSync(resolve(tmpdir(),"geosolve-m98-http-"));
  initProject(folder);
  writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode:"editable"}));
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
  } finally {chmodSync(f.folder,0o700);}
});
