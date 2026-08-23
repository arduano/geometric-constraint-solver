// SPDX-License-Identifier: GPL-3.0-or-later

import {
  aliasPort,
  createNode,
  draft,
  input,
  leaf,
  nodePort,
  patch,
  session,
  sessionIdentity,
  setInstanceLeaf,
  stableNode,
  stablePort,
} from "../src/index.js";

const first = session("11111111111111111111111111111111");
const second = session("22222222222222222222222222222222");
const component = {
  revision: "0000000000000000",
  digest: "0000000000000000000000000000000000000000000000000000000000000000",
};
const firstIdentity = sessionIdentity(first, {
  session: first.id,
  revision: component.revision,
  digest: component.digest,
  graph: component,
  instance: component,
  reservations: component,
  organization: component,
  external_inputs: component,
});
const secondIdentity = sessionIdentity(second, {
  session: second.id,
  revision: component.revision,
  digest: component.digest,
  graph: component,
  instance: component,
  reservations: component,
  organization: component,
  external_inputs: component,
});

const point = stablePort(first, stableNode(first, "0000000000000001"), "0000000000000011", "point");
const scalar = stablePort(first, stableNode(first, "0000000000000002"), "0000000000000012", "scalar");
const curve = stablePort(first, stableNode(first, "0000000000000003"), "0000000000000013", "curve");
const foreignPoint = stablePort(second, stableNode(second, "0000000000000001"), "0000000000000011", "point");

input(first, "point", 0, point);
input(first, "curve", 0, curve);
leaf(point, "x");
leaf(scalar, "angle");

// @ts-expect-error A curve cannot occupy a point input.
input(first, "point", 0, curve);
// @ts-expect-error Branded references from another session cannot be mixed.
input(first, "point", 0, foreignPoint);
// @ts-expect-error A point has no scalar value leaf.
leaf(point, "value");
// @ts-expect-error A scalar has no Cartesian leaf.
leaf(scalar, "x");

const start = aliasPort(first, "start", nodePort("primary"), "point");
const segmentDraft = draft(first, "segment.main", {
  family: "geometry",
  recipe: "segment",
}, {
  inputs: [input(first, "point", 0, start)],
});
const operation = createNode(first, "segment", segmentDraft);
patch(first, firstIdentity, "require_accepted", [operation]);

const foreignDraft = draft(second, "point.foreign", {
  family: "geometry",
  recipe: "sketch_point",
});
const foreignOperation = createNode(second, "foreign", foreignDraft);

// @ts-expect-error Patch operations are branded to one exact session namespace.
patch(first, firstIdentity, "require_accepted", [foreignOperation]);
// @ts-expect-error A full identity from another session cannot stamp the patch.
patch(first, secondIdentity, "require_accepted", [operation]);

// @ts-expect-error Instance leaves are numerical quantity literals on the Rust wire.
setInstanceLeaf(first, leaf(point, "x"), { kind: "enum", value: "invalid" });
