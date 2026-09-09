// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import { definePatch, deg, mm, recordedSketch, sketch, t } from "../src/index.js";
import type { SketchBuilder, SketchExecutionProject } from "../src/index.js";

test("recorded sketch preserves executed order, semantic IDs, values and source-owned presentation", () => {
  const options = { title: "Fixture", dimensions: { areKeyConstraintsByDefault: true } };
  const authored = sketch(options, ($) => {
    const holeCount = $.parameter("holeCount", 2, { label: "Hole count" });
    const radius = $.parameter("radius", mm(3), { isKeyParameter: true });
    const anotherRadius = $.parameter("anotherRadius", mm(3));
    const circles = [];
    for (let index = 0; index < holeCount; index += 1) {
      const circle = $.geometry.centerRadiusCircle(`bore-${index}`, { center: [index * 20, 0], radius: index === 0 ? radius : anotherRadius });
      circles.push(circle);
      $.dimension.radius(`radius-${index}`, { curve: circle.curve, value: radius, label: `Bore ${index}`, isKeyConstraint: false });
    }
    $.group("Bores", circles);
    $.suppress(circles[1]!);
    return { bores: circles, scalars: { radius, anotherRadius }, count: holeCount };
  });
  options.title = "Changed later";
  const artifact = recordedSketch(authored);
  assert.equal(artifact.format, "geosolve-generated-sketch-v1");
  assert.equal(artifact.sdk_abi, "geosolve-sketch-code-v2");
  assert.deepEqual(artifact.declarations.map(({ identity, family }) => [identity, family]), [
    [["bore-0"], "geometry.centerRadiusCircle"], [["radius-0"], "dimension.radius"],
    [["bore-1"], "geometry.centerRadiusCircle"], [["radius-1"], "dimension.radius"],
  ]);
  assert.deepEqual(artifact.document, { title: "Fixture", dimensions: { areKeyConstraintsByDefault: true } });
  assert.deepEqual(artifact.parameters.map(({ id }) => id), ["holeCount", "radius", "anotherRadius"]);
  assert.deepEqual(artifact.parameters[1]?.value, { kind: "unit", value: { unit: "mm", value: 3 } });
  assert.deepEqual(artifact.groups, [{ name: "Bores", declarations: [["bore-0"], ["bore-1"]] }]);
  assert.deepEqual(artifact.suppressions, [["bore-1"]]);
  const second = artifact.declarations[1]!;
  assert.equal(second.arguments.kind, "object");
  if (second.arguments.kind !== "object") assert.fail();
  assert.deepEqual(second.arguments.value.curve, { kind: "reference", value: { identity: ["bore-0"], path: ["curve"], kind: "curve" } });
  assert.deepEqual(second.arguments.value.isKeyConstraint, { kind: "bool", value: false });
  assert.ok(Object.isFrozen(artifact));
  assert.ok(Object.isFrozen(second.arguments.value.curve));
  assert.equal("source_digest" in artifact, false);
  assert.equal("ir_digest" in artifact, false);
  assert.equal("value_consumers" in artifact, false, "equal parameter values do not invent lexical consumers");
});

test("ordinary dynamic patches retain complete evaluated member identities and invocation metadata", () => {
  const patch = definePatch({ corners: t.keyed(t.corner()), radius: t.length({ label: "Bend radius", isKeyParameter: true }) }, (p, { corners, radius }) => ({
    rounded: p.each(corners, (corner) => p.computed.fillet("bend", { corner, radius })),
    extra: p.mapRecord({ first: [0, 0] as const, second: [2, 0] as const }, (point) => p.geometry.sketchPoint("point", { point })),
  }));
  const artifact = recordedSketch(sketch(($) => {
    const path = $.geometry.polyline("path", { vertices: [{ key: "a", position: [0, 0] }, { key: "b", position: [3, 0] }, { key: "c", position: [3, 3] }] });
    const first = $.use("first", patch, { corners: path.filletableCorners, radius: mm(1) }, { label: "First bends" });
    const second = $.use("second", patch, { corners: path.filletableCorners, radius: mm(2) });
    $.group("Rounded", [first, second]);
    return { first, middle: second.rounded.byKey.b };
  }));
  assert.deepEqual(artifact.declarations.slice(1, 6).map(({ identity }) => identity), [
    ["first", "a", "bend"], ["first", "b", "bend"], ["first", "c", "bend"], ["first", "first", "point"], ["first", "second", "point"],
  ]);
  assert.deepEqual(artifact.applications[0]?.declarations, artifact.declarations.slice(1, 6).map(({ identity }) => identity));
  assert.deepEqual(artifact.applications[0]?.input_presentation, { radius: { label: "Bend radius", isKeyParameter: true } });
  assert.deepEqual(artifact.applications[0]?.presentation, { label: "First bends" });
  assert.deepEqual(artifact.groups, [{ name: "Rounded", declarations: [["first"], ["second"]] }]);
  const bend = artifact.declarations[2]!;
  if (bend.arguments.kind !== "object") assert.fail();
  assert.deepEqual(bend.arguments.value.corner, { kind: "reference", value: { identity: ["path"], path: ["filletableCorners", { member: "b" }], kind: "feature_corner" } });
});

