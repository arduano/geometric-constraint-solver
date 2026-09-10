// SPDX-License-Identifier: GPL-3.0-or-later
import type { AuthoritySnapshot, Command, Connection, JournalRecord, Receipt } from "./host.js";
import type { TextEdit, TextRevision } from "./index.js";
import type { UserTextHistory } from "./source.js";
export type { Command, Connection, Receipt, Outcome, JournalRecord } from "./host.js";

export interface Participant { readonly userId:string; readonly clientId:string; readonly role:"editor"|"viewer" }
export interface CollaborationState<Document=unknown> {
  readonly authority:AuthoritySnapshot; readonly document:Document;
  readonly participants:readonly Participant[]; readonly presence:readonly unknown[]; readonly workerError?:string;
}
export type PendingRequest = {
  readonly route:"commands"|"text"; readonly requestId:string; readonly body:Readonly<Record<string,unknown>>;
};
export interface PendingCheckpoint {
  readonly format:"geosolve-client-pending-v1"; readonly documentId:string; readonly documentEpoch:string;
  readonly userId:string; readonly clientId:string; readonly requests:readonly PendingRequest[];
}
export interface ClientOptions<Document> {
  readonly baseUrl:string; readonly inviteToken:string; readonly clientId:string;
  readonly fetch?:typeof fetch; readonly pending?:PendingCheckpoint;
  /** Persist before transmitting. Failure retains intent in memory and prevents send. */
  readonly savePending?:(checkpoint:PendingCheckpoint)=>void|Promise<void>;
  /** Host-provided local ownership fence, checked before connect/retry/send. */
  readonly assertOwned?:()=>void|Promise<void>;
  readonly onState?:(state:CollaborationState<Document>)=>void;
  readonly onEvent?:(event:{type:string;data:unknown})=>void;
  readonly onConnection?:(state:"connecting"|"connected"|"disconnected"|"closed",error?:Error)=>void;
  readonly requestTimeoutMs?:number; readonly maxPendingBytes?:number; readonly maxPendingRequests?:number;
}
export class CollaborationRequestError extends Error {
  constructor(message:string,readonly code:string,readonly status:number){super(message);}
}
const clone=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const byteLength=(value:unknown)=>new TextEncoder().encode(JSON.stringify(value)).length;
/** getRandomValues is available on the trusted HTTP/Tailscale demo as well as
 * HTTPS; randomUUID alone is restricted to secure browser contexts. */
export function collaborationRequestId():string{
  return Array.from(globalThis.crypto.getRandomValues(new Uint8Array(16)),byte=>byte.toString(16).padStart(2,"0")).join("");
}

/** Connected HTTP/SSE client. This contains no server authority, model equations,
 * text algorithm or canvas navigation. Unknown outcomes retain their original
 * IDs and exact payloads until the server returns a durable receipt.
 */
