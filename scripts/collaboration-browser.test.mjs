// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { cp, mkdtemp, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { get } from "node:http";
import { chromium } from "../crates/geosolve-demo-web/frontend/node_modules/playwright-core/index.mjs";
import { openCollaborationRuntime } from "./collaboration-runtime.mjs";
import { inventory, mediaType, hash } from "../crates/geosolve-demo-web/frontend/scripts/release-artifact-lib.mjs";
import { CollaborationClient } from "../packages/geosolve-collaboration/dist/client.js";
import { createSharedText } from "../packages/geosolve-collaboration/dist/index.js";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return {promise,resolve}; };
function afterMinimum(ms,run){
  const deadline=performance.now()+ms;let timer;
  const finish=()=>{const remaining=deadline-performance.now();if(remaining>0)timer=setTimeout(finish,Math.ceil(remaining));else run();};
  timer=setTimeout(finish,ms);return ()=>clearTimeout(timer);
}
async function until(predicate, message, timeout=30_000) {
  const deadline=performance.now()+timeout;
  while(performance.now()<deadline){if(await predicate())return;await sleep(20);}
  throw Error(message);
}
const p95 = values => [...values].sort((a,b)=>a-b)[Math.ceil(values.length*.95)-1];
const progress = (stage,detail={}) => { if(process.env.GEOSOLVE_BROWSER_TRACE==="1")process.stderr.write(JSON.stringify({stage,...detail})+"\n"); };
const initialSource=`"use geosolve sketch";
import {sketch,mm} from "@geosolve/sketch-code";
// Unicode ownership 😀
export default sketch(($)=>{
 const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});
 const other=$.geometry.centerRadiusCircle("other",{center:[20,0],radius:mm(3)});
 return {bore,other};
});
`;
async function fixture(t,options={}){
  const folder=await mkdtemp(join(tmpdir(),"geosolve-shared-browser-"));
  if(options.projectFolder)await cp(options.projectFolder,folder,{recursive:true});
  else{
    await writeFile(join(folder,"geosolve.json"),JSON.stringify({format:"geosolve-folder-v2",entry:"sketch.ts",mode:"editable"}));
    await writeFile(join(folder,"sketch.ts"),options.projectSource??initialSource);
  }
  const invitations=new Map([...Array.from({length:8},(_,i)=>[`editor-${i}`,{userId:`editor-${i}`,role:"editor"}]),...Array.from({length:24},(_,i)=>[`viewer-${i}`,{userId:`viewer-${i}`,role:"viewer"}])]);
  const dist=resolve(process.env.GEOSOLVE_DIST??"crates/geosolve-demo-web/dist"),files=await inventory(dist),routes=new Map();
  for(const file of files){const body=await readFile(join(dist,file.path));assert.equal(hash(body),file.sha256);routes.set(`/${file.path}`,{body,type:mediaType(file.path)});}
  routes.set("/",routes.get("/index.html"));
  const runtime=await openCollaborationRuntime(folder,{initialize:true,invitations,workbenchScenes:true,staticRoutes:routes,domainOptions:options.domainOptions,limits:options.limits,authoringPreview:options.authoringPreview});
  const address=await runtime.listen(),origin=`http://127.0.0.1:${address.port}`,baseUrl=`${origin}/api/collaboration/`;
  const clients=[],replicas=[],cleanup=[];let browser;
  t.after(async()=>{for(const action of cleanup)await action();for(const client of clients)client.dispose();await browser?.close();await runtime.close();for(const replica of replicas)replica.dispose();await rm(folder,{recursive:true,force:true});});
  return {runtime,folder,origin,baseUrl,files,clients,replicas,cleanup,
    async browser(){return browser??=await chromium.launch({headless:true,executablePath:process.env.GEOSOLVE_CHROMIUM_PATH??"/home/arduano/.nix-profile/bin/google-chrome",args:["--disable-dev-shm-usage","--enable-webgl","--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader"]});},
    async client(index,role="editor",fetchImpl){const client=new CollaborationClient({baseUrl,inviteToken:`${role}-${index}`,clientId:`load-${role}-${index}`,...(fetchImpl?{fetch:fetchImpl}:{})});clients.push(client);await client.connect();return client;},
    async replica(client){const state=await client.refresh(),replica=await createSharedText({actor:Uint8Array.from(state.document.textActor),checkpoint:Uint8Array.from(state.document.textCheckpoint)});replicas.push(replica);return replica;},
  };
}
const presented = page => page.locator("canvas").evaluate(canvas=>{
  const frame=canvas.__geosolvePresentedFrame;
  // Native provisional geometry is deliberately noninteractive. Observe the
  // painted source geometry in either state, excluding presence and chrome.
  return frame?JSON.stringify(frame.items.filter(item=>item.layer!=="presence"&&item.className.split(/\s/u).some(name=>name==="wb-point"||name==="wb-curve")).map(({kind,id,center,radius,points})=>({kind,id,center,radius,points}))):null;
});
async function ready(page){await page.getByRole("region",{name:"Shared document",exact:true}).waitFor({timeout:90_000});await page.locator('canvas[data-render-state="ready"]').waitFor({timeout:90_000});}
async function resized(page){
  await until(()=>page.locator("canvas").evaluate(canvas=>{
    const bounds=canvas.getBoundingClientRect(),frame=canvas.__geosolvePresentedFrame,renderer=canvas.__geosolveRendererDiagnostics;
    return frame&&renderer&&Math.abs(frame.viewBox[2]-bounds.width)<.1&&Math.abs(frame.viewBox[3]-bounds.height)<.1&&Math.abs(renderer.width-bounds.width)<.1&&Math.abs(renderer.height-bounds.height)<.1;
  }),"The native canvas must present its resized viewport before navigation measurement");
}
async function select(page,name){await page.locator("[data-navigation-row]").filter({hasText:name}).first().click();await until(async()=>await page.locator('[data-navigation-row][aria-pressed="true"]').filter({hasText:name}).count()>0,`Explorer ${name} did not select`);}
async function toolPage(f,index=0,mode="client"){
  const browser=await f.browser(),context=await browser.newContext({viewport:{width:1440,height:900}});
  await context.addInitScript(()=>{
    window.geosolveToolTrace=[];const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker{
      constructor(url,options){super(url,options);if(String(url).includes("collaboration-authoring-worker"))this.addEventListener("message",({data})=>{if(data?.result?.operation){window.geosolveToolTrace.push(data.result.operation);if(window.geosolveToolTrace.length>20)window.geosolveToolTrace.shift();}});}
    };
  });
  const page=await context.newPage();
  const errors=[];page.on("pageerror",error=>errors.push(String(error)));
  f.cleanup.push(async()=>progress("tool-page-final",{index,errors,alerts:await page.getByRole("alert").allTextContents(),shared:await page.getByRole("region",{name:"Shared document",exact:true}).allTextContents(),options:await page.getByRole("group",{name:"Active tool options",exact:true}).allTextContents(),trace:await page.evaluate(()=>window.geosolveToolTrace)}));
  await page.goto(`${f.origin}/?collaboration=1&authoringPreview=${mode}#invite=editor-${index}`);await ready(page);
  await page.getByRole("group",{name:"Workspace layout",exact:true}).getByRole("button",{name:"design",exact:true}).click();await resized(page);
  return {page,errors};
}
async function chooseTool(page,section,name,waitOptions=true){
  await page.getByRole("button",{name:section,exact:true}).click();
  await page.getByRole("menuitem",{name,exact:true}).click();
  if(waitOptions)await page.getByRole("group",{name:"Active tool options",exact:true}).waitFor();await resized(page);
}
// Locations come from the actual native drawing, including its current camera;
// these helpers never duplicate curve evaluation or predict accepted geometry.
async function curvePosition(page,index=0){
  return page.locator("canvas").evaluate((canvas,index)=>{
    const frame=canvas.__geosolvePresentedFrame,curves=frame.items.filter(item=>item.kind==="polyline"&&item.className.split(/\s/u).includes("wb-curve"));
    const item=curves[index];if(!item)throw Error(`Missing native curve ${index}`);
    const points=item.points,mid=Math.floor((points.length-1)/2),a=points[mid],b=points[mid+1];
    const bounds=canvas.getBoundingClientRect();
    return {x:bounds.x+(a[0]+b[0])/2-frame.viewBox[0],y:bounds.y+(a[1]+b[1])/2-frame.viewBox[1]};
  },index);
}
async function clickCurve(page,index=0){const point=await curvePosition(page,index);await page.mouse.click(point.x,point.y);}
async function setToolNumber(page,label,value){const input=page.getByLabel(label,{exact:true});await input.fill(String(value));await input.press("Enter");}
async function published(f,revision,page){
  await until(()=>f.runtime.host.snapshot().acceptedRevision===revision,`Native tool did not publish revision ${revision}`);
  if(page)await until(()=>page.locator("canvas").evaluate(canvas=>canvas.__geosolvePresentedFrame?.provenance.scene==="accepted-presentation"),"Accepted scene did not replace the native draft");
  return f.runtime.source().snapshot().accepted.files["sketch.ts"];
}
const cornerSource=`"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
 const corner = $.geometry.polyline("corner", {vertices:[
  {key:"start",position:[200,0]}, {key:"corner",position:[220,0]}, {key:"end",position:[220,20]}
 ]});
 return {corner};
});
`;
async function traceNavigation(context){
  await context.addInitScript(()=>{
    window.geosolveNavigationTrace=[];window.geosolveTextTrace=[];window.geosolveLongTasks=[];
    new PerformanceObserver(list=>{for(const entry of list.getEntries())window.geosolveLongTasks.push({at:entry.startTime,duration:entry.duration});}).observe({type:"longtask",buffered:true});
    window.addEventListener("wheel",()=>window.geosolveNavigationTrace.push({stage:"wheel-event",at:performance.now()}),{capture:true});
    const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker{
      constructor(url,options){super(url,options);this.traceLocal=String(url).includes("local-interaction-worker");this.traceText=String(url).includes("collaboration-text-worker");this.traceMethods=new Map();
        if(this.traceLocal||this.traceText)this.addEventListener("message",({data})=>{const method=this.traceMethods.get(data?.id);if(method==="wheel")window.geosolveNavigationTrace.push({stage:"wheel-reply",at:performance.now(),id:data.id,error:data.error??null});if(method==="edit")window.geosolveTextTrace.push({stage:"edit-reply",at:performance.now(),id:data.id,error:data.error??null});this.traceMethods.delete(data?.id);});
      }
      postMessage(message,...args){if(this.traceLocal||this.traceText){this.traceMethods.set(message?.id,message?.method);if(message?.method==="wheel")window.geosolveNavigationTrace.push({stage:"wheel-send",at:performance.now(),id:message.id});if(message?.method==="edit")window.geosolveTextTrace.push({stage:"edit-send",at:performance.now(),id:message.id});}return super.postMessage(message,...args);}
    };
  });
}