test("recording snapshots arguments and rejects foreign, copied, cyclic and unsupported values", () => {
  const values = { start: [0, 0] as [number, number], end: [4, 0] as [number, number] };
  const artifact = recordedSketch(sketch(($) => {
    const edge = $.geometry.segment("edge", values);
    values.end[0] = 20;
    return { edge, boolean: true, empty: null, text: "export" };
  }));
  assert.equal(JSON.stringify(artifact.declarations).includes('"value":4'), true);
  assert.equal(JSON.stringify(artifact.declarations).includes('"value":20'), false);
  const foreign = sketch(($) => $.geometry.sketchPoint("point", { point: [0, 0] })).output.point;
  assert.throws(() => sketch(($) => $.geometry.segment("line", { start: foreign, end: [3, 0] })), /another sketch/u);
  const copied = Object.create(null);
  for (const key of Reflect.ownKeys(foreign)) Object.defineProperty(copied, key, Object.getOwnPropertyDescriptor(foreign, key)!);
  assert.throws(() => sketch(() => copied), /ordinary enumerable string-keyed fields/u);
  const cyclic: Record<string, unknown> = {}; cyclic.self = cyclic;
  assert.throws(() => sketch(() => cyclic), /cyclic/u);
  assert.throws(() => sketch(() => Infinity), /finite/u);
  assert.throws(() => sketch(() => ({ bad: undefined })), /undefined/u);
  assert.throws(() => sketch(() => new Date()), /plain data/u);
  assert.throws(() => sketch(() => [, 1]), /dense/u);
  assert.throws(() => sketch(() => ({ get value(): never { return assert.fail("getter must not execute"); } })), /ordinary enumerable/u);
  assert.throws(() => recordedSketch({ output: {} } as never), /created by this GeoSolve SDK/u);
  let retained: SketchBuilder<SketchExecutionProject> | undefined;
  sketch(($) => { retained = $; });
  assert.throws(() => retained!.geometry.sketchPoint("late", { point: [0, 0] }), /synchronous callback/u);
  assert.throws(() => sketch(() => Promise.resolve(3)), /promises/u);
});

test("generated channels retain the native patch-only declaration and complete input references", () => {
  const channel = definePatch({ polyline: t.feature("polyline"), width: t.length(), bendRadius: t.length() }, (p, values) => ({ channel: p.computed.polylineChannel("walls", { ...values, caps: "both" }) }));
  const artifact = recordedSketch(sketch(($) => {
    const path = $.geometry.polyline("path", { vertices: [{ key: "a", position: [0, 0] }, { key: "b", position: [40, 0] }, { key: "c", position: [40, 40] }] });
    return $.use("water", channel, { polyline: path, width: mm(12), bendRadius: mm(12) });
  }));
  assert.deepEqual(artifact.declarations[1]?.identity, ["water", "walls"]);
  assert.equal(artifact.declarations[1]?.family, "computed.polylineChannel");
  assert.deepEqual(artifact.applications[0]?.declarations, [["water", "walls"]]);
});

test("caught patch failures remove partial recording and permit a corrected invocation", () => {
  let fail = true;
  const patch = definePatch({}, (p) => {
    const point = p.geometry.sketchPoint("point", { point: [0, 0] });
    if (fail) throw new Error("bad generator branch");
    return point;
  });
  const artifact = recordedSketch(sketch(($) => {
    assert.throws(() => $.use("patch", patch, {}), /bad generator branch/u);
    fail = false;
    return $.use("patch", patch, {});
  }));
  assert.equal(artifact.declarations.length, 1);
  assert.equal(artifact.applications.length, 1);
  assert.deepEqual(artifact.declarations[0]?.identity, ["patch", "point"]);
  assert.throws(() => sketch(($) => $.use("patch", patch, { unexpected: 2 } as never)), /exactly match/u);
});

test("runtime patch schema validates units and authentic input kinds even when unused", () => {
  const length = definePatch({ width: t.length() }, () => ({ ok: true }));
  assert.throws(() => sketch(($) => $.use("length", length, { width: deg(5) } as never)), /length schema/u);
  const curve = definePatch({ curve: t.curve() }, () => ({ ok: true }));
  assert.throws(() => sketch(($) => {
    const point = $.geometry.sketchPoint("point", { point: [0, 0] });
    return $.use("curve", curve, { curve: point.point } as never);
  }), /curve schema/u);
  const polyline = definePatch({ path: t.feature("polyline") }, () => ({ ok: true }));
  assert.throws(() => sketch(($) => {
    const circle = $.geometry.centerRadiusCircle("circle", { center: [0, 0], radius: mm(1) });
    return $.use("polyline", polyline, { path: circle } as never);
  }), /feature schema/u);
});
