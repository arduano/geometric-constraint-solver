<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M74 — Production-style sketch reference UX

Status: **complete under the supervising caller's scoped close decision on 2026-08-16**. The
datum/reference scope, axis-symmetry follow-up, clean release qualification, immutable Tailscale
candidate and exact final GitHub Pages publication pass. M74-U1 through M74-U8 remain deferred,
not passed, and transfer with any findings to the next bug-fixing/UAT follow-up milestone, which
was unstarted at M74 close and is now active as M75.

## Goal

Make the desktop workbench a polished demonstration of how a future CAD host can present
constraint-aware sketch reference geometry. Every sketch owns an immutable Cartesian Origin,
X axis and Y axis; users can see, select and constrain against them without turning those datums
into ordinary document objects. The same milestone tightens grid, camera, coordinate, cursor,
keyboard and viewport-edge behavior without moving solver authority into the browser.

## Accepted work

### M74-R001 — intrinsic Cartesian datums

- Add `SketchDatum::{Origin, XAxis, YAxis}` as intrinsic semantic operands. They always exist and
  have no persistent IDs, variables, document rows, allocator state, history entries or independent
  persistence records. The axes are mathematically infinite through `[0, 0]`.
- Datums are selectable and inspectable, but cannot be dragged, deleted, suppressed, unsuppressed,
  unconstrained, role-converted, locked or unlocked. Any mixed selection containing a datum rejects
  an object-mutation action atomically with the typed `ProtectedDatum` reason.
- Datums do not contribute to geometry counts or Fit bounds. Relations referring to them remain
  ordinary document constraints and may be suppressed, reactivated, deleted and restored through
  normal history.

### M74-R002 — datum-backed relation semantics

- Add retained `CoincidentWithOrigin`, `PointOnDatumAxis` and `CollinearWithDatumAxis` definitions.
  Origin coincidence fixes both point coordinates at zero; point-on-axis fixes the coordinate
  normal to that axis; line collinearity constrains both line endpoints to the intrinsic axis.
- Accept point/Origin, point/axis and affine-line/axis operands in either selection order. Parallel
  or Perpendicular against an intrinsic axis lowers to the existing ordinary Horizontal or
  Vertical line relation rather than creating another datum relation family.
- Give every new runtime row a structured audit description, finite independent residual
  validation and central finite-difference Jacobian coverage. Invalid references, non-finite
  geometry, dependency deletion and suppression/reactivation retain the existing fail-closed
  transaction rules.
- Keep canonical sketch v1-v4 byte-for-byte frozen. Datum relations live only in the existing
  unsupported draft-v5 side records; attempts to encode them as canonical v4 return the exact
  typed `UnsupportedM74State` error. M74 does not activate or support sketch v5.

### M74-R003 — datum picking and inference

- Publish finite viewport DTOs for painting and picking while retaining intrinsic infinite-axis
  semantics. Native points and curves win overlaps before datums; Origin wins an Origin/axis tie.
- Use screen-space, zoom-independent hysteresis: Origin enters at **6 px** Euclidean distance and
  remains latched through **9 px**; an axis enters at **4 px** perpendicular distance and remains
  latched through **7 px**.
- Permit datum inference only for point-bearing construction stages. Circle circumference/radius
  placement is excluded. Shift suppression, hidden reference geometry, cancellation, stage change,
  camera/policy change and document mutation clear or exclude datum inference exactly as they do
  other authenticated candidates.
- A live Horizontal span owns its endpoint's Y coordinate and therefore suppresses same-coordinate
  X-axis inference. A live Vertical span owns X and suppresses same-coordinate Y-axis inference.
  The orthogonal combinations remain valid: Horizontal may compose with Y-axis inference and
  Vertical may compose with X-axis inference. Native geometry still outranks every datum candidate,
  and Origin emits one Origin relation rather than two competing axis relations.
- Inferred datum guides, adjusted coordinates, retained relations and the atomic commit plan must
  describe one authenticated terminal candidate. Undo/Redo restores the whole accepted edit in one
  history step.

### M74-F001 — point-pair symmetry across datum axes

- Extend ordinary Symmetric authoring to accept two distinct stored points plus intrinsic X or Y
  axis. Active authoring remains point → point → line/reference axis; a complete preselection works
  in every operand order. Origin and repeated-point operands reject without mutation.
- Persist `SymmetricAboutDatumAxis { first, second, axis }` as one ordinary relation without a
  hidden line, datum variable or datum ID. X-axis symmetry enforces
  `(first.y + second.y)/2 = 0` and `second.x - first.x = 0`; Y-axis symmetry swaps X/Y.
- Normalize both rows by model scale, provide an analytic Jacobian and structured audit bindings,
  and independently validate finite accepted geometry. Two otherwise-free points have rank two
  and two remaining degrees of freedom.
- Reuse the Symmetry glyph and paired-point anchor, include the datum in related operands, and keep
  suppression, delete, dependency, Undo/Redo and draft-v5 reload in the ordinary relation
  lifecycle. Canonical v1-v4 remains frozen and returns `UnsupportedM74State` on export.
- Preserve drawn-line symmetry unchanged. This finding does not add Origin/point symmetry,
  geometry duplication, a Mirror tool, angle-to-axis dimensions, rectangle snapping or new
  tangent/distance datum relations.

### M74-W001 — production-style desktop presentation

- Render a dedicated **Reference geometry** tree group, infinite-looking colour-distinct axes,
  Origin glyph, axis labels and consistent normal/hover/selected/related states. A datum inspector
  identifies each item as an intrinsic protected reference and exposes no mutating controls.
