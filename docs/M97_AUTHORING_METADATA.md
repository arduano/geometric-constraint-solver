<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 amendment — author design intent in source

## Status and decision

This is the completed design and implementation plan requested after reviewing the
default-priority preview. The APIs and interactions below are **planned, not yet
implemented**. M97 remains open; the qualified product is still
`fc3fdcb71b4d910815a51cf030128c42317ff2ed`, served at
`http://100.94.63.83:18104/`. Its catalog-owned priorities are the starting point,
not evidence that this amendment works. [Current qualification](M97_PRIORITY_DIMENSIONS.md)
and [M97 goals](M97_GOALS.md) retain the existing product contract.

Put reusable design intent beside its declaration. A dimension's overview status,
a parameter's public name/help, and the document's title must travel with ordinary
sketch source and its patches. The Inspector edits those same properties. Authors
must never maintain a parallel list of declaration names or field paths in a sample
manifest to make a design usable.

Keep the API small: extend existing declaration options, add one named parameter
builder and one document-options overload, and allow patch schemas to describe their
public dimensional inputs. Reuse existing groups and display labels.

## Everyday workflow

1. Create a dimension on canvas or select an existing one. The Inspector provides
   **Name**, **Description**, and **Show in overview** beside the existing value and
   driving/reference controls. The name is a display label; source identity stays
   separate. The overview toggle is an accessible checkbox, distinct from a temporary
   pin. Its help reads “Keep this measurement available without selecting its geometry.”
2. Toggle overview or edit a name. Commit through the ordinary source transaction;
   when Split is already visible, it shows the small source change without moving
   the cursor or focus. Undo/Redo, save, reload and export carry
   it. A reference measurement can change presentation while its measured value stays
   read-only. Reset to document default removes the local `key` override.
3. For a shared input such as channel width, use the source-ordered **Parameters**
   section. One row shows “Channel width · 12 mm”, its help and all four channel
   consumers. Selecting any channel reveals that row. Editing it updates every actual
   consumer through the current validated value-edit path.
4. An inline patch input or supported ordinary scalar binding offers **Make named
   parameter**. This extracts/wraps its exact editable value as `$.parameter(...)`,
   preserving units, uses and comments. The new parameter then owns its name, help
   and overview status. If the value has no supported reverse edit, show its existing
   read-only reason and source navigation; do not invent an editable literal.
5. In document properties, edit title/description or enable **Show authored dimensions
   in overview by default**. Gridfinity uses this default. Generated patch dimensions
   stay under their existing disclosure. A local dimension override can opt out.

Overview status keeps a measurement eligible; current collision handling may still
hide its canvas label. It remains in the Inspector. Focused/All/Hidden, selection,
hover and the four personal pins retain their current ranking and behavior. Overview
metadata never changes solver hard/soft priority, driving/reference mode or geometry.

For an empty overview, show a short hint and an **All measurements** disclosure with
the same row actions. An unmarked imported document must remain discoverable without
automatically promoting arbitrary dimensions. A newly created GUI dimension writes
`key: true` explicitly. Source-authored dimensions use the document default.

## Source API

The following is proposed syntax. Existing `sketch(callback)` and unannotated
declarations remain valid. This shortened example uses existing rectangle outputs;
the complete manifold retains its current anchors, constraints and patches.

```ts
export default sketch({
  title: "PC water manifold",
  description: "A distribution plate with four water passages and a silicone seal.",
}, ($) => {
  const channelWidth = $.parameter("channelWidth", mm(12), {
    label: "Channel width",
    description: "Full passage width, shared by all four channels.",
    key: true,
  });

  const plate = $.geometry.twoPointAlignedRectangle("plate", {
    firstCorner: [-120, -60],
    oppositeCorner: [120, 60],
    label: "Manifold plate",
  });
  const plateWidth = $.dimension.curveLength("plateWidth", {
    curve: plate.spans[0],
    value: mm(240),
    label: "Plate width",
    key: true,
  });

  // Pass channelWidth directly as a patch input, just like mm(12).
  return {};
});
```

Gridfinity's document options include:

```ts
dimensions: { keyByDefault: true }
```

`keyByDefault` applies to directly authored dimensions, including reference
measurements and dimensions on construction geometry. Its default is false.
Explicit `key: false` always opts out, even when no other key dimensions remain.
Patch-generated measurements and public parameters do not inherit this switch.

