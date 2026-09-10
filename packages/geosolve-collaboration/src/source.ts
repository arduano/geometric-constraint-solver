// SPDX-License-Identifier: GPL-3.0-or-later
import type { TextEdit, TextLimits, TextRevision, TextSnapshot } from "./index.js";
import type { OperationId } from "./host.js";
import { decode, encode, unicode } from "./host-codec.js";

export interface TextHistoryHorizon {
  readonly generation:number;readonly discardedEvents:number;readonly oldestRevision:TextRevision|null;
}
export interface UserTextHistory {
  readonly undoCount:number;readonly redoCount:number;readonly canUndo:boolean;readonly canRedo:boolean;
  readonly undoUnavailable:string|null;readonly redoUnavailable:string|null;
  readonly horizon:TextHistoryHorizon;
}
export interface SourceHostConfiguration {
  readonly documentEpoch: string;
  readonly serverEpoch: string;
  readonly initialInput: string;
  readonly files: Readonly<Record<string,string>>;
  readonly limits?: TextLimits;
}
export interface AcceptedSource { readonly modelRevision:number; readonly acceptedInput:string; readonly files:Readonly<Record<string,string>> }
export interface SourceSnapshot {
  readonly sequence:number;
  readonly accepted:AcceptedSource;
  readonly working:TextSnapshot;
  readonly fileIds:Readonly<Record<string,string>>;
  readonly pendingNotices:readonly { readonly path:string; readonly acceptedRevision:number; readonly acceptedSourceDigest:string; readonly reason:string }[];
  readonly hasPendingStage:boolean;
  readonly needsRecovery:boolean;
}
export interface SourceEdit { readonly start:number; readonly end:number; readonly expected:string; readonly replacement:string }
export interface SourcePatch { readonly baseSourceDigest:string; readonly candidateSourceDigest:string; readonly edits:readonly SourceEdit[] }
export interface SourceCapture {
  readonly handle:string;
  /** Exact durable admission capture; restoration also needs the authenticated journal basis. */
  readonly captureJson:string;
  readonly acceptedBasis:AcceptedSource;
  readonly working:TextSnapshot;
  readonly fileIds:Readonly<Record<string,string>>;
}
export interface PreparedSource {
  readonly ticket:string;
  readonly acceptedBasis:AcceptedSource;
  readonly candidateFiles:Readonly<Record<string,string>>;
  readonly changedPaths:readonly string[];
}
export type SourceReconciliation =
  | { readonly kind:"captured"; readonly path:string }
  | { readonly kind:"reconciled"; readonly path:string; readonly patch:SourcePatch }
  | { readonly kind:"pending"; readonly path:string; readonly reason:string };
/** Candidate source envelope, not a text/model ACK. The host durably pairs model
 * publication with the authority terminal event before installing either stage.
 */
export interface SourceStage {
  readonly status:"staged";
  readonly stageId:string;
  readonly sequence:number;
  readonly checkpointJson:string;
  readonly accepted:AcceptedSource;
  readonly workingRevision:TextRevision;
}
export type PersistSourceStage = (stage:SourceStage)=>Promise<void>;
export interface SourceNativeHandle {
  snapshot():string;
  checkpoint():string;
  textCheckpoint():Uint8Array;
  textChangesSince(revisionJson:string):string;
  generateSyncMessage(peer:string):Uint8Array|undefined;
  forgetPeer(peer:string):void;
  stageTextSync(peer:string,message:Uint8Array,actor:Uint8Array):string;
  stageTextChanges(changesJson:string,actor:Uint8Array):string;
  stageHostEdits(expectedJson:string,editsJson:string):string;
  stageUserTextChanges(changesJson:string,actor:Uint8Array,operationJson:string):string;
  stageUserFileEdits(expectedJson:string,editsJson:string,operationJson:string):string;
  stageUserWorkingEdits(expectedJson:string,editsJson:string,operationJson:string):string;
  stageUserUndo(operationJson:string):string;
  stageUserRedo(operationJson:string):string;
  userHistory(userId:string):string;
  captureApply():string;
  restoreApplyCapture(captureJson:string,basisJson:string):string;
  applyNeedsRebase(capture:string):boolean;
  prepareCanvasUpdate(patchesJson:string):string;
  prepareApplyUpdate(capture:string,filesJson:string):string;
  releaseHandle(handle:string):boolean;
  stageValidatedPublication(ticket:string,acceptedInput:string,workingJson:string,reconciliationsJson:string):string;
  commitStage(id:string):string;
  failStage(id:string):void;
  discardUnpersistedStage(id:string):void;
  free():void;
}
export interface SourceWasmModule {
  default(options:{module_or_path:Uint8Array|URL|Response|WebAssembly.Module}):Promise<unknown>;
  TrustedSourceHost:{ new(configurationJson:string,actor:Uint8Array):SourceNativeHandle; restore(configurationJson:string,actor:Uint8Array,checkpointJson:string):SourceNativeHandle };
}
export interface SourceHostOptions {
  readonly configuration:SourceHostConfiguration;
  readonly actor:Uint8Array;
  readonly checkpointJson?:string;
  readonly wasmModule?:SourceWasmModule;
  readonly wasm?:Uint8Array|URL|Response|WebAssembly.Module;
}

