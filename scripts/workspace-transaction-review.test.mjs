// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { initProject, openProject, serveProject } from "./file-workspace.mjs";
import { acquireWorkspaceLock, createWorkspaceStorage } from "./workspace-storage.mjs";
import { evaluateWorkspaceSnapshot, readWorkspaceSnapshot } from "./workspace-loader.mjs";
import { engineModuleUrl } from "./workspace-runtime-paths.mjs";

async function fixture(t, populate, options) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-review-"));
  populate(folder);
  const bridge = await serveProject(folder, options);
  t.after(async () => { await bridge.close(); rmSync(folder, { recursive: true, force: true }); });
  const rpc = async (method, input, state, operationId) => {
    const response = await fetch(`${bridge.origin}/api/rpc`, { method: "POST",
      headers: { Authorization: `Bearer ${bridge.token}`, "Content-Type": "application/json" },
      body: JSON.stringify({ method, input, clientId: "transaction-review", authority: state?.authority, baseHash: state?.currentHash, operationId }) });
    return { status: response.status, ...await response.json() };
  };
  return { folder, bridge, rpc };
}

test("manifold GUI widths preserve complete profiles through semantic sidecar reconstruction", { timeout: 120000 }, async (t) => {
  const f = await fixture(t, (folder) => cpSync(resolve("examples/file-workspace-manifold"), folder, { recursive: true }));
  let current = await f.rpc("session.join");
  assert.equal(current.state.ok, true, JSON.stringify(current.state));
  const { createEngine } = await import(engineModuleUrl);
  const engine = await createEngine(); t.after(() => engine.dispose());
  const evidence = [];
  for (const width of [12, 10, 11]) {
    if (width !== 12) {
      const parameter = current.result.parameters.find((parameter) => parameter.label === "Channel width");
      assert.ok(parameter, JSON.stringify(current.result.parameters));
      current = await f.rpc("dispatch", { version: 2, command: "parameter.edit", payload: { id: parameter.id, value: String(width) } }, current.state, `manifold-width-${width}`);
      assert.equal(current.status, 200, current.error);
      assert.equal(current.state.ok, true, JSON.stringify(current.state));
    }
    const snapshot = readWorkspaceSnapshot(f.folder);
    const compiled = await evaluateWorkspaceSnapshot(snapshot);
    const project = engine.compileProject({ project: "code-authored-sketch", compiled: compiled.compiled, customFiles: compiled.customFiles, artifacts: compiled.artifacts, lock: compiled.lock });
    const design = readFileSync(resolve(f.folder, ".geosolve/design.json"), "utf8");
    const session = engine.openEditableSession(project, { design });
    const accepted = session.accepted;
    const before = JSON.stringify(accepted);
    const circles = accepted.geometry.curves.filter((item) => item.curve.definition.kind === "circle").map((item) => ({ label: item.curve.label,
      center: accepted.geometry.points.find((point) => point.id === item.curve.definition.center)?.position,
      radius: accepted.geometry.scalars.find((scalar) => scalar.id === item.curve.definition.radius)?.value }));
    let exported, error;
    try { exported = await engine.exportProfiles(accepted, { chordErrorMm: 0.02 }); } catch (cause) { error = String(cause); }
    evidence.push({ width, circles, regions: exported?.regions?.length, error, source: snapshot.files.find((file) => file.path === "sketch.ts").contents,
      geometry: accepted.geometry, design: JSON.parse(design) });
    assert.equal(JSON.stringify(accepted), before);
    session.dispose();
  }
  writeFileSync(resolve("target/m98/manifold-width-characterization.json"), JSON.stringify(evidence));
  t.diagnostic(JSON.stringify(evidence.map(({ width, circles, regions, error }) => ({ width, circles, regions, error: error?.slice(0, 140) }))));
  for (const row of evidence) {
    assert.equal(row.circles.filter((circle) => circle.radius === 3).length, 5);
    assert.equal(row.error, undefined, `${row.width} mm export must certify disjoint contours`);
    assert.equal(row.regions, 18);
  }
});

async function directFixture(t, { fault = () => {}, populate } = {}) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-direct-review-"));
  initProject(folder);
  writeFileSync(resolve(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }));
  populate?.(folder);
  const lock = acquireWorkspaceLock(folder);
  const storage = createWorkspaceStorage(folder, { lock, fault });
  let project = await openProject(folder, { storage });
  t.after(async () => { await project.dispose(); lock.release(); rmSync(folder, { recursive: true, force: true }); });
  const clientId = "direct-review";
  await project.request("session.join", undefined, undefined, { clientId });
  const request = (method, input, operationId, basis = project.state(clientId)) => project.request(method, input, basis.currentHash, { clientId, authority: basis.authority, operationId });
  return { folder, storage, get project() { return project; }, request, state: () => project.state(clientId), source: () => readFileSync(resolve(folder, "sketch.ts"), "utf8"),
    restart: async () => { await project.dispose(); project = await openProject(folder, { storage }); await project.request("session.join", undefined, undefined, { clientId }); } };
}

