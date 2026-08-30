// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import { adaptiveLanterns } from "../examples/adaptive-lanterns.patch.js";
import { crossBrace } from "../examples/braced-frame.patch.js";
import { bridgeCables } from "../examples/bridge-cables.patch.js";
import { compassCore } from "../examples/compass-core.patch.js";
import { cornerReliefs } from "../examples/corner-reliefs.patch.js";
import { harnessRoute } from "../examples/harness-route.patch.js";
import { mountingPlate } from "../examples/mounting-plate.patch.js";
import { roundEveryCorner } from "../examples/rounded-polyline.patch.js";
import { fillets as mappedFillets } from "../examples/typed-panel.patch.js";
import { waterChannel } from "../examples/water-channel.patch.js";
import { compilePatchArtifact } from "../src/compiler.js";
import {
  CodeControlClient,
  CodeControlRpcProtocolError,
  MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES,
  MAX_CODE_CONTROL_RPC_REQUEST_BYTES,
  PATCH_ARTIFACT_FORMAT,
  SKETCH_CODE_SDK_ABI,
  createProject,
  definePatch,
  encodeCodeControlRpcRequest,
  fillets,
  mm,
  point,
  polyline,
  rectangle,
  t,
  type CodeSessionIdentity,
  type ManagedControlManifest,
  type ManagedValue,
} from "../src/index.js";

function codeControlFixture() {
  const identity: CodeSessionIdentity = {
    session: 7,
    revision: 3,
    digest: "a".repeat(64),
  };
  const expected: ManagedValue = {
    kind: "unit",
    value: { unit: "mm", value: 4 },
  };
  const token = {
    id: "b".repeat(64),
    project: "typed-panel",
    project_digest: "c".repeat(64),
    source_digest: "d".repeat(64),
    declaration: "cornerFillets",
    path: ["radius"],
    expected,
    generation_digest: "e".repeat(64),
    authentication: "f".repeat(64),
  } as const;
  const manifest: ManagedControlManifest = {
    project: "typed-panel",
    project_digest: token.project_digest,
    source_digest: token.source_digest,
    expansion_digest: "1".repeat(64),
    controls: [{
      id: token.id,
      source: {
        declaration: token.declaration,
        path: token.path,
        kind: "literal",
        span: { start: 120, end: 125 },
        source_text: "mm(4)",
      },
      value: expected,
      schema: {
        kind: "unit",
        unit: "mm",
        number: "real",
        minimum: { value: 0, inclusive: false },
        maximum: null,
      },
      consumers: [{
        target: {
          target: "generated",
          address: {
            invocation: "cornerFillets",
            template: ["fillet"],
            member_key: ["upperLeft"],
            output: ["arc"],
          },
          identity: { allocation: 11, generation: 0 },
          artifact_digest: "2".repeat(64),
          family: "computed.fillet",
        },
        property: ["radius"],
      }],
      access: { access: "editable", token },
    }],
  };
  return { identity, manifest, token };
}

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

