<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M43 implementation record: immutable external 2D references

Status: implemented and qualified (2026-07-27).
This file owns the M43 working contract only. It does not prescribe M44 UI work or
M54 stable-diagnostic compatibility.

## Completion evidence

- Document-local monotone bindings, closed point/directed-line snapshots, explicit
  external operands and point-coincidence/line-collinearity constraints implement the
  deliberately narrow v1 contract below.
- `ExternalSnapshotSet` canonicalizes and independently validates revision/digest,
  feature kind, topology, orientation, finite geometry, scale, domain and bounded
  resources before any lowering or solver work.
- Retained attempts stamp the exact snapshot set, resolve M41 activity closure, lower
  external geometry only as fixed coefficients, independently validate accepted rows,
  expose structured audit provenance and publish atomically.
- Draft-v5 round-trips M43 state while supported sketch v1-v4 remains frozen. The
  unstable scene capsule reconstructs exact design/parameter/activation/snapshot inputs
  through the ordinary retained-session path and never imports stored status or geometry
  as authoritative.
- `crates/geosolve-sketch/tests/m43.rs` contains ten focused regressions, including the
  external-line finite-difference Jacobian test. Independent review found no concrete
  defect and passed every M43 checkbox, gate and acceptance bullet.
- Qualification passed formatting and diff checks, warnings-denied locked workspace
  Clippy, full locked workspace tests, locked WASM, release Trunk and browser E2E. The
  recurring Cargo `license`/`license-file` warnings predate M43.

## Requirements

- Persist a stable **local** external-binding identity and its expected closed feature
  kind (`PLAN.md:1908`). Host/PDM keys, topology names, projection recipes, 3D
  geometry, units, configuration and rebinding policy remain host sidecars, never
  canonical sketch state or equations (ADR 0026:19-42; `PLAN.md:1913`).
- An attempt accepts exactly one bounded, immutable, finite external snapshot set
  before lowering. Its set revision and canonical digest are recorded in the exact
  attempt and accepted input stamps. Evaluation, Jacobians, validation, diagnostics,
  measurements and profiles make no host call, callback, lazy fetch, resolver or
  mutable shared-object read (ADR 0026:44-56; `PLAN.md:1909,1916-1917`).
- Snapshot entries must carry source revision/digest, geometry, parameter domain,
  orientation, scale and kind-specific resource evidence. Missing, stale, duplicate,
  wrong-kind, non-finite, oversized, malformed-domain/regularity, and
  topology-incompatible inputs are typed unsolved-design outcomes; they never become
  accepted geometry (`PLAN.md:1909-1912`; ADR 0026:78-95).
- External geometry is a fixed coefficient. It has no core variable, tangent
  coordinate, accepted unknown, hidden editable native copy, or proximity-based
  identity repair. Native unknowns alone solve against it; independent hard/domain/
  branch validation still gates acceptance (ADR 0026:86-90; `ARCHITECTURE.md:250-256`).
- Family, span identity, topology or orientation-contract changes require a retained
  design rebind/remap transaction. Equal/near coordinates never select a replacement
  or branch (`PLAN.md:1911`; ADR 0026:106-111; `ACCEPTANCE.md:755-757`).
- Preserve ADR 0025's three identities. A failed external input gets a never-reused
  attempt tied to its exact stamp, may retain diagnostic/candidate evidence, and leaves
  parent accepted bytes, audit and input stamp unchanged. Only a fresh independently
  valid candidate publishes an accepted revision (ADR 0025:22-100).
- Frozen sketch v1-v4 fields, variants, readers, writers and canonical v4 bytes remain
  unchanged. Supported v4 export rejects non-default M43 state; a private unsupported
  draft-v5 document DTO and a separately versioned external-set envelope may evolve
  until M61 (ADR 0025:102-117; M41 inventory:22-28; M42 inventory:66-69).
- A diagnostic capsule may bundle design, parameter batch, activation input and exact
  external set for reproduction, but is diagnostic exchange only: saved status,
  attempted geometry and host identity never become authoritative persistence
  (`PLAN.md:1914`; `docs/SCENARIOS.md:1365-1372`).

## Evidence and source pointers

