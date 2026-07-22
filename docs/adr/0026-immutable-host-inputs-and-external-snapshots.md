# ADR 0026: Immutable host inputs and external snapshots

Status: accepted

## Context

A production sketch is commonly driven by application expressions,
configurations and projected model geometry. Calling those systems while lowering,
solving or validating would make equations depend on mutable host state, prevent an
attempt from having complete input identity and make native worker scheduling differ
from single-threaded WASM. Importing host expressions, PDM identity or projection
logic into the sketch document would also make GeoSolve a second application model.

M41-M43 need one ownership boundary: hosts evaluate their own systems first and give
GeoSolve immutable typed values and immutable finite 2D geometry for one attempt.

## Decision

### Host ownership

The embedding host owns:

- expression syntax, parsing, evaluation and dependency graphs;
- authored and display units, unit conversion and formatting;
- configuration definitions, selection and dependency evaluation;
- PDM keys, model topology, topological naming and replacement policy;
- construction of 3D-to-2D projections and all projection tolerances;
- application commands, undo/redo, workspace history and cross-system transactions;
  and
- application persistence that joins those systems to local sketch bindings.

GeoSolve defines only the finite typed input kinds and canonical numeric conventions
required by sketch equations. The host converts its values before constructing an
input batch. GeoSolve validates type, domain, finiteness, resource limits, local
binding identity and revision evidence; it does not parse a formula, convert a user
unit expression, select a configuration or resolve a PDM object.

Canonical sketch state stores persistent local parameter and external-binding
identities plus their expected kinds. Arbitrary host keys, expression nodes,
projection recipes and application history remain in host-owned sidecars or the
separate desktop-workspace envelope. They are not sketch equations or canonical
sketch identity.

### Immutable attempt boundary and no callbacks

One solve attempt receives an immutable parameter batch and one immutable external
snapshot set before lowering begins. Lowering, residual and Jacobian evaluation,
independent validation, diagnostics, measurements and profile analysis must not call
host code or lazily fetch a missing value. The contract admits no evaluator closure,
trait callback, projection callback, asynchronous resolver or mutable shared host
object.

An implementation may own, clone or immutably share validated snapshot storage, but
the bytes named by the attempt stamp cannot change for the duration of that attempt.
Missing or invalid input produces a typed unsolved-design outcome and retains the
last accepted state; it never triggers an implicit host query.

### Parameter batches are coefficients

A parameter binding names a persistent local parameter and its required typed kind,
including length, angle, dimensionless and activation values. An immutable batch
contains exactly one finite value for each supplied binding together with batch
revision, canonical digest and provenance evidence. Duplicate, wrong-document,
wrong-kind, non-finite, stale or cyclic input/output ownership rejects atomically.

During one attempt a parameter value is a fixed coefficient of every bound target.
It is not a solver variable, contributes no tangent coordinate and cannot be changed
by solving. One parameter may drive several dimensions or activation inputs without
creating an artificial unknown or equality between hidden parameter variables.
Dependency mapping may dirty the affected sources and components, but it does not
transfer expression-graph ownership to GeoSolve.

Declared reference measurements are returned as immutable, typed, revision-stamped
output proposals with provenance and the complete input stamp from ADR 0025. They do
not call or mutate the host, and GeoSolve does not apply them to an expression graph
or application history.

### External snapshots are fixed geometry

A persisted local external binding declares its expected point, direction, support,
curve, span or other closed feature kind. The host supplies a bounded immutable 2D
snapshot set keyed by those local bindings. Each snapshot carries finite geometry,
source revision, canonical digest, parameter domain, orientation, scale and resource
evidence required by its kind.

External geometry is constant input. It contributes no solver variable, tangent
coordinate, accepted unknown or hidden editable copy. Constraints may solve native
sketch unknowns against external coefficients, and audit names the local binding and
exact snapshot evidence. The host, not GeoSolve, computes projection from 3D model
geometry and decides which model feature a local binding denotes.

The external envelope is a closed versioned data language rather than an arbitrary
curve plugin. Unsupported families, malformed domains, non-finite values, excessive
resources and unavailable regularity reject as typed input outcomes before they can
produce accepted geometry.

### Revisions, digests and rebinding

Parameter batches, external snapshot sets and their entries carry explicit revisions
and versioned canonical-payload digests. Revisions provide ordering and stale-input
checks; digests identify exact bytes for reproducibility. Reusing one revision with a
different digest is invalid, and matching a digest does not permit an older revision
to overwrite current input. The exact batch and set revisions and digests enter the
complete attempt and accepted-state stamp defined by ADR 0025.

Updating values or geometry without changing a binding's declared semantic kind and
topology is a new immutable input revision. A changed feature family, span identity,
orientation contract or topology requires an explicit rebind/remap design
transaction. Missing, duplicated, stale and topology-incompatible bindings leave the
design unsolved. Coordinate equality or proximity never repairs identity, chooses a
replacement or changes a branch.

### Milestone allocation

M41 implements construction roles, activation inputs and explicit inactivity
reasons. M42 implements immutable typed parameter batches, fixed-coefficient
bindings and stamped output proposals. M43 implements immutable external 2D
snapshots, digest evidence and explicit rebinding. Later prepared jobs and
compare-and-swap publication consume these immutable inputs; they do not revise this
ownership boundary.

The separate parameter and external-snapshot wire envelopes remain subject to the
M53 freeze described by ADR 0025. This ADR is an M33 contract decision only. M33
adds no parameter, external-reference, callback, persistence or public session API.

## Consequences

- A solve attempt is deterministic with respect to named host inputs and cannot
  observe mid-solve host mutation.
- Native hosts may schedule immutable work on their own workers while
  single-threaded WASM uses the same semantic contract; GeoSolve owns no async
  runtime or host callback lifecycle.
- Shared parameters do not inflate rank or mobility, and external geometry never
  masquerades as a solved native unknown.
- Hosts must materialize complete finite batches and snapshots before solving and
  explicitly rebind topology changes.
- Formula graphs, unit systems, configurations, PDM identity, projection and
  application history remain replaceable host concerns rather than solver state.
