<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0042: Managed controls and headless authoring

Status: accepted for M87 implementation.

## Context

ADR 0041 made managed source and pinned custom artifacts authoritative, but allowed generated GUI
properties only through optional edit lenses and kept camera/SVG/PNG orchestration in the browser.
Typed Panel demonstrates that forward expansion provenance is insufficient: a generated property
can consume an exact source literal while the Inspector has no authenticated reverse transaction.

## Decision

`geosolve-sketch-code` derives a transient managed-control manifest from parser-owned literal spans,
declaration schemas, artifact dependency bindings, and semantic consumers. A control token is an
exact-CAS source capability, not a solver value: it carries stable semantic address, project/source
digests, schema, and exact expected value. Session/selection identity is authenticated separately
at consumption time. Manifests never enter persisted expansion or session wire authority.

Only explicit non-DoF definition values receive controls. Existing point and solver-instance draft
state remains outside code. Structural references and absent values are never fabricated. Shared
inputs edit their one source leaf and update their complete authenticated consumer group.

Artifact-v1 `EditLens` is retained as optional presentation metadata. It cannot authorize an edit
without independently proven source/property dependency.

`SketchCodeSession` remains the sole live history. Code-mode nested Intent mutations reject; a
separate code-control RPC reuses managed edit transactions and outer Undo/Redo.

Presentation is split into a target-neutral `geosolve-sketch-render` crate and a native
`geosolve-headless` orchestration crate/binary. The former owns shared camera/SVG and native-only
PNG; the browser retains DOM/events/storage/download behavior. The latter accepts only managed
source, canonical pinned project data, or bundled demos and never executes TypeScript.

## Consequences

Managed parameters are uniformly inspectable/editable from GUI and agents, shared impact is
explicit, and invalid edits retain accepted authority. Plain solver/editor deployments remain free
of code/headless dependencies. Static SVG/report output is deterministic; raster output is visual
evidence rather than solver authority. M87 adds no equation or solved-value write-back path.