- M43's seven checkboxes and gate are authoritative at `PLAN.md:1901-1917`; the three
  acceptance outcomes are `ACCEPTANCE.md:753-757`. The UAT point captured at M45 and
  relocated to M53 needs visible stale/missing/valid external-reference recovery over prior accepted geometry, but its
  revision/digest/atomicity claims remain automated (`docs/SCENARIOS.md:1339-1345`).
- ADR 0026 allocates immutable external snapshots, digest evidence and explicit
  rebinding specifically to M43 (`docs/adr/0026-immutable-host-inputs-and-external-snapshots.md:78-124`).
  ADR 0025 requires document/design, parameter, external-set, activation and policy
  revision+digest members in every eventual complete stamp (same ADR:65-87).
- Current M41 reserves the reason vocabulary: `InactivityReason::UnavailableExternalReference`
  and `HostActivationOverride::UnavailableExternalReference` (`document.rs:1172-1186,
  1278-1307`). M41 requires one ordered effective-activity closure before lowering,
  profiles, branch or ownership consumers; M43 must replace the temporary host override
  path with snapshot-derived unavailability without adding a fifth public reason
  (`docs/M41_IMPLEMENTATION.md:30-34,59-70`).
- M42 establishes the exact precedent: `ParameterBatch` sorts IDs, rejects duplicates
  and non-finite values, hashes version/revision/typed exact `f64` bits, and has one
  canonical empty value (`document_session.rs:31-208`; `docs/M42_IMPLEMENTATION.md:71-83`).
  `SketchAttemptInput` currently carries activation and parameter stamps and repeats it
  on attempts/accepted states (`document_session.rs:573-660`).
- Lowering already receives an immutable resolved parameter/activity view, excludes
  inactive geometry before runtime allocation, and works on scratch state
  (`document_lowering.rs:332-399`). Retained attempts validate inputs before lowering
  and classify failures without publication (`document_session.rs:3325-3418`).
- Existing compiler mappings relate a domain source to zero/one core source and expose
  audit ownership (`compiler.rs:87-104`). Semantic catalog validation freshly compiles,
  audits finite rows and checks template ownership (`semantic.rs:2833-2905`). These are
  the audit/validation seam—not a reason to duplicate residuals.
- `DocumentElementId` deliberately names persistent graph objects but never runtime IDs
  (`document.rs:1130-1158`). `CurveSpan`, contacts and existing point/curve operand
  records use native persistent IDs (`document.rs:502-645,786-819`), so M43 needs an
  explicit parallel external operand identity rather than pretending an external value
  is a `CurveId` or `DesignPointId`.
- The web crate is a replaceable public-API consumer, has no equations and always
  renders accepted geometry/audit from the same result (`ARCHITECTURE.md:200-222`).

## Decisions / inferred constraints

### Small public and persistent vocabulary

Proposed names (all exported from `lib.rs`; no generic provider trait):

| Purpose | Proposed small API |
| --- | --- |
| local retained identity | `DocumentExternalBindingId`, `DocumentExternalBinding { id, label, expected_kind, expected_topology }`, `ExternalFeatureKindV1` |
| binding mutation | `SketchDocument::add_external_binding`, `SketchDocument::rebind_external_binding`; `DocumentEdit::{AddExternalBinding, RebindExternalBinding, RemoveExternalBinding}` |
| immutable input | `ExternalSnapshotSet`, `ExternalSnapshotEntry`, `ExternalSnapshotSetDigest`, `ExternalSnapshotDigest`, `ExternalSnapshotFeatureV1` |
| semantic operands | `DocumentExternalPointRef { binding }`, `DocumentExternalLineSupportRef { binding, direction }`; use them only in explicit M43 constraint variants, never overload native IDs or broaden every existing point/support consumer |
| resolved/lowered proof | crate-private `ResolvedExternalSnapshots` and `ExternalRuntimeMapping`; public audit records expose binding and snapshot provenance, not runtime IDs |
| typed outcome | add `ExternalSnapshotInput` to `SketchAttemptFailureKind` (or parent-approved closed replacement of the M42 name), with machine-readable `ExternalSnapshotInputError` reason plus binding ID |

