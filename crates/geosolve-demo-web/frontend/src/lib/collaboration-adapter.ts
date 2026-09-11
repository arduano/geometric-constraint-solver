// SPDX-License-Identifier: GPL-3.0-or-later
import { CollaborationClient, collaborationRequestId, type CollaborationState, type Command, type Participant, type PendingCheckpoint, type Receipt, type JournalRecord } from "../../../../../packages/geosolve-collaboration/src/client";
import type { SourceSnapshot, UserTextHistory } from "../../../../../packages/geosolve-collaboration/src/source";
import type { TextSnapshot } from "../../../../../packages/geosolve-collaboration/src/index";
import type { SemanticTarget, UserSemanticHistory } from "../../../../../packages/geosolve-collaboration/src/semantic";
import type { PointGestureHandle } from "../../../../../packages/geosolve-engine/src/point-gesture";
import type { EditableDesign, PointGestureCommand } from "../../../../../packages/geosolve-engine/src/index";
import { assertWorkbenchSnapshot, markCanvasOnlySnapshot, stampCanvasSnapshot, type PointerSample, type WheelSample, type WorkbenchAdapter, type WorkbenchSnapshot } from "./adapter";
import { LocalInteractionWorker, type InteractionSeed, type LocalInteractionClient, type LocalInteractionUpdate } from "./local-interaction-adapter";
import { CollaborationTextWorker, type TextRevision, type TextWorkerUpdate } from "./collaboration-text-adapter";
import { WorkbenchActivity } from "./workbench-activity";
import { assertToolCatalog, type ToolCatalog } from "./tool-catalog";
import type { SourceEditorEdit } from "../components/code-editor";
import { LocalAuthoringWorker, type LocalAuthoringClient, type AuthoringModel, type AuthoringPreview, type AuthoringView } from "./collaboration-authoring-adapter";
import { CollaborationAuthoringController, supportsCollaborativeTool, type AuthoringCommand } from "./collaboration-authoring-controller";
import { projectAuthoredSource, projectCanonicalSource, projectSourceNavigation, type SourceProjection } from "./collaboration-source-projection";
import type { BrowsingNavigationCommand, BrowsingEditCommand, BrowsingChrome } from "./collaboration-browsing-worker";
import type { ManagedSketchMutation } from "./managed-compiler";
import { LocalBrowsingWorker, type LocalBrowsingClient } from "./collaboration-browsing-adapter";
import { createRemoteAuthoringClient } from "./collaboration-remote-authoring";

export interface CollaborativeDocument extends SourceSnapshot {
  readonly authoringPreview?:{readonly server:boolean;readonly preferred:"client"|"server"};
  readonly textHistory:UserTextHistory;
  readonly semanticHistory:UserSemanticHistory;
  readonly targets:Readonly<Record<string,SemanticTarget>>;
  readonly documentTarget:SemanticTarget;
  readonly sourceProjection?:SourceProjection;
  readonly mirror?:{status:string;notices:readonly {path:string;reason:string}[]};
  readonly textActor:readonly number[];
  readonly textCheckpoint:readonly number[];
  readonly pointTargets:readonly PointGestureHandle[];
  readonly model:{readonly project:string;readonly design:EditableDesign;readonly sourceDesignDigest:string};
  readonly inventory:{readonly objects:readonly {object:string;declaration:string;dependencies:readonly string[]}[];
    readonly properties:readonly {object:string;declaration:string;path:readonly unknown[];value:unknown}[]};
}
export interface CollaborativeScene {snapshot:WorkbenchSnapshot;seed:InteractionSeed;toolCatalog:ToolCatalog}
export type CollaborativeTextClient=Pick<CollaborationTextWorker,"open"|"edit"|"receive"|"undo"|"redo"|"snapshot"|"dispose">;
interface PeerPresence {userId:string;clientId:string;sequence:number;cursor:readonly[number,number]|null;selection:readonly string[]}
const localCommands=new Set(["explorer.visibility.set","explorer.visibility.isolate","explorer.visibility.restore","view.construction.toggle","view.fit","view.origin","view.grid.toggle","dimensions.hover","dimensions.hover.clear","dimensions.navigation.begin","dimensions.navigation.end","dimensions.mode","dimensions.pin","dimensions.clearPins","dimensions.focus","selection.clear"]);

/** Each tab owns presentation, shared-text replica and authoring context.
 * The client package owns retry/order; native workers own text and interaction.
 */
