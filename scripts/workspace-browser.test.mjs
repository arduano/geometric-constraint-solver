// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, renameSync, rmSync, cpSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { chromium, expect } from "../crates/geosolve-demo-web/frontend/node_modules/@playwright/test/index.mjs";
import { initProject, serveProject, bakeProject, hash } from "../packages/geosolve-cli/runtime/file-workspace.mjs";
import { nativeBrowsing } from "./workspace-native-test.mjs";

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
  const openingStarted=performance.now();
  await page.goto(bridge.url);
  await expect(page.locator('canvas[data-renderer="webgl2"]')).toHaveAttribute("data-render-state","ready",{timeout:options.readinessTimeout??5000});
  const openingMs=performance.now()-openingStarted;
  return {folder,get bridge(){return bridge;},browser,context,page,errors,openingMs,
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

// Observe the real worker boundaries, retaining native projection results rather
// than reproducing screen/model conversion or authoring equations in the test.
function captureLocalAuthoring() {
  window.m98LocalUpdates=[];window.m98PointerInputs=[];window.m98PointerTerminals=[];
  window.m98WorkerInputs=[];window.m98WorkerResults=[];
  const NativeWorker=window.Worker;
  window.Worker=class extends NativeWorker {
    constructor(...args){super(...args);this.requests=new Map();this.addEventListener("message",({data})=>{
      const request=this.requests.get(data?.id);this.requests.delete(data?.id);
      if(request)window.m98WorkerResults.push({request,result:structuredClone(data.result),error:data.error});
      if(data?.result?.frame&&data.result.state){
        window.m98LocalUpdates.push(structuredClone(data.result));
        if(request?.method==="pointer"&&request.input?.phase==="up")window.m98PointerTerminals.push({input:request.input,state:structuredClone(data.result.state)});
      }
    });}
    postMessage(message,...rest){this.requests.set(message.id,structuredClone(message));window.m98WorkerInputs.push(structuredClone(message));return super.postMessage(message,...rest);}
  };
  for(const phase of ["down","move","up"])window.addEventListener(`pointer${phase}`,(event)=>{
    const host=document.querySelector('[role="application"]');if(!host?.contains(event.target))return;
    const box=host.getBoundingClientRect();window.m98PointerInputs.push({version:2,phase,pointerId:event.pointerId,x:event.clientX-box.x,y:event.clientY-box.y,buttons:event.buttons,modifiers:{alt:event.altKey,ctrl:event.ctrlKey,meta:event.metaKey,shift:event.shiftKey}});
  },true);
}
function assertNoPersonalRpc(delivered) {
  assert.deepEqual(delivered.filter(({request})=>["pointer","wheel","wheelBatch","resize","interaction.sync"].includes(request.method)),[],"personal input and state never travel to the server");
}
async function assertPointTerminal(page,delivered,inputs) {
  const {workerInputs,workerResults}=await page.evaluate(()=>({workerInputs:window.m98WorkerInputs,workerResults:window.m98WorkerResults}));
  assertNoPersonalRpc(delivered);
  const commits=delivered.filter(({request})=>request.method==="authoring.commit");
  assert.equal(commits.length,1,"a complete point gesture publishes one native terminal");
  assert.equal(commits[0].request.input.kind,"point");
  const command=commits[0].request.input.command;
  const begins=workerInputs.filter(({method})=>method==="beginPoint");
  assert.equal(begins.length,1);
  assert.deepEqual(command.viewport,begins[0].viewport,"native replay uses the exact captured local camera");
  assert.deepEqual(command.target,begins[0].target);
  const down=inputs.find(({phase})=>phase==="down");assert.ok(down);
  const projections=workerResults.filter(({request})=>request.method==="authoringPointer");
  const projectionFor=(input)=>{
    const matches=projections.filter(({request})=>request.input.x===input.x&&request.input.y===input.y&&request.input.captured===(input.phase!=="down"));
    assert.ok(matches.length,`native projection exists for ${JSON.stringify(input)}`);
    return matches.at(-1).result;
  };
  assert.deepEqual(command.target,projectionFor(down).target);
  assert.deepEqual(command.viewport,projectionFor(down).viewport);
  assert.deepEqual(workerInputs.filter(({method,input})=>method==="pointer"&&(input.phase!=="move"||(input.buttons&1))).map(({input})=>input),inputs,"local Rust receives every original down, subthreshold move, semantic move and release in order");
  const semantic=inputs.filter((input)=>input.phase==="up"||(input.phase==="move"&&Math.hypot(input.x-down.x,input.y-down.y)>=3));
  assert.equal(semantic.length,5,"fixture includes four admitted movements and the release, plus a subthreshold move");
  const expected=semantic.map((input,index)=>({sequence:index+1,position:projectionFor(input).position}));
  const nativeSamples=workerInputs.filter(({method})=>method==="advancePoint").flatMap(({samples})=>samples);
  assert.deepEqual(nativeSamples,expected,"all semantic samples retain their exact native model positions and order");
  assert.deepEqual(command.samples,expected,"server replays the complete native semantic trace");
  const terminals=workerResults.filter(({request})=>request.method==="finishPoint");
  assert.equal(terminals.length,1);assert.deepEqual(command,terminals[0].result.terminal.command,"HTTP publishes the native terminal without translation");
  return command;
}

test("M98-F016/F017 folder canvas navigates and selects locally during a stalled edit, then reconciles native authority",{timeout:60000},async(t)=>{
  const f=await setup(t,undefined,{initScript:()=>{
    window.m98LocalInputs=[];window.m98LocalUpdates=[];
    const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker {
      constructor(...args){super(...args);this.addEventListener("message",({data})=>{
        if(data?.result?.frame&&data.result.state)window.m98LocalUpdates.push(structuredClone(data.result));
      });}
      postMessage(message,...rest){
        if(["pointer","wheel","resize","dispatch"].includes(message?.method))window.m98LocalInputs.push(structuredClone(message));
        return super.postMessage(message,...rest);
      }
    };
  }}),{folder,page,errors}=f;
  const host=page.getByRole("application"),canvas=page.locator('canvas[data-renderer="webgl2"]');
  await expect(host).toHaveAttribute("aria-busy","false");
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await expect(radiusField(page)).toHaveValue("10");
  await expect.poll(()=>page.evaluate(()=>window.m98LocalUpdates.length)).toBeGreaterThan(0);
  await expect.poll(()=>canvas.evaluate((element)=>{
    const frame=Reflect.get(element,"__geosolvePresentedFrame"),box=element.getBoundingClientRect();
    return Math.abs(frame.viewBox[2]-box.width)<0.01&&Math.abs(frame.viewBox[3]-box.height)<0.01;
  })).toBe(true);
  const beforeGeometry=await geometry(page),beforeState=await status(f),beforeSource=readSource(folder);
  const beforeDesign=await f.bridge.project.adapter.exportWorkspaceDesign();
  const beforeProject=await f.bridge.project.adapter.exportProject();
  const requests=[],responses=[];
  await page.route("**/api/rpc",async(route)=>{requests.push(route.request().postDataJSON());await route.continue();});
  page.on("response",(response)=>{
    if(new URL(response.url()).pathname!=="/api/rpc")return;
    responses.push((async()=>({request:response.request().postDataJSON(),status:response.status(),encoding:response.headers()["content-encoding"],body:await response.json()}))());
  });
  const wheel=async(input)=>host.evaluate(async(element,input)=>{
    const box=element.getBoundingClientRect(),event=new WheelEvent("wheel",{bubbles:true,clientX:box.x+input.x,clientY:box.y+input.y,deltaX:input.deltaX,deltaY:input.deltaY,ctrlKey:input.ctrl});
    element.dispatchEvent(event);await new Promise(requestAnimationFrame);
    return {...input,x:event.clientX-box.x,y:event.clientY-box.y};
  },input);
  const presentedCurve=()=>canvas.evaluate((element)=>Reflect.get(element,"__geosolvePresentedFrame").items.find((item)=>item.layer==="geometry"&&item.kind==="polyline"&&item.points.length>12));
  let held=false,heldAt;const release=Promise.withResolvers();
  const mutation=f.bridge.project.adapter.mutation;
  f.bridge.project.adapter.mutation=async(input)=>{
    if(!held){held=true;heldAt=performance.now();await release.promise;}
    return mutation(input);
  };
  try{
    // Preserve the preceding ordered-anchor regression, now at the local Rust
    // worker boundary. Navigation itself performs no HTTP or authored work.
    let box=await host.boundingBox();assert.ok(box);
    for(let x=30;x<=70;x+=2)await page.mouse.move(box.x+x,box.y+20);
    await page.evaluate(()=>{window.m98LocalInputs=[];});
    const expected=[];
    for(let index=0;index<20;index++){
      const pair=Math.floor(index/2);
      expected.push(await wheel({version:2,x:20+pair*3,y:20+pair*2,deltaX:0,deltaY:index%2?12:-12,ctrl:index%4<2}));
    }
    await expect.poll(()=>page.evaluate(()=>window.m98LocalInputs.filter(({method})=>method==="wheel").flatMap(({input})=>input.samples??[input]).length)).toBe(20);
    const actual=await page.evaluate(()=>window.m98LocalInputs.filter(({method})=>method==="wheel").flatMap(({input})=>input.samples??[input]));
    assert.deepEqual(actual,expected,"every ordered anchor, delta and Ctrl flag reaches shared local Rust");
    await expect.poll(async()=>Math.abs(curveWidth(await geometry(page))/curveWidth(beforeGeometry)-1)).toBeLessThan(1e-9);
    await page.setViewportSize({width:1560,height:1000});
    await expect.poll(()=>canvas.evaluate((element)=>Math.abs(Reflect.get(element,"__geosolvePresentedFrame").viewBox[2]-element.getBoundingClientRect().width))).toBeLessThan(0.01);
    await page.setViewportSize({width:1600,height:1000});
    await expect.poll(()=>canvas.evaluate((element)=>Math.abs(Reflect.get(element,"__geosolvePresentedFrame").viewBox[2]-element.getBoundingClientRect().width))).toBeLessThan(0.01);
    assert.equal(readSource(folder),beforeSource);
    assert.deepEqual(await f.bridge.project.adapter.exportWorkspaceDesign(),beforeDesign);
    assert.deepEqual(await f.bridge.project.adapter.exportProject(),beforeProject);
    const navigationState=await status(f);
    for(const key of ["authority","currentHash","acceptedHash","sourceHash","revision","acceptedRevision","writes","externalApplies"])assert.deepEqual(navigationState[key],beforeState[key],key);

    // Hold the actual server edit before native evaluation, longer than the
    // loading threshold. No client response can complete this operation yet.
    await radiusField(page).fill("12");await radiusField(page).press("Enter");
    await expect.poll(()=>held).toBe(true);
    await expect(page.getByText("You can keep navigating the last accepted sketch.",{exact:true})).toBeVisible();
    await expect(host).toHaveAttribute("aria-busy","true");
    assert.ok(performance.now()-heldAt>=450,"the delayed busy feedback is visible during the held request");
    const start=performance.now();await wheel({version:2,x:40,y:40,deltaX:0,deltaY:-90,ctrl:false});
    await expect.poll(async()=>curveWidth(await geometry(page))/curveWidth(beforeGeometry),{timeout:1000}).toBeGreaterThan(1.1);
    const localZoomMs=performance.now()-start;
    box=await host.boundingBox();assert.ok(box);
    const beforePan=await presentedCurve();
    await page.mouse.move(box.x+40,box.y+40);await page.mouse.down({button:"middle"});
    await page.mouse.move(box.x+58,box.y+52);await page.mouse.up({button:"middle"});
    await expect.poll(async()=>{
      const after=await presentedCurve();
      return Math.hypot(after.points[0][0]-beforePan.points[0][0]-18,after.points[0][1]-beforePan.points[0][1]-12);
    },{timeout:1000}).toBeLessThan(1e-9);
    const curve=await presentedCurve(),point=curve.points[Math.floor(curve.points.length/8)];
    await page.mouse.move(box.x+point[0],box.y+point[1]);
    await expect.poll(async()=>(await presentedCurve()).style.stroke,{timeout:1000}).not.toBe(curve.style.stroke);
    await page.mouse.click(box.x+point[0],box.y+point[1]);
    await page.mouse.move(box.x+20,box.y+20);
    await expect.poll(()=>page.evaluate(()=>window.m98LocalUpdates.at(-1).state.selection.length),{timeout:1000}).toBeGreaterThan(0);
    await expect.poll(async()=>(await presentedCurve()).style.shadow!==null,{timeout:1000}).toBe(true);
    const localBeforeRelease=await page.evaluate(()=>window.m98LocalUpdates.at(-1).state);
    const heldGeometry=await geometry(page),heldDurationMs=performance.now()-heldAt;
    assert.ok(heldDurationMs>500);
    assert.equal(readSource(folder),beforeSource,"navigation/selection cannot perform the held edit locally");
    assert.deepEqual(await f.bridge.project.adapter.exportProject(),beforeProject);
    assert.equal(requests.filter(({method})=>["pointer","wheel","wheelBatch","resize"].includes(method)).length,0,"camera, hover and selection never wait for HTTP");
    assert.equal(requests.filter(({method})=>method==="interaction.sync").length,0,"local Inspector browsing does not synchronize personal state");
    release.resolve();
    await expect.poll(()=>readSource(folder)).toContain("value: mm(12)");
    await expect(folderNotice(page)).toContainText("Saved to disk");
    await expect.poll(async()=>Math.abs(curveWidth(await geometry(page))/curveWidth(heldGeometry)-1.2)).toBeLessThan(0.01);
    const reconciled=await f.bridge.project.adapter.snapshot();
    assert.deepEqual(reconciled.seed.selection,[],"server seeds do not install personal selection");
    const localAfter=await page.evaluate(()=>window.m98LocalUpdates.at(-1).state);
    assert.deepEqual(localAfter.viewport,localBeforeRelease.viewport);
    assert.deepEqual(localAfter.selection,localBeforeRelease.selection);
    assert.notEqual(localAfter.sceneKey,localBeforeRelease.sceneKey,"local interaction now references the newly accepted scene");
    const native=await f.bridge.project.adapter.bakeProfile(0.01);
    assert.ok(native.regions[0].outer.every(([x,y])=>Math.abs(Math.hypot(x,y)-12)<1e-7),"server independently accepted the requested radius");
    const afterState=await status(f);
    assert.equal(afterState.writes,beforeState.writes+1);
    assert.equal(afterState.externalApplies,beforeState.externalApplies);
    assert.equal(afterState.currentHash,afterState.acceptedHash);
    assert.equal(reconciled.history.canUndo,true);
    const delivered=await Promise.all(responses);
    assert.ok(delivered.some(({encoding,body})=>encoding==="gzip"&&body.result?.seed),"Chromium receives compressed semantic model seeds");
    assert.ok(delivered.every(({status,body})=>status===200&&!body.error),JSON.stringify(delivered.filter(({status,body})=>status!==200||body.error)));
    assertNoPersonalRpc(delivered);
    assert.deepEqual(errors,[]);
    writeFileSync(resolve(evidence,"local-folder-navigation.json"),JSON.stringify({localZoomMs,heldDurationMs,wheelSamples:actual,navigationRpcCount:0,localBeforeRelease,localAfter,selection:localAfter.selection,serverSelection:reconciled.seed.selection,editWrites:afterState.writes-beforeState.writes,requests:requests.map(({method,input})=>({method,command:input?.command})),compressedSeeds:delivered.filter(({encoding,body})=>encoding==="gzip"&&body.result?.seed).length},null,2));
  }finally{release.resolve();f.bridge.project.adapter.mutation=mutation;await page.unroute("**/api/rpc");}
});

test("M98-F017 local folder authoring previews, multi-click Finish, exact point drag and history retain the local camera",{timeout:90000},async(t)=>{
  const f=await setup(t,undefined,{initScript:captureLocalAuthoring}),{folder,page,errors}=f;
  const host=page.getByRole("application"),canvas=page.locator('canvas[data-renderer="webgl2"]');
  const frame=()=>canvas.evaluate((element)=>Reflect.get(element,"__geosolvePresentedFrame"));
  const localState=()=>page.evaluate(()=>window.m98LocalUpdates.at(-1)?.state);
  const settled=async()=>{await expect(host).toHaveAttribute("aria-busy","false");await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));await expect(host).toHaveAttribute("aria-busy","false");};
  const accepted=async()=>{
    await settled();
    const native=await f.bridge.project.adapter.snapshot();
    await expect.poll(async()=>(await localState()).sceneKey).toBe(native.seed.sceneKey);
    await expect.poll(async()=>(await frame()).provenance.scene).toBe("accepted-presentation");
  };
  const responses=[];
  page.on("response",(response)=>{if(new URL(response.url()).pathname==="/api/rpc")responses.push((async()=>({request:response.request().postDataJSON(),status:response.status(),body:await response.json()}))());});
  const capture=async()=>{
    if(page.isClosed())return;
    writeFileSync(resolve(evidence,"local-folder-authoring-debug.json"),JSON.stringify({source:readSource(folder),responses:await Promise.all(responses),frame:await frame(),local:await localState(),notice:await folderNotice(page).innerText(),native:await f.bridge.project.adapter.snapshot(),worker:await page.evaluate(()=>({inputs:window.m98WorkerInputs,results:window.m98WorkerResults,pointers:window.m98PointerInputs})),errors},null,2));
  };
  try {
  await settled();await expect.poll(localState).toBeTruthy();
  const initialSource=readSource(folder),initialGeometry=await geometry(page),initialStatus=await status(f);
  const select=page.getByRole("navigation",{name:"Primary tools"}).getByRole("button",{name:"Select",exact:true});
  await page.getByRole("navigation",{name:"Primary tools"}).getByRole("button",{name:"Sketch",exact:true}).click();
  await page.getByRole("menuitem",{name:"Polyline",exact:true}).click();await settled();
  const box=await host.boundingBox();assert.ok(box);
  const positions=[[0.18,0.18],[0.32,0.23],[0.40,0.15]].map(([x,y])=>[box.x+box.width*x,box.y+box.height*y]);
  const finish=page.getByRole("button",{name:"Finish",exact:true});
  await expect(finish).toBeDisabled();
  // M98 U03 requires mid-tool navigation to preserve collected inputs. The
  // shared controller records the new native viewport before the next sample.
  await page.mouse.click(...positions[0]);await settled();
  const armedViewport=(await localState()).viewport;
  const firstClick=await page.evaluate(()=>window.m98WorkerInputs.filter(({method})=>method==="advanceConstruction").flatMap(({samples})=>samples).find(({input})=>input.event==="click"));
  assert.ok(firstClick);
  await page.mouse.wheel(0,-55);
  await expect.poll(async()=>JSON.stringify((await localState()).viewport)).not.toBe(JSON.stringify(armedViewport));
  await settled();await expect(finish).toBeDisabled();
  assert.equal(readSource(folder),initialSource,"navigation cannot publish a partial draft");
  assert.equal((await status(f)).writes,initialStatus.writes,"wheel performs no authoring transaction");
  await page.mouse.move(...positions[1]);
  await expect.poll(async()=>(await frame()).items.some(({layer})=>layer==="draft")).toBe(true);
  assert.equal(readSource(folder),initialSource,"preview cannot publish an incomplete polyline");
  await page.mouse.click(...positions[1]);await settled();await expect(finish).toBeEnabled();
  await page.mouse.move(...positions[2]);
  await expect.poll(async()=>(await frame()).items.some(({layer})=>layer==="draft")).toBe(true);
  await page.mouse.click(...positions[2]);await settled();
  assert.equal(readSource(folder),initialSource,"multiple draft clicks remain unpublished until Finish");
  await finish.click();
  await expect.poll(()=>readSource(folder)).toContain("$.geometry.polyline");await accepted();
  const authoredSource=readSource(folder),authoredStatus=await status(f);
  assert.equal(authoredStatus.writes,initialStatus.writes+1,"Finish writes exactly one source transaction");
  const constructionCommits=(await Promise.all(responses)).filter(({request})=>request.method==="authoring.commit"&&request.input.kind==="construction");
  assert.equal(constructionCommits.length,1);
  const constructionCommand=constructionCommits[0].request.input.command;
  const clicks=constructionCommand.samples.filter(({input})=>input.event==="click");
  assert.equal(clicks.length,3,"mid-tool zoom preserves exactly the three original clicks");
  assert.deepEqual(clicks[0],firstClick,"the first staged vertex retains its exact native model position across zoom");
  assert.ok(constructionCommand.samples.some(({sequence,input})=>input.event==="viewport"&&sequence>clicks[0].sequence&&sequence<clicks[1].sequence),"the native terminal records the changed camera before the next click");
  const polyline=constructionCommand.expected_declarations.find(({builder_path})=>builder_path.join(".")==="geometry.polyline");
  assert.equal(polyline.arguments.value.vertices.value.length,3,"accepted polyline contains exactly the three collected vertices");
  assert.ok((await geometry(page)).length>initialGeometry.length,"new polyline has actual accepted painted spans");
  await select.click();await settled();
  await page.mouse.move(box.x+40,box.y+40);await page.mouse.wheel(0,-70);
  const terminalCount=await page.evaluate(()=>window.m98PointerTerminals.length);
  await page.mouse.down({button:"middle"});await page.mouse.move(box.x+55,box.y+51);await page.mouse.up({button:"middle"});
  // Browser pointer-up delivery precedes the asynchronous native acknowledgement.
  // A viewport differing from the server can already be satisfied by the wheel;
  // take the history baseline only after this pan's exact terminal has arrived.
  await expect.poll(()=>page.evaluate(()=>window.m98PointerTerminals.length)).toBe(terminalCount+1);
  const terminal=await page.evaluate(()=>window.m98PointerTerminals.at(-1));
  const terminalInput=await page.evaluate(()=>window.m98PointerInputs.filter(input=>input.phase==="up").at(-1));
  assert.deepEqual(terminal.input,terminalInput);
  const camera=terminal.state.viewport;
  assert.deepEqual((await localState()).viewport,camera);
  assert.notDeepEqual(camera,JSON.parse((await f.bridge.project.adapter.snapshot()).seed.scene).viewport);
  await page.getByRole("button",{name:"Undo",exact:true}).click();await expect.poll(()=>readSource(folder)).toBe(initialSource);await accepted();
  assert.deepEqual((await localState()).viewport,camera,"Undo cannot rewind local navigation");
  await page.getByRole("button",{name:"Redo",exact:true}).click();await expect.poll(()=>readSource(folder)).toBe(authoredSource);await accepted();
  assert.deepEqual((await localState()).viewport,camera,"Redo cannot rewind local navigation");
  const beforeFrame=await frame();
  const point=beforeFrame.items.filter(item=>item.layer==="points"&&item.kind==="circle"&&item.interactive).sort((a,b)=>a.center[0]-b.center[0])[0];
  assert.ok(point,"authoring publishes an independently pickable point");
  const pointId=point.metadata.persistentId;
  const pointAfter=async()=>(await frame()).items.find(item=>item.layer==="points"&&item.kind==="circle"&&item.metadata.persistentId===pointId);
  const beforeNative=JSON.parse((await f.bridge.project.adapter.snapshot()).seed.scene).points;
  const beforeDrag=await status(f);const beforeDesign=await f.bridge.project.adapter.exportWorkspaceDesign();
  await page.evaluate(()=>{window.m98PointerInputs=[];window.m98WorkerInputs=[];window.m98WorkerResults=[];});const responseStart=responses.length;
  const dragBox=await host.boundingBox(),start=[dragBox.x+point.center[0],dragBox.y+point.center[1]];
  await page.mouse.move(...start);await page.mouse.down();
  for(const [dx,dy] of [[1,0],[5,2],[10,4],[16,7],[22,9]])await page.mouse.move(start[0]+dx,start[1]+dy);
  await page.mouse.up();
  await expect.poll(async()=>Math.hypot(...((await pointAfter())?.center??point.center).map((value,index)=>value-point.center[index])),{timeout:10000}).toBeGreaterThan(15);
  await settled();await expect.poll(async()=>(await status(f)).writes).toBe(beforeDrag.writes+1);await accepted();
  const pointerInputs=(await page.evaluate(()=>window.m98PointerInputs)).filter(({phase,buttons})=>phase!=="move"||(buttons&1));
  const command=await assertPointTerminal(page,await Promise.all(responses.slice(responseStart)),pointerInputs);
  assert.equal(readSource(folder),authoredSource,"point movement remains a semantic sidecar edit");
  assert.notDeepEqual(await f.bridge.project.adapter.exportWorkspaceDesign(),beforeDesign);
  const afterNative=JSON.parse((await f.bridge.project.adapter.snapshot()).seed.scene).points;
  assert.notDeepEqual(afterNative.map(({model_position})=>model_position),beforeNative.map(({model_position})=>model_position),"native accepted coordinates actually moved");
  assert.ok(afterNative.every(({model_position})=>model_position.every(Number.isFinite)));
  await page.getByRole("button",{name:"Undo",exact:true}).click();await settled();
  await expect.poll(async()=>Math.hypot(...((await pointAfter())?.center??point.center).map((value,index)=>value-point.center[index]))).toBeLessThan(1e-7);
  await page.getByRole("button",{name:"Redo",exact:true}).click();await settled();
  await expect.poll(async()=>Math.hypot(...((await pointAfter())?.center??point.center).map((value,index)=>value-point.center[index]))).toBeGreaterThan(15);
  assert.deepEqual((await localState()).viewport,camera);
  const delivered=await Promise.all(responses);assert.ok(delivered.every(({status,body})=>status===200&&!body.error),JSON.stringify(delivered.filter(({status,body})=>status!==200||body.error)));
  assert.deepEqual(errors,[]);
  writeFileSync(resolve(evidence,"local-folder-authoring.json"),JSON.stringify({pointerInputs,command,pointId,camera,finishWrites:authoredStatus.writes-initialStatus.writes,dragWrites:(await status(f)).writes-beforeDrag.writes-2,sourceHash:hash(authoredSource),beforeNative,afterNative},null,2));
  }finally{await capture();}
});

