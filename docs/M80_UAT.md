<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 focused UAT — native topology-preserving Profile Offset

Status: **prepared but not started**. Implementation and broad pre-nomination mechanics pass, but
no clean source, immutable distribution or human acceptance is recorded. Run this scorecard only
against the exact clean-qualified candidate entered below.

Product source: pending

Candidate tree: pending

Tailscale endpoint: `http://100.94.63.83:8080/` after nomination

Immutable snapshot and ordered-manifest aggregate: pending

Keep the exact no-rebuild snapshot running locally over Tailscale until the supervising human
accepts it or an M80 finding explicitly withdraws it. A replacement candidate gets new source,
tree, snapshot and evidence; withdrawn bytes remain historical and are never silently overwritten.

Use the ordinary editable workbench at approximately `1440x900` and `1024x720`, with coarse and
fine zoom. Direct tests, not visual judgment, own exact residual thresholds, branch bytes,
persistence bytes, rank/DOF and topology proof.

## U1 — face selection, direction and shared distance

Create a plain rectangle and a non-axis-aligned polygon. Activate Modify → Offset, hover/click each
face and apply both Outward and Inward examples. The whole eligible boundary should highlight as
one operand; one preview and one annotation should describe the whole result. Outward must expand
the region and Inward shrink it, with one editable positive shared distance and one Undo step. Edit
a target-side corner of one closed face: the source must follow while keeping the same offset and
topology, rather than behaving as one-way generated geometry. If the source uses a polyline span,
inspect the created target: that span must become one standalone native line, not a rebuilt
polyline container or sampled approximation.

Flip before Apply and enter a negative distance once, including once before selecting any operand.
Both actions should reverse direction without leaving a negative persisted magnitude. Blur, click
the canvas, pan and zoom while authoring: the bottom-left panel should remain open. Cancel or its
close control should make no geometry/history change and return focus to Select.

## U2 — circles, arcs, mixed loops and holes

Offset a circular face in both directions and edit its source radius afterward. The target should
remain concentric at the requested signed radius difference. Try shrinking through zero: the last
accepted complete scene must remain visible with a local refusal.

Build a bounded Profile face containing lines and circular arcs, including at least one smooth
tangent join and one miter. Offset it, then move eligible source geometry without crossing a branch
barrier. The target should remain same-family, connected and smooth/mitered as authored, with no
faceting or browser-side jump.

Create a face with two holes, including one circular hole. Outward must expand the outer loop while
shrinking both holes; Inward must shrink the outer loop while expanding them. Increase the value
until outer/hole or hole/hole contours would touch, or a hole would disappear. Apply must become
unavailable or reject locally without publishing a trimmed, missing or self-intersecting partial
result.

## U3 — manually ordered open chains

Offset a single line and confirm the result is an exact translated segment with matching endpoints.
Offset a single circular arc and confirm center, signed radius, Start and End all follow; no
antipodal endpoint jump is allowed.

Collect a multi-edge line/arc chain in deliberate order. Traversal arrows plus visible Start/End
markers should communicate the whole ordered chain and its terminals, including when the first
picked edge is traversed in reverse. Compare Left and Right with Flip, Apply, and then edit both a
source control and an eligible target control. Connectivity and the shared offset should survive,
and source order must not reverse after Undo/Redo or reload. Disconnected, branching or closed-
circle chain picks should retain a clear unavailable hover/reason rather than being guessed.

## U4 — ordinary target geometry and association lifecycle

Inspect the created objects: target curves are ordinary native Profile geometry, target joins use
ordinary persistent connectivity and exactly one driving Profile Offset dimension owns the shared
distance. There must not be one hidden dimension per edge or any computed-feature output.

Move the grouped annotation perpendicular to its witness, switch tools and return; its presentation
offset should remain. Delete/suppress only the Profile Offset association. Target curves and their
ordinary connectivity must remain and become freely editable; Undo/Redo must restore the exact
association and identities. Save/reload must preserve a compatible annotation placement as well as
the operand, direction, traversal and branches. Copy/load a reproduction payload separately: it
must preserve the authoritative operand and branch state but intentionally omit and ignore the
disposable annotation cache, so placement is recomputed safely.

## U5 — unsupported and topology-changing cases fail atomically

Try selecting Construction geometry, external geometry, an ellipse/elliptical arc, conic, Bezier,
B-spline, NURBS, a computed Fillet boundary and a partial arrangement fragment. Each must remain
visibly unavailable on hover and reject with concise local feedback; no approximated target, stale
global Problem, identity consumption or history step may appear.

For otherwise supported profiles, try values that cause edge/loop collapse, self-intersection,
non-adjacent contact, split/merge, hole loss and a miter-to-tangent crossing. The preview may retain
its last valid position, but Apply must never publish changed topology. Reducing the distance back
into the valid cell should recover immediately without refresh or tool reset. Unrelated sketch
arrangement geometry is outside this certificate: only the selected source and target operand paths
and their contours determine whether the native association preserves topology.

## U6 — hover/click, stale preview and repeated authoring polish

At overlapping or crowded geometry, verify the face/edge highlighted on hover is exactly the
operand collected on click. Select an eligible curve once from the tree and once through keyboard
activation; both must use the same ordered-chain semantics and must not also contribute a duplicate
canvas click. Preview geometry must not intercept selection or masquerade as accepted geometry.
Change accepted geometry with Undo/Redo or another edit while an Offset preview is live; the stale
preview must revoke and cannot apply to the new scene.

Separately, complete an ordinary accepted point move first and then activate Offset on the edited
face without refreshing. Hover/selection, preview and Apply should work immediately against that
current scene; consumed drag guidance must not make the new Offset session appear stale.

After a successful Apply, Offset should remain active and remember the last valid distance only.
The previous face/chain must not remain secretly selected. Repeat all core checks at both viewport
sizes and zoom levels, including keyboard focus, concise disabled reasons and absence of clipped or
blur-dismissed controls.

## Acceptance record

Pending explicit supervising-human decision. Mechanical qualification or a live Tailscale server
does not itself pass any UAT row and does not close M80. After acceptance, record the exact source,
tree, immutable manifest and decision here; then perform the normal GitHub Pages publication and
hosted-byte verification before marking the milestone complete.
