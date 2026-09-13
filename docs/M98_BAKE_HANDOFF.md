<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98-B accepted baked-profile export handoff

Historical prototype evidence for the [v1 profile contract](M98_BAKE_CONTRACT.md),
introduced by `18371de` (native sampler) and `3863059` (WASM/CLI and fixtures).
This early follow-up did not claim release qualification or human acceptance.
[Final M98 qualification](M98_QUALIFICATION.md) and
[M99 consumer qualification](M99_QUALIFICATION.md) supersede its delivery status.

## Run

Use the current [profile-export examples](../examples/file-workspace-bake/README.md)
after [Getting started](GETTING_STARTED.md). The prototype command used a former
`scripts/file-workspace.mjs`; the maintained entry point is
`packages/geosolve-cli/bin/geosolve.mjs bake`.

No running UI or token is required. The output parent must exist outside the source
project. Successful export atomically replaces a regular output; failure retains it.

## Authority, math and integration seams

- `geosolve-sketch-topology::TopologyProductionProfile::sample_polygons` accepts the exact
  retained session, validates the production stamp before/after, and evaluates fragment source
  parameters through `SketchDocument::evaluate_curve_jet`. Output has CCW outer/CW hole loops,
  implicit closure and file-local region IDs. Lines/polyline spans need no curved subdivision.
- Circle/arc segment count uses the accepted radius and evaluator parameter speed, with
  `delta <= min(sqrt(error / radius), pi/2)`. Thus the ideal sagitta
  `r * (1 - cos(delta/2)) <= r * delta²/8 <= error/8`. Endpoint joins must fit `error/8`;
  a conservative scale/angle-dependent floating-point allowance must also fit `error/8`.
  Remaining margin avoids claiming the entire budget for ideal math. Precision/work-limit
  failures reject output. Polygon finiteness, winding, intersections, holes and joins are
  checked independently of render tessellation. No solver residual/equation changes.
The original adapter routed this through `WorkbenchHandle.bakeProfile` and refused
computed features. M98's dedicated engine subsequently added accepted computed
line/arc/channel profile export. M99 moved the production loader and CLI into
`packages/geosolve-cli/runtime`. Native sampling and independent validation remain
the geometry authority; current commands are in the profile example README.

## Scope and next consumer checks

The first cut admitted one manifest and managed entry file, with native lines,
polyline spans, circles and circular arcs. It refused computed or unsupported
geometry, incomplete topology, stale state and invalid current source. Sampling
had bounds of 65,536 vertices and 2,000,000 edge-pair tests; precision targets
could fail explicitly. Later engine export expands supported boundaries while
retaining bounded work and truthful rejection.

Each bounded arrangement face is exported, including circular interiors. The nominal
board-with-four-holes example requires selecting `region-4` for its recorded source.
No depth, material removal, physical fit or persistent topology identity is inferred.

## Actual bake evidence

The nominal board export used an 85 × 56 mm rectangle and four diameter-2.7 mm holes.
A separate disk solved to radius 12 from a radius-10 seed. A radius-5 quarter arc
closed through a chord used explicit endpoint contacts. Changing the board width
from 85 to 90 mm changed the accepted export, while invalid current source refused
export even with a valid recovery copy.

These exports were produced by the real compiler and accepted Rust geometry.
The final external modeling proof is recorded with exact installed archives in
[M99 qualification](M99_QUALIFICATION.md#exact-artifacts-and-consumer-use).

## Exact focused checks and outcomes

The original sampler suite passed 3/3; exporter tests passed 7/7; the actual-browser
file-sync suite passed 14/14 including its parent; frontend adapter/receipt tests
passed 20/20. TypeScript, Vite, optimized WASM and build-contract checks passed.
They covered winding/holes/hash, solved radius/arc samples, repeatability, source
changes, invalid-source retention, unsupported topology, output guards and exact
BOM-byte hashing. A 900 ms small-sketch save observation included concurrent build
activity and was not a performance guarantee.

Strict WASM-target Clippy initially failed on five pre-existing findings. A scoped
follow-up allowed those categories and passed; that was not a strict Clippy pass.
The later integrated gate supplies the strict qualification. Historical build,
regression and browser logs were recorded under `target/m98/logs/bake-*`.

## Preserved preview and next manual steps

The prototype exporter required no preview service. Current users can bake a copied
example, inspect its provenance and explicit region, change a source dimension,
and repeat. The independent consumer must validate its own solid/STL output;
GeoSolve's accepted 2D geometry does not certify downstream manufacturing behavior.