`$.parameter(id, value, options?)` introduces an explicitly public, named value,
with `label`, `description` and `key` options. Initially accept finite numbers and
unit literals, matching the existing supported scalar binding kinds. Boolean/string
public parameters are deferred; their existing editable control leaves remain supported.
The return type remains compatible with the supplied value; this is not a new
solver variable or expression language. An unused parameter is still visible and
editable as an authored value, with a base schema derived from its explicit literal
type and no geometric effect. Intersect that base schema with every native consumer
restriction when consumers exist; never fabricate a consumer for metadata authority.
Existing unmarked bindings remain
legal and retain their current contextual control behavior.

The stable `id` belongs to the same declaration namespace as geometry IDs. Duplicate
IDs reject. Local variable spelling, stable ID and display label are separate.
The compiler must preserve source identity even for two equal primitive values;
runtime numeric equality is never evidence that they share a parameter.

Reuse the current `label` property throughout the authoring surface. Add optional
plain-text `description` to the same presentation options. A patch invocation can
use an optional fourth `$.use` argument containing only `label` and `description`:

```ts
const upperChannel = $.use("upperChannel", waterChannel, {
  polyline: upperCenterline,
  width: channelWidth,
  bendRadius: mm(8),
}, { label: "Upper channel", description: "Reservoir to upper outlet." });
```

This describes the invocation itself. It does not contain an input selector map
or override the named parameter. Existing three-argument calls are unchanged.

Patch authors describe dimensional inputs beside their schemas:

```ts
export const waterChannel = definePatch({
  polyline: t.feature("polyline"),
  width: t.length({
    label: "Channel width",
    description: "Full width across the passage.",
    key: true,
  }),
  bendRadius: t.length({
    label: "Bend radius",
    description: "Centreline radius; must exceed half the channel width.",
  }),
}, (p, { polyline, width, bendRadius }) => ({
  profile: p.computed.polylineChannel("channel", {
    polyline, width, bendRadius, caps: "end",
  }),
}));
```

`t.length()` and `t.angle()` retain their current type semantics and accept optional
label/description/key defaults. Help text does not enforce a radius restriction;
the existing native channel validation remains authoritative. New author-defined
bounds, sliders, precision, units of display and arbitrary widget schemas are outside
this amendment. Existing consumer-derived validation may never be widened by metadata.

## Ownership and resolution

| Item | Authoritative source and rule |
| --- | --- |
| Dimension overview | Local `key` overrides document `dimensions.keyByDefault`; omitted document default is false. |
| Named parameter | Its own options, with key default false and label fallback to its ID. Consumer schema metadata cannot rename or promote it. |
| Inline patch input | The exact invocation input inherits its patch schema's label/help/key. To customize presentation, extract a named parameter. |
| Ordinary shared binding | Keep its existing single contextual control and identifier fallback. Do not arbitrarily inherit one consumer's schema. Make it named to author its public presentation. |
| Declaration label/help | Existing source `label` plus optional `description`; fallback label is the stable declaration ID. Apply consistently in Explorer, Inspector, dimensions, navigation and accessibility. |
| Document title/description | Optional `sketch` options. Without title, display “Untitled code sketch”; absent description is empty. Catalog identity grants no document metadata. |
| Groups/order | Existing `$.group` declarations own geometry organization. Parameters have one source-ordered section and may have consumers in several groups. |
| Catalog | Keep ordinal, category, sample key, expected DOF and provenance/NOTICE records. Derive displayed title, summary and group names from compiled source. |
| Personal view | Focused/All/Hidden, pins, camera, hover, selection, hide/isolate and annotation placement keep their current owners and persistence rules. |
| Compiler authority | Source spans, consumer IDs, generation digests and edit capabilities are generated, validated data. Authors cannot set them as metadata. |

Do not add `$.group("Flow", [channelWidth])` in this amendment: groups currently
take feature references, while parameters preserve ordinary value types. Sharing
does not give a parameter one arbitrary owning geometry group.

Deduplicate parameter rows by authenticated source-control identity, never name or
numeric value. A shared parameter has one edit route and lists its consumers. If it
also drives a native dimension, retain that dimension's independently inspectable
measurement and canvas identity; direct the value edit to the shared parameter and
avoid duplicating it in the overview summary. Different sources with equal values
remain distinct. Metadata alone does not make a transformed value invertible.

