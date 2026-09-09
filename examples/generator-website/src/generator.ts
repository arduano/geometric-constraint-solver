// SPDX-License-Identifier: GPL-3.0-or-later
import { defineGenerator, definePatch, mm, sketch, t, type GeneratorInputValues } from "@geosolve/sketch-code";

const corners = definePatch({ corners: t.keyed(t.corner()), radius: t.length() }, (p, inputs) => ({
  rounded: p.each(inputs.corners, (corner) => p.computed.fillet("corner", { corner, radius: inputs.radius })),
}));

/** An ordinary TS generator: loops, branches and helpers own the design structure. */
export const footprint = defineGenerator({
  columns: { type: "integer", default: 3, min: 1, max: 3, label: "Columns", description: "42 mm cells across the footprint." },
  rows: { type: "integer", default: 2, min: 1, max: 3, label: "Rows", description: "42 mm cells from front to back." },
  clearance: { type: "number", default: 0.5, min: 0, max: 1.5, unit: "mm", label: "Total clearance", description: "Total amount removed from each outside dimension." },
  mounting: { type: "choice", default: "magnets", choices: ["magnets", "screws", "none"], label: "Mounting", description: "Four bores on a 26 mm square in each cell: 6 mm magnets or 3 mm screws." },
  rounded: { type: "boolean", default: true, label: "Rounded corners", description: "A 3.75 mm radius on the outside footprint." },
}, (inputs) => {
  const pitch = 42;
  const width = inputs.columns * pitch - inputs.clearance;
  const height = inputs.rows * pitch - inputs.clearance;
  return sketch({ title: "Modular storage footprint", description: "A Gridfinity-style 2D mounting footprint. Dimensions and bores are generated from ordinary TypeScript." }, ($) => {
    const outline = $.geometry.polyline("outline", { closed: true, vertices: [
      { key: "bottomLeft", position: [-width / 2, -height / 2] },
      { key: "bottomRight", position: [width / 2, -height / 2] },
      { key: "topRight", position: [width / 2, height / 2] },
      { key: "topLeft", position: [-width / 2, height / 2] },
    ] });
    const widthDimension = $.dimension.curveLength("width", { curve: outline.segments.byKey.bottomLeft, value: mm(width), label: "Overall width", isKeyConstraint: true });
    const depthDimension = $.dimension.curveLength("depth", { curve: outline.segments.byKey.bottomRight, value: mm(height), label: "Overall depth", isKeyConstraint: true });
    if (inputs.rounded) $.use("outlineCorners", corners, { corners: outline.filletableCorners, radius: mm(3.75) });
    const bores = [];
    if (inputs.mounting !== "none") {
      const radius = inputs.mounting === "magnets" ? 3 : 1.5;
      for (let row = 0; row < inputs.rows; row++) {
        for (let column = 0; column < inputs.columns; column++) {
          const center = [(column - (inputs.columns - 1) / 2) * pitch, (row - (inputs.rows - 1) / 2) * pitch];
          for (const [index, offset] of [[-13, -13], [13, -13], [13, 13], [-13, 13]].entries()) {
            bores.push($.geometry.centerRadiusCircle(`bore-${column}-${row}-${index}`, {
              center: [center[0]! + offset[0]!, center[1]! + offset[1]!], radius: mm(radius),
            }));
          }
        }
      }
    }
    $.group("Footprint", [outline, widthDimension, depthDimension]);
    if (bores.length) $.group("Mounting bores", bores);
    return { outline, bores };
  });
});

export type FootprintInputs = GeneratorInputValues<typeof footprint.inputs>;
export default footprint;
