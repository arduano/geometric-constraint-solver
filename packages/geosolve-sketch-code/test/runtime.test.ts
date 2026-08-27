// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import { adaptiveLanterns } from "../examples/adaptive-lanterns.patch.js";
import { crossBrace } from "../examples/braced-frame.patch.js";
import { bridgeCables } from "../examples/bridge-cables.patch.js";
import { compassCore } from "../examples/compass-core.patch.js";
import { mountingPlate } from "../examples/mounting-plate.patch.js";
import { roundEveryCorner } from "../examples/rounded-polyline.patch.js";
import { fillets as mappedFillets } from "../examples/typed-panel.patch.js";
import { waterChannel } from "../examples/water-channel.patch.js";
import { compilePatchArtifact } from "../src/compiler.js";
import {
  PATCH_ARTIFACT_FORMAT,
  SKETCH_CODE_SDK_ABI,
  createProject,
  definePatch,
  fillets,
  mm,
  point,
  polyline,
  rectangle,
  t,
} from "../src/index.js";

test("semantic results expose fixed names, exact mapped records, and keyed collections", () => {
  const project = createProject("runtime");
  const panel = rectangle(project, "panel");
  assert.deepEqual(Object.keys(panel.corners), [
    "lowerLeft",
    "lowerRight",
    "upperRight",
    "upperLeft",
  ]);
  assert.deepEqual(Object.keys(panel.edges), ["bottom", "right", "top", "left"]);
  assert.ok(panel.profile);

  const path = polyline(project, "path", ["lowerLeft", "upperRight", "tail"] as const);
  assert.deepEqual(path.filletableCorners.keys, ["lowerLeft", "upperRight", "tail"]);
  const mapped = fillets(project, {
    lowerLeft: path.filletableCorners.byKey.lowerLeft,
    upperRight: path.filletableCorners.byKey.upperRight,
  });
  assert.deepEqual(Object.keys(mapped), ["lowerLeft", "upperRight"]);
  assert.ok(mapped.lowerLeft.arc);
});

test("each records a canonical Rust-shaped, equation-free artifact", () => {
  const compiled = compilePatchArtifact({
    source: "source",
    moduleSpecifier: "./patches/rounded-polyline.patch.ts",
    exportName: "roundEveryCorner",
    patch: roundEveryCorner,
  });
  assert.equal(compiled.artifact.format, PATCH_ARTIFACT_FORMAT);
  assert.equal(compiled.artifact.sdk_abi, SKETCH_CODE_SDK_ABI);
  assert.equal(compiled.artifact.source_digest, "41cf6794ba4200b839c53531555f0f3998df4cbb01a4d5cb0b94e3ca5e23947d");
  assert.deepEqual(compiled.artifact.inputs, { corners: "collection", radius: "scalar" });
  assert.deepEqual(compiled.artifact.outputs, { fillets: "collection" });
  assert.deepEqual(compiled.artifact.collections, [{
    rule: "each",
    path: ["fillets"],
    input: "corners",
    member_key_field: "key",
    templates: [["fillet"]],
  }]);
  assert.equal(compiled.artifact.templates[0]?.declaration_family, "computed.fillet");
  assert.equal(compiled.artifactDigest, "f1be23340eee0210215ee2e6ed61c2c679b5c300ffabf2644cac861f5bbbedcf");
  assert.deepEqual(JSON.parse(compiled.canonicalJson), compiled.artifact);
  assert.doesNotMatch(compiled.canonicalJson, /node_id|port_id|equation|residual|function/u);
});

test("mapRecord and all eight examples compile deterministically", () => {
  const builds = [
    () => compilePatchArtifact({
      source: "adaptiveLanterns",
      moduleSpecifier: "./patches/adaptive-lanterns.patch.ts",
      exportName: "adaptiveLanterns",
      patch: adaptiveLanterns,
    }),
    () => compilePatchArtifact({
      source: "bridgeCables",
      moduleSpecifier: "./patches/bridge-cables.patch.ts",
      exportName: "bridgeCables",
      patch: bridgeCables,
    }),
    () => compilePatchArtifact({
      source: "compassCore",
      moduleSpecifier: "./patches/compass-core.patch.ts",
      exportName: "compassCore",
      patch: compassCore,
    }),
    () => compilePatchArtifact({
      source: "roundEveryCorner",
      moduleSpecifier: "./patches/round-every-corner.patch.ts",
      exportName: "roundEveryCorner",
      patch: roundEveryCorner,
    }),
    () => compilePatchArtifact({
      source: "fillets",
      moduleSpecifier: "./patches/fillet-record.patch.ts",
      exportName: "fillets",
      patch: mappedFillets,
    }),
    () => compilePatchArtifact({
      source: "crossBrace",
      moduleSpecifier: "./patches/cross-brace.patch.ts",
      exportName: "crossBrace",
      patch: crossBrace,
    }),
    () => compilePatchArtifact({
      source: "mountingPlate",
      moduleSpecifier: "./patches/mounting-plate.patch.ts",
      exportName: "mountingPlate",
      patch: mountingPlate,
    }),
    () => compilePatchArtifact({
      source: "waterChannel",
      moduleSpecifier: "./patches/water-channel.patch.ts",
      exportName: "waterChannel",
      patch: waterChannel,
    }),
  ];
  for (const build of builds) {
    const first = build();
    const second = build();
    assert.equal(first.canonicalJson, second.canonicalJson);
    assert.equal(first.artifactDigest, second.artifactDigest);
  }
  const mapped = compilePatchArtifact({
    source: "fillets",
    moduleSpecifier: "./patches/fillet-record.patch.ts",
    exportName: "fillets",
    patch: mappedFillets,
  });
  assert.deepEqual(mapped.artifact.collections, [{
    rule: "map_record",
    path: ["fillets"],
    input: "corners",
    templates: [["fillet"]],
  }]);
});