A full-width 12 mm parameter must remain 12 mm; it neither renames the generated
6 mm wall offsets nor promotes every internal consumer. Generated measurements
keep their current contextual visibility and truthful edit restrictions. Their
overview toggle is unavailable in this first version; the Inspector links to the
owning patch/public input. Per-instance generated-output overrides are deferred.

Name changes do not rename symbols, change selection ownership or retarget pins.
Renaming a source variable preserves its explicit parameter ID. Deleting a declaration
removes its metadata with it; unresolved references reject through normal compilation.
Descriptions render as plain text, never markup or executable content. Validate all
fields in TypeScript and Rust, including unknown fields, types and finite numeric values.
Labels obey the existing native `IntentKey` rules: at most 256 UTF-8 bytes and no
padding or control characters. Titles allow 128 Unicode scalar values and help allows
2,048; these fields are distinct from native display-name keys. Empty labels/titles
use the documented fallback before constructing a native key; description may be empty.

## Source-backed Inspector edits

Use a dedicated bounded metadata mutation, alongside existing prepared value edits.
Ordinary control capabilities currently require proven value consumers; metadata
must not masquerade as a solver input just to become writable.

1. Rust prepares the exact target and allowed metadata delta against accepted
   source, IR/artifact digests, project revision and custom-patch inputs. This supports
   inserting, replacing and removing properties, document options and parameter
   extraction. A shared input edit targets the parameter, not a consumer copy.
2. The managed compiler rewrites owned source spans, preserving unrelated code,
   comments, units, explicit IDs and group order, and returns the new authenticated
   compilation. Document titles and metadata have independent owned spans from values.
3. Rust validates the complete envelope and permitted semantic delta, cold
   materializes/validates the candidate through the existing accepted-state path,
   and publishes source, projections and history atomically. Metadata-only edits must
   preserve geometry, residuals, DOF/rank, branch state and generated identities.
4. Undo/Redo, export/import and reload replay the same source-owned state. A failed
   compile, invalid retained draft, stale request or publication failure leaves the
   complete accepted source/scene/metadata intact. Pending source edits never get
   overwritten by a metadata control; use the existing apply/discard workflow.

Add typed document/parameter/presentation records and source sites to both managed
IR and execution receipts. Authenticate patch-schema metadata against custom-file
digests as well. Runtime recording, independent IR evaluation and Rust validation
must agree; a frontend DTO or cached catalog flag cannot grant authority. Parameter
value provenance must survive direct references without relying on boxed primitive
identity or value equality. This is the first implementation checkpoint.

Version the changed managed IR/executed artifact envelope explicitly; do not silently
change V3's meaning. The current reader accepts exactly V3 and rejects V1/V2. Add an
explicitly bounded V3 compatibility reader alongside the new envelope. Preserve old
saved source and history without automatically normalizing their text; upgrade an
entry only through an authenticated source transaction. Distinguish authored metadata
from derived projections. Extend persistence envelopes only where
required by their strict validators; do not store a second mutable metadata owner.

## Catalog migration and compatibility

Migrate the current bundled source before removing runtime catalog selectors:

- Gridfinity and other `all_authored` entries get the explicit document default.
  Other samples put `key: true` on the exact current curated dimensions.
- Manifold preserves its six selected dimension identities. Convert the shared
  `channelWidth` binding into the named 12 mm parameter without changing its uses;
  extract the inline 2.4 mm groove width into a named `sealGrooveWidth` parameter.
  Do not change any channel geometry, numeric targets or grouping.
- Move live sample title/summary into source title/description, and derive catalog
  title/summary/group projections at build time. Remove `dimension_presentation`
  from live manifests and the origin lookup from dimension presentation. Keep
  independent sample assertions in qualification fixtures; preserve group uniqueness,
  nonempty groups and exactly-once ownership checks. Do not replace them with
  assertions that merely compare two copies of generated output.

All samples then use the same mechanism as a new authored project. Copying its source
and patches into a standalone project and recompiling through the normal authenticated
patch/project pipeline preserves names, help and key status. Existing exports may
instead carry their required pinned patch artifacts/project lock. These generated
compilation records remain necessary for execution authority; authors do not edit
them to describe a dimension or parameter.
Renaming a sample key or removing its catalog entry cannot change that behavior.

