<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M59 implementation report

Date: 2026-07-29

Status: complete

## 1. Files and public APIs

M59 adds the pure-safe-Rust `geosolve-sketch-topology` workspace crate. Its direct workspace
dependencies are only `geosolve-sketch` and `geosolve-geometry`; it has no direct dependency on
`geosolve-core`, `geosolve-linkage`, `geosolve-sketch-ops` or either UI crate.

The companion publishes:

- `TopologySnapshot`, captured only from a current independently accepted state for the complete
  retained `PreparedSketchInput`;
- worker-movable `PreparedTopologyQuery` and controlled read-only execution;
- `TopologyRequest`, with explicit native profile/construction scope, immutable external-line
  scope, tangency/overlap/touching/T-junction/self-intersection policy and deterministic limits;
- separate outer `OperationOutcome` and inner `TopologyCompleteness` evidence;
- `TopologyScopeEvidence`, exact eligible sources, ignored external points and bounded work
  counters;
- typed incomplete `TopologyIssue` values with persistent native/external source scope; and
- `TopologyProductionProfile`, oriented `TopologyWire` fragments and outer/hole
  `TopologyRegion` records, available only for complete independently checked output.

`TopologyProductionProfile::validate_current` compares the complete captured input and accepted
state identity before host consumption. Query-local wire/region IDs deliberately do not claim
persistent B-rep naming.

## 2. Mathematical and transaction behavior

M59 adds no residual, Jacobian, rank rule, nonlinear solve, accepted publication path or B-rep
entity. It never mutates the captured or live sketch.

The existing all-family visual-profile analyzer supplies bounded arrangement candidates only. The
companion then independently checks:

- that every active source in the declared profile/construction/external scope is represented;
- exact native visible-interval or external binding/revision/digest/domain provenance;
- source parameter containment in certified endpoint enclosures;
- fresh source-curve evaluation at both fragment parameters;
- complete eligible-source interval coverage;
- ordered wire closure;
- finite signed area whose uncertainty certifies the claimed clockwise/counterclockwise
  orientation; and
- requested wire and region limits.

Any failed check publishes typed `Skipped` or `Truncated` evidence and no
`TopologyProductionProfile`. Cancellation and deterministic work exhaustion remain outer
operation outcomes. Stale consumption fails exact-input validation. NaN/Inf, ambiguous geometry,
open eligible supports, overlaps, rejected tangencies/T-junctions/self-intersections and
unresolved provenance cannot become complete topology.

Construction inclusion is query-local: the immutable accepted document is cloned and selected
construction roles are changed only in that private analysis copy. Immutable external line
segments are likewise query-local curves and retain their original host provenance. External
points are explicitly listed as ignored. No coordinate-proximity join is created.

## 3. Commands and outcomes

Focused implementation and compatibility qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all && cargo clippy --locked -p geosolve-sketch-topology --all-targets --all-features -- -D warnings && cargo test --locked -p geosolve-sketch-topology --test m59'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch-topology --all-features && cargo test --locked -p geosolve-sketch-ops --all-features && cargo test --locked -p geosolve-sketch --test m26 --test m28 --test m31 --test m34_lifecycle --test m57'
```

Outcome: pass. The M59 suite contains 15 direct cases. The focused compatibility set contains all
M58 operation cases and all selected M26/M28/M31/M34/M57 cases.

Complete release qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-topology --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Outcome: pass. Existing Cargo warnings about crates declaring both the inherited SPDX `license`
and `license-file` remain non-failing repository-wide metadata warnings; no licence metadata was
removed.

Repository review:

```bash
git diff --check
cargo tree -p geosolve-sketch-topology --depth 1
git worktree list --porcelain
```

Outcome: pass. The companion dependency direction matches the accepted M33 boundary and the
repository has one worktree.

## 4. Acceptance criteria passed

- Complete output is stamped with the full input and independently accepted-state identity.
- Native/construction/external scope and every ambiguity policy are explicit request evidence.
- Complete wires publish exact directed source fragments, closure, certified orientation and
  area; regions publish deterministic outer/hole relationships.
- Every declared eligible source must be completely covered.
- Incomplete, ambiguous, truncated, cancelled, exhausted and stale output is not consumable.
- Repeated identical snapshot/request queries are deterministic.
- Prepared queries move safely to a native worker; immutable inputs and outputs are `Send + Sync`.
- M58 multi-interval visible supports retain exact provenance and produce complete topology.
- The companion owns no equation, live solver/session state, private publication or B-rep entity.

No new residual was introduced, so M59 requires no new residual Jacobian implementation or
finite-difference Jacobian test.

## 5. Known limitations and next blocker

- M43 external line snapshots have exact binding/revision/digest/domain identity but no persistent
  relationship that declares one external endpoint identical to a native endpoint. M59 therefore
  refuses to proximity-weld mixed native/external endpoints; such a would-be loop remains
  incomplete.
- The only external production geometry admitted in M59 is the frozen M43 line-segment language.
  External point entries are explicitly ignored.
- Tangencies, overlaps, touching contours and T-junctions are rejected. Transverse
  self-intersections may be resolved only when the explicit policy requests it and all independent
  checks remain complete.
- Wire and region IDs are deterministic within one result, not persistent topological names.
- A production profile is a host-consumable sketch boundary DTO, not a B-rep feature or meshing
  result.
- The next blocker is M60: expose the qualified advanced curves, branch choices, operations and
  production topology through the one directly tested desktop workbench, then prepare M61 UAT.
