// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import { definePatch, deg, mm, sketch, t } from "../src/authoring.js";

test("ordinary callback output retains named references and exact keyed children", () => {
  const authored = sketch((s) => {
    const path = s.geometry.polyline("path", {
      vertices: [
        { key: "start", position: [0, 0] },
        { key: "corner", position: [2, 0] },
        { key: "end", position: [2, 2] },
      ],
    });
    const bezier = s.geometry.quadraticBezier("bezier", {
      start: path.vertices.byKey.start,
      control: path.vertices.byKey.corner,
      end: path.vertices.byKey.end,
    });
    return { path, bezier };
  });

  assert.deepEqual(authored.output.path.vertices.keys, ["start", "corner", "end"]);
  assert.deepEqual(authored.output.path.segments.keys, ["start", "corner"]);
  assert.ok(authored.output.bezier.start);
  assert.ok(authored.output.bezier.control);
  assert.ok(authored.output.bezier.end);
});

test("units and mandatory declaration identity reject invalid runtime values", () => {
  assert.deepEqual(mm(4), { unit: "mm", value: 4 });
  assert.throws(() => mm(Number.NaN), /must be finite/u);
  assert.throws(() => sketch((s) => s.geometry.sketchPoint("", { point: [0, 0] })),
    /invalid declaration ID/u);
  assert.throws(() => sketch((s) => {
    s.geometry.sketchPoint("same", { point: [0, 0] });
    return s.constraint.coincidentWithOrigin("same", {
      point: s.geometry.sketchPoint("other", { point: [1, 0] }).point,
    });
  }), /duplicate declaration ID/u);
});

test("runtime authoring rejects unknown, retired, and host-gated methods", () => {
  type UnsafeMethod = (
    id: string,
    values: Readonly<Record<string, unknown>>,
  ) => unknown;
  const call = (
    namespace: object,
    method: string,
    id: string,
    values: Readonly<Record<string, unknown>>,
  ) => (namespace as Readonly<Record<string, UnsafeMethod>>)[method]!(id, values);

  assert.throws(
    () => sketch((s) => call(s.geometry, "bogus", "bad", {})),
    /unsupported authoring method geometry\.bogus/u,
  );
  assert.throws(
    () => sketch((s) => call(s.geometry, "line", "legacy", {
      start: [0, 0],
      end: [1, 0],
    })),
    /unsupported authoring method geometry\.line/u,
  );
  assert.throws(
    () => sketch((s) => call(s.constraint, "externalPointCoincident", "external", {})),
    /requires immutable host-snapshot authority/u,
  );
  assert.throws(
    () => sketch((s) => call(s.computed, "fillet", "hostOnly", {})),
    /unsupported authoring method computed\.fillet/u,
  );
});

test("B-spline and NURBS methods enforce distinct control and gauge contracts", () => {
  assert.throws(() => sketch((s) => s.geometry.openControlNurbs("badGauge", {
    controls: [
      { key: "a", position: [0, 0], weight: 1 },
      { key: "b", position: [1, 0], weight: 1 },
      { key: "c", position: [2, 0], weight: 1 },
    ],
    degree: 2,
    gauge: "missing" as "a" | "b" | "c",
  })), /gauge must name a control key/u);

  assert.throws(() => sketch((s) => s.geometry.openControlNurbs("badWeight", {
    controls: [
      { key: "a", position: [0, 0], weight: 1 },
      { key: "b", position: [1, 0], weight: 0 },
      { key: "c", position: [2, 0], weight: 1 },
    ],
    degree: 2,
    gauge: "a",
  })), /weight must be positive/u);

  const bspline = sketch((s) => s.geometry.openControlBSpline("bspline", {
    controls: [
      { key: "a", position: [0, 0] },
      { key: "b", position: [1, 1] },
      { key: "c", position: [2, 0] },
    ],
    degree: 2,
  }));
  assert.ok(bspline.output.controls.byKey.b.position);
  assert.equal("weight" in bspline.output.controls.byKey.b, false);

  type UnsafeSpline = (
    id: string,
    values: Readonly<Record<string, unknown>>,
  ) => unknown;
  const callSpline = (
    method: UnsafeSpline,
    id: string,
    values: Readonly<Record<string, unknown>>,
  ) => method(id, values);
  assert.throws(
    () => sketch((s) => callSpline(
      s.geometry.openControlBSpline as unknown as UnsafeSpline,
      "invalidDegreeBSpline",
      {
        controls: [
          { key: "a", position: [0, 0] },
          { key: "b", position: [1, 1] },
        ],
        degree: 0,
      },
    )),
    /spline degree must be a positive integer/u,
  );
  assert.throws(
    () => sketch((s) => callSpline(
      s.geometry.openControlBSpline as unknown as UnsafeSpline,
      "weightedBSpline",
      {
        controls: [
          { key: "a", position: [0, 0], weight: 1 },
          { key: "b", position: [1, 1], weight: 1 },
          { key: "c", position: [2, 0], weight: 1 },
        ],
        degree: 2,
      },
    )),
    /B-spline controls cannot declare a weight/u,
  );
  assert.throws(
    () => sketch((s) => callSpline(
      s.geometry.openControlBSpline as unknown as UnsafeSpline,
      "gaugedBSpline",
      {
        controls: [
          { key: "a", position: [0, 0] },
          { key: "b", position: [1, 1] },
          { key: "c", position: [2, 0] },
        ],
        degree: 2,
        gauge: "a",
      },
    )),
    /B-spline cannot declare a weight gauge/u,
  );
  assert.throws(
    () => sketch((s) => callSpline(
      s.geometry.openControlNurbs as unknown as UnsafeSpline,
      "weightlessNurbs",
      {
        controls: [
          { key: "a", position: [0, 0] },
          { key: "b", position: [1, 1] },
          { key: "c", position: [2, 0] },
        ],
        degree: 2,
        gauge: "a",
      },
    )),
    /NURBS controls require a weight/u,
  );

  const edge = definePatch(
    { start: t.point(), end: t.point() },
    (p, { start, end }) => ({
      segment: p.geometry.segment("segment", { start, end }),
    }),
  );
  const authored = sketch((s) => {
    const start = s.geometry.sketchPoint("start", { point: [0, 0] });
    const end = s.geometry.sketchPoint("end", { point: [10, 0] });
    const first = s.use("firstEdge", edge, { start: start.point, end: end.point });
    const second = s.use("secondEdge", edge, { start: end.point, end: start.point });
    return { first, second };
  });
  assert.ok(authored.output.first.segment.span);
  assert.ok(authored.output.second.segment.span);
});