export class CollaborationClient<Document=unknown> {
  connection?:Connection;
  state?:CollaborationState<Document>;
  private token?:string;
  private readonly pending=new Map<string,PendingRequest>();
  private pendingIdentity?:Omit<PendingCheckpoint,"requests">;
  private readonly receipts=new Map<string,Receipt>();
  private closed=false;
  private connecting?:Promise<CollaborationState<Document>>;
  private eventController?:AbortController;
  private readonly requests=new Set<AbortController>();
  private epoch=0;
  private lastSequence=0;
  private ingressTail:Promise<unknown>=Promise.resolve();
  private persistTail:Promise<unknown>=Promise.resolve();
  private reconnectTimer?:ReturnType<typeof setTimeout>;
  private retryMs=250;
  private heartbeatTimer?:ReturnType<typeof setTimeout>;
  private readonly fetch:typeof fetch;
  private readonly base:string;
  constructor(private readonly options:ClientOptions<Document>){
    this.base=new URL(options.baseUrl).href.replace(/\/?$/u,"/");
    this.fetch=options.fetch??globalThis.fetch.bind(globalThis);
    for(const value of [options.requestTimeoutMs??30_000,options.maxPendingBytes??8*1024*1024,options.maxPendingRequests??128])
      if(!Number.isSafeInteger(value)||value<1)throw Error("Client limits must be positive safe integers");
    if(options.pending){
      const p=clone(options.pending);
      if(p.format!=="geosolve-client-pending-v1"||p.clientId!==options.clientId||!Array.isArray(p.requests))throw Error("Pending work belongs to another client");
      this.pendingIdentity={format:p.format,documentId:p.documentId,documentEpoch:p.documentEpoch,userId:p.userId,clientId:p.clientId};
      for(const item of p.requests){
        if(!["commands","text"].includes(item.route)||typeof item.requestId!=="string"||item.body?.requestId!==item.requestId||this.pending.has(item.requestId))throw Error("Malformed pending request");
        this.pending.set(item.requestId,item);
      }
      this.checkBounds();
    }
  }
  get pendingRequests():readonly PendingRequest[]{return clone([...this.pending.values()]);}
  get connected():boolean{return !!this.token&&!this.closed;}
  checkpoint():PendingCheckpoint{
    if(!this.pendingIdentity)throw Error("Join a document before storing pending work");
    return {...this.pendingIdentity,requests:this.pendingRequests};
  }
  connect():Promise<CollaborationState<Document>>{
    if(this.closed)return Promise.reject(Error("Collaboration client is closed"));
    if(this.connecting)return this.connecting;
    clearTimeout(this.reconnectTimer);this.reconnectTimer=undefined;
    this.eventController?.abort();this.token=undefined;
    const epoch=++this.epoch;
    this.options.onConnection?.("connecting");
    this.connecting=(async()=>{
      const joined=await this.rpc<{connection:Connection;token:string}>("join",{protocol:1,inviteToken:this.options.inviteToken,clientId:this.options.clientId},false);
      if(this.closed||epoch!==this.epoch)throw Error("Connection was superseded");
      const c=joined.connection;
      if(c.protocol!==1||c.clientId!==this.options.clientId||!c.documentId||!c.documentEpoch||!c.userId||!["editor","viewer"].includes(c.role))throw Error("Invalid collaboration join identity");
      const identity={format:"geosolve-client-pending-v1" as const,documentId:c.documentId,documentEpoch:c.documentEpoch,userId:c.userId,clientId:c.clientId};
      if(this.pendingIdentity&&this.pending.size&&JSON.stringify(identity)!==JSON.stringify(this.pendingIdentity))throw Error("Pending work belongs to another document or user; export it before joining this document");
      this.connection=c;this.token=joined.token;this.pendingIdentity=identity;
      const state=await this.refresh();this.lastSequence=state.authority.latestSequence;
      this.startEvents(epoch);
      // Original IDs are replayed even when the response was lost after fsync.
      for(const request of [...this.pending.values()]){
        try{await this.sendPending(request);}
        catch(error){if(!(error instanceof CollaborationRequestError)||error.code!=="text_rejected")throw error;}
      }
      this.retryMs=250;this.scheduleHeartbeat(epoch);this.options.onConnection?.("connected");return state;
    })().catch(error=>{this.disconnected(error,epoch);throw error;}).finally(()=>{this.connecting=undefined;});
    return this.connecting;
  }
  async refresh():Promise<CollaborationState<Document>>{
    const state=await this.rpc<CollaborationState<Document>>("state");
    this.state=state;this.options.onState?.(state);return state;
  }
  async textDelta(revision:TextRevision):Promise<{sourceSequence:number;workingRevision:TextRevision;changes:readonly (readonly number[])[];history:UserTextHistory}>{
    return this.rpc("text-state",{revision});
  }
  /** Disposable compute: never persisted, queued with edits, or automatically retried. */
  async authoringPreview<T=unknown>(request:unknown,signal?:AbortSignal):Promise<T>{
    return await(await this.response("authoring-preview",request,true,signal)).json() as T;
  }
  async scene<T>():Promise<{revision:number;value:T}>{
    const response=await this.response("scene");
    const revision=Number(response.headers.get("X-Geosolve-Revision"));
    if(!Number.isSafeInteger(revision)||revision<0)throw Error("Scene response has no accepted revision");
    return {revision,value:await response.json() as T};
  }
  /** Text batches serialize within a client. Other clients and model solving
   * continue independently. Keep native change bytes while disconnected. */
  writeText(changes:readonly Uint8Array[],requestId=collaborationRequestId()):Promise<unknown>{
    const item=this.add("text",requestId,{requestId,changes:changes.map(bytes=>Array.from(bytes))});
    return this.enqueue(item);
  }
  /** Ordered draft lifecycle and personal history use the same durable source
   * gateway as typing. They do not apply or replace the accepted model. */
  editFiles(edits:readonly Exclude<TextEdit,{kind:"splice"}>[],revision:TextRevision,requestId=collaborationRequestId()):Promise<unknown>{
    return this.enqueue(this.add("text",requestId,{requestId,action:"files",revision,edits}));
  }
  editWorking(edits:readonly TextEdit[],revision:TextRevision,requestId=collaborationRequestId()):Promise<unknown>{
    return this.enqueue(this.add("text",requestId,{requestId,action:"working",revision,edits}));
  }
  undoText(redo=false,requestId=collaborationRequestId()):Promise<unknown>{
    return this.enqueue(this.add("text",requestId,{requestId,action:redo?"redo":"undo"}));
  }
  /** Apply waits for this client's earlier text ACKs before server capture. */
  async submit(command:Command,requestId=collaborationRequestId()):Promise<Receipt>{
    const item=this.add("commands",requestId,{requestId,command});
    return await this.enqueue(item) as Receipt;
  }
  async receipt(requestId:string):Promise<Receipt|null>{
    const {receipt}=await this.rpc<{receipt:Receipt|null}>(`receipt?requestId=${encodeURIComponent(requestId)}`);
    if(receipt)await this.observeReceipt(receipt);return receipt;
  }
  async result<T=unknown>(requestId:string):Promise<T|null>{
    return (await this.rpc<{result:T|null}>(`result?requestId=${encodeURIComponent(requestId)}`)).result;
  }
  presence(value:{sequence:number;cursor?:readonly[number,number]|null;selection?:readonly string[]}):Promise<unknown>{return this.rpc("presence",value);}
  /** Explicit reconnect also retries requests that received admission backpressure. */
  async retry():Promise<CollaborationState<Document>>{const state=await this.connect();this.ingressTail=Promise.resolve();return state;}
  async leave():Promise<void>{
    try{if(this.token&&!this.closed)await this.rpc("leave",{});}finally{this.dispose();}
  }
  dispose():void{
    if(this.closed)return;this.closed=true;++this.epoch;clearTimeout(this.reconnectTimer);clearTimeout(this.heartbeatTimer);
    this.eventController?.abort();for(const request of this.requests)request.abort();this.token=undefined;
    this.options.onConnection?.("closed");
  }
  private add(route:PendingRequest["route"],requestId:string,body:Record<string,unknown>):PendingRequest{
    if(this.closed||!this.connection||!this.pendingIdentity)throw Error("Join before authoring; retain local work until connected");
    if(this.connection.role!=="editor")throw Error("This document connection is read only");
    const item=clone({route,requestId,body}),existing=this.pending.get(requestId);
    if(existing){if(JSON.stringify(existing)!==JSON.stringify(item))throw Error("Pending request ID was reused");return existing;}
    this.pending.set(requestId,item);
    try{this.checkBounds();}catch(error){this.pending.delete(requestId);throw error;}
    return item;
  }
  private checkBounds():void{
    if(this.pending.size>(this.options.maxPendingRequests??128)||byteLength([...this.pending.values()])>(this.options.maxPendingBytes??8*1024*1024))throw Error("Pending work is full; reconnect before adding more edits");
  }
  private persist():Promise<void>{
    const checkpoint=this.checkpoint();
    const run=this.persistTail.catch(()=>{}).then(async()=>{
      await this.options.savePending?.(checkpoint);
      this.options.onEvent?.({type:"pending_saved",data:{requestIds:checkpoint.requests.map(request=>request.requestId)}});
    });
    this.persistTail=run;return run;
  }
  private enqueue(item:PendingRequest):Promise<unknown>{
    // Persist immediately even if an earlier unknown outcome blocks admission.
    const saved=this.persist();
    const operation=this.ingressTail.then(async()=>{await saved;return this.sendPending(item);});
    this.ingressTail=operation.catch(error=>{
      // A fsynced source refusal is terminal. Unknown transport outcomes still
      // block ordering until exact replay; a rejected Undo must not strand typing.
      if(error instanceof CollaborationRequestError&&error.code==="text_rejected")return;
      throw error;
    });
    void this.ingressTail.catch(()=>{});
    void saved.catch(()=>{});void operation.catch(()=>{});return operation;
  }
  private async sendPending(item:PendingRequest):Promise<unknown>{
    // A reconnect replay never substitutes a new ID or a newer command payload.
    try{
      const result=await this.rpc<unknown>(item.route,item.body);
      if(item.route==="commands"){
        const receipt=(result as {receipt:Receipt}).receipt;await this.observeReceipt(receipt);return receipt;
      }
      this.pending.delete(item.requestId);await this.persist();
      this.options.onEvent?.({type:"text_receipt",data:{requestId:item.requestId,ack:result}});return result;
    }catch(error){
      if(error instanceof CollaborationRequestError&&error.code==="text_rejected"){
        this.pending.delete(item.requestId);await this.persist();
        this.options.onEvent?.({type:"text_refused",data:{requestId:item.requestId,message:error.message}});
      }else if(!(error instanceof CollaborationRequestError)||[401,503].includes(error.status))this.disconnected(error,this.epoch);
      throw error;
    }
  }
  private async observeReceipt(receipt:Receipt):Promise<void>{
    if(receipt.operation.userId!==this.connection?.userId||receipt.operation.clientId!==this.options.clientId)return;
    this.receipts.set(receipt.operation.requestId,receipt);
    // Receipt retention is disposable; durable retry remains on the server.
    if(this.receipts.size>256)this.receipts.delete(this.receipts.keys().next().value!);
    if(receipt.outcome&&this.pending.delete(receipt.operation.requestId))await this.persist();
    this.options.onEvent?.({type:"receipt",data:receipt});
  }
  private startEvents(epoch:number):void{
    const controller=new AbortController();this.eventController=controller;
    void(async()=>{
      await this.options.assertOwned?.();
      if(this.closed||epoch!==this.epoch)return;
      const response=await this.fetch(new URL(`events?after=${this.lastSequence}`,this.base),{headers:{Authorization:`Bearer ${this.token}`},signal:controller.signal});
      await this.checkResponse(response);
      if(!response.body||!response.headers.get("Content-Type")?.startsWith("text/event-stream"))throw Error("Missing collaboration event stream");
      for await(const message of readCollaborationEvents(response.body)){
        if(this.closed||epoch!==this.epoch)return;
        if(message.type==="operation"){
          const record=message.data as JournalRecord;
          if(record.protocol!==1||record.documentId!==this.connection?.documentId||record.documentEpoch!==this.connection.documentEpoch||!Number.isSafeInteger(record.sequence))throw Error("Foreign or malformed document event");
          if(record.sequence<=this.lastSequence)continue;
          if(record.sequence!==this.lastSequence+1)throw Error("Document event gap requires reconnect");
          this.lastSequence=record.sequence;
          if(record.event.event==="finished"){
            const previous=this.receipts.get(record.event.operation.requestId);
            if(previous)await this.observeReceipt({...previous,outcome:record.event.outcome});
            else if(this.pending.has(record.event.operation.requestId))await this.receipt(record.event.operation.requestId);
          }
        }else if(message.type==="checkpoint_required")throw Error("Document event checkpoint required");
        this.options.onEvent?.(message);
      }
      throw Error("Document event stream disconnected");
    })().catch(error=>{if(!controller.signal.aborted)this.disconnected(error,epoch);});
  }
  private scheduleHeartbeat(epoch:number):void{
    clearTimeout(this.heartbeatTimer);
    this.heartbeatTimer=setTimeout(()=>{void this.rpc("heartbeat",{}).then(()=>{
      if(!this.closed&&epoch===this.epoch)this.scheduleHeartbeat(epoch);
    },error=>this.disconnected(error,epoch));},20_000);
  }
  private disconnected(error:unknown,epoch:number):void{
    if(this.closed||epoch!==this.epoch)return;
    clearTimeout(this.heartbeatTimer);this.token=undefined;this.eventController?.abort();
    this.options.onConnection?.("disconnected",error instanceof Error?error:Error(String(error)));
    if(this.reconnectTimer)return;
    this.reconnectTimer=setTimeout(()=>{this.reconnectTimer=undefined;void this.retry().catch(()=>{});},this.retryMs);
    this.retryMs=Math.min(this.retryMs*2,10_000);
  }
  private async rpc<T>(route:string,body?:unknown,authenticated=true):Promise<T>{
    return await(await this.response(route,body,authenticated)).json() as T;
  }
  private async response(route:string,body?:unknown,authenticated=true,signal?:AbortSignal):Promise<Response>{
    if(this.closed)throw Error("Collaboration client is closed");
    await this.options.assertOwned?.();
    if(this.closed)throw Error("Collaboration client is closed");
    if(authenticated&&!this.token)throw Error("Document connection is unavailable; pending work is retained");
    const controller=new AbortController();this.requests.add(controller);
    const abort=()=>controller.abort();signal?.addEventListener("abort",abort,{once:true});if(signal?.aborted)abort();
    const timeout=setTimeout(()=>controller.abort(),this.options.requestTimeoutMs??30_000);
    try{
      const response=await this.fetch(new URL(route,this.base),{method:body===undefined?"GET":"POST",headers:{...(body===undefined?{}:{"Content-Type":"application/json"}),...(authenticated?{Authorization:`Bearer ${this.token}`}:{})},...(body===undefined?{}:{body:JSON.stringify(body)}),signal:controller.signal});
      // Keep timeout/cancellation active through the body, not only headers.
      // Snapshots can be large, but a response cannot grow memory without bound.
      const maximum=route==="scene"||route==="authoring-preview"?64*1024*1024:128*1024*1024;
      const reader=response.body?.getReader(),chunks:Uint8Array[]=[];let length=0;
      if(reader)try{
        while(true){const part=await reader.read();if(part.done)break;length+=part.value.byteLength;
          if(length>maximum)throw Error("Collaboration response exceeds byte limit");chunks.push(part.value);}
      }finally{await reader.cancel().catch(()=>{});reader.releaseLock();}
      const bytes=new Uint8Array(length);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
      const complete=new Response(bytes,{status:response.status,headers:response.headers});
      await this.checkResponse(complete);return complete;
    }finally{clearTimeout(timeout);signal?.removeEventListener("abort",abort);this.requests.delete(controller);}
  }
  private async checkResponse(response:Response):Promise<void>{
    if(response.ok)return;
    let detail:{error?:{message?:string;code?:string}}={};try{detail=await response.json();}catch{/* HTTP status is still a definite transport rejection. */}
    throw new CollaborationRequestError(detail.error?.message??`Collaboration request failed (${response.status})`,detail.error?.code??"http_error",response.status);
  }
}

