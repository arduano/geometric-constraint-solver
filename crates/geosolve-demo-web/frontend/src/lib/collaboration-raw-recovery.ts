// SPDX-License-Identifier: GPL-3.0-or-later
import type { SourceEditorEdit } from "../components/code-editor";
export type RecoverableSourceIntent={path:string;edit:SourceEditorEdit};
/** Synchronous per-tab recovery covers keystrokes before the worker has produced
 * a durable native request. Native IDB outboxes own retry after that handoff. */
export class CollaborationRawRecovery {
  constructor(private readonly key:string,private readonly storage:Pick<Storage,"getItem"|"setItem"|"removeItem">=sessionStorage){}
  read():readonly RecoverableSourceIntent[]{
    const raw=this.storage.getItem(this.key);if(!raw)return [];
    const saved=JSON.parse(raw);
    if(saved?.format!=="geosolve-raw-source-intents-v1"||!Array.isArray(saved.intents)||saved.intents.length>128
      ||saved.intents.some((value:RecoverableSourceIntent)=>typeof value?.path!=="string"||typeof value.edit?.before!=="string"||typeof value.edit.after!=="string"||!Array.isArray(value.edit.changes)))throw Error("Saved source keystrokes are unreadable; their recovery bytes have been preserved");
    return saved.intents;
  }
  write(intents:readonly RecoverableSourceIntent[]){
    if(!intents.length){this.storage.removeItem(this.key);return;}
    const serialized=JSON.stringify({format:"geosolve-raw-source-intents-v1",intents});
    if(intents.length>128||serialized.length>2*1024*1024)throw Error("Pending source recovery is full; reconnect before adding more text");
    try{this.storage.setItem(this.key,serialized);}catch{throw Error("Browser storage could not retain your pending keystrokes; keep this editor open and export your source");}
  }
}
