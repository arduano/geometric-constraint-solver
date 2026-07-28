# ADR 0028: Sketch operations and production topology companions

Status: accepted

## Context

The sketch solver owns design intent, equations, accepted geometry and truthful
diagnostics. A CAD host also needs drafting operations such as split, trim, extend,
mirror and chamfer, plus production-grade wire/profile extraction suitable as input
to a separate feature system. Putting either concern into `geosolve-core` or private
sketch lowering would expand solver semantics, while calling the existing M31 visual
faces production topology would overstate a display-oriented bounded analysis.

Operations must be replaceable by explicit host transactions. Production topology
must be computed from one immutable accepted snapshot and remain unable to change the
sketch it describes. Neither concern justifies a B-rep kernel, persistent region
entity, private residual family or second accepted-state machine.

## Decision

### Milestone ownership

M33 accepts this crate and product-boundary contract only. It does not add either
crate, operation, topology result or implemented capability, and documentation must
continue to describe them as target behavior until their gates pass.

M103X implements `geosolve-sketch-ops`. M104X implements
`geosolve-sketch-topology`. Their APIs and tests may be designed earlier, but no
implementation status is claimed before those milestones.

### Dependency graph

The allowed direct workspace dependencies are one-way:

- `geosolve-sketch-ops` may depend on `geosolve-sketch` and
  `geosolve-geometry`;
- `geosolve-sketch-topology` may depend on `geosolve-sketch` and
  `geosolve-geometry`;
- neither companion may depend directly on `geosolve-core`,
  `geosolve-linkage` or the other companion; and
- `geosolve-sketch`, `geosolve-core`, `geosolve-geometry` and
  `geosolve-linkage` may not depend on either companion.

A host or `geosolve-demo-web` may depend on either or both and coordinates their
results through public persistent sketch identities.

The companions have no direct dependency on `geosolve-core` and do not consume core
variable, residual, source, component or runtime-map identities. They use immutable
numerical types and operations from `geosolve-geometry` plus public
`geosolve-sketch` documents, snapshots, semantic features, curve evaluation,
transactions and persistent source provenance. This boundary is protected by Cargo
dependency checks and public-API integration tests.

Both crates remain pure safe Rust, `GPL-3.0-or-later` and compatible with the native
and WASM contract. They follow ADR 0027 for cancellation, deterministic work
exhaustion, immutable prepared work and stale-result handling. Neither crate owns an
async runtime or thread pool.

### Shared exclusions

Neither companion may define or execute a residual formula, Jacobian, nonlinear
solver, rank policy, active set, solver variable or accepted-state commit path. An
operation may request an existing public sketch constraint or dimension in its output
transaction, but the equation and independent validation remain owned by
`geosolve-sketch`. The companions neither store nor mutate solver or session state.

Neither companion owns a B-rep body, shell, face, edge, vertex, boolean, feature
history or solid-model topology. Neither adds a persistent region/profile object to
canonical sketch state. Output-local wire and fragment identities are ephemeral and
are not a promise of topological naming across design revisions.

Coordinate proximity, render tessellation and initial coordinates never establish
identity, ownership, a weld, branch selection or production completeness. Persistent
sketch identity, explicit coincidence/contact/operation ownership, exact source
parameters and explicit branch/span/winding state remain authoritative.

### `geosolve-sketch-ops`

The operations companion is a deterministic transaction producer. An operation reads
an immutable stamped design snapshot and, when geometric construction requires it,
the matching independently accepted snapshot. It receives typed operands, explicit
branch/side/retained-piece policy and deterministic work limits. It returns either a
typed failure/incomplete outcome or:

- a proposal expressed entirely through public sketch transaction forms;
- an identity/provenance mapping that states which source identities are retained,
  replaced, split or newly proposed; and
- the complete input stamp, exact accepted-state identity when used, and immutable
  operation-request evidence needed to reject stale application.

The companion does not mutate a `SketchDocumentSession`, reserve a hidden accepted
revision or bypass command/history validation. The single-writer host applies the
proposal through the normal sketch transaction path, where structure, finite values,
dependencies, equations, branches and independent accepted-state validation remain
authoritative. Applying a stale proposal fails under ADR 0027 rather than rebasing by
coordinate similarity.