`DocumentExternalBindingId` should use the existing monotone persistent allocator, be a
`DocumentObjectId`/`DocumentElementId` variant, never be reused, and participate in
canonical persistent ordering. Binding identity is document-local. A rebind changes
only the binding declaration by an explicit retained design transaction; it does not
mutate a historical snapshot or infer a host replacement. Binding records persist in
the unsupported draft-v5 document DTO; supported v1-v4 import defaults to no bindings,
and v4 export rejects any binding/operand external state rather than dropping it.

### Closed external snapshot language v1

Use a separately versioned, self-contained, serde DTO envelope with
`deny_unknown_fields`; do **not** serialize native `CurveDefinition`, since that type
contains editable local IDs/scalars. Proposed set form is:

```text
ExternalSnapshotSet { version: 1, revision: u64, digest, entries: Vec<ExternalSnapshotEntry> }
ExternalSnapshotEntry {
  binding: DocumentExternalBindingId,
  source_revision: u64, source_digest, feature: ExternalSnapshotFeatureV1
}
```

Canonicalization sorts by local binding ID, rejects duplicate bindings before hashing,
and hashes a version tag, set revision, ordered entries, source evidence, enum tags and
exact IEEE-754 bits. Reconstructing from claimed digest recomputes and compares it.
`Default` is the one canonical empty set at revision 0/digest(empty); non-empty sets
require positive revision. A session rejects a lower revision; same set revision with a
different digest is invalid, and a matching digest never permits an older revision to
replace retained input. Source revision/digest identify the host-provided feature bytes
within the set; they are evidence, not host/PDM lookup keys.

The parent-approved **closed** `ExternalFeatureKindV1` /
`ExternalSnapshotFeatureV1` alternatives are deliberately only:

1. `Point { position, scale, resources }`; and
2. `LineSegment { start, end, domain: [0, 1], orientation: StartToEnd, scale,
   topology_digest, resources }`.

Every case carries finite coordinates/scalars and positive finite `scale`. The line
also carries the explicit canonical parameter domain, direction contract and stable
topology digest expected by its local binding; changing the topology digest or feature
kind requires an explicit rebind. The envelope carries bounded resource evidence:
declared `point_count`, `control_count`, and `span_count` must exactly match `(1,0,0)`
for a point or `(2,0,1)` for a line segment and remain under fixed M43 limits.
Validation rejects degenerate direction, invalid domain, non-finite values, bad
orientation, claimed-count mismatch or excess before lowering. This two-kind language
is the smallest closed proof of both required point and curve snapshots. Circles,
arcs, Beziers, conics and splines remain later closed-language additions, not a plugin
escape hatch.

### Stamps, activation, absence and ordering

Extend `SketchAttemptInput` with `external_snapshot_set_revision: u64` and
`external_snapshot_set_digest: ExternalSnapshotSetDigest`; every
`SketchDocumentAttempt`, `SketchAcceptedDocumentState`, accepted audit/proposal and
diagnostic capsule repeats the same captured value. Capture/validate the complete
`ParameterBatch`, `ExternalSnapshotSet`, activation closure and solver policy once,
before activity closure/lowering. Do not fold external digest into parameter or
activation digests. At synchronous publication compare design identity, parameter,
external, activation and solver-policy members exactly; any mismatch makes the attempt
stale/non-publishable while retaining its evidence. This is the M43 synchronous
foundation, not M55's prepared-job API.

Availability is resolved before M41 closure: a declared external binding that lacks one
valid, exact-kind entry contributes `UnavailableExternalReference` to that binding and
its typed consumers; M41 then propagates `UnavailableDependency` to dependents in
canonical element order. A user/host-inactive element remains governed by M41's existing
precedence; external validity must not reactivate it. Validate set shape and revision
evidence first, then binding completeness/kind/topology, then derive activity, then
require values only for active external consumers. A malformed/duplicate/set-stale input
is an atomic input failure; a well-formed set missing a declared active binding is an
unsolved attempt whose activity/audit names unavailable external reference. This ordering
prevents lowering or profile side paths from observing invalid geometry.

### Lowering, audit and provenance

Lower each validated external feature into immutable coefficients owned by the
lowering scratch object—not into `Sketch::add_named_*`, a `DesignPoint`, a `CurveId`,
or any solver variable. The initial executable surface is exactly (1) coincidence of
one native point with one external point, reusing the existing point-target evaluator,
and (2) collinearity of one native line support with one fixed external line support,
using the existing two-row collinearity mathematics with native-variable incidence
only. The runtime constraint/audit variants are external-specific so provenance is not
mislabelled as an authored fixed point or a second native line. No hidden fixed point,
segment, contact scalar or editable shadow geometry is allocated.

