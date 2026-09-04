"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  // The top shelf keeps the analytic families spatially separated. These are
  // deliberately free specimens: their source literals remain easy to edit
  // while the contact studies below demonstrate solver-owned relationships.
  const datumLine = $.geometry.segment("datumLine", {
    start: [-36, 18],
    end: [-26, 18],
    branchDirection: [1, 0],
    label: "Finite line span",
    role: "construction",
  });
  const referenceCircle = $.geometry.centerRadiusCircle("referenceCircle", {
    center: [-19, 18],
    radius: mm(3),
    label: "Full circle",
    role: "profile",
  });
  const circularArc = $.geometry.centerArc("circularArc", {
    center: [-9, 18],
    start: [-6, 18],
    end: [-9, 21],
    sweep: "counterClockwise",
    label: "Counter-clockwise circular arc",
    role: "profile",
  });
  const referenceEllipse = $.geometry.centerAxesEllipse("referenceEllipse", {
    center: [2, 18],
    majorAxisPoint: [6, 18],
    minorAxisPoint: [2, 20],
    label: "Major/minor-axis ellipse",
    role: "profile",
  });
  const ellipticalArc = $.geometry.centerAxesEllipticalArc("ellipticalArc", {
    center: [14, 18],
    majorAxisPoint: [18, 18],
    minorAxisPoint: [14, 20],
    start: [18, 18],
    end: [14, 20],
    sweep: "counterClockwise",
    label: "Counter-clockwise elliptical arc",
    role: "profile",
  });
  const rationalConic = $.geometry.rationalQuadraticConic("rationalConic", {
    start: [24, 16],
    end: [32, 16],
    weightedMiddle: [21, 16.5],
    middleWeight: 0.75,
    label: "Rational quadratic conic",
    role: "profile",
  });
  const parabola = $.geometry.parabola("parabola", {
    vertex: [40, 16],
    focus: [40, 17],
    trimStart: -2,
    trimEnd: 2,
    label: "Trimmed parabola",
    role: "profile",
  });
  const hyperbola = $.geometry.hyperbola("hyperbola", {
    center: [52, 18],
    transverseAxisPoint: [55, 18],
    semiConjugate: mm(2),
    trimStart: -1,
    trimEnd: 1,
    branch: "positive",
    label: "Positive hyperbola branch",
    role: "profile",
  });
  // The middle shelf compares polynomial, B-spline and rational spline
  // control structures without hiding their editable source coordinates.
  const quadraticWave = $.geometry.quadraticBezier("quadraticWave", {
    start: [-36, 7],
    control: [-31, 13],
    end: [-26, 7],
    label: "Quadratic Bezier",
    role: "profile",
  });
  const cubicWave = $.geometry.cubicBezier("cubicWave", {
    start: [-21, 7],
    firstControl: [-18, 13],
    secondControl: [-14, 1],
    end: [-11, 7],
    label: "Cubic Bezier",
    role: "profile",
  });
  const openSpline = $.geometry.openControlBSpline("openSpline", {
    controls: [{
      key: "start",
      position: [-6, 7],
    }, {
      key: "crest",
      position: [-3, 12],
    }, {
      key: "trough",
      position: [1, 2],
    }, {
      key: "end",
      position: [5, 7],
    }],
    degree: 3,
    label: "Open cubic B-spline",
    role: "profile",
  });
  const periodicNurbs = $.geometry.periodicControlNurbs("periodicNurbs", {
    controls: [{
      key: "west",
      position: [10, 7],
      weight: 1,
    }, {
      key: "north",
      position: [15, 12],
      weight: 1.4,
    }, {
      key: "east",
      position: [20, 7],
      weight: 1,
    }, {
      key: "south",
      position: [15, 2],
      weight: 0.8,
    }],
    degree: 2,
    gauge: "north",
    label: "Periodic quadratic NURBS",
    role: "profile",
  });
  // One anchor removes rigid drift from this specimen. The contact carries
  // explicit parameter, winding, endpoint-neighbourhood and orientation state.
  const tangentRail = $.geometry.segment("tangentRail", {
    start: [27, 3],
    end: [39, 3],
    branchDirection: [1, 0],
    label: "Tangent rail",
    role: "construction",
  });
  const rollingCircle = $.geometry.centerRadiusCircle("rollingCircle", {
    center: [33, 6],
    radius: mm(3),
    label: "Rolling contact circle",
    role: "profile",
  });
  const railAnchor = $.constraint.fixedPoint("railAnchor", {
    point: tangentRail.start,
    target: [27, 3],
    label: "Rail datum anchor",
  });
  const railDirection = $.constraint.horizontal("railDirection", {
    span: tangentRail.span,
    label: "Rail horizontal",
  });
  const railLength = $.dimension.curveLength("railLength", {
    curve: tangentRail.span,
    value: mm(12),
    mode: "driving",
    label: "Rail length · 12 mm",
  });
  const rollerRadius = $.dimension.radius("rollerRadius", {
    curve: rollingCircle.curve,
    value: mm(3),
    mode: "driving",
    label: "Roller radius · 3 mm",
  });
  const rollingContact = $.constraint.lineCircleTangency("rollingContact", {
    line: tangentRail.span,
    circle: rollingCircle.curve,
    contacts: {
      first: {
        parameter: 0.5,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 4.71238898038469,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
    },
    side: "left",
    label: "Explicit lower-arc contact branch",
  });
  const contactWitness = $.geometry.sketchPoint("contactWitness", {
    point: [33, 9],
    label: "Upper-arc point witness",
  });
  const witnessOnCircle = $.constraint.pointOnCurve("witnessOnCircle", {
    point: contactWitness.point,
    curve: rollingCircle.span,
    contact: {
      parameter: 1.5707963267948966,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "Explicit upper-arc point contact",
  });
  // These cubics are an exact parametric-C2 continuation. Both endpoint
  // contacts are explicit, including their local endpoint neighbourhoods.
  const c2Incoming = $.geometry.cubicBezier("c2Incoming", {
    start: [-16, -8],
    firstControl: [-12, -4],
    secondControl: [-8, -4],
    end: [-4, -8],
    label: "Incoming cubic",
    role: "profile",
  });
  const c2Outgoing = $.geometry.cubicBezier("c2Outgoing", {
    start: [-4, -8],
    firstControl: [0, -12],
    secondControl: [4, -20],
    end: [9, -12],
    label: "Outgoing cubic",
    role: "profile",
  });
  const parametricC2Seam = $.constraint.endpointContinuity("parametricC2Seam", {
    first: c2Incoming.span,
    second: c2Outgoing.span,
    continuity: "parametricC2",
    firstRate: 1,
    secondRate: 1,
    contacts: {
      first: {
        parameter: 1,
        winding: 0,
        neighborhood: {
          kind: "end",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 0,
        winding: 0,
        neighborhood: {
          kind: "start",
        },
        orientation: "aligned",
      },
    },
    label: "Parametric C2 seam",
  });
  $.group("Analytic curve vocabulary", [datumLine, referenceCircle, circularArc, referenceEllipse, ellipticalArc, rationalConic, parabola, hyperbola]);
  $.group("Spline curve vocabulary", [quadraticWave, cubicWave, openSpline, periodicNurbs]);
  $.group("Explicit rolling contact", [tangentRail, rollingCircle, railAnchor, railDirection, railLength, rollerRadius, rollingContact, contactWitness, witnessOnCircle]);
  $.group("Parametric C2 continuity", [c2Incoming, c2Outgoing, parametricC2Seam]);
  return {
    analytic: {
      line: datumLine,
      circle: referenceCircle,
      arc: circularArc,
      ellipse: referenceEllipse,
      ellipticalArc: ellipticalArc,
      conic: rationalConic,
      parabola: parabola,
      hyperbola: hyperbola,
    },
    splines: {
      quadratic: quadraticWave,
      cubic: cubicWave,
      open: openSpline,
      periodic: periodicNurbs,
    },
    contact: rollingContact,
    continuity: parametricC2Seam,
  };
});