M103X owns general split/break, trim, extend, exact family-supported mirror, chamfer,
existing fillet ownership integration and ordinary rectangle/polygon/slot/pattern
expansion. These operations may generalize public visible topology to several explicit
intervals per immutable support. They preserve an existing persistent identity when
the operation's documented semantic retained piece remains that object; all other
identity changes are explicit in the result mapping. A support definition is not
silently rewritten into sampled geometry.

Macros and associations expand to ordinary sketch geometry, source definitions and
explicit ownership. They are not privileged residuals. Approximate general
spline/conic offsets and a persistent pattern-object personality remain outside the
M109X contract; an unsupported exact transformation returns a typed unsupported outcome
rather than an approximation.

For the same stamped input, operation policy and public ID-allocation context, a
proposal and its source/result mapping are deterministic. A host can replace any
operation with an equivalent explicit public transaction without changing solver
semantics.

### `geosolve-sketch-topology`

The production topology companion is a read-only query over exactly one immutable,
independently accepted sketch snapshot. Structurally valid but unsolved design,
attempted geometry and a retained accepted state from a different design/input stamp
cannot be mixed into one query.

A query declares:

- the complete ADR 0025 input stamp and exact accepted-state identity;
- immutable topology-request and production-policy evidence;
- included geometry roles and explicit construction/external-geometry scope;
- visible support intervals and persistent source/contact/ownership provenance;
- production policies for tangency, overlap, touching contours, T-junctions and
  self-intersections; and
- deterministic intersection, subdivision, integration, containment, fragment and
  output limits.

A complete result publishes oriented wires, outer/inner nesting, holes, bounded region
boundaries and traversal-ordered exact source-span provenance. Every result carries
its query policy, configured and consumed work, issues, completion evidence and exact
input stamp. Intersections and traversal fragments are query-local evidence; they do
not become sketch entities or solver sources.

Operation control and topology completeness remain orthogonal. `Completed`,
`Cancelled` and `WorkExhausted` describe why execution stopped;
`Complete`, `Truncated` and `Skipped` describe the bounded topology evidence returned.
A cancelled or exhausted operation cannot carry `Complete` topology.

`Complete` means every eligible visible interval in the declared production scope was
processed and every required intersection, endpoint join, outgoing order,
orientation, nesting and hole decision was resolved. `Truncated`, `Skipped`,
cancelled, exhausted, ambiguous or stale output is not a production profile. It may
carry diagnostic or provisional geometry for inspection, but that geometry is not
present in the complete wire collection and cannot be promoted by a consumer.

Only a `Completed` operation with `Complete` output whose complete input stamp,
accepted-state identity and topology request still match may be passed to a host B-rep
feature. Doing so does not make the topology crate a B-rep owner: conversion, feature
history, topological naming across revisions, 3D projection, faces, solids and
booleans remain host responsibilities. The topology query never mutates design,
accepted state, history, activation, visible intervals or audit.

### Visual versus production topology

M31 visual profile analysis remains a `geosolve-sketch` display and diagnostic aid.
It may retain provably disjoint clean components or a deterministic face prefix while
the overall result is `Truncated`. Its faces are pointer-transparent, non-persistent
and unsuitable as CAD feature input.

Production topology is exposed only by `geosolve-sketch-topology` under the stricter
M104X query contract. It uses distinct result types and requires full resolution of the
declared production scope before exposing consumable wires. A visual `Complete` face
is not implicitly a production wire, and there is no unchecked conversion or status
relabeling between the APIs. Production analysis must evaluate or freshly validate
the exact accepted geometry, source provenance, scope and production policy itself.

Both layers may use the same public immutable curve geometry and certified numerical
techniques. They differ in product authority and publication rules, not by allowing
the production layer to guess harder cases. Rendering samples remain non-authoritative
in both.

## Consequences

- Solver and document crates remain independent of convenience drafting algorithms
  and manufacturing-facing topology policy.
- Operations can create rich ordinary transactions without gaining private equation
  access or a second commit path.
- Production wires can feed a CAD host only with exact accepted-state provenance and
  complete bounded evidence.
- Existing visual faces retain their useful fail-closed display behavior without being
  promoted to production topology.
- Hosts still own B-rep entities, projection, feature history, application undo and
  cross-revision naming.
- Separate crates add packaging and API surface, but make forbidden dependency and
  ownership edges mechanically reviewable at M103X and M104X.
