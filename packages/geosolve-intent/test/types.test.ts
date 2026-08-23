// SPDX-License-Identifier: GPL-3.0-or-later

import {
  aliasPort,
  createNode,
  draft,
  input,
  patch,
  session,
  stablePort,
} from "../src/index.js";

const first = session("session-first");
const second = session("session-second");
const point = stablePort(first, "node-1", "port-1", "point");
const curve = stablePort(first, "node-2", "port-2", "curve");
const foreignPoint = stablePort(second, "node-1", "port-1", "point");

input(first, "point", 0, point);
input(first, "curve", 0, curve);

// @ts-expect-error A curve cannot occupy a point input.
input(first, "point", 0, curve);
// @ts-expect-error Branded references from another session cannot be mixed.
input(first, "point", 0, foreignPoint);

const start = aliasPort(first, "start", "node:primary:0000", "point");
const pointDraft = draft(first, "point.start", {
  family: "geometry",
  recipe: "sketch_point",
});
const segmentDraft = draft(first, "segment.main", {
  family: "geometry",
  recipe: "segment",
}, {
  inputs: [input(first, "point", 0, start)],
});
const operation = createNode(first, "segment", segmentDraft);
patch(first, "identity-1", "require_accepted", [operation]);

const foreignDraft = draft(second, "point.foreign", {
  family: "geometry",
  recipe: "sketch_point",
});
const foreignOperation = createNode(second, "foreign", foreignDraft);

// @ts-expect-error Patch operations are branded to one exact session.
patch(first, "identity-1", "require_accepted", [foreignOperation]);

void pointDraft;