test("renamed nested multi-output results retain the exact selected output", () => {
  const aliasedProfile = definePatch(
    { width: t.length(), height: t.length(), radius: t.length() },
    (p, input) => {
      const rounded = p.roundedRectangle(input.width, input.height, input.radius);
      return { nested: { shape: rounded.profile } };
    },
  );
  const compiled = compilePatchArtifact({
    source: "aliasedProfile",
    moduleSpecifier: "./patches/aliased-profile.patch.ts",
    exportName: "aliasedProfile",
    patch: aliasedProfile,
  });
  assert.deepEqual(compiled.artifact.outputs, { nested: "collection" });
  assert.deepEqual(compiled.artifact.templates[0]?.path, ["nested", "shape"]);
  assert.equal(compiled.artifact.templates[0]?.result_output, "profile");

  const ambiguousRoot = definePatch(
    { width: t.length(), height: t.length(), radius: t.length() },
    (p, input) => ({
      shape: p.roundedRectangle(input.width, input.height, input.radius),
    }),
  );
  assert.throws(
    () => compilePatchArtifact({
      source: "ambiguousRoot",
      moduleSpecifier: "./patches/ambiguous-root.patch.ts",
      exportName: "ambiguousRoot",
      patch: ambiguousRoot,
    }),
    /multi-output template result must select one explicit named output/u,
  );

  const mappedProfiles = definePatch(
    {
      centers: t.keyed(t.point()),
      width: t.length(),
      height: t.length(),
      radius: t.length(),
    },
    (p, input) => ({
      profiles: p.each(
        input.centers,
        () => p.roundedRectangle(input.width, input.height, input.radius).profile,
      ),
    }),
  );
  const mapped = compilePatchArtifact({
    source: "mappedProfiles",
    moduleSpecifier: "./patches/mapped-profiles.patch.ts",
    exportName: "mappedProfiles",
    patch: mappedProfiles,
  });
  assert.equal(mapped.artifact.templates[0]?.result_output, "profile");
});

test("compiler rejects invalid authority and non-data literals", () => {
  assert.throws(() => compilePatchArtifact({
    source: "source",
    moduleSpecifier: "../remote.patch.ts",
    exportName: "bad",
    patch: roundEveryCorner,
  }), /invalid custom patch module specifier/u);

  const invalid = definePatch({ center: t.point() }, (p, { center }) => ({
    circle: p.circle(center, Number.NaN as never),
  }));
  assert.throws(() => compilePatchArtifact({
    source: "invalid",
    moduleSpecifier: "./patches/invalid.patch.ts",
    exportName: "invalid",
    patch: invalid,
  }), /template number must be finite/u);

  const invalidKeySelector = definePatch(
    { corners: t.keyed(t.corner()), radius: t.length() },
    (p, { corners, radius }) => ({
      fillets: p.each(
        corners,
        (corner) => p.fillet({ corner, radius }),
        { key: () => "constant" },
      ),
    }),
  );
  assert.throws(() => compilePatchArtifact({
    source: "invalid-key-selector",
    moduleSpecifier: "./patches/invalid-key-selector.patch.ts",
    exportName: "invalidKeySelector",
    patch: invalidKeySelector,
  }), /key selector must return the symbolic member key/u);
});

test("canonical encoding distinguishes Rust f64 literals from path indexes", () => {
  const literal = definePatch({ center: t.point() }, (p, { center }) => {
    p.editLens({
      output: ["circles", 0],
      invocationArgument: ["radius"],
      expectedKind: "scalar",
    });
    return { circle: p.circle(center, mm(4)) };
  });
  const compiled = compilePatchArtifact({
    source: "literal",
    moduleSpecifier: "./patches/literal.patch.ts",
    exportName: "literal",
    patch: literal,
  });
  assert.match(compiled.canonicalJson, /"value":\{"unit":"mm","value":4\.0\}/u);
  assert.match(compiled.canonicalJson, /"output":\["circles",0\]/u);
  assert.deepEqual(JSON.parse(compiled.canonicalJson), compiled.artifact);
});

test("public semantic constructors never surface raw wire identities", () => {
  const project = createProject("sealed");
  const reference = point(project, "point.main");
  assert.deepEqual(Object.keys(reference), []);
  assert.equal("node" in reference, false);
  assert.equal("port" in reference, false);
  assert.equal("id" in reference, false);
});

test("the managed sketch project identity is reserved from public construction", () => {
  assert.throws(
    () => createProject("__geosolve_managed_v1__"),
    /managed sketch project name is reserved/u,
  );
});