test("shared tool parity exposes every native family and shares advanced construction, reference dimensions and personal history",{timeout:180_000},async t=>{
  const f=await fixture(t,{authoringPreview:{enabled:false,preferred:"client"}});
  const first=await toolPage(f),second=await toolPage(f,1),page=first.page,peer=second.page;
  for(const [section,count] of [["Sketch",25],["Constraint",13],["Dimension",5],["Modify",2]]){
    await page.getByRole("button",{name:section,exact:true}).click();
    const items=page.getByRole("menuitem");assert.equal(await items.count(),count);
    assert.equal(await items.evaluateAll(items=>items.filter(item=>item.hasAttribute("disabled")||item.getAttribute("aria-disabled")==="true").length),0,`${section} has disabled native tools`);
    await page.keyboard.press("Escape");
  }
  await chooseTool(page,"Sketch","Rational Quadratic");await setToolNumber(page,"Middle weight",2);
  const box=await page.locator("canvas").boundingBox();
  for(const [x,y] of [[.25,.18],[.45,.1],[.62,.22]])await page.mouse.click(box.x+box.width*x,box.y+box.height*y);
  const conic=await published(f,1,page);assert.match(conic,/\$\.geometry\.rationalQuadraticConic/u);assert.match(conic,/middleWeight:\s*2/u);
  await until(()=>peer.locator("[data-navigation-row]").filter({hasText:"Rational"}).count(),"Peer did not receive the advanced curve");
  // A native option chosen before any operands must survive opening the tool
  // with an exact preselected curve and commit the reference measurement.
  await peer.bringToFront();await chooseTool(peer,"Dimension","Radius");
  await peer.getByRole("combobox",{name:"Dimension",exact:true}).selectOption("reference");
  await peer.getByRole("button",{name:"Select",exact:true}).click();await resized(peer);
  await clickCurve(peer,0);await chooseTool(peer,"Dimension","Radius",false);
  const dimension=await published(f,2,peer);assert.match(dimension,/\$\.dimension\.radius/u);assert.match(dimension,/mode:\s*"reference"/u);
  await page.bringToFront();await page.getByRole("button",{name:"Undo",exact:true}).click();
  const undone=await published(f,3,page);assert.doesNotMatch(undone,/\$\.geometry\.rationalQuadraticConic/u);assert.match(undone,/\$\.dimension\.radius/u);
  await page.getByRole("button",{name:"Redo",exact:true}).click();
  const redone=await published(f,4,page);assert.match(redone,/\$\.geometry\.rationalQuadraticConic/u);assert.match(redone,/\$\.dimension\.radius/u);
  assert.deepEqual([...first.errors,...second.errors],[]);
});

test("shared tool parity keeps native constraint preselection and pick tolerance across camera changes",{timeout:120_000},async t=>{
  const source=`"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";
export default sketch(($) => {
 const first = $.geometry.segment("first", {start:[0,0],end:[30,15]});
 const second = $.geometry.segment("second", {start:[0,35],end:[25,48]});
 return {first,second};
});`;
  const f=await fixture(t,{projectSource:source,authoringPreview:{enabled:false,preferred:"client"}}),{page,errors}=await toolPage(f),commands=[];
  page.on("request",request=>{if(new URL(request.url()).pathname.endsWith("/commands"))commands.push(request.postDataJSON());});
  await clickCurve(page,0);await chooseTool(page,"Constraint","Parallel");
  const before=await presented(page),box=await page.locator("canvas").boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.wheel(0,-90);
  await until(async()=>await presented(page)!==before,"Active native constraint did not reproject after zoom");
  await clickCurve(page,1);
  const accepted=await published(f,1,page);assert.match(accepted,/\$\.constraint\.parallel/u);
  const operation=commands.map(request=>request.command?.payload?.gesture).find(gesture=>gesture?.tool==="parallel");
  assert.ok(operation,"Expected native parallel command");assert.ok(operation.selection.length>0,"Native preselection was lost");
  assert.ok(operation.samples.some(sample=>sample.input.event==="viewport"),"Current native pick camera was not recorded for replay");
  assert.deepEqual(errors,[]);
});

