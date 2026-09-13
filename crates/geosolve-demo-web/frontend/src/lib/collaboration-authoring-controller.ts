// SPDX-License-Identifier: GPL-3.0-or-later
import type { ToolOperationTool, ToolOperationOperand, ToolOperationEvent, ToolOperationCommand, ToolOperationFrame } from "../../../../../packages/geosolve-engine/src/tool-operations";
import type { ConstructionFrame } from "../../../../../packages/geosolve-engine/src/construction";
import type { PointerSample } from "./adapter";
import type { AuthoringPointer } from "./local-interaction-adapter";
import type { AuthoringModel, AuthoringPreview, AuthoringView, LocalAuthoringClient } from "./collaboration-authoring-adapter";
import type { PointGestureCommand, PointGestureSample, ConstructionCommand, ConstructionEvent } from "../../../../../packages/geosolve-engine/src/index";

import { constructionTools as tools, operationTools } from "../../../../../packages/geosolve-engine/src/tool-catalog";
export function supportsCollaborativeConstruction(tool:string){return Object.hasOwn(tools,tool);}
export function supportsCollaborativeTool(tool:string){return tool === "select" || supportsCollaborativeConstruction(tool) || Object.hasOwn(operationTools,tool);}
export interface OperationStart { view:AuthoringView; viewport:AuthoringPointer["viewport"]; selection?:readonly ToolOperationOperand[] }
export type AuthoringCommand=PointGestureCommand|ConstructionCommand|ToolOperationCommand;
type Gesture={id:number;generation:number;kind:"point"|"construction"|"operation";pointerId?:number;sequence:number;started:boolean;finishing?:boolean;committing?:boolean;projection:AuthoringPointer;origin:readonly[number,number];model:AuthoringModel;view:AuthoringView};
/** Event routing only. Rust owns picking/projection, inference, geometry and
 * command replay. This queue is never awaited by canvas input/navigation. */
