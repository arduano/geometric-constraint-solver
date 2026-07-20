# ADR 0021: Visual-only line-profile analysis

Status: accepted

## Context

M26 needs reusable bounded-face detection for accepted line arrangements, mainly
to support diagnostic visualization before fillet and trim work. Publishing a
region entity now would imply persistent CAD topology, selection semantics and
solid/B-rep guarantees that the project does not yet own. Proximity snapping
would also turn display analysis into an undocumented topology edit.

The input may contain shared point identities, explicit coincidence constraints,
proper crossings, T-junctions, open chains, nested loops, collinear overlap or
near-degenerate intersections. Analysis must be deterministic, bounded and
truthful when a component cannot be classified reliably.

## Decision

### Read-only API and topology

`SketchDocument::analyze_visual_profiles` consumes only public persistent points,
line/polyline definitions and unsuppressed `Coincident` constraints. Shared point
identity and the transitive closure of those explicit coincidences weld vertices.
Distinct endpoint identities are never welded merely because their coordinates
are close or equal.

Because a schema-valid document can contain an unsolved constraint state, every
profile-relevant coincidence class is checked against the default normalized
hard-residual tolerance before welding. A disagreement returns a typed
`InconsistentCoincidence` issue and no faces; analysis never teleports an
endpoint and labels the result complete.

Every bounded source span is intersected pairwise within an explicit candidate
budget. Proper interior crossings split both spans ephemerally. An exact endpoint
on another span's interior creates a T-junction and splits only the interior span.
These vertices and fragments have no persistent IDs and never enter the document,
history, lowering or solver.

Collinear positive-length overlap skips its entire affected arrangement
component. Near-collinear intersection classification, an intersection within the
floating endpoint uncertainty band, or coincident incompatible split parameters
is a typed numerical ambiguity and likewise skips that component. Unaffected
components may still publish faces, but the overall status is `Truncated`, never
`Complete`.

### Face extraction and publication

Bridge fragments are removed by an iterative deterministic bridge walk. Remaining
directed half-edges are sorted by geometric angle and walked with the face on the
left. Positive-area cycles are bounded contours; the negative unbounded walk is
not published. Disconnected nested cycles are assigned by strict point-in-polygon
containment so an outer contour publishes direct child contours as clockwise
holes while each child also owns its inner bounded face.

`VisualProfileFace` publishes ordered contours, finite visual area and one
`VisualProfileEdge` per fragment with source `CurveSpan` and source-parameter
interval provenance. The result carries `Complete`, `Truncated` or `Skipped`,
typed issues, candidate count and fragment count. Candidate, fragment, cross-
component containment-candidate and face limits are deterministic caller-visible
options. Candidate-pair arithmetic divides before multiplying and fails closed
on count overflow. Component/cycle bounds reject impossible nesting before
contour tests, and every cross-component bound/cycle comparison consumes budget.
Exceeding candidate, fragment or containment limits publishes no partial faces;
exceeding the face limit publishes a deterministic prefix with `Truncated` status.

### Browser visualization

The disposable playground converts published contours directly to SVG paths with
even-odd fill. Overlay paths have no object/data IDs and CSS fixes
`pointer-events: none`. They are recomputed from the accepted document and never
participate in selection, autosave, canonical JSON or command history.

## Consequences

- The API is suitable for diagnostics and visual profile highlighting only; it
  does not create usable CAD regions, trim loops or manufacturing topology.
- Explicit point/coincidence topology is authoritative. Coordinate-equal but
  unrelated endpoints remain unrelated.
- Proper crossings and T-junctions are visible without destructively splitting
  source curves.
- Overlap and numerical ambiguity remain typed and component-local rather than
  guessed through hidden tolerances.
- Pairwise candidate generation is intentionally simple and bounded. A future
  sweep-line accelerator may preserve the same public result contract.
