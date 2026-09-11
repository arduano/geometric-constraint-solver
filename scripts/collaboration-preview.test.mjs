// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { setTimeout as delay } from "node:timers/promises";
import { createEngine } from "../packages/geosolve-engine/dist/index.js";
import { createCollaborationPreviewService } from "./collaboration-preview.mjs";
import { createCollaborationPreviewRoute } from "./collaboration-preview-route.mjs";
const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 };
const connection = (sessionId = "alice-1", role = "editor") => ({ documentEpoch: "document-life", serverEpoch: "server-life", sessionId, userId: sessionId, clientId: `${sessionId}-tab`, role });
async function fixture(t, options = {}) {
  const compiled = JSON.parse(await readFile(new URL("../crates/geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json", import.meta.url), "utf8"));
  const engine = await createEngine();
  const project = engine.compileProject({ project: "server-preview", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  const native = engine.openEditableSession(project);
  const basis = { documentEpoch: "document-life", revision: 0, sourceDesignDigest: native.sourceDesignDigest() };
  const model = { ...basis, project: native.exportProject(), design: native.exportDesign() };
  const active = new Set(), services = [];
  let captures = 0;
  function service(extra = {}) {
    const value = createCollaborationPreviewService({ enabled: true,
      authenticate: c => { if (!active.has(c.sessionId)) throw Error("Connection is no longer authenticated"); },
      captureBasis: () => { captures++; return model; }, ...options, ...extra });
    services.push(value); return value;
  }
  const api = service(), alice = connection(); active.add(alice.sessionId);
  t.after(async () => { await Promise.all(services.map(value => value.close())); native.dispose(); engine.dispose(); });
  const begin = { action: "begin", basis, kind: "point", gestureId: 9, viewport, target: native.pointGestureTargets()[0].target };
  return { api, service, alice, active, native, basis, model, begin, captures: () => captures };
}
function frozenAuthority(native) { return { state: native.state, project: native.exportProject(), design: native.exportDesign(), digest: native.sourceDesignDigest() }; }
function sameAuthority(native, before) { assert.equal(native.state, before.state); assert.equal(native.exportProject(), before.project); assert.deepEqual(native.exportDesign(), before.design); assert.equal(native.sourceDesignDigest(), before.digest); }

test("server point preview uses the same native trace and returns intent without publishing", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native), route = createCollaborationPreviewRoute(f.api);
  const local = f.native.beginPointGesture(f.begin.target, { expected: f.native.token, gestureId: 9, viewport });
  const start = await route(f.alice, f.begin);
  assert.equal(start.kind, "preview"); assert.deepEqual(start.basis, f.basis);
  assert.equal(typeof start.presentation, "string"); assert.equal(Object.hasOwn(start, "view"), false);
  const samples = [{ sequence: 1, position: [3, 2] }, { sequence: 2, position: [6, 4] }];
  let localFrame; for (const sample of samples) localFrame = local.advance(sample);
  const advanced = await route(f.alice, { action: "advance", ticket: start.ticket, samples });
  assert.deepEqual(advanced.point, localFrame);
  assert.deepEqual(JSON.parse(advanced.presentation).bindings, JSON.parse(local.presentationJSON()).bindings);
  sameAuthority(f.native, before);
  const finished = await route(f.alice, { action: "finish", ticket: start.ticket });
  assert.equal(finished.kind, "point"); assert.deepEqual(finished.terminal, local.finish());
  assert.equal(Object.hasOwn(finished, "ticket"), false);
  assert.equal(f.api.stats().previews, 0); sameAuthority(f.native, before);
  await assert.rejects(route(f.alice, { action: "finish", ticket: start.ticket }), error => error.code === "preview_missing");
  const prepared = f.native.preparePointGestureCommit(finished.terminal.command, { expected: f.native.token });
  assert.ok(prepared.result.validation.hard_residuals_validated && prepared.result.validation.all_active_features_current);
  const [a, b] = prepared.result.geometry.points.map(value => value.position);
  assert.ok(Math.abs(Math.hypot(a[0] - b[0], a[1] - b[1]) - 20) < 1e-8);
  f.native.releasePointGestureCommit(prepared); sameAuthority(f.native, before);
});

