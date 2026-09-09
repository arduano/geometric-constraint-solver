// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, renameSync, rmSync, cpSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { chromium, expect } from "../crates/geosolve-demo-web/frontend/node_modules/@playwright/test/index.mjs";
import { initProject, serveProject, bakeProject, hash } from "./file-workspace.mjs";

const evidence=resolve(process.env.GEOSOLVE_BROWSER_EVIDENCE ?? "target/m98/browser");
mkdirSync(evidence,{recursive:true});
const executablePath=process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome";
const manifest=(mode)=>JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode});
async function setup(t,prepare,options={}) {
  const folder=mkdtempSync(resolve(tmpdir(),"geosolve-m98-browser-"));
  initProject(folder);writeFileSync(resolve(folder,"geosolve.json"),manifest("editable"));
  prepare?.(folder);
  let bridge=await serveProject(folder);
  const browser=await chromium.launch({executablePath,args:["--disable-dev-shm-usage"]});
  t.after(async()=>{await browser.close();await bridge.close();rmSync(folder,{recursive:true,force:true});});
  const context=await browser.newContext({viewport:{width:1600,height:1000}});
  if(options.initScript) await context.addInitScript(options.initScript);
  const page=await context.newPage();
  const errors=[];page.on("pageerror",e=>errors.push(String(e)));
  await page.goto(bridge.url);
  await expect(page.locator('canvas[data-renderer="webgl2"]')).toHaveAttribute("data-render-state","ready");
  return {folder,get bridge(){return bridge;},browser,context,page,errors,
    async restart(prepare){await bridge.close();prepare?.(folder);bridge=await serveProject(folder);await page.goto(bridge.url);await expect(page.locator('canvas[data-renderer="webgl2"]')).toHaveAttribute("data-render-state","ready");}};
}

const sourcePath=(folder)=>resolve(folder,"sketch.ts");
const readSource=(folder)=>readFileSync(sourcePath(folder),"utf8");
function replaceSource(folder,contents){writeFileSync(sourcePath(folder)+".external",contents);renameSync(sourcePath(folder)+".external",sourcePath(folder));}
const folderNotice=(page)=>page.getByRole("region",{name:"Local folder"});
const radiusField=(page)=>page.getByRole("textbox",{name:"ringRadius · value",exact:true});
const geometry=(page)=>page.locator('canvas[data-renderer="webgl2"]').evaluate((element)=>Reflect.get(element,"__geosolvePresentedFrame").items
  .filter((item)=>item.layer==="geometry").map(({id,kind,points,center,radius})=>({id,kind,points,center,radius})));
const status=async(fixture)=>{
  const response=await fetch(`${fixture.bridge.origin}/api/status`,{headers:{Authorization:`Bearer ${fixture.bridge.token}`}});
  assert.equal(response.status,200);return response.json();
};
function curveWidth(items){const points=items.find((item)=>item.kind==="polyline"&&item.points.length>12)?.points;assert.ok(points,"real presented circle has sampled points");return Math.max(...points.map(([x])=>x))-Math.min(...points.map(([x])=>x));}

