<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# @geosolve/sketch-code

Typed, equation-free TypeScript authoring for GeoSolve sketches and reusable patches.
The source owns geometry, dimensions, public parameters and their presentation.
A compiler host executes the admitted authoring program; Rust validates the resulting
IR and execution receipt before materializing and solving a candidate.

## Author dimensions and public parameters

```typescript
"use geosolve sketch";
import { mm, sketch } from "@geosolve/sketch-code";

export default sketch({
  title: "Channel plate",
  description: "A plate with a shared passage width.",
}, ($) => {
  const channelWidth = $.parameter("channelWidth", mm(12), {
    label: "Channel width",
    description: "Full passage width, shared by its consumers.",
    isKeyParameter: true,
  });
  const plate = $.geometry.twoPointAlignedRectangle("plate", {
    firstCorner: [0, 0],
    oppositeCorner: [120, 60],
    label: "Plate outline",
  });
  const plateWidth = $.dimension.curveLength("plateWidth", {
    curve: plate.spans[0],
    value: mm(120),
    label: "Plate width",
    isKeyConstraint: true,
  });
  $.group("Plate", [plate, plateWidth]);
  return { plate, plateWidth, channelWidth };
});
```

`isKeyConstraint` makes an authored dimension eligible for the overview. It does not
change driving/reference mode, solver priority or the measured value. Reference
measurements and construction dimensions can also carry this flag. Collision
handling and the selected dimension-display mode still govern canvas callouts.

To make every directly authored dimension eligible, supply the document default:

```typescript
export default sketch({
  title: "Gridfinity section",
  dimensions: { areKeyConstraintsByDefault: true },
}, ($) => {
  return {};
});
```

The default is false. A local `isKeyConstraint: false` opts out; removing the local
property restores the document default. Generated patch measurements and public
parameters do not inherit this document switch.

`$.parameter(id, value, options?)` accepts a finite number or unit literal and returns
that same ordinary value type. Pass it directly to dimension values and patch inputs.
Its stable ID, lexical variable name and display label are separate. Stable IDs share
the declaration namespace, including ordinary scalar bindings' implicit identities;
duplicates reject. Two parameters with equal numeric values remain distinct, while
one shared parameter retains all its actual consumers. An unused named parameter is
still editable; native consumer restrictions apply when consumers exist.

The parameter owns `label`, `description` and `isKeyParameter` (default false).
Ordinary scalar bindings remain legal contextual controls. They do not acquire a
public name or overview status from an arbitrary consuming patch. Groups continue
to contain feature references; public scalar parameters have their own section.

## Describe reusable patch inputs

Patch dimensional schemas accept presentation beside their type:

```typescript
import { definePatch, t } from "@geosolve/sketch-code";

export const waterChannel = definePatch({
  polyline: t.feature("polyline"),
  width: t.length({
    label: "Channel width",
    description: "Full width across the passage.",
    isKeyParameter: true,
  }),
  bendRadius: t.length({ label: "Bend radius" }),
}, (p, { polyline, width, bendRadius }) => ({
  profile: p.computed.polylineChannel("channel", {
    polyline, width, bendRadius, caps: "end",
  }),
}));
```

`t.angle(options?)` accepts the same presentation options. Metadata adds no numerical
bounds and never widens native validation. An inline invocation input inherits its
schema's name, help and overview default. A named parameter used as that input keeps
its own presentation instead.

An invocation can have a name and description in a fourth argument:

```typescript
const upperChannel = $.use("upperChannel", waterChannel, {
  polyline: upperCenterline,
  width: channelWidth,
  bendRadius: mm(8),
}, { label: "Upper channel", description: "Reservoir to outlet." });
```

These options describe the invocation, not an input override map. To customize an
inline input's presentation, extract it into a named parameter. A full-width parameter
remains distinct from generated half-width offsets and other internal measurements.

## Source-backed editing and compatibility

The workbench's **Show in overview**, **Name** and **Description** controls edit these
source properties through Rust-prepared transactions. **Make named parameter** wraps
an ordinary scalar binding or extracts a supported inline numeric/unit literal before
its consuming declaration. Wrapping preserves the original variable and all uses;
inline extraction replaces only the chosen consumer expression. It does not make an
unsupported transformed value invertible. Reset removes the local property.

Title, description and dimension defaults belong to `sketch` options. Personal pins,
visibility modes and camera state retain their presentation owners. Metadata edits
use the same accepted-source checks, candidate validation and atomic Undo/Redo history
as other source changes. A stale request or failed candidate preserves accepted state.

`sketch(callback)`, unannotated bindings and three-argument `$.use` remain valid.
The compiler emits managed IR/execution receipts in V4. Rust also accepts the bounded
V3 format for existing saved projects and history; V1/V2 remain unsupported. Restoring
V3 does not silently rewrite its source. Its first authenticated edit recompiles a V4
candidate. Source-only copies retain authored metadata when compiled with their required
patches; offline project exports also retain the pinned patch artifacts and lock.

Display labels allow at most 256 UTF-8 bytes with no surrounding whitespace or control
characters. Titles allow 128 Unicode scalar values; descriptions allow 2,048. Descriptions
are plain text. Empty labels/titles use the normal identity/untitled fallback.

The public root exports authoring types and builders. Compiler APIs live at
`@geosolve/sketch-code/ir` and `@geosolve/sketch-code/compiler`; live control DTOs live at
`@geosolve/sketch-code/control`. Internal rewrite helpers are not public ticketless
editing APIs. See the [Rust companion README](../../crates/geosolve-sketch-code/README.md)
for native project admission and materialization.