test("mapRecord and all ten examples compile deterministically", () => {
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
      source: "cornerReliefs",
      moduleSpecifier: "./patches/corner-reliefs.patch.ts",
      exportName: "cornerReliefs",
      patch: cornerReliefs,
    }),
    () => compilePatchArtifact({
      source: "harnessRoute",
      moduleSpecifier: "./patches/harness-route.patch.ts",
      exportName: "harnessRoute",
      patch: harnessRoute,
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

  const reliefs = compilePatchArtifact({
    source: "cornerReliefs",
    moduleSpecifier: "./patches/corner-reliefs.patch.ts",
    exportName: "cornerReliefs",
    patch: cornerReliefs,
  });
  assert.deepEqual(reliefs.artifact.inputs, { centers: "collection", radius: "scalar" });
  assert.deepEqual(reliefs.artifact.outputs, { reliefs: "collection" });
  assert.deepEqual(reliefs.artifact.collections, [{
    rule: "map_record",
    path: ["reliefs"],
    input: "centers",
    templates: [["circle"], ["radius"]],
  }]);
  assert.deepEqual(reliefs.artifact.edit_lenses, [{
    output: ["reliefs"],
    invocation_argument: ["radius"],
    expected_kind: "scalar",
  }]);
  assert.equal(reliefs.artifact.templates[0]?.declaration_family, "geometry.circle");
  assert.equal(reliefs.artifact.templates[0]?.result_output, "circle");
  assert.equal(reliefs.artifact.templates[1]?.declaration_family, "dimension.radius");
  assert.equal(reliefs.artifact.templates[1]?.result_output, null);
  assert.deepEqual(reliefs.artifact.templates[1]?.inputs, {
    curve: {
      source: "template_output",
      template: ["circle"],
      output: "circle",
      expected_kind: "curve",
    },
    target: {
      source: "input",
      name: "radius",
      path: [],
      expected_kind: "scalar",
    },
  });
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

test("code-control client inspects, edits, and moves authenticated outer history", async () => {
  const fixture = codeControlFixture();
  const after: CodeSessionIdentity = {
    session: fixture.identity.session,
    revision: fixture.identity.revision + 1,
    digest: "3".repeat(64),
  };
  const requests: unknown[] = [];
  const client = new CodeControlClient({
    apply: (encoded) => {
      const request = JSON.parse(encoded) as { method: string };
      requests.push(request);
      switch (request.method) {
        case "inspect_managed_controls":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "managed_controls",
              snapshot: {
                identity: fixture.identity,
                manifest: fixture.manifest,
                can_undo: false,
                can_redo: false,
              },
            },
          });
        case "edit_managed_controls":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "managed_control_edit",
              receipt: {
                receipt: {
                  before: fixture.identity,
                  after,
                  label: "Apply managed source",
                  retained_failure: false,
                },
                diagnostic: null,
              },
            },
          });
        case "undo":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "history",
              receipt: {
                identity: fixture.identity,
                moved: true,
                receipt: {
                  before: after,
                  after: fixture.identity,
                  label: "Undo Apply managed source",
                  retained_failure: false,
                },
              },
            },
          });
        case "redo":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "history",
              receipt: {
                identity: after,
                moved: true,
                receipt: {
                  before: fixture.identity,
                  after,
                  label: "Redo Apply managed source",
                  retained_failure: false,
                },
              },
            },
          });
        default:
          throw new Error(`unexpected method ${request.method}`);
      }
    },
  });

  const inspected = await client.inspect();
  assert.equal(inspected.outcome, "success");
  if (inspected.outcome !== "success") return;
  const edited = await client.edit(inspected.value.snapshot, {
    edits: [{
      token: fixture.token,
      value: { kind: "unit", value: { unit: "mm", value: 2 } },
    }],
  });
  assert.equal(edited.outcome, "success");
  assert.equal(
    edited.outcome === "success" && edited.value.receipt.receipt.after.digest,
    after.digest,
  );
  assert.equal((await client.undo(after)).outcome, "success");
  assert.equal((await client.redo(fixture.identity)).outcome, "success");
  assert.deepEqual(
    requests.map((request) => (request as { method: string }).method),
    ["inspect_managed_controls", "edit_managed_controls", "undo", "redo"],
  );
  const editRequest = requests[1] as Record<string, unknown>;
  assert.deepEqual(Object.keys(editRequest), ["method", "expected", "batch"]);
});

test("code-control encoder preserves signed zero and rejects loose or unsafe authority", () => {
  const fixture = codeControlFixture();
  const encoded = encodeCodeControlRpcRequest({
    method: "edit_managed_controls",
    expected: fixture.identity,
    batch: {
      edits: [{ token: fixture.token, value: { kind: "number", value: -0 } }],
    },
  });
  const parsed = JSON.parse(encoded) as {
    batch: { edits: [{ value: { value: number } }] };
  };
  assert.ok(Object.is(parsed.batch.edits[0].value.value, -0));

  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "undo",
      expected: {
        ...fixture.identity,
        revision: Number.MAX_SAFE_INTEGER + 1,
      },
    }),
    /exactly represented unsigned integer/u,
  );
  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "inspect_managed_controls",
      browser_only: true,
    } as never),
    /unknown or missing fields/u,
  );
  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "edit_managed_controls",
      expected: fixture.identity,
      batch: {
        edits: [
          { token: fixture.token, value: { kind: "number", value: 1 } },
          { token: fixture.token, value: { kind: "number", value: 2 } },
        ],
      },
    }),
    /repeats control/u,
  );
  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "edit_managed_controls",
      expected: fixture.identity,
      batch: {
        edits: [{
          token: fixture.token,
          value: { kind: "string", value: "x".repeat(MAX_CODE_CONTROL_RPC_REQUEST_BYTES) },
        }],
      },
    }),
    /request exceeds/u,
  );
});

test("code-control token authentication is independent of DTO property order", async () => {
  const fixture = codeControlFixture();
  const reorderedToken = Object.fromEntries(
    Object.entries(fixture.token).reverse(),
  ) as unknown as typeof fixture.token;
  let called = false;
  const response = await new CodeControlClient({
    apply: () => {
      called = true;
      return JSON.stringify({
        outcome: "failure",
        failure: {
          code: "control_edit_rejected",
          message: "server-side test stop",
          identity: fixture.identity,
        },
      });
    },
  }).edit(
    { identity: fixture.identity, manifest: fixture.manifest },
    {
      edits: [{
        token: reorderedToken,
        value: { kind: "unit", value: { unit: "mm", value: 2 } },
      }],
    },
  );
  assert.ok(called);
  assert.equal(response.outcome, "failure");
});

