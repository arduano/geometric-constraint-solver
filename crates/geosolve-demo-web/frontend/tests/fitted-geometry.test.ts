// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { compareFittedGeometry, FITTED_GEOMETRY_TOLERANCE_PIXELS } from "./fitted-geometry";

const fixture = () => ({ viewBox: [0, 0, 1000, 700], tokens: [
  { kind: "polyline", points: [[0, 636], [557.2, 636]], closed: false },
  { kind: "circle", center: [557.2, 636], radius: 5 },
] });
const encode = (value: unknown) => JSON.stringify(value);

describe("fitted geometry presentation comparison", () => {
  it("accepts measured binary-roundtrip noise without mutating raw evidence", () => {
    const expected = encode(fixture());
    const actual = expected.replaceAll("557.2", "557.1999999999999");
    const result = compareFittedGeometry(actual, expected);
    expect(result.equal).toBe(true);
    expect(result.maximumCoordinateDelta).toBe(1.1368683772161603e-13);
    expect(actual).not.toBe(expected);
    expect(compareFittedGeometry(actual, expected).difference).toBeNull();
  });

  it("uses the explicit absolute pixel boundary without a magnitude-relative allowance", () => {
    const expected = fixture();
    const boundary = fixture();
    boundary.tokens[0].points![0][0] = FITTED_GEOMETRY_TOLERANCE_PIXELS;
    expect(compareFittedGeometry(encode(boundary), encode(expected)).equal).toBe(true);
    boundary.tokens[0].points![0][0] = FITTED_GEOMETRY_TOLERANCE_PIXELS * 1.01;
    expect(compareFittedGeometry(encode(boundary), encode(expected)).equal).toBe(false);
    const large = encode(expected).replaceAll("557.2", "1000000000");
    const moved = encode(expected).replaceAll("557.2", "1000000000.000001");
    expect(compareFittedGeometry(moved, large).equal).toBe(false);
  });

  it("requires meaningful displacement for the same comparison's inequality result", () => {
    const expected = encode(fixture());
    const noisy = expected.replaceAll("557.2", "557.1999999999999");
    const moved = expected.replaceAll("557.2", "557.21");
    expect(compareFittedGeometry(noisy, expected).equal).toBe(true);
    expect(compareFittedGeometry(moved, expected).equal).toBe(false);
  });

  it("retains exact primitive kinds, array order/counts and closure flags", () => {
    const expected = fixture();
    const mutations = [
      { ...expected, tokens: [...expected.tokens].reverse() },
      { ...expected, tokens: expected.tokens.slice(0, 1) },
      { ...expected, tokens: [{ ...expected.tokens[0], points: [[0, 636]] }, expected.tokens[1]] },
      { ...expected, tokens: [{ ...expected.tokens[0], closed: true }, expected.tokens[1]] },
      { ...expected, tokens: [{ kind: "circle", center: [0, 636], radius: 5 }, expected.tokens[1]] },
    ];
    for (const actual of mutations) expect(compareFittedGeometry(encode(actual), encode(expected)).equal).toBe(false);
  });

  it("compares text and radian rotations exactly, including otherwise equal geometry", () => {
    const expected = encode({ viewBox: [0, 0, 1000, 700], tokens: [
      { kind: "ellipse", center: [30, 40], radii: [4, 5], rotation: 0 },
      { kind: "rect", x: 1, y: 2, width: 3, height: 4, radius: 0 },
      { kind: "text", position: [10, 20], text: "radius", rotation: 0 },
    ] });
    expect(compareFittedGeometry(expected, expected).equal).toBe(true);
    expect(compareFittedGeometry(expected.replace('"text":"radius"', '"text":"diameter"'), expected).equal).toBe(false);
    expect(compareFittedGeometry(expected.replace('"rotation":0', '"rotation":1e-13'), expected).equal).toBe(false);
  });

  it.each([
    "broken JSON", "null", "{}",
    encode({ ...fixture(), viewBox: [0, 0, 0, 700] }),
    encode({ ...fixture(), tokens: [] }),
    encode({ ...fixture(), extra: true }),
    encode({ ...fixture(), tokens: [{ kind: "circle", center: [0, null], radius: 5 }] }),
    encode({ ...fixture(), tokens: [{ kind: "circle", center: [0, 1], radius: -5 }] }),
    encode({ ...fixture(), tokens: [{ kind: "circle", center: [0, 1], radius: 5, extra: true }] }),
    encode({ ...fixture(), tokens: [{ kind: "polyline", points: [], closed: false }] }),
    encode({ ...fixture(), tokens: [{ kind: "circle", center: [0, 1, 2], radius: 5 }] }),
    encode({ ...fixture(), tokens: [{ kind: "unsupported", center: [0, 1], radius: 5 }] }),
    encode(fixture()).replace("557.2", "1e400"),
    encode(fixture()).replace("557.2", '"557.2"'),
  ])("rejects malformed/nonfinite input before equality or inequality: %s", (invalid) => {
    const valid = encode(fixture());
    expect(() => compareFittedGeometry(invalid, valid)).toThrow();
    expect(() => compareFittedGeometry(valid, invalid)).toThrow();
    expect(() => compareFittedGeometry(invalid, invalid)).toThrow();
  });
});