test("M98-F017 local folder point drag keeps exact semantic samples, one release write and camera-preserving history",{timeout:60000},async(t)=>{
  const f=await setup(t,undefined,{initScript:captureLocalAuthoring}),{page,folder,errors}=f;
  const host=page.getByRole("application"),canvas=page.locator('canvas[data-renderer="webgl2"]');
  const frame=()=>canvas.evaluate(element=>Reflect.get(element,"__geosolvePresentedFrame"));
  const localState=()=>page.evaluate(()=>window.m98LocalUpdates.at(-1)?.state);
  const settled=async()=>{await expect(host).toHaveAttribute("aria-busy","false");await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));await expect(host).toHaveAttribute("aria-busy","false");};
  const responses=[];page.on("response",response=>{if(new URL(response.url()).pathname==="/api/rpc")responses.push((async()=>({request:response.request().postDataJSON(),status:response.status(),body:await response.json()}))());});
  try{
    await settled();await expect.poll(localState).toBeTruthy();
    const before=(await frame()).items.find(item=>item.layer==="points"&&item.kind==="circle"&&item.interactive);assert.ok(before);
    const pointId=before.metadata.persistentId;
    const point=async()=>(await frame()).items.find(item=>item.layer==="points"&&item.kind==="circle"&&item.metadata.persistentId===pointId);
    const source=readSource(folder),beforeStatus=await status(f),beforeDesign=await f.bridge.project.adapter.exportWorkspaceDesign();
    const beforeNative=JSON.parse((await f.bridge.project.adapter.snapshot()).seed.scene).points;
    const camera=(await localState()).viewport,box=await host.boundingBox(),start=[box.x+before.center[0],box.y+before.center[1]];
    await page.mouse.move(...start);await page.evaluate(()=>{window.m98PointerInputs=[];window.m98WorkerInputs=[];window.m98WorkerResults=[];});
    await page.mouse.down();for(const[dx,dy]of[[1,0],[5,2],[10,4],[16,7],[22,9]])await page.mouse.move(start[0]+dx,start[1]+dy);await page.mouse.up();
    await expect.poll(async()=>Math.hypot((await point()).center[0]-before.center[0],(await point()).center[1]-before.center[1]),{timeout:10000}).toBeGreaterThan(15);
    await settled();await expect.poll(async()=>(await status(f)).writes).toBe(beforeStatus.writes+1);
    const inputs=(await page.evaluate(()=>window.m98PointerInputs)).filter(({phase,buttons})=>phase!=="move"||(buttons&1));
    const command=await assertPointTerminal(page,await Promise.all(responses),inputs);
    assert.equal(readSource(folder),source);assert.notDeepEqual(await f.bridge.project.adapter.exportWorkspaceDesign(),beforeDesign);
    const afterNative=JSON.parse((await f.bridge.project.adapter.snapshot()).seed.scene).points;
    assert.notDeepEqual(afterNative.map(({model_position})=>model_position),beforeNative.map(({model_position})=>model_position));assert.ok(afterNative.every(({model_position})=>model_position.every(Number.isFinite)));
    const moved=(await point()).center;
    await page.getByRole("button",{name:"Undo",exact:true}).click();await settled();await expect.poll(async()=>(await point()).center).toEqual(before.center);
    await page.getByRole("button",{name:"Redo",exact:true}).click();await settled();await expect.poll(async()=>(await point()).center).toEqual(moved);
    assert.deepEqual((await localState()).viewport,camera);assert.deepEqual(errors,[]);
    const delivered=await Promise.all(responses);assert.ok(delivered.every(({status,body})=>status===200&&!body.error),JSON.stringify(delivered.filter(({status,body})=>status!==200||body.error)));
    writeFileSync(resolve(evidence,"local-folder-point-drag.json"),JSON.stringify({inputs,command,pointId,camera,beforeNative,afterNative,moved,writeDelta:(await status(f)).writes-beforeStatus.writes},null,2));
  }finally{writeFileSync(resolve(evidence,"local-folder-point-drag-debug.json"),JSON.stringify({responses:await Promise.all(responses),source:readSource(folder),frame:await frame(),local:await localState(),native:await f.bridge.project.adapter.snapshot(),notice:await folderNotice(page).innerText(),errors},null,2));}
});

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
  const elapsed=Date.now()-start;
  // Inspector publication can precede the renderer's next animation frame.
  await expect.poll(()=>geometry(page),{message:"external radius edit must reach the presented canvas"}).not.toEqual(before);
  const after=await geometry(page);
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
  const canvas=page.locator('canvas[data-renderer="webgl2"]');
  const fullWidth=await canvas.evaluate((element)=>element.getBoundingClientRect().width);
  await page.getByRole("button",{name:"split",exact:true}).click();
  await expect(page.locator(".cm-content")).toContainText("value: mm(14)");
  // CodeMirror can appear before the resize RPC publishes its camera frame.
  // Capture the exact retained-geometry baseline only at the new Split extent.
  await expect.poll(()=>canvas.evaluate((element,previousWidth)=>{
    const frame=Reflect.get(element,"__geosolvePresentedFrame"),box=element.getBoundingClientRect();
    return box.width<previousWidth && Math.abs(frame.viewBox[2]-box.width)<0.01 && Math.abs(frame.viewBox[3]-box.height)<0.01;
  },fullWidth),{message:"Split resize must be presented before retaining its geometry baseline"}).toBe(true);
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
  const browsing=await nativeBrowsing(snapshot);
  try {
    const dimensions=browsing.initial.snapshot.dimensions;
    assert.ok(dimensions.allMeasurements.some((entry)=>entry.label==="ringRadius"&&Number(entry.value)===18),JSON.stringify(dimensions));
  } finally { browsing.dispose(); }
  await assert.rejects(f.bridge.project.adapter.exportProject(),/Canonical export is unavailable while source is invalid or unfinished/);
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

