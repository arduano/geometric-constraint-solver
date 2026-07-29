<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0029: Headless constraint-editor state machine

Status: accepted

## Context

M40 human UAT found repeated failures in basic drafting, projected drag and canvas
selection after M39 browser automation had passed. The numerical and persistent
sketch layers were mechanically qualified, but interaction correctness was split
across DOM target ancestry, SVG markup, CSS hit strokes, browser event handlers and
private web application state. Native tests could not prove that a screen-space line
pick selected the intended persistent span, that multiselection exposed the intended
constraint, or that a click emitted no point edit.

Browser tests remain useful integration evidence, but they are too indirect to be the
primary oracle for deterministic editor transitions. Human UAT must assess usability,
not discover objective state-machine defects that a headless test can enumerate.

## Decision

Add the pure safe-Rust `geosolve-constraint-editor` crate as a presentation-independent
consumer over public `geosolve-sketch` APIs. It owns deterministic constraint-editing
interaction policy and exposes typed state, scene primitives, inputs, transitions,
available actions and effects for native, WASM and application consumers.

The allowed dependency direction is:

- `geosolve-constraint-editor` may depend on `geosolve-sketch` and
  `geosolve-geometry`;
- `geosolve-sketch`, `geosolve-geometry`, `geosolve-core` and
  `geosolve-linkage` may not depend on the editor; and
- `geosolve-demo-web` and external hosts may depend on the editor.

The editor may coordinate a public `RetainedSketchDocumentSession`, but accepted
geometry, equations, validation, rank, audit, persistent IDs and branch semantics
remain authoritative only in `geosolve-sketch`. An editor success-like lifecycle view
must be derived from the matching public session outcome and accepted identity.

The editor owns normalized input, viewport and interaction geometry, deterministic
accepted-scene tessellation through public curve jets, persistent point/span picking,
ordered selection, gestures, drafting, snapping, action applicability, typed effects,
lifecycle presentation state and deterministic replay fixtures.

It owns no DOM, renderer, widget toolkit, browser storage, platform event loop,
residual, Jacobian, measurement, projection or branch equation, alternative sketch
persistence schema, host expression system or B-rep behavior.

Presentation code maps platform events into normalized editor inputs, renders returned
scene/state DTOs and applies typed effects. It must not independently reinterpret
selection compatibility, gesture thresholds, drafting completion or lifecycle status.
DOM hit targets may improve accessibility or event routing, but are not authoritative
selection geometry.

## Verification policy

Native transition tests are the primary oracle. They enumerate exact boundaries,
input permutations, cancellation, stale revisions, malformed/non-finite inputs,
overlapping hits and deterministic persistent identity. Model-based action sequences
compare editor state and emitted public edits against explicit invariants.

WASM tests prove the same crate behaves under the target. Browser tests are limited to
adapter, rendering, accessibility, storage and platform-event integration. Human UAT
resumes only after the browser workbench is a thin consumer of the mechanically
qualified editor state machine.

## Consequences

Selection and drafting policy move out of `geosolve-demo-web`; presentation-specific
code becomes smaller and replaceable. The editor is a public pre-1.0 crate and may
evolve until a future API-freeze milestone is explicitly scoped. Existing M39/M40 browser evidence is retained as
historical regression evidence but does not qualify the replacement architecture.
