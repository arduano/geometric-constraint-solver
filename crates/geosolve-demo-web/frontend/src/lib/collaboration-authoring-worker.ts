// SPDX-License-Identifier: GPL-3.0-or-later
import initializeEngine, * as engineWasm from "../../../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm.js";
import engineWasmUrl from "../../../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm_bg.wasm?url";
import initializePresentation, { renderAuthoringPreview } from "../generated/geosolve_demo_web.js";
import { createEngine, type Engine, type EditableSession, type EditableDesign, type RetainedPointGesture, type ConstructionPrediction, type PointGestureTarget, type PointGestureViewport, type PointGestureSample, type PointGestureFrame, type PointGestureTerminal, type ConstructionTool, type ConstructionSample, type ConstructionFrame, type ConstructionCommand } from "../../../../../packages/geosolve-engine/src/index";
import type { WorkbenchSnapshot } from "./adapter";
import type { InteractionSeed, InteractionState } from "./local-interaction-worker";

export interface AuthoringModelIdentity { readonly documentEpoch: string; readonly revision: number; readonly sourceDesignDigest: string }
export interface AuthoringModel extends AuthoringModelIdentity { readonly project: string; readonly design: EditableDesign }
/** The exact local view is a compatibility witness, not a server editing context. */
export interface AuthoringView { readonly seed: InteractionSeed; readonly state: InteractionState }
interface GestureStart { readonly gestureId: number; readonly viewport: PointGestureViewport; readonly view: AuthoringView }
export type AuthoringAction =
  | { method: "replace"; model: AuthoringModel }
  | ({ method: "beginPoint"; target: PointGestureTarget } & GestureStart)
  | { method: "advancePoint"; samples: readonly PointGestureSample[]; view: AuthoringView }
  | { method: "finishPoint" }
  | ({ method: "beginConstruction"; tool: ConstructionTool; role: "profile" | "construction" } & GestureStart)
  | { method: "advanceConstruction"; samples: readonly ConstructionSample[]; view: AuthoringView }
  | { method: "finishConstruction" }
  | { method: "render"; view: AuthoringView }
  | { method: "cancel" };
export type AuthoringWorkerRequest = AuthoringAction & { id: number; generation: number };
export interface AuthoringPreview {
  readonly kind: "preview";
  readonly model: AuthoringModelIdentity;
  readonly view: AuthoringView;
  readonly frame: WorkbenchSnapshot["frame"];
  /** Opaque native candidate geometry for solve-free local reprojection. */
  readonly presentation?: string;
  readonly point?: PointGestureFrame;
  readonly construction?: ConstructionFrame;
}
export type AuthoringResult =
  | { readonly kind: "ready"; readonly model: AuthoringModelIdentity }
  | AuthoringPreview
  | { readonly kind: "point"; readonly model: AuthoringModelIdentity; readonly terminal: PointGestureTerminal }
  | { readonly kind: "construction"; readonly model: AuthoringModelIdentity; readonly command: ConstructionCommand }
  | { readonly kind: "cancelled"; readonly model: AuthoringModelIdentity };
export type AuthoringWorkerResponse = { id: number; generation: number; result: AuthoringResult } | { id: number; generation: number; error: string };
export interface AuthoringRuntime {
  open(model: AuthoringModel): Pick<EditableSession, "token" | "sourceDesignDigest" | "exportDesign" | "exportProject" | "beginPointGesture" | "beginConstruction" | "dispose" | "accepted">;
  render(presentation: string, view: AuthoringView, construction?: Pick<ConstructionFrame, "preview" | "inference_guides">): WorkbenchSnapshot["frame"];
  release(session: ReturnType<AuthoringRuntime["open"]>): void;
}
type Session = ReturnType<AuthoringRuntime["open"]>;
const canonical = (value: unknown): string => JSON.stringify(value, (_key, child: unknown) => {
  if (child && typeof child === "object" && !Array.isArray(child)) return Object.fromEntries(Object.entries(child).sort(([a], [b]) => a.localeCompare(b)));
  return child;
});

/** Serial native authoring queue, isolated from navigation and shared-text workers.
 * Every admitted gesture sample is replayed; only intermediate painting is skipped.
 * Neither this protocol nor its replies expose a native publication method/ticket.
 */
