<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M58 implementation report

Date: 2026-07-29

Status: complete

## 1. Files and public APIs

M58 adds the pure-safe-Rust `geosolve-sketch-ops` workspace crate. Its direct workspace
dependencies are only `geosolve-sketch` and `geosolve-geometry`; it has no direct dependency on
`geosolve-core`, `geosolve-linkage`, `geosolve-sketch-topology` or either UI crate.

The companion publishes:

- `SketchOperationSnapshot`, `PreparedSketchOperation` and controlled execution over one complete
  immutable `PreparedSketchInput`;
- the closed `SketchOperationRequest`/`SketchOperationKind` surface for split, break, trim, line
  extension, mirror, chamfer, associative fillet, rectangle, regular polygon, slot and linear
  pattern;
- `SketchOperationResult` with typed proposed, unsupported and incomplete outcomes;
- `SketchOperationProposal`, exact accepted/input provenance and deterministic
  `SketchOperationIdentityChange` mappings; and
- `SketchOperationProposal::apply`, which performs exact-input compare-and-swap before the normal
  `RetainedSketchDocumentSession::transact` boundary.

`geosolve-sketch` adds:

- `DocumentTrimBoundary::ConstraintContact`;
- `SketchDocument::trim_views_for_span`, `visible_intervals` and `replace_trim_views`;
- flattening of every visible interval through `visible_curve_intervals`,
  `is_parameter_visible`, visual profiles and editor scene construction; and
- `RetainedSketchDocumentSession::prepared_input`, a read-only complete current stamp.

The legacy single-interval accessors remain conservative compatibility seams:
`trim_view` returns only a sole view and `visible_interval` rejects multi-interval supports.
Canonical v4 serialization returns `UnsupportedM58State` for M58-only topology. The explicitly
unsupported draft-v5 codec round-trips it pending the M62 schema freeze.

## 2. Mathematical and transaction behavior

The companion adds no equation, residual, Jacobian, rank policy, nonlinear solve or
accepted-state path.

Visibility operations retain immutable source definitions and replace only explicit visible
intervals. Intervals are finite, increasing, traversal-ordered and non-overlapping. Exact fixed
parameter bits plus winding, or exact owner/contact identity, define interval endpoint adjacency;
coordinate proximity is never identity evidence.

Geometry-dependent operations read only the independently accepted document for the same retained
design. Line extension computes one exact line-line intersection and requires it to extend the
selected endpoint. Mirror and pattern use existing public exact point-defined construction/copy
paths. Chamfer uses two ordinary point-on-curve constraints and two ordinary driving
point-distance dimensions, with the contact owners defining the two visible boundaries. Deleting
such an owner freezes the accepted parameter into a fixed boundary before owned contact state is
removed. Associative fillet is only a wrapper around the existing public generic-fillet
transaction.

Each proposal is first replayed and validated against scratch public document state. Applying it
requires the complete retained input stamp to match, then replays the same public transaction and
checks the resulting identity map before ordinary solve/publication. A stale proposal returns
`StaleInput`; cancelled or work-exhausted execution returns no proposal. Unsupported exact
families are never tessellated or approximated.

## 3. Commands and outcomes

Focused implementation and compatibility qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all && cargo clippy --locked -p geosolve-sketch-ops --all-targets --all-features -- -D warnings && cargo test --locked -p geosolve-sketch-ops --all-features && cargo test --locked -p geosolve-sketch --test m28 --test m31 --test m34_lifecycle --test m57 && cargo test --locked -p geosolve-constraint-editor'
```

Outcome: pass. The M58 suite contains 18 direct cases; the focused compatibility set contains all
M28/M31/M34/M57 cases and all editor unit/M55 cases.

Complete release qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-ops --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Outcome: pass. Existing Cargo warnings about crates declaring both the inherited SPDX `license`
and `license-file` remain non-failing repository-wide metadata warnings; no licence metadata was
removed.

Repository review:

```bash
git diff --check
cargo tree -p geosolve-sketch-ops --depth 1
git worktree list --porcelain
```

Outcome: pass. The companion dependency direction matches ADR 0028 and the repository has one
worktree.

## 4. Acceptance criteria passed

- The companion owns no residual formula, solver state, accepted revision or B-rep entity.
- All completed requests produce deterministic public document proposals and explicit identity
  disposition.
- Split/break/trim support several explicit visible intervals without rewriting curve supports.
- Line extension, supported exact mirror, line chamfer, fillet integration and ordinary macros/
  patterns pass normal sketch validation and publication.
- Unsupported and incomplete outcomes are typed; non-finite, excessive, ambiguous and foreign
  inputs fail closed.
- Stale, cancelled and exhausted work cannot change design, lifecycle or accepted geometry.
- Exact boundary identity preserves a split closed profile without proximity welding.
- Frozen v4 rejects M58 state; the temporary draft-v5 bridge round-trips it.
- Native, WASM, workspace Clippy/test and release Trunk gates pass.

No new residual was introduced, so M58 requires no new residual Jacobian implementation or
finite-difference Jacobian test. Every emitted constraint/dimension uses an existing public sketch
residual with its existing audit and derivative qualifications.

## 5. Known limitations and next blocker

- Exact mirror and linear pattern currently support point-defined line/polyline, quadratic/cubic
  Bezier and non-rational B-spline families. Other families return typed unsupported.
- Mirror and pattern require accepted geometry to equal retained design geometry exactly. A host
  must first resolve retained unsolved intent rather than mixing accepted coordinates from an
  older attempt into a new proposal.
- Line extension and chamfer are intentionally line/polyline-span operations in M58.
- Full-period circle/ellipse multi-interval split/trim is typed unsupported; no seam policy is
  guessed.
- Slot is an ordinary fixed construction expansion, not a persistent feature or pattern object.
- M58 topology is visible sketch topology only. The next blocker is M59:
  `geosolve-sketch-topology`, whose complete revision-stamped production wires, nesting, holes,
  ambiguity policy and provenance remain separate and must not be inferred from visual profiles.