function sourceEdit(source, radius) {
  return { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: source.replace("value: mm(10)", `value: mm(${radius})`) } };
}

test("storage staging failure restores native accepted geometry and authored bytes", async (t) => {
  let armed = false;
  const f = await directFixture(t, { fault: (point) => { if (armed && point === "staged") throw Error("injected staged publication failure"); } });
  const source = f.source();
  const before = await f.project.adapter.bakeProfile(0.02);
  const basis = f.state();
  armed = true;
  await assert.rejects(f.request("dispatch", sourceEdit(source, 12), "staged-failure", basis), /injected staged publication failure/);
  assert.equal(f.source(), source);
  assert.deepEqual((await f.project.adapter.bakeProfile(0.02)).regions, before.regions);
  assert.equal(f.state().acceptedHash, basis.acceptedHash);
  assert.equal(f.storage.outcome("staged-failure").state, "conflict");
});

test("semantic export failure after native dispatch restores the last accepted geometry", async (t) => {
  const f = await directFixture(t);
  const source = f.source();
  const before = await f.project.adapter.bakeProfile(0.02);
  const exportDesign = f.project.adapter.exportWorkspaceDesign;
  f.project.adapter.exportWorkspaceDesign = async () => { throw Error("injected semantic export failure"); };
  await assert.rejects(f.request("dispatch", sourceEdit(source, 12), "export-failure"), /injected semantic export failure/);
  f.project.adapter.exportWorkspaceDesign = exportDesign;
  assert.equal(f.source(), source);
  assert.deepEqual((await f.project.adapter.bakeProfile(0.02)).regions, before.regions);
  assert.equal(f.storage.outcome("export-failure"), null);
});

test("acknowledgment failure cannot advertise rolled-back native state as matching published disk", async (t) => {
  let armed = false;
  const f = await directFixture(t, { fault: (point) => { if (armed && point === "before-acknowledge") throw Error("injected acknowledgment failure"); } });
  const source = f.source();
  const before = await f.project.adapter.bakeProfile(0.02);
  const basis = f.state();
  const input = sourceEdit(source, 12);
  armed = true;
  await assert.rejects(f.request("dispatch", input, "ack-failure", basis), /injected acknowledgment failure/);
  assert.match(f.source(), /value: mm\(12\)/);
  assert.deepEqual((await f.project.adapter.bakeProfile(0.02)).regions, before.regions);
  assert.equal(f.storage.outcome("ack-failure").state, "published");
  assert.equal(f.state().ok, false, "published disk and rolled-back geometry are different accepted revisions");
  armed = false;
  await f.request("dispatch", input, "ack-failure", basis);
  assert.equal(f.state().ok, true, "same receipt retry must reconcile the published source before returning");
  assert.ok((await f.project.adapter.bakeProfile(0.02)).regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 12) < 1e-7));
  const accepted = f.state();
  await f.request("dispatch", input, "ack-failure", basis);
  assert.equal(f.state().writes, accepted.writes);
  assert.equal(f.state().acceptedHash, accepted.acceptedHash);
});

test("corrupt cached helper hashes cannot make Undo publish geometry without its matching source", async (t) => {
  const original = "export const center=[0,0] as const;";
  const edited = "export const center=[20,0] as const;";
  const f = await directFixture(t, { populate: (folder) => {
    writeFileSync(resolve(folder, "sketch.ts"), '"use geosolve sketch"; import {sketch,mm} from "@geosolve/sketch-code"; import {hole} from "./hole.ts"; export default sketch(($)=>{const bore=$.use("bore",hole,{radius:mm(3)});return {bore};});');
    writeFileSync(resolve(folder, "hole.ts"), 'import {definePatch,t} from "@geosolve/sketch-code"; import {center} from "./helper.ts"; export const hole=definePatch({radius:t.length()},(p,{radius})=>({circle:p.geometry.centerRadiusCircle("circle",{center,radius})}));');
    writeFileSync(resolve(folder, "helper.ts"), original);
  } });
  assert.equal(f.state().ok, true);
  const before = await f.project.adapter.bakeProfile(0.02);
  writeFileSync(resolve(f.folder, "helper.ts"), edited);
  writeFileSync(resolve(f.folder, "sketch.ts"), f.source().replace("radius:mm(3)", "radius:mm(4)"));
  await f.project.scan(true);
  assert.equal(f.state().ok, true);
  const after = await f.project.adapter.bakeProfile(0.02);
  assert.notDeepEqual(after.regions, before.regions);
  await f.project.saveDerived();
  const path = resolve(f.folder, ".geosolve/derived-session.json");
  const derived = JSON.parse(readFileSync(path, "utf8"));
  const currentHash = readWorkspaceSnapshot(f.folder).files.find((file) => file.path === "helper.ts").sha256;
  for (const [, snapshot] of derived.sources) for (const file of snapshot.files) if (file.path === "helper.ts") file.sha256 = currentHash;
  writeFileSync(path, JSON.stringify(derived));
  await f.restart();
  assert.equal(f.state().ok, true);
  try {
    await f.request("dispatch", { version: 2, command: "history.undo" }, "corrupt-history-undo");
  } catch (error) {
    assert.match(String(error), /histor|cache|match|hash|source|authored/i);
  }
  // A corrupted optional cache may be ignored/rejected or reconstructed safely.
  // In either case disk and accepted geometry must describe the same circle.
  const disk = readFileSync(resolve(f.folder, "helper.ts"), "utf8");
  assert.ok([original, edited].includes(disk));
  const center = disk === original ? 0 : 20;
  const radius = /radius:\s*mm\(4\)/.test(f.source()) ? 4 : 3;
  const actual = await f.project.adapter.bakeProfile(0.02);
  assert.equal(actual.regions.length, 1);
  assert.ok(actual.regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x - center, y) - radius) < 1e-7),
    `accepted geometry must match disk center ${center} radius ${radius}`);
});