test("managed object decoding preserves __proto__ as an ordinary own field", async () => {
  const fixture = codeControlFixture();
  const manifest = JSON.parse(JSON.stringify(fixture.manifest)) as {
    controls: [{ value: ManagedValue; access: { token: { expected: ManagedValue } } }];
  };
  const protoValue = JSON.parse(
    '{"kind":"object","value":{"__proto__":{"kind":"number","value":7}}}',
  ) as ManagedValue;
  manifest.controls[0].value = protoValue;
  manifest.controls[0].access.token.expected = protoValue;
  const response = await new CodeControlClient({
    apply: () => JSON.stringify({
      outcome: "success",
      value: {
        result: "managed_controls",
        snapshot: {
          identity: fixture.identity,
          manifest,
          can_undo: false,
          can_redo: false,
        },
      },
    }),
  }).inspect();
  assert.equal(response.outcome, "success");
  if (response.outcome !== "success") return;
  const value = response.value.snapshot.manifest.controls[0]?.value;
  assert.equal(value?.kind, "object");
  if (value?.kind !== "object") return;
  assert.ok(Object.hasOwn(value.value, "__proto__"));
  assert.deepEqual(Object.keys(value.value), ["__proto__"]);
  assert.equal(Object.getPrototypeOf(value.value), Object.prototype);
});

test("code-control mutation receipts are bound to the requested outer identity", async () => {
  const fixture = codeControlFixture();
  const foreignBefore = {
    ...fixture.identity,
    revision: fixture.identity.revision + 6,
    digest: "6".repeat(64),
  };
  const foreignAfter = {
    ...foreignBefore,
    revision: foreignBefore.revision + 1,
    digest: "7".repeat(64),
  };
  const batch = {
    edits: [{
      token: fixture.token,
      value: { kind: "unit", value: { unit: "mm", value: 2 } },
    }],
  } as const;
  await assert.rejects(
    () => new CodeControlClient({
      apply: () => JSON.stringify({
        outcome: "success",
        value: {
          result: "managed_control_edit",
          receipt: {
            receipt: {
              before: foreignBefore,
              after: foreignAfter,
              label: "Foreign edit",
              retained_failure: false,
            },
            diagnostic: null,
          },
        },
      }),
    }).edit({ identity: fixture.identity, manifest: fixture.manifest }, batch),
    /does not start at the requested code-session identity/u,
  );
  await assert.rejects(
    () => new CodeControlClient({
      apply: () => JSON.stringify({
        outcome: "success",
        value: {
          result: "history",
          receipt: {
            identity: foreignAfter,
            moved: true,
            receipt: {
              before: foreignBefore,
              after: foreignAfter,
              label: "Foreign Undo",
              retained_failure: false,
            },
          },
        },
      }),
    }).undo(fixture.identity),
    /does not start at the requested code-session identity/u,
  );
  const noOp = await new CodeControlClient({
    apply: () => JSON.stringify({
      outcome: "success",
      value: {
        result: "history",
        receipt: { identity: fixture.identity, moved: false, receipt: null },
      },
    }),
  }).redo(fixture.identity);
  assert.equal(noOp.outcome, "success");
});

test("code-control client rejects malformed, method-confused, and unsafe responses", async () => {
  const fixture = codeControlFixture();
  const invoke = async (response: string) => {
    const client = new CodeControlClient({ apply: () => response });
    await client.inspect();
  };

  await assert.rejects(() => invoke("not json"), CodeControlRpcProtocolError);
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "failure",
      failure: { code: "browser_guess", message: "unknown", identity: null },
    })),
    /failure code has unknown value/u,
  );
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "success",
      value: {
        result: "history",
        receipt: { identity: fixture.identity, moved: false, receipt: null },
      },
    })),
    /expected result managed_controls/u,
  );
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "success",
      value: {
        result: "managed_controls",
        snapshot: {
          identity: { ...fixture.identity, session: Number.MAX_SAFE_INTEGER + 1 },
          manifest: fixture.manifest,
          can_undo: false,
          can_redo: false,
        },
      },
    })),
    /exactly represented unsigned integer/u,
  );
  await assert.rejects(
    () => new CodeControlClient({
      apply: () => " ".repeat(MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES + 1),
    }).undo(fixture.identity),
    /response exceeds/u,
  );

  const failure = await new CodeControlClient({
    apply: () => JSON.stringify({
      outcome: "failure",
      failure: {
        code: "control_edit_rejected",
        message: "stale token",
        identity: fixture.identity,
      },
    }),
  }).inspect();
  assert.equal(failure.outcome, "failure");
  assert.equal(
    failure.outcome === "failure" && failure.failure.code,
    "control_edit_rejected",
  );
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
