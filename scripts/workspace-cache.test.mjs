// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { initProject, openProject, hash } from "./file-workspace.mjs";

function fixture(t) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-cache-"));
  t.after(() => rmSync(folder, { recursive: true, force: true }));
  initProject(folder);
  mkdirSync(resolve(folder, ".geosolve"));
  return folder;
}

test("M98-F002: invalid UTF-8 derived cache cannot prevent valid disk source from opening", async (t) => {
  const folder = fixture(t);
  writeFileSync(resolve(folder, ".geosolve/last-good.ts"), Buffer.from([0xff]));
  const project = await openProject(folder);
  t.after(() => project.dispose());
  assert.equal(project.state().ok, true);
  assert.equal(project.state().acceptedHash, hash(readFileSync(resolve(folder, "sketch.ts"))));
});

test("invalid disk source retains valid last accepted geometry without publishing the cache to disk", async (t) => {
  const folder = fixture(t);
  const previous = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  writeFileSync(resolve(folder, ".geosolve/last-good.ts"), previous);
  writeFileSync(resolve(folder, "sketch.ts"), "invalid source");
  const project = await openProject(folder);
  t.after(() => project.dispose());
  assert.equal(project.state().ok, false);
  assert.equal(project.state().currentHash, hash("invalid source"));
  assert.equal(project.state().acceptedHash, hash(previous));
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), "invalid source");
  assert.equal((await project.adapter.snapshot()).source.dirty, true);
});

test("invalid disk and corrupt cache produce diagnostics while preserving disk bytes", async (t) => {
  const folder = fixture(t);
  writeFileSync(resolve(folder, ".geosolve/last-good.ts"), Buffer.from([0xff]));
  writeFileSync(resolve(folder, "sketch.ts"), "invalid source");
  const project = await openProject(folder);
  t.after(() => project.dispose());
  assert.equal(project.state().ok, false);
  assert.equal(project.state().acceptedHash, null);
  assert.ok(project.state().warnings.some((warning) => warning.includes("cache")));
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), "invalid source");
});


test("invalid cached TypeScript cannot replace current disk diagnostics", async (t) => {
  const folder = fixture(t);
  writeFileSync(resolve(folder, ".geosolve/last-good.ts"), "broken cache");
  writeFileSync(resolve(folder, "sketch.ts"), "broken disk");
  const project = await openProject(folder);
  t.after(() => project.dispose());
  assert.equal(project.state().ok, false);
  assert.equal(project.state().acceptedHash, null);
  const snapshot = await project.adapter.snapshot();
  assert.equal(snapshot.source.files.find((file) => file.path === "sketch.ts").contents, "broken disk");
  assert.ok(project.state().warnings.some((warning) => warning.includes("cache")));
});
