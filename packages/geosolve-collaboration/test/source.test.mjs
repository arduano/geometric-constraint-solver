// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createTrustedSourceHost, createDocumentAuthorityHost } from "../dist/host.js";
import { createSharedText } from "../dist/index.js";
const actor=name=>new TextEncoder().encode(name);
const files={"main.ts":"const width = 12;\n","helper.ts":"export const height = 8;\n"};
const configuration=serverEpoch=>({documentEpoch:"doc-1",serverEpoch,initialInput:"initial-model",files});
const open=()=>createTrustedSourceHost({configuration:configuration("process-1"),actor:actor("server")});
const digest=text=>createHash("sha256").update(text).digest("hex");
function patch(source,before,after){const start=source.indexOf(before);return{baseSourceDigest:digest(source),candidateSourceDigest:digest(source.replace(before,after)),edits:[{start,end:start+before.length,expected:before,replacement:after}]};}
const widthPatch=()=>patch(files["main.ts"],"12","14");
async function edit(host,path,start,deleted,insert){return host.editWorking([{kind:"splice",path,start_utf16:start,delete_utf16:deleted,insert}],host.snapshot().working.revision,async()=>{});}
function deferred(){let resolve;const promise=new Promise(yes=>{resolve=yes;});return{promise,resolve};}

test("actual WASM source text ACK awaits persistence while model preparation retains independent basis",async()=>{
  const host=await open();const client=await createSharedText({actor:actor("alice"),checkpoint:host.textCheckpoint()});
  try{
    const prepared=host.prepareCanvasUpdate([{path:"main.ts",patch:widthPatch()}]);
    const before=host.snapshot();const basis=client.capture().revision;
    client.edit([{kind:"splice",path:"helper.ts",start_utf16:0,delete_utf16:0,insert:"import { incomplete\n"}]);
    const persistence=deferred();let returned=false,stored;
    const operation=host.receiveTextChanges(client.changesSince(basis),actor("alice"),async write=>{stored=write;await persistence.promise;}).then(value=>{returned=true;return value;});
    assert.equal(host.snapshot().hasPendingStage,true);
    assert.deepEqual(host.snapshot().working,before.working);
    assert.equal(host.snapshot().sequence,0);
    assert.throws(()=>host.captureApply(),/pending/);
    assert.throws(()=>host.dispose(),/pending/);
    await new Promise(resolve=>setImmediate(resolve));assert.equal(returned,false);
    persistence.resolve();await operation;
    assert.equal(host.snapshot().sequence,1);
    assert(host.snapshot().working.files["helper.ts"].startsWith("import { incomplete"));
    assert.deepEqual(host.snapshot().accepted,before.accepted);
    assert.throws(()=>host.stageValidatedPublication(prepared,"model-14",before.working.revision,[{kind:"reconciled",path:"main.ts",patch:widthPatch()}]),/text changed/);
    const terminal=host.stageValidatedPublication(prepared,"model-14",host.snapshot().working.revision,[{kind:"reconciled",path:"main.ts",patch:widthPatch()}]);
    assert.equal(host.snapshot().accepted.modelRevision,0);
    assert.equal(terminal.accepted.modelRevision,1);
    assert.throws(()=>host.commitStage({...terminal}),/stale/);
    host.commitStage(terminal);
    assert.equal(host.snapshot().accepted.files["main.ts"],"const width = 14;\n");
    assert(host.snapshot().working.files["helper.ts"].startsWith("import { incomplete"));
    const restored=await createTrustedSourceHost({configuration:configuration("process-2"),actor:actor("restored"),checkpointJson:stored.checkpointJson});
    try{assert.equal(restored.snapshot().sequence,1);assert.equal(restored.snapshot().accepted.modelRevision,0);}finally{restored.dispose();}
  }finally{host.dispose();client.dispose();}
});