test("server construction preview preserves native samples, inference and exact expected declarations", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native);
  const begin = { action: "begin", basis: f.basis, kind: "construction", gestureId: 15, viewport, tool: "segment", role: "construction" };
  const local = f.native.beginConstruction("segment", { expected: f.native.token, gestureId: 15, viewport, role: "construction" });
  const start = await f.api.request(f.alice, begin);
  const samples = [[-25, 15], [-15, 25]].map((position, index) => ({ sequence: index + 1, input: { event: "click", position, suppressed: true, regularized: false } }));
  let frame; for (const sample of samples) frame = local.advance(sample);
  const advanced = await f.api.request(f.alice, { action: "advance", ticket: start.ticket, samples });
  assert.deepEqual(advanced.construction, frame);
  const finished = await f.api.request(f.alice, { action: "finish", ticket: start.ticket });
  assert.deepEqual(finished.command, local.finish());
  assert.equal(finished.command.role, "construction"); assert.ok(finished.command.expected_declarations.length > 0);
  sameAuthority(f.native, before);
});

test("server advanced construction traces preserve staged geometry and explicit branch options", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native);
  for (const [tool, points] of [
    ["center_arc", [[30, 30], [34, 30], [32, 33]]],
    ["center_axes_elliptical_arc", [[30, 30], [34, 30], [30, 32], [34, 30], [30, 32]]],
    ["open_control_nurbs", [[30, 30], [34, 30], [34, 34], [30, 34]]],
  ]) {
    const local = f.native.beginConstruction(tool, { expected: f.native.token, gestureId: 21, viewport });
    const start = await f.api.request(f.alice, { action: "begin", basis: f.basis, kind: "construction", gestureId: 21, viewport, tool });
    assert.deepEqual(start.construction, local.initialFrame);
    let sequence = 0;
    for (const [index, position] of points.entries()) {
      const samples = ["move", "click"].map(event => ({ sequence: ++sequence, input: { event, position, suppressed: true, regularized: false } }));
      let frame; for (const sample of samples) frame = local.advance(sample);
      const remote = await f.api.request(f.alice, { action: "advance", ticket: start.ticket, samples });
      assert.deepEqual(remote.construction, frame, tool);
      if (tool === "center_arc" && index === 1) {
        const sample = { sequence: ++sequence, input: { event: "flip_branch" } };
        assert.deepEqual((await f.api.request(f.alice, { action: "advance", ticket: start.ticket, samples: [sample] })).construction, local.advance(sample));
      }
    }
    if (tool.endsWith("nurbs")) {
      const sample = { sequence: ++sequence, input: { event: "complete" } };
      assert.deepEqual((await f.api.request(f.alice, { action: "advance", ticket: start.ticket, samples: [sample] })).construction, local.advance(sample));
    }
    const terminal = await f.api.request(f.alice, { action: "finish", ticket: start.ticket });
    assert.deepEqual(terminal.command, local.finish(), tool);
    sameAuthority(f.native, before);
  }
});

test("server relation activation preserves preselected operands, native options and terminal intent", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native);
  const curve = f.native.accepted.geometry.curves[0];
  assert.ok(curve);
  const selection = f.native.toolOperationOperands({ items: [{ Curve: { curve: curve.curve.id, segment: 0 } }], curve_picks: [] });
  const defaults = f.native.beginToolOperation("segment_length", { expected: f.native.token, gestureId: 25, viewport, selection: [] });
  const options = { authoring_options: { ...defaults.initialFrame.authoring_options, dimension_mode: "reference" } };
  defaults.cancel();
  const local = f.native.beginToolOperation("segment_length", { expected: f.native.token, gestureId: 26, viewport, selection, options });
  const started = await f.api.request(f.alice, { action: "begin", basis: f.basis, kind: "operation", tool: "segment_length", gestureId: 26, viewport, selection, options });
  assert.equal(started.operation.completed, true);
  assert.equal(started.operation.authoring_options.dimension_mode, "reference");
  assert.deepEqual(started.operation, local.initialFrame);
  const finished = await f.api.request(f.alice, { action: "finish", ticket: started.ticket });
  assert.equal(finished.kind, "operation");
  assert.deepEqual(finished.command, local.finish());
  sameAuthority(f.native, before);
});

test("server preview refuses stale, forged, viewer and cross-session input before native work", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native);
  await assert.rejects(f.api.request(f.alice, { ...f.begin, basis: { ...f.basis, revision: 1 } }), error => error.code === "preview_stale_basis");
  await assert.rejects(f.api.request(f.alice, { ...f.begin, model: f.model }), error => error.code === "invalid_preview");
  await assert.rejects(f.api.request(f.alice, { action: "navigation", viewport }), error => error.code === "invalid_preview");
  await assert.rejects(f.api.request(f.alice, { ...f.begin, viewport: { ...viewport, pixels_per_model_unit: NaN } }), error => error.code === "invalid_preview");
  const viewer = connection("viewer", "viewer"); f.active.add(viewer.sessionId);
  const captures = f.captures();
  await assert.rejects(f.api.request(viewer, f.begin), error => error.code === "preview_forbidden"); assert.equal(f.captures(), captures);
  const disabled = f.service({ enabled: false });
  await assert.rejects(disabled.request(f.alice, f.begin), error => error.code === "preview_disabled");
  const start = await f.api.request(f.alice, f.begin), bob = connection("bob"); f.active.add(bob.sessionId);
  await assert.rejects(f.api.request(bob, { action: "cancel", ticket: start.ticket }), error => error.code === "preview_missing");
  assert.equal(f.api.stats().previews, 1);
  const forged = structuredClone(f.begin); forged.target.address.owner.generation++;
  await f.api.request(f.alice, { action: "cancel", ticket: start.ticket });
  await assert.rejects(f.api.request(f.alice, forged), error => error.code === "preview_rejected");
  assert.equal(f.api.stats().previews, 0); sameAuthority(f.native, before);
});