export class CollaborativeWorkbenchAdapter implements WorkbenchAdapter {
  readonly activity=new WorkbenchActivity();
  readonly client:CollaborationClient<CollaborativeDocument>;
  notice="Connecting to shared document…";
  participants:readonly Participant[]=[];
  state?:CollaborationState<CollaborativeDocument>;
  private readonly local:LocalInteractionClient;
  private readonly text:CollaborativeTextClient;
  private authoring?:CollaborationAuthoringController;
  private readonly browsing?:LocalBrowsingClient;
  private browsingReady:Promise<unknown>=Promise.resolve();
  private browsingGeneration=0;
  private browsingScheduled=false;
  private browsingAgain=false;
  private seed?:InteractionSeed;
  private view?:AuthoringView;
  private acceptedFrame?:WorkbenchSnapshot["frame"];
  private prediction?:AuthoringPreview;
  private panning?:number;
  private remoteWork?:()=>void;
  private readonly activeRemoteOperations=new Set<string>();
  private installed?:WorkbenchSnapshot;
  private catalog?:ToolCatalog;
  private latestText?:TextWorkerUpdate;
  /** Heads for the text the editor has actually seen. Pending own edits advance
   * this branch without claiming that unseen remote text was displayed. */
  private displayedText?:TextSnapshot;
  private displayId=0;
  private readonly displayedBranches=new Map<number,TextSnapshot>();
  private serverText?:TextRevision;
  private readonly listeners=new Set<(snapshot?:WorkbenchSnapshot)=>void>();
  private localTail:Promise<unknown>=Promise.resolve();
  private textTail:Promise<unknown>=Promise.resolve();
  private refreshTail:Promise<unknown>=Promise.resolve();
  private sequence=0;
  private modelRevision=-1;
  private selectedPath?:string;
  private disposed=false;
  private opening?:Promise<WorkbenchSnapshot>;
  private refreshPending=false;
  private refreshAgain=false;
  private textPending=false;
  private textAgain=false;
  private draftRevision=0;
  private readonly operations=new Map<string,{finish:()=>void;resolve:(receipt:Receipt)=>void;reject:(error:Error)=>void}>();
  private readonly sourceIntents=new Map<number,{path:string;edit:SourceEditorEdit}>();
  private readonly peers=new Map<string,PeerPresence>();
  private presenceTimer?:ReturnType<typeof setTimeout>;
  private presencePaintTimer?:ReturnType<typeof setTimeout>;
  private presenceSequence=0;
  private presenceSending=false;
  private presenceAgain=false;
  private cursor:readonly[number,number]|null=null;
  private ownSelection:readonly string[]=[];
  private readonly sourceRequests=new Map<string,number>();
  private readonly durableSourceIntents=new Set<number>();
  private readonly saveSourceIntents?:(intents:readonly {path:string;edit:SourceEditorEdit}[])=>void;
  recoveryActions:readonly {label:string;run:()=>void}[]=[];
  get responsiveCanvas(){return this.installed!==undefined;}
  get selectedGeometryRoleBlockedReason(){return this.editingBlockedReason??(!this.authoring?"Authoring is unavailable in this session":undefined);}
  get editingBlockedReason(){return this.client.connection?.role==="viewer"?"You have view access to this document.":undefined;}
  get pending(){return this.client.pendingRequests;}
  get pendingSourceEdits(){return [...this.sourceIntents.values()];}
  get textHistory(){return this.state?.document.textHistory;}
  constructor(private readonly options:{baseUrl:string;inviteToken:string;clientId:string;authoringPreview?:"client"|"server";pending?:PendingCheckpoint;fetch?:typeof fetch;assertOwned?:()=>void|Promise<void>;saveSourceIntents?:(intents:readonly {path:string;edit:SourceEditorEdit}[])=>void;savePending?:(checkpoint:PendingCheckpoint)=>void|Promise<void>},local:LocalInteractionClient=new LocalInteractionWorker(),text:CollaborativeTextClient=new CollaborationTextWorker(),private readonly suppliedAuthoring:LocalAuthoringClient|null|undefined=undefined,browsing:LocalBrowsingClient|null=new LocalBrowsingWorker()){
    this.local=local;this.text=text;this.saveSourceIntents=options.saveSourceIntents;
    this.browsing=browsing??undefined;
    this.client=new CollaborationClient({...options,
      onState:state=>{
        if(this.state&&state.authority.acceptedRevision<this.state.authority.acceptedRevision)return;
        this.state=state;this.participants=state.participants;
        this.peers.clear();for(const peer of state.presence??[])this.receivePresence(peer);this.prunePresence();
        if(state.authority.pendingCount>0)this.remoteWork??=this.activity.begin();
        else {this.remoteWork?.();this.remoteWork=undefined;this.activeRemoteOperations.clear();}
      },
      onConnection:(state,error)=>{this.notice=state==="connected"?"Shared document is up to date":state==="connecting"?"Reconnecting…":error?.message??"Connection closed";this.notify();if(state==="connected"&&this.installed){this.scheduleTextRefresh();this.scheduleRefresh();}},
      onEvent:event=>{
        if(event.type==="participants"){this.participants=event.data as Participant[];this.prunePresence();this.notify();}
        if(event.type==="presence")this.receivePresence(event.data);
        if(event.type==="text")this.scheduleTextRefresh();
        if(event.type==="pending_saved"){
          for(const requestId of (event.data as {requestIds:string[]}).requestIds){const intent=this.sourceRequests.get(requestId);if(intent!==undefined)this.durableSourceIntents.add(intent);}
          this.persistSourceIntents();
        }
        if(event.type==="text_receipt"){
          const requestId=(event.data as {requestId:string}).requestId,intent=this.sourceRequests.get(requestId);
          if(intent!==undefined){this.sourceRequests.delete(requestId);this.sourceIntents.delete(intent);this.durableSourceIntents.delete(intent);this.persistSourceIntents();if(this.installed)this.notify(this.projectText());}
        }
        if(event.type==="text_refused"){this.notice=(event.data as {message:string}).message;this.notify();}
        if(event.type==="receipt"){
          const receipt=event.data as Receipt,operation=this.operations.get(receipt.operation.requestId);
          if(receipt.outcome&&operation){this.operations.delete(receipt.operation.requestId);operation.finish();operation.resolve(receipt);}
        }
        if(event.type==="operation"){
          const record=event.data as JournalRecord,key=JSON.stringify(record.event.operation);
          if(record.event.event==="admitted"){this.activeRemoteOperations.add(key);this.remoteWork??=this.activity.begin();}
          else {this.activeRemoteOperations.delete(key);if(!this.activeRemoteOperations.size){this.remoteWork?.();this.remoteWork=undefined;}this.scheduleRefresh();}
        }
      },
    });
  }
  private initializeAuthoring(server:{server:boolean;preferred:"client"|"server"}|undefined){
    if(this.authoring||this.suppliedAuthoring===null)return;
    const mode=this.options.authoringPreview??server?.preferred??"client";
    if(!this.suppliedAuthoring&&mode==="server"&&!server?.server)throw Error("Server authoring preview is unavailable for this document");
    const authoring=this.suppliedAuthoring??(mode==="server"?createRemoteAuthoringClient((request,signal)=>this.client.authoringPreview(request,signal)):new LocalAuthoringWorker());
    this.authoring=new CollaborationAuthoringController(authoring,{
      paint:preview=>this.presentPrediction(preview),
      changed:()=>{if(this.installed)this.notify(this.projectText());},
      cleared:()=>this.restoreAcceptedFrame(),
      activity:()=>this.activity.begin(),
      error:error=>{this.notice=String(error);this.notify();},
      commit:(model,command,kind)=>this.commitGesture(model,command,kind),
    });
  }
  construct(){
    if(this.opening)return this.opening;
    this.opening=(async()=>{
      const state=await this.client.connect();
      this.initializeAuthoring(state.document.authoringPreview);
      // Start independent native reconstruction from the authenticated model
      // while shared text and the accepted presentation finish loading.
      const connection=this.client.connection;
      if(connection)this.authoring?.replace({...state.document.model,revision:state.document.accepted.modelRevision,documentEpoch:connection.documentEpoch});
      this.latestText=await this.text.open(state.document.textActor,state.document.textCheckpoint);
      this.displayedText=this.latestText.snapshot;
      this.displayedBranches.set(this.displayId,this.displayedText);
      this.serverText=state.document.working.revision;
      await this.refreshScene();await this.receiveText();return this.snapshot();
    })();return this.opening;
  }
  async snapshot(){if(!this.installed)throw Error("Shared workbench has not opened");return this.installed;}
  async toolCatalog():Promise<ToolCatalog>{
    if(!this.catalog)throw Error("Shared tool catalog has not loaded");
    const unavailableReason=this.editingBlockedReason??(!this.authoring?"Authoring is unavailable in this session":undefined);
    return {...this.catalog,geometryRole:{...this.catalog.geometryRole,unavailableReason},sections:this.catalog.sections.map(section=>({...section,commands:section.commands.map(command=>({...command,
      unavailableReason:unavailableReason??command.unavailableReason??(supportsCollaborativeTool(command.toolId)?undefined:"This tool is not available in shared editing yet"),
    }))}))};
  }
  subscribe(listener:(snapshot?:WorkbenchSnapshot)=>void){this.listeners.add(listener);return()=>{this.listeners.delete(listener);};}
  async refresh(){await this.client.refresh();await this.receiveText();return this.refreshScene();}
  async dispatch(input:{version:2;command:string;payload?:unknown}):Promise<WorkbenchSnapshot>{
    if(localCommands.has(input.command))return await this.localUpdate("dispatch",input)??this.snapshot();
    if(input.command==="navigation.rows.select"||input.command==="navigation.source.select")return this.navigate(input.command,input.payload);
    if(["parameter.edit","dimensions.edit","authoring.metadata.set","authoring.parameter.extract","declaration.move","declaration.delete","declaration.suppression.set"].includes(input.command))return this.editStructured(input.command as BrowsingEditCommand,input.payload);
    if(input.command==="tool.select"){
      if(!this.authoring)throw Error("Local authoring is unavailable");
      if(this.editingBlockedReason&&(input.payload as {id:string}).id!=="select")throw Error(this.editingBlockedReason);
      const id=(input.payload as {id:string}).id;
      if(id!=="select"){
        if(!this.local.authoringPointer||!this.view)throw Error("Accepted tool selection is still loading");
        await this.queueLocal(async()=>{const {viewport}=await this.local.authoringPointer!({x:0,y:0});this.authoring!.select(id,{viewport,view:this.view!});});
      }else this.authoring.select(id);
      this.restoreAcceptedFrame();return this.projectText();
    }
    if(input.command==="tool.finish"||input.command==="feature.apply"||input.command==="tool.step_back"){
      if(input.command!=="tool.step_back")this.authoring?.finish();else this.authoring?.stepBack();return this.snapshot();
    }
    if(input.command==="tool.construction.input"||input.command==="tool.operation.input"){
      if(this.editingBlockedReason)throw Error(this.editingBlockedReason);
      if(!this.authoring)throw Error("Authoring is unavailable in this session");
      if(input.command==="tool.construction.input")this.authoring.constructionEvent(input.payload as Parameters<CollaborationAuthoringController["constructionEvent"]>[0]);
      else this.authoring.operationEvent(input.payload as Parameters<CollaborationAuthoringController["operationEvent"]>[0]);
      return this.projectText();
    }
    if(input.command==="geometry.role.toggle"){
      if(this.editingBlockedReason)throw Error(this.editingBlockedReason);
      if(!this.authoring||!this.local.authoringPointer||!this.view)throw Error("Accepted tool selection is still loading");
      await this.queueLocal(async()=>{const {viewport}=await this.local.authoringPointer!({x:0,y:0});this.authoring!.applyOperation("toggle_geometry_role",{viewport,view:this.view!});});
      return this.projectText();
    }
    if(input.command==="geometry.authoring-role.toggle"){this.authoring?.toggleRole();return this.projectText();}
    if(input.command==="source.select"){
      const path=(input.payload as {path:string}).path;
      if(!this.latestText||!Object.hasOwn(this.latestText.snapshot.files,path))throw Error("Unknown shared source file");
      this.selectedPath=path;return this.projectText();
    }
    if(input.command==="history.undo"||input.command==="history.redo"||input.command==="source.prepare"){
      const kind=input.command==="source.prepare"?"apply":input.command==="history.undo"?"undo":"redo";
      await this.textTail;
      const receipt=await this.submit({kind,basisRevision:this.modelRevision,payload:{}});
      if(receipt.outcome?.status==="rejected")throw Error(receipt.outcome.message);
      return this.refresh();
    }
    throw Error(`Shared command is not connected yet: ${input.command}`);
  }
  private async navigate(command:BrowsingNavigationCommand,payload:unknown):Promise<WorkbenchSnapshot>{
    if(!this.browsing||!this.view)throw Error("Local browsing is unavailable");
    const requested=payload as {authority?:unknown}|null;
    if(!requested||typeof requested.authority!=="string"||requested.authority!==this.installed?.navigation?.authority)throw Error("This navigation belongs to an older source revision");
    if(command==="navigation.source.select"){
      if(this.installed?.source.dirty||this.sourceIntents.size)throw Error("Apply the shared draft before navigating from its source");
      const span=payload as {path:string;from:number;to:number};payload={...span,...projectCanonicalSource(span,this.state?.document.sourceProjection)};
    }
    const view=this.view,generation=this.browsingGeneration;await this.browsingReady;
    // The server supplies the first Explorer while the local worker opens. Its
    // native authority must be translated after validating the displayed input.
    const current=await this.browsing.present(view);
    if(generation!==this.browsingGeneration||current.model.revision!==this.modelRevision||JSON.stringify(view.state)!==JSON.stringify(this.view?.state))throw Error("Your selection changed; select the item again");
    const authority=current.chrome.navigation?.authority;
    if(!authority)throw Error("Accepted navigation details are still loading");
    payload={...(payload as Record<string,unknown>),authority};
    const result=await this.browsing.navigate(view,command,payload);
    return this.queueLocal(async()=>{
      if(generation!==this.browsingGeneration||JSON.stringify(view.state)!==JSON.stringify(this.view?.state))throw Error("Your selection changed; select the item again");
      const update=await this.local.update("restoreSelection",{expected:view.state,state:result.state});
      if(update){await this.installLocal(update);this.installChrome(result.chrome);if(this.view&&this.authoring){const projection=await this.local.authoringPointer?.({x:0,y:0});this.authoring.pickSelection(this.view,projection?.viewport);}}
      return this.projectText();
    });
  }
  private async editStructured(command:BrowsingEditCommand,payload:unknown):Promise<WorkbenchSnapshot>{
    if(this.editingBlockedReason)throw Error(this.editingBlockedReason);
    const document=this.state?.document,view=this.view,generation=this.browsingGeneration;
    if(!this.browsing||!document||!view)throw Error("Accepted Inspector details are still loading");
    await this.browsingReady;
    // Initial server chrome and the independently reconstructed local Inspector
    // have different native authorities. Resolve from the local view after its
    // worker is ready; never capture the server authority before this await.
    const current=await this.browsing.present(view);
    if(generation!==this.browsingGeneration||current.model.revision!==document.accepted.modelRevision||JSON.stringify(current.view.state)!==JSON.stringify(this.view?.state))throw Error("The accepted sketch or selection changed; review the current Inspector");
    const authority=current.chrome.authoringDocument?.authority;
    if(!authority)throw Error("Accepted Inspector details are still loading");
    this.installChrome(current.chrome);
    const described=await this.browsing.describe(view,authority,command,payload);
    if(generation!==this.browsingGeneration||described.model.revision!==document.accepted.modelRevision)throw Error("The accepted sketch changed; review the current Inspector");
    if(!described.mutation)return this.snapshot();
    const mutation=described.mutation;
    const target=(declaration:string)=>{
      const object=document.inventory.objects.find(item=>item.declaration===declaration)?.object,next=object&&document.targets[object];
      if(!next)throw Error("Source declaration no longer has a current target");return next;
    };
    let intent:unknown;
    if(mutation.mutation==="set_value"||mutation.mutation==="set_values"){
      const values=mutation.mutation==="set_value"?[mutation]:mutation.values;
      intent={action:"values",writes:values.map(write=>({target:target(write.declaration),declaration:write.declaration,path:write.path,value:write.value}))};
    }else{
      const names=mutation.mutation==="extract_parameter"?[mutation.declaration]:mutation.mutation==="reorder_declaration"?[mutation.declaration,...(mutation.before?[mutation.before]:[])]:
        (mutation.mutation==="set_metadata"||mutation.mutation==="delete"||mutation.mutation==="set_suppressed")&&(mutation.target.target==="declaration"||mutation.target.target==="parameter")?[mutation.target.declaration]:mutation.mutation==="set_suppressed"&&mutation.target.target==="generated"?[mutation.target.address.invocation]:[];
      const targets=mutation.mutation==="set_metadata"&&mutation.target.target==="document"?[document.documentTarget]:[...new Set(names)].map(target);
      if(!targets.length)throw Error("This shared source mutation has no supported explicit declaration target");
      const value:{action:string;mutation:ManagedSketchMutation;targets:SemanticTarget[];deletion?:{roots:SemanticTarget[];closure:SemanticTarget[]}}={action:"mutation",mutation,targets};
      if(mutation.mutation==="delete"){
        const closure=new Set(targets.map(item=>item.object));let changed=true;
        while(changed){changed=false;for(const object of document.inventory.objects)if(!closure.has(object.object)&&object.dependencies.some(dependency=>closure.has(dependency))){closure.add(object.object);changed=true;}}
        value.deletion={roots:targets,closure:[...closure].sort().map(object=>document.targets[object])};
      }
      intent=value;
    }
    const receipt=await this.submit({kind:"semantic",basisRevision:document.accepted.modelRevision,payload:intent});
    if(receipt.outcome?.status==="rejected")throw Error(receipt.outcome.message);
    return this.refresh();
  }
  async submit(command:Command):Promise<Receipt>{
    if(this.editingBlockedReason)throw Error(this.editingBlockedReason);
    const id=collaborationRequestId(),finish=this.activity.begin();
    const terminal=new Promise<Receipt>((resolve,reject)=>this.operations.set(id,{finish,resolve,reject}));
    void terminal.catch(()=>{});
    try{
      const receipt=await this.client.submit(command,id);
      if(receipt.outcome){this.operations.delete(id);finish();return receipt;}
      return await terminal;
    }catch(error){this.operations.delete(id);finish();throw error;}
  }
  /** UI keeps the immediate value; native processing returns exact changes. */
  editSource(path:string,edit:SourceEditorEdit):Promise<void>{
    if(this.disposed)return Promise.reject(Error("This shared editor is closed; saved source work is retained"));
    const revision=++this.draftRevision;
    const displayId=edit.displayId??this.displayId;
    this.sourceIntents.set(revision,{path,edit:structuredClone(edit)});
    try{this.persistSourceIntents();}catch(error){return Promise.reject(error);}
    return this.queueText(async()=>{
      const displayed=this.displayedBranches.get(displayId);
      if(!this.latestText||!displayed)throw Error("Shared source display is unavailable; your edit is retained");
      if(displayed.files[path]!==edit.before)throw Error("Source changed while typing; your edit is retained for reconciliation");
      const updated=await this.text.edit(displayed.revision,[...edit.changes].reverse().map(change=>({kind:"splice" as const,path,start_utf16:change.from,delete_utf16:change.to-change.from,insert:change.insert})));
      if(!updated.localRevision)throw Error("Native typing did not return its displayed branch revision");
      this.displayedText={revision:updated.localRevision,files:{...displayed.files,[path]:edit.after}};
      this.displayedBranches.set(displayId,this.displayedText);
      this.latestText=updated;
      if(updated.changes.length){
        const requestId=collaborationRequestId();this.sourceRequests.set(requestId,revision);
        void this.client.writeText(updated.changes.map(bytes=>Uint8Array.from(bytes)),requestId).catch(error=>{this.notice=String(error);this.notify();});
      }else {this.sourceIntents.delete(revision);this.persistSourceIntents();}
      if(revision===this.draftRevision)this.notify(this.projectText());
    });
  }
  async undoText(redo=false):Promise<void>{
    await this.textTail;
    await this.client.undoText(redo);
    await this.receiveText();
  }
  pointer(input:PointerSample){
    return this.queueLocal(async()=>{
      if(!this.installed)return null;
      if(input.phase==="down"&&input.buttons===4)this.panning=input.pointerId;
      const navigating=this.panning===input.pointerId;
      const drawing=this.authoring&&this.authoring.tool!=="select"&&!navigating;
      const update=drawing?null:await this.local.update("pointer",input);
      if(update)await this.installLocal(update);
      if(navigating&&input.phase==="up")this.panning=undefined;
      if(this.local.authoringPointer&&this.view){
        const projection=await this.local.authoringPointer({x:input.x,y:input.y,captured:input.phase!=="down"});
        this.cursor=projection.position;this.schedulePresence();
        if(!navigating&&!this.editingBlockedReason)this.authoring?.pointer(input,projection,this.view);
      }
      return update?this.installed:null;
    });
  }
  wheel(input:WheelSample){return this.localUpdate("wheel",input);}
  wheelBatch(samples:WheelSample[]){return this.localUpdate("wheel",{version:2,samples});}
  resize(input:{version:2;width:number;height:number;pixelRatio:number}){return this.localUpdate("resize",input);}
  cancel(input:{version:2;reason:"escape"|"lost-capture"|"blur"}){this.authoring?.cancel();this.restoreAcceptedFrame();return this.localUpdate("cancel",input);}
  async managedCompilerContext(){return {version:2 as const,patches:{}};}
  async exportProject(){if(!this.state)throw Error("No accepted project");return {version:2 as const,filename:"sketch.geosolve.json",contents:this.state.document.model.project};}
  async persistProject():Promise<{version:2;contents:string}>{throw Error("Shared edits are persisted by the document server");}
  async exportReproduction(){return {version:2 as const,filename:"shared-document.json",contents:JSON.stringify(this.state?.document)};}
  async exportInteractionTrace(){return {version:2 as const,filename:"local-view.json",contents:JSON.stringify(await this.local.state())};}
  dispose(){
    if(this.disposed)return;this.disposed=true;this.client.dispose();this.local.dispose();this.text.dispose();this.authoring?.dispose();this.browsing?.dispose();this.activity.reset();this.notify();clearTimeout(this.presenceTimer);clearTimeout(this.presencePaintTimer);
    for(const operation of this.operations.values())operation.reject(Error("Shared workbench closed; pending requests retained"));this.operations.clear();this.listeners.clear();
  }
  private persistSourceIntents(){this.saveSourceIntents?.([...this.sourceIntents].filter(([id])=>!this.durableSourceIntents.has(id)).map(([,value])=>value));}
  private notify(snapshot?:WorkbenchSnapshot){for(const listener of this.listeners)listener(snapshot);}
  private projectText():WorkbenchSnapshot{
    if(!this.installed||!this.latestText)throw Error("Shared workbench has not opened");
    if(!this.sourceIntents.size){
      if(JSON.stringify(this.displayedText?.revision)!==JSON.stringify(this.latestText.snapshot.revision)){
        this.displayId++;this.displayedBranches.set(this.displayId,this.latestText.snapshot);
        // Old installed frames may still have a React event queued. Retain a
        // bounded display horizon; never reinterpret an expired frame's offsets.
        while(this.displayedBranches.size>128)this.displayedBranches.delete(this.displayedBranches.keys().next().value!);
      }
      this.displayedText=this.latestText.snapshot;
    }
    const visible=this.displayedText??this.latestText.snapshot;
    const files=Object.entries(visible.files).map(([path,contents])=>({path,contents,sharedRevision:this.displayId,language:(path.endsWith(".json")?"json":/\.[cm]?[jt]s$/u.test(path)?"typescript":"text") as "json"|"typescript"|"text",readOnly:!!this.editingBlockedReason}));
    const selectedPath=files.find(file=>file.path===(this.selectedPath??this.installed!.source.selectedPath))?.path??files[0]?.path??"";
    const dirty=JSON.stringify(visible.files)!==JSON.stringify(this.state?.document.accepted.files);
    const history=this.state?.document.semanticHistory;
    this.installed=stampCanvasSnapshot({...this.installed,source:{files,selectedPath,dirty},authoringContext:this.authoring?{construction:this.authoring.construction,operation:this.authoring.operation}:undefined,presentation:{...this.installed.presentation,
      canUndo:history?.canUndo??false,canRedo:history?.canRedo??false,
      activeTool:this.authoring?.tool??"select",canFinish:this.authoring?.canFinish??false,geometryRole:this.authoring?.role??"profile",
    }},++this.sequence);return this.installed;
  }
  private async refreshScene():Promise<WorkbenchSnapshot>{
    let scene=await this.client.scene<CollaborativeScene>();
    for(let attempt=0;this.state?.document.accepted.modelRevision!==scene.revision&&attempt<3;attempt++){
      await this.client.refresh();scene=await this.client.scene<CollaborativeScene>();
    }
    if(this.state?.document.accepted.modelRevision!==scene.revision)throw Error("The shared sketch is changing; waiting for a matching model and scene");
    if(scene.revision<this.modelRevision)return this.snapshot();
    if(scene.revision===this.modelRevision)return this.projectText();
    return this.queueLocal(async()=>{
      if(scene.revision<this.modelRevision)return this.snapshot();
      if(scene.revision===this.modelRevision)return this.projectText();
      const snapshot=assertWorkbenchSnapshot(scene.value.snapshot);
      this.catalog=assertToolCatalog(scene.value.toolCatalog);
      const local=this.installed?await this.local.replace(scene.value.seed,true):await this.local.construct(scene.value.seed);
      this.prediction=undefined;this.modelRevision=scene.revision;this.seed=scene.value.seed;this.view={seed:this.seed,state:local.state};this.acceptedFrame=local.frame;this.schedulePresencePaint();
      this.installed={...snapshot,...projectSourceNavigation(snapshot.navigation,snapshot.explorer,this.state?.document.sourceProjection),
        selection:snapshot.selection&&{...snapshot.selection,source:snapshot.selection.source&&projectAuthoredSource(snapshot.selection.source,this.state?.document.sourceProjection)},frame:local.frame};
      const document=this.state?.document,connection=this.client.connection;
      if(document&&connection&&document.accepted.modelRevision===scene.revision){
        const model={...document.model,revision:scene.revision,documentEpoch:connection.documentEpoch};this.authoring?.replace(model);
        if(this.browsing){
          this.browsingGeneration++;
          this.browsingReady=this.browsing.replace(model,this.seed);void this.browsingReady.catch(()=>{});this.scheduleBrowsing();
        }
      }
      return this.projectText();
    });
  }
  private queueLocal<T>(run:()=>Promise<T>):Promise<T>{const result=this.localTail.then(run,run);this.localTail=result.catch(()=>{});return result;}
  private queueText<T>(run:()=>Promise<T>):Promise<T>{const result=this.textTail.then(run,run);this.textTail=result;void result.catch(()=>{});return result;}
  private localUpdate(method:"dispatch"|"pointer"|"wheel"|"resize"|"cancel",input:unknown):Promise<WorkbenchSnapshot|null>{
    return this.queueLocal(async()=>{
      if(!this.installed)return null;
      const update:LocalInteractionUpdate|null=await this.local.update(method,input);if(!update)return null;
      await this.installLocal(update);return this.installed;
    });
  }
  private async installLocal(update:LocalInteractionUpdate){
    this.acceptedFrame=update.frame;
    const previous=JSON.stringify(this.view?.state);
    if(this.seed)this.view={seed:this.seed,state:update.state};
    const changed=JSON.stringify(update.state)!==previous,prediction=this.prediction;
    if(changed&&this.view&&prediction?.presentation&&this.local.projectPrediction){
      const construction=prediction.construction?{preview:prediction.construction.preview,inference_guides:prediction.construction.inference_guides}:undefined;
      const projected=await this.local.projectPrediction({presentation:prediction.presentation,view:this.view,construction});
      if(this.prediction===prediction)this.prediction={...prediction,view:this.view,frame:projected.frame};
    }
    this.installed=stampCanvasSnapshot(markCanvasOnlySnapshot({...this.installed!,frame:this.prediction?this.predictedFrame():update.frame}),++this.sequence);
    if(changed){this.scheduleBrowsing();if(this.view&&(!prediction?.presentation||!this.local.projectPrediction))this.authoring?.render(this.view);}
  }
  private scheduleBrowsing(){
    if(!this.browsing||!this.view)return;
    if(this.browsingScheduled){this.browsingAgain=true;return;}this.browsingScheduled=true;
    void(async()=>{
      do{
        this.browsingAgain=false;
        const generation=this.browsingGeneration;
        let result;
        try{await this.browsingReady;const view=this.view!;result=await this.browsing!.present(view);}
        catch(error){if(generation!==this.browsingGeneration||this.disposed)continue;throw error;}
        if(!this.installed||this.disposed||generation!==this.browsingGeneration||result.model.revision!==this.modelRevision||JSON.stringify(result.view.state)!==JSON.stringify(this.view?.state))continue;
        this.installChrome(result.chrome);
        this.notify(this.projectText());
      }while(this.browsingAgain&&!this.disposed);
    })().catch(error=>{if(!this.disposed){this.notice=String(error);this.notify();}}).finally(()=>{this.browsingScheduled=false;if(this.browsingAgain&&!this.disposed)this.scheduleBrowsing();});
  }
  private installChrome(value:BrowsingChrome){
    const {authoringDocument,selection,selectedGeometryRole,constructionVisible,visibilityRestoreAvailable,...chrome}=value;
    const selected=new Set(value.navigation?.rows.filter(row=>row.state==="selected").map(row=>row.id));
    const declarations:string[]=[];
    const collect=(rows:typeof value.explorer)=>{for(const row of rows){if(selected.has(row.id)&&row.source){const projection=this.state?.document.sourceProjection;for(const span of projection?.spans??[])if(row.source.from<span.canonical.to&&row.source.to>span.canonical.from){const object=this.state?.document.inventory.objects.find(item=>item.declaration===span.declaration)?.object;if(object)declarations.push(object);}}collect(row.children);}};
    collect(value.explorer);this.ownSelection=[...new Set(declarations)];this.schedulePresence();
    const projected=projectSourceNavigation(chrome.navigation,chrome.explorer,this.state?.document.sourceProjection);
    this.installed=stampCanvasSnapshot({...this.installed!,...chrome,...projected,authoringDocument:authoringDocument??undefined,selection:selection?{...selection,source:selection.source&&projectAuthoredSource(selection.source,this.state?.document.sourceProjection)}:undefined,
      presentation:{...this.installed!.presentation,selectedGeometryRole:selectedGeometryRole??undefined,constructionVisible:constructionVisible??this.installed!.presentation.constructionVisible,visibilityRestoreAvailable:visibilityRestoreAvailable??this.installed!.presentation.visibilityRestoreAvailable}},++this.sequence);
  }
  private restoreAcceptedFrame(){this.prediction=undefined;if(this.installed&&this.acceptedFrame)this.installed=stampCanvasSnapshot(markCanvasOnlySnapshot({...this.installed,frame:this.acceptedFrame}),++this.sequence);}
  private presentPrediction(preview:AuthoringPreview){
    if(!this.installed||preview.model.revision!==this.modelRevision||preview.model.documentEpoch!==this.client.connection?.documentEpoch
      ||preview.model.sourceDesignDigest!==this.state?.document.model.sourceDesignDigest
      ||JSON.stringify(preview.view.state)!==JSON.stringify(this.view?.state))return;
    this.prediction=preview;
    this.installed=stampCanvasSnapshot(markCanvasOnlySnapshot({...this.installed,frame:this.predictedFrame()}),++this.sequence);this.notify(this.installed);
  }
  private predictedFrame():WorkbenchSnapshot["frame"]{
    const frame=this.prediction!.frame;
    // Presence is independent from document geometry. Accepted picking still
    // owns every hit test, but cannot flash its old coordinates over prediction.
    return {...frame,scene:{...frame.scene,items:[...frame.scene.items.filter(item=>item.layer!=="presence"),...(this.acceptedFrame?.scene.items.filter(item=>item.layer==="presence")??[])]}};
  }
  private async commitGesture(model:AuthoringModel,gesture:AuthoringCommand,kind:"point"|"construction"|"operation"){
    if(model.revision!==this.modelRevision||model.documentEpoch!==this.client.connection?.documentEpoch)throw Error("The accepted sketch changed during drawing; draw again on the current sketch");
    let payload:unknown={action:kind==="operation"?"tool_operation":"construction",gesture};
    if(kind==="point"){
      const target=(gesture as PointGestureCommand).target,addresses=target.target==="point"?[target.address]:[target.lower_left,target.upper_right];
      const names=[...new Set(addresses.map(address=>address.owner.address.owner==="direct_declaration"?address.owner.address.declaration:address.owner.address.address.invocation))];
      const targets=names.map(declaration=>{
        const object=this.state?.document.inventory.objects.find(item=>item.declaration===declaration)?.object;
        const target=object&&this.state?.document.targets[object];if(!target)throw Error("Drawing target is no longer available");return target;
      });
      payload={action:"point_gesture",targets,gesture};
    }
    const receipt=await this.submit({kind:"semantic",basisRevision:model.revision,payload});
    if(receipt.outcome?.status==="rejected")throw Error(receipt.outcome.message);
    this.notify(await this.refresh());
  }
  private receivePresence(value:unknown){
    const peer=value as PeerPresence;
    if(!peer||typeof peer.userId!=="string"||typeof peer.clientId!=="string"||!Number.isSafeInteger(peer.sequence)||peer.sequence<0
      ||!Array.isArray(peer.selection)||peer.selection.length>256||peer.selection.some(id=>typeof id!=="string")
      ||peer.cursor!==null&&(!Array.isArray(peer.cursor)||peer.cursor.length!==2||!peer.cursor.every(Number.isFinite)))return;
    const self=this.client?.connection;if(self?.userId===peer.userId&&self.clientId===peer.clientId)return;
    const key=JSON.stringify([peer.userId,peer.clientId]),old=this.peers.get(key);
    if(old&&old.sequence>=peer.sequence)return;if(!old&&this.peers.size>=128)return;
    this.peers.set(key,peer);this.schedulePresencePaint();
  }
  private prunePresence(){
    const live=new Set(this.participants.map(peer=>JSON.stringify([peer.userId,peer.clientId])));
    for(const key of this.peers.keys())if(!live.has(key))this.peers.delete(key);this.schedulePresencePaint();
  }
  private schedulePresencePaint(){
    if(this.presencePaintTimer||!this.seed||this.disposed)return;
    this.presencePaintTimer=setTimeout(()=>{this.presencePaintTimer=undefined;
      void this.queueLocal(async()=>{
        if(!this.seed||this.disposed)return;
        const declarations=new Map(this.state?.document.inventory.objects.map(item=>[item.object,item.declaration]));
        const presence=[...this.peers.values()].map(peer=>({...peer,clientId:JSON.stringify([peer.userId,peer.clientId]),selection:peer.selection.flatMap(object=>{const declaration=declarations.get(object);return declaration?[declaration]:[];})}));
        const update=await this.local.update("presence",{sceneKey:this.seed.sceneKey,presence});
        if(update){await this.installLocal(update);this.notify(this.installed);}
      }).catch(()=>{});
    },100);
  }
  private schedulePresence(){
    if(this.disposed)return;
    if(this.presenceSending){this.presenceAgain=true;return;}
    if(this.presenceTimer)return;
    this.presenceTimer=setTimeout(()=>{this.presenceTimer=undefined;if(!this.client.connected)return;this.presenceSending=true;
      void this.client.presence({sequence:++this.presenceSequence,cursor:this.cursor,selection:this.ownSelection}).catch(()=>{}).finally(()=>{
        this.presenceSending=false;if(this.presenceAgain){this.presenceAgain=false;this.schedulePresence();}
      });
    },100);
  }
  private async receiveText(){
    if(!this.serverText)return;
    const delta=await this.client.textDelta(this.serverText);
    await this.queueText(async()=>{
      this.latestText=await this.text.receive(delta.changes);this.serverText=delta.workingRevision;
      if(this.state&&delta.history)this.state={...this.state,document:{...this.state.document,textHistory:delta.history}};
      if(this.installed)this.notify(this.projectText());
    });
  }
  private scheduleTextRefresh(){
    if(!this.latestText)return;
    if(this.textPending){this.textAgain=true;return;}this.textPending=true;
    void(async()=>{do{this.textAgain=false;await this.receiveText();}while(this.textAgain&&!this.disposed);})()
      .catch(error=>{this.notice=String(error);this.notify();}).finally(()=>{this.textPending=false;});
  }
  private scheduleRefresh(){
    if(!this.installed)return;
    if(this.refreshPending){this.refreshAgain=true;return;}this.refreshPending=true;
    const run=this.refreshTail.then(async()=>{do{this.refreshAgain=false;const next=await this.refresh();this.notify(next);}while(this.refreshAgain&&!this.disposed);});
    this.refreshTail=run.catch(error=>{this.notice=String(error);this.notify();}).finally(()=>{this.refreshPending=false;});
  }
}
