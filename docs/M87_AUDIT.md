<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M87 UX, code and headless-authoring audit

Status: **audit complete; M87 accepted and closed on 2026-08-31**. The pre-dogfood, routing-board
and original manufacturing dirty-worktree gates remain historical evidence for their exact pre-F003
sources. M87-F003 revises the CNC/Gridfinity sample authority and separately reviewed ledger. Exact
post-F003 source `32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`, passes the complete clean release gate. The user's
milestone-level close decision accepts U9/U10 without claiming a separate row replay. No immutable
freeze, public deployment or service retirement is claimed.

## Finding

At baseline `1ea4940`, Typed Panel declared one managed input,
`cornerFillets.radius: mm(4)`, which the pinned artifact routed unchanged into two generated
Fillets. Expansion authenticated the forward dependency but published no reverse property route.
The Inspector therefore attempted a nested Intent edit that the outer code session could not
classify, and terminal parity restored the code-owned radius.

This historical reproduction is **M87-F001**, an optional `geosolve-sketch-code`/workbench
authority defect, not a Fillet equation, solver, convergence, tolerance, or picking defect. The
implemented focused regression
`m87_f001_typed_panel_shared_radius_has_one_source_and_complete_fillet_fan_out` now proves that the
one radius-4 source owns both generated Fillet consumers and supplies the authenticated reverse
route. It records the repaired disposition; it is not a currently failing reproduction.

The later sample-authority review reproduced **M87-F003** in the two additive manufacturing
dogfoods. Gridfinity's 26 contour points were literal solver seeds with no native constraints, so
the apparent standard profile had no relational design authority. CNC had ordinary dimensions but
used seven unrelated `FixedPoint` locks to locate its components. This is a sample-authoring and
rank/DOF acceptance defect, not a solver equation, convergence, Fillet or rendering defect.

The repair preserves the accepted nominal coordinates, topology, generated Fillets and 2D/2.5D
manufacturing meaning. CNC now uses one `FixedPoint`, zero `FixedCoordinate` rows and seven
horizontal/vertical length-governed construction datums to locate every station relationally.
Gridfinity now uses zero `FixedPoint` rows, one Y `FixedCoordinate`, thirteen
`symmetricAboutDatumAxis` relations and ten orthogonal construction spans for its five diagonal
stages. Both accepted documents must report numerical and structural left/right nullity zero and
equality/bidirectional bounded DOF zero. Literal point coordinates remain seeds, not authority.
The post-F003 manufacturing owner suite passes 3/3, native composition passes 13/13, the exact
reviewed-ledger check passes 1/1 and all-demo headless/deterministic products pass 10/10. Fresh
mutable review bundles exist; U9/U10 are accepted by milestone-level close disposition.

## Product decision

Editability follows authenticated managed-source ownership, not optional artifact `EditLens`
declarations or browser special cases.

- Every explicit non-DoF semantic definition literal is a managed parameter: numeric/unit values,
  integers, booleans, closed enums, applicable text, dimension/constraint state, explicit branch
  choices, and computed-feature parameters.
- Point coordinates and other solver-instance/DoF values retain the existing draft/overlay model.
  Code never receives solved coordinates and M87 does not change point dragging.
- References, identities, structural keys, ordering, and `null` are not invented as editable
  values. They navigate to an owner or carry a typed read-only reason. An absent field has no
  parser-owned span, so the manifest does not fabricate a row for it.
- A shared literal remains one source value. Editing it updates every authenticated consumer in one
  transaction and discloses the complete fan-out.
- Custom patch source remains caller/AI-owned TypeScript. Rust/WASM consumes only pinned data-only
  artifacts and never executes or rewrites that source.

At the audited baseline, the parser already authenticated recursive object/array literal spans and
supported exact-CAS rewriting, while expansion already recorded direct and artifact template
bindings. The missing reusable contract was a bounded, transient managed-parameter/control manifest
joining source leaves to semantic consumers and their typed schemas.

The implemented disposition also adds `ManagedScalarBinding`, one deliberately narrow lexical
sharing form. Its right-hand side must be one finite number or unit literal and it may be referenced
only by later declaration arguments. Chains, aliases, general expressions, child paths, sketch
outputs and organization membership reject. PC Water Manifold uses exactly one
`channelBendRadius = mm(5)` binding for the upper, middle and lower `waterChannel` invocations; its
one control discloses and edits all three invocation-local consumer groups.

Canonical `CodeProject` JSON is also strict at its top-level authority boundary: an unknown
top-level field rejects rather than being silently ignored. Existing canonical fields still undergo
the ordinary complete source reparse and project validation.

`EditLens` remains artifact-v1-compatible presentation metadata only. It may label or rank a
proven independently editable input, but cannot create inverse-edit authority.

## UX and transaction decision

