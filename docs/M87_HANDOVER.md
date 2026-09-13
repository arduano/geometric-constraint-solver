<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M87 managed controls and headless authoring: closure summary

M87 was accepted and closed on 2026-08-31. Qualified source
`32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`, passed the complete clean release gate.
Milestone-level acceptance includes M87-U9/U10; no separately logged visual replay
or immutable public deployment is claimed.

The original pause log has been consolidated here. Detailed API and regression
records remain in [implementation](M87_IMPLEMENTATION.md),
[headless authoring](M87_HEADLESS.md), [audit](M87_AUDIT.md) and [UAT](M87_UAT.md).

## Authority boundary to preserve

- Managed-control derivation, exact-CAS source edits, generated-consumer fan-out and outer history
  are optional `geosolve-sketch-code`/workbench concerns.
- Solver-instance point movement stays on the existing draft/semantic-overlay route and never
  writes solved coordinates into `sketch.ts`.
- Keyed reconciliation/topology, native materialization, computed Fillets and rendering retain
  separate owners. The reusable `cornerReliefs` patch knows only caller-owned keyed centres and a
  radius; it has no coupon-layout, widget, renderer or LOD knowledge.
- `geosolve-sketch-render` consumes independently accepted scene authority. It owns no solver,
  persistence, managed-control, selection or hit-ranking state.
- The entire experimental adaptive-detail/LOD prototype remains deleted. Camera navigation always
  retains the complete scene paint; completed M88 did not restore LOD.
- The CNC and Gridfinity entries are 2D/2.5D design sketches only. They claim no CAM, toolpath,
  automatic cutter compensation, boolean, solid, print-fit, machinability or production-
  manufacturing authority.
- Performance optimization was deferred to a subsequent milestone.

## Delivered M87 work

- A bounded transient managed-control manifest authenticates code-owned non-DoF source values,
  schemas, exact source spans and complete semantic consumer fan-out. Shared controls edit one
  token in one outer transaction; stale, foreign, dirty, wrong-unit, overlapping and over-bound
  batches fail closed.
- Inspector, Code panel, grouped generated-Fillet grip and strict code-control RPC use the same
  source authority while ordinary GUI controls and point overlays retain their previous owners.
- `geosolve-headless` provides browser-free inspect, exact-CAS edit and deterministic render APIs/
  CLI. `geosolve-sketch-render` provides shared target-neutral camera/SVG composition and native
  pure-Rust PNG with bundled resources.
- The tenth bundled project is the robotic routing board: eight keyed routes, 80 clips, 64 host
  Fillets, shared/local controls, one solver-instance service loop and keyed structural insertion.
- The eleventh project is the CNC joinery fit coupon: one 120 x 140 mm blank, three 70 mm-wide
  mortises with loose/nominal/press heights 18.4/18.0/17.6 mm, three 95 x 18 mm tabs, twelve
  shared-radius keyed corner-relief circles and ten handling Fillets.
- The twelfth project is one symmetric closed 26-point Gridfinity 1 x 1 x 3U central material
  section with reviewed 41.5/35.6 mm widths, 4.75/7 mm base levels, 21 mm body, 0.95 mm walls,
  4.4 mm nominal lip rise and independent two-floor/two-lip Fillet controls.
- Post-F003 accepted native inventories, in declaration/generated/output/point/curve/constraint/
  host-output/feature/computed-edge order, are CNC
  `(62, 69, 14, 29, 47, 36, 10, 10, 23)` with 33 dimensions and Gridfinity
  `(62, 66, 3, 31, 36, 31, 4, 4, 11)` with 18 dimensions. CNC owns exactly one `FixedPoint` and no
  `FixedCoordinate`; Gridfinity owns no `FixedPoint` and one Y `FixedCoordinate`.

## Manufacturing sample correction: M87-F003

CNC replaces seven unrelated `FixedPoint` locks with one absolute point and seven
relational construction datums. Gridfinity replaces a 26-point literal-seed contour
with one scalar Y datum, thirteen mirror relations and ten dimensioned orthogonal
projection spans. Nominal geometry, topology, generated Fillets and manufacturing
meaning remain unchanged. Both independently prove zero numerical and structural
nullity and zero equality and bidirectional bounded DOF.

Post-correction qualification passed `m87_manufacturing_sketches` 3/3, native
composition 13/13, the exact reviewed-ledger check 1/1, and all-demo headless and
deterministic products 10/10. The final clean release gate qualified the source above.
Earlier snapshots of the unconstrained samples are historical evidence only.

## Review coverage

M87-U9 covers the CNC loose, nominal and press-fit stations plus shared cutter and
handling controls. M87-U10 covers the Gridfinity section and independent floor and lip
controls. Exact conditions remain in [M87 UAT](M87_UAT.md); maintainer acceptance
covers both without asserting a separate replay.

No M87 implementation or acceptance work remains. For current development, use the
[documentation index](README.md).
