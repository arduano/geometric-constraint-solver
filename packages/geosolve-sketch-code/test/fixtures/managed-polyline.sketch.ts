// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const geometry1 = $.geometry.polyline("geometry1", {
    vertices: [
      { key: "v0", position: [0, 0] },
      { key: "v1", position: [20, 0] },
      { key: "v2", position: [20, 10] },
    ],
    closed: false,
    role: "profile",
  });
  const constraint2 = $.constraint.horizontal("constraint2", {
    span: geometry1.segments.byKey.v0,
  });
  const constraint3 = $.constraint.vertical("constraint3", {
    span: geometry1.segments.byKey.v1,
  });
  $.group("Canvas additions", [geometry1, constraint2, constraint3]);
  return {};
});