test("durable immutable Apply captures authenticate historical heads/files/IDs and leave later typing unapplied",async()=>{
  const host=await open();let restored;
  try{
    await edit(host,"main.ts",14,2,"14");const capture=host.captureApply();
    await edit(host,"main.ts",14,2,"16");
    restored=await createTrustedSourceHost({configuration:configuration("process-2"),actor:actor("restored"),checkpointJson:host.checkpoint()});
    assert.throws(()=>restored.prepareApplyUpdate(capture,capture.working.files),/capture/);
    const old=restored.restoreApplyCapture(capture.captureJson,capture.acceptedBasis);
    assert.deepEqual(old.working,capture.working);assert.deepEqual(old.fileIds,capture.fileIds);
    assert.equal(restored.applyNeedsRebase(old),false);
    const prepared=restored.prepareApplyUpdate(old,old.working.files);
    const terminal=restored.stageValidatedPublication(prepared,"model-captured14",restored.snapshot().working.revision,[{kind:"captured",path:"main.ts"}]);
    restored.commitStage(terminal);
    assert.equal(restored.snapshot().accepted.files["main.ts"],"const width = 14;\n");
    assert.equal(restored.snapshot().working.files["main.ts"],"const width = 16;\n");
    assert.equal(restored.applyNeedsRebase(old),true);
    const forged=JSON.parse(capture.captureJson);forged.files["main.ts"]="forged";
    assert.throws(()=>restored.restoreApplyCapture(JSON.stringify(forged),capture.acceptedBasis));
    assert.throws(()=>restored.restoreApplyCapture(capture.captureJson,{...capture.acceptedBasis,acceptedInput:"forged-basis"}));
    restored.release(old);assert.throws(()=>restored.applyNeedsRebase(old),/capture/);
  }finally{host.dispose();restored?.dispose();}
});

test("canvas under invalid working syntax retains pending notice and paired authority/source durability",async()=>{
  const source=await open();const authority=await createDocumentAuthorityHost({configuration:{documentId:"doc",documentEpoch:"doc-1",serverEpoch:"process-1",initialInput:"initial-model"}});
  try{
    const connection=authority.connect({userId:"alice",role:"editor"},"a","session-a");
    const records=[];await authority.admit({connection,requestId:"width14",command:{kind:"semantic",basisRevision:0,payload:{width:14}}},async write=>records.push(JSON.parse(write.recordJson)));
    const worker=authority.beginNext();const prepared=source.prepareCanvasUpdate([{path:"main.ts",patch:widthPatch()}]);
    await edit(source,"main.ts",0,17,"const width =");const invalid=source.snapshot().working;
    const sourceWrite=source.stageValidatedPublication(prepared,"model-14",invalid.revision,[{kind:"pending",path:"main.ts",reason:"target syntax incomplete"}]);
    const authorityWrite=authority.stageValidatedCompletion(worker,{status:"accepted",acceptedInput:"model-14",summary:"validated width"});
    assert.equal(source.snapshot().accepted.modelRevision,0);assert.equal(authority.snapshot().acceptedRevision,0);
    const transaction={source:sourceWrite.checkpointJson,record:JSON.parse(authorityWrite.recordJson)};
    // Real host writes this pair plus accepted model/design in one synced transaction.
    await Promise.resolve();records.push(transaction.record);
    source.commitStage(sourceWrite);const receipt=authority.commitStage(authorityWrite);
    assert.equal(receipt.outcome.accepted_input,source.snapshot().accepted.acceptedInput);
    assert.deepEqual(source.snapshot().working,invalid);
    assert.equal(source.snapshot().pendingNotices.length,1);
    const recovered=await createTrustedSourceHost({configuration:configuration("process-2"),actor:actor("new"),checkpointJson:transaction.source});
    try{assert.deepEqual(recovered.snapshot(),source.snapshot());}finally{recovered.dispose();}
  }finally{source.dispose();authority.dispose();}
});

test("source uncertain write recovery, actor guard and capture resource bounds",async()=>{
  const host=await open();let durable;
  try{
    const captures=[];for(let i=0;i<32;i++)captures.push(host.captureApply());
    assert.throws(()=>host.captureApply(),/capacity/);host.release(captures.pop());host.release(host.captureApply());
    for(const capture of captures)host.release(capture);
    const client=await createSharedText({actor:actor("alice"),checkpoint:host.textCheckpoint()});
    try{
      const basis=client.capture().revision;client.edit([{kind:"splice",path:"main.ts",start_utf16:14,delete_utf16:2,insert:"16"}]);
      const changes=client.changesSince(basis);assert.throws(()=>host.stageTextChanges(changes,actor("forged")),/writer/);
      await assert.rejects(host.receiveTextChanges(changes,actor("alice"),async write=>{durable=write.checkpointJson;throw Error("source fsync uncertain");}),/uncertain/);
      assert.equal(host.snapshot().needsRecovery,true);assert.equal(host.snapshot().sequence,0);assert.throws(()=>host.captureApply(),/recovery/);
      const recovered=await createTrustedSourceHost({configuration:configuration("process-2"),actor:actor("new"),checkpointJson:durable});
      try{assert.equal(recovered.snapshot().working.files["main.ts"],"const width = 16;\n");assert.equal(recovered.snapshot().accepted.files["main.ts"],files["main.ts"]);}finally{recovered.dispose();}
    }finally{client.dispose();}
  }finally{host.dispose();}
});