test("shared tool parity authors Fillet options and selected geometry roles with reversible native history",{timeout:180_000},async t=>{
  const f=await fixture(t,{projectSource:cornerSource,authoringPreview:{enabled:false,preferred:"client"}}),{page,errors}=await toolPage(f);
  await chooseTool(page,"Modify","Fillet");await setToolNumber(page,"Radius",2);
  await clickCurve(page,0);await clickCurve(page,1);
  await page.getByRole("combobox",{name:"Branch target",exact:true}).waitFor();
  await page.getByRole("combobox",{name:"Branch target",exact:true}).selectOption("0");
  assert.equal(await page.getByLabel("Alternate arc",{exact:true}).isChecked(),false);
  await until(()=>page.getByRole("button",{name:"Finish",exact:true}).isEnabled(),"Native Fillet did not enable Finish");
  const draft=await page.locator("canvas").evaluate(canvas=>canvas.__geosolvePresentedFrame);
  assert.equal(draft.provenance.scene,"provisional");
  assert.ok(draft.items.some(item=>/fillet/u.test(item.className)),"Native Fillet geometry must be painted before publication");
  assert.equal(f.runtime.host.snapshot().acceptedRevision,0);
  await page.getByRole("button",{name:"Finish",exact:true}).click();
  const created=await published(f,1,page);assert.match(created,/\$\.computed\.filletSet/u);assert.match(created,/radius:\s*mm\(2/u);
  await page.getByRole("button",{name:"Select",exact:true}).click();
  await select(page,"corner");
  await page.getByRole("button",{name:/Selected curves:.*Change to Construction/u}).click();
  const role=await published(f,2,page);assert.match(role,/role:\s*"construction"/u);assert.match(role,/\$\.computed\.filletSet/u);
  await page.getByRole("button",{name:"Undo",exact:true}).click();
  const undone=await published(f,3,page);assert.doesNotMatch(undone,/role:\s*"construction"/u);assert.match(undone,/\$\.computed\.filletSet/u);
  await page.getByRole("button",{name:"Redo",exact:true}).click();
  assert.match(await published(f,4,page),/role:\s*"construction"/u);assert.deepEqual(errors,[]);
});

for(const mode of ["client","server"])test(`shared tool parity ${mode} Offset keeps native preview and navigation responsive during held publication`,{timeout:120_000},async t=>{
  const held=deferred();let holding=false;
  const f=await fixture(t,{projectSource:cornerSource,authoringPreview:{enabled:mode==="server",preferred:mode},domainOptions:{beforeJob:async input=>{if(input.kind==="tool_operation"){holding=true;await held.promise;}}}});
  f.cleanup.push(()=>held.resolve());const {page,errors}=await toolPage(f,0,mode),requests=[];
  page.on("request",request=>{if(/\/api\/collaboration\/(?:commands|authoring-preview|scene)$/u.test(new URL(request.url()).pathname))requests.push(request.url());});
  await chooseTool(page,"Modify","Offset");await setToolNumber(page,"Offset distance",2);
  await clickCurve(page,0);await until(()=>page.getByRole("button",{name:"Clear picks",exact:true}).isEnabled(),"Offset did not retain its first native pick");
  await page.keyboard.press("Escape");
  await until(async()=>!await page.getByRole("button",{name:"Clear picks",exact:true}).isEnabled(),"First Escape did not reset native operands");
  assert.equal(await page.getByLabel("Offset distance",{exact:true}).inputValue(),"2");
  await page.keyboard.press("Escape");
  await until(async()=>await page.getByRole("group",{name:"Active tool options",exact:true}).count()===0,"Second Escape did not leave the empty tool");
  await chooseTool(page,"Modify","Offset");
  // Explorer expansion must feed one native selection event, preserving the
  // two ordered spans of this declaration as the chosen open chain.
  await select(page,"corner");
  await until(()=>page.getByRole("button",{name:"Finish",exact:true}).isEnabled(),"Explorer picks did not author a native Offset chain");
  await page.getByRole("button",{name:"Flip offset",exact:true}).click();
  await page.getByRole("button",{name:"Flip offset",exact:true}).click();
  assert.equal(f.runtime.host.snapshot().acceptedRevision,0);
  await page.getByRole("button",{name:"Finish",exact:true}).click();await until(()=>holding,"Offset commit did not reach held server job");
  await page.getByText("Solving…",{exact:true}).waitFor();
  const canvas=page.locator("canvas"),measurements=[];
  const drawing=()=>canvas.evaluate(canvas=>JSON.stringify({view:canvas.__geosolvePresentedFrame.viewBox,items:canvas.__geosolvePresentedFrame.items}));
  const count=requests.length;
  for(const action of ["wheel","pan","resize"]){
    const before=await drawing(),start=performance.now(),bounds=await canvas.boundingBox();
    if(action==="wheel"){await page.mouse.move(bounds.x+bounds.width*.6,bounds.y+bounds.height*.6);await page.mouse.wheel(0,-60);}
    if(action==="pan"){await page.mouse.down({button:"middle"});await page.mouse.move(bounds.x+bounds.width*.6+24,bounds.y+bounds.height*.6+18);await page.mouse.up({button:"middle"});}
    if(action==="resize")await page.setViewportSize({width:1480,height:900});
    await until(async()=>await drawing()!==before,`${action} waited for the Offset server job`,2000);
    const elapsedMs=performance.now()-start;measurements.push({action,elapsedMs});assert.ok(elapsedMs<500,`${mode} ${action} took ${elapsedMs} ms`);
    assert.equal(await canvas.evaluate(canvas=>canvas.__geosolvePresentedFrame.provenance.scene),"provisional");
  }
  assert.equal(requests.length,count,"Offset navigation issued a model, preview or scene RPC");assert.equal(f.runtime.host.snapshot().acceptedRevision,0);
  held.resolve();const source=await published(f,1,page);assert.match(source,/\$\.operation\.profileOffset/u);assert.match(source,/distance:\s*mm\(2/u);
  assert.deepEqual(errors,[]);t.diagnostic(JSON.stringify({mode,measurements}));
});

test("shared circle dragging paints continuously through pending acceptance and reaches peers promptly", {timeout:120_000}, async t=>{
  // The frame witness belongs to the actual canvas renderer. Geometry is never
  // synthesized here: the fixture uses native point preview and server replay.
  const held=deferred();let holding=false,holdNext=true;
  const f=await fixture(t,{authoringPreview:{enabled:false,preferred:"client"},domainOptions:{beforeJob:async input=>{
    if(input.kind==="point_gesture"&&holdNext){holdNext=false;holding=true;await held.promise;}
  }}});
  f.cleanup.push(()=>held.resolve());
  const browser=await f.browser(),pages=[],errors=[],notices=[];
  const observer=await f.client(0,"viewer");
  for(let i=0;i<2;i++){
    const context=await browser.newContext({viewport:{width:1280,height:900}}),page=await context.newPage();pages.push(page);
    await context.addInitScript(()=>{
      window.geosolveDragGpu=[];window.geosolveDragLongTasks=[];
      new PerformanceObserver(list=>{for(const e of list.getEntries())if(window.geosolveDragLongTasks.length<128)window.geosolveDragLongTasks.push({at:e.startTime,duration:e.duration});}).observe({type:"longtask",buffered:true});
      for(const method of ["compileShader","linkProgram","getProgramParameter","drawElements","drawArrays","getError","flush","checkFramebufferStatus","getShaderParameter"]){
        const original=WebGL2RenderingContext.prototype[method];
        WebGL2RenderingContext.prototype[method]=function(...args){const at=performance.now();try{return Reflect.apply(original,this,args);}finally{const elapsed=performance.now()-at;if(elapsed>5&&window.geosolveDragGpu.length<128)window.geosolveDragGpu.push({method,at,elapsed});}};
      }
    });
    page.on("pageerror",error=>errors.push(String(error)));
    await page.goto(`${f.origin}/?collaboration=1&authoringPreview=client#invite=editor-${i}`);await ready(page);
    await page.getByRole("group",{name:"Workspace layout",exact:true}).getByRole("button",{name:"design",exact:true}).click();await resized(page);
  }
  const page=pages[0],peer=pages[1],trials=[];
  for(let trial=0;trial<3;trial++){
    await page.bringToFront();
    const before=await observer.refresh(),peerBefore=await presented(peer),canvas=page.locator("canvas"),box=await canvas.boundingBox();
    const point=await canvas.evaluate(canvas=>{
      const frame=canvas.__geosolvePresentedFrame;
      const item=frame.items.filter(item=>item.interactive&&item.kind==="circle"&&item.className.split(/\s/u).includes("wb-point")).sort((a,b)=>a.center[0]-b.center[0])[0];
      if(!item)throw Error("Missing native draggable center");
      return {id:item.id,x:item.center[0]-frame.viewBox[0],y:item.center[1]-frame.viewBox[1]};
    });
    await page.evaluate(id=>{
      const trace={frames:[],events:[],active:true};window.geosolveDragTrace=trace;
      const event=e=>{if(trace.active&&trace.events.length<256)trace.events.push({kind:e.type,at:performance.now(),x:e.clientX,y:e.clientY});};
      window.addEventListener("pointermove",event,{capture:true});window.addEventListener("pointerup",event,{capture:true});
      const tick=()=>{
        if(!trace.active){window.removeEventListener("pointermove",event,{capture:true});window.removeEventListener("pointerup",event,{capture:true});return;}
        const canvas=document.querySelector("canvas"),frame=canvas?.__geosolvePresentedFrame;
        // Native predictions deliberately disable picking and use their own
        // draw IDs. Match the left centre in this non-crossing two-circle scene.
        const item=frame?.items.filter(item=>item.kind==="circle"&&item.className.split(/\s/u).includes("wb-point")).sort((a,b)=>a.center[0]-b.center[0])[0];
        if(frame&&item&&trace.frames.length<2400)trace.frames.push({at:performance.now(),id:item.id,x:item.center[0]-frame.viewBox[0],y:item.center[1]-frame.viewBox[1],provenance:frame.provenance});
        requestAnimationFrame(tick);
      };requestAnimationFrame(tick);
    },point.id);
    const x=box.x+point.x,y=box.y+point.y;
    await page.mouse.move(x,y);await page.mouse.down();
    // Distinct moves are separated by one display interval, exposing accepted
    // pointer/presence frames interleaved with the native authoring preview.
    for(let step=1;step<=12;step++){await page.mouse.move(x+step*3,y-step);await sleep(20);}
    const released=performance.now();await page.mouse.up();
    if(trial===0){
      await until(()=>holding,"The point commit did not reach the held server job");
      // Pointer and presence updates must not repaint the pre-drag scene while
      // its exact terminal is waiting for server authority.
      for(let i=0;i<5;i++){await page.mouse.move(x+60+i,y+40);await sleep(40);}
      held.resolve();
    }
    await until(()=>f.runtime.host.snapshot().acceptedRevision===before.authority.acceptedRevision+1,"Point drag did not publish",20_000);
    await until(async()=>await presented(peer)!==peerBefore,"Peer did not present accepted point movement",20_000);
    const releaseToPeerMs=performance.now()-released;
    await sleep(120);
    const trace=await page.evaluate(()=>{window.geosolveDragTrace.active=false;return {...window.geosolveDragTrace,gpu:window.geosolveDragGpu,longTasks:window.geosolveDragLongTasks};});
    const after=await observer.refresh();
    assert.notDeepEqual(after.document.pointTargets,before.document.pointTargets);
    assert.deepEqual(after.document.pointTargets.find(item=>item.target.address.owner.address.declaration==="other").position,[20,0]);
    const moved=trace.frames.findIndex(frame=>frame.x>point.x+2);
    const reversals=moved<0?[]:trace.frames.slice(moved+1).filter((frame,i)=>frame.x<trace.frames[moved+i].x-1);
    const moveEvents=trace.events.filter(event=>event.kind==="pointermove"&&event.x> x&&event.x<=x+36&&event.y<y);
    const localMs=moveEvents.map(event=>{
      const frame=trace.frames.find(frame=>frame.at>=event.at&&frame.x>=event.x-box.x-1);
      return frame?frame.at-event.at:Infinity;
    });
    const result={trial,releaseToPeerMs,localP95Ms:p95(localMs),reversals:reversals.length,trace};trials.push(result);
    progress("circle-drag-trial",{trial,releaseToPeerMs,localP95Ms:result.localP95Ms,reversals:result.reversals});
    notices.push(await page.getByRole("region",{name:"Shared document",exact:true}).innerText());
  }
  t.diagnostic(JSON.stringify({trials,errors,notices}));
  assert.deepEqual(errors,[]);
  assert.ok(notices.every(notice=>!notice.includes("Browsing model was replaced")));
  for(const trial of trials){
    assert.ok(trial.trace.frames.length>12,"Require actual display observations throughout drag and release");
    assert.equal(trial.reversals,0,`Trial ${trial.trial}: accepted coordinates flashed over the forward drag`);
    // Initial browser/GPU setup has the existing 500 ms navigation budget;
    // subsequent gestures must meet the tighter sustained interaction budget.
    const localBudget=trial.trial===0?500:200;
    assert.ok(trial.localP95Ms<localBudget,`Trial ${trial.trial}: event-to-painted-preview p95 ${trial.localP95Ms} ms`);
    if(trial.trial>0)assert.ok(trial.releaseToPeerMs<1000,`Warm release-to-peer ${trial.releaseToPeerMs} ms`);
  }
});

for(const mode of ["client","server"])test(`${mode} predicted geometry pans zooms and resizes while authoring and server completion are independently held`,{timeout:120_000},async t=>{
  const terminal=deferred();let serverHeld=false;
  const f=await fixture(t,{authoringPreview:{enabled:mode==="server",preferred:mode},domainOptions:{beforeJob:async input=>{if(input.kind==="point_gesture"){serverHeld=true;await terminal.promise;}}}});
  f.cleanup.push(()=>terminal.resolve());
  const browser=await f.browser(),context=await browser.newContext({viewport:{width:1280,height:900}});
  await context.addInitScript(()=>{
    window.geosolveHoldAuthoring=false;window.geosolveHeldAuthoring=false;
    const NativeWorker=window.Worker;
    window.Worker=class extends NativeWorker{
      constructor(url,options){
        super(url,options);
        if(!/collaboration-(?:authoring-worker|remote-authoring-renderer)/u.test(String(url)))return;
        this.addEventListener("message",event=>{
          if(!window.geosolveHoldAuthoring||event.data?.result?.kind!=="preview")return;
          event.stopImmediatePropagation();window.geosolveHeldAuthoring=true;
          // Hold delivery of one genuine native response, keeping the author's
          // queue occupied without manufacturing any solver/presentation data.
          window.geosolveReleaseAuthoring=()=>{window.geosolveHoldAuthoring=false;this.dispatchEvent(new MessageEvent("message",{data:event.data}));};
        });
      }
    };
  });
  const page=await context.newPage(),errors=[],requests=[],measurements=[];
  page.on("pageerror",error=>errors.push(String(error)));
  page.on("request",request=>{if(/\/api\/collaboration\/(?:commands|authoring-preview|scene)$/u.test(new URL(request.url()).pathname))requests.push(request.url());});
  await page.goto(`${f.origin}/?collaboration=1&authoringPreview=${mode}#invite=editor-0`);await ready(page);
  await page.getByRole("group",{name:"Workspace layout",exact:true}).getByRole("button",{name:"design",exact:true}).click();await resized(page);
  const canvas=page.locator("canvas"),box=await canvas.boundingBox();
  const location=await canvas.evaluate(canvas=>{
    const frame=canvas.__geosolvePresentedFrame,point=frame.items.filter(item=>item.kind==="circle"&&item.className.split(/\s/u).includes("wb-point")).sort((a,b)=>a.center[0]-b.center[0])[0];
    return [point.center[0]-frame.viewBox[0],point.center[1]-frame.viewBox[1]];
  });
  const x=box.x+location[0],y=box.y+location[1];
  await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+20,y-10);
  await until(()=>canvas.evaluate(c=>c.__geosolvePresentedFrame?.provenance.scene==="provisional"),"Missing genuine point preview");
  await page.evaluate(()=>{window.geosolveHoldAuthoring=true;});
  await page.mouse.move(x+40,y-20);
  await until(()=>page.evaluate(()=>window.geosolveHeldAuthoring),"Authoring response was not held");
  await page.mouse.up();
  const drawing=()=>canvas.evaluate(c=>JSON.stringify({view:c.__geosolvePresentedFrame.viewBox,items:c.__geosolvePresentedFrame.items.filter(i=>i.kind==="circle").map(i=>({center:i.center,radius:i.radius}))}));
  async function navigation(phase){
    const count=requests.length;
    for(const action of ["wheel","pan","resize"]){
      const before=await drawing(),start=performance.now();
      if(action==="wheel")await page.mouse.wheel(0,-60);
      if(action==="pan"){
        const bounds=await canvas.boundingBox();await page.mouse.move(bounds.x+bounds.width*.6,bounds.y+bounds.height*.6);
        await page.mouse.down({button:"middle"});await page.mouse.move(bounds.x+bounds.width*.6+24,bounds.y+bounds.height*.6+18);await page.mouse.up({button:"middle"});
      }
      if(action==="resize")await page.setViewportSize({width:phase==="prediction"?1320:1280,height:900});
      await until(async()=>await drawing()!==before,`${action} waited for held ${phase}`,2000);
      const elapsedMs=performance.now()-start;measurements.push({phase,action,elapsedMs});
      assert.equal(await canvas.evaluate(c=>c.__geosolvePresentedFrame.provenance.scene),"provisional");
      assert.ok(elapsedMs<500,`${phase} ${action} took ${elapsedMs} ms`);
    }
    assert.equal(requests.length,count,"Prediction navigation must issue zero model/scene/preview RPCs");
    assert.equal(f.runtime.host.snapshot().acceptedRevision,0);
  }
  await navigation("prediction");assert.equal(serverHeld,false);
  await page.evaluate(()=>window.geosolveReleaseAuthoring());
  await until(()=>serverHeld,"Finished authoring did not reach the held durable server job");
  await navigation("terminal");terminal.resolve();
  await until(()=>f.runtime.host.snapshot().acceptedRevision===1,"Held gesture did not publish after release");
  await until(()=>canvas.evaluate(c=>c.__geosolvePresentedFrame.provenance.scene==="accepted-presentation"),"Accepted scene did not replace provisional frame");
  assert.deepEqual(errors,[]);t.diagnostic(JSON.stringify({measurements,errors}));
});

test("four real browser editors retain independent navigation and typing during a ten-second solve",{timeout:180_000},async t=>{
  const release=deferred();let held=false,started=0,released=0,timer;
  const f=await fixture(t,{domainOptions:{beforeJob:async input=>{if(input.kind==="values"&&!held){held=true;started=performance.now();timer=afterMinimum(10_000,()=>{released=performance.now();release.resolve();});await release.promise;}}}});
  f.cleanup.push(()=>{timer?.();release.resolve();});
  const browser=await f.browser(),pages=[],errors=[],requests=[];
  for(let i=0;i<4;i++){
    const context=await browser.newContext({viewport:{width:1280,height:900}});await traceNavigation(context);const page=await context.newPage();pages.push(page);
    page.on("pageerror",error=>{errors.push(String(error));progress("page-error",{page:i,error:String(error)});});
    page.on("request",request=>{if(request.url().includes("/api/collaboration/"))requests.push({page:i,path:new URL(request.url()).pathname,at:performance.now()});});
    await page.goto(`${f.origin}/?collaboration=1#invite=editor-${i}`);await ready(page);
    if(process.env.GEOSOLVE_BROWSER_DIAGNOSTIC_NO_BACKDROP==="1")await page.addStyleTag({content:".geosolve-solving-overlay { backdrop-filter: none !important; }"});
    if(process.env.GEOSOLVE_BROWSER_DIAGNOSTIC_NO_TOOLBAR_BACKDROP==="1"){
      await page.addStyleTag({content:'[role="toolbar"][aria-label="Canvas view"] { backdrop-filter: none !important; }'});
      assert.equal(await page.getByRole("toolbar",{name:"Canvas view",exact:true}).evaluate(element=>getComputedStyle(element).backdropFilter),"none");
    }
    progress("editor-ready",{page:i});
  }
  await until(()=>f.runtime.transport.stats().streams===4,"Four event streams did not connect");
  const ids=await Promise.all(pages.map(page=>page.evaluate(()=>Object.entries(sessionStorage).find(([key])=>key.endsWith(".client"))?.[1])));
  assert.equal(new Set(ids).size,4);
  await select(pages[0],"bore");await select(pages[1],"other");
  assert.equal(await pages[2].locator('[data-navigation-row][aria-pressed="true"]').count(),0);
  await pages[1].getByRole("group",{name:"Workspace layout",exact:true}).getByRole("button",{name:"split",exact:true}).click();
  const editor=pages[1].locator(".cm-content[contenteditable=true]");await editor.waitFor();
  await pages[2].getByRole("group",{name:"Workspace layout",exact:true}).getByRole("button",{name:"split",exact:true}).click();
  const peerEditor=pages[2].locator(".cm-content[contenteditable=true]");await peerEditor.waitFor();
  await resized(pages[1]);await resized(pages[2]);
  const beforeOther=await presented(pages[2]);
  const radius=pages[0].getByRole("textbox",{name:"bore · radius value",exact:true});await radius.fill("5");await radius.press("Enter");
  progress("simple-edit-submitted");
  await until(()=>held,"Inspector did not reach server solver",20_000);
  await pages[0].getByText("Solving…",{exact:true}).waitFor({timeout:5000});
  const navigation=[],navigationStages=[],commandCount=requests.filter(item=>/\/(?:commands|authoring-preview|scene)$/u.test(item.path)).length;
  for(let i=0;i<10;i++){
    const page=pages[i%4];await page.bringToFront();
    assert.equal(await page.evaluate(()=>document.visibilityState),"visible");
    await page.evaluate(()=>{window.geosolveNavigationTrace=[];});
    const canvas=page.locator("canvas"),box=await canvas.boundingBox(),before=await presented(page),at=performance.now();
    await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5);const movedAt=performance.now();await page.mouse.wheel(0,i%2?-32:32);const wheeledAt=performance.now();
    await until(async()=>await presented(page)!==before,"Local zoom did not paint before server completion",2000);navigation.push(performance.now()-at);
    navigationStages.push({page:i%4,moveMs:movedAt-at,wheelApiMs:wheeledAt-movedAt,pollMs:performance.now()-wheeledAt,trace:await page.evaluate(()=>window.geosolveNavigationTrace),renderer:await canvas.evaluate(canvas=>canvas.__geosolveRendererDiagnostics)});
    if(i===0)assert.equal(await presented(pages[2]),beforeOther,"Another editor must retain its independent camera");
    progress("simple-navigation",{sample:i,elapsedMs:navigation.at(-1),...navigationStages.at(-1)});
  }
  assert.equal(requests.filter(item=>/\/(?:commands|authoring-preview|scene)$/u.test(item.path)).length,commandCount,"Navigation must issue zero model, preview or scene RPCs");
  await pages[1].bringToFront();await editor.click();await pages[1].keyboard.press("Control+Home");
  const textResponse=pages[1].waitForResponse(response=>new URL(response.url()).pathname.endsWith("/text")&&response.request().method()==="POST",{timeout:3000});
  const textStarted=performance.now();await pages[1].keyboard.insertText("// concurrent typing 😀\n");
  const acknowledged=await textResponse;assert.equal(acknowledged.status(),200);await acknowledged.finished();
  const textAck=performance.now()-textStarted;
  progress("simple-text-ack",{textAck});
  assert.match(f.runtime.source().snapshot().working.files["sketch.ts"],/concurrent typing 😀/u);
  await until(async()=>(await peerEditor.innerText()).includes("concurrent typing 😀"),"Remote CodeMirror did not receive typing before solve release",3000);
  assert.equal(released,0,"Shared text must synchronize before ten-second solve release");
  assert.equal(f.runtime.host.snapshot().acceptedRevision,0);
  await release.promise;assert.ok(released-started>=10_000);
  await until(()=>f.runtime.host.snapshot().acceptedRevision===1,"Held model edit did not publish",30_000);
  assert.match(f.runtime.source().snapshot().accepted.files["sketch.ts"],/mm\(5\)/u);
  assert.match(f.runtime.source().snapshot().working.files["sketch.ts"],/concurrent typing 😀/u);
  // The raw draft is independent of the accepted edit, and every context still
  // owns its native selection. Reload uses the original fenced tab identity.
  await pages[1].reload();await ready(pages[1]);
  progress("simple-reloaded");
  assert.equal(await pages[1].evaluate(()=>Object.entries(sessionStorage).find(([key])=>key.endsWith(".client"))?.[1]),ids[1]);
  assert.ok(beforeOther);assert.equal(errors.length,0,errors.join("\n"));
  t.diagnostic(JSON.stringify({editors:4,diagnosticNoBackdrop:process.env.GEOSOLVE_BROWSER_DIAGNOSTIC_NO_BACKDROP==="1",diagnosticNoToolbarBackdrop:process.env.GEOSOLVE_BROWSER_DIAGNOSTIC_NO_TOOLBAR_BACKDROP==="1",navigationMs:navigation,navigationStages,heldSolveMs:released-started,navigationP95Ms:p95(navigation),textAckMs:textAck,navigationRpc:0,transport:f.runtime.transport.stats(),rssBytes:process.memoryUsage().rss}));
  assert.ok(p95(navigation)<500,`p95 local navigation ${p95(navigation)} ms`);
  assert.ok(textAck<500,`shared text ACK ${textAck} ms`);
});