test("external rename updates the open canvas once, invalid text retains it, and browser storage remains separate",{timeout:120000},async(t)=>{
  const f=await setup(t,undefined,{initScript:()=>{
    localStorage.setItem("geosolve.project.v1","unrelated browser project");
    localStorage.setItem("geosolve.source-draft.v1","unrelated browser draft");
  }});
  const {folder,page,errors}=f;
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await expect(radiusField(page)).toHaveValue("10");
  assert.deepEqual(await page.evaluate(()=>[localStorage.getItem("geosolve.project.v1"),localStorage.getItem("geosolve.source-draft.v1")]),["unrelated browser project","unrelated browser draft"]);
  await page.evaluate(()=>{window.m98PageIdentity="same open page";});
  const before=await geometry(page),initial=await status(f),start=Date.now();
  const external=readSource(folder).replace("value: mm(10)","value: mm(12)");
  replaceSource(folder,external);
  await expect(radiusField(page)).toHaveValue("12");
  const elapsed=Date.now()-start,after=await geometry(page);
  assert.notDeepEqual(after,before);
  assert.ok(Math.abs(curveWidth(after)/curveWidth(before)-1.2)<0.01,"external radius edit preserves the camera scale");
  assert.equal(await page.evaluate(()=>window.m98PageIdentity),"same open page");
  const settled=await status(f);
  assert.equal(settled.externalApplies,initial.externalApplies+1);
  assert.equal(readSource(folder),external,"only the semantic sidecar may be published for external source");
  await page.waitForTimeout(450);
  const quiet=await status(f);
  assert.equal(quiet.externalApplies,settled.externalApplies,"watcher does not reapply its own publication");
  assert.equal(quiet.writes,settled.writes,"watcher does not create a repeated sidecar write");

  await radiusField(page).fill("14");await radiusField(page).press("Enter");
  await expect.poll(()=>readSource(folder)).toContain("value: mm(14)");
  await expect(folderNotice(page)).toContainText("Saved to disk");
  const edited=await status(f);
  assert.equal(edited.writes,quiet.writes+1);
  assert.equal(edited.externalApplies,quiet.externalApplies);
  await page.waitForTimeout(450);
  assert.equal((await status(f)).writes,edited.writes);
  assert.equal((await status(f)).externalApplies,edited.externalApplies);
  await page.getByRole("button",{name:"split",exact:true}).click();
  await expect(page.locator(".cm-content")).toContainText("value: mm(14)");
  const good=readSource(folder),accepted=await geometry(page);
  const invalid=good.replace("value: mm(14)","value: mm(");
  replaceSource(folder,invalid);
  await expect(folderNotice(page)).toContainText("Last accepted geometry retained");
  const rejected=await status(f);
  writeFileSync(resolve(evidence,"invalid-folder-state.json"),JSON.stringify({source:invalid,acceptedState:edited,rejected,acceptedGeometry:accepted,visibleGeometry:await geometry(page)},null,2));
  assert.equal(readSource(folder),invalid);assert.deepEqual(await geometry(page),accepted);
  assert.equal(rejected.ok,false);assert.notEqual(rejected.currentHash,rejected.acceptedHash);
  assert.ok(rejected.diagnostics.some((item)=>(item.file??item.path)==="sketch.ts"&&item.line>=1),JSON.stringify(rejected.diagnostics));
  await page.screenshot({path:resolve(evidence,"invalid-folder-retained.png")});
  replaceSource(folder,good);await expect(folderNotice(page)).toContainText("Saved to disk");
  await f.restart();
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await expect(radiusField(page)).toHaveValue("14");
  assert.equal(readSource(folder),good);assert.equal((await status(f)).ok,true);
  assert.deepEqual(await page.evaluate(()=>[localStorage.getItem("geosolve.project.v1"),localStorage.getItem("geosolve.source-draft.v1")]),["unrelated browser project","unrelated browser draft"]);
  writeFileSync(resolve(evidence,"folder-save-latency.json"),JSON.stringify({externalSaveToInspectorMs:elapsed},null,2));
  assert.deepEqual(errors,[]);
});

test("a stale code draft remains downloadable and canonical folder export follows accepted disk after Revert",{timeout:120000},async(t)=>{
  const f=await setup(t),{folder,page,errors}=f;
  await page.getByRole("button",{name:"split",exact:true}).click();
  const source=readSource(folder),draft=source.replace("value: mm(10)","value: mm(99)"),external=source.replace("value: mm(10)","value: mm(18)");
  await page.locator(".cm-content").fill(draft);
  replaceSource(folder,external);
  await expect.poll(async()=>(await status(f)).ok).toBe(true);
  await expect.poll(async()=>(await status(f)).sourceHash).toBe(hash(external));
  await page.getByRole("button",{name:"Apply",exact:true}).click();
  await expect(folderNotice(page)).toContainText("Conflict");
  await expect(page.locator(".cm-content")).toContainText("value: mm(99)");
  assert.equal(readSource(folder),external);
  const event=page.waitForEvent("download");await page.getByRole("button",{name:"Download pending intent"}).click();
  const pendingPath=resolve(evidence,"folder-pending-intent.json");await(await event).saveAs(pendingPath);
  const pending=JSON.parse(readFileSync(pendingPath,"utf8"));
  assert.equal(pending.method,"dispatch");assert.equal(pending.input.command,"source.prepare");
  assert.equal(pending.input.payload.contents,draft);assert.ok(pending.operationId);assert.ok(pending.authority);
  await page.getByRole("button",{name:"Revert",exact:true}).click();
  await page.getByRole("button",{name:"Refresh from disk"}).click();
  await expect(page.locator(".cm-content")).toContainText("value: mm(18)");
  await page.getByRole("button",{name:"File menu"}).click();
  const exportEvent=page.waitForEvent("download");await page.getByRole("menuitem",{name:"Export canonical project…"}).click();
  const exportPath=resolve(evidence,"folder-canonical-export.json");await(await exportEvent).saveAs(exportPath);
  const exported=JSON.parse(readFileSync(exportPath,"utf8"));
  assert.match(JSON.stringify(exported),/ringRadius/);
  assert.ok(JSON.stringify(exported).includes("value: mm(18)"),"canonical export contains the accepted external radius");
  assert.equal(readSource(folder),external);assert.deepEqual(errors,[]);
});

