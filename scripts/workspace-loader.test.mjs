// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { createWorkspaceLoader, evaluateWorkspaceSnapshot, readWorkspaceSnapshot } from "../packages/geosolve-cli/runtime/workspace-loader.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const sdkDirectory = process.env.GEOSOLVE_LOADER_TEST_SDK_DIRECTORY ?? resolve(root, "packages/geosolve-sketch-code/dist/src");
const options = { sdkDirectory };

function fixture(t, files = {}, mode = "generator") {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-loader-"));
  t.after(() => rmSync(folder, { recursive: true, force: true }));
  const write = (path, contents) => { mkdirSync(dirname(resolve(folder, path)), { recursive: true }); writeFileSync(resolve(folder, path), contents); };
  write("geosolve.json", JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode }));
  for (const [path, contents] of Object.entries(files)) write(path, contents);
  return { folder, write, snapshot: (extra) => readWorkspaceSnapshot(folder, { ...options, ...extra }), run: (snapshot, extra) => evaluateWorkspaceSnapshot(snapshot, { ...options, ...extra }) };
}

test("generator loader discovers code-owned defaults and transitive helpers from one immutable snapshot", async (t) => {
  const f = fixture(t, {
    "sketch.ts": 'import {defineGenerator,sketch} from "@geosolve/sketch-code"; import {point} from "./helpers/point.js"; export default defineGenerator({count:{type:"integer",default:2,min:1}}, ({count})=>sketch(($)=>{const points=[];for(let i=0;i<count;i++) points.push($.geometry.sketchPoint(`p-${i}`,{point:point(i)})); return {points};}));',
    "helpers/point.ts": 'import {spacing} from "./spacing.ts"; export const point=(i:number)=>[i*spacing,0] as const;',
    "helpers/spacing.ts": "export const spacing=10;",
  });
  const before = f.snapshot();
  f.write("helpers/spacing.ts", "export const spacing=30;");
  const after = f.snapshot({ inputs: { count: 3 } });
  assert.notEqual(before.revision, after.revision);
  assert.equal(before.sourceHash, after.sourceHash);
  assert.deepEqual(before.files.map(({ path }) => path), ["geosolve.json", "helpers/point.ts", "helpers/spacing.ts", "sketch.ts"]);
  const first = await f.run(before);
  assert.equal(first.generated.declarations.length, 2);
  assert.deepEqual(first.generated.declarations[1].arguments.value.point.value.map(({ value }) => value), [10, 0]);
  assert.deepEqual(first.inputs, { count: 2 });
  assert.equal(first.inputDefinitions.count.type, "integer");
  assert.equal(first.revision, before.revision);
  const second = await f.run(after);
  assert.equal(second.generated.declarations.length, 3);
  assert.deepEqual(second.generated.declarations[1].arguments.value.point.value.map(({ value }) => value), [30, 0]);
});

test("editable loader compiles fresh patch artifacts and exact source/interface lock pins", async (t) => {
  const f = fixture(t, {
    "sketch.ts": '"use geosolve sketch"; import {sketch,mm} from "@geosolve/sketch-code"; import {hole} from "./patches/hole.patch.ts"; export default sketch(($)=>{const bore=$.use("bore",hole,{radius:mm(3)});return {bore};});',
    "patches/hole.patch.ts": 'import {definePatch,t} from "@geosolve/sketch-code"; import {center} from "./helper.ts"; export const hole=definePatch({radius:t.length()},(p,{radius})=>({circle:p.geometry.centerRadiusCircle("circle",{center,radius})}));',
    "patches/helper.ts": "export const center=[0,0] as const;",
  }, "editable");
  const before = f.snapshot();
  const first = await f.run(before);
  const pin = first.lock.modules["./patches/hole.patch.ts"];
  assert.equal(first.compiled.inputSourceDigest, before.sourceHash);
  assert.equal(first.artifacts[pin.artifact].source_digest, pin.source);
  assert.equal(first.customFiles["patches/hole.patch.ts"].source_digest, pin.source);
  assert.equal(first.artifacts[pin.artifact].interface_digest, pin.interface);
  assert.equal(first.generated, undefined);
  f.write("patches/helper.ts", "export const center=[20,0] as const;");
  const second = await f.run(f.snapshot());
  assert.notEqual(second.revision, first.revision);
  assert.notEqual(second.lock.modules["./patches/hole.patch.ts"].artifact, pin.artifact);
  assert.equal(second.lock.modules["./patches/hole.patch.ts"].source, pin.source, "full project provenance owns helper changes independently of direct source hash");
});

