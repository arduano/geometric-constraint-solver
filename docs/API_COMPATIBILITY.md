# API and persistence compatibility

## Release line

GeoSolve `0.2.0` is the current supported preview release; `0.1.0` was the first. The
seven library crates (`geosolve-geometry`, `geosolve-core`, `geosolve-sketch`,
`geosolve-linkage`, `geosolve-sketch-ops`, `geosolve-sketch-topology` and
`geosolve-constraint-editor`) version and release in lockstep. `geosolve-demo-web` is a
non-published diagnostic consumer.

Before `1.0`, a minor version may contain source-breaking changes. Patch releases
must remain source-compatible except where retaining behavior would preserve a
soundness issue, false success, invalid accepted geometry or a security defect.
After `1.0`, Rust API compatibility follows Cargo SemVer.

M33 completes the production-embedding contract and baseline freeze without adding
target APIs. M34 adds the retained-design lifecycle and M35 adds cooperative
operation-control APIs; M36-M44 complete the current implementation transition. Cleanup
M46-M53 preserves released v1-v4 wire compatibility and the accepted-state safety contract
while evolving the new pre-1.0 editor/workbench surface. M61 closes the currently approved
advanced-workbench scope, M62 closes approved CAD-style authoring and M63 closes approved canvas
constraint presentation. Approved M64 adds only public alpha fixtures and an explicit
interaction-request preference helper; it does not freeze a new schema. M65 completes approved
predictable, bounded projected dragging without freezing a new persistence language or claiming
final API/schema hardening. M66 is the active pre-1.0 helper-operation authoring cut; its editor
and operations APIs may evolve during qualification and likewise add no persistence language.
The unreleased M61
`DocumentSolveRequest::stability_target` field and helper were withdrawn before the next
published minor release because a sample-selected second Temporary target conflicts with M65's
sample-agnostic locality contract; neither API was part of the published `0.2.0` surface. Any
draft-v5 representation remains explicitly unsupported until a future schema-freeze milestone is
deliberately scoped, qualified and approved, and must not be treated as a released wire language.

The minimum supported Rust version is `1.89`. Raising it requires a minor release
before `1.0`, a major release after `1.0`, and a changelog entry.

## API tiers

The supported domain entry points are:

- `SketchDocument` and accepted-only `SketchDocumentSession` for persistent sketches;
- `RetainedSketchDocumentSession` for separate retained design, attempt and accepted views;
- `PlanarLinkageDocument` and `PlanarLinkageSession` for planar kinematics;
- `SpatialAssemblyDocument` and `SpatialAssemblyDocumentSession` for spatial
  kinematics;
- immutable geometry and accepted domain result/audit types returned by those
  workflows;
- `geosolve-sketch-ops` immutable snapshots, controlled prepared proposals and exact-input
  application for equation-free sketch operations;
- `geosolve-sketch-topology` complete accepted-input production-wire and region profiles; and
- `geosolve-constraint-editor` state, scene, normalized input and typed effect APIs for
  presentation-independent constraint, dimension and helper-operation authoring over those sketch
  workflows.

Legacy direct `Sketch`, `Linkage` and `SpatialAssembly` builders remain supported
compatibility facades in the `0.2` line.

Compiler products, runtime ID maps, direct `geosolve-core` reports and fixture or
performance builders are public for advanced diagnostics and verification, but are
explicitly unstable before `1.0`. They must not be persisted or used as application
identity. M29 has reviewed these exports and retains them intentionally because the
diagnostic consumer and independent audit tooling inspect the same validated report;
new application APIs should prefer persistent domain IDs and domain-owned views.

Public error and status enums may gain variants. Callers should include a wildcard
arm unless an enum is documented as closed. Public structs intended as reports may
gain fields in a minor `0.x` release. Request and persisted document types are not
extended silently within a schema version.

## Deprecation

A supported domain API is deprecated before planned removal. Deprecation includes:

1. a Rust `#[deprecated]` annotation with a replacement and target release;
2. an entry under `Unreleased` in `CHANGELOG.md`;
3. at least one minor release before removal in the `0.x` line;
4. removal only in a later minor release before `1.0`, or a major release after
   `1.0`.

Immediate removal is reserved for unsoundness, false-success paths or security
defects and must be called out prominently in the changelog.

## Persistence

Schema versions are independent from crate versions. Import always validates size,
syntax, IDs, references, finite values, geometry, branch state and the solved
candidate before publication. Unknown future versions reject atomically.

| Domain | Accepted input | Canonical output | Migration |
| --- | --- | --- | --- |
| Sketch | v1, v2, v3, v4 | v4 | Frozen old languages migrate directly to v4 |
| Planar linkage | v1 | v1 | None required |
| Spatial assembly | v1 | v1 | None required |

Canonical output is byte-stable for the same accepted document and schema version.
Runtime generational IDs never form persisted identity. A schema language is never
expanded after release; new fields or variants require a new schema version and a
frozen reader for each retained old version.

Any future sketch v5 transition must retain direct deterministic migration from v1-v4
and uses separately versioned host-parameter, immutable external-snapshot and desktop-
workspace envelopes. Host expressions, PDM keys, projection callbacks and application
undo are not added to canonical sketch equations. The current table remains the supported
contract until a future explicitly scoped schema milestone updates it.

The project supports reading every schema listed above throughout the `0.2` line.
Dropping an input schema requires a minor release before `1.0`, a major release
after `1.0`, a changelog entry and an external migration path. A migration that
cannot preserve explicit branch or ownership state must reject or retain the old
semantics; it must not infer a different branch from coordinates.

Planar and spatial v1 in-memory records are the frozen v1 language for `0.1.0`.
Before either model gains a new persisted field or variant, it must first be split
behind a private v1 wire DTO in the same manner as sketch persistence.

## Features and platform support

The release has no optional Cargo feature contract. Native Linux x86-64 and
`wasm32-unknown-unknown` are release-gated. Other Rust-supported targets are
best-effort unless added to a future release matrix. Linux, Windows and macOS release expansion is
not currently scheduled. No C ABI is in the currently approved roadmap. The WASM workbench is
not a separate product API and does not define document semantics; cleanup qualification
uses direct Rust/WASM tests rather than browser E2E, and there is no mobile or responsive
support contract.

## Publication

The publishable crates are released in dependency order:

1. `geosolve-geometry`;
2. `geosolve-core` after the matching geometry version is visible;
3. `geosolve-sketch` and `geosolve-linkage` after the matching core version is
   visible;
4. `geosolve-sketch-ops` and `geosolve-sketch-topology` after the matching sketch version is
   visible;
5. `geosolve-constraint-editor` after the matching sketch and sketch-operations versions are
   visible.

Cargo cannot create a registry-ready dependent archive before its path dependency
version exists in the registry. The pre-publication gate therefore checks the exact
archive file list for all seven crates and builds every workspace target from path
dependencies. Each package includes `LICENSE` and `README.md`. Registry publication
itself remains a maintainer action after a repository URL and release tag exist.