/** Fetch-based SSE permits Authorization headers without exposing tokens in URLs.
 * Bounded decoding handles split UTF-8, CRLF and multiple data lines. */
export async function* readCollaborationEvents(body:ReadableStream<Uint8Array>,maxFrameBytes=1024*1024):AsyncGenerator<{type:string;data:unknown}>{
  const reader=body.getReader(),decoder=new TextDecoder("utf-8",{fatal:true});
  let buffer="",type="message",data:string[]=[],frameBytes=0;
  try{
    while(true){
      const part=await reader.read();
      if(part.done){buffer+=decoder.decode();if(buffer.length||data.length)throw Error("Truncated collaboration event");return;}
      buffer+=decoder.decode(part.value,{stream:true});
      let index;
      while((index=buffer.indexOf("\n"))>=0){
        const line=buffer.slice(0,index).replace(/\r$/u,"");buffer=buffer.slice(index+1);
        frameBytes+=new TextEncoder().encode(line).length+1;
        if(frameBytes>maxFrameBytes)throw Error("Collaboration event exceeds frame limit");
        if(line===""){
          if(data.length)yield {type,data:JSON.parse(data.join("\n"))};
          type="message";data=[];frameBytes=0;
        }else if(!line.startsWith(":")){
          const colon=line.indexOf(":"),field=colon<0?line:line.slice(0,colon),value=colon<0?"":line.slice(colon+1).replace(/^ /u,"");
          if(field==="event")type=value;else if(field==="data")data.push(value);
        }
      }
      if(frameBytes+new TextEncoder().encode(buffer).length>maxFrameBytes)throw Error("Collaboration event exceeds frame limit");
    }
  }finally{await reader.cancel().catch(()=>{});reader.releaseLock();}
}
