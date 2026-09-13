<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# API and persistence compatibility

This document describes the current compatibility boundaries. For package selection,
see [Architecture](../ARCHITECTURE.md); for executable examples, see
[Getting started](GETTING_STARTED.md) and [Authoring](AUTHORING.md).
[Architecture decisions](adr/README.md) retain the reasoning behind earlier APIs.

## Versions and stability

The Rust workspace version is `0.2.0`. The TypeScript authoring SDK uses `0.2.0`;
the engine, collaboration and CLI packages currently use `0.1.0`. Their manifests
are the version authority. These numbers do not imply that every package has been
published to a registry: the maintained Node installation workflow uses four
matching local archives.

Before `1.0`, a minor release may contain source-breaking changes. Patch releases
remain source-compatible except where retaining behavior would preserve unsoundness,
false success, invalid accepted geometry or a security defect. After `1.0`, Rust
API compatibility follows Cargo SemVer. The workbench and its private browser
protocol are example-host interfaces, not a stable application SDK.

The workspace baseline is Rust `1.89`; the collaboration crates require Rust
`1.90` for their pinned Automerge dependency. Raising a crate's minimum supported
Rust version requires a minor release before `1.0`, a major release afterward,
and a changelog entry.

## API layers

| Layer | Intended entry points |
| --- | --- |
| Sketch domain | `SketchDocument`, accepted-only `SketchDocumentSession`, and `RetainedSketchDocumentSession` for separate design, attempt and accepted views |
| Linkage domain | `PlanarLinkageDocument`/`PlanarLinkageSession` and `SpatialAssemblyDocument`/`SpatialAssemblyDocumentSession` |
| Geometry operations | `geosolve-sketch-ops` prepared proposals, `geosolve-sketch-topology` accepted profiles, and `geosolve-sketch-features` computed-feature intent |
| Headless UI | `geosolve-constraint-editor` scenes, normalized input, selection, drafts, dimensions and typed effects |
| Authored source | `geosolve-sketch-intent`, `geosolve-sketch-code` and `@geosolve/sketch-code` for typed declarations, source preparation and authenticated compilation |
| Embedding engine | `geosolve-sketch-engine` and `@geosolve/engine` for accepted evaluations, editable sessions, replayable authoring and profile export |
| Collaboration | `geosolve-collaboration` and its dedicated WASM/TypeScript bindings for shared text, ordered authority and personal contribution history |
| Reference hosts | The React workbench, `@geosolve/cli` local/shared server and `geosolve-headless` rendering tools |

Legacy direct `Sketch`, `Linkage` and `SpatialAssembly` builders remain
compatibility facades in the `0.2` line. Intent, authored source, engine and
collaboration are evolving pre-1.0 companion APIs. Applications can use the sketch
and headless editor without depending on TypeScript execution or a browser host.

Compiler products, runtime ID maps, direct `geosolve-core` reports, fixture builders
and performance builders are explicitly unstable diagnostic surfaces before `1.0`.
Use persistent domain IDs and domain-owned views for application identity. A public
DTO, serialized scene or copied success field does not confer accepted-state or
publication authority; the owning engine independently validates candidates.

Public error/status enums may gain variants. Include a wildcard arm unless an enum
is documented as closed. Report structs may gain fields in a minor `0.x` release.
Request and persisted-document languages are never extended silently within a
released schema version.

## Deprecation

A supported domain API is deprecated before planned removal, with:

1. a Rust `#[deprecated]` annotation naming a replacement and target release;
2. an entry under `Unreleased` in `CHANGELOG.md`;
3. at least one minor release before removal in the `0.x` line; and
4. removal only in a later minor release before `1.0`, or a major release afterward.

Immediate removal is reserved for unsoundness, false-success paths or security
defects and must be called out in the changelog. Unreleased experimental facades
may be retired without becoming supported APIs. For example,
[ADR 0030](adr/0030-headless-sketch-operation-authoring.md) was superseded by the
computed-feature authoring path; the underlying supported Fillet, Offset and
Mirror domain operations remain separately owned.

## Domain persistence

Schema versions are independent of crate versions. Import validates size, syntax,
IDs, references, finite values, geometry and explicit branch state. A reconstructed
candidate must pass independent validation before publication. Unknown future
versions reject atomically.

| Domain | Accepted input | Canonical output | Migration |
| --- | --- | --- | --- |
| Sketch | v1, v2, v3, v4 | v4 | Frozen old languages migrate directly to v4 |
| Planar linkage | v1 | v1 | None required |
| Spatial assembly | v1 | v1 | None required |