test("remote server preview leaves browser navigation local and its finished point gesture commits durably",{timeout:120_000},async t=>{
  const release=deferred(),f=await fixture(t,{authoringPreview:{enabled:true,preferred:"server"}});
  let held=false,finished;
  f.cleanup.push(()=>release.resolve());
  const observer=await f.client(0,"viewer"),initial=await observer.refresh();
  assert.deepEqual(initial.document.authoringPreview,{server:true,preferred:"server"});
  const browser=await f.browser(),context=await browser.newContext({viewport:{width:1280,height:900}});
  await traceNavigation(context);
  const page=await context.newPage(),errors=[],requests=[],commands=[],authorityEvents=[];
  f.cleanup.push(f.runtime.host.subscribe(envelope=>{if(envelope.payload.kind==="authority")authorityEvents.push(envelope.payload.record);}));
  page.on("pageerror",error=>errors.push(String(error)));
  page.on("request",request=>{
    const path=new URL(request.url()).pathname;
    if(path.endsWith("/authoring-preview"))requests.push(request.postDataJSON());
    if(path.endsWith("/commands"))commands.push(request.postDataJSON());
  });
  await page.route("**/api/collaboration/authoring-preview",async route=>{
    const request=route.request().postDataJSON(),response=await route.fetch(),body=await response.json();
    assert.equal(response.status(),200,JSON.stringify(body));
    // Execute the real authenticated native operation first, then delay only
    // delivery of its unchanged response. No geometry/solver result is mocked.
    if(request.action==="advance"&&!held){held=true;progress("remote-preview-held");await release.promise;}
    if(request.action==="finish")finished=body;
    await route.fulfill({response});
  });
  await page.goto(`${f.origin}/?collaboration=1&authoringPreview=server#invite=editor-0`);await ready(page);
  const canvas=page.locator("canvas"),box=await canvas.boundingBox();
  const location=await canvas.evaluate(canvas=>{
    const frame=canvas.__geosolvePresentedFrame;
    const point=frame.items.filter(item=>item.interactive&&item.kind==="circle"&&item.className.split(/\s/u).includes("wb-point")).sort((a,b)=>a.center[0]-b.center[0])[0];
    if(!point)throw Error("The accepted fixture must display its draggable bore center");
    return [(point.center[0]-frame.viewBox[0])/frame.viewBox[2],(point.center[1]-frame.viewBox[1])/frame.viewBox[3]];
  });
  const x=box.x+location[0]*box.width,y=box.y+location[1]*box.height;
  await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+35,y-25);
  await until(()=>held,"Point drag did not request an actual server preview",20_000);
  assert.equal(f.runtime.previews.stats().previews,1);
  const previewCount=requests.length,commandCount=commands.length,navigation=[];
  for(let i=0;i<10;i++){
    const before=await presented(page),at=performance.now();
    await page.mouse.wheel(0,i%2?-32:32);
    await until(async()=>await presented(page)!==before,"Local zoom waited for the held remote preview",2000);
    navigation.push(performance.now()-at);
  }
  assert.equal(requests.length,previewCount,"Camera changes must issue zero preview RPCs");
  assert.equal(commands.length,commandCount,"Camera changes must issue zero model RPCs");
  assert.equal(commandCount,0,"A provisional drag must not submit a durable edit");
  assert.equal(f.runtime.host.snapshot().acceptedRevision,0);
  assert.deepEqual(f.runtime.source().snapshot().accepted,initial.document.accepted);
  await page.mouse.up();release.resolve();
  await until(()=>f.runtime.host.snapshot().acceptedRevision===1||authorityEvents.some(record=>record.event.event==="finished"&&record.event.outcome.status!=="accepted"),"Remote gesture did not reach durable completion",30_000);
  assert.equal(f.runtime.host.snapshot().acceptedRevision,1,JSON.stringify({commands,authorityEvents,errors}));
  assert.equal(commands.length,1,"Finishing preview submits exactly one ordinary durable command");
  assert.equal(finished?.kind,"point");assert.equal(f.runtime.previews.stats().previews,0);
  const final=await observer.refresh(),point=final.document.pointTargets.find(item=>item.target.address?.owner.address.declaration==="bore");
  assert.deepEqual(point.position,finished.terminal.accepted_position,"Published position must match the native remote terminal");
  assert.notDeepEqual(point.position,[0,0]);
  assert.deepEqual(final.document.pointTargets.find(item=>item.target.address?.owner.address.declaration==="other").position,[20,0]);
  assert.notEqual(final.authority.acceptedInput,initial.authority.acceptedInput);
  // Native point edits persist their writable design override. Raw authored
  // coordinates remain the immutable source intent, matching local gestures.
  assert.equal(final.document.accepted.files["sketch.ts"],initial.document.accepted.files["sketch.ts"]);
  assert.notDeepEqual(final.document.model.design,initial.document.model.design);
  const acceptedInput=final.authority.acceptedInput;await page.reload();await ready(page);
  assert.equal((await observer.refresh()).authority.acceptedInput,acceptedInput);
  await context.close();observer.dispose();await f.runtime.close();
  const reopened=await openCollaborationRuntime(f.folder,{invitations:new Map([["reopen-viewer",{userId:"reopen-viewer",role:"viewer"}]])});
  f.cleanup.push(()=>reopened.close());
  const address=await reopened.listen(),coldClient=new CollaborationClient({baseUrl:`http://127.0.0.1:${address.port}/api/collaboration/`,inviteToken:"reopen-viewer",clientId:"reopen-viewer"});
  f.clients.push(coldClient);await coldClient.connect();
  const cold=await coldClient.refresh();
  assert.equal(cold.authority.acceptedInput,acceptedInput);
  assert.deepEqual(cold.document.model.design,final.document.model.design);
  assert.deepEqual(cold.document.pointTargets.find(item=>item.target.address?.owner.address.declaration==="bore").position,point.position);
  assert.deepEqual(errors,[]);
  t.diagnostic(JSON.stringify({serverPreview:true,heldNativeResponse:true,navigationMs:navigation,navigationP95Ms:p95(navigation),navigationPreviewRpc:0,navigationModelRpc:0,previewActions:requests.map(request=>request.action),durableCommands:commands.length,acceptedPosition:point.position}));
  assert.ok(p95(navigation)<500,`remote preview local navigation p95 ${p95(navigation)} ms`);
});

