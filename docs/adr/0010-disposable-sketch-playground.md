# ADR 0010: Disposable browser sketch playground

Status: accepted

## Context

The existing WASM crate is a hardcoded compatibility/audit consumer. The user-approved M10-M14 cut needs a functional 2D Sketch Playground Alpha to exercise real editing workflows, but putting equations, document semantics or accepted state in the browser would create a second solver frontend and make a prototype UI an architectural dependency.

## Decision

M13-M14 turn `geosolve-demo-web` into a disposable, non-authoritative playground over public embeddable `geosolve-sketch` and `geosolve-geometry` APIs.

Reusable Rust APIs own:

- `SketchDocument`, `SketchSession`, typed commands and command history;
- versioned serialization/import validation and persistent-ID remapping;
- curve evaluation, constraints, dimensions, branch validation and all equations;
- accepted geometry, rollback, solve reports, diagnostics and structured audit rows.

The web crate owns only:

- selection, box-selection and compatible multi-selection presentation;
- hit testing, pointer/touch/keyboard routing and transient tool state;
- SVG/canvas rendering, viewport pan/zoom and responsive desktop/mobile layout;
- browser file transfer and `localStorage` integration.

The playground supports the exact M13 interaction scope in `PLAN.md`. Prospective coincident/horizontal/vertical inference is an uncommitted visual proposal. It changes no `SketchDocument` until the user confirms it and the web crate submits the corresponding public command. Rectangle creation invokes the library command macro; the web crate does not reconstruct its constraints.

Every edit, drag, undo/redo, import and autosave restore flows through public document/session/command/serialization APIs. Rendering uses only the accepted revision. Failed commands, solves or imports retain prior accepted geometry visibly; candidate geometry may be previewed only when clearly non-authoritative and must not be mixed with accepted diagnostics.

M14 adds desktop/mobile browser E2E for A1-A10, atomic malformed-import handling and deterministic small/medium performance measurements. Performance budgets may govern responsiveness or supported alpha document size but may not weaken residual tolerance, rank policy, branch validation or rollback.

The playground contains no residual, Jacobian, curve, measurement, contact/tangency, inference-commit or document-validation equations. It remains replaceable and is not the public API specification.

## Consequences

- Programmatic/native consumers can construct, edit, solve, audit and serialize every alpha workflow without a browser.
- Browser iteration cannot fork solver semantics or persisted identity.
- UI-only state is intentionally not portable document state.
- M14 can complete the playground alpha without claiming completion of Deliverable 1.
- The no-physics boundary is unchanged; browser dragging is a temporary geometric objective, not force or dynamics simulation.