test("real manifold loads as editable project with every current patch freshly recorded", async (t) => {
  const f = fixture(t, {}, "editable");
  const sample = resolve(root, "crates/geosolve-sketch-code/assets/bundled-samples/pc-water-manifold");
  f.write("sketch.ts", readFileSync(resolve(sample, "sketch.ts"), "utf8"));
  cpSync(resolve(sample, "patches"), resolve(f.folder, "patches"), { recursive: true });
  const snapshot = f.snapshot();
  assert.equal(snapshot.files.length, 5, "cached artifact files are not dependency authority");
  const result = await f.run(snapshot);
  assert.equal(Object.keys(result.customFiles).length, 3);
  assert.equal(Object.keys(result.artifacts).length, 3);
  assert.equal(result.compiled.artifact.parameters.length, 2);
  assert.equal(result.compiled.artifact.groups.length, 7);
  assert.equal(result.compiled.normalizedSource, readFileSync(resolve(f.folder, "sketch.ts"), "utf8"));
  f.write("geosolve.json", JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "generator" }));
  const generated = await f.run(f.snapshot());
  assert.equal(generated.generated.declarations.length, 146);
  assert.equal(generated.generated.applications.length, 5);
  assert.equal(generated.generated.parameters.length, 2);
  assert.equal(generated.generated.groups.length, 7);
  assert.equal(generated.compiled, undefined);
});

test("legacy entry, flexible nested entry, sidecar changes and malformed dependencies are explicit", async (t) => {
  const f = fixture(t, { "sketch.ts": '"use geosolve sketch"; import {sketch} from "@geosolve/sketch-code"; export default sketch(($)=>{return {};});' }, "editable");
  f.write("geosolve.json", JSON.stringify({ format: "geosolve-folder-v1", entry: "sketch.ts" }));
  assert.equal((await f.run(f.snapshot())).mode, "editable");
  f.write("geosolve.json", JSON.stringify({ format: "geosolve-folder-v2", entry: "design/main.ts", mode: "generator" }));
  f.write("design/main.ts", 'import {sketch} from "@geosolve/sketch-code";export default ({count})=>sketch(()=>({count}));');
  f.write(".geosolve/inputs.json", '{"count":3}');
  const first = f.snapshot();
  assert.deepEqual((await f.run(first)).generated.output, { kind: "object", value: { count: { kind: "number", value: 3 } } });
  f.write(".geosolve/design.json", '{"format":"geosolve-design-v1"}');
  assert.notEqual(f.snapshot().revision, first.revision);
  f.write("design/main.ts", 'import {x} from "./missing.ts";export default x;');
  assert.throws(() => f.snapshot(), (error) => error.code === "invalid_project" && error.path === "design/main.ts" && error.line === 1);
  f.write("design/main.ts", "export default (");
  assert.throws(() => f.snapshot(), (error) => error.code === "invalid_project" && error.line === 1);
});

test("loader refuses package/remote/computed imports, escapes, symlinks and invalid input values", (t) => {
  const f = fixture(t);
  for (const source of ['import x from "node:fs";export default x;', 'import x from "https://example.org/x.ts";export default x;', 'const p="./x.ts";export default import(p);', 'export default require("./x.ts");', 'import x from "../outside.ts";export default x;']) {
    f.write("sketch.ts", source);
    assert.throws(() => f.snapshot(), (error) => error.code === "invalid_project");
  }
  f.write("sketch.ts", 'import x from "./alias.ts";export default x;');
  f.write("actual.ts", "export default 1;");
  symlinkSync(resolve(f.folder, "actual.ts"), resolve(f.folder, "alias.ts"));
  assert.throws(() => f.snapshot(), /symlink/u);
  f.write("sketch.ts", 'import {sketch} from "@geosolve/sketch-code";export default sketch(()=>null);');
  assert.throws(() => f.snapshot({ inputs: { n: NaN } }), /finite JSON/u);
});