test("eight editors and twenty-four viewers synchronize bounded text and recover a disconnected subscriber",{timeout:180_000},async t=>{
  const f=await fixture(t),clients=[];let receivedBytes=0,sentBytes=0,sseReceivedBytes=0;
  const measuredFetch=async(input,init)=>{
    if(init?.body)sentBytes+=Buffer.byteLength(String(init.body));const response=await fetch(input,init);
    if(!response.headers.get("content-type")?.includes("text/event-stream")){const bytes=await response.arrayBuffer();receivedBytes+=bytes.byteLength;return new Response(bytes,{status:response.status,headers:response.headers});}
    const body=response.body.pipeThrough(new TransformStream({transform(chunk,controller){sseReceivedBytes+=chunk.byteLength;receivedBytes+=chunk.byteLength;controller.enqueue(chunk);}}));
    return new Response(body,{status:response.status,headers:response.headers});
  };
  const memoryBefore=process.memoryUsage().rss;
  for(let i=0;i<8;i++)clients.push(await f.client(i,"editor",measuredFetch));
  for(let i=0;i<24;i++)clients.push(await f.client(i,"viewer",measuredFetch));
  await until(()=>f.runtime.transport.stats().streams===32,"32 independent subscribers did not open");
  const replicas=await Promise.all(clients.slice(0,8).map(client=>f.replica(client))),acks=[];let maxIngress=0,maxSubscriberBytes=0;
  const sampling=setInterval(()=>{maxIngress=Math.max(maxIngress,f.runtime.host.snapshot().ingressCount);maxSubscriberBytes=Math.max(maxSubscriberBytes,f.runtime.transport.stats().subscriberBytes);},5);f.cleanup.push(()=>clearInterval(sampling));
  for(let round=0;round<3;round++)await Promise.all(replicas.map(async(replica,index)=>{
    const client=clients[index],basis=replica.capture().revision,delta=await client.textDelta(basis);replica.applyServerChanges(delta.changes.map(bytes=>Uint8Array.from(bytes)));
    const before=replica.capture().revision;replica.edit([{kind:"splice",path:"sketch.ts",start_utf16:0,delete_utf16:0,insert:`// editor ${index} round ${round} 😀\n`}]);
    const start=performance.now();await client.writeText(replica.changesSince(before),`load-${index}-${round}`);acks.push(performance.now()-start);
  }));
  const state=await clients[31].refresh();
  for(let i=0;i<8;i++)for(let round=0;round<3;round++)assert.ok(state.document.working.files["sketch.ts"].includes(`editor ${i} round ${round} 😀`));
  assert.equal(state.authority.acceptedRevision,0);assert.ok(clients.every(client=>client.pendingRequests.length===0));
  const stale=state.document.working.revision;await clients[31].leave();
  const source=replicas[0],old=source.capture().revision;source.edit([{kind:"splice",path:"sketch.ts",start_utf16:0,delete_utf16:0,insert:"// reconnect witness\n"}]);await clients[0].writeText(source.changesSince(old),"offline-witness");
  const recovered=await f.client(23,"viewer",measuredFetch),delta=await recovered.textDelta(stale);assert.ok(delta.changes.length>0);
  assert.match(recovered.state.document.working.files["sketch.ts"],/reconnect witness/u);
  assert.ok(p95(acks)<500,`32-client text ACK p95 ${p95(acks)} ms`);
  assert.ok(maxIngress<=128);assert.ok(maxSubscriberBytes<=32*256*1024);
  t.diagnostic(JSON.stringify({editors:8,viewers:24,textOperations:acks.length,textAckP95Ms:p95(acks),sentBytes,receivedBytes,sseReceivedBytes,maxIngress,maxSubscriberBytes,rssBeforeBytes:memoryBefore,rssAfterBytes:process.memoryUsage().rss,slowRecoveryChanges:delta.changes.length}));
});