- Provide independent **References** and **Grid** display toggles. Hiding References removes canvas
  datum painting, picking and inference; the always-present tree may still explicitly select a
  hidden protected datum for inspection. The Grid is presentation-only and never becomes a snap
  target, selection item, constraint operand or document state.
- Replace the fixed CSS background with an Origin-aligned adaptive SVG grid whose major spacing
  follows a stable `1–2–5 × 10^n` sequence across zoom. Datums paint behind native geometry and
  neither grid nor datums enlarge Fit bounds.
- Add an Origin camera control that recentres without changing zoom. Fit continues to use native
  accepted geometry; Fit on an empty sketch resets to the canonical camera rather than fitting the
  infinite axes.
- Show a compact coordinate HUD. It displays raw model coordinates normally and the exact adjusted
  inference coordinate while snapping, with the raw coordinate available as explanatory text.
- Use contextual canvas cursors for Select, drawing, relation authoring, Fillet interaction and
  active pan.
- Add `Ctrl/Cmd+Z` Undo, `Ctrl/Cmd+Shift+Z` Redo and `Ctrl+Y` Redo. Editable controls,
  content-editable regions and dialogs own their keystrokes; ambiguous Ctrl+Command and Alt-modified
  chords do not mutate history.
- Ignore new pointer, double-click and wheel interaction in SVG letterbox bands. Valid interaction
  inside the mapped sketch plane and existing captured-gesture terminal behavior remain unchanged.

### M74-Q001 — qualification and publication

- Add focused domain, editor, inference and web-presentation regressions, including operand
  reversal, datum protection, lifecycle, picking priority, exact tolerances, Shift/visibility
  suppression, circle exclusion, same-axis precedence, orthogonal bundles and atomic history.
- Cover datum-axis symmetry on both axes, three model scales, swapped point order, every complete
  preselection permutation, active collection/rejection, rank/DOF, central finite differences,
  audit, lifecycle, scene ownership and native/WASM checkpoint reload.
- Review any required authoring/scene golden expansion row-by-row. Do not regenerate the golden
  blindly; retain the existing fixture unchanged if datum authoring is fully owned by focused tests.
- Pass format, warnings-denied workspace Clippy, locked all-feature tests, relevant native/WASM
  checks, Trunk release assembly, golden survey/check/clean and the complete clean release gate.
- Freeze and byte-verify an immutable Tailscale candidate for the focused scorecard in
  `docs/M74_UAT.md`. Keep it running for follow-up fixes until explicit approval, then deploy and
  exact-verify the accepted source through GitHub Pages.

Mechanical nomination record (2026-08-16): product source
`55693372bea4759c9a67eee14f1af3d6a9e0690c`, tree
`866fbf8b58ec19e72cbe6936e06f3615dba2f692`, passes the complete clean release gate. Its exact
read-only snapshot `/tmp/geosolve-m74-uat.jFfAm4`, aggregate
`1e5d00474c383102f4f6189a534e5acb395d92e94a7c0853b72d9c25b0f4fe13`, is byte-verified at
`http://100.94.63.83:8080/` and was current focused-UAT authority at nomination. Human approval and
public Pages publication remained pending at that checkpoint.

Scoped closure record (2026-08-16): the supervising caller accepted the existing automated,
independent-review, clean-gate and frozen-artifact evidence and explicitly waived a separate M74
hands-on UAT pass. The U1-U8 scorecard remains available but is deferred, not marked passed. Its
future execution and any findings belong to the next bug-fixing/UAT follow-up milestone. This
record reserves that handoff without activating or planning the next milestone.

Final public publication record (2026-08-16): accepted product source
`55693372bea4759c9a67eee14f1af3d6a9e0690c`, tree
`866fbf8b58ec19e72cbe6936e06f3615dba2f692`, is deployed from documentation-only approval
descendant `b6b1d62b49466ea06522dbdd3f5444a324d36584`, tree
`cba65ae9349a4d1f6e79cebc2f1994aab8be19c3`, by successful Pages run `31923806117`, artifact
`9257602997` and deployment `5927348343`. The downloaded ZIP and inner tar SHA-256 values are
`60cf4c4985e08517c6a9a949bdacb4faf31f7069079a65e9b5e8c8f7ef21f955` and
`14ef2ae52b641620f958fb9df66bb40570f0b26911da695e632ac747bb7a9985`; the seven-file manifest
aggregate is `df421cc0050c31008e5cb5620092c4d05e91191fd1eccaaf020ca437ce97e725`. The public root
and every artifact path return HTTP 200 and match the artifact byte-for-byte, application URLs are
repository-prefixed, media types are correct, and the public M72/M74 two-size Chromium checks pass.

## Acceptance

- Intrinsic datums remain identity-free, immutable and outside persistence/history/count/Fit
  semantics while their ordinary retained relations solve, audit and follow normal lifecycle.
- Picking, contextual authoring and inference obey the exact priority, tolerance, suppression and
  composition rules above through public headless APIs; the browser only renders and translates
  input.
- The grid, camera, HUD, cursors, keyboard shortcuts and letterbox behavior pass focused
  presentation tests without adding grid snapping or browser-owned geometry. Hands-on feel review
  is retained as deferred follow-up work rather than claimed M74 evidence.
- The clean release gate and immutable Tailscale candidate pass. The explicit scoped closure
  approval accepts that evidence without separate hands-on UAT; exact accepted-source Pages
  publication passes. Every M74 goal is complete under the scoped decision.

## Non-goals

M74 does not release sketch v5, persist the intrinsic datums, add user-created datum systems,
construction planes, general geometry mirroring/a Mirror tool, grid snapping, unit/expression
entry, mobile/tablet layout, a new solver priority system or a production renderer. It does not
alter canonical v1-v4 bytes or make the demonstration workbench a separate product API.