test("binary text handshake survives durable source staging and invalidates deleted-file typing",async()=>{
  const host=await open();const client=await createSharedText({actor:actor("alice"),checkpoint:host.textCheckpoint()});
  try{
    client.edit([{kind:"splice",path:"main.ts",start_utf16:14,delete_utf16:2,insert:"16"}]);
    let writes=0,settled=false;
    for(let i=0;i<20;i++){
      const outbound=host.generateSyncMessage("alice"),inbound=client.generateSyncMessage("server");
      if(!outbound&&!inbound){settled=true;break;}
      if(outbound)client.receiveSyncMessage("server",outbound);
      if(inbound)await host.receiveText("alice",inbound,actor("alice"),async()=>{writes++;});
    }
    assert(settled);assert(writes>0);assert.deepEqual(host.snapshot().working,client.capture());
    assert.equal(host.snapshot().accepted.files["main.ts"],files["main.ts"]);
    const before=client.capture().revision;
    const removed=host.stageHostEdits([{kind:"remove_file",path:"main.ts"}]);host.commitStage(removed);
    client.edit([{kind:"splice",path:"main.ts",start_utf16:0,delete_utf16:0,insert:"pending old file"}]);
    assert.throws(()=>host.stageTextChanges(client.changesSince(before),actor("alice")),/absent\/replaced/);
    host.forgetPeer("alice");client.forgetPeer("server");
  }finally{host.dispose();client.dispose();}
});

test("known unpersisted source staging can abort but uncertain storage still requires recovery",async()=>{
  const host=await open();
  try{
    const before=host.checkpoint(),head=host.snapshot().working.revision;
    const edits=[{kind:"splice",path:"main.ts",start_utf16:14,delete_utf16:2,insert:"14"}];
    const first=host.stageHostEdits(edits,head);
    host.discardUnpersistedStage(first);
    assert.equal(host.checkpoint(),before);assert.equal(host.snapshot().needsRecovery,false);
    const second=host.stageHostEdits(edits,head);
    assert.notEqual(second.stageId,first.stageId);
    assert.throws(()=>host.commitStage(first),/stale/);
    host.failStage(second);
    assert.equal(host.snapshot().needsRecovery,true);
    assert.throws(()=>host.discardUnpersistedStage(second),/stale/);
  }finally{host.dispose();}
});

test("committed incremental text preserves local pending edits and excludes unpersisted server changes",async()=>{
  const host=await open();
  const alice=await createSharedText({actor:actor("alice"),checkpoint:host.textCheckpoint()});
  const bob=await createSharedText({actor:actor("bob"),checkpoint:host.textCheckpoint()});
  try{
    const basis=alice.capture().revision;
    alice.edit([{kind:"splice",path:"main.ts",start_utf16:14,delete_utf16:2,insert:"16"}]);
    bob.edit([{kind:"splice",path:"helper.ts",start_utf16:0,delete_utf16:0,insert:"// pending bob 😀\n"}]);
    const staged=host.stageTextChanges(alice.changesSince(basis),actor("alice"));
    assert.deepEqual(host.textChangesSince(basis).changes,[]);
    host.commitStage(staged);
    const delta=host.textChangesSince(basis);
    assert.equal(delta.sourceSequence,1);assert.equal(delta.changes.length,1);
    bob.applyServerChanges(delta.changes.map(bytes=>Uint8Array.from(bytes)));
    assert.equal(bob.capture().files["main.ts"],"const width = 16;\n");
    assert.match(bob.capture().files["helper.ts"],/^\/\/ pending bob 😀/u);
    const write=host.stageTextChanges(bob.changesSince(delta.workingRevision),actor("bob"));
    host.commitStage(write);
    const final=host.textChangesSince(delta.workingRevision);
    alice.applyServerChanges(final.changes.map(bytes=>Uint8Array.from(bytes)));
    assert.deepEqual(alice.capture(),host.snapshot().working);
    assert.throws(()=>host.textChangesSince({heads:["0".repeat(64)]}),/revision|heads/u);
    bob.undo();
    assert.equal(bob.capture().files["main.ts"],"const width = 16;\n");
    assert.equal(bob.capture().files["helper.ts"],files["helper.ts"]);
  }finally{host.dispose();alice.dispose();bob.dispose();}
});

