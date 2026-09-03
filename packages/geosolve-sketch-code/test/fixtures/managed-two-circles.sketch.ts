// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const geometry1 = $.geometry.centerRadiusCircle("geometry1", {
    center: [-3.599999564034599, 1.7571432931082598],
    label: "center-radius-circle-0000000000000001",
    radius: mm(1.7999986921037952),
    role: "profile",
  });
  const geometry2 = $.geometry.centerRadiusCircle("geometry2", {
    center: [2.999999999999998, 1.7571432931082598],
    label: "center-radius-circle-0000000000000002",
    radius: mm(1.8000008719308043),
    role: "profile",
  });
  $.group("Canvas additions", [geometry1, geometry2]);
  return {};
});