Audit source mappings remain semantic-source owned. Add a structured external operand
provenance record to every affected source/row: local binding ID, expected/actual v1
kind, set revision/digest, entry source revision/digest, domain, orientation and scale.
The accepted audit identifies the accepted-state complete stamp; attempt audit is
explicitly non-authoritative. Independent validation re-evaluates against the same
resolved immutable feature bytes. Diagnostic capsules may include canonical input
envelopes and their digests, but not arbitrary host keys or stored acceptance status.

### Typed failure and atomic-retention matrix

| Condition before/during attempt | Typed result | Design / attempt | Accepted state and outputs |
| --- | --- | --- | --- |
| malformed set, duplicate entry, non-finite, resource/domain/orientation error, wrong document/revision evidence | `ExternalSnapshotInput` with structured reason | valid retained design; allocate attempt | unchanged |
| lower set revision or same revision/different digest | stale/invalid external input | retain set and design; attempt only if input reached attempt boundary | unchanged |
| missing active binding, wrong expected kind, family/span/topology/orientation incompatibility | typed unsolved external-reference outcome; activity reports `UnavailableExternalReference` | retain intent and attempt evidence | unchanged |
| valid snapshot, solve/rejection/cancel/work exhaustion/independent validation failure | existing typed solve/control failure | retain attempt/candidate if finite | unchanged |
| input stamp changes before publish | stale publication outcome (M43 synchronous check) | attempt remains inspectable | unchanged |
| valid exact set plus independently hard/domain/branch-valid solve | accepted publication | new attempt and accepted identity | atomically replace geometry, audit, complete stamp and M42 proposals |

No failure edits history/accepted bytes partially. Attempts are never reused; all accepted
output proposals stay attached to their old accepted stamp until a new accepted solve.

## Implementation slices with disjoint file scopes

1. **Persistent identity and draft DTO — `document.rs`, `lib.rs` only.** Add local
   binding ID/declarations, external operand DTO variants, validation/rebind edits,
   `DocumentElementId` integration, draft-v5 fields/codec and supported-v4 rejection.
   Characterize v1-v4 bytes. No session, lowering, core or web change.
2. **Immutable input/stamp resolution — `document_session.rs`, `lib.rs` only.** Add
   canonical external set/digests, revision-staleness checks, resolved snapshot view,
   exact `SketchAttemptInput` propagation, typed failure payload and atomic retention.
   Preserve M42 batch stamps and M41 closure semantics.
3. **Activity/lowering/audit — `document_lowering.rs`, `model.rs`, `curves.rs`,
   `residuals.rs`, `compiler.rs` only.**
   Resolve missing/incompatible bindings into M41 closure before allocation; lower fixed
   point and line coefficients through external-specific runtime variants; add
   provenance and independent validation. The fixed-line evaluator is a coefficient
   specialization of existing collinearity mathematics and requires its own FD test;
   it adds no core variable, hidden geometry or callback trait.
4. **Diagnostic capsule boundary — `crates/geosolve-demo-web/src/playground.rs` only;
   do not alter sketch v1-v4 codecs.**
   Add optional canonical parameter/activation/external input bundle and exact stamps;
   reject bad capsule input without treating capsule status as authority.
5. **Qualification — `crates/geosolve-sketch/tests/m43.rs` only**, plus narrowly needed
   existing focused regression edits. No `geosolve-demo-web` implementation: M44 owns
   presentation/rebind workflows.

## Focused test and acceptance matrix