The code layer decorates the ordinary Inspector outside `geosolve-constraint-editor`; the editor
must not depend back on optional code concepts. A selected code-owned semantic property resolves
through accepted alias/selector provenance into a managed control. The Code panel also exposes all
managed parameters, including source parameters such as width/height that do not have a one-to-one
Inspector property.

Control edits use one exact-CAS batch and the outer `SketchCodeSession`. Accepted edits publish
source/native authority once. Representable invalid edits retain attempted source and diagnostic
over the prior accepted scene. Rejected/stale edits change nothing. A shared Fillet-radius gesture
previews all consumers transiently without parsing source on pointer frames, then performs the same
managed transaction on release.

Mutating Intent RPC is invalid while a code project owns authority. A separate code-control RPC
exposes inspect/edit/outer Undo/Redo without allowing nested Intent history to bypass code.

## Headless decision

At the audited baseline, the solve/scene pipeline was already DOM-free through `CodeProject`, keyed
reconciliation, `materialize_code_project_cold`, independently validated accepted authority, and
`ProjectionalEditorSession::scene`. Camera fitting, SVG composition, standalone export, native PNG,
and file orchestration were the missing boundaries now supplied by M87.

M87 adds:

- `geosolve-sketch-render`, shared by native and WASM for camera and SVG composition, with
  native-only pure-Rust PNG rasterization;
- `geosolve-headless`, a native library and stateless CLI for inspect, exact-CAS edit, solve,
  report, SVG, and PNG;
- managed-only source, canonical pinned `CodeProject` JSON, and bundled demo inputs;
- deterministic digest-derived IDs, fitted authoritative canvas output, and atomic generation
  directories which never overwrite prior output on failure.

The CLI never starts a server/browser, uses a DOM/network/system fonts, invokes Node, or evaluates
TypeScript. Source control remains durable history; a stateful JSONL service is outside M87.

## Routing-board dogfood amendment

The tenth bundled project deliberately crosses the existing architecture at composition seams
rather than adding a routing subsystem. One 360 x 220 mm fixture board carries eight keyed open
Polyline harnesses. The data-only `harnessRoute` artifact maps their vertices to 80 clip circles and
their current interior corners to 64 existing host Fillets. One clip-radius literal and one
bend-radius literal are genuinely shared; one invocation can localize either input without
coupling the other routes. Interior points remain solver-instance overlays, and keyed insertion
continues through ordinary reconciliation generations.

The dogfood exposed two owning-layer problems which broad visual or browser work would have hidden:

- aggregate open-chain validation had called whole-scene visual-profile arrangement. Deliberate
  clip/route intersections could therefore truncate an unrelated open chain. Aggregate intent
  validation now uses exact endpoint/shared-point/active-Coincident connectivity; Offset operands
  retain complete visual-profile authority where arrangement is actually the semantic contract;
- noninteractive composition previewed and published each of 64 Fillets separately. Code
  composition now builds and translates exact candidates independently, then publishes all host
  nodes in one atomic patch (plus one optional suppression patch). Interactive Fillet preview and
  one-feature ownership remain unchanged.

These corrections keep control derivation, keyed topology, point overlays, feature composition and
rendering modular. In particular, a service-loop drag neither parses nor rewrites source, a shared
control edit does not invoke overlay logic, and the renderer consumes only independently accepted
scene authority. Repeated headless board renders directly freeze report/control/logical-scene/SVG/
PNG equality without giving raster bytes solver authority.

## CNC and Gridfinity dogfood amendment

The eleventh and twelfth bundled projects deliberately reuse the same composition seams rather
than broadening the managed grammar or introducing a manufacturing subsystem.

- **CNC joinery fit coupon** composes one fully constrained 120 x 140 mm blank, three 70 mm-wide mortises
  with loose/nominal/press heights of 18.4/18.0/17.6 mm and three 95 x 18 mm tabs. The small
  `cornerReliefs` patch is a data-only `mapRecord` from caller-owned keyed point centres to ordinary
  circles. Its shared 3.175 mm radius owns twelve consumers; the existing record-Fillet patch owns
  four blank and six tab handling corners at 6 mm. Invocation localization and keyed generation
  tests remain patch/reconciliation concerns, independent of tabs, dimensions and rendering. One
  absolute point plus seven relational construction datums replace the former seven unrelated
  component locks.
- **Gridfinity 1 x 1 x 3U section** is one explicit symmetric closed 26-point material contour,
  not a solid. It records the 41.5 mm outer and 35.6 mm base-bottom widths, 4.75 mm staged base,
  7 mm cavity floor, 21 mm body, 0.95 mm walls and 4.4 mm nominal stacking-lip rise. Two 2.8 mm
  floor Fillets and two 0.6 mm lip Fillets reuse the existing selective record patch and retain
  independent controls. Thirteen live mirror relations, five dimensioned diagonal projection
  stages and one scalar base-height datum replace the former unconstrained literal-seed contour.