/** Trusted server composition of accepted source and independent raw draft text.
 * Preparation/capture handles do not lock typing while external model jobs run.
 */
export class TrustedSourceHost {
  private disposed=false;
  private pending?:SourceStage;
  private readonly captures=new Set<SourceCapture>();
  private readonly prepared=new Set<PreparedSource>();
  constructor(private readonly native:SourceNativeHandle) {}
  snapshot():SourceSnapshot {this.live();return decode(this.native.snapshot());}
  checkpoint():string {this.live();return this.native.checkpoint();}
  textCheckpoint():Uint8Array {this.live();return this.native.textCheckpoint();}
  /** Read committed deltas without retaining per-client server handshakes. */
  textChangesSince(revision:TextRevision):{readonly sourceSequence:number;readonly workingRevision:TextRevision;readonly changes:readonly (readonly number[])[]} {
    this.live();return decode(this.native.textChangesSince(encode(revision)));
  }
  generateSyncMessage(peer:string):Uint8Array|undefined {this.live();unicode(peer);return this.native.generateSyncMessage(peer);}
  forgetPeer(peer:string):void {this.live();unicode(peer);this.native.forgetPeer(peer);}
  /** Host must bind actor to authenticated editor session before calling. */
  stageTextSync(peer:string,message:Uint8Array,actor:Uint8Array):SourceStage {
    this.live();unicode(peer);checkActor(actor);return this.retainStage(this.native.stageTextSync(peer,message,actor));
  }
  stageTextChanges(changes:readonly Uint8Array[],actor:Uint8Array):SourceStage {
    this.live();checkActor(actor);return this.retainStage(this.native.stageTextChanges(encode(changes.map(bytes=>Array.from(bytes))),actor));
  }
  /** Principal/operation comes from authenticated host admission. Native derives
   * character ownership from change bytes; no client inverse token is accepted. */
  stageUserTextChanges(changes:readonly Uint8Array[],actor:Uint8Array,operation:OperationId):SourceStage {
    this.live();checkActor(actor);return this.retainStage(this.native.stageUserTextChanges(encode(changes.map(bytes=>Array.from(bytes))),actor,encode(operation)));
  }
  stageUserFileEdits(edits:readonly TextEdit[],operation:OperationId,expected:TextRevision=this.snapshot().working.revision):SourceStage {
    this.live();return this.retainStage(this.native.stageUserFileEdits(encode(expected),encode(edits),encode(operation)));
  }
  /** Trusted host ordered mixed splices/file lifecycle as one durable contribution. */
  stageUserWorkingEdits(edits:readonly TextEdit[],operation:OperationId,expected:TextRevision=this.snapshot().working.revision):SourceStage {
    this.live();return this.retainStage(this.native.stageUserWorkingEdits(encode(expected),encode(edits),encode(operation)));
  }
  stageUserUndo(operation:OperationId):SourceStage {this.live();return this.retainStage(this.native.stageUserUndo(encode(operation)));}
  stageUserRedo(operation:OperationId):SourceStage {this.live();return this.retainStage(this.native.stageUserRedo(encode(operation)));}
  userHistory(userId:string):UserTextHistory {this.live();unicode(userId);return decode(this.native.userHistory(userId));}
  /** External mirror/lifecycle edits already ordered by the host gateway. */
  stageHostEdits(edits:readonly TextEdit[],expected:TextRevision=this.snapshot().working.revision):SourceStage {
    this.live();return this.retainStage(this.native.stageHostEdits(encode(expected),encode(edits)));
  }
  captureApply():SourceCapture {this.live();const capture=decode<SourceCapture>(this.native.captureApply());this.captures.add(capture);return capture;}
  /** basis must come from authenticated durable admission, never a client body. */
  restoreApplyCapture(captureJson:string,authenticatedBasis:AcceptedSource):SourceCapture {
    this.live();unicode(captureJson);const capture=decode<SourceCapture>(this.native.restoreApplyCapture(captureJson,encode(authenticatedBasis)));this.captures.add(capture);return capture;
  }
  applyNeedsRebase(capture:SourceCapture):boolean {this.captureOwned(capture);return this.native.applyNeedsRebase(capture.handle);}
  prepareCanvasUpdate(patches:readonly {readonly path:string;readonly patch:SourcePatch}[]):PreparedSource {
    this.live();return this.retainPrepared(this.native.prepareCanvasUpdate(encode(patches)));
  }
  prepareApplyUpdate(capture:SourceCapture,rebasedFiles:Readonly<Record<string,string>>):PreparedSource {
    this.captureOwned(capture);return this.retainPrepared(this.native.prepareApplyUpdate(capture.handle,encode(rebasedFiles)));
  }
  release(handle:SourceCapture|PreparedSource):void {
    this.live();
    if ("handle" in handle) {this.captureOwned(handle);this.native.releaseHandle(handle.handle);this.captures.delete(handle);}
    else {this.preparedOwned(handle);this.native.releaseHandle(handle.ticket);this.prepared.delete(handle);}
  }
  /** Only after independent engine validation of this exact ticket's candidate.
   * Reconciliation must refer to latest working heads after the model job finishes.
   */
  stageValidatedPublication(prepared:PreparedSource,acceptedInput:string,expectedWorking:TextRevision,reconciliations:readonly SourceReconciliation[]):SourceStage {
    this.preparedOwned(prepared);unicode(acceptedInput);
    return this.retainStage(this.native.stageValidatedPublication(prepared.ticket,acceptedInput,encode(expectedWorking),encode(reconciliations)));
  }
  /** Persist exact envelope and associated authority/model transaction first. */
  commitStage(stage:SourceStage):SourceSnapshot {
    this.stageOwned(stage);const snapshot=decode<SourceSnapshot>(this.native.commitStage(stage.stageId));this.pending=undefined;
    for (const prepared of this.prepared) if (prepared.acceptedBasis.modelRevision<snapshot.accepted.modelRevision) {
      this.native.releaseHandle(prepared.ticket);this.prepared.delete(prepared);
    }
    return snapshot;
  }
  failStage(stage:SourceStage):void {this.stageOwned(stage);this.native.failStage(stage.stageId);this.pending=undefined;this.captures.clear();this.prepared.clear();}
  /** Only before any filesystem append starts; uncertain writes require failStage. */
  discardUnpersistedStage(stage:SourceStage):void {this.stageOwned(stage);this.native.discardUnpersistedStage(stage.stageId);this.pending=undefined;}
  async receiveText(peer:string,message:Uint8Array,actor:Uint8Array,persist:PersistSourceStage):Promise<SourceSnapshot> {return this.persist(this.stageTextSync(peer,message,actor),persist);}
  async receiveTextChanges(changes:readonly Uint8Array[],actor:Uint8Array,persist:PersistSourceStage):Promise<SourceSnapshot> {return this.persist(this.stageTextChanges(changes,actor),persist);}
  async editWorking(edits:readonly TextEdit[],expected:TextRevision,persist:PersistSourceStage):Promise<SourceSnapshot> {return this.persist(this.stageHostEdits(edits,expected),persist);}
  dispose():void {
    if (!this.disposed) {if (this.pending) throw Error("Resolve pending source persistence before disposing");this.disposed=true;this.captures.clear();this.prepared.clear();this.native.free();}
  }
  private async persist(stage:SourceStage,persist:PersistSourceStage):Promise<SourceSnapshot> {
    try {await persist(stage);}catch(failure){this.failStage(stage);throw failure;}
    return this.commitStage(stage);
  }
  private retainStage(text:string):SourceStage {const stage=decode<SourceStage>(text);this.pending=stage;return stage;}
  private retainPrepared(text:string):PreparedSource {const prepared=decode<PreparedSource>(text);this.prepared.add(prepared);return prepared;}
  private live():void {if(this.disposed)throw Error("Source host has been disposed");}
  private stageOwned(stage:SourceStage):void {this.live();if(stage!==this.pending)throw Error("Unknown or stale source stage");}
  private captureOwned(capture:SourceCapture):void {this.live();if(!this.captures.has(capture))throw Error("Unknown or stale source capture");}
  private preparedOwned(prepared:PreparedSource):void {this.live();if(!this.prepared.has(prepared))throw Error("Unknown or stale source preparation");}
}
export async function createTrustedSourceHost(options:SourceHostOptions):Promise<TrustedSourceHost> {
  checkActor(options.actor);const configuration=encode(options.configuration);
  if(options.checkpointJson!==undefined)unicode(options.checkpointJson);
  const path="./wasm/geosolve_collaboration_wasm.js";
  const module:SourceWasmModule=options.wasmModule??await import(path);
  let bytes=options.wasm;
  if(!bytes){const url=new URL("./wasm/geosolve_collaboration_wasm_bg.wasm",import.meta.url);if(url.protocol==="file:"){const nodeFs="node:fs/promises";bytes=await(await import(nodeFs)).readFile(url);}else bytes=url;}
  await module.default({module_or_path:bytes!});
  return new TrustedSourceHost(options.checkpointJson===undefined?new module.TrustedSourceHost(configuration,options.actor):module.TrustedSourceHost.restore(configuration,options.actor,options.checkpointJson));
}
function checkActor(actor:Uint8Array):void {if(!(actor instanceof Uint8Array)||actor.length===0||actor.length>64)throw Error("Actor must contain 1..64 bytes");}