test("workers reject generator failure and can terminate an infinite loop without blocking the caller", async (t) => {
  const f = fixture(t, { "sketch.ts": 'export default ()=>{throw Error("generator rejected this input");};' });
  await assert.rejects(f.run(f.snapshot()), (error) => error.code === "evaluation_failed" && error.path === "sketch.ts" && /rejected this input/u.test(error.message));
  f.write("sketch.ts", 'export default ()=>{for(;;){};};');
  const started = Date.now();
  await assert.rejects(f.run(f.snapshot(), { timeoutMs: 500 }), (error) => error.code === "timeout");
  assert.ok(Date.now() - started < 3000);
  const controller = new AbortController();
  const pending = f.run(f.snapshot(), { signal: controller.signal });
  controller.abort();
  await assert.rejects(pending, (error) => error.code === "cancelled");
});

test("newer loader request supersedes previous evaluation and disposal prevents further work", async (t) => {
  const f = fixture(t, { "sketch.ts": 'export default ()=>{for(;;){};};' });
  const loader = createWorkspaceLoader(options);
  t.after(() => loader.dispose());
  const first = loader.load(f.folder);
  const rejected = assert.rejects(first, (error) => error.code === "cancelled");
  f.write("sketch.ts", 'import {sketch} from "@geosolve/sketch-code";export default ()=>sketch(()=>({ready:true}));');
  const second = await loader.load(f.folder);
  await rejected;
  assert.equal(second.result.generated.output.value.ready.value, true);
  loader.dispose();
  await assert.rejects(loader.load(f.folder), (error) => error.code === "cancelled");
});

test("candidate file graph is compiled without changing original disk and can add local dependencies", async (t) => {
  const f = fixture(t, { "sketch.ts": 'import {sketch} from "@geosolve/sketch-code";export default ()=>sketch(()=>({value:1}));' });
  const original = f.snapshot();
  const fileOverrides = {
    "sketch.ts": 'import {sketch} from "@geosolve/sketch-code";import {value} from "./added.ts";export default ()=>sketch(()=>({value}));',
    "added.ts": 'export const value=42;',
  };
  const candidate = f.snapshot({ fileOverrides });
  assert.notEqual(candidate.revision, original.revision);
  const result = await f.run(candidate);
  assert.equal(result.generated.output.value.value.value, 42);
  assert.equal(f.snapshot().revision, original.revision);
  assert.throws(() => f.snapshot({ fileOverrides: { "../escape.ts": "x" } }), /escapes/);
});

test("complete collaborative source snapshots never import omitted disk bytes or newer mirror edits", async (t) => {
  const f = fixture(t, {
    "sketch.ts": 'import {sketch} from "@geosolve/sketch-code"; import {point} from "./helper.ts"; export default sketch(($)=>{const p=$.geometry.sketchPoint("p",{point}); return {p};});',
    "helper.ts": "export const point=[12,8] as const;",
  });
  const disk = f.snapshot();
  const capturedFiles = Object.fromEntries(disk.files.map(({ path, contents }) => [path, contents]));
  f.write("helper.ts", "export const point=[99,99] as const;");
  const captured = f.snapshot({ capturedFiles });
  assert.equal(captured.revision, disk.revision);
  const result = await f.run(captured);
  assert.deepEqual(result.generated.declarations[0].arguments.value.point.value.map(({ value }) => value), [12, 8]);
  const omitted = { ...capturedFiles }; delete omitted["helper.ts"];
  assert.throws(() => f.snapshot({ capturedFiles: omitted }), /Cannot resolve local import/u);
  assert.throws(() => f.snapshot({ capturedFiles, fileOverrides: {} }), /Choose disk overrides/u);
  const absentManifest = { ...capturedFiles }; delete absentManifest["geosolve.json"];
  assert.throws(() => f.snapshot({ capturedFiles: absentManifest }), /absent from the captured source tree/u);
});