test("slow external generator evaluation dims the retained canvas after half a second and clears on acceptance or rejection",{timeout:90000},async(t)=>{
  const source='import {sketch,mm} from "@geosolve/sketch-code";export default sketch(($)=>({port:$.geometry.centerRadiusCircle("port",{center:[0,0],radius:mm(6)})}));';
  const {folder,page,errors}=await setup(t,(folder)=>{
    writeFileSync(resolve(folder,"geosolve.json"),manifest("generator"));writeFileSync(sourcePath(folder),source);
  },{initScript:()=>{
    window.m98Activity=[];
    const NativeEventSource=window.EventSource;
    window.EventSource=class extends NativeEventSource {
      constructor(...args) {
        super(...args);
        this.addEventListener("activity",(event)=>window.m98Activity.push({...JSON.parse(event.data),at:performance.now()}));
      }
    };
  }});
  const indicator=page.getByRole("status").filter({hasText:"Solving…"});
  await expect(indicator).toHaveCount(0);
  await page.waitForTimeout(600);
  await expect(indicator).toHaveCount(0);
  const before=await geometry(page);
  await page.evaluate(()=>{window.m98Activity=[];});
  const pause='const until=Date.now()+2500;while(Date.now()<until){};';
  replaceSource(folder,pause+source.replace("mm(6)","mm(8)"));
  await expect.poll(()=>page.evaluate(()=>window.m98Activity.some((event)=>event.busy))).toBe(true);
  await page.waitForTimeout(250);
  await expect(indicator).toHaveCount(0);
  await expect(indicator).toBeVisible();
  const shownAfterMs=await page.evaluate(()=>performance.now()-window.m98Activity.find((event)=>event.busy).at);
  assert.ok(shownAfterMs>=500,`indicator must wait 500 ms, observed ${shownAfterMs}`);
  assert.deepEqual(await geometry(page),before,"last accepted canvas remains visible while the external generator is running");
  const host=page.getByRole("application"),box=await host.boundingBox();assert.ok(box);
  const startNavigation=performance.now();
  await page.mouse.move(box.x+40,box.y+40);await page.mouse.wheel(0,-90);
  await expect.poll(async()=>curveWidth(await geometry(page))/curveWidth(before),{timeout:1000}).toBeGreaterThan(1.1);
  const localNavigationMs=performance.now()-startNavigation;
  await expect(indicator).toBeVisible();
  const navigated=await geometry(page);
  await page.mouse.click(box.x+20,box.y+20);
  const overlay=page.locator(".geosolve-solving-overlay");
  assert.notEqual(await overlay.evaluate((element)=>getComputedStyle(element).backgroundColor),"rgba(0, 0, 0, 0)","overlay visibly dims the canvas");
  await page.screenshot({path:resolve(evidence,"slow-folder-solving.png")});
  await expect.poll(async()=>curveWidth(await geometry(page))/curveWidth(navigated),{timeout:10000}).toBeGreaterThan(1.3);
  await expect(indicator).toHaveCount(0);
  const accepted=await geometry(page);
  replaceSource(folder,pause+'throw Error("deliberately rejected generator");'+source);
  await expect(indicator).toBeVisible();
  assert.deepEqual(await geometry(page),accepted);
  await expect(folderNotice(page)).toContainText("deliberately rejected generator",{timeout:10000});
  await expect(indicator).toHaveCount(0);
  assert.deepEqual(await geometry(page),accepted,"rejection clears feedback without replacing accepted geometry");
  writeFileSync(resolve(evidence,"slow-folder-solving.json"),JSON.stringify({shownAfterMs,localNavigationMs,activity:await page.evaluate(()=>window.m98Activity),retainedAfterRejection:true},null,2));
  assert.deepEqual(errors,[]);
});

