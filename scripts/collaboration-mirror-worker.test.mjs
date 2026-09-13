// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtemp, mkdir, open, readFile, writeFile, rename, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { Worker } from "node:worker_threads";
import { createTrustedSourceHost } from "../packages/geosolve-collaboration/dist/host.js";
import { createMirrorWorker } from "../packages/geosolve-cli/runtime/collaboration-mirror-worker-bridge.mjs";

const actor = value => new TextEncoder().encode(value);
const deferred = () => { let resolve; const promise = new Promise(done => { resolve = done; }); return {promise,resolve}; };
const sleep = ms => new Promise(resolve => setTimeout(resolve,ms));
const splice = (path,start,deleted,insert) => ({kind:"splice",path,start_utf16:start,delete_utf16:deleted,insert});
async function durable(path,value) {
  const handle = await open(`${path}.tmp`,"w",0o600);
  try { await handle.writeFile(JSON.stringify(value)); await handle.sync(); } finally { await handle.close(); }
  await rename(`${path}.tmp`,path);
  const directory = await open(dirname(path),"r"); try { await directory.sync(); } finally { await directory.close(); }
}
async function fixture(files={"main.ts":"a😀0 middle b0\n","other.ts":"const = ("}) {
  const folder = await mkdtemp(join(tmpdir(),"geosolve-mirror-worker-"));
  for(const [path,value] of Object.entries(files)){await mkdir(dirname(join(folder,path)),{recursive:true});await writeFile(join(folder,path),value);}
  await mkdir(join(folder,".geosolve"));
  const configuration={documentEpoch:"document-epoch",serverEpoch:"server-epoch",initialInput:"initial",files};
  let source=await createTrustedSourceHost({configuration,actor:actor("server")});
  let journal={},sequence=0,loseAck=false;
  const requests=[],workers=new Set(),journalPath=join(folder,".geosolve","test-journal.json");
  const persist=checkpoint=>durable(journalPath,{checkpoint,journal});
  await persist(source.checkpoint());
  const options={folder,documentId:"document",documentEpoch:configuration.documentEpoch,clientId:"external-mirror",userId:"disk-editor",
    readCommitted:async()=>({checkpoint:source.textCheckpoint(),snapshot:source.snapshot()}),
    admitWorkingEdits:async request=>{
      requests.push(structuredClone(request));const key=JSON.stringify(request.operation),previous=journal[key];
      if(previous){assert.deepEqual(request,previous.request);return previous.result;}
      let stage,result;
      try{stage=source.stageUserWorkingEdits(request.edits,request.operation,request.expectedRevision);result={status:"committed"};}
      catch(error){result={status:"rejected",reason:error.message};}
      journal[key]={request,result};await persist(stage?.checkpointJson??source.checkpoint());if(stage)source.commitStage(stage);
      if(loseAck){loseAck=false;throw Error("lost ACK after durable commit");}return result;
    },
  };
  return {folder,options,requests,source:()=>source,write:(path,value)=>writeFile(join(folder,path),value),read:path=>readFile(join(folder,path),"utf8"),
    async mirror(extra={}){const mirror=await createMirrorWorker({...options,...extra});workers.add(mirror);return mirror;},
    loseAck(){loseAck=true;},
    async shared(edits){const stage=source.stageUserWorkingEdits(edits,{userId:"bob",clientId:"bob-tab",requestId:`shared-${++sequence}`});await persist(stage.checkpointJson);source.commitStage(stage);},
    async restart(){for(const worker of workers)await worker.close();workers.clear();const saved=JSON.parse(await readFile(journalPath,"utf8"));journal=saved.journal;source.dispose();source=await createTrustedSourceHost({configuration:{...configuration,serverEpoch:`server-${++sequence}`},actor:actor(`server-${sequence}`),checkpointJson:saved.checkpoint});},
    async close(){for(const worker of workers)await worker.close();source.dispose();await rm(folder,{recursive:true,force:true});},
  };
}

test("worker mirror preserves native three-way Unicode and invalid source without changing accepted model",{timeout:30000},async()=>{
  const fx=await fixture();
  try{
    const mirror=await fx.mirror();assert.equal((await mirror.reconcile()).status,"synchronized");
    await fx.write("main.ts","a🦀1 middle b2\n");await fx.shared([splice("main.ts",5,6,"SHARED")]);
    const accepted=fx.source().snapshot().accepted;
    assert.equal((await mirror.reconcile()).status,"synchronized");
    assert.equal(await fx.read("main.ts"),"a🦀1 SHARED b2\n");assert.equal(await fx.read("other.ts"),"const = (");
    assert.deepEqual(fx.source().snapshot().accepted,accepted);assert.equal(fx.source().userHistory("disk-editor").undoCount,1);
  }finally{await fx.close();}
});

