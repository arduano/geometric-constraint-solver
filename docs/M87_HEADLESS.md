<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M87 browser-free authoring loop

`geosolve-headless` runs managed-code expansion, the native solver, independent acceptance checks,
scene fitting, SVG composition and PNG rasterization without a browser, web server, DOM, network,
Node or TypeScript runtime. It is suitable for an AI agent or native host that needs exact semantic
controls and visual evidence from the same accepted design authority.

M87 is accepted and closed. The supervising user's 2026-08-31 milestone-level close decision accepts
U9/U10 without a separate row replay; the earlier U1-U8 disposition remains historical. This
workflow, all twelve bundled demos and the deterministic manufacturing-sketch products are
retained. Exact source `32c7289`, tree `38f7175`, passes the complete clean release gate. No
immutable artifact is frozen, no release is published and the mutable Tailscale development
listener is not retired.

After M87-F003, the focused manufacturing owner suite passes 3/3, native composition passes 13/13,
the reviewed-ledger check passes 1/1 and the all-demo headless/deterministic-product suite passes
10/10. Fresh manufacturing review renders remain available; the pre-F003 products below remain
historical only.

## Inputs

Choose exactly one input for each command:

- `--managed sketch.ts --project-key KEY` accepts artifact-free managed-v1 source;
- `--project project.json` accepts a complete pinned `CodeProject`, including data-only artifacts;
- `--demo KEY` selects one of the twelve checked-in offline projects listed by `demos`.

Custom patch TypeScript is never executed. A project that uses a custom patch must already contain
its canonical digest-pinned artifact, normally by using `project.json` or a bundled demo.

## Inspect

```bash
cargo run --locked -p geosolve-headless -- \
  inspect --demo typed-panel > inspection.json
```

The JSON contains a deterministic solve/render report and a transient managed-control manifest.
Each editable row has an exact source token, typed schema, current value and complete bounded list
of semantic consumers. Read-only rows explain whether the value is structure, a reference, a
structural identity, a solver-instance/DoF value or another deliberately unsupported inverse.

Tokens authenticate the exact project, source bytes, expected typed value and generated consumer
identities. Do not synthesize or reuse a token after any source or project edit; inspect again.

## Exact-CAS edit

Copy one or more complete `access.token` objects from the inspection into a batch. The replacement
uses the tagged `ManagedValue` wire shape. For example, a length change is:

```json
{
  "edits": [
    {
      "token": { "copy": "the complete token from inspection.json" },
      "value": {
        "kind": "unit",
        "value": { "unit": "mm", "value": 2.0 }
      }
    }
  ]
}
```

The `token` placeholder above is explanatory, not valid input: replace it with the complete token
object verbatim. Number, boolean, choice and text replacements respectively use `number`, `bool`
and `string` tagged values. A unit replacement must retain the schema's exact unit.

```bash
cargo run --locked -p geosolve-headless -- \
  edit --demo typed-panel --edit batch.json --out /tmp/geosolve-typed-panel-r2
```

The batch validates completely before one source rewrite and reparse. The candidate is then cold
expanded, solved and independently validated before publication. A stale token, wrong type/unit,
non-finite value, invalid source, failed materialization or failed acceptance publishes no output
directory. Shared controls edit their one source token and update every disclosed consumer.

For the next stateless iteration, inspect the emitted `project.json`, then use its new tokens:

```bash
cargo run --locked -p geosolve-headless -- \
  inspect --project /tmp/geosolve-typed-panel-r2/project.json > inspection-r2.json
```

## Render products

```bash
cargo run --locked -p geosolve-headless -- \
  render --project project.json --out /tmp/geosolve-generation-new
```

The destination must be a new directory. On Linux, Android, Apple platforms and Redox,
publication uses an atomic no-replace directory rename and never overwrites an existing
generation. Other targets currently fail closed before publication because the crate does not yet
have a proven atomic no-clobber directory primitive for them. A successful directory contains:

- `report.json` — input/digest identities, independent validation, fitted camera and output hashes;
- `controls.json` — the exact transient managed-control manifest;
- `project.json` — canonical complete project authority;
- `sketch.ts` — the accepted managed source;
- `scene.svg` — deterministic static vector output;
- `scene.png` — `2000 × 1400` native raster output.