const userOperation=(userId,requestId)=>({userId,clientId:`tab-${userId}`,requestId});
async function userType(host,user,id,path,start,deleted,insert){
  const client=await createSharedText({actor:actor(user),checkpoint:host.textCheckpoint()});
  try{const basis=client.capture().revision;client.editFromRevision([{kind:"splice",path,start_utf16:start,delete_utf16:deleted,insert}],basis);host.commitStage(host.stageUserTextChanges(client.changesSince(basis),actor(user),userOperation(user,id)));}finally{client.dispose();}
}
test("actual WASM durable personal text ownership survives independent actor restart and preserves other users",async()=>{
  let host=await open();
  try{
    await userType(host,"alice","first","main.ts",14,2,"14");await userType(host,"alice","second","main.ts",14,2,"16");
    await userType(host,"bob","helper","helper.ts",22,1,"9");
    const accepted=host.snapshot().accepted,before=host.checkpoint();const first=host.stageUserUndo(userOperation("alice","undo-second"));
    assert.equal(host.checkpoint(),before);assert.equal(host.userHistory("alice").undoCount,2);host.discardUnpersistedStage(first);
    const retry=host.stageUserUndo(userOperation("alice","undo-second"));assert.notEqual(first.stageId,retry.stageId);host.commitStage(retry);
    let checkpointJson=host.checkpoint();host.dispose();host=await createTrustedSourceHost({configuration:configuration("new"),actor:actor("new-server"),checkpointJson});
    host.commitStage(host.stageUserUndo(userOperation("alice","undo-first")));assert.equal(host.snapshot().working.files["main.ts"],files["main.ts"]);
    checkpointJson=host.checkpoint();host.dispose();host=await createTrustedSourceHost({configuration:configuration("third"),actor:actor("third-server"),checkpointJson});
    host.commitStage(host.stageUserRedo(userOperation("alice","redo-first")));host.commitStage(host.stageUserRedo(userOperation("alice","redo-second")));
    assert.equal(host.snapshot().working.files["main.ts"],"const width = 16;\n");assert.equal(host.snapshot().working.files["helper.ts"],"export const height = 9;\n");assert.deepEqual(host.snapshot().accepted,accepted);
  }finally{host.dispose();}
});
test("actual WASM durable same-value ownership and file lifetime restoration guard text Undo",async()=>{
  let host=await open();
  try{
    await userType(host,"alice","first","main.ts",14,2,"14");await userType(host,"bob","same","main.ts",14,2,"14");
    const before=host.checkpoint();assert.throws(()=>host.stageUserUndo(userOperation("alice","blocked")),/ownership/);assert.equal(host.checkpoint(),before);
    host.commitStage(host.stageUserUndo(userOperation("bob","undo")));const original=host.snapshot().fileIds["main.ts"];
    host.commitStage(host.stageUserFileEdits([{kind:"remove_file",path:"main.ts"}],userOperation("alice","delete")));
    const checkpointJson=host.checkpoint();host.dispose();host=await createTrustedSourceHost({configuration:configuration("new"),actor:actor("new-server"),checkpointJson});
    host.commitStage(host.stageUserUndo(userOperation("alice","restore")));assert.notEqual(host.snapshot().fileIds["main.ts"],original);
    host.commitStage(host.stageUserUndo(userOperation("alice","undo-first")));assert.equal(host.snapshot().working.files["main.ts"],files["main.ts"]);
  }finally{host.dispose();}
});