Existing source without the new metadata still compiles with contextual defaults.
Retire the arbitrary first-six priority heuristic. Old saved source is not silently
rewritten on load or replaced by a newer catalog sample: its exact text, geometry
and history remain intact. Its old implicit overview priorities may disappear until
the author uses the new controls; this is an intentional presentation compatibility
change, with the empty-overview discovery path above. Archived fixtures and old
qualification artifacts retain their original bytes and readers. Old pins/manual
placements must continue to resolve only their exact surviving identities.

## Implementation order and acceptance

Amend the open M97 milestone in this order; these are uncompleted obligations:

1. **Compiler/domain:** SDK options and parameter API, managed parser/source sites,
   executed metadata, Rust validation, provenance and version compatibility. Prove
   two equal-valued named parameters stay distinct; a shared one retains every real
   consumer and exact value-edit authority.
2. **Transactions/projections:** prepared structural metadata edits, atomic history,
   consistent labels/help/title, source-driven priority resolution and shared row
   deduplication. Regress stale envelopes, tampered metadata, unrelated semantic
   changes, failed drafts and publication failure at the owning Rust boundary.
3. **Authoring UI:** checkbox/reset, name/help fields, document defaults/properties,
   parameter extraction and discovery. Keyboard access and accessible names must
   follow the existing Inspector. GUI-created dimensions write explicit intent.
4. **Samples:** migrate the live catalog and regenerated compiler artifacts, preserve
   legal records and independent geometry/sample witnesses, remove selector lookup
   and the first-six heuristic. Validate source-only imports without sample origin.
5. **Qualification:** run focused compiler/type/runtime, Rust control/project,
   frontend and actual-WASM tests, then the integrated clean-source gate. Keep all
   M97 visibility, navigation, collision, pin and editing checks. Nominate a new
   frozen, served-byte-verified preview only after qualification.

Acceptance must independently cover:

- Explicit true/false and omitted flags, document true/false defaults, reference and
  construction measurements, inline schema defaults and named-parameter precedence.
- Manifold's six overview dimensions and exact 12/2.4 mm public parameters;
  Gridfinity's 20 overview measurements; source-only export/import parity.
- One shared row across four channels, separate equal-valued controls, full-width
  versus generated-offset values, and no writable authority inferred from metadata.
- Name/help/title edits, parameter extraction, unused editable parameters, multibyte
  spans/comments and native label byte boundaries, explicit
  false/reset, stale/tampered requests, Undo/Redo/reload and retained-invalid source.
- V3 restoration, Undo into a V3 history entry and the first authenticated V3-to-new
  metadata edit, with exact old source text and accepted geometry retained.
- Metadata-only changes preserving accepted mathematical invariants and identity;
  parameter numeric edits preserving existing solve/rejection behavior.
- Personal view changes leaving source/history untouched, and overview eligibility
  remaining subject to the existing layout budget, Hidden mode and collision policy.

Implementation touches the existing owners in
[authoring.ts](../packages/geosolve-sketch-code/src/authoring.ts),
[managed.ts](../packages/geosolve-sketch-code/src/managed.ts),
[managed.rs](../crates/geosolve-sketch-code/src/managed.rs),
[managed_control.rs](../crates/geosolve-sketch-code/src/managed_control.rs),
[code_projects.rs](../crates/geosolve-demo-web/src/workbench/code_projects.rs),
[dimension projection](../crates/geosolve-demo-web/src/workbench/bridge/dimensions.rs),
the Inspector/frontend bridge and the bundled-sample generator. No new solver
primitive, residual, branch policy, dependency or generic metadata framework is needed.
Use the repository defect-hardening skill for any reproduced headless failure or
golden expansion; qualify under [RELEASE_QUALIFICATION.md](RELEASE_QUALIFICATION.md).

## Design verification

Read-only review checked the proposed surface against current authoring types,
binding/control schemas, source mutations, native display-name validation and
artifact readers. Numeric/unit parameters, explicit V3 compatibility, byte-valid
labels and an independently typed unused-parameter edit route are required above.

`git diff --check` and
`./scripts/release-gate.sh --docs-only --since 28d9e83` pass for this documentation
amendment (seven files and 19 added links). This validates the prose/link/input scope;
no production code, sample artifacts, golden bytes or preview files changed, and
no implementation tests or replacement product qualification are claimed.