test("a stalled TCP SSE reader stays bounded and reconnects without losing accepted or working state",{timeout:120_000},async t=>{
  const limit=128*1024,f=await fixture(t,{limits:{http:{maxSubscriberBytes:limit}}});
  const editors=await Promise.all(Array.from({length:8},(_,i)=>f.client(i))),events=[];
  const healthy=new CollaborationClient({baseUrl:f.baseUrl,inviteToken:"viewer-0",clientId:"healthy-reader",onEvent:event=>events.push(event)});
  f.clients.push(healthy);await healthy.connect();
  const joinResponse=await fetch(`${f.baseUrl}join`,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({protocol:1,inviteToken:"viewer-1",clientId:"stalled-reader"})});
  assert.equal(joinResponse.status,200);const joined=await joinResponse.json();
  const before=await healthy.refresh(),afterSequence=before.authority.latestSequence;
  let serverResponse,reader,networkRequest,sawBlocked=false,closedByServer=false,maxSlowBytes=0,maxWritableBytes=0,wireBytes=0;
  const observe=(request,response)=>{
    if(request.url.startsWith("/api/collaboration/events")&&request.headers.authorization===`Bearer ${joined.token}`){
      serverResponse=response;const socket=response.socket;response.on("close",()=>{wireBytes=socket.bytesWritten;closedByServer=true;});
    }
  };
  f.runtime.transport.server.on("request",observe);
  f.cleanup.push(()=>{f.runtime.transport.server.off("request",observe);reader?.destroy();networkRequest?.destroy();});
  await new Promise((resolve,reject)=>{
    networkRequest=get(`${f.baseUrl}events?after=${afterSequence}`,{headers:{Authorization:`Bearer ${joined.token}`}},response=>{
      reader=response;assert.equal(response.statusCode,200);response.on("error",()=>{});
      response.once("data",chunk=>{
        assert.match(chunk.toString(),/event: ready/u);
        // Pause the actual HTTP/TCP consumer. Do not override response.write,
        // writableLength, drain, or any subscriber/authority implementation.
        response.pause();response.socket.pause();resolve();
      });
    });networkRequest.on("error",reject);
  });
  await until(()=>f.runtime.transport.stats().streams===10,"Expected nine healthy streams and one stalled reader");
  const sample=()=>{
    if(serverResponse?.writableNeedDrain&&serverResponse.writableLength>0)sawBlocked=true;
    maxWritableBytes=Math.max(maxWritableBytes,serverResponse?.writableLength??0);
    maxSlowBytes=Math.max(maxSlowBytes,f.runtime.transport.stats().subscriberBytes);
  };
  const sampling=setInterval(sample,2);f.cleanup.push(()=>clearInterval(sampling));
  const selection=Array.from({length:24},(_,index)=>`${index}:`+"x".repeat(490));
  let rounds=0;
  // Different presence owners prevent coalescing from hiding a stalled socket;
  // each individual frame is below the configured subscriber byte limit.
  while(!closedByServer&&rounds<160){
    rounds++;
    for(const editor of editors){await editor.presence({sequence:rounds,selection});sample();if(closedByServer)break;}
  }
  t.diagnostic(JSON.stringify({stage:"slow-subscriber-bound",rounds,closedByServer,sawBlocked,maxWritableBytes,maxSlowBytes,wireBytes,transport:f.runtime.transport.stats()}));
  assert.ok(sawBlocked,"The real server TCP writable must stall before the subscriber is retired");
  assert.ok(closedByServer,"The paused reader must be retired at the subscriber buffer bound");
  assert.ok(maxWritableBytes>0);assert.ok(maxSlowBytes<=10*limit,`subscriber bytes ${maxSlowBytes}`);
  await until(()=>f.runtime.transport.stats().streams===9,"Only the slow subscriber should close");
  const replica=await f.replica(editors[0]),revision=replica.capture().revision;
  replica.edit([{kind:"splice",path:"sketch.ts",start_utf16:0,delete_utf16:0,insert:"// after genuine TCP backpressure 😀\n"}]);
  const at=performance.now();await editors[0].writeText(replica.changesSince(revision),"after-stall");const textAck=performance.now()-at;
  await until(()=>events.some(event=>event.type==="text"),"Healthy subscriber did not receive new working state");
  const live=await healthy.refresh();assert.match(live.document.working.files["sketch.ts"],/after genuine TCP backpressure 😀/u);
  assert.equal(live.authority.acceptedInput,before.authority.acceptedInput);
  // The same persistent tab identity rejoins after an actual bounded drop. Its
  // stale text revision still retrieves the native changes made while stalled.
  reader.destroy();networkRequest.destroy();
  const recovered=new CollaborationClient({baseUrl:f.baseUrl,inviteToken:"viewer-1",clientId:"stalled-reader"});f.clients.push(recovered);await recovered.connect();
  const delta=await recovered.textDelta(before.document.working.revision);assert.ok(delta.changes.length>0);
  assert.deepEqual(recovered.state.document.working,live.document.working);
  assert.equal(recovered.state.authority.acceptedInput,live.authority.acceptedInput);
  await until(()=>f.runtime.transport.stats().streams===10,"Recovered reader did not open a healthy stream");
  assert.ok(textAck<500,`healthy text ACK after slow subscriber drop ${textAck} ms`);
  t.diagnostic(JSON.stringify({realTcpPause:true,rounds,wireBytes,maxWritableBytes,maxSubscriberBytes:maxSlowBytes,perSubscriberLimit:limit,healthySubscribers:9,textAckMs:textAck,recoveryChanges:delta.changes.length}));
});

