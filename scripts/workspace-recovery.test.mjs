// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { initProject, openProject } from "../packages/geosolve-cli/runtime/file-workspace.mjs";
import { acquireWorkspaceLock, createWorkspaceStorage } from "../packages/geosolve-cli/runtime/workspace-storage.mjs";

for (const corrupt of [false, true]) test(`v2 invalid source opens with diagnostics and ${corrupt ? "refuses corrupt cached source" : "independently reconstructed previous geometry"}`, async (t) => {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-cold-recovery-"));
  initProject(folder);
  writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode:"editable"}));
  const lock = acquireWorkspaceLock(folder), storage = createWorkspaceStorage(folder,{lock});
  let project = await openProject(folder,{storage});
  t.after(async()=>{await project?.dispose();lock.release();rmSync(folder,{recursive:true,force:true});});
  const before = await project.adapter.bakeProfile(0.02);
  const acceptedHash = project.state().acceptedHash;
  await project.saveDerived();await project.dispose();project=null;
  const derivedPath=resolve(folder,".geosolve/derived-session.json");
  const cache=JSON.parse(readFileSync(derivedPath,"utf8"));
  if(corrupt){
    for(const [,snapshot] of cache.sources) for(const file of snapshot.files) if(file.path==="sketch.ts") file.contents=file.contents.replace("value: mm(10)","value: mm(30)");
    writeFileSync(derivedPath,JSON.stringify(cache));
  }
  const invalid="export default function broken( {";
  writeFileSync(resolve(folder,"sketch.ts"),invalid);
  project=await openProject(folder,{storage});
  const state=project.state();
  assert.equal(state.ok,false);assert.equal(state.currentHash,null);
  assert.ok(state.diagnostics.some((item)=>item.path==="sketch.ts"&&item.line>=1));
  assert.equal(readFileSync(resolve(folder,"sketch.ts"),"utf8"),invalid);
  if(corrupt) assert.equal(state.acceptedHash,null);
  else {
    assert.equal(state.acceptedHash,acceptedHash);
    assert.deepEqual((await project.adapter.bakeProfile(0.02)).regions,before.regions);
  }
});