| M43 requirement/gate | Focused M43 test evidence |
| --- | --- |
| persistent stable local binding / expected kind | allocation never reuses ID; draft-v5 canonical round trip; v4 rejects non-default binding; host key absent |
| finite point/curve snapshot with revision/digest/domain/orientation/scale/resources | each v1 kind canonicalizes order/IEEE bits; claimed-digest mismatch, NaN/Inf, bad domain/orientation, bad scale and each limit reject |
| exact immutable set / no callback | solve twice with same captured bytes gives identical geometry/audit/stamp; mutate host-owned source after construction has no effect; API has no callback/provider field |
| typed operand, no unknown/hidden copy | native-point/external-point coincidence and native-line/external-line collinearity have native-variable incidence only, no external runtime variable or hidden geometry, and audit names source provenance; FD Jacobian checks the fixed-line specialization |
| explicit rebinding | changed family/span/topology/orientation fails until `RebindExternalBinding`; equal/near geometry cannot repair; old accepted geometry/stamp survives |
| missing/stale/duplicate/wrong-kind/non-finite/oversized/incompatible | table-driven typed reason tests; all retain design/attempt/accepted separation and M42 proposals |
| M41 activation ordering | missing external gives the existing unavailable-external reason; dependents get unavailable-dependency; user/host inactive precedence and reactivation preserve discrete bytes exactly |
| complete stamp / publication | attempt and accepted repeat exact external revision/digest alongside activation+parameter; changed set before commit is stale and cannot publish |
| audit and capsule | attempted versus accepted provenance is distinct; canonical capsule reproduces input/audit but stored status cannot publish; arbitrary host/PDM key absent |
| M43 gate and acceptance | one attempt consumes one set with exact evidence/no callback; malformed/stale/topology-incompatible inputs retain truth; explicit rebind/no proximity repair (`ACCEPTANCE.md:753-757`) |

Existing regression anchors are M41 activity/retention/reactivation
(`tests/m41.rs:81-100,250-472`), M42 canonical batch/stamp/stale retention
(`tests/m42.rs:75-232`), M36 fixed-scalar/Jacobian audit coverage, and M38 independent
measurement/audit provenance coverage. New residual instantiations require their own
finite-difference characterization even when they reuse a residual family.

## Architecture decision investigation

### Recommendation: smallest coherent M43 v1

Adopt a closed external language with two declared binding kinds and two finite snapshot
feature alternatives:

| Binding / snapshot feature | M43 v1 decision |
| --- | --- |
| `Point` | `Point { position, scale, resources }` |
| `LineSegment` | `LineSegment`, with one explicit directed `[0,1]` domain/span, `StartToEnd` orientation, positive scale, exact resource counts and stable topology digest |

The binding records the expected feature kind and, for a line segment, its stable
topology digest and orientation contract. Thus a family, span/topology or orientation-
contract change cannot be mistaken for an ordinary coordinate update and requires
`RebindExternalBinding`.

The exact initial external semantic operands and consumers are deliberately only:

1. `DocumentExternalPointRef { binding }` in
   `DocumentConstraintDefinition::ExternalPointCoincident { point, external }`; and
2. `DocumentExternalLineSupportRef { binding, direction }` in
   `DocumentConstraintDefinition::ExternalLineCollinear { line, external }`.

These explicit variants avoid adding an external arm to ubiquitous
`DocumentPointRef`, `DocumentLineSupportRef`, measurements and M37 consumers whose
lowerers currently assume native persistent geometry. The external point is a fixed
target coefficient. The external line is a fixed directed support coefficient and the
native line alone supplies solver incidence. This proves both required external feature
classes and the typed operand/audit seam without a generic provider, hidden native
entity, contact scalar or new branch protocol.

Do **not** initially add external arms to `DocumentPointRef`,
`DocumentDirectionRef`, native `DocumentLineSupportRef`/`DocumentCurveSpanRef`,
curve-curve contact/tangency, external measurements, or external circle/Bezier/conic/
spline/NURBS families. Those require broader consumer matrices, contact/neighborhood or
one-sided-span policy and are not necessary for M43's point/curve snapshot gate.

### Codec, capsule, and revision decisions

- **External-set codec:** expose `ExternalSnapshotSetV1` and its canonical
  encode/decode/digest API publicly, but mark the API and wire language explicitly
  **unstable until M61**.  It is a separate, versioned envelope, not draft-v5 document
  JSON and not supported sketch v1-v4 JSON.  It uses strict version dispatch and
  `deny_unknown_fields`; entries canonicalize by local binding ID and hash version, set
  revision, entry source evidence, variant/domain/orientation/scale/resource evidence,
  and exact finite `f64` bits.  This is required for a host to construct reproducible
  immutable input and for a capsule to carry exact input; no stable compatibility claim
  is made before M61.
