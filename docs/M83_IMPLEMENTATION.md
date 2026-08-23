<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 implementation ledger — Projectional sketch design intent

Status: **active**. This ledger records implementation and qualification against ADR 0040 and
`docs/M83_GOALS.md`.

## Baseline and disposition

- Accepted product baseline: M81 closeout plus the M82 exact rollback on `main`.
- Rejected chronological research: branch `archive/m83-chronological-lineage-2026-08-23`, tip
  `be62a1c`.
- Replacement implementation begins after rollback commit `af77877` and reuses only independently
  justified native seams. It does not restore the archived ledger, JSON owner rewrite or mirrored
  history.

## Implementation slices

### I1 — native materialization prerequisites

Completed preparatory commits expose canonical parameter batches, atomic scalar edits, neutral
continuation seeding, independently authenticated accepted rematerialization, native-Fillet intent
continuation and transient-only pointer rendering. These APIs remain useful below any lineage
model and keep accepted-state validation inside `geosolve-sketch`.

### I2 — `geosolve-sketch-intent`

In progress. The new pure-Rust crate owns stable graph/port/child/reservation identities, closed
catalog schemas, separate instance/organization/external identities, unordered patches, exact CAS,
retained accepted/failure authority, canonical persistence and one bounded Undo/Redo history. It
contains no solver equation and depends on neither the sketch domain nor the editor.

### I3 — editor-owned materializer

Pending. Add typed graph-to-document lowering, exact reservation translation, dependency schedule,
logical/native ownership, reverse free-leaf bindings, host-input decoding, authenticated native
Fillet/Profile Offset paths, independent solve validation and cold reconstruction.

### I4 — coordinator integration and bootstrap

Pending. Install one intent session beside the retained coordinator, bootstrap existing flat
workspaces honestly, route durable mutations through composite patches and retain only ephemeral
gesture state outside intent. Remove any second history authority.

### I5 — projections, RPC and workspace v8

Pending. Add Design panel projections, typed Inspector edits, organization-only reorder,
TypeScript-shaped structured source, read-only History, DOM-free WASM/RPC, branded TypeScript
bindings and strict workspace-v8 persistence with v1-v6 bootstrap migration.

### I6 — qualification and nomination

Pending. Complete inventory/differential/golden/persistence/drag/RPC/package coverage, pass the
clean release gate, freeze without rebuilding and serve the exact candidate over Tailscale for
human UAT. Do not publish Pages before approval.

## Findings

No replacement-architecture defect is currently open. Implementation concerns are corrected in
their owning slice before nomination and receive an `M83-Fxxx` ID only after an exact reproduction
exists.

## Qualification record

The preparatory transient-rendering slice passes:

```text
RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-demo-web --lib
155 passed

cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings
pass

cargo check --locked --target wasm32-unknown-unknown -p geosolve-demo-web --all-features
pass
```

The default test-thread stack still encounters the repository's known isolated stack overflow;
the 16 MiB test stack passes. Final qualification remains pending.