test("complete manifold opens from plain files and shared channel edits export all regions",{timeout:180000},async(t)=>{
  // Native browser reconstruction is asynchronous startup, within this case's
  // existing overall deadline. Input latency/veil tests keep their own measured
  // budgets; an implicit locator default is not a startup performance contract.
  const {folder,page,errors,openingMs}=await setup(t,(folder)=>cpSync(resolve("examples/file-workspace-manifold"),folder,{recursive:true}),{readinessTimeout:30000});
  writeFileSync(resolve(evidence,"manifold-startup.json"),JSON.stringify({openingMs,geometryItems:(await geometry(page)).length},null,2));
  await expect(page.getByRole("region",{name:"Local folder"})).toContainText("Saved to disk");
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  await page.screenshot({path:resolve(evidence,"manifold-folder.png")});
  // Native owning tests certify analytic geometry; this adapter test checks real
  // source/parameter visibility and complete export after a revision-bound GUI edit.
  const input=page.getByRole("textbox",{name:"Channel width",exact:true}).first();
  await expect(input).toHaveValue("12");
  // Enter starts a complete asynchronous source transaction. Wait for its
  // response before inspecting disk; the source and export remain independent
  // assertions rather than a 30-second performance deadline for this sample.
  await input.fill("11");
  const [edited]=await Promise.all([
    page.waitForResponse(response=>{
      if(new URL(response.url()).pathname!=="/api/rpc")return false;
      const request=response.request().postDataJSON();
      return request?.method==="authoring.mutation";
    },{timeout:60000}),
    input.press("Enter"),
  ]);
  assert.equal(edited.status(),200);
  assert.equal((await edited.json()).error,undefined);
  await expect.poll(()=>readFileSync(resolve(folder,"sketch.ts"),"utf8"),{timeout:30000}).toContain('$.parameter("channelWidth", mm(11)');
  const output=resolve(evidence,"manifold-from-gui.json");
  const baked=await bakeProject(folder,output,0.02);
  assert.equal(baked.regions.length,18);
  assert.deepEqual(errors,[]);
});
