// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, writeFileSync, readFileSync, rmSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { serveProject, bakeProject } from "./file-workspace.mjs";

const source = `import {defineGenerator,sketch,mm} from "@geosolve/sketch-code";
export default defineGenerator({ count:{type:"integer",default:2,min:1,max:4,label:"Ports"}, radius:{type:"number",default:6,min:1,max:8,unit:"mm",label:"Port radius"} },
({count,radius})=>sketch(($)=>{const ports=[];for(let i=0;i<count;i++)ports.push($.geometry.centerRadiusCircle("port-"+i,{center:[i*25,0],radius:mm(radius)}));return {ports};}));`;

test("generator inputs are validated, journaled and restored while reverse editing is refused", async (t) => {
  const parent = mkdtempSync(resolve(tmpdir(), "geosolve-m98-generator-"));
  const folder = resolve(parent, "project");
  const {mkdirSync} = await import("node:fs");mkdirSync(folder);
  writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"ports.ts",mode:"generator"}));
  writeFileSync(resolve(folder,"ports.ts"),source);
  let bridge = await serveProject(folder);
  t.after(async()=>{await bridge.close();rmSync(parent,{recursive:true,force:true});});
  const rpc=async(method,input,state,operationId)=>{
    const response=await fetch(`${bridge.origin}/api/rpc`,{method:"POST",headers:{Authorization:`Bearer ${bridge.token}`,"Content-Type":"application/json"},body:JSON.stringify({method,input,baseHash:state?.currentHash,authority:state?.authority,clientId:"generator-editor",operationId})});
    return {status:response.status,...await response.json()};
  };
  const initial=await rpc("session.join");
  assert.equal(initial.state.ok,true,JSON.stringify(initial.state));
  assert.deepEqual(initial.state.inputs,{count:2,radius:6});
  assert.equal(initial.state.capabilities.geometryEditing,false);
  assert.equal(initial.state.sourceFiles[0].contents,source);
  const invalid=await rpc("inputs.set",{values:{count:1.5}},initial.state,"invalid-input");
  assert.equal(invalid.status,400);
  assert.equal(existsSync(resolve(folder,".geosolve/inputs.json")),false);
  const denied=await rpc("dispatch",{version:2,command:"history.undo"},invalid.state);
  assert.equal(denied.status,400);
  const edited=await rpc("inputs.set",{values:{count:3,radius:8}},invalid.state,"accepted-input");
  assert.equal(edited.status,200,edited.error);
  assert.deepEqual(JSON.parse(readFileSync(resolve(folder,".geosolve/inputs.json"),"utf8")),{count:3,radius:8});
  assert.equal(readFileSync(resolve(folder,"ports.ts"),"utf8"),source);
  const output=resolve(parent,"ports.json");
  await bakeProject(folder,output,0.02,"/ports/1");
  const exported=JSON.parse(readFileSync(output,"utf8"));
  assert.equal(exported.regions.length,1);
  assert.ok(exported.regions[0].outer.every(([x,y])=>Math.abs(Math.hypot(x-25,y)-8)<1e-7));
  assert.deepEqual(exported.project.inputs,{count:3,radius:8});
  await bridge.close();bridge=await serveProject(folder);
  const restarted=await rpc("session.join");
  assert.equal(restarted.state.ok,true,JSON.stringify(restarted.state));
  assert.deepEqual(restarted.state.inputs,{count:3,radius:8});
});
