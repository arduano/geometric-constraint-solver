"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { fixtureCells } from "./patches/fixture-cells.patch.ts";

export default sketch(($) => {
  // The plate and three readable serpentine lattices are the only authored
  // geometry. A keyed structural patch produces exact concentric Circles at
  // every lattice point: 192 pilots plus 192 counterbores, with no LOD.
  const fixtureEnvelope = $.geometry.twoPointAlignedRectangle("fixtureEnvelope", {
    firstCorner: [-180, -70],
    oppositeCorner: [180, 70],
    role: "profile",
    label: "360 × 140 mm fixture envelope",
  });
  const northernLattice = $.geometry.polyline("northernLattice", {
    vertices: [{
      key: "r00c00",
      position: [-172.5, 52.5],
    }, {
      key: "r00c01",
      position: [-157.5, 52.5],
    }, {
      key: "r00c02",
      position: [-142.5, 52.5],
    }, {
      key: "r00c03",
      position: [-127.5, 52.5],
    }, {
      key: "r00c04",
      position: [-112.5, 52.5],
    }, {
      key: "r00c05",
      position: [-97.5, 52.5],
    }, {
      key: "r00c06",
      position: [-82.5, 52.5],
    }, {
      key: "r00c07",
      position: [-67.5, 52.5],
    }, {
      key: "r00c08",
      position: [-52.5, 52.5],
    }, {
      key: "r00c09",
      position: [-37.5, 52.5],
    }, {
      key: "r00c10",
      position: [-22.5, 52.5],
    }, {
      key: "r00c11",
      position: [-7.5, 52.5],
    }, {
      key: "r00c12",
      position: [7.5, 52.5],
    }, {
      key: "r00c13",
      position: [22.5, 52.5],
    }, {
      key: "r00c14",
      position: [37.5, 52.5],
    }, {
      key: "r00c15",
      position: [52.5, 52.5],
    }, {
      key: "r00c16",
      position: [67.5, 52.5],
    }, {
      key: "r00c17",
      position: [82.5, 52.5],
    }, {
      key: "r00c18",
      position: [97.5, 52.5],
    }, {
      key: "r00c19",
      position: [112.5, 52.5],
    }, {
      key: "r00c20",
      position: [127.5, 52.5],
    }, {
      key: "r00c21",
      position: [142.5, 52.5],
    }, {
      key: "r00c22",
      position: [157.5, 52.5],
    }, {
      key: "r00c23",
      position: [172.5, 52.5],
    }, {
      key: "r01c23",
      position: [172.5, 37.5],
    }, {
      key: "r01c22",
      position: [157.5, 37.5],
    }, {
      key: "r01c21",
      position: [142.5, 37.5],
    }, {
      key: "r01c20",
      position: [127.5, 37.5],
    }, {
      key: "r01c19",
      position: [112.5, 37.5],
    }, {
      key: "r01c18",
      position: [97.5, 37.5],
    }, {
      key: "r01c17",
      position: [82.5, 37.5],
    }, {
      key: "r01c16",
      position: [67.5, 37.5],
    }, {
      key: "r01c15",
      position: [52.5, 37.5],
    }, {
      key: "r01c14",
      position: [37.5, 37.5],
    }, {
      key: "r01c13",
      position: [22.5, 37.5],
    }, {
      key: "r01c12",
      position: [7.5, 37.5],
    }, {
      key: "r01c11",
      position: [-7.5, 37.5],
    }, {
      key: "r01c10",
      position: [-22.5, 37.5],
    }, {
      key: "r01c09",
      position: [-37.5, 37.5],
    }, {
      key: "r01c08",
      position: [-52.5, 37.5],
    }, {
      key: "r01c07",
      position: [-67.5, 37.5],
    }, {
      key: "r01c06",
      position: [-82.5, 37.5],
    }, {
      key: "r01c05",
      position: [-97.5, 37.5],
    }, {
      key: "r01c04",
      position: [-112.5, 37.5],
    }, {
      key: "r01c03",
      position: [-127.5, 37.5],
    }, {
      key: "r01c02",
      position: [-142.5, 37.5],
    }, {
      key: "r01c01",
      position: [-157.5, 37.5],
    }, {
      key: "r01c00",
      position: [-172.5, 37.5],
    }],
    closed: false,
    role: "construction",
    label: "Northern two-row mounting lattice",
  });
  const northernCells = $.use("northernCells", fixtureCells, {
    centers: northernLattice.vertices,
    pilotRadius: mm(2.5),
    counterboreRadius: mm(4.5),
  });
  const centralLattice = $.geometry.polyline("centralLattice", {
    vertices: [{
      key: "r02c00",
      position: [-172.5, 22.5],
    }, {
      key: "r02c01",
      position: [-157.5, 22.5],
    }, {
      key: "r02c02",
      position: [-142.5, 22.5],
    }, {
      key: "r02c03",
      position: [-127.5, 22.5],
    }, {
      key: "r02c04",
      position: [-112.5, 22.5],
    }, {
      key: "r02c05",
      position: [-97.5, 22.5],
    }, {
      key: "r02c06",
      position: [-82.5, 22.5],
    }, {
      key: "r02c07",
      position: [-67.5, 22.5],
    }, {
      key: "r02c08",
      position: [-52.5, 22.5],
    }, {
      key: "r02c09",
      position: [-37.5, 22.5],
    }, {
      key: "r02c10",
      position: [-22.5, 22.5],
    }, {
      key: "r02c11",
      position: [-7.5, 22.5],
    }, {
      key: "r02c12",
      position: [7.5, 22.5],
    }, {
      key: "r02c13",
      position: [22.5, 22.5],
    }, {
      key: "r02c14",
      position: [37.5, 22.5],
    }, {
      key: "r02c15",
      position: [52.5, 22.5],
    }, {
      key: "r02c16",
      position: [67.5, 22.5],
    }, {
      key: "r02c17",
      position: [82.5, 22.5],
    }, {
      key: "r02c18",
      position: [97.5, 22.5],
    }, {
      key: "r02c19",
      position: [112.5, 22.5],
    }, {
      key: "r02c20",
      position: [127.5, 22.5],
    }, {
      key: "r02c21",
      position: [142.5, 22.5],
    }, {
      key: "r02c22",
      position: [157.5, 22.5],
    }, {
      key: "r02c23",
      position: [172.5, 22.5],
    }, {
      key: "r03c23",
      position: [172.5, 7.5],
    }, {
      key: "r03c22",
      position: [157.5, 7.5],
    }, {
      key: "r03c21",
      position: [142.5, 7.5],
    }, {
      key: "r03c20",
      position: [127.5, 7.5],
    }, {
      key: "r03c19",
      position: [112.5, 7.5],
    }, {
      key: "r03c18",
      position: [97.5, 7.5],
    }, {
      key: "r03c17",
      position: [82.5, 7.5],
    }, {
      key: "r03c16",
      position: [67.5, 7.5],
    }, {
      key: "r03c15",
      position: [52.5, 7.5],
    }, {
      key: "r03c14",
      position: [37.5, 7.5],
    }, {
      key: "r03c13",
      position: [22.5, 7.5],
    }, {
      key: "r03c12",
      position: [7.5, 7.5],
    }, {
      key: "r03c11",
      position: [-7.5, 7.5],
    }, {
      key: "r03c10",
      position: [-22.5, 7.5],
    }, {
      key: "r03c09",
      position: [-37.5, 7.5],
    }, {
      key: "r03c08",
      position: [-52.5, 7.5],
    }, {
      key: "r03c07",
      position: [-67.5, 7.5],
    }, {
      key: "r03c06",
      position: [-82.5, 7.5],
    }, {
      key: "r03c05",
      position: [-97.5, 7.5],
    }, {
      key: "r03c04",
      position: [-112.5, 7.5],
    }, {
      key: "r03c03",
      position: [-127.5, 7.5],
    }, {
      key: "r03c02",
      position: [-142.5, 7.5],
    }, {
      key: "r03c01",
      position: [-157.5, 7.5],
    }, {
      key: "r03c00",
      position: [-172.5, 7.5],
    }, {
      key: "r04c00",
      position: [-172.5, -7.5],
    }, {
      key: "r04c01",
      position: [-157.5, -7.5],
    }, {
      key: "r04c02",
      position: [-142.5, -7.5],
    }, {
      key: "r04c03",
      position: [-127.5, -7.5],
    }, {
      key: "r04c04",
      position: [-112.5, -7.5],
    }, {
      key: "r04c05",
      position: [-97.5, -7.5],
    }, {
      key: "r04c06",
      position: [-82.5, -7.5],
    }, {
      key: "r04c07",
      position: [-67.5, -7.5],
    }, {
      key: "r04c08",
      position: [-52.5, -7.5],
    }, {
      key: "r04c09",
      position: [-37.5, -7.5],
    }, {
      key: "r04c10",
      position: [-22.5, -7.5],
    }, {
      key: "r04c11",
      position: [-7.5, -7.5],
    }, {
      key: "r04c12",
      position: [7.5, -7.5],
    }, {
      key: "r04c13",
      position: [22.5, -7.5],
    }, {
      key: "r04c14",
      position: [37.5, -7.5],
    }, {
      key: "r04c15",
      position: [52.5, -7.5],
    }, {
      key: "r04c16",
      position: [67.5, -7.5],
    }, {
      key: "r04c17",
      position: [82.5, -7.5],
    }, {
      key: "r04c18",
      position: [97.5, -7.5],
    }, {
      key: "r04c19",
      position: [112.5, -7.5],
    }, {
      key: "r04c20",
      position: [127.5, -7.5],
    }, {
      key: "r04c21",
      position: [142.5, -7.5],
    }, {
      key: "r04c22",
      position: [157.5, -7.5],
    }, {
      key: "r04c23",
      position: [172.5, -7.5],
    }, {
      key: "r05c23",
      position: [172.5, -22.5],
    }, {
      key: "r05c22",
      position: [157.5, -22.5],
    }, {
      key: "r05c21",
      position: [142.5, -22.5],
    }, {
      key: "r05c20",
      position: [127.5, -22.5],
    }, {
      key: "r05c19",
      position: [112.5, -22.5],
    }, {
      key: "r05c18",
      position: [97.5, -22.5],
    }, {
      key: "r05c17",
      position: [82.5, -22.5],
    }, {
      key: "r05c16",
      position: [67.5, -22.5],
    }, {
      key: "r05c15",
      position: [52.5, -22.5],
    }, {
      key: "r05c14",
      position: [37.5, -22.5],
    }, {
      key: "r05c13",
      position: [22.5, -22.5],
    }, {
      key: "r05c12",
      position: [7.5, -22.5],
    }, {
      key: "r05c11",
      position: [-7.5, -22.5],
    }, {
      key: "r05c10",
      position: [-22.5, -22.5],
    }, {
      key: "r05c09",
      position: [-37.5, -22.5],
    }, {
      key: "r05c08",
      position: [-52.5, -22.5],
    }, {
      key: "r05c07",
      position: [-67.5, -22.5],
    }, {
      key: "r05c06",
      position: [-82.5, -22.5],
    }, {
      key: "r05c05",
      position: [-97.5, -22.5],
    }, {
      key: "r05c04",
      position: [-112.5, -22.5],
    }, {
      key: "r05c03",
      position: [-127.5, -22.5],
    }, {
      key: "r05c02",
      position: [-142.5, -22.5],
    }, {
      key: "r05c01",
      position: [-157.5, -22.5],
    }, {
      key: "r05c00",
      position: [-172.5, -22.5],
    }],
    closed: false,
    role: "construction",
    label: "Central four-row mounting lattice",
  });
  const centralCells = $.use("centralCells", fixtureCells, {
    centers: centralLattice.vertices,
    pilotRadius: mm(2.5),
    counterboreRadius: mm(4.5),
  });
  const southernLattice = $.geometry.polyline("southernLattice", {
    vertices: [{
      key: "r06c00",
      position: [-172.5, -37.5],
    }, {
      key: "r06c01",
      position: [-157.5, -37.5],
    }, {
      key: "r06c02",
      position: [-142.5, -37.5],
    }, {
      key: "r06c03",
      position: [-127.5, -37.5],
    }, {
      key: "r06c04",
      position: [-112.5, -37.5],
    }, {
      key: "r06c05",
      position: [-97.5, -37.5],
    }, {
      key: "r06c06",
      position: [-82.5, -37.5],
    }, {
      key: "r06c07",
      position: [-67.5, -37.5],
    }, {
      key: "r06c08",
      position: [-52.5, -37.5],
    }, {
      key: "r06c09",
      position: [-37.5, -37.5],
    }, {
      key: "r06c10",
      position: [-22.5, -37.5],
    }, {
      key: "r06c11",
      position: [-7.5, -37.5],
    }, {
      key: "r06c12",
      position: [7.5, -37.5],
    }, {
      key: "r06c13",
      position: [22.5, -37.5],
    }, {
      key: "r06c14",
      position: [37.5, -37.5],
    }, {
      key: "r06c15",
      position: [52.5, -37.5],
    }, {
      key: "r06c16",
      position: [67.5, -37.5],
    }, {
      key: "r06c17",
      position: [82.5, -37.5],
    }, {
      key: "r06c18",
      position: [97.5, -37.5],
    }, {
      key: "r06c19",
      position: [112.5, -37.5],
    }, {
      key: "r06c20",
      position: [127.5, -37.5],
    }, {
      key: "r06c21",
      position: [142.5, -37.5],
    }, {
      key: "r06c22",
      position: [157.5, -37.5],
    }, {
      key: "r06c23",
      position: [172.5, -37.5],
    }, {
      key: "r07c23",
      position: [172.5, -52.5],
    }, {
      key: "r07c22",
      position: [157.5, -52.5],
    }, {
      key: "r07c21",
      position: [142.5, -52.5],
    }, {
      key: "r07c20",
      position: [127.5, -52.5],
    }, {
      key: "r07c19",
      position: [112.5, -52.5],
    }, {
      key: "r07c18",
      position: [97.5, -52.5],
    }, {
      key: "r07c17",
      position: [82.5, -52.5],
    }, {
      key: "r07c16",
      position: [67.5, -52.5],
    }, {
      key: "r07c15",
      position: [52.5, -52.5],
    }, {
      key: "r07c14",
      position: [37.5, -52.5],
    }, {
      key: "r07c13",
      position: [22.5, -52.5],
    }, {
      key: "r07c12",
      position: [7.5, -52.5],
    }, {
      key: "r07c11",
      position: [-7.5, -52.5],
    }, {
      key: "r07c10",
      position: [-22.5, -52.5],
    }, {
      key: "r07c09",
      position: [-37.5, -52.5],
    }, {
      key: "r07c08",
      position: [-52.5, -52.5],
    }, {
      key: "r07c07",
      position: [-67.5, -52.5],
    }, {
      key: "r07c06",
      position: [-82.5, -52.5],
    }, {
      key: "r07c05",
      position: [-97.5, -52.5],
    }, {
      key: "r07c04",
      position: [-112.5, -52.5],
    }, {
      key: "r07c03",
      position: [-127.5, -52.5],
    }, {
      key: "r07c02",
      position: [-142.5, -52.5],
    }, {
      key: "r07c01",
      position: [-157.5, -52.5],
    }, {
      key: "r07c00",
      position: [-172.5, -52.5],
    }],
    closed: false,
    role: "construction",
    label: "Southern two-row mounting lattice",
  });
  const southernCells = $.use("southernCells", fixtureCells, {
    centers: southernLattice.vertices,
    pilotRadius: mm(2.5),
    counterboreRadius: mm(4.5),
  });
  const horizontalPitch = $.dimension.pointDistance("horizontalPitch", {
    first: northernLattice.vertices.byKey.r00c00,
    second: northernLattice.vertices.byKey.r00c01,
    value: mm(15),
    mode: "reference",
    label: "Horizontal pitch · 15 mm",
  });
  const verticalPitch = $.dimension.pointDistance("verticalPitch", {
    first: northernLattice.vertices.byKey.r00c00,
    second: northernLattice.vertices.byKey.r01c00,
    value: mm(15),
    mode: "reference",
    label: "Vertical pitch · 15 mm",
  });
  $.group("Fixture envelope", [fixtureEnvelope]);
  $.group("Northern mounting bank", [northernLattice, northernCells]);
  $.group("Central mounting bank", [centralLattice, centralCells]);
  $.group("Southern mounting bank", [southernLattice, southernCells]);
  $.group("Fixture pitch annotations", [horizontalPitch, verticalPitch]);
  return {
    envelope: fixtureEnvelope,
    cells: {
      northern: northernCells,
      central: centralCells,
      southern: southernCells,
    },
    pitch: {
      horizontal: horizontalPitch,
      vertical: verticalPitch,
    },
  };
});
