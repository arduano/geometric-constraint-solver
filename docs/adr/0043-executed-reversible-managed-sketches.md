<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0043: executed, reversible managed sketches

Status: accepted for M89 implementation on 2026-09-01.

## Context

Managed `sketch.ts` is currently interpreted primarily by a Rust source parser. The TypeScript
`sketch()` callback is not the semantic source of the compiled declaration graph, source-backed
canvas authoring covers only a small promoted vocabulary, and newly authored geometry may remain
owned only by the nested GUI document. That split makes runtime-defined values difficult to edit,
requires explicit reverse-edit hints such as `p.editLens`, and prevents the managed representation
from reproducing the complete sketch.

M89 requires one representation that joins code authoring, structured UX edits and browser-free
agent workflows without giving JavaScript, the browser or a sidecar any solver authority.

## Decision

### Closed managed language

`sketch.ts` is a deliberately closed, deterministic TypeScript subset. Parsing owns lexical facts:
imports, attached comments, declaration names, groups, declaration order, expressions and explicit
suppression. The subset admits only supported managed declaration expressions and immutable local
bindings. It rejects loops, arbitrary helpers, conditionals, mutation, async, dynamic imports,
ambient I/O and other constructs before execution.

The parser produces a versioned `ManagedSketchIrV2`. A canonical printer regenerates normalized
`sketch.ts` from that IR. Byte-exact formatting is not authority, but semantic order, identifiers,
expressions, group structure, explicit suppression and attached comments are. The following
round-trip is stable:

```text
source -> ManagedSketchIrV2 -> normalized source -> ManagedSketchIrV2
```

`sketch.ts` has no terminal `return $.outputs(...)` in v2. Every top-level declaration materializes
independently and the executed artifact exposes its complete typed declaration/result tree. Groups
organize declarations and establish presentation/source order; they do not control materialization.
Custom `.patch.ts` modules retain their return values and remain the liberal TypeScript extension
point. They are not required to be reversible into managed IR.

### Instrumented execution and provenance

The compiler resolves the pinned SDK and injects stable source-site identifiers into supported
declaration/value calls. A deterministic recorder then executes the managed callback and emits an
`ExecutedSketchArtifactV2` containing the semantic declaration graph, generated patch members,
complete typed results, source-site provenance and value-consumer provenance.

Source-site IDs are the durable edit-address mechanism. Raw stacks and source maps may be retained
for diagnostics, but never choose an edit target. Correct Rust UTF-8 byte offsets are converted to
CodeMirror UTF-16 positions at the presentation boundary.

The browser hosts compilation and recorder execution. Native source compilation uses a pinned Deno
sidecar. Canonical already-compiled projects continue to materialize, solve, independently validate
and render in pure Rust without Deno. Browser and Deno must emit identical normalized source, IR,
artifact and digests for the same inputs.

### One source/IR/artifact transaction

Structured source, panel and canvas edits use one two-phase transaction:

1. Rust prepares a digest-bound mutation or authoring ticket against the exact accepted project.
2. The browser or Deno mutates IR, prints source and executes the candidate.
3. Rust validates source, IR, artifact, ticket identity and the exact permitted semantic delta.
4. Existing Intent expansion/materialization, native solving and independent residual/domain/branch
   validation run from a cold candidate.
5. Source, IR, artifact, accepted scene and one outer history entry publish atomically.

One completed gesture creates one transaction. Preview frames never compile managed code. A
rejection retains the complete previous accepted canvas and publication authority while preserving
the candidate source draft and positioned diagnostic for correction. There is no GUI-owned fallback
for source projects: every enabled geometry, constraint, dimension and operation must have a managed
source representation, using a typed generic recipe declaration when no ergonomic builder exists.

New canvas declarations enter a visible `Canvas additions` group. A persisted monotonic high-water
allocator supplies stable names such as `segment42`; deleting or undoing an insertion never reuses a
name. Reordering changes actual IR/source declaration order and dependency-invalid moves reject.
Suppression is explicit source/IR state for both top-level declarations and generated patch members,
not a hidden projection overlay.

The declaration panel presents top-level `sketch.ts` declarations, with patch-generated members
nested under their owner. Its edit, reorder and suppression commands all use the same prepared
transaction.

### Compatibility boundary

Checked-in legacy samples and untouched v1 projects are not normalized in M89. They use an isolated
legacy parser/artifact compatibility path. Their first source-aware structured edit upgrades only
the active project copy to v2; bundled sample bytes remain unchanged. Legacy `p.editLens` is parsed
and ignored for compatibility, while v2 omits it because executed value-consumer provenance and the
existing artifact template bindings provide fan-out. Persisted projects containing GUI-only hybrid
additions reject as a whole instead of silently losing geometry.

A later clean-break milestone will normalize every legacy sample and remove the v1 path.

## Invariants

- TypeScript execution records declarations; it does not evaluate solver equations or publish
  accepted geometry.
- Rust validates bounded source/IR/artifact envelopes and remains sole owner of Intent expansion,
  materialization, solving, branch/domain checks, independent residual validation and history.
- A success-like state requires finite accepted geometry and independently normalized hard residual
  at most `1e-9`.
- Invalid geometry, non-finite values, stale tickets, unsupported syntax and semantic-delta mismatch
  publish no partial source, IR, artifact, scene or history.
- Branch, orientation, span, winding and generated-member identity remain explicit durable state.
- No solver FFI or `unsafe` code is introduced.

## Consequences

Managed files become normalizable rather than byte-preserving. Runtime execution replaces parser
guessing for semantic provenance while the parser continues to preserve author-facing syntax. The
same compiled project remains portable and browser-free; editing raw source natively additionally
requires the pinned compiler sidecar. Legacy compatibility is intentionally temporary and isolated,
so the future sample-normalization milestone can remove it cleanly.

## M90 clean-break amendment

This amendment governs the active M90 implementation. It leaves the M89 decision and compatibility
record above intact as history, but supersedes that compatibility boundary for M90:

- `"use geosolve sketch"` is the only managed directive;
- `geosolve-managed-sketch-ir-v3` and `geosolve-executed-sketch-artifact-v3` are the only admitted
  managed compiler formats;
- every public geometry, standalone constraint, dimension, operation and aggregate uses a named
  typed method and named argument object, while computed Fillet uses `computed.filletSet`;
- public generic recipes, tuple-key input tables, edit lenses, result manifests and operation-
  output transport payloads are not admitted managed-language forms;
- all twelve bundled samples are normalized V3 projects, with no managed-v1/v2 parser, upgrade or
  compatibility path; and
- raw-source mutation is always Rust-prepared, compiler-host-applied and Rust-resolved, while
  already compiled inspect/solve/render remains pure Rust and browser-free.

Custom patch modules remain separately compiled, pinned extension points and need not be reversible
managed lexical IR. Their private transport representation does not reopen a public generic managed
API. Profile Offset may retain its explicit aggregate-helper/root declaration closure. The two
host-external constraint variants remain input-gated rather than acquiring fabricated standalone
authority. All original invariants continue to apply: TypeScript records declarations only, Rust
owns materialization/solving/validation/publication, invalid or non-finite candidates publish
nothing, branch state is explicit, and success-like results require independent residual
validation.
