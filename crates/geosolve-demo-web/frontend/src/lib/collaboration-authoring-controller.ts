// SPDX-License-Identifier: GPL-3.0-or-later
import type { PointerSample } from "./adapter";
import type { AuthoringPointer } from "./local-interaction-adapter";
import type { AuthoringModel, AuthoringPreview, AuthoringView, LocalAuthoringClient } from "./collaboration-authoring-adapter";
import type { ConstructionTool, PointGestureCommand, PointGestureSample, ConstructionCommand } from "../../../../../packages/geosolve-engine/src/index";

const tools:Readonly<Record<string,ConstructionTool>>={segment:"segment",polyline:"polyline","center-radius-circle":"center_radius_circle","two-point-aligned-rectangle":"two_point_aligned_rectangle"};
export function supportsCollaborativeConstruction(tool:string){return Object.hasOwn(tools,tool);}
type Gesture={id:number;generation:number;kind:"point"|"construction";pointerId?:number;sequence:number;started:boolean;finishing?:boolean;committing?:boolean;projection:AuthoringPointer;origin:readonly[number,number];model:AuthoringModel;view:AuthoringView};
/** Event routing only. Rust owns picking/projection, inference, geometry and
 * command replay. This queue is never awaited by canvas input/navigation. */
export class CollaborationAuthoringController {
  tool="select";
  role:"profile"|"construction"="profile";
  canFinish=false;
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
    error:(error:unknown)=>void;
    commit:(model:AuthoringModel,command:PointGestureCommand|ConstructionCommand,kind:"point"|"construction")=>Promise<void>;
  }){}
  replace(model:AuthoringModel){
    const current=this.model;
    if(current&&current.documentEpoch===model.documentEpoch&&current.revision===model.revision&&current.sourceDesignDigest===model.sourceDesignDigest
      &&current.project===model.project&&JSON.stringify(current.design)===JSON.stringify(model.design))return;
    ++this.generation;this.model=model;this.gesture=undefined;this.canFinish=false;this.pointBatch=undefined;
    const generation=this.generation;
    this.ready=this.worker.replace(model);void this.ready.catch(error=>{if(generation===this.generation&&!this.disposed){this.model=undefined;this.callbacks.error(error);}});this.callbacks.cleared?.();this.callbacks.changed();
  }
  select(tool:string){
    if(tool!=="select"&&!supportsCollaborativeConstruction(tool))throw Error("This tool is not connected to collaborative authoring yet");
    this.cancel();this.tool=tool;this.callbacks.changed();
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
      if(!captured.started){captured.started=true;this.enqueue(captured,()=>this.worker.beginPoint({target:captured.projection.target!,gestureId:captured.id,viewport:captured.projection.viewport,view}));}
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
    if(input.phase==="up")return;
    let gesture=this.gesture;
    if(!gesture){
      gesture={id:++this.id,generation:this.generation,kind:"construction",sequence:0,started:true,projection,origin:[input.x,input.y],model:this.model,view};
      this.gesture=gesture;
      const tool=tools[this.tool],role=this.role;
      this.enqueue(gesture,()=>this.worker.beginConstruction({tool,role,gestureId:gesture!.id,viewport:projection.viewport,view}));
    }
    gesture.view=view;
    const sequence=++gesture.sequence,event=input.phase==="down"?"click":"move";
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.advanceConstruction({sequence,input:{event,position:projection.position,suppressed:input.modifiers.alt,regularized:input.modifiers.shift}},view));
  }
  finish(){
    const gesture=this.gesture;if(!gesture||gesture.kind!=="construction"||gesture.finishing||gesture.committing)return;
    const sequence=++gesture.sequence;
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.advanceConstruction({sequence,input:{event:"complete"}},gesture.view));
  }
  stepBack(){
    const gesture=this.gesture;if(!gesture||gesture.kind!=="construction"||gesture.finishing||gesture.committing)return;
    const sequence=++gesture.sequence;
    this.enqueue(gesture,()=>gesture.finishing||gesture.committing?Promise.resolve():this.worker.advanceConstruction({sequence,input:{event:"step_back"}},gesture.view));
  }
  render(view:AuthoringView){const gesture=this.gesture;if(gesture?.started){gesture.view=view;this.enqueue(gesture,()=>this.worker.render(view));}}
  cancel(){
    this.gesture=undefined;this.canFinish=false;this.pointBatch=undefined;this.callbacks.cleared?.();
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
  private commit(gesture:Gesture,command:PointGestureCommand|ConstructionCommand){
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
    const result=this.tail.then(async()=>{
      if(this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
      await ready;
      if(this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
      const preview=await run();
      if(!preview||this.disposed||gesture.generation!==this.generation||this.gesture!==gesture)return;
      this.canFinish=preview.construction?.can_finish??false;this.callbacks.paint(preview);this.callbacks.changed();
      if(preview.construction?.completed){
        gesture.finishing=true;const terminal=await this.worker.finishConstruction();this.commit(gesture,terminal.command);
      }
    }).catch(error=>{if(gesture.generation===this.generation&&this.gesture===gesture&&!this.disposed){this.callbacks.error(error);this.cancel();}}).finally(()=>{this.queued--;finished?.();});
    this.tail=result;
  }
}