The logical scene is `1000 × 700`, fitted with a 64 px margin, a 2–2000 px/unit scale clamp and a
0.25 px chord tolerance. Static output includes grid, datums, geometry and annotations; selection,
hover, drafts, inference, action affordances and error overlays are absent. SVG/report bytes are
deterministic authority. PNG is deterministic visual evidence for the pinned build inputs, not a
solver oracle.

Every success-like result has finite current accepted geometry, independently validated hard
residuals no greater than `1e-9` (or a validated empty hard set), and Current active computed
features. No solved coordinate is ever written back into managed source.

## Routing-board dogfood

The densest bundled project exercises the same stateless path without a browser:

```bash
cargo run --locked -p geosolve-headless -- \
  inspect --demo robotic-routing-board > routing-board-inspection.json

cargo run --locked -p geosolve-headless -- \
  render --demo robotic-routing-board --out /tmp/geosolve-routing-board-new
```

Its accepted inventory is 104 points, 176 curves, 41 constraints, two dimensions, 64 Current
features and 136 computed edges. The source contains eight keyed ten-vertex open routes; the pinned
artifact derives 80 clip circles and 64 Fillets. Repeating the exact render must reproduce the
pretty-encoded report and controls, logical scene markup, standalone SVG and PNG bytes for the
pinned build. PNG equality remains a deterministic build regression, not cross-platform solver
authority.

## CNC and Gridfinity dogfood

The two additive manufacturing-sketch entries use the same stateless commands:

```bash
cargo run --locked -p geosolve-headless -- \
  render --demo cnc-joinery-fit-coupon --out /tmp/geosolve-cnc-coupon-new

cargo run --locked -p geosolve-headless -- \
  render --demo gridfinity-1x1x3-section --out /tmp/geosolve-gridfinity-section-new
```

The CNC report must describe the fully constrained 120 x 140 mm blank, three 70 mm-wide mortises with
loose/nominal/press heights of 18.4/18.0/17.6 mm, three 95 x 18 mm tabs, twelve shared cutter-radius
circles and ten handling Fillets. Its accepted inventory is 29 points, 47 curves, 36 constraints,
33 dimensions, 10 Current features and 23 computed edges. Exactly one `FixedPoint`, no
`FixedCoordinate` and seven relational construction datums locate the components.

The Gridfinity report must retain one fully constrained symmetric closed 26-point
material contour with the reviewed 41.5/35.6 mm widths, 4.75/7 mm base levels, 21 mm body, 0.95 mm
walls, 4.4 mm nominal lip rise and two floor plus two lip Fillets. Its accepted inventory is 31
points, 36 curves, 31 constraints, 18 dimensions, four Current features and 11 computed edges.
Thirteen datum-axis symmetry relations and ten orthogonal construction spans govern the contour
from one Y `FixedCoordinate` and no `FixedPoint`. Both reports must expose numerical and structural
left/right nullity zero plus equality and bidirectional bounded DOF zero.

Repeated report/control/logical-scene/SVG/PNG products pass the post-F003 regression. Fresh mutable
review bundles are:

- M87-U9 evidence: `/tmp/geosolve-m87-post-f003.FPHP3b/cnc`;
- M87-U10 evidence: `/tmp/geosolve-m87-post-f003.FPHP3b/gridfinity`.

The prior bundles below were generated from the pre-F003 sources and are historical only:

- pre-F003 M87-U9 evidence: `/tmp/geosolve-m87-manufacturing-uat.K7YRaV/cnc`;
- pre-F003 M87-U10 evidence: `/tmp/geosolve-m87-manufacturing-uat.K7YRaV/gridfinity`.

Regenerate into new non-existing directories if these mutable `/tmp` products are unavailable.

These outputs are 2D/2.5D design evidence. They do not create CAM, toolpath, cutter-compensation,
boolean, solid, printer-fit or manufacturing authority. Diagnosed performance/stack work is carried
into active M88's ordered stability prerequisite.