test("worker mailbox coalesces reconcile and leaves HTTP readable while durable admission is held",{timeout:30000},async()=>{
  const fx=await fixture({"main.ts":"a0b0"}),held=deferred(),release=deferred();
  const server=createServer((_request,response)=>response.end("readable"));
  await new Promise(resolve=>server.listen(0,"127.0.0.1",resolve));
  try{
    let calls=0;
    const mirror=await fx.mirror({admitWorkingEdits:async request=>{calls++;held.resolve();await release.promise;return fx.options.admitWorkingEdits(request);}});
    await mirror.reconcile();await fx.write("main.ts","a1b0");
    const work=mirror.reconcile();assert.equal(mirror.reconcile(),work);for(let i=0;i<100;i++)assert.equal(mirror.reconcile(),work);
    await held.promise;
    const start=performance.now(),response=await fetch(`http://127.0.0.1:${server.address().port}/`);
    assert.equal(await response.text(),"readable");assert.ok(performance.now()-start<500,"held admission cannot block HTTP event delivery");
    assert.equal(fx.source().snapshot().working.files["main.ts"],"a0b0");assert.equal(calls,1);
    release.resolve();assert.equal((await work).status,"synchronized");assert.equal(fx.source().snapshot().working.files["main.ts"],"a1b0");
  }finally{release.resolve();await fx.close();await new Promise(resolve=>server.close(resolve));}
});

test("worker close drains held authority callback, preserves pending admission, and restart deduplicates exact bytes",{timeout:30000},async()=>{
  const fx=await fixture({"main.ts":"a0"}),held=deferred(),release=deferred();
  try{
    const mirror=await fx.mirror({admitWorkingEdits:async request=>{held.resolve();await release.promise;return fx.options.admitWorkingEdits(request);}});
    await mirror.reconcile();await fx.write("main.ts","a1");const work=mirror.reconcile();void work.catch(()=>{});await held.promise;
    let closed=false;const closing=mirror.close().then(()=>{closed=true;});await sleep(30);assert.equal(closed,false,"close must not abandon a callback that can durably write");
    release.resolve();await assert.rejects(work,/closed after callback/u);await closing;
    const path=join(fx.folder,".geosolve/collaboration-mirror/manifest.json"),saved=await readFile(path,"utf8");
    assert.equal(JSON.parse(saved).pending.kind,"admission");assert.equal(fx.source().snapshot().working.files["main.ts"],"a1");
    await sleep(30);assert.equal(await readFile(path,"utf8"),saved,"closed worker has no orphan file writer");
    await fx.restart();const recovered=await fx.mirror();assert.equal((await recovered.reconcile()).status,"synchronized");
    assert.equal(fx.requests.length,2);assert.deepEqual(fx.requests[0],fx.requests[1]);assert.equal(fx.source().userHistory("disk-editor").undoCount,1);
  }finally{release.resolve();await fx.close();}
});

