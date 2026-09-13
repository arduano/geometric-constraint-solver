// SPDX-License-Identifier: GPL-3.0-or-later
// One disposable worker retains one exact native authoring continuation. It has
// no route to commit, source mutation, filesystem writes, or navigation state.
import { parentPort, workerData } from "node:worker_threads";
import { engineModuleUrl } from "./workspace-runtime-paths.mjs";
const { createEngine } = await import(engineModuleUrl);
const input = JSON.parse(workerData.encoded);
const canonical = value => JSON.stringify(value, (_key, child) => child && typeof child === "object" && !Array.isArray(child)
  ? Object.fromEntries(Object.keys(child).sort().map(key => [key, child[key]])) : child);
const engine = await createEngine();
const session = engine.openEditableSession(input.model.project, { design: input.model.design });
if (session.sourceDesignDigest() !== input.model.sourceDesignDigest
  || canonical(JSON.parse(session.exportProject())) !== canonical(typeof input.model.project === "string" ? JSON.parse(input.model.project) : input.model.project)
  || canonical(session.exportDesign()) !== canonical(input.model.design)) throw Error("Preview reconstruction differs from trusted accepted source/design");
let prediction, kind, point, construction, operation, next = 1;
const basis = input.basis;
const preview = () => ({ kind: "preview", basis, presentation: prediction.presentationJSON(), ...(point ? { point } : {}), ...(construction ? { construction } : {}), ...(operation ? { operation } : {}) });
let viewport;
async function mappedOperands(view) {
  if (!input.viewSeed) throw Error("Accepted authoring selection scene is unavailable");
  return session.toolOperationViewOperands(viewport, { seed: input.viewSeed, state: view.state });
}
async function run(request) {
  if (request.action === "begin") {
    if (prediction) throw Error("Preview already started");
    const options = { expected: session.token, gestureId: request.gestureId, viewport: request.viewport };
    viewport = request.viewport;
    kind = request.kind;
    let selection = request.selection;
    if (kind === "operation" && selection === undefined) {
      selection = await mappedOperands(request.view);
    }
    prediction = kind === "point" ? session.beginPointGesture(request.target, options)
      : kind === "operation" ? session.beginToolOperation(request.tool, { ...options, selection, options: request.options })
      : session.beginConstruction(request.tool, { ...options, role: request.role ?? "profile" });
    if (kind === "operation") operation = prediction.initialFrame;
    if (kind === "construction") construction = prediction.initialFrame;
    return preview();
  }
  if (!prediction) throw Error("Preview is finished or cancelled");
  if (request.action === "pick_selection") {
    if (kind !== "operation") throw Error("Selection input requires an active native operation");
    operation = prediction.advance({ sequence: request.sequence, input: { event: "pick_selection", operands: await mappedOperands(request.view) } });
    return preview();
  }
  if (request.action === "advance") {
    for (const sample of request.samples) {
      const frame = prediction.advance(sample);
      if (sample.input?.event === "viewport") viewport = sample.input.viewport;
      if (kind === "point") point = frame; else if (kind === "operation") operation = frame; else construction = frame;
    }
    return preview();
  }
  if (request.action === "finish") {
    const value = prediction.finish(); prediction = undefined;
    return kind === "point" ? { kind: "point", basis, terminal: value } : { kind, basis, command: value };
  }
  if (request.action === "cancel") { prediction.cancel(); prediction = undefined; return { kind: "cancelled", basis }; }
  throw Error("Unsupported preview operation");
}
let tail = Promise.resolve();
parentPort.on("message", message => {
  const execute = async () => {
  try {
    if (!Number.isSafeInteger(message?.id) || message.id !== next++) throw Error("Unordered preview worker request");
    const result = await run(message.request);
    const encoded = JSON.stringify(result);
    if (Buffer.byteLength(encoded) > workerData.maxResponseBytes) throw Error("Preview response exceeds configured byte limit");
    parentPort.postMessage({ id: message.id, ok: true, encoded });
  } catch (error) {
    try { prediction?.cancel(); } catch {} prediction = undefined;
    parentPort.postMessage({ id: message?.id, ok: false, error: String(error).slice(0, 8192) });
  }
  };
  tail = tail.then(execute, execute);
});
