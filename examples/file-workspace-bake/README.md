<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Accepted sketch profile export

From the repository root, after preparing the dedicated engine WASM and TypeScript package:

```sh
mkdir -p target/m98/bake
node scripts/file-workspace.mjs bake examples/file-workspace-bake/pi-footprint \
  --out target/m98/bake/pi-footprint.json --chord-error-mm 0.02
node scripts/file-workspace.mjs bake examples/file-workspace-bake/circle-arc \
  --out target/m98/bake/circle-arc.json --chord-error-mm 0.02
```

The CLI captures the complete local source graph, then compiles, solves and independently
validates it through the dedicated Rust engine in a bounded worker. It queries production
topology and samples accepted curve fragments in model space. It needs
no browser, server, token or recovery cache. JSON contains geometry under the
[v1 contract](../../docs/M98_BAKE_CONTRACT.md), with SHA-256 of the exact source bytes plus complete dependency, input and toolchain provenance.
Output parent directories must exist; output must be outside the selected source project.
Successful bake atomically replaces an existing regular output file. Failure leaves it intact.

`pi-footprint` is a nominal reference: 85 × 56 mm, diameter-2.7 mm holes at
(3.5, 3.5), (61.5, 3.5), (3.5, 52.5), (61.5, 52.5). It is not a case or hardware-fit proof.
Production topology exports every bounded face: the board with four holes **and** the four
interior disks. Select the board's region ID explicitly in the consumer; never extrude all five
as the board. The checked fixture's board is **`region-4`**. IDs are file-local and may change
after editing; the handoff records the source hash associated with this observed ID.

`circle-arc` has an accepted radius-12 disk (source seed radius 10) and a radius-5 quarter arc
closed by a straight chord using explicit endpoint `pointOnCurve` contacts. This exercises
solved geometry, production closure and interior arc samples. Reusing endpoint coordinates
alone does not establish the owned topological relationship.

Lines, polyline spans, circles and circular arcs are supported within complete production
profiles. Construction geometry is excluded. Open, ambiguous, self-intersecting, unsupported or
numerically uncertain profiles fail explicitly. Supported computed line/arc/channel boundaries
include the complete [manifold](../file-workspace-manifold/README.md); other computed forms
without an accepted analytic profile projection fail explicitly. Sampling has finite vertex/work limits and can refuse
precision targets that cannot be represented. The chord error bounds source-curve polygonization;
it does not include later Boolean, STL or manufacturing error. No geometry equations run in Node.

The [engine implementation](../../docs/M98_ENGINE_IMPLEMENTATION.md) records current APIs and
focused evidence. [M98_BAKE_HANDOFF.md](../../docs/M98_BAKE_HANDOFF.md) preserves the historical
prototype exports and their exact source identity.
