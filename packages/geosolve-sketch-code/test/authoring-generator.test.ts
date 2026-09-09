// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import { defineGenerator, mm, sketch } from "../src/index.js";

test("generator inputs infer ordinary values, fill defaults and execute dynamic structure", () => {
  let executions = 0;
  const definition = defineGenerator({
    count: { type: "integer", default: 2, min: 1, max: 5, label: "Columns" },
    spacing: { type: "number", default: 10, min: 0.1, unit: "mm" },
    holes: { type: "boolean", default: true },
    name: { type: "string", default: "Panel" },
    shape: { type: "choice", default: "circle", choices: ["circle", "point"] },
  }, (parameters) => {
    executions += 1;
    const shape: "circle" | "point" = parameters.shape;
    const addHole = (builder: Parameters<Parameters<typeof sketch>[1]>[0], index: number) =>
      shape === "circle"
        ? builder.geometry.centerRadiusCircle(`hole-${index}`, { center: [index * parameters.spacing, 0], radius: mm(1) })
        : builder.geometry.sketchPoint(`hole-${index}`, { point: [index * parameters.spacing, 0] });
    return sketch({ title: parameters.name }, ($) => {
      const outputs = [];
      if (parameters.holes) for (let index = 0; index < parameters.count; index += 1) outputs.push(addHole($, index));
      return outputs;
    });
  });
  assert.deepEqual(definition.parseInputs(), { count: 2, spacing: 10, holes: true, name: "Panel", shape: "circle" });
  assert.equal(executions, 0);
  assert.equal(definition().output.length, 2);
  assert.equal(definition({ count: 4, shape: "point" }).output.length, 4);
  assert.equal(definition({ holes: false }).output.length, 0);
  assert.equal(executions, 3);
  assert.ok(Object.isFrozen(definition.inputs));
  assert.ok(Object.isFrozen(definition.inputs.shape.choices));
  assert.throws(() => definition({ count: 1.5 }), /safe integer/u);
  assert.throws(() => definition({ count: 6 }), /at most 5/u);
  assert.throws(() => definition({ spacing: Infinity }), /finite number/u);
  assert.throws(() => definition.parseInputs({ count: 0 }), /at least 1/u);
  assert.throws(() => definition.parseInputs({ shape: "square" } as never), /declared choices/u);
  assert.throws(() => definition.parseInputs({ missing: 3 } as never), /unknown generator input/u);
  assert.throws(() => definition.parseInputs({ name: undefined } as never), /must be a string/u);
  assert.equal(executions, 3, "invalid inputs never execute generator code");
});

test("generator schemas reject invalid defaults and do not reuse overview metadata", () => {
  const build = () => sketch(() => null);
  assert.throws(() => defineGenerator({ n: { type: "number", default: NaN } }, build), /finite/u);
  assert.throws(() => defineGenerator({ n: { type: "number", default: 3, min: 5, max: 2 } }, build), /minimum exceeds maximum/u);
  assert.throws(() => defineGenerator({ choice: { type: "choice", default: "a", choices: ["a", "a"] } }, build), /distinct string choices/u);
  assert.throws(() => defineGenerator({ choice: { type: "choice", default: "b", choices: ["a"] } }, build), /declared choices/u);
  assert.throws(() => defineGenerator({ n: { type: "number", default: 3, isKeyParameter: true } } as never, build), /unknown generator input field/u);
  assert.throws(() => defineGenerator({ n: { type: "number", default: 3, unit: "foot" } } as never, build), /unsupported generator input unit/u);
  assert.throws(() => defineGenerator({ n: { type: "boolean", default: 1 } } as never, build), /must be boolean/u);
  assert.throws(() => defineGenerator({ n: { type: "object", default: {} } } as never, build), /unsupported type/u);
});
