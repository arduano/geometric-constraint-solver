<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M89 goals: executed, reversible managed sketches

Status: **Superseded by M90's closed typed clean-break contract.** M89-F004/F005 implementation,
provisional dirty-tree mechanical qualification and its immutable F005 nomination completed, but
the Compass Rose retest, targeted manual preflight and M89-U1 through M89-U8 were never run and are
not retrospectively passed or waived. Every M89 snapshot is historical evidence; M90-F006 is the
current product/UAT authority.

M89 makes managed `sketch.ts` a reversible source of the complete standalone-authoritative sketch;
host-external relations whose immutable binding cannot yet be expressed remain gated. Syntax is
captured by a closed-subset parser; semantics and edit provenance come from instrumented execution;
Rust remains the sole materialization, solver, validation and publication authority.

## Required outcomes

- Introduce versioned reversible `ManagedSketchIrV2`, its normalized printer and a deterministic
  `ExecutedSketchArtifactV2` with complete declaration results and value-consumer provenance.
- Reject unsupported managed syntax before execution and use injected source-site IDs, not stack
  trace guessing, for durable navigation and edits.
- Remove the need for v2 `p.editLens` and the terminal managed `$.outputs(...)` declaration while
  retaining an isolated, byte-preserving v1 compatibility path.
- Make every enabled canvas authoring recipe whose complete authority is expressible in standalone
  managed source source-backed; gate rather than approximate a host-external recipe whose authority
  is not representable. One completed gesture must atomically publish regenerated source, IR,
  artifact, independently accepted scene and one outer history row. The gesture owns one user-
  facing declaration closure: ordinary recipes use one declaration; a multi-declaration recipe
  such as Profile Offset retains authenticated, source-visible helper declarations under the
  lifecycle of its user-facing root.
- Keep canvas-authored geometry concise across the complete 25-variant catalog. Segment uses
  `$.geometry.line(...)`, exact native Polyline uses `$.geometry.polyline(...)`, and the remaining
  23 variants use lossless `$.geometry.recipe(...)`; transport-level `$.intent.recipe(...)` is not
  the authored geometry surface. Rust must independently derive and authenticate every recipe
  input, field, writable value and semantic result against the central Intent descriptors. The
  TypeScript layer records data and references only and owns no geometry equation.
- Give the persistent constraint catalog the same honest source boundary. Of 35 persistent
  `ConstraintKind` variants, 33 must cold-replay from standalone managed source: Horizontal and
  Vertical retain direct builders, while the other 31 use descriptor-authenticated
  `$.constraint.recipe(...)`. The two host-external variants must fail closed until immutable
  external snapshot/binding authority can be represented in `sketch.ts`.
- Project a canvas-authored computed Fillet as one direct `$.computed.filletSet(...)` declaration
  with lexical native-span parents, exact parameter/winding/neighborhood/normal-side/retained-
  endpoint/periodic-anchor/endpoint-order/sweep state, radius, display name and explicit source-
  owned suppression. Cold replay and every source lifecycle operation must retain computed
  ownership and explicit branch state.
- Restore ordered top-level declaration management with nested generated members, genuine source
  reorder and explicit source-owned suppression.
- Add a visible `Canvas additions` group and persisted non-reusing declaration-name allocator.
- Provide equivalent browser and pinned-Deno source compilation. Keep canonical compiled-project
  inspect/solve/render entirely browser-free and Deno-free in pure Rust.
- Preserve checked-in legacy samples unchanged; upgrade only an active copy on its first structured
  source edit. Reject legacy persisted source/GUI hybrids that cannot be represented completely.

## Exclusions

- This milestone does not normalize the checked-in legacy sample corpus or remove managed-v1
  compatibility. M90 is the planned clean break for that work; it is not activated or partially
  implemented by M89.
- `.patch.ts` remains a liberal executed extension and is not reconstructed from managed IR.
- M89 adds no solver equations, tolerances, branch inference, geometry family or browser-owned
  semantic authority.
- `ExternalPointCoincident` and `ExternalLineCollinear` remain host-authority-gated. Their wire
  schemas remain compile-visible and schema-authenticated, but they require separately supplied
  host snapshots. F004/F005 implement no standalone host-snapshot execution path, so they are not
  counted among the 33 standalone-replayable constraint variants.
- F004/F005 add no Offset work. The previously implemented Profile Offset closure remains
  unchanged, and no broader Offset claim is introduced.

## Implemented outcome

The required outcomes above are implemented. Browser and pinned-Deno compilation agree on
normalized source, IR, execution artifact and digests; already-compiled projects inspect, solve
and render in pure Rust. Exact source-site and runtime value-consumer provenance replaces
managed-v2 `p.editLens`, and v2 has no terminal `$.outputs(...)`. Every canvas recipe admitted to
standalone source projects into managed source. The complete 25-variant geometry palette uses the
compact three-way contract above: direct Segment, direct exact-native Polyline, and descriptor-
authenticated `$.geometry.recipe(...)` for Sketch Point, Midpoint Line, every rectangle, circle,
arc, ellipse, elliptical arc, Bezier, conic and NURBS variant. Source/IR reorder, suppression, deletion,
active-copy v1 upgrade, compiler-failure correction, reload and outer Undo/Redo all retain the
atomic authority contract.

Of the 35 persistent constraint variants, 33 now cold-replay from standalone managed source:
Horizontal and Vertical remain direct, while the other 31 use
`$.constraint.recipe(...)`. `ExternalPointCoincident` and `ExternalLineCollinear` are deliberately
gated rather than serialized without their missing host authority. Canvas Fillet authoring now
emits one direct `$.computed.filletSet(...)`; cold replay preserves computed ownership and exact
branch/contact state, selection navigates to the whole authenticated declaration, radius edits
retain selection, and suppress/restore plus delete/Undo reproduce exact source. Final provisional
dirty-tree qualification passes the pinned complete gate, including sketch-code unit `114`,
geometry `4/4`, constraint matrix `1/1`, direct Fillet `7/7`, editor insertion `19/19`, demo-web
`358/358`, TypeScript runtime `28/28`, mutation `19/19`, Deno parity `2/2` and frontend `55/55`.
The historical immutable F005 candidate is `/tmp/geosolve-m89-f005-uat.hzNuDxF0`, aggregate
`fb488ad2bf29e8897cf9811c002b748693e5d211bae4bb54c83ed060db5db668`, served at
`http://100.94.63.83:18089/`. This is not clean-source qualification or human acceptance. The
Compass retest, targeted preflight, M89-U1 through M89-U8 and explicit closeout remain pending/not
run and are owned by `docs/M89_IMPLEMENTATION.md` and `docs/M89_UAT.md`.
