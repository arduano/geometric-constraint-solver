// SPDX-License-Identifier: GPL-3.0-or-later
import type { TextWorkerRequest, TextWorkerResponse, TextWorkerUpdate } from "./collaboration-text-worker";
import type { TextEdit, TextRevision } from "../../../../../packages/geosolve-collaboration/src/index";
export type { TextWorkerUpdate, TextEdit, TextRevision };
type WithoutId<T>=T extends unknown?Omit<T,"id">:never;
export class CollaborationTextWorker {
  private id=0;
  private readonly pending=new Map<number,{resolve:(value:TextWorkerUpdate)=>void;reject:(error:Error)=>void}>();
  private failure?:Error;
  private readonly worker:Worker;
  constructor(){
    this.worker=new Worker(new URL("./collaboration-text-worker.ts",import.meta.url),{type:"module"});
    this.worker.onmessage=({data}:MessageEvent<TextWorkerResponse>)=>{
      const pending=this.pending.get(data.id);if(!pending)return;this.pending.delete(data.id);
      if("error"in data)pending.reject(Error(data.error));else pending.resolve(data.result);
    };
    this.worker.onerror=event=>{event.preventDefault();this.fail(Error(event.message||"Shared text worker stopped"));};
    this.worker.onmessageerror=()=>this.fail(Error("Shared text response could not be decoded"));
  }
  open(actor:readonly number[],checkpoint:readonly number[],saved?:readonly number[]){return this.request({method:"open",actor,checkpoint,saved});}
  edit(revision:TextRevision,edits:readonly TextEdit[]){return this.request({method:"edit",revision,edits});}
  receive(changes:readonly (readonly number[])[]){return this.request({method:"receive",changes});}
  undo(){return this.request({method:"undo"});}
  redo(){return this.request({method:"redo"});}
  snapshot(){return this.request({method:"snapshot"});}
  dispose(){this.fail(Error("Shared text worker disposed"));}
  private request(input:WithoutId<TextWorkerRequest>):Promise<TextWorkerUpdate>{
    if(this.failure)return Promise.reject(this.failure);
    const id=++this.id;
    return new Promise((resolve,reject)=>{this.pending.set(id,{resolve,reject});try{this.worker.postMessage({...input,id});}catch(error){this.pending.delete(id);reject(error);}});
  }
  private fail(error:Error){if(this.failure)return;this.failure=error;this.worker.terminate();for(const pending of this.pending.values())pending.reject(error);this.pending.clear();}
}
