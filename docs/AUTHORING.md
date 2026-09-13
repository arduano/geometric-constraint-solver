<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Authoring with code and the canvas

GeoSolve gives a sketch a readable TypeScript source representation. You can
change supported geometry and dimensions in the workbench or edit the same source
in an editor. An AI agent can work through files or revision-checked CLI commands;
the compiler and Rust engine validate changes before they become accepted geometry.

Use [Getting started](GETTING_STARTED.md) to open the browser workbench or run a
local project. The [small folder example](../examples/file-workspace/README.md)
is a useful first design; the [manifold](../examples/file-workspace-manifold/README.md)
shows reusable patches and shared parameters.

In the browser, **New sketch** starts the visual workflow: author directly with
canvas tools and dimensions. **Start from code** opens a sketch with editable
TypeScript source.

## Choose editable or generator mode

| Mode | Typical use | UI edits |
| --- | --- | --- |
| Editable sketch | Humans and agents drawing and editing the same source | Supported tools, dimensions and Inspector edits update authored source, with explicit semantic sidecar state where required. |
| Generator | Ordinary TypeScript computes geometry using loops, conditions and helper modules | Typed controls rerun the generator. Output geometry does not automatically gain reverse source editing. |

Editable sketches declare `"use geosolve sketch"` and use the supported reversible
TypeScript vocabulary. The SDK records geometry, constraints, dimensions, groups,
parameters and patch instances under stable authored names. This is a defined
source language with an inspectable representation, not a promise to reverse
arbitrary TypeScript programs.

In a local editable folder, `geosolve.json` selects the entry file and project
mode. Source owns names, help, overview intent and public input definitions.
`.geosolve/design.json` carries necessary semantic overrides that cannot be
represented by the supported source edit; caches and personal presentation have
different authority. See [storage and recovery](M98_WORKSPACE_STORAGE.md).

## A normal editing session

1. Open a sketch and navigate to the geometry of interest. Selection reveals its
   contextual dimensions and Inspector controls.
2. Edit a driving dimension, drag a free point or use a native construction tool.
   The engine checks the candidate and publishes its supported source change.
3. Edit the source directly to change declarations, names or reusable structure.
   The code compiles back through the same native geometry and validation.
4. Use Undo/Redo, diagnostics and the visible pending state to inspect the result.
   An invalid draft can remain visible as text while the canvas retains the last
   accepted geometry. Provisional geometry is not an accepted model.

The standalone browser owns its local document. A folder server watches ordinary
files and coordinates one editing lease between a browser tab and an agent. A
shared server synchronizes multiple editors' unfinished text and accepts model
operations authoritatively. In shared mode, Apply captures a particular draft;
later typing remains unapplied. Cameras, selection and other personal views stay
local while geometry edits are processed independently.

## Make design intent visible in source

Name an important reusable input once and mark it for the overview:

```ts
const channelWidth = $.parameter("channelWidth", mm(12), {
  label: "Channel width",
  description: "Full passage width, shared by all four channels.",
  isKeyParameter: true,
});
```

Pass that parameter to each consuming patch. Its identity, not equality of its
numeric value, determines whether consumers share one public control. An inline
patch input can inherit its definition's labels/help; an extracted named parameter
owns its own metadata.

Dimensions support `label`, `description` and `isKeyConstraint`. A document-wide
`dimensions.areKeyConstraintsByDefault` option marks every directly authored dimension
as overview intent, as in Gridfinity. It defaults to false and does not apply to
generated patch dimensions or public parameters. Explicit per-dimension overrides
remain possible.
The Inspector's overview controls edit these source properties. They are not
hidden sample-manifest heuristics and do not change the solver's priority semantics.

Focused dimension presentation shows relevant overview and selected measurements
without exposing every generated offset at once. Hover previews, selection,
Inspector rows and personal pins provide more detail. Hidden mode suppresses
canvas callouts; collision handling may omit overlapping labels, so overview
intent does not guarantee every label is painted simultaneously. Personal view
choices never rewrite authored intent. See [metadata APIs](M97_AUTHORING_METADATA.md)
for full defaults and migration behavior.

## Reuse patches and generated inputs

Custom patches turn a typed input into a named set of ordinary native geometry
and constraints. The manifold's channel patches take polylines and widths, offset
the walls, round bends and cap endpoints. The resulting water channels and seal
grooves have real planar area. Their public width is separate from generated
half-width offsets shown in expanded inspection.

For general computation, `defineGenerator` declares typed defaults, labels,
units, ranges and choices in code. A host can build controls from that schema and
run the same generator in Node or a browser worker. The
[generator website](../examples/generator-website/README.md) demonstrates this
without importing the demo UI. Generator code is trusted host code; workers
provide cancellation and isolation from the UI event loop, not a security sandbox.

## Work with an agent

An agent can read and modify the ordinary source files. For coordinated live
changes, use CLI status and explicit mutation commands so a stale edit does not
silently overwrite a newer one. Read fresh expected state with a stable client
ID, acquire the folder's editing lease when needed, and submit a unique operation
ID for each new intent. If a response is lost, query its outcome or retry the exact
original request; do not create a second operation for an uncertain first one.

The [CLI guide](../packages/geosolve-cli/README.md) documents commands and platform
requirements. The [complete authoring quickstart](M98_AUTHORING_QUICKSTART.md)
includes an executable file-map edit, explicit lease takeover and shared-mode
commands. Shared personal Undo preserves peer contributions when its ownership
checks succeed; conflicts, replacement lifetimes and stale authority remain
explicit failures rather than inferred merges.
