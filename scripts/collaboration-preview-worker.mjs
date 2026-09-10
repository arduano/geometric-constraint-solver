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
let prediction, kind, point, construction, next = 1;
const basis = input.basis;
const preview = () => ({ kind: "preview", basis, presentation: prediction.presentationJSON(), ...(point ? { point } : {}), ...(construction ? { construction } : {}) });
function run(request) {
  if (request.action === "begin") {
    if (prediction) throw Error("Preview already started");
    const options = { expected: session.token, gestureId: request.gestureId, viewport: request.viewport };
    kind = request.kind;
    prediction = kind === "point" ? session.beginPointGesture(request.target, options)
      : session.beginConstruction(request.tool, { ...options, role: request.role ?? "profile" });
    return preview();
  }
  if (!prediction) throw Error("Preview is finished or cancelled");
  if (request.action === "advance") {
    for (const sample of request.samples) {
      const frame = prediction.advance(sample);
      if (kind === "point") point = frame; else construction = frame;
    }
    return preview();
  }
  if (request.action === "finish") {
    const value = prediction.finish(); prediction = undefined;
    return kind === "point" ? { kind: "point", basis, terminal: value } : { kind: "construction", basis, command: value };
  }
  if (request.action === "cancel") { prediction.cancel(); prediction = undefined; return { kind: "cancelled", basis }; }
  throw Error("Unsupported preview operation");
}
parentPort.on("message", message => {
  try {
    if (!Number.isSafeInteger(message?.id) || message.id !== next++) throw Error("Unordered preview worker request");
    const result = run(message.request);
    const encoded = JSON.stringify(result);
    if (Buffer.byteLength(encoded) > workerData.maxResponseBytes) throw Error("Preview response exceeds configured byte limit");
    parentPort.postMessage({ id: message.id, ok: true, encoded });
  } catch (error) {
    try { prediction?.cancel(); } catch {} prediction = undefined;
    parentPort.postMessage({ id: message?.id, ok: false, error: String(error).slice(0, 8192) });
  }
});
