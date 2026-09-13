// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { pointCommand } from "./workspace-native-test.mjs";
import { serveProject, initProject } from "../packages/geosolve-cli/runtime/file-workspace.mjs";

async function setup(t, { entry = "model/design.ts" } = {}) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-project-"));
  initProject(folder);
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  mkdirSync(resolve(folder, "model"));
  writeFileSync(resolve(folder, entry), source);
  rmSync(resolve(folder, "sketch.ts"));
  writeFileSync(resolve(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry, mode: "editable" }));
  let bridge = await serveProject(folder);
  t.after(async () => { await bridge.close(); rmSync(folder, { recursive: true, force: true }); });
  const rpc = async (method, input, state, operationId) => {
    const response = await fetch(`${bridge.origin}/api/rpc`, { method: "POST",
      headers: { Authorization: `Bearer ${bridge.token}`, "Content-Type": "application/json" },
      body: JSON.stringify({ method, input, clientId: "project-editor", authority: state?.authority, baseHash: state?.currentHash, operationId }) });
    return { status: response.status, ...await response.json() };
  };
  const restart = async () => { await bridge.close(); bridge = await serveProject(folder); return rpc("session.join"); };
  return { folder, entry, source, rpc, restart, get bridge() { return bridge; } };
}

test("complete folder writes entry and semantic design, Undo/Redo and restart retain source", async (t) => {
  const fixture = await setup(t);
  const { folder, entry, source, rpc, restart } = fixture;
  const initial = await rpc("session.join");
  assert.equal(initial.state.ok, true, JSON.stringify(initial.state));
  assert.equal(initial.result.history.canUndo, false);
  const candidate = source.replace("value: mm(10)", "value: mm(12)");
  const edited = await rpc("dispatch", { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: candidate } }, initial.state, "v2-source-edit");
  assert.equal(edited.status, 200, edited.error);
  assert.match(readFileSync(resolve(folder, entry), "utf8"), /value: mm\(12\)/);
  const design = JSON.parse(readFileSync(resolve(folder, ".geosolve/design.json"), "utf8"));
  assert.equal(design.format, "geosolve-design-v1");
  assert.deepEqual(Object.keys(design).sort(), ["format", "generated", "overrides", "project"]);
  const undo = await rpc("dispatch", { version: 2, command: "history.undo" }, edited.state, "v2-undo");
  assert.equal(undo.status, 200, undo.error);
  assert.equal(readFileSync(resolve(folder, entry), "utf8"), source);
  const redo = await rpc("dispatch", { version: 2, command: "history.redo" }, undo.state, "v2-redo");
  assert.equal(redo.status, 200, redo.error);
  const written = readFileSync(resolve(folder, entry), "utf8");
  const reopened = await restart();
  assert.equal(reopened.state.ok, true, JSON.stringify(reopened.state));
  assert.equal(readFileSync(resolve(folder, entry), "utf8"), written);
  const geometry = await fixture.bridge.project.adapter.bakeProfile(0.01);
  assert.ok(geometry.regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 12) < 1e-7));
});

test("rejected complete project retains accepted scene but cannot overwrite rejected disk on Undo", async (t) => {
  const { folder, entry, source, bridge, rpc } = await setup(t);
  let current = await rpc("session.join");
  current = await rpc("dispatch", { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: source.replace("value: mm(10)", "value: mm(12)") } }, current.state);
  assert.equal(current.status, 200, current.error);
  const rejected = source.replace("value: mm(10)", "value: mm(-12)");
  writeFileSync(resolve(folder, entry), rejected);
  await bridge.project.scan(true);
  const observed = await rpc("snapshot");
  assert.equal(observed.state.ok, false);
  const undo = await rpc("dispatch", { version: 2, command: "history.undo" }, observed.state);
  assert.equal(undo.status, 409, undo.error);
  assert.equal(readFileSync(resolve(folder, entry), "utf8"), rejected);
  await bridge.project.scan(true);
  assert.equal(bridge.project.state().ok, false);
});

test("invalid v2 GUI source retains its diagnostic draft without publishing or rolling it back", async (t) => {
  const { folder, entry, source, bridge, rpc } = await setup(t);
  const initial = await rpc("session.join");
  const design = await bridge.project.adapter.exportWorkspaceDesign();
  const draft = "// retained incomplete 😀\nnot valid managed source";
  const rejected = await rpc("dispatch", { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: draft } }, initial.state, "invalid-gui-source");
  assert.equal(rejected.status, 200, rejected.error);
  assert.equal(rejected.result.status, "retained");
  assert.equal(rejected.result.source.dirty, true);
  assert.equal(rejected.result.source.files[0].contents, draft);
  assert.ok(rejected.result.problems.length > 0);
  assert.deepEqual(rejected.result.seed, initial.result.seed);
  assert.deepEqual(rejected.result.history, initial.result.history);
  assert.deepEqual(await bridge.project.adapter.exportWorkspaceDesign(), design);
  assert.equal(readFileSync(resolve(folder, entry), "utf8"), source);
  assert.equal(rejected.state.currentHash, initial.state.currentHash);
  assert.equal(rejected.state.writes, initial.state.writes);
  await assert.rejects(bridge.project.adapter.exportProject(), /Canonical export.*invalid/);
  const reverted = await rpc("dispatch", { version: 2, command: "source.revert" }, rejected.state, "revert-gui-source");
  assert.equal(reverted.status, 200, reverted.error);
  assert.equal(reverted.result.source.dirty, false);
  assert.equal(reverted.result.source.files[0].contents, initial.result.source.files[0].contents);
  assert.equal(readFileSync(resolve(folder, entry), "utf8"), source);
  assert.equal(reverted.state.writes, initial.state.writes);
  assert.deepEqual(reverted.result.seed, initial.result.seed);
});