test("the same folder server opens the ordinary demo without a token or folder authority",{timeout:90000},async(t)=>{
  const f=await setup(t),before=readSource(f.folder);
  const page=await f.browser.newPage({viewport:{width:1400,height:900}});
  const errors=[];page.on("pageerror",(error)=>errors.push(String(error)));
  await page.goto(f.bridge.origin);
  await expect(page.locator('canvas[data-renderer="webgl2"]')).toHaveAttribute("data-render-state","ready");
  await expect(folderNotice(page)).toHaveCount(0);
  await page.getByRole("button",{name:"File menu"}).click();
  await page.getByRole("menuitem",{name:/Open/}).click();
  await expect(page.getByPlaceholder(/Search \d+ samples/)).toBeVisible();
  await page.getByRole("button",{name:"Start from code",exact:true}).click();
  await expect(page.locator(".cm-content")).toContainText("use geosolve sketch");
  assert.equal(readSource(f.folder),before);assert.deepEqual(errors,[]);
  await page.screenshot({path:resolve(evidence,"ordinary-demo-folder-server.png")});
});

test("legacy single-file cold reopen retains its last accepted canvas while incomplete disk text stays untouched",{timeout:90000},async(t)=>{
  const f=await setup(t,(folder)=>writeFileSync(resolve(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v1",entry:"sketch.ts"})));
  const {page,folder,errors}=f;
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await radiusField(page).fill("18");await radiusField(page).press("Enter");
  await expect.poll(()=>readSource(folder)).toContain("value: mm(18)");
  const good=readSource(folder),invalid=good.replace("value: mm(18)","value: mm(");
  await f.restart(()=>replaceSource(folder,invalid));
  await expect(folderNotice(page)).toContainText("Last accepted geometry retained");
  const invalidState=await status(f);
  assert.equal(invalidState.ok,false);assert.equal(invalidState.acceptedHash,hash(good));
  assert.equal(invalidState.currentHash,hash(invalid));assert.equal(readSource(folder),invalid);
  assert.ok((await geometry(page)).some((item)=>item.kind==="polyline"&&item.points.length>12));
  const snapshot=await f.bridge.project.adapter.snapshot();
  assert.ok(snapshot.dimensions.allMeasurements.some((entry)=>entry.label==="ringRadius"&&Number(entry.value)===18),JSON.stringify(snapshot.dimensions));
  await assert.rejects(f.bridge.project.adapter.exportProject(),/canonical export.*invalid/);
  await page.screenshot({path:resolve(evidence,"legacy-invalid-cold-reopen.png")});
  replaceSource(folder,good);await expect(folderNotice(page)).toContainText("Saved to disk");
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await expect(radiusField(page)).toHaveValue("18");
  assert.equal(readSource(folder),good);assert.deepEqual(errors,[]);
});

test("folder UI writes source, preserves stale field text, and offers explicit editor handoff",{timeout:120000},async(t)=>{
  const {folder,bridge,context,page,errors}=await setup(t);
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  const field=page.getByRole("textbox",{name:"ringRadius · value",exact:true});
  await expect(field).toHaveValue("10");
  await field.fill("14");await field.press("Enter");
  await expect.poll(()=>readFileSync(resolve(folder,"sketch.ts"),"utf8")).toContain("value: mm(14)");
  await expect(page.getByRole("region",{name:"Local folder"})).toContainText("Saved to disk");
  const saved=readFileSync(resolve(folder,"sketch.ts"),"utf8");
  await field.fill("77");
  writeFileSync(resolve(folder,"sketch.ts"),saved.replace("value: mm(14)","value: mm(15)"));
  await expect(page.getByRole("region",{name:"Local folder"})).toContainText("Disk changed");
  await expect(field).toHaveValue("77");
  await field.press("Enter");
  await expect(page.getByRole("button",{name:"Download pending intent"})).toBeVisible();
  assert.match(readFileSync(resolve(folder,"sketch.ts"),"utf8"),/value: mm\(15\)/);
  await field.press("Escape");await page.getByRole("button",{name:"Refresh from disk"}).click();
  await expect(field).toHaveValue("15");
  const second=await context.newPage();await second.goto(bridge.url);
  await expect(second.getByRole("button",{name:"Take over editing"})).toBeVisible();
  await second.getByRole("button",{name:"Take over editing"}).click();
  await expect(page.getByRole("button",{name:"Take over editing"})).toBeVisible();
  await page.screenshot({path:resolve(evidence,"folder-handoff.png")});
  assert.deepEqual(errors,[]);
});

test("generator workbench shows code-owned inputs and read-only source, and updates accepted geometry",{timeout:120000},async(t)=>{
  const source='import {defineGenerator,sketch,mm} from "@geosolve/sketch-code";export default defineGenerator({radius:{type:"number",default:6,min:1,max:12,label:"Port radius",unit:"mm",description:"Width of the water port."}},({radius})=>sketch(($)=>({port:$.geometry.centerRadiusCircle("port",{center:[0,0],radius:mm(radius)})})));';
  const {folder,page,errors}=await setup(t,(folder)=>{writeFileSync(resolve(folder,"geosolve.json"),manifest("generator"));writeFileSync(resolve(folder,"sketch.ts"),source);});
  const form=page.getByRole("region",{name:"Generator inputs"});
  await expect(form).toBeVisible();
  const input=form.getByRole("textbox",{name:"Port radius",exact:true});
  await expect(input).toHaveValue("6");
  await input.fill("8");await form.getByRole("button",{name:"Apply Port radius",exact:true}).click();
  await expect.poll(()=>existsSync(resolve(folder,".geosolve/inputs.json")) ? JSON.parse(readFileSync(resolve(folder,".geosolve/inputs.json"),"utf8")).radius : null).toBe(8);
  await page.getByRole("button",{name:"split",exact:true}).click();
  await expect(page.locator(".cm-content")).toHaveAttribute("aria-readonly","true");
  const codeBefore=await page.locator(".cm-content").innerText();
  await page.locator(".cm-content").click();await page.keyboard.press("Control+Home");await page.keyboard.type("forbidden edit");
  assert.equal(await page.locator(".cm-content").innerText(),codeBefore);
  await expect(page.locator(".cm-content")).toContainText("defineGenerator");
  await expect(page.getByRole("button",{name:"Undo",exact:true})).toBeDisabled();
  assert.equal(readFileSync(resolve(folder,"sketch.ts"),"utf8"),source);
  await page.screenshot({path:resolve(evidence,"generator-workbench.png")});
  assert.deepEqual(errors,[]);
});

test("complete manifold opens from plain files and shared channel edits export all regions",{timeout:180000},async(t)=>{
  const {folder,page,errors}=await setup(t,(folder)=>cpSync(resolve("examples/file-workspace-manifold"),folder,{recursive:true}));
  await expect(page.getByRole("region",{name:"Local folder"})).toContainText("Saved to disk");
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await page.screenshot({path:resolve(evidence,"manifold-folder.png")});
  // Native owning tests certify analytic geometry; this adapter test checks real
  // source/parameter visibility and complete export after a revision-bound GUI edit.
  const input=page.getByRole("textbox",{name:"Channel width",exact:true}).first();
  await expect(input).toHaveValue("12");
  await input.fill("11");await input.press("Enter");
  await expect.poll(()=>readFileSync(resolve(folder,"sketch.ts"),"utf8"),{timeout:30000}).toContain('$.parameter("channelWidth", mm(11)');
  const output=resolve(evidence,"manifold-from-gui.json");
  const baked=await bakeProject(folder,output,0.02);
  assert.equal(baked.regions.length,18);
  assert.deepEqual(errors,[]);
});