for(const sample of [
  {name:"manifold",projectFolder:resolve("examples/file-workspace-manifold"),parameter:"Channel width",initial:"12",edited:"11",minimumGeometry:50,acceptedPattern:/\$\.parameter\("channelWidth", mm\(11\)/u},
  {name:"Gridfinity",sourcePath:resolve("crates/geosolve-sketch-code/assets/bundled-samples/gridfinity-bin-section/sketch.ts"),dimension:"Plan depth",field:"planDepth · value",initial:"41.5",edited:"41",minimumGeometry:30,acceptedPattern:/dimension\.curveLength\("planDepth",\s*\{[^}]*value: mm\(41\)/u},
])test(`dense ${sample.name} keeps local navigation and source typing responsive during a shared dimension edit`,{timeout:180_000},async t=>{
  const release=deferred(),jobs=[];let started=0,released=0,timer;
  const f=await fixture(t,{projectFolder:sample.projectFolder,projectSource:sample.sourcePath?await readFile(sample.sourcePath,"utf8"):undefined,domainOptions:{timeoutMs:120_000,beforeJob:async input=>{
    jobs.push({kind:input.kind,at:performance.now()});progress("dense-domain-job",{sample:sample.name,kind:input.kind});
    if(input.kind==="values"&&!started){started=performance.now();timer=afterMinimum(10_000,()=>{released=performance.now();release.resolve();});await release.promise;}
  }}});
  f.cleanup.push(()=>{timer?.();release.resolve();});
  const sourceTimings=[];
  for(const method of ["stageUserTextChanges","commitStage","textChangesSince"]){const source=f.runtime.source(),original=source[method].bind(source);source[method]=(...args)=>{const at=performance.now();try{return original(...args);}finally{sourceTimings.push({method,at,duration:performance.now()-at});}};}
  const authorityEvents=[];f.cleanup.push(f.runtime.host.subscribe(envelope=>{if(envelope.payload.kind==="authority")authorityEvents.push(envelope.payload.record);}));
  const browser=await f.browser(),context=await browser.newContext({viewport:{width:1600,height:1000}});await traceNavigation(context);
  const page=await context.newPage(),errors=[],commands=[],textRequests=[],computationRequests=[];
  page.on("pageerror",error=>errors.push(String(error)));
  page.on("request",request=>{const path=new URL(request.url()).pathname;if(/\/(?:commands|authoring-preview|scene)$/u.test(path))computationRequests.push(path);if(path.endsWith("/commands"))commands.push(request.postDataJSON());if(path.endsWith("/text")&&request.method()==="POST")textRequests.push({at:performance.now(),bytes:Buffer.byteLength(request.postData()??"")});});
  await page.goto(`${f.origin}/?collaboration=1#invite=editor-0`);await ready(page);progress("dense-ready",{sample:sample.name});
  const before=f.runtime.source().snapshot(),canvas=page.locator("canvas");
  const geometryCount=await canvas.evaluate(canvas=>canvas.__geosolvePresentedFrame.items.filter(item=>item.interactive&&item.layer==="geometry").length);
  assert.ok(geometryCount>sample.minimumGeometry,`fixture must present the complete dense ${sample.name}`);
  if(sample.dimension)await select(page,sample.dimension);
  await page.getByRole("tab",{name:"Parameters",exact:true}).click();
  progress("dense-parameter-fields",{sample:sample.name,fields:await page.getByRole("textbox").evaluateAll(inputs=>inputs.map(input=>({name:input.getAttribute("aria-label"),value:input.value})).filter(input=>/width|depth/i.test(input.name??"")))});
  const width=page.getByRole("textbox",{name:sample.parameter??sample.field,exact:true}).first();
  await width.waitFor();assert.equal(await width.inputValue(),sample.initial);
  await width.fill(sample.edited);await width.press("Enter");
  try{await until(()=>started>0,`${sample.name} dimension edit did not enter solver`,20_000);}
  catch(error){t.diagnostic(JSON.stringify({stage:`${sample.name}-edit-ingress`,jobs,commands,errors,acceptedRevision:f.runtime.host.snapshot().acceptedRevision,notice:await page.getByRole("region",{name:"Shared document",exact:true}).innerText(),alerts:await page.getByRole("alert").allTextContents(),width:await width.inputValue()}));throw error;}
  await page.getByText("Solving…",{exact:true}).waitFor({timeout:5000});
  const commandCount=commands.length,computationCount=computationRequests.length,navigation=[];
  for(let i=0;i<10;i++){
    const box=await canvas.boundingBox(),beforeFrame=await presented(page),at=performance.now();
    await page.mouse.move(box.x+box.width*.48,box.y+box.height*.48);await page.mouse.wheel(0,i%2?30:-30);
    await until(async()=>await presented(page)!==beforeFrame,"Dense local zoom did not paint while edit held",2000);navigation.push(performance.now()-at);progress("dense-navigation",{sample:sample.name,index:i,elapsedMs:navigation.at(-1)});
  }
  await page.getByRole("group",{name:"Workspace layout",exact:true}).getByRole("button",{name:"split",exact:true}).click();
  const editor=page.locator(".cm-content[contenteditable=true]");await editor.waitFor();await editor.click();await page.keyboard.press("Control+Home");
  const pageTextStart=await page.evaluate(()=>performance.now());
  const textResponse=page.waitForResponse(response=>new URL(response.url()).pathname.endsWith("/text")&&response.request().method()==="POST",{timeout:3000});
  const textStarted=performance.now();await page.keyboard.insertText("// dense concurrent typing 😀\n");const keyboardFinished=performance.now();
  const acknowledged=await textResponse;assert.equal(acknowledged.status(),200);await acknowledged.finished();
  const textAck=performance.now()-textStarted;
  progress("dense-text-ack",{sample:sample.name,textAckMs:textAck,keyboardMs:keyboardFinished-textStarted,requests:textRequests.map(request=>({...request,delayMs:request.at-textStarted})),httpTiming:acknowledged.request().timing(),httpRequestDelayMs:acknowledged.request().timing().startTime+acknowledged.request().timing().requestStart-(performance.timeOrigin+textStarted),pageTextStart,workerTrace:await page.evaluate(()=>window.geosolveTextTrace),longTasks:await page.evaluate(start=>window.geosolveLongTasks.filter(task=>task.at+task.duration>=start),pageTextStart),sourceTimings:sourceTimings.filter(timing=>timing.at>=textStarted).map(timing=>({...timing,delayMs:timing.at-textStarted}))});
  assert.match(f.runtime.source().snapshot().working.files["sketch.ts"],/dense concurrent typing 😀/u);
  assert.equal(commands.length,commandCount,"Dense navigation/typing cannot submit a model command");
  assert.equal(computationRequests.length,computationCount,"Dense navigation/typing cannot request server preview or scene computation");
  assert.equal(released,0);assert.equal(f.runtime.host.snapshot().acceptedRevision,0);assert.deepEqual(f.runtime.source().snapshot().accepted,before.accepted);
  await release.promise;assert.ok(released-started>=10_000);
  try{await until(()=>f.runtime.host.snapshot().acceptedRevision===1||authorityEvents.some(record=>record.event.event==="finished"&&record.event.outcome.status!=="accepted"),`${sample.name} dimension edit did not publish`,90_000);assert.equal(f.runtime.host.snapshot().acceptedRevision,1,"Dense edit reached a terminal rejection");}
  catch(error){t.diagnostic(JSON.stringify({stage:`${sample.name}-edit-publication`,jobs,commands,authorityEvents,errors,authority:{acceptedRevision:f.runtime.host.snapshot().acceptedRevision,pendingCount:f.runtime.host.snapshot().pendingCount,needsRecovery:f.runtime.host.snapshot().needsRecovery},alerts:await page.getByRole("alert").allTextContents(),notice:await page.getByRole("region",{name:"Shared document",exact:true}).innerText(),navigationMs:navigation,textAckMs:textAck}));throw error;}
  const after=f.runtime.source().snapshot();assert.match(after.accepted.files["sketch.ts"],sample.acceptedPattern);
  assert.match(after.working.files["sketch.ts"],/dense concurrent typing 😀/u);
  if(sample.name==="Gridfinity")assert.match(after.accepted.files["sketch.ts"],/areKeyConstraintsByDefault: true/u);
  assert.deepEqual(Object.keys(after.accepted.files).sort(),Object.keys(before.accepted.files).sort());
  for(const path of Object.keys(before.accepted.files).filter(path=>path!=="sketch.ts"))assert.equal(after.accepted.files[path],before.accepted.files[path]);
  t.diagnostic(JSON.stringify({sample:sample.name,geometryCount,heldSolveMs:released-started,navigationMs:navigation,navigationP95Ms:p95(navigation),textAckMs:textAck,navigationRpc:0,sourceFiles:Object.keys(after.accepted.files).length}));
  assert.ok(p95(navigation)<500,`dense ${sample.name} navigation p95 ${p95(navigation)} ms`);assert.ok(textAck<500,`dense ${sample.name} text ACK ${textAck} ms`);
  assert.deepEqual(errors,[]);
});