Canonical output is byte-stable for the same accepted document and schema version.
Runtime generational IDs never form persisted identity. A released schema language
is frozen: new fields or variants require a new version and retained readers for
supported old versions.

The private sketch draft-v5 representation is **not a supported domain schema**.
Some newer computed-feature, Profile Offset and datum relations cannot be encoded
in canonical sketch v4; encoding must reject unsupported state rather than lose it.
Use the owning source/project/workspace codec when saving a complete authored
project. A host workspace version does not release a new sketch-domain language.

The `0.2` line retains every domain input schema listed above. Dropping one requires
a minor release before `1.0`, a major release afterward, a changelog entry and an
external migration path. A migration must preserve explicit branches and ownership
or reject; it cannot select a new branch from coordinates. Planar and spatial v1
in-memory records retain their frozen language until a future private wire DTO
separates new model fields from the old reader.

## Source, workspace and host formats

These formats have separate owners and lifetimes:

- **Intent graph/session:** canonical wire v2 uses SHA-256 identities. The bounded
  experimental v1 reader validates its original canonical fields, nested state and
  integrity fingerprints before deterministic migration; legacy FNV-1a is not a
  cryptographic authentication primitive.
- **Managed compiler envelope:** current source compilation emits V4 IR/execution
  receipts. The bounded V3 reader preserves existing source and history on load;
  the first authenticated edit recompiles a V4 candidate. V1/V2 are unsupported.
  The [authoring SDK](../packages/geosolve-sketch-code/README.md) documents the source
  metadata and compiler entry points.
- **Saved source/native history:** the source codec reads its v4/v5 workspace
  envelopes, including unfinished text and native Current/Undo/Redo checkpoints.
  Restoration independently validates current and historical authority. The engine
  can opt into complete persistable history; a semantic design sidecar alone is
  not a complete Undo/Redo archive.
- **Local folders:** new projects use `geosolve-folder-v2` with editable or generator
  mode. The retained v1 reader covers the original single-file fixture. Source,
  semantic design, input values, recovery data and personal presentation have
  distinct owners; see the [storage contract](M98_WORKSPACE_STORAGE.md).
- **Reproduction transport:** `GEOSOLVE_REPRO_V1` wraps bounded compressed workspace
  text. Its corruption check is not publication authority. Successful decoding
  still requires the appropriate complete workspace decoder and reconstruction.
- **Collaboration:** native text checkpoints and the ordered host journal retain
  their versioned identities and checked contribution ownership. Durable recovery
  reconstructs and validates accepted source/model state before serving edits.
  [Collaboration APIs](../packages/geosolve-collaboration/README.md) describe the
  trusted completion and persistence boundaries.

M99 consolidated these codecs and native services under reusable owners without
introducing a new wire version. Malformed nested history, stale source receipts,
foreign session tokens and unsupported future formats continue to reject.

## Transient interaction and rendering

The workbench request/reply protocol is v2. Numeric `geosolve-draw-frame-v1` frames
replace the old SVG browser payload; native SVG and PNG export remain available.
JS and WASM consumers ship together, and incompatible request versions reject.

Detached scene JSON retains native analytic curves, explicit computed geometry,
annotations and camera context for local navigation and picking. Import creates
presentation data, never an accepted session, solver certificate or prepared edit.
Exact scene identity, source correspondence and server/session revision checks
remain mandatory when converting local selection into an authored operation.

Personal camera, selection, visibility, annotation placement and dimension pins do
not change source, constraints or accepted geometry. `isKeyConstraint`,
`isKeyParameter` and document dimension defaults are authored metadata with normal
source/history validation; they do not change hard/soft solver priority.

## Platforms, packaging and qualification

All solving is pure Rust. The project has no solver FFI, C ABI or optional Cargo
feature compatibility contract. Native Linux x86-64 and `wasm32-unknown-unknown`
are release-gated; other Rust targets are best-effort. The full offline CLI requires
Node 22 or later and Linux `flock`; its archive includes an esbuild binary for its
recorded CPU architecture. Browser/Node engine WASM packages are independent of
that CLI binary.

The desktop workbench targets layouts of at least 1024 × 720. It demonstrates
embedding and collaboration, with bounded tested workloads rather than a mobile,
arbitrary-document-size or production service-capacity promise.

The release gate checks Rust package file lists, native and WASM behavior, golden
scenarios, browser workflows, performance limits and installed Node packages.
Packages retain the project license and required third-party notices. Registry
publication and website deployment are separate release actions; building or
qualifying local archives performs neither. See
[Release qualification](RELEASE_QUALIFICATION.md) for exact commands and evidence.