export function createAuthoringWorkerHandler(runtime: Promise<AuthoringRuntime>, reply: (response: AuthoringWorkerResponse) => void) {
  let native: AuthoringRuntime | undefined, session: Session | undefined, model: AuthoringModelIdentity | undefined;
  let generation = 0, point: RetainedPointGesture | undefined, construction: ConstructionPrediction | undefined;
  let pointFrame: PointGestureFrame | undefined, constructionFrame: ConstructionFrame | undefined;
  let retainedPresentation: string | undefined;
  let tail = Promise.resolve();
  void runtime.catch(() => undefined);
  const cancel = () => {
    retainedPresentation = undefined;
    try { point?.cancel(); } finally { point = undefined; pointFrame = undefined; }
    try { construction?.cancel(); } finally { construction = undefined; constructionFrame = undefined; }
  };
  const paint = (view: AuthoringView): AuthoringPreview => {
    const scene = point?.presentationJSON() ?? construction?.presentationJSON() ?? retainedPresentation;
    if (!scene || !native || !model) throw Error("No active authoring prediction");
    return { kind: "preview", model, view, frame: native.render(scene, view, constructionFrame), presentation: scene, point: pointFrame, construction: constructionFrame };
  };
  return ({ data }: MessageEvent<AuthoringWorkerRequest>) => {
    const run = async () => {
      try {
        if (!Number.isSafeInteger(data?.id) || data.id < 1 || !Number.isSafeInteger(data.generation) || data.generation < 1) throw Error("Invalid authoring worker request");
        let result: AuthoringResult;
        if (data.method === "replace") {
          if (data.generation <= generation) throw Error("Obsolete authoring model replacement");
          if (!data.model.documentEpoch || !Number.isSafeInteger(data.model.revision) || data.model.revision < 0 || !data.model.sourceDesignDigest) throw Error("Invalid accepted authoring identity");
          native = await runtime;
          cancel();
          const next = native.open(data.model);
          try {
            if (next.sourceDesignDigest() !== data.model.sourceDesignDigest || canonical(JSON.parse(next.exportProject())) !== canonical(JSON.parse(data.model.project)) || canonical(next.exportDesign()) !== canonical(data.model.design)) throw Error("Authoring reconstruction disagrees with the accepted source/design");
          } catch (error) { native.release(next); throw error; }
          if (session) native.release(session);
          session = next; generation = data.generation;
          model = { documentEpoch: data.model.documentEpoch, revision: data.model.revision, sourceDesignDigest: data.model.sourceDesignDigest };
          result = { kind: "ready", model };
        } else {
          if (!session || !model || data.generation !== generation) throw Error("Authoring request belongs to an obsolete accepted model");
          if (data.method === "beginPoint" || data.method === "beginConstruction") {
            if (point || construction) throw Error("Finish or cancel the active authoring gesture");
            retainedPresentation = undefined;
            const options = { expected: session.token, gestureId: data.gestureId, viewport: data.viewport };
            if (data.method === "beginPoint") point = session.beginPointGesture(data.target, options);
            else construction = session.beginConstruction(data.tool, { ...options, role: data.role });
            result = paint(data.view);
          } else if (data.method === "advancePoint" || data.method === "advanceConstruction") {
            if (!Array.isArray(data.samples) || data.samples.length < 1 || data.samples.length > 256) throw Error("Authoring batch must contain 1–256 ordered samples");
            if (data.method === "advancePoint") {
              if (!point) throw Error("No active point gesture");
              try { for (const sample of data.samples) pointFrame = point.advance(sample); }
              catch (error) { cancel(); throw error; }
            } else {
              if (!construction) throw Error("No active construction gesture");
              try { for (const sample of data.samples) constructionFrame = construction.advance(sample); }
              catch (error) { cancel(); throw error; }
            }
            result = paint(data.view);
          } else if (data.method === "finishPoint") {
            if (!point) throw Error("No active point gesture");
            try { const presentation = point.presentationJSON();result = { kind: "point", model, terminal: point.finish() };retainedPresentation = presentation; }
            finally { point = undefined; pointFrame = undefined; }
          } else if (data.method === "finishConstruction") {
            if (!construction) throw Error("No active construction gesture");
            try { const presentation = construction.presentationJSON();result = { kind: "construction", model, command: construction.finish() };retainedPresentation = presentation; }
            finally { construction = undefined; constructionFrame = undefined; }
          } else if (data.method === "render") result = paint(data.view);
          else if (data.method === "cancel") { cancel(); result = { kind: "cancelled", model }; }
          else throw Error("Unsupported authoring operation");
        }
        reply({ id: data.id, generation: data.generation, result });
      } catch (error) { reply({ id: data?.id, generation: data?.generation, error: error instanceof Error ? error.message : String(error) }); }
    };
    tail = tail.then(run, run);
  };
}

async function createBrowserRuntime(): Promise<AuthoringRuntime> {
  const [engine] = await Promise.all([
    createEngine({ wasmModule: { ...engineWasm, default: initializeEngine }, wasm: new URL(engineWasmUrl, import.meta.url) }),
    initializePresentation(),
  ]);
  return {
    open: model => engine.openEditableSession(model.project, { design: model.design }),
    release: session => releaseSession(engine, session),
    render: (presentation, view, construction) => JSON.parse(renderAuthoringPreview(JSON.stringify({ ...JSON.parse(presentation), view, construction: construction ? { preview: construction.preview, inference_guides: construction.inference_guides } : null }))),
  };
}
function releaseSession(engine: Engine, session: Session) {
  const result = session.accepted;
  session.dispose(); engine.release(result);
}
if (typeof document === "undefined" && typeof self !== "undefined") {
  self.onmessage = createAuthoringWorkerHandler(createBrowserRuntime(), response => self.postMessage(response));
}
