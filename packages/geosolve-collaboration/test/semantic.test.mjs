// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { createTrustedSemanticHost } from "../dist/host.js";
import * as client from "../dist/index.js";
const configuration=serverEpoch=>({documentEpoch:"doc-1",serverEpoch,objects:[{object:"edge",dependencies:["width"]},{object:"width"},{object:"height"}]});
const open=()=>createTrustedSemanticHost({configuration:configuration("process-1")});
const operation=(userId,requestId)=>({userId,clientId:`tab-${userId}`,requestId});
const change=(host,object,before,after)=>({address:{target:host.current(object),property:"value"},before,after});
const record=(host,user,id,object,before,after,persist=async()=>{})=>host.record({basisRevision:host.snapshot().revision,revision:host.snapshot().revision+1,operation:operation(user,id),changes:[change(host,object,before,after)]},persist);
const transact=(host,request,persist=async()=>{})=>host.transact({basisRevision:host.snapshot().revision,revision:host.snapshot().revision+1,...request},persist);
const inverse=(host,prepared,user,id,persist=async()=>{})=>host.inverse(prepared,operation(user,id),host.snapshot().revision+1,persist);
function deferred(){let resolve;const promise=new Promise(yes=>{resolve=yes;});return{promise,resolve};}

test("actual WASM semantic write is committed only after persistence with exact core checkpoint strings",async()=>{
  assert.equal(client.TrustedSemanticHost,undefined);assert.equal(client.createTrustedSemanticHost,undefined);
  const host=await open(),other=await open();
  try{
    const before=host.checkpoint(),persistence=deferred();let returned=false,stored;
    const pending=record(host,"alice","width","width",12,14,async stage=>{stored=stage;await persistence.promise;}).then(result=>{returned=true;return result;});
    assert.equal(host.snapshot().hasPendingStage,true);assert.equal(host.snapshot().revision,0);
    assert.deepEqual(host.checkpoint(),before);assert.equal(host.propertyOwner(change(host,"width",12,14).address),null);
    assert.throws(()=>host.prepareUndo("alice"),/pending/);assert.throws(()=>host.dispose(),/pending/);
    assert.throws(()=>other.commitStage(stored),/stale/);assert.throws(()=>host.commitStage({...stored}),/stale/);
    await new Promise(resolve=>setImmediate(resolve));assert.equal(returned,false);
    persistence.resolve();await pending;
    assert.equal(host.snapshot().revision,1);assert.equal(host.checkpoint().targetsJson,stored.targetsJson);assert.equal(host.checkpoint().historyJson,stored.historyJson);
    assert.throws(()=>host.commitStage(stored),/stale/);
    const restored=await createTrustedSemanticHost({configuration:configuration("new"),checkpoint:host.checkpoint()});
    try{assert.deepEqual(restored.checkpoint(),host.checkpoint());}finally{restored.dispose();}
  }finally{host.dispose();other.dispose();}
});

test("actual WASM same-value semantic ownership in either actor order protects concurrent Undo",async()=>{
  for(const [first,second]of[["alice","bob"],["bob","alice"]]){
    const host=await open();
    try{
      await record(host,first,"first","width",12,14);const old=host.prepareUndo(first);
      await record(host,second,"second","width",14,14);
      assert.throws(()=>host.prepareUndo(first),/owns/);
      assert.throws(()=>host.stageValidatedInverse(old,operation(first,"stale"),3),/stale/);
      const latest=host.prepareUndo(second);assert.equal(latest.changes[0].before,14);assert.equal(latest.changes[0].after,14);
      assert(Object.isFrozen(latest.changes[0].address.target));
      await inverse(host,latest,second,"undo");
      const earlier=host.prepareUndo(first);assert.equal(earlier.changes[0].after,12);
      assert.deepEqual(host.propertyOwner(earlier.changes[0].address),operation(first,"first"));
      host.release(earlier);
    }finally{host.dispose();}
  }
});

test("actual WASM deletion authenticates exact current dependents and recreated generations invalidate old actions",async()=>{
  const host=await open();
  try{
    await record(host,"alice","width","width",12,14);
    const width=host.current("width"),plan=host.planDelete([width]);assert.equal(plan.closure.length,2);
    await transact(host,{create:[{object:"late",dependencies:[{kind:"existing",target:width}]}]});
    assert.throws(()=>host.authenticateDelete(plan),/changed/);
    const before=host.checkpoint();
    assert.throws(()=>host.stageValidatedTransaction({basisRevision:2,revision:3,deletions:[plan],dependencies:[{target:{kind:"existing",target:host.current("late")},dependencies:[]}]}),/changed/);
    assert.deepEqual(host.checkpoint(),before);
    let stored;
    await transact(host,{deletions:[host.planDelete([width])],create:[{object:"child",dependencies:[{kind:"created",object:"width"}]},{object:"width"}]},async write=>{stored=write;assert.equal(host.current("width").generation,width.generation);});
    assert.equal(stored.created.length,2);assert(host.current("width").generation>width.generation);
    assert.equal(host.current("late"),null);assert.equal(host.current("edge"),null);assert.throws(()=>host.authenticate(width),/lifetime/);
    assert.throws(()=>host.prepareUndo("alice"),/recreated/);
    assert.equal(host.snapshot().historyRevision,1);assert.equal(host.snapshot().revision,3);
    const restored=await createTrustedSemanticHost({configuration:configuration("new"),checkpoint:host.checkpoint()});
    try{assert.deepEqual(restored.current("width"),host.current("width"));assert.throws(()=>restored.prepareUndo("alice"),/recreated/);}finally{restored.dispose();}
  }finally{host.dispose();}
});

