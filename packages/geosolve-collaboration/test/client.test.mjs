// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { CollaborationClient, readCollaborationEvents } from "../dist/client.js";
const encoder=new TextEncoder();
function deferred(){let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return{promise,resolve,reject};}
const json=value=>new Response(JSON.stringify(value),{headers:{"Content-Type":"application/json"}});
const connection={protocol:1,documentId:"doc",documentEpoch:"epoch",serverEpoch:"server",sessionId:"session",clientId:"tab",userId:"alice",role:"editor"};
const authority={acceptedRevision:0,acceptedInput:"input",latestSequence:0,pendingCount:0,ledgerBytes:0,needsRecovery:false,hasPendingStage:false};
function fixture(handlers={}){
  const calls=[],saved=[],eventStreams=[];
  const fetch=async(input,options={})=>{
    const route=new URL(input).pathname.split("/").at(-1),body=options.body?JSON.parse(options.body):undefined;
    calls.push({route,body});
    if(handlers[route])return handlers[route](body,options);
    if(route==="join")return json({connection,token:"a".repeat(64)});
    assert.equal(options.headers.Authorization,`Bearer ${"a".repeat(64)}`);
    if(route==="state")return json({authority,document:{},participants:[],presence:[]});
    if(route==="events")return new Response(new ReadableStream({start(controller){eventStreams.push(controller);options.signal.addEventListener("abort",()=>{try{controller.error(Error("aborted"));}catch{}});}}),{headers:{"Content-Type":"text/event-stream"}});
    if(route==="commands")return json({receipt:{operation:{userId:"alice",clientId:"tab",requestId:body.requestId},admission:1,outcome:null}});
    if(route==="text")return json({sourceSequence:1,workingRevision:{heads:[]}});
    throw Error(`Unexpected route ${route}`);
  };
  const client=new CollaborationClient({baseUrl:"http://test/api/collaboration/",inviteToken:"invite",clientId:"tab",fetch,savePending:p=>saved.push(structuredClone(p))});
  return{client,calls,saved,eventStreams,fetch};
}

test("bounded SSE decodes split UTF-8 and CRLF and rejects truncated/malformed/oversized frames",async()=>{
  const bytes=encoder.encode('event: presence\r\ndata: {"cursor":"😀"}\r\n\r\nevent: heartbeat\ndata: {}\n\n');
  const stream=new ReadableStream({start(c){for(const byte of bytes)c.enqueue(Uint8Array.of(byte));c.close();}});
  const events=[];for await(const event of readCollaborationEvents(stream))events.push(event);
  assert.deepEqual(events,[{type:"presence",data:{cursor:"😀"}},{type:"heartbeat",data:{}}]);
  for(const [source,maximum] of [['data: {}\n',1024],['data: not json\n\n',1024],['data: "too big"\n\n',8]]){
    const invalid=new ReadableStream({start(c){c.enqueue(encoder.encode(source));c.close();}});
    await assert.rejects(async()=>{for await(const _ of readCollaborationEvents(invalid,maximum)){assert.fail("Invalid frame emitted");}});
  }
});

test("text then Apply then later typing preserves exact admission order without waiting for a model result",async()=>{
  const hold=deferred(),started=deferred();let textCount=0;
  const f=fixture({text:async()=>{if(++textCount===1){started.resolve();await hold.promise;}return json({sourceSequence:textCount});}});
  try{
    await f.client.connect();
    const bytes=Uint8Array.of(1,2),first=f.client.writeText([bytes],"first");bytes[0]=99;
    await started.promise;
    const apply=f.client.submit({kind:"apply",basisRevision:0,payload:{}},"apply");
    const later=f.client.writeText([Uint8Array.of(3)],"later");
    await Promise.resolve();
    assert.equal(f.calls.filter(x=>x.route==="commands").length,0);
    assert.deepEqual(f.client.pendingRequests.map(x=>x.requestId),["first","apply","later"]);
    hold.resolve();await Promise.all([first,apply,later]);
    assert.deepEqual(f.calls.filter(x=>["text","commands"].includes(x.route)).map(x=>x.body.requestId),["first","apply","later"]);
    assert.deepEqual(f.calls.find(x=>x.route==="text").body.changes,[[1,2]]);
    assert.ok(f.saved.some(p=>p.requests.some(x=>x.requestId==="first")));
    assert.deepEqual(f.client.pendingRequests.map(x=>x.requestId),["apply"]);
  }finally{hold.resolve();f.client.dispose();}
});

