// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { Engine } from "../dist/index.js";
import { sketch, mm } from "@geosolve/sketch-code";
const makeCircle = ({ radius }) => sketch(($) => ({ ring: $.geometry.centerRadiusCircle("ring", { center: [0, 0], radius: mm(radius) }) }));

function transport() {
  let calls = 0;
  let free = 0;
  const released = [];
  return {
    get calls() { return calls; }, get freed() { return free; }, released,
    evaluateGenerated(json) {
      calls++;
      const recorded = JSON.parse(json);
      const radius = recorded.declarations[0].arguments.value.radius.value.value;
      if (radius < 0) throw Error("native invalid geometry");
      return JSON.stringify({ format: "geosolve-engine-result-v1", status: "accepted", result_id: String(radius),
        mode: "generator", validation: { hard_residuals_validated: true, all_active_features_current: true },
        geometry: { points: [], scalars: [{ value: radius }], curves: [], computed_edges: [] } });
    },
    evaluateManaged() { throw Error("unexpected managed request"); },
    exportProfiles(id, chord) { return JSON.stringify({ format: "geosolve-baked-profile-v1", regions: [], id, chord }); },
    releaseResult(id) { released.push(id); return true; },
    free() { free++; },
  };
}

test("wrapper retains immutable accepted result across callback errors and native rejection", async () => {
  const native = transport(); const engine = new Engine(native);
  const first = await engine.evaluate({ definition: makeCircle, parameters: { radius: 12 } });
  assert.equal(first.status, "accepted");
  assert.equal(first.geometry.scalars[0].value, 12);
  assert.ok(Object.isFrozen(first.geometry.scalars));
  assert.equal((await engine.evaluate({ definition: () => { throw Error("bad input"); }, parameters: {} })).status, "rejected");
  assert.equal((await engine.evaluate({ definition: makeCircle, parameters: { radius: -1 } })).status, "rejected");
  assert.equal(engine.lastAccepted, first);
  assert.equal(native.calls, 2);
  assert.equal((await engine.exportProfiles(first, { chordErrorMm: 0.02 })).id, "12");
  await assert.rejects(() => engine.exportProfiles({ ...first }, { chordErrorMm: 0.02 }), /another engine/);
  engine.release(first);
  await assert.rejects(() => engine.exportProfiles(first, { chordErrorMm: 0.02 }), /released/);
  engine.dispose(); engine.dispose(); assert.equal(native.freed, 1);
});

test("supersession and cancelled queues never run stale generator code", async () => {
  const native = transport(); const engine = new Engine(native);
  const old = engine.evaluate({ definition: makeCircle, parameters: { radius: 10 } });
  const latest = engine.evaluate({ definition: makeCircle, parameters: { radius: 15 } });
  assert.equal((await old).status, "cancelled");
  assert.equal((await latest).status, "accepted");
  const controller = new AbortController(); controller.abort();
  const cancelled = await engine.evaluate({ definition: () => { throw Error("must not run"); }, parameters: {}, signal: controller.signal });
  assert.equal(cancelled.status, "cancelled"); assert.equal(native.calls, 1);
  assert.equal(engine.lastAccepted.result_id, "15");
  engine.dispose();
  await assert.rejects(() => engine.evaluate({ definition: makeCircle, parameters: { radius: 1 } }), /disposed/);
});

test("equivalent retained IDs stay exportable until their last result owner releases", async () => {
  const native = transport(); const engine = new Engine(native);
  const first = await engine.evaluate({ definition: makeCircle, parameters: { radius: 10 } });
  const second = await engine.evaluate({ definition: makeCircle, parameters: { radius: 10 } });
  engine.release(first); assert.deepEqual(native.released, []);
  await engine.exportProfiles(second, { chordErrorMm: 0.1 });
  engine.release(second); assert.deepEqual(native.released, ["10"]);
  engine.dispose();
});