test("lossless conic inputs and scalar-radius circles retain their typed runtime outputs", () => {
  const authored = sketch((s) => {
    const circle = s.geometry.centerRadiusCircle("circle", {
      center: [0, 0],
      radius: mm(4),
    });
    const conic = s.geometry.rationalQuadraticConic("conic", {
      start: [0, 0],
      weightedMiddle: [2, 3],
      end: [4, 0],
      middleWeight: 0.75,
    });
    const parabola = s.geometry.parabola("parabola", {
      vertex: conic.weightedMiddle,
      focus: [2, 4],
      trimStart: -2,
      trimEnd: 3,
    });
    const hyperbola = s.geometry.hyperbola("hyperbola", {
      center: circle.center,
      transverseAxisPoint: [3, 0],
      semiConjugate: mm(2),
      trimStart: -1,
      trimEnd: 1,
      branch: "positive",
    });
    return { circle, conic, parabola, hyperbola };
  });

  assert.ok(authored.output.circle.radius);
  assert.ok(authored.output.conic.middleWeight);
  assert.ok(authored.output.parabola.trimStart);
  assert.ok(authored.output.parabola.trimEnd);
  assert.ok(authored.output.hyperbola.semiConjugate);
});

test("Rust-owned result descriptors drive nested constraint, dimension, and operation outputs", () => {
  const authored = sketch((s) => {
    const first = s.geometry.segment("first", { start: [0, 0], end: [4, 0] });
    const second = s.geometry.segment("second", { start: [4, 0], end: [4, 3] });
    const contact = s.constraint.curveCurveTangency("contact", {
      first: first.span,
      second: second.span,
    });
    const length = s.dimension.curveLength("length", {
      curve: first.span,
      value: mm(4),
    });
    const split = s.operation.split("split", {
      source: first.span,
      parameter: 0.5,
      retained: "before",
    });
    return { contact, length, split };
  });

  assert.ok(authored.output.contact.contacts.first.parameter);
  assert.ok(authored.output.contact.contacts.second.contact);
  assert.ok(authored.output.length.value);
  assert.ok(authored.output.length.dimension);
  assert.ok(authored.output.split.operation);
  assert.ok(authored.output.split.before);
  assert.ok(authored.output.split.after);
});

test("typed operation results expose native spans and topology-dependent members", () => {
  const authored = sketch((s) => {
    const first = s.geometry.segment("first", { start: [0, 0], end: [4, 0] });
    const second = s.geometry.segment("second", { start: [4, 0], end: [4, 4] });
    const axis = s.geometry.segment("axis", { start: [0, -2], end: [0, 6] });
    const curve = s.geometry.quadraticBezier("curve", {
      start: [1, 0],
      control: [2, 2],
      end: [3, 0],
    });
    const mirror = s.operation.mirror("mirror", {
      source: curve.curve,
      axis: axis.span,
    });
    const fillet = s.operation.associativeFillet("fillet", {
      radius: mm(1),
      radiusMode: "driving",
      parents: [{
        span: first.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "end",
        periodicAnchor: { kind: "none" },
      }, {
        span: second.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "start",
        periodicAnchor: { kind: "none" },
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    });
    const rectangle = s.operation.rectangle("rectangle", {
      origin: [10, 0],
      width: mm(4),
      height: mm(3),
      role: "profile",
    });
    const polygon = s.operation.regularPolygon("polygon", {
      center: [15, 10],
      radius: mm(3),
      sides: 5,
      rotation: deg(18),
      role: "profile",
    });
    const slot = s.operation.slot("slot", {
      firstCenter: [10, 20],
      secondCenter: [16, 20],
      radius: mm(2),
      role: "profile",
    });
    const pattern = s.operation.linearPattern("pattern", {
      sources: [first.curve],
      instances: 3,
      step: [0, 5],
    });
    const chain = s.aggregate.openChain("chain", { spans: [first.span] });
    const offset = s.operation.profileOffset("offset", {
      sources: [chain.chain],
      distance: mm(1),
      side: "left",
      firstTraversal: "forward",
    });
    return { mirror, fillet, rectangle, polygon, slot, pattern, offset };
  });

  assert.ok(authored.output.mirror.span);
  assert.ok(authored.output.mirror.controls["curve.control"]);
  assert.ok(authored.output.mirror.symmetryConstraints["curve.control"]);
  assert.ok(authored.output.fillet.span);
  assert.ok(authored.output.rectangle.spans.bottom);
  assert.ok(authored.output.polygon.spans[0]);
  assert.ok(authored.output.slot.spans.top);
  assert.ok(authored.output.slot.arcs.left.span);
  assert.ok(authored.output.pattern.instances[0]?.sources["first.curve"]?.span);
  assert.ok(authored.output.offset.operand.chain?.edges["first.span"]?.span);
});