test("preview rejects unordered batches and cannot retain exhausted, cancelled or disconnected work", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native);
  const pending = f.api.request(f.alice, f.begin);
  await assert.rejects(f.api.request(f.alice, f.begin), error => error.code === "preview_backpressure");
  const start = await pending;
  const advance = f.api.request(f.alice, { action: "advance", ticket: start.ticket, samples: [{ sequence: 1, position: [3, 2] }] });
  await assert.rejects(f.api.request(f.alice, { action: "finish", ticket: start.ticket }), error => error.code === "preview_backpressure");
  await advance;
  await assert.rejects(f.api.request(f.alice, { action: "advance", ticket: start.ticket, samples: [{ sequence: 1, position: [6, 4] }] }), error => error.code === "preview_rejected");
  assert.equal(f.api.stats().previews, 0);
  const disconnected = await f.api.request(f.alice, f.begin);
  f.active.delete(f.alice.sessionId);
  await assert.rejects(f.api.request(f.alice, { action: "finish", ticket: disconnected.ticket }), /authenticated/u);
  await f.api.dropConnection(f.alice); assert.equal(f.api.stats().previews, 0);
  f.active.add(f.alice.sessionId);
  const controller = new AbortController(), aborted = f.api.request(f.alice, f.begin, { signal: controller.signal }); controller.abort();
  await assert.rejects(aborted, error => error.code === "preview_cancelled");
  assert.equal(f.api.stats().previews, 0); assert.equal(f.api.stats().retainedBasisBytes, 0); sameAuthority(f.native, before);
});

test("native preview workers enforce time, idle and byte bounds and drain on close", async t => {
  const f = await fixture(t), before = frozenAuthority(f.native);
  const timeout = f.service({ limits: { timeoutMs: 1 } });
  await assert.rejects(timeout.request(f.alice, f.begin), error => error.code === "preview_timeout");
  assert.equal(timeout.stats().previews, 0);
  const bytes = f.service({ limits: { maxBasisBytes: 1 } });
  await assert.rejects(bytes.request(f.alice, f.begin), error => error.code === "preview_limit");
  assert.equal(bytes.stats().previews, 0);
  const response = f.service({ limits: { maxResponseBytes: 1 } });
  await assert.rejects(response.request(f.alice, f.begin), error => error.code === "preview_rejected");
  assert.equal(response.stats().previews, 0);
  const idle = f.service({ limits: { idleMs: 15 } }), held = await idle.request(f.alice, f.begin);
  await delay(60);
  await assert.rejects(idle.request(f.alice, { action: "finish", ticket: held.ticket }), error => error.code === "preview_missing");
  await idle.close(); assert.equal(idle.stats().retainedBasisBytes, 0);
  const pending = f.api.request(f.alice, f.begin); void pending.catch(() => {});
  await f.api.close(); await assert.rejects(pending, error => error.code === "preview_cancelled");
  assert.equal(f.api.stats().previews, 0); sameAuthority(f.native, before);
});


test("preview reauthenticates an editor after native work and reserves document-wide capacity", async t => {
  const f = await fixture(t, { limits: { maxPreviews: 1 } }), before = frozenAuthority(f.native);
  const pending = f.api.request(f.alice, f.begin); void pending.catch(() => {});
  const bob = connection("bob"); f.active.add(bob.sessionId);
  await assert.rejects(f.api.request(bob, f.begin), error => error.code === "preview_backpressure");
  f.active.delete(f.alice.sessionId);
  await assert.rejects(pending, /authenticated/u);
  assert.equal(f.api.stats().previews, 0);
  const bobPreview = await f.api.request(bob, f.begin);
  assert.equal(bobPreview.kind, "preview");
  await f.api.dropConnection(bob); assert.equal(f.api.stats().retainedBasisBytes, 0);
  sameAuthority(f.native, before);
});
