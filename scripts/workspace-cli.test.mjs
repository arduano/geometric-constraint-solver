// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { runCli } from "./geosolve-cli.mjs";
import { serveProject } from "./file-workspace.mjs";

test("plaintext CLI inspection, explicit ownership, accepted apply and idempotent outcome", async (t) => {
  const parent = mkdtempSync(resolve(tmpdir(), "geosolve-m98-cli-"));
  const folder = resolve(parent, "project");
  t.after(() => rmSync(parent, { recursive: true, force: true }));
  await runCli(["init", folder]);
  const inspected = await runCli(["inspect", folder]);
  assert.equal(inspected.files.length, 2);
  assert.equal(inspected.mode, "editable");
  const bridge = await serveProject(folder);
  t.after(() => bridge.close());
  const expected = resolve(parent, "status.json");
  const flags = ["--client", "cli-editor-1", "--expected", expected];
  await runCli(["status", folder, "--client", "cli-editor-1", "--out", expected]);
  await runCli(["takeover", folder, ...flags]);
  await runCli(["status", folder, "--client", "cli-editor-1", "--out", expected]);
  const files = resolve(parent, "files.json");
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  writeFileSync(files, JSON.stringify({ "sketch.ts": source.replace("value: mm(10)", "value: mm(14)") }));
  const applied = await runCli(["apply", folder, ...flags, "--operation", "cli-apply-1", "--files", files]);
  assert.equal(applied.state.ok, true);
  assert.match(readFileSync(resolve(folder, "sketch.ts"), "utf8"), /value: mm\(14\)/);
  const repeated = await runCli(["apply", folder, ...flags, "--operation", "cli-apply-1", "--files", files]);
  assert.equal(repeated.state.writes, applied.state.writes);
  const outcome = await runCli(["outcome", folder, "--operation", "cli-apply-1"]);
  assert.equal(outcome.operation.result.state, "acknowledged");
  await assert.rejects(runCli(["apply", folder, ...flags, "--operation", "cli-stale", "--files", files]), /Conflict/);
});