export class CollaborationAuthoringController {
  tool="select";
  role:"profile"|"construction"="profile";
  canFinish=false;
  construction?:ConstructionFrame;
  operation?:ToolOperationFrame;
  private operationOptions=new Map<ToolOperationTool,Partial<Pick<ToolOperationFrame,"authoring_options"|"fillet_options"|"offset_distance">>>();
  private constructionOptions?:Pick<ConstructionFrame,"conic_options"|"nurbs_options">;
  private model?:AuthoringModel;
  private ready:Promise<unknown>=Promise.resolve();
  private generation=0;
  private id=0;
  private gesture?:Gesture;
  private tail:Promise<unknown>=Promise.resolve();
  private queued=0;
  private pointSamples=0;
  private pointBatch?:{gesture:Gesture;samples:PointGestureSample[];view:AuthoringView};
  private disposed=false;
  constructor(private readonly worker:LocalAuthoringClient,private readonly callbacks:{
    paint:(preview:AuthoringPreview)=>void;
    changed:()=>void;
    cleared?:()=>void;
    activity?:()=>()=>void;
    error:(error:unknown)=>void;
    commit:(model:AuthoringModel,command:AuthoringCommand,kind:Gesture["kind"])=>Promise<void>;
  }){}
  replace(model:AuthoringModel){
    const current=this.model;
    if(current&&current.documentEpoch===model.documentEpoch&&current.revision===model.revision&&current.sourceDesignDigest===model.sourceDesignDigest
      &&current.project===model.project&&JSON.stringify(current.design)===JSON.stringify(model.design))return;
    ++this.generation;this.model=model;this.gesture=undefined;this.canFinish=false;this.construction=undefined;this.operation=undefined;this.pointBatch=undefined;
    const generation=this.generation;
    this.ready=this.worker.replace(model);void this.ready.catch(error=>{if(generation===this.generation&&!this.disposed){this.model=undefined;this.callbacks.error(error);}});this.callbacks.cleared?.();this.callbacks.changed();
  }
  select(tool:string,start?:OperationStart){
    if(!supportsCollaborativeTool(tool))throw Error("This tool is not connected to collaborative authoring yet");
    this.cancel();this.tool=tool;
    if(start){if(Object.hasOwn(operationTools,tool))this.beginOperation(start);else if(supportsCollaborativeConstruction(tool))this.beginConstruction(start);}
    this.callbacks.changed();
  }
  toggleRole(){this.cancel();this.role=this.role==="profile"?"construction":"profile";this.callbacks.changed();}
  pointer(input:PointerSample,projection:AuthoringPointer,view:AuthoringView){
    if(!this.model||input.buttons===4)return;
    const captured=this.gesture;
    if(captured?.finishing||captured?.committing)return;
    if(this.tool==="select"){
      if(input.phase==="down"&&input.buttons===1&&!input.modifiers.shift&&!input.modifiers.ctrl&&!input.modifiers.meta&&projection.target){
        this.gesture={id:++this.id,generation:this.generation,kind:"point",pointerId:input.pointerId,sequence:0,started:false,projection,origin:[input.x,input.y],model:this.model,view};return;
      }
      if(!captured||captured.kind!=="point"||captured.pointerId!==input.pointerId)return;
      if(input.phase==="move"&&!captured.started&&Math.hypot(input.x-captured.origin[0],input.y-captured.origin[1])<3)return;
      if(input.phase==="up"&&!captured.started){this.gesture=undefined;return;}
      if(!captured.started){
        captured.started=true;
        // The first movement is already queued below. Await native setup and
        // validation without painting its unchanged origin ahead of that move.
        this.enqueue(captured,async()=>{await this.worker.beginPoint({target:captured.projection.target!,gestureId:captured.id,viewport:captured.projection.viewport,view});});
      }
      if(input.phase==="move"||input.phase==="up"){
        const sequence=++captured.sequence;
        this.advancePoint(captured,{sequence,position:projection.position},view);
        if(input.phase==="up"){
          captured.finishing=true;
          this.enqueue(captured,async()=>{const terminal=await this.worker.finishPoint();this.commit(captured,terminal.terminal.command);});
        }
      }
      return;
    }
    if(Object.hasOwn(operationTools,this.tool)){
      if(input.phase==="up")return;
      const gesture=this.gesture??this.beginOperation({view,viewport:projection.viewport,selection:[]});
      if(!gesture)return;
      gesture.view=view;
      this.updateViewport(gesture,projection.viewport);
      this.operationEvent({event:input.phase==="down"?"click":"move",position:projection.position});
      return;
    }
    if(input.phase==="up")return;
    let gesture=this.gesture;
    if(!gesture){
      gesture=this.beginConstruction({view,viewport:projection.viewport});
      if(!gesture)return;
    }
    gesture.view=view;
    this.updateViewport(gesture,projection.viewport);
    const sequence=++gesture.sequence,event=input.phase==="down"?"click":"move";
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.advanceConstruction({sequence,input:{event,position:projection.position,suppressed:input.modifiers.alt,regularized:input.modifiers.shift}},view));
  }
  finish(){if(this.gesture?.kind==="operation")this.operationEvent({event:"complete"});else this.constructionEvent({event:"complete"});}
  stepBack(){if(this.gesture?.kind==="operation")this.operationEvent({event:"step_back"});else this.constructionEvent({event:"step_back"});}
  operationEvent(input:ToolOperationEvent){
    const gesture=this.gesture;if(!gesture||gesture.kind!=="operation"||gesture.finishing||gesture.committing)return;
    const sequence=++gesture.sequence;
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.advanceOperation({sequence,input},gesture.view));
  }
  private beginConstruction(start:OperationStart){
    if(!this.model)return;
    const gesture:Gesture={id:++this.id,generation:this.generation,kind:"construction",sequence:0,started:true,projection:{viewport:start.viewport,position:[0,0],target:null},origin:[0,0],model:this.model,view:start.view};
    this.gesture=gesture;
    const tool=tools[this.tool as keyof typeof tools],role=this.role;
    const remembered=this.constructionOptions;
    this.enqueue(gesture,()=>this.worker.beginConstruction({tool,role,gestureId:gesture.id,viewport:start.viewport,view:start.view}));
    if(remembered){
      this.constructionEvent({event:"conic_options",options:remembered.conic_options});
      this.constructionEvent({event:"nurbs_options",options:remembered.nurbs_options});
    }
    return gesture;
  }
  pickSelection(view:AuthoringView,viewport?:AuthoringPointer["viewport"]){
    const gesture=this.gesture;if(!gesture||gesture.kind!=="operation"||gesture.finishing||gesture.committing)return;
    gesture.view=view;if(viewport)this.updateViewport(gesture,viewport);const sequence=++gesture.sequence;
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.pickOperationSelection(sequence,view));
  }
  applyOperation(tool:ToolOperationTool,start:OperationStart){this.cancel();this.beginOperation(start,tool);}
  private beginOperation(start:OperationStart,requestedTool?:ToolOperationTool){
    if(!this.model)return;
    const gesture:Gesture={id:++this.id,generation:this.generation,kind:"operation",sequence:0,started:true,projection:{viewport:start.viewport,position:[0,0],target:null},origin:[0,0],model:this.model,view:start.view};
    this.gesture=gesture;
    const tool=requestedTool??operationTools[this.tool as keyof typeof operationTools];
    this.enqueue(gesture,()=>this.worker.beginOperation({tool,gestureId:gesture.id,...start,options:this.operationOptions.get(tool)}));
    return gesture;
  }
  constructionEvent(input:ConstructionEvent){
    const gesture=this.gesture;if(!gesture||gesture.kind!=="construction"||gesture.finishing||gesture.committing)return;
    const sequence=++gesture.sequence;
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.advanceConstruction({sequence,input},gesture.view));
  }
  private updateViewport(gesture:Gesture,viewport:AuthoringPointer["viewport"]){
    if(JSON.stringify(gesture.projection.viewport)===JSON.stringify(viewport))return;
    gesture.projection={...gesture.projection,viewport};
    // Camera changes stay local while browsing. Immediately before another
    // authoring pick, record its exact camera so native pixel tolerances replay
    // identically on both predictors and the authoritative server.
    if(gesture.kind==="operation")this.operationEvent({event:"viewport",viewport});
    else if(gesture.kind==="construction")this.constructionEvent({event:"viewport",viewport});
  }
  render(view:AuthoringView){const gesture=this.gesture;if(gesture?.started){gesture.view=view;this.enqueue(gesture,()=>this.worker.render(view));}}
  cancel(){
    this.gesture=undefined;this.canFinish=false;this.construction=undefined;this.operation=undefined;this.pointBatch=undefined;this.callbacks.cleared?.();
    const ready=this.ready,generation=this.generation;
    const result=this.tail.then(async()=>{await ready;if(generation===this.generation&&!this.disposed)await this.worker.cancel();});
    this.tail=result.catch(()=>{});this.callbacks.changed();
  }
  dispose(){this.disposed=true;this.gesture=undefined;this.worker.dispose();}
  private advancePoint(gesture:Gesture,sample:PointGestureSample,view:AuthoringView){
    if(this.pointSamples>=4096){this.callbacks.error(Error("Drawing queue is full; finish or cancel the gesture"));this.cancel();return;}
    this.pointSamples++;gesture.view=view;
    const pending=this.pointBatch;
    if(pending?.gesture===gesture&&pending.samples.length<256){pending.samples.push(sample);pending.view=view;return;}
    const batch={gesture,samples:[sample],view};this.pointBatch=batch;
    this.enqueue(gesture,async()=>{
      if(this.pointBatch===batch)this.pointBatch=undefined;
      // Admit every path sample to the native mailbox together. Its bounded
      // batching skips intermediate paint, never native path/branch evaluation.
      const previews=await Promise.all(batch.samples.map(sample=>this.worker.advancePoint(sample,batch.view)));
      return previews.at(-1);
    },()=>{this.pointSamples-=batch.samples.length;});
  }
  private commit(gesture:Gesture,command:AuthoringCommand){
    if(this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
    gesture.committing=true;this.canFinish=false;this.callbacks.changed();
    // Durable publication must not occupy the disposable prediction queue:
    // native reprojection remains usable while this terminal awaits its ACK.
    void this.callbacks.commit(gesture.model,command,gesture.kind).then(()=>{
      if(gesture.generation===this.generation&&this.gesture===gesture)this.cancel();
    },error=>{
      if(gesture.generation===this.generation&&this.gesture===gesture&&!this.disposed){this.callbacks.error(error);this.cancel();}
    });
  }
  private enqueue(gesture:Gesture,run:()=>Promise<AuthoringPreview|void>,finished?:()=>void){
    if(this.queued>=4096){finished?.();this.callbacks.error(Error("Drawing queue is full; finish or cancel the gesture"));this.cancel();return;}
    this.queued++;
    const ready=this.ready;
    let finishActivity:(()=>void)|undefined;
    const result=this.tail.then(async()=>{
      if(this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
      finishActivity=this.callbacks.activity?.();
      await ready;
      if(this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
      const preview=await run();
      if(!preview||this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
      this.construction=preview.construction;this.operation=preview.operation;
      if(preview.operation){const tool=operationTools[this.tool as keyof typeof operationTools];if(tool)this.operationOptions.set(tool,{authoring_options:preview.operation.authoring_options,fillet_options:preview.operation.fillet_options,offset_distance:preview.operation.offset_distance});}
      if(preview.construction)this.constructionOptions={conic_options:preview.construction.conic_options,nurbs_options:preview.construction.nurbs_options};
      this.canFinish=preview.construction?.can_finish??preview.operation?.can_finish??false;this.callbacks.paint(preview);this.callbacks.changed();
      if(preview.construction?.completed){
        gesture.finishing=true;const terminal=await this.worker.finishConstruction();this.commit(gesture,terminal.command);
      }else if(preview.operation?.completed){
        gesture.finishing=true;const terminal=await this.worker.finishOperation();this.commit(gesture,terminal.command);
      }
    }).catch(error=>{if(gesture.generation===this.generation&&this.gesture===gesture&&!this.disposed){this.callbacks.error(error);this.cancel();}}).finally(()=>{finishActivity?.();this.queued--;finished?.();});
    this.tail=result;
  }
}