- **Diagnostic capsule boundary/file:** keep the existing private, disposable scene
  capsule producer/parser in `crates/geosolve-demo-web/src/playground.rs` as the capsule
  boundary.  Extend its `DecodedSceneCapsule` envelope only with canonical public
  `ParameterBatch` and `ExternalSnapshotSetV1` payloads/digests and captured input
  stamps; on import it must rebuild inputs and enter the ordinary public
  document-session validation/attempt path.  It may carry display evidence, but it must
  not import stored solve status, attempted geometry, accepted state, host keys, or any
  authority to publish.  This changes diagnostic interchange only, not M44 tree/status/
  rebind UI and not M54 stable diagnostic DTOs.
- **Source revision:** use `u64` `source_revision` plus a 32-byte canonical
  `source_digest` for each entry.  The set itself likewise uses `u64` revision plus
  digest.  Revision supplies monotone stale ordering; digest supplies exact feature-byte
  identity.  An opaque host revision string is neither needed nor desirable: it imports
  host/PDM identity into the sketch boundary and adds no stale/reproducibility property
  beyond the required revision/digest pair.

### Evidence, rationale, and rejected alternatives

- `PLAN.md:1905-1917` requires immutable finite **point/curve** snapshots, typed
  operands/audit without variables, explicit topology rebinding, and reproducible
  capsules; it does not require every built-in curve family or every relation consumer.
  `ACCEPTANCE.md:753-757` likewise gates one exact set/no callback, truthful unsolved
  retention, and explicit non-proximity rebinding only.
- ADR 0026:78-111 requires a *closed* versioned feature language, local bindings,
  per-entry revision/digest/domain/orientation/scale/resource evidence, fixed
  coefficients, and rebind on family/span/topology/orientation change.  ADR 0025:65-88
  requires separate external-set revision/digest members in immutable attempt and
  accepted stamps; its M61 rule permits an unstable separate envelope now but forbids
  presenting it as a supported sketch wire language.
- Existing native operands are capability-specific: `DocumentPointRef` is closed
  (`document.rs:583-593`) and native curve operands are `CurveSpan` plus explicit
  winding (`document.rs:499-505,634-640`). Keeping the two M43 external operand DTOs in
  explicit constraint variants preserves that model without forcing native-only
  resolvers and measurements to accept fixed coefficients; overloading `CurveId`,
  `DesignPointId`, or coordinate proximity would violate it. `document_lowering.rs`
  already resolves immutable input before
  allocation, and `SketchAttemptInput` already carries independent activation and
  parameter stamps (`document_session.rs:573-660`), making a third independent external
  stamp the narrow compatible extension.
- The currently implemented scene capsule is located in `playground.rs:41-339,
  2597-2682`; `ARCHITECTURE.md:102-106` calls it a private disposable-browser interchange
  format.  Keeping it there avoids falsely turning diagnostic exchange into sketch
  persistence.  M44 owns presentation and rebind workflows (`PLAN.md:1919-1934`), while
  M54 owns stable diagnostic compatibility (`PLAN.md:1953-1969`).

Rejected: (a) six or all-family snapshot alternatives now, because only point plus one
closed curve kind is required and broader families expand regularity/span policy; (b)
adding external variants to every existing native operand consumer, because their
resolvers assume native IDs and this would silently broaden M37/M38; (c) a public
callback/provider trait, because ADR 0026 forbids callbacks/lazy resolution; (d)
input-only Rust structs, because they cannot provide canonical external bytes for
reproducible capsules; (e) an opaque host revision token, because host keys remain
sidecar data; and (f) moving capsule or external-reference UI policy into M43 workbench
code, because that is M44/M54 scope.

### Open questions

None blocking.  The governing documents agree on a closed, revision-and-digest stamped,
immutable external input boundary; the above deliberately narrow v1 meets that boundary
without claiming M44, M54, or M61 behavior.

## Out of scope

- M44 browser/workbench tree entries, styles, status display and rebind interaction.
- M54 stable diagnostics compatibility/freeze, M55 prepared asynchronous jobs/CAS API,
  and M61 supported v5/persistence-envelope freeze.
- Host formula evaluation, units/display conversion, PDM/topological naming, projection,
  callbacks, host undo/history and host output commit.
- New solver variables, hidden fixed native geometry, generic curve plugins, new
  constraint mathematics, weighted-priority substitutes or core-solver changes.
