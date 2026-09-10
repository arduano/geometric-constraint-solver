// SPDX-License-Identifier: GPL-3.0-or-later
import initializeWasm, * as wasm from "../../../../../packages/geosolve-collaboration/dist/wasm/geosolve_collaboration_wasm.js";
import wasmUrl from "../../../../../packages/geosolve-collaboration/dist/wasm/geosolve_collaboration_wasm_bg.wasm?url";
import { createSharedText, type SharedText, type TextEdit, type TextRevision, type TextSnapshot } from "../../../../../packages/geosolve-collaboration/src/index";

export type TextWorkerRequest =
  | {id:number;method:"open";actor:readonly number[];checkpoint:readonly number[];saved?:readonly number[]}
  | {id:number;method:"edit";revision:TextRevision;edits:readonly TextEdit[]}
  | {id:number;method:"receive";changes:readonly (readonly number[])[]}
  | {id:number;method:"undo"|"redo"|"snapshot"};
export interface TextWorkerUpdate {snapshot:TextSnapshot;localRevision?:TextRevision;changes:readonly (readonly number[])[];checkpoint:readonly number[];history:{readonly undo:number;readonly redo:number}}
export type TextWorkerResponse={id:number;result:TextWorkerUpdate}|{id:number;error:string};

/** Text editing has its own worker; compiler, authoring and navigation queues
 * cannot delay a native CRDT edit. Every edit carries its exact UTF-16 basis. */
export function createTextWorkerHandler(open:(actor:Uint8Array,checkpoint:Uint8Array)=>Promise<SharedText>,reply:(response:TextWorkerResponse)=>void){
  let text:SharedText|undefined;
  let tail=Promise.resolve();
  return({data}:MessageEvent<TextWorkerRequest>)=>{
    const run=async()=>{
      try{
        if(!Number.isSafeInteger(data.id)||data.id<1)throw Error("Invalid text worker request");
        let changes:readonly Uint8Array[]=[];
        let localRevision:TextRevision|undefined;
        if(data.method==="open"){
          const next=await open(Uint8Array.from(data.actor),Uint8Array.from(data.saved??data.checkpoint));
          try{
            if(data.saved){
              const server=await open(Uint8Array.from(data.actor),Uint8Array.from(data.checkpoint));
              try{next.applyServerChanges(server.changesSince({heads:[]}));}finally{server.dispose();}
            }
          }catch(error){next.dispose();throw error;}
          text?.dispose();text=next;
        }else{
          if(!text)throw Error("Shared text has not opened");
          const basis=text.capture().revision;
          if(data.method==="edit")localRevision=text.editFromRevision(data.edits,data.revision).localRevision;
          else if(data.method==="receive")text.applyServerChanges(data.changes.map(bytes=>Uint8Array.from(bytes)));
          else if(data.method==="undo")text.undo();
          else if(data.method==="redo")text.redo();
          else if(data.method!=="snapshot")throw Error("Unknown shared text request");
          if(data.method!=="receive"&&data.method!=="snapshot")changes=text.changesSince(basis);
        }
        reply({id:data.id,result:{snapshot:text!.capture(),localRevision,changes:changes.map(bytes=>Array.from(bytes)),checkpoint:Array.from(text!.save()),history:text!.history}});
      }catch(error){reply({id:data.id,error:error instanceof Error?error.message:String(error)});}
    };
    tail=tail.then(run,run);
  };
}
if(typeof document==="undefined"&&typeof self!=="undefined"){
  const ready=initializeWasm({module_or_path:new URL(wasmUrl,import.meta.url)});
  self.onmessage=createTextWorkerHandler(async(actor,checkpoint)=>{await ready;return createSharedText({actor,checkpoint,wasmModule:{...wasm,default:initializeWasm},wasm:new URL(wasmUrl,import.meta.url)});},response=>self.postMessage(response));
}
