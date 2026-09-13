// SPDX-License-Identifier: GPL-3.0-or-later
// Test-only local native client. Production folder servers do not load demo WASM.
import { readFileSync } from "node:fs";
import { engineModuleUrl } from "../packages/geosolve-cli/runtime/workspace-runtime-paths.mjs";

let browser;
export async function nativeBrowsing(folderModel) {
  if (!browser) browser = (async () => {
    const module = await import("../crates/geosolve-demo-web/frontend/src/generated/geosolve_demo_web.js");
    await module.default({ module_or_path: readFileSync(new URL("../crates/geosolve-demo-web/frontend/src/generated/geosolve_demo_web_bg.wasm", import.meta.url)) });
    return module;
  })();
  const module = await browser;
  const handle = new module.BrowsingHandle(JSON.stringify({ ...(folderModel.model ? { project: folderModel.model.project, design: folderModel.model.design } : { generated: JSON.stringify(folderModel.generated) }), seed: folderModel.seed }));
  const initial = JSON.parse(handle.initialize());
  const interaction = new module.InteractionHandle(JSON.stringify(initial.seed));
  return { handle, interaction, initial, dispose() { interaction.free(); handle.free(); } };
}
export async function describedMutation(folderModel, command, payload) {
  const local = await nativeBrowsing(folderModel);
  try {
    const state = JSON.parse(local.interaction.state());
    const chrome = JSON.parse(local.handle.update(JSON.stringify(state)));
    const resolved = typeof payload === "function" ? payload(chrome) : payload;
    return JSON.parse(local.handle.describe(JSON.stringify({ state, authority: chrome.authoringDocument.authority, command, payload: resolved })));
  } finally { local.dispose(); }
}
export async function nativeSession(folderModel) {
  const { createEngine } = await import(engineModuleUrl);
  const engine = await createEngine();
  try {
    const session = engine.openEditableSession(folderModel.model.project, { design: folderModel.model.design });
    return { engine, session, viewport: JSON.parse(folderModel.seed.scene).viewport, dispose() { session.dispose(); engine.dispose(); } };
  } catch (error) { engine.dispose(); throw error; }
}
export async function pointCommand(folderModel, delta = [4, -2], choose = targets => targets[0]) {
  const local = await nativeSession(folderModel);
  try {
    const target = choose(local.session.pointGestureTargets());
    if (!target) throw Error("Fixture has no matching native point target");
    const gesture = local.session.beginPointGesture(target.target, { expected: local.session.token, gestureId: 1, viewport: local.viewport });
    const result = gesture.advance({ sequence: 1, position: target.position.map((value, index) => value + delta[index]) });
    if (!result.accepted) throw Error("Native test point prediction was rejected");
    return gesture.finish().command;
  } finally { local.dispose(); }
}
export async function polylineCommand(folderModel, positions, onSample = () => {}) {
  const local = await nativeSession(folderModel);
  try {
    const prediction = local.session.beginConstruction("polyline", { expected: local.session.token, gestureId: 1, viewport: local.viewport, role: "profile" });
    let sequence = 0;
    for (const position of positions) {
      const frame = prediction.advance({ sequence: ++sequence, input: { event: "click", position, suppressed: true, regularized: false } });
      if (frame.diagnostic) throw Error(frame.diagnostic);
      await onSample();
    }
    const frame = prediction.advance({ sequence: ++sequence, input: { event: "complete" } });
    if (!frame.completed) throw Error(frame.diagnostic || "Native test polyline is incomplete");
    return prediction.finish();
  } finally { local.dispose(); }
}
export async function roleCommand(folderModel) {
  const local = await nativeSession(folderModel);
  try {
    const { curve } = local.session.accepted.geometry.curves.find(({ curve }) => curve.definition.kind === "circle");
    const span = { curve: curve.id, segment: 0 };
    const selection = local.session.toolOperationOperands({ items: [{ Curve: span }], curve_picks: [{ span, parameter: 0, origin: null }] });
    const prediction = local.session.beginToolOperation("toggle_geometry_role", { expected: local.session.token, gestureId: 1, viewport: local.viewport, selection });
    if (!prediction.initialFrame.completed) throw Error("Fixture role operation did not complete");
    return prediction.finish();
  } finally { local.dispose(); }
}