for (const mode of ["editable", "generator"]) test(`external ${mode} save during native evaluation cannot install stale geometry`, async (t) => {
  const f = await directFixture(t, { populate: mode === "generator" ? (folder) => {
    writeFileSync(resolve(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode }));
    writeFileSync(resolve(folder, "sketch.ts"), 'import {defineGenerator,sketch,mm} from "@geosolve/sketch-code"; export default defineGenerator({},()=>sketch(($)=>({bore:$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(10)})})));');
  } : undefined });
  assert.equal(f.state().ok, true, JSON.stringify(f.state()));
  const source = f.source();
  const geometry = async () => mode === "editable" ? (await f.project.adapter.bakeProfile(0.02)).regions
    : (await f.project.adapter.snapshot()).frame.scene.items.filter((item) => item.layer === "geometry");
  const before = await geometry();
  const candidate = source.replaceAll("mm(10)", "mm(12)");
  const external = source.replaceAll("mm(10)", "mm(14)");
  writeFileSync(resolve(f.folder, "sketch.ts"), candidate);
  const dispatch = f.project.adapter.dispatch;
  let injected = false;
  f.project.adapter.dispatch = async (input) => {
    const result = await dispatch(input);
    if (input.command === `workspace.${mode === "generator" ? "generator" : "project"}.apply`) {
      injected = true;
      writeFileSync(resolve(f.folder, "sketch.ts"), external);
    }
    return result;
  };
  await f.project.scan(true);
  f.project.adapter.dispatch = dispatch;
  assert.equal(injected, true);
  assert.equal(f.source(), external);
  assert.equal(f.state().ok, false);
  assert.deepEqual(await geometry(), before);
  await f.project.scan(true);
  assert.equal(f.state().ok, true, JSON.stringify(f.state()));
  if (mode === "editable") assert.ok((await geometry())[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 14) < 1e-7));
  else assert.notDeepEqual(await geometry(), before);
});

for (const field of ["runtime", "revision", "project", "design"]) test(`corrupted derived ${field} identity leaves authored geometry authoritative`, async (t) => {
  const f = await directFixture(t);
  const source = f.source();
  const before = await f.project.adapter.bakeProfile(0.02);
  await f.project.saveDerived();
  const path = resolve(f.folder, ".geosolve/derived-session.json");
  const derived = JSON.parse(readFileSync(path, "utf8"));
  derived[field] = `corrupt-${field}`;
  derived.contents = "invalid discarded history";
  writeFileSync(path, JSON.stringify(derived));
  await f.restart();
  assert.equal(f.state().ok, true, JSON.stringify(f.state()));
  assert.equal(f.source(), source);
  assert.deepEqual((await f.project.adapter.bakeProfile(0.02)).regions, before.regions);
});

test("external save after acknowledgment cannot acquire authority for another native geometry", async (t) => {
  let folder, external;
  const f = await directFixture(t, { fault: (point) => {
    if (point === "acknowledged" && external) writeFileSync(resolve(folder, "sketch.ts"), external);
  } });
  folder = f.folder;
  const source = f.source();
  const before = await f.project.adapter.bakeProfile(0.02);
  external = source.replaceAll("mm(10)", "mm(14)");
  await assert.rejects(f.request("dispatch", sourceEdit(source, 12), "acknowledged-race"), /Conflict:.*publication|changed/i);
  assert.equal(f.source(), external);
  assert.equal(f.state().ok, false);
  assert.deepEqual((await f.project.adapter.bakeProfile(0.02)).regions, before.regions);
  assert.equal(f.storage.outcome("acknowledged-race").state, "acknowledged");
  external = undefined;
  await f.project.scan(true);
  assert.equal(f.state().ok, true);
  assert.ok((await f.project.adapter.bakeProfile(0.02)).regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 14) < 1e-7));
});