test("actual WASM property/lifecycle batch rejection is atomic and accepts ordinary finite JSON values",async()=>{
  const host=await open();
  try{
    await record(host,"alice","first","width",12,14);
    const before=host.checkpoint();
    const request={basisRevision:1,revision:2,create:[{object:"new"}],record:{operation:operation("alice","second"),changes:[change(host,"width",12,16)]}};
    assert.throws(()=>host.stageValidatedTransaction(request),/no longer matches/);
    assert.deepEqual(host.checkpoint(),before);assert.equal(host.current("new"),null);
    request.record.changes[0].before=14;const staged=host.stageValidatedTransaction(request);
    assert.equal(host.current("new"),null);host.commitStage(staged);assert(host.current("new"));
    await record(host,"bob","float","height",{position:[-1.25,0.5]},{position:[-3.75,1.125]});
    const prepared=host.prepareUndo("bob");assert.deepEqual(prepared.changes[0].after,{position:[-1.25,0.5]});host.release(prepared);
    await assert.rejects(record(host,"bob","invalid","height",0,Infinity),/Nonfinite/);
    assert.throws(()=>host.stageValidatedRecord({basisRevision:3.5,revision:4,operation:operation("bob","fraction"),changes:[]}),/safe integer/);
    assert.throws(()=>host.current("\ud800"),/surrogate/);
  }finally{host.dispose();}
});

test("actual WASM restart restores native history Redo with unrelated ownership preserved",async()=>{
  const host=await open();let restored;
  try{
    await record(host,"alice","width","width",12,14);await record(host,"bob","height","height",8,9);
    await inverse(host,host.prepareUndo("alice"),"alice","undo");const old=host.prepareRedo("alice");
    restored=await createTrustedSemanticHost({configuration:configuration("new"),checkpoint:host.checkpoint()});
    assert.deepEqual(restored.checkpoint(),host.checkpoint());
    assert.throws(()=>restored.stageValidatedInverse(old,operation("alice","redo"),4),/stale/);
    const prepared=restored.prepareRedo("alice");assert.throws(()=>restored.stageValidatedInverse({...prepared},operation("alice","redo"),4),/stale/);
    assert.throws(()=>restored.stageValidatedInverse(prepared,operation("bob","redo"),4));
    await inverse(restored,prepared,"alice","redo");
    assert.deepEqual(restored.propertyOwner(change(restored,"height",8,9).address),operation("bob","height"));
    assert.deepEqual(restored.propertyOwner(change(restored,"width",12,14).address),operation("alice","width"));
    await assert.rejects(createTrustedSemanticHost({configuration:configuration("new"),checkpoint:{...host.checkpoint(),documentEpoch:"other"}}),/identity/);
    await assert.rejects(createTrustedSemanticHost({configuration:configuration("new"),checkpoint:{...host.checkpoint(),revision:2}}),/ahead/);
  }finally{host.dispose();restored?.dispose();}
});

test("actual WASM uncertain semantic persistence requires reconstruction and bounds native inverse tickets",async()=>{
  const host=await open();let stored;
  try{
    await record(host,"alice","first","width",12,14);
    const handles=Array.from({length:32},()=>host.prepareUndo("alice"));
    assert.throws(()=>host.prepareUndo("alice"),/capacity/);host.release(handles.pop());host.release(host.prepareUndo("alice"));
    await assert.rejects(inverse(host,handles[0],"alice","undo",async write=>{stored=write;throw Error("fsync uncertain");}),/uncertain/);
    assert.equal(host.snapshot().needsRecovery,true);assert.equal(host.snapshot().revision,1);assert.throws(()=>host.prepareUndo("alice"),/recovery/);
    const restored=await createTrustedSemanticHost({configuration:configuration("new"),checkpoint:{documentEpoch:"doc-1",revision:stored.revision,targetsJson:stored.targetsJson,historyJson:stored.historyJson}});
    try{assert.equal(restored.snapshot().revision,2);assert.equal(restored.prepareRedo("alice").changes[0].after,14);}finally{restored.dispose();}
  }finally{host.dispose();}
});

test("actual WASM inventory and adapter limits reject duplicates, dangling dependencies, and oversized budgets",async()=>{
  for(const objects of [[{object:"a"},{object:"a"}],[{object:"a",dependencies:["missing"]}],[{object:"a",dependencies:["a"]}]]){
    await assert.rejects(createTrustedSemanticHost({configuration:{...configuration("process"),objects}}));
  }
  await assert.rejects(createTrustedSemanticHost({configuration:{...configuration("process"),limits:{maxObjectsIncludingTombstones:2}}}),/limit/);
  await assert.rejects(createTrustedSemanticHost({configuration:{...configuration("process"),limits:{maxTotalDependencies:1_000_001}}}),/maximum/);
});

