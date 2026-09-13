<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Architecture

GeoSolve separates numerical solving, domain models, headless interaction, authored
source, and application hosting. The same Rust domain and interaction code runs
natively and in WebAssembly. The demo shows how to compose these libraries; an
embedding application can supply its own renderer, input adapter and storage.

## Ownership map

| Layer | Responsibility |
| --- | --- |
| `geosolve-core` | Variables, residuals, normalized linearization, nonlinear solving, rank, hierarchy and diagnostics. |
| `geosolve-geometry` | Shared geometric evaluation and manifold operations. |
| `geosolve-sketch` | Persistent 2D sketch design, constraints, branches, dimensions, accepted state and transactions. |
| `geosolve-linkage` | Separate planar/spatial mechanism model, assemblies, joints, continuation and velocity queries. |
| `geosolve-sketch-features`, `-ops`, `-topology` | Feature semantics, drafting operations and certified planar profiles. |
| `geosolve-constraint-editor` | Presentation-independent picking, selection, tool drafts, dragging, dimensions and scene interaction. |
| `geosolve-sketch-intent`, `geosolve-sketch-code` | Authored identities, semantic/source artifacts, native lowering and source edit plans. |
| `geosolve-sketch-engine` | Editable sessions, authored transactions, accepted results, history and headless exports. |
| `geosolve-sketch-engine-wasm`, `@geosolve/engine` | WASM binding and TypeScript embedding API for the shared engine. |
| `@geosolve/sketch-code` | Typed TypeScript authoring, restricted reversible source compiler and generator controls. |
| `geosolve-collaboration`, its WASM crate and TypeScript package | Shared text, authoritative operations, synchronization and personal-history contracts. |
| `geosolve-demo-web` and its frontend | WASM demo adapter, rendering, browser input, editor panels and browser persistence. |
| `@geosolve/cli` | Node hosts, project files, generator execution, recovery, collaboration gateway and offline distribution. |
| `geosolve-sketch-render`, `geosolve-headless` | Static rendering and browser-free inspection/export. |

Sketch and linkage remain independent domain models over the numerical core.
Neither the browser renderer nor the TypeScript authoring layer implements solver
equations. Tool catalogs and completed-authoring receipts come from native owners;
hosts consume them instead of maintaining parallel geometric recipes.

## From source to accepted geometry

TypeScript source compiles into named semantic intent. Rust lowers that intent
into a retained sketch design and evaluates it through the domain solver. An
accepted result includes independently validated geometry, audit and provenance.
The headless editor projects that result into selectable, draggable scene objects.

A supported UI edit produces a source mutation and, where needed, an explicit
semantic design-sidecar update. The compiler and engine validate the exact
candidate before a host publishes it. Authored IDs, branches, source provenance
and revision checks link this round trip. A generator may expose typed inputs and
multiple outputs, but arbitrary generated geometry has no automatic reversible
source mapping.

The system keeps four kinds of state distinct:

- Working source and design intent, which may be incomplete or invalid.
- The last independently accepted model and its authoritative scene.
- Provisional tool or drag geometry, which cannot acquire publication authority.
- Personal presentation, including camera, selection and dimension visibility.

Rejected work retains the accepted model and history. A candidate's appearance in
a preview is not proof that it is accepted. The reusable `EditableSession` and
`WorkbenchSession` boundaries share mechanics while leaving each host's storage,
history and publication rules explicit.

## Numerical contracts

The solver normalizes residuals and tangent columns before convergence and rank
decisions. Hard validity, nonlinear termination, secondary optimization and
rank/mobility are independently inspectable. Ordinary constraints are hard;
weighted least squares is not an implicit priority policy.

Acceptance requires finite values, independent hard-residual validation and valid
domains/branches. Invalid knots, rational poles, collapsed geometry, zero-speed
jets, NaN and infinity reject transactionally. Discrete orientation, winding,
contact neighborhoods and assembly choices are explicit state.

The [detailed architecture reference](docs/reference/ARCHITECTURE_DETAILS.md)
specifies normalization, machine-floor rank thresholds, nullity, active bounds,
priority semantics, curve domains, persistence and linear algebra policy. Those
contracts and the [acceptance thresholds](ACCEPTANCE.md) remain authoritative.

## Local interaction and shared authority

In shared mode the server owns accepted edits and conflict resolution. Unfinished
text synchronizes separately from the accepted geometric model. Operation identity,
revision checks, receipts and durable outcomes make stale requests and retries
explicit; personal Undo preserves peers' contributions when its ownership checks
succeed.

Each client holds an authenticated accepted-scene projection. Camera navigation,
picking, selection and highlighting use the shared Rust interaction code locally.
Expensive authoring and solving run in separate workers or on the server. Clients
can predict supported gestures with the same engine, but only server publication
changes shared authority. Reconciliation is scoped to the exact accepted model
and request identity.

The demo greys the canvas after 500 ms of pending work while local navigation
continues. Shared editing is a bounded reference implementation: source/history
limits, backpressure, restart and conflict behavior are explicit. It is not an
unmeasured large-scale hosted service. See the [collaboration contract](docs/M98_COLLABORATION.md)
and [known measured limits](ACCEPTANCE.md#current-limits-and-review-status).

## Design records

The [ADR index](docs/adr/README.md) explains major decisions and superseded
approaches. [API compatibility](docs/API_COMPATIBILITY.md) defines public and
experimental surfaces. The [milestone index](docs/history/README.md) records how
these boundaries evolved; historical operational instructions are not current
architecture requirements.