test("point drag publishes semantic sidecar and survives cold reopen without its derived cache", async (t) => {
  const f = await setup(t);
  const compiled = JSON.parse(readFileSync(new URL("../packages/geosolve-sketch-code/test/fixtures/managed-empty-circle.json", import.meta.url), "utf8"));
  writeFileSync(resolve(f.folder, f.entry), compiled.normalizedSource);
  await f.bridge.project.scan(true);
  let current = await f.rpc("session.join");
  const before = await f.bridge.project.adapter.bakeProfile(0.01);
  const command = await pointCommand(current.result, [4, -2]);
  current = await f.rpc("authoring.commit", { kind: "point", command }, current.state, "drag-release");
  assert.equal(current.status, 200, current.error);
  const after = await f.bridge.project.adapter.bakeProfile(0.01);
  assert.notDeepEqual(after.regions,before.regions,"drag must change accepted model-space points");
  const design = JSON.parse(readFileSync(resolve(f.folder,".geosolve/design.json"),"utf8"));
  assert.ok(design.overrides.drafts.length > 0);
  assert.equal(readFileSync(resolve(f.folder,f.entry),"utf8"),compiled.normalizedSource);
  // The next restart must prove semantic reconstruction rather than checkpoint replay.
  f.bridge.project.saveDerived = async () => {};
  rmSync(resolve(f.folder,".geosolve/derived-session.json"),{force:true});
  const reopened=await f.restart();
  assert.equal(reopened.state.ok,true,JSON.stringify(reopened.state));
  const restored=await f.bridge.project.adapter.bakeProfile(0.01);
  assert.deepEqual(restored.regions,after.regions);
});

test("transitive helper edits change geometry and Undo/Redo restore dependency bytes across restart", async (t) => {
  const f=await setup(t);
  const entry='"use geosolve sketch"; import {sketch,mm} from "@geosolve/sketch-code"; import {hole} from "./hole.ts"; export default sketch(($)=>{const bore=$.use("bore",hole,{radius:mm(3)});return {bore};});';
  const patch='import {definePatch,t} from "@geosolve/sketch-code"; import {center} from "./helper.ts"; export const hole=definePatch({radius:t.length()},(p,{radius})=>({circle:p.geometry.centerRadiusCircle("circle",{center,radius})}));';
  const helper=resolve(f.folder,"model/helper.ts");
  writeFileSync(resolve(f.folder,f.entry),entry);
  writeFileSync(resolve(f.folder,"model/hole.ts"),patch);
  writeFileSync(helper,"export const center=[0,0] as const;");
  await f.bridge.project.scan(true);
  let current=await f.rpc("session.join");
  assert.equal(current.state.ok,true,JSON.stringify(current.state));
  const before=await f.bridge.project.adapter.bakeProfile(0.01);
  writeFileSync(helper,"export const center=[20,0] as const;");
  await f.bridge.project.scan(true);
  current=await f.rpc("snapshot");
  assert.equal(current.state.ok,true,JSON.stringify(current.state));
  const after=await f.bridge.project.adapter.bakeProfile(0.01);
  assert.notDeepEqual(after.regions,before.regions);
  current=await f.restart();
  assert.equal(current.state.ok,true,JSON.stringify(current.state));
  current=await f.rpc("dispatch",{version:2,command:"history.undo"},current.state,"helper-undo");
  assert.equal(current.status,200,current.error);
  assert.equal(readFileSync(helper,"utf8"),"export const center=[0,0] as const;");
  assert.deepEqual((await f.bridge.project.adapter.bakeProfile(0.01)).regions,before.regions);
  current=await f.rpc("dispatch",{version:2,command:"history.redo"},current.state,"helper-redo");
  assert.equal(current.status,200,current.error);
  assert.equal(readFileSync(helper,"utf8"),"export const center=[20,0] as const;");
  assert.equal(readFileSync(resolve(f.folder,"model/hole.ts"),"utf8"),patch);
});

test("repairing rejected files to the last accepted bytes does not add an Undo step", async (t) => {
  const f = await setup(t);
  const initial = await f.rpc("session.join");
  const edited = await f.rpc("dispatch", { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: f.source.replace("value: mm(10)", "value: mm(12)") } }, initial.state);
  assert.equal(edited.status, 200, edited.error);
  const saved = readFileSync(resolve(f.folder, f.entry), "utf8");
  writeFileSync(resolve(f.folder, f.entry), saved.replace("value: mm(12)", "value: mm(-12)"));
  await f.bridge.project.scan(true);
  assert.equal(f.bridge.project.state().ok, false);
  writeFileSync(resolve(f.folder, f.entry), saved);
  await f.bridge.project.scan(true);
  const repaired = await f.rpc("snapshot");
  assert.equal(repaired.state.ok, true);
  const undo = await f.rpc("dispatch", { version: 2, command: "history.undo" }, repaired.state);
  assert.equal(undo.status, 200, undo.error);
  assert.equal(readFileSync(resolve(f.folder, f.entry), "utf8"), f.source);
});