test("lost ACK retains exact pending bytes and replays the same ID on reconnect",async()=>{
  let sends=0;const f=fixture({text:async()=>{if(++sends===1)throw Error("Lost response after fsync");return json({sourceSequence:1});}});
  try{
    await f.client.connect();await assert.rejects(f.client.writeText([Uint8Array.of(4,5)],"durable"),/Lost response/);
    assert.equal(f.client.pendingRequests.length,1);
    const original=f.client.checkpoint();assert.equal(original.requests[0].requestId,"durable");
    await f.client.retry();assert.equal(f.client.pendingRequests.length,0);
    const text=f.calls.filter(x=>x.route==="text");assert.equal(text.length,2);assert.deepEqual(text[0],text[1]);
    assert.equal(f.saved.at(-1).requests.length,0);
  }finally{f.client.dispose();}
});

test("durable text refusal clears only that request and permits later typing in admission order",async()=>{
  const f=fixture({text:async(body)=>body.action==="undo"
    ?new Response(JSON.stringify({error:{code:"text_rejected",message:"Another author owns this contribution"}}),{status:400,headers:{"Content-Type":"application/json"}})
    :json({sourceSequence:2})});
  try{
    await f.client.connect();
    const undo=f.client.undoText(false,"refused"),typing=f.client.writeText([Uint8Array.of(7)],"later");
    await assert.rejects(undo,/Another author/);await typing;
    assert.deepEqual(f.client.pendingRequests,[]);
    assert.deepEqual(f.calls.filter(x=>x.route==="text").map(x=>x.body.requestId),["refused","later"]);
    assert.equal(f.client.connected,true);
  }finally{f.client.dispose();}
});

test("pending work cannot be sent into another document and persistence failure prevents transmission",async()=>{
  const f=fixture();await f.client.connect();await f.client.submit({kind:"undo",basisRevision:0,payload:{}},"undo");
  const pending=f.client.checkpoint();f.client.dispose();
  const mismatch=new CollaborationClient({baseUrl:"http://test/api/collaboration/",inviteToken:"invite",clientId:"tab",pending,fetch:async(input,options)=>new URL(input).pathname.endsWith("join")?json({connection:{...connection,documentEpoch:"other"},token:"a".repeat(64)}):f.fetch(input,options)});
  try{await assert.rejects(mismatch.connect(),/another document/);assert.equal(mismatch.pendingRequests.length,1);}finally{mismatch.dispose();}
  const blocked=new CollaborationClient({baseUrl:"http://test/api/collaboration/",inviteToken:"invite",clientId:"tab",fetch:f.fetch,savePending:()=>{throw Error("storage full");}});
  try{await blocked.connect();const count=f.calls.length;await assert.rejects(blocked.writeText([Uint8Array.of(1)],"unsaved"),/storage full/);assert.equal(f.calls.length,count);assert.equal(blocked.pendingRequests.length,1);}finally{blocked.dispose();}
});

test("authoring previews are abortable ephemeral RPCs and never enter the durable retry outbox",async()=>{
  let previewSignal;
  const started=deferred();
  const f=fixture({"authoring-preview":async(_body,options)=>{
    assert.equal(options.headers.Authorization,`Bearer ${"a".repeat(64)}`);
    previewSignal=options.signal;started.resolve();
    return await new Promise((_resolve,reject)=>options.signal.addEventListener("abort",()=>reject(Error("preview aborted")),{once:true}));
  }});
  try{
    await f.client.connect();
    const savedBefore=f.saved.length,abort=new AbortController();
    const pending=f.client.authoringPreview({action:"begin",basis:{revision:0}},abort.signal);
    const rejection=assert.rejects(pending,/preview aborted/);
    await started.promise;
    await f.client.writeText([Uint8Array.of(7)],"typing-during-preview");
    assert.deepEqual(f.client.pendingRequests,[]);
    abort.abort();await rejection;
    assert.equal(previewSignal.aborted,true);
    await f.client.retry();
    assert.equal(f.calls.filter(call=>call.route==="authoring-preview").length,1);
    assert.ok(f.saved.slice(savedBefore).every(checkpoint=>checkpoint.requests.every(request=>request.kind!=="authoring-preview")));
    assert.equal(f.client.connected,true);
  }finally{f.client.dispose();}
});