Their post-F003 accepted inventories, in declaration/generated/output/point/curve/constraint/
host-output/feature/computed-edge order, are CNC
`(62, 69, 14, 29, 47, 36, 10, 10, 23)` with 33 dimensions and Gridfinity
`(62, 66, 3, 31, 36, 31, 4, 4, 11)` with 18 dimensions. CNC owns exactly one `FixedPoint` and no
`FixedCoordinate`; Gridfinity owns no `FixedPoint` and exactly one `FixedCoordinate`.

The corner circles are authored conservative dogbone-style overcuts, not inferred cutter
compensation. Both projects are 2D/2.5D manufacturing-intent drawings only; neither proves CAM,
toolpaths, booleans, solids, print fit or machinability. Focused owner, twelve-demo native/headless
inventory, deterministic render and reviewed-ledger qualification remain historical pre-F003
dirty mechanical evidence. The complete dirty-worktree release gate also passed at exit `0` on
2026-08-30 before F003; its exact command is recorded in `docs/M87_IMPLEMENTATION.md` and does not
qualify the revised sources. The reviewed twelve-row code-project ledger has SHA-256
`f6ecd037cef8befc59f9a057fef499a14f0851f8ec5a0d3a1468a69e66a9d1bc`. The bundles under
`/tmp/geosolve-m87-manufacturing-uat.K7YRaV/` are historical pre-F003 evidence and must be replaced
for current source review. Exact post-F003 source `32c7289` passes the complete clean gate, and the
user's milestone-level decision accepts both rows without a separate replay. Diagnosed Gridfinity
performance/stack work is carried into active M88's stability prerequisite.

## Prior scoped-disposition graphics audit and M87-F002

The prior scoped-disposition graphics audit preserved the extracted `geosolve-sketch-render`
boundary, frozen
browser/composer bytes, current paint order and headless accepted-scene authority. The web
`scene`/`icons` modules are thin shims over that crate, native rasterization remains a bounded
pure-Rust `resvg` consumer behind `cfg(not(target_arch = "wasm32"))`, and neither renderer owns
solver, persistence, selection or hit-ranking authority. The previously reported z-ordering and
Fillet/endpoint arbitration problems have owning editor/adapter regressions and were not
reproduced in this audit, so M87 deliberately makes no further paint-stack rewrite.

The same audit independently reproduced two fail-closed defects at the new renderer owner and
records them together as **M87-F002**:

- `CanvasCamera::pan_from` accepted finite screen inputs whose subtraction overflowed and could
  publish a non-finite model centre. The exact owning regression
  `scene::tests::camera_pan_rejects_finite_overflow_without_mutation` failed before repair and now
  proves transactional rejection. Camera fields are private, construction is validated, public
  reads use getters, and pan/zoom publish only a complete finite candidate.
- the then-public grid-path helper accepted negative, zero or pathologically small spacing; its
  additive loop could fail to advance or run without a useful bound. A timeout-isolated pre-repair
  reproduction exited `124`. `scene::tests::grid_path_rejects_invalid_or_unbounded_work` now
  proves rejection of negative, zero, NaN, infinite-extent and non-advancing tiny work. The helper
  is private, returns `Option`, validates its inputs and stops after at most 4,096 total lines.

Both defects are isolated presentation-owner cases, not a missing golden authoring dimension, so
the milestone-neutral golden matrix is unchanged. Normal shared-composer bytes retain digest
`bd364332a1c4c1024aa17434ec862d5e4968124a2e8cb09829b04c677b898605` after the repair.

The audit defers broader, non-blocking cleanup: replacing wildcard renderer re-exports, collapsing
the SVG overload family into one request object, deduplicating browser/static style constants and
adding arbitrary-SVG complexity policy beyond the current byte/pixel/resource bounds. Those are
reviewable future API changes, not safe work for the current M87 visual-UAT boundary. Any future navigation optimization must
wrap the projectional and flat presentation boundaries and remain outside the authority-only
`RetainedCameraQueue`.

## Main risks and controls

- Freeze existing composer bytes before renderer extraction and require browser/shared parity.
- Bound controls and source-consumer edges independently so a valid large project cannot force an
  unbounded transitive manifest.
- Derive manifests and tokens transiently; do not change persisted expansion/session wire digests.
  The explicit `ManagedScalarBinding` parse node is durable managed-source authority, not transient
  token material.
- Reject unknown top-level `CodeProject` fields before they can be mistaken for additional
  authority.
- Compare expected floating values by IEEE bits, including signed zero, and reject mixed/stale or
  overlapping batches before rewrite.
- Treat SVG/report as exact deterministic authority. PNG must have deterministic inputs, dimensions,
  and semantic pixels but is not a mathematical or cross-platform byte oracle.
- Promise atomic no-clobber headless directory publication only on Linux, Android, Apple platforms
  and Redox. Other targets fail closed until they have a proven atomic directory primitive.