test("worker callback exception preserves lost ACK request across native host restart",{timeout:30000},async()=>{
  const fx=await fixture({"main.ts":"a0"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();await fx.write("main.ts","a1");fx.loseAck();
    await assert.rejects(mirror.reconcile(),/lost ACK/u);assert.equal((await mirror.status()).pendingPhase,"admission");
    await fx.restart();assert.equal((await(await fx.mirror()).reconcile()).status,"synchronized");
    assert.equal(fx.requests.length,2);assert.deepEqual(fx.requests[0],fx.requests[1]);
  }finally{await fx.close();}
});

test("worker rejects unknown request protocol and exits without calling an authority callback",{timeout:10000},async()=>{
  const worker=new Worker(new URL("../packages/geosolve-cli/runtime/collaboration-mirror-worker.mjs",import.meta.url),{workerData:{},execArgv:[]});
  const messages=[];worker.on("message",message=>messages.push(message));
  const exited=new Promise((resolve,reject)=>{worker.once("exit",resolve);worker.once("error",reject);});
  worker.postMessage({protocol:"geosolve-mirror-worker-v1",kind:"request",id:1,method:"arbitrary-write"});
  await exited;assert.ok(messages.some(message=>message.kind==="fatal"));assert.equal(messages.some(message=>message.kind==="callback"),false);
});

test("dense native checkpoint/range work leaves root timers and HTTP responsive",{timeout:60000},async(t)=>{
  // A real native text corpus; root snapshots are captured outside measurement.
  const size=24_000,original="a".repeat(size)+" middle "+"b".repeat(size);
  const fx=await fixture({"main.ts":original}),held=deferred(),release=deferred();
  const server=createServer((_request,response)=>response.end("ready"));
  await new Promise(resolve=>server.listen(0,"127.0.0.1",resolve));
  let timer;
  try{
    let snapshot=await fx.options.readCommitted(),ticks=0,maxGap=0,last=0,measure=false;
    const mirror=await fx.mirror({
      readCommitted:()=>snapshot,
      admitWorkingEdits:async request=>{
        measure=false;held.resolve();await release.promise;
        const result=await fx.options.admitWorkingEdits(request);snapshot=await fx.options.readCommitted();return result;
      },
    });
    await mirror.reconcile();await fx.write("main.ts","X"+original.slice(1,-1)+"Y");
    measure=true;last=performance.now();
    timer=setInterval(()=>{if(!measure)return;const now=performance.now();maxGap=Math.max(maxGap,now-last);last=now;ticks++;},5);
    const work=mirror.reconcile();void work.catch(()=>{});
    const start=performance.now(),response=await fetch(`http://127.0.0.1:${server.address().port}/`);
    assert.equal(await response.text(),"ready");const httpMs=performance.now()-start;
    await held.promise;clearInterval(timer);
    assert.ok(ticks>0,"native checkpoint/range processing overlaps root timer delivery");
    assert.ok(httpMs<500,`root HTTP ${httpMs}ms`);assert.ok(maxGap<500,`root timer gap ${maxGap}ms`);
    t.diagnostic(JSON.stringify({sourceScalars:original.length,httpMs,maxRootTimerGapMs:maxGap,rootTimerTicks:ticks}));
    release.resolve();assert.equal((await work).status,"synchronized");
    assert.equal(await fx.read("main.ts"),"X"+original.slice(1,-1)+"Y");
  }finally{clearInterval(timer);release.resolve();await fx.close();await new Promise(resolve=>server.close(resolve));}
});

test("deadline termination still drains the trusted callback and keeps exact admission retryable",{timeout:15000},async()=>{
  const fx=await fixture({"main.ts":"a0"}),held=deferred(),release=deferred();
  try{
    const mirror=await fx.mirror({timeoutMs:1000,admitWorkingEdits:async request=>{held.resolve();await release.promise;return fx.options.admitWorkingEdits(request);}});
    await mirror.reconcile();await fx.write("main.ts","a1");const work=mirror.reconcile();void work.catch(()=>{});await held.promise;
    await assert.rejects(work,error=>error.code==="mirror_timeout");
    let closed=false;const closing=mirror.close().then(()=>{closed=true;});await sleep(30);
    assert.equal(closed,false,"termination cannot abandon an active root persistence callback");
    release.resolve();await closing;
    const manifest=await readFile(join(fx.folder,".geosolve/collaboration-mirror/manifest.json"),"utf8");
    assert.equal(JSON.parse(manifest).pending.kind,"admission");
    await fx.restart();const recovered=await fx.mirror();assert.equal((await recovered.reconcile()).status,"synchronized");
    assert.equal(fx.requests.length,2);assert.deepEqual(fx.requests[0],fx.requests[1]);
    assert.equal(fx.source().userHistory("disk-editor").undoCount,1);
  }finally{release.resolve();await fx.close();}
});

test("worker callback replies require exact current request and callback identities",{timeout:10000},async()=>{
  const folder=await mkdtemp(join(tmpdir(),"geosolve-mirror-protocol-"));
  const worker=new Worker(new URL("../packages/geosolve-cli/runtime/collaboration-mirror-worker.mjs",import.meta.url),{
    workerData:{folder,documentId:"document",documentEpoch:"epoch",userId:"user",clientId:"client"},execArgv:[],
  });
  const messages=[],exited=new Promise((resolve,reject)=>{worker.once("exit",resolve);worker.once("error",reject);});
  try{
    worker.on("message",message=>{
      messages.push(message);
      if(message.kind==="result"&&message.id===1&&message.ok)worker.postMessage({protocol:"geosolve-mirror-worker-v1",kind:"request",id:2,method:"reconcile"});
      if(message.kind==="callback")worker.postMessage({protocol:"geosolve-mirror-worker-v1",kind:"callback_result",id:message.id+1,requestId:message.requestId,ok:true,value:{}});
    });
    worker.postMessage({protocol:"geosolve-mirror-worker-v1",kind:"request",id:1,method:"initialize"});
    await exited;assert.ok(messages.some(message=>message.kind==="fatal"&&message.error.includes("Mismatched")));
    assert.equal(messages.filter(message=>message.kind==="callback").length,1);
  }finally{await worker.terminate();await rm(folder,{recursive:true,force:true});}
});

test("worker admits external file deletion through the native remove_file vocabulary", {timeout:30000}, async()=>{
  const fx=await fixture({"main.ts":"kept", "removed.ts":"notes"});
  try {
    const mirror=await fx.mirror();await mirror.reconcile();
    await rm(join(fx.folder,"removed.ts"));
    assert.equal((await mirror.reconcile()).status,"synchronized");
    assert.equal(fx.source().snapshot().working.files["removed.ts"],undefined);
    assert.equal(fx.requests[0].edits[0].kind,"remove_file");
    assert.equal(fx.source().userHistory("disk-editor").undoCount,1);
  }finally{await fx.close();}
});