test("actual WASM mixed structural history remaps restored sibling generations and preserves Bob",async()=>{
  let host=await open();
  try{
    await transact(host,{create:[{object:"parent"},{object:"child",dependencies:[{kind:"created",object:"parent"}]}],record:{operation:operation("alice","create"),changes:[],structural:{created:[
      {object:"parent",payload:{source:"parent()"},position:{previous:null,next:{kind:"created",object:"child"}}},
      {object:"child",payload:{source:"child(parent)"},position:{previous:{kind:"created",object:"parent"},next:null}}
    ]}}});
    const old=host.current("parent");
    await record(host,"alice","edit","parent",12,14);await record(host,"bob","independent","height",8,9);
    await inverse(host,host.prepareUndo("alice"),"alice","undo-edit");
    const removal=host.prepareUndo("alice");assert.equal(removal.structural.delete.closure.length,2);
    await inverse(host,removal,"alice","undo-create");assert.equal(host.current("parent"),null);
    const checkpoint=host.checkpoint();host.dispose();host=await createTrustedSemanticHost({configuration:configuration("restart"),checkpoint});
    const restore=host.prepareRedo("alice"),highWater=host.snapshot().highWater;
    assert.equal(restore.structural.create.length,2);assert.deepEqual(restore.structural.create[0].position.next,restore.structural.create[1].target);
    assert(restore.structural.create.every(object=>object.target.generation>highWater));
    const stage=host.stageValidatedInverse(restore,operation("alice","redo-create"),host.snapshot().revision+1);
    assert.equal(host.snapshot().highWater,highWater);assert.equal(host.current("parent"),null);
    host.discardUnpersistedStage(stage);assert.equal(host.snapshot().needsRecovery,false);
    const retry=host.stageValidatedInverse(restore,operation("alice","redo-create"),host.snapshot().revision+1);
    assert.notEqual(stage.stageId,retry.stageId);assert.deepEqual(stage.created,retry.created);
    assert.throws(()=>host.commitStage(stage),/stale/);host.commitStage(retry);
    assert.throws(()=>host.authenticate(old),/lifetime/);
    const redo=host.prepareRedo("alice");assert.deepEqual(redo.changes[0].address.target,host.current("parent"));
    await inverse(host,redo,"alice","redo-edit");
    assert.deepEqual(host.propertyOwner(change(host,"height",8,9).address),operation("bob","independent"));
  }finally{host.dispose();}
});

test("actual WASM exact deletion observation restores a closure with fresh handles after restart",async()=>{
  let host=await open();
  try{
    const width=host.current("width"),edge=host.current("edge"),plan=host.planDelete([width]);
    await transact(host,{deletions:[plan],record:{operation:operation("alice","delete"),changes:[],structural:{deleted:[
      {target:width,payload:{source:"width()"},position:{previous:null,next:null}},
      {target:edge,payload:{source:"edge(width)"},position:{previous:null,next:null}}
    ]}}});
    const checkpoint=host.checkpoint();host.dispose();host=await createTrustedSemanticHost({configuration:configuration("restart"),checkpoint});
    const restored=host.prepareUndo("alice");assert.equal(restored.structural.create.length,2);
    await inverse(host,restored,"alice","undo-delete");assert.throws(()=>host.authenticate(width),/lifetime/);
    assert.equal(host.planDelete([host.current("width")]).closure.length,2);
    await record(host,"bob","same","width",12,12);
    assert.throws(()=>host.prepareRedo("alice"),/owns/);
    await inverse(host,host.prepareUndo("bob"),"bob","undo");
    await inverse(host,host.prepareRedo("alice"),"alice","redo-delete");
    assert.equal(host.current("width"),null);
  }finally{host.dispose();}
});

test("actual WASM structural validation rejects missing observations and tracks same-value dependencies",async()=>{
  const host=await open();
  try{
    const before=host.checkpoint();
    assert.throws(()=>host.stageValidatedTransaction({basisRevision:0,revision:1,create:[{object:"new"}],record:{operation:operation("alice","bad"),changes:[],structural:{}}}));
    assert.deepEqual(host.checkpoint(),before);assert.equal(host.snapshot().hasPendingStage,false);
    await transact(host,{create:[{object:"new"}],record:{operation:operation("alice","create"),changes:[],structural:{created:[{object:"new",payload:{source:"new()"},position:{previous:null,next:null}}]}}});
    await transact(host,{dependencies:[{target:{kind:"existing",target:host.current("new")},dependencies:[]}],record:{operation:operation("bob","same-deps"),changes:[],structural:{}}});
    assert.throws(()=>host.prepareUndo("alice"),/owns/);
    const undo=host.prepareUndo("bob");assert.equal(undo.changes.length,0);assert.equal(undo.structural.dependencies.length,1);
    await inverse(host,undo,"bob","undo");host.release(host.prepareUndo("alice"));
  }finally{host.dispose();}
});