test("actual WASM same-path contribution blocks file removal until repeated personal Undo restores ownership",async()=>{
  let host=await open();
  try{
    host.commitStage(host.stageUserFileEdits([{kind:"create_file",path:"new.ts",text:"x"}],userOperation("alice","create")));
    host.commitStage(host.stageUserFileEdits([{kind:"rename_file",path:"new.ts",new_path:"new.ts"}],userOperation("bob","same-path")));
    assert.throws(()=>host.stageUserUndo(userOperation("alice","blocked")),/ownership/);
    host.commitStage(host.stageUserUndo(userOperation("bob","undo")));
    const checkpointJson=host.checkpoint();host.dispose();host=await createTrustedSourceHost({configuration:configuration("restart"),actor:actor("new-server"),checkpointJson});
    host.commitStage(host.stageUserRedo(userOperation("bob","redo")));assert.throws(()=>host.stageUserUndo(userOperation("alice","blocked-again")),/ownership/);
    host.commitStage(host.stageUserUndo(userOperation("bob","undo-again")));host.commitStage(host.stageUserUndo(userOperation("alice","undo-create")));
    assert.equal(host.snapshot().working.files["new.ts"],undefined);
  }finally{host.dispose();}
});

test("actual WASM raw typing continues past retained Undo capacity with a durable visible horizon",async context=>{
  let host=await createTrustedSourceHost({configuration:{...configuration("horizon"),files:{"main.ts":"a0"}},actor:actor("server")});
  const client=await createSharedText({actor:actor("alice"),checkpoint:host.textCheckpoint()});
  try{
    let maximumStageMs=0,pruneMs=0;
    for(let index=0;index<520;index++){
      const started=performance.now();
      const before=client.capture().revision;
      client.editFromRevision([{kind:"splice",path:"main.ts",start_utf16:1,delete_utf16:1,insert:index%2?"0":"1"}],before);
      host.commitStage(host.stageUserTextChanges(client.changesSince(before),actor("alice"),userOperation("alice",`key-${index}`)));
      const elapsed=performance.now()-started;maximumStageMs=Math.max(maximumStageMs,elapsed);if(index===512)pruneMs=elapsed;
    }
    context.diagnostic(`520 native edits: maximum stage ${maximumStageMs.toFixed(1)}ms, pruning stage ${pruneMs.toFixed(1)}ms`);
    const availability=host.userHistory("alice");assert(availability.horizon.generation>0);assert(availability.horizon.discardedEvents>=256);assert(availability.undoCount>200&&availability.undoCount<=512);
    const checkpointJson=host.checkpoint();host.dispose();host=await createTrustedSourceHost({configuration:{...configuration("restart"),files:{"main.ts":"a0"}},actor:actor("new-server"),checkpointJson});
    assert.deepEqual(host.userHistory("alice").horizon,availability.horizon);
    host.commitStage(host.stageUserUndo(userOperation("alice","undo-last")));assert.equal(host.snapshot().working.files["main.ts"],"a1");
  }finally{host.dispose();client.dispose();}
});

test("actual WASM trusted mixed working edits wait for persistence and undo as one contribution",async()=>{
  let host=await open();
  try{
    const before=host.checkpoint(),basis=host.snapshot().working.revision;
    const edits=[{kind:"rename_file",path:"main.ts",new_path:"moved.ts"},{kind:"splice",path:"moved.ts",start_utf16:14,delete_utf16:2,insert:"😀("},{kind:"create_file",path:"broken.ts",text:"const = ("}];
    assert.throws(()=>host.stageUserFileEdits(edits,userOperation("alice","strict"),basis));
    assert.throws(()=>host.stageUserWorkingEdits([...edits,{kind:"remove_file",path:"absent.ts"}],userOperation("alice","bad"),basis));
    assert.equal(host.checkpoint(),before);
    const stage=host.stageUserWorkingEdits(edits,userOperation("alice","mixed"),basis);
    assert.equal(host.checkpoint(),before);host.commitStage(stage);
    assert.match(host.snapshot().working.files["moved.ts"],/😀\(/u);assert.equal(host.userHistory("alice").undoCount,1);
    const checkpointJson=host.checkpoint();host.dispose();host=await createTrustedSourceHost({configuration:configuration("restart"),actor:actor("new-server"),checkpointJson});
    host.commitStage(host.stageUserUndo(userOperation("alice","undo")));assert.deepEqual(host.snapshot().working.files,files);
  }finally{host.dispose();}
});
