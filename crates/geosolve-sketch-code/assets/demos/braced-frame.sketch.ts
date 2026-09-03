"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { crossBrace } from "./patches/cross-brace.patch.ts";

export default sketch(($) => {
  const frame = $.geometry.twoPointAlignedRectangle("frame", {
    firstCorner: [0, 0],
    oppositeCorner: [60, 35],
  });
  const brace = $.use("brace", crossBrace, {
    frame: frame,
  });
  const round = $.computed.filletSet("round", {
    radius: mm(1),
    corners: [{
      key: "braceCorner",
      parents: [{
        span: frame.spans[0],
        parameter: 0.1,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        normalSide: "left",
        trimEndpoint: "start",
        periodicAnchor: {
          kind: "none",
        },
      }, {
        span: brace.diagonals.rising.span,
        parameter: 0.1,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        normalSide: "right",
        trimEndpoint: "start",
        periodicAnchor: {
          kind: "none",
        },
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
  });
  $.suppress(round);
  // A diagonal of an axis-aligned frame cannot itself be horizontal. Keep the
  // downstream ordinary relation as an explicit, editable suppressed example
  // rather than publishing an invalid demonstration scene.
  const datum = $.constraint.horizontal("datum", {
    span: brace.diagonals.rising.span,
    suppressed: true,
  });
  $.group("Frame", [frame, round, datum]);
  return {
    frame: frame,
    brace: brace,
    rising: brace.diagonals.rising,
    round: round,
    datum: datum,
  };
});
