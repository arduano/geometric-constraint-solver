<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 implementation ledger — Projectional sketch design intent

Status: **implementation complete; clean nomination in progress**. This ledger records
implementation and qualification against ADR 0040 and `docs/M83_GOALS.md`. Human UAT remains
pending and accepted M81 GitHub Pages bytes remain public authority.

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

Complete. The new pure-Rust crate owns stable graph/port/child/reservation identities, closed
catalog schemas, separate instance/organization/external identities, unordered patches, exact CAS,
retained accepted/failure authority, canonical persistence and one bounded Undo/Redo history. It
contains no solver equation and depends on neither the sketch domain nor the editor.

The first review-hardening pass adds a durable reservation/tombstone ledger to semantic and
session identity, stable developer symbols independent of mutable display names, non-writable
logical handles, accurately named host materialization artifacts and deterministic clock-free
transaction descriptors for the read-only History projection. Focused lifecycle coverage includes
suppression, deletion, retained failure, Undo/Redo, divergent edits, bounded-history eviction and
canonical reload. The equation-free branded TypeScript package consumes the same closed wire
vocabulary and carries no geometry or residual implementation.

### I3 — editor-owned materializer

Complete. `geosolve-constraint-editor` lowers the closed geometry, relation, dimension, operation,
computed-Fillet, parameter and external catalogs in dependency order; translates exact native
reservations; records logical/native ownership and reverse free-leaf bindings; decodes canonical
host inputs; and uses the existing authenticated native Fillet/Profile Offset paths. Accepted
publication requires the existing independent solve validation, and canonical cold reconstruction
is covered against warm/editor materialization.

### I4 — coordinator integration and bootstrap

Complete. One projectional coordinator owns intent, accepted materialization and bounded composite
history. Fresh and legacy flat workspaces normalize into typed per-object bootstrap declarations
with exact existing native bindings, and supported point ejection continues identity in place.
Canvas construction, contextual relations/dimensions, Inspector/source edits, native role changes,
computed Fillet, Profile Offset, operation deletion and live RPC all publish through the same patch
authority. Pointer preview state remains transient; terminal release publishes at most one exact
instance/property transaction.

### I5 — projections, RPC and workspace v8

Complete. Commit `50d2ec6` installed the Design-panel shell; subsequent slices add Rust-backed
`Outline | Structured source | History`, schema-derived Inspector edits, organization-only button
and drag/drop reorder, deterministic TypeScript-shaped source tokens, read-only History, DOM-free
WASM/RPC, a branded TypeScript client and strict workspace-v8 persistence. Version 7 rejects and
v1-v6 migrate through their original strict decoder before per-object bootstrap normalization.

The M76 annotation-layout cache remains an explicitly separate presentation-only workspace field:
it is compatibility-filtered and recomputable, omitted from reproduction authority, and never
enters intent identity, dependency scheduling, materialization, solver input or composite history.

Final interaction hardening at `62378c9` projects a unique canvas/tree owner to the same stable
declaration selected by Outline/source; ambiguous, protected, unowned or multi-owner native
selection clears declaration targeting. Toolbar and Delete/Backspace share editor-owned deletion,
authoring retains keyboard precedence, failed deletion does not save, retained-invalid Profile
Offset deletes by stable declaration with its exact private aggregate closure, and grouped private
helper source rows cannot become invisible selection/reorder targets while recognized token edits
remain typed.

### I6 — qualification and nomination

In progress. Inventory, order-independence, reservation/tombstone, retained-failure, cold/warm
differential, construction/application/operation, Fillet/Offset, persistence, native/WASM RPC,
TypeScript and split pointer/terminal performance suites are implemented. The release gate now
includes TypeScript package/runtime/type checks and separate release interaction benchmarks.
Remaining nomination work is the final clean all-workspace gate, no-rebuild immutable freeze,
served-byte verification and recording that exact candidate in `docs/M83_UAT.md`. Do not publish
Pages before human approval.

## Findings

No replacement-architecture finding is open. The final selection/deletion review found and closed
one pre-nomination interaction seam before assigning a public `M83-Fxxx` identity: browser and
editor selection could name different mutation targets, and private Offset helpers could be
selected from source despite being grouped out of Outline. Exact owner regressions now freeze the
corrected contract; no solver equation or mathematical behavior changed.

## Qualification record

Current focused/proportional evidence includes:

```text
cargo test --locked -p geosolve-constraint-editor --test m83_projectional_editor
8 passed

cargo test --locked -p geosolve-constraint-editor --test m83_projectional_offset
11 passed

RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-demo-web --lib
189 passed

cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web \
  --all-targets --all-features -- -D warnings
pass
```

Before the final selection/deletion slice, the locked all-feature workspace suite, relevant WASM
checks, TypeScript package, unchanged 271-row golden and performance suites passed. Final
qualification reruns the complete matrix from the committed candidate; only commands actually
completed there will be recorded as nomination evidence. The repository's known default test-thread
stack limitation remains handled with `RUST_MIN_STACK=16777216` in the authoritative gate.
