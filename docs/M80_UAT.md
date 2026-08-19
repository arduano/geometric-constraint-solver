<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 focused UAT — native topology-preserving Profile Offset

Status: **amended implementation and development qualification complete; clean replacement
nomination and focused human UAT are pending**. The pre-amendment candidate is withdrawn from
acceptance. Its snapshot remains immutable historical evidence but is no longer served. Mechanical
evidence does not pass any UAT row, and the pre-amendment bytes must not be used to score final M80
acceptance.

Superseded pre-amendment product source: `b83dad2b18cdfbb241fc012337ac5dbfa7234a9a`

Superseded candidate tree: `440d66ef07b7df963164e69ebed4b75509f56bd6`

Historical Tailscale endpoint: `http://100.94.63.83:8080/` (recorded PID `3837538` has exited; no
`geosolve-m80-uat.service` currently exists)

Immutable snapshot: `/tmp/geosolve-m80-uat.hggNdd` (directory `0555`; seven regular non-symlink
files `0444`)

Ordered-manifest aggregate:
`d8d740fb852e793925ce4e54e8777a225b68ea5cfa39b2f36060bd3566938e37`

Amended replacement source/tree/snapshot/manifest: **pending**.

Superseded pre-amendment clean gate: 2026-08-19 13:38:33–13:50:00 AEST;
`/tmp/geosolve-m80-clean-gate.b83dad2.nix.log` (262,051 bytes, 3,416 lines, SHA-256
`3e44403e3f2038467aa0c06193030feb6c099cd53634b315a8308ca111113fa0`)

Superseded pre-amendment freeze/HTTP evidence: `/tmp/geosolve-m80-freeze-evidence.1S2bfA`,
`/tmp/geosolve-m80-temp-verify.mGxuDM` and `/tmp/geosolve-m80-final-verify.M0ThFH`. The temporary and
final eight-request result ledgers have respective SHA-256
`1628a6c2e87ab519351598371712ace67584616d1573f510c80fbe48f3cd9bea` and
`03e93b8fcd4d53231d7e3bafd95c6b4315ba2a12515bb8a4f763a91abc8c0b28`; root plus every file
returned HTTP 200 with exact media/length/bytes, no redirects or content encoding and the exact
frozen manifest aggregate. The withdrawn listener remained live until temporary verification
passed, and the temporary replacement remained live until final `:8080` verification passed.

Withdrawn predecessor: source `949c3db`, tree `23a6f8d`, snapshot
`/tmp/geosolve-m80-uat.Nnxsu7`, aggregate
`18677a4488848e56d463a90ffe2e2653e34fe6931767d25b63d3dc47b69084d9`. It is historical, no
longer served and must not be used for scoring.

Leave the shared Tailscale endpoint unserved until the amended replacement passes its clean gate,
freeze and temporary-port byte verification. Start `:8080` only after that proof. The replacement
gets new source, tree, snapshot and evidence; withdrawn bytes remain historical and are never
silently overwritten. Run every UAT row below only against that future amended candidate.

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

At a T-junction, select exactly two incident edges that make one continuous path. The selected pair
must preview/apply while the third unselected arm stays outside the highlight, target and
association. Reset and try to include all three arms: that selected set must report branching and
retain the prior complete operand. Repeat once with an isolated edge and a selected closed loop to
confirm their distinct typed refusals remain intact. Include a bounded arc whose own Start and End
are joined by ordinary G0 continuity: it is already closed and must not masquerade as a one-edge
open chain.

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

While changing Distance quickly, a superseded provisional frame must not retain a grab highlight
that the same press refuses. Hover and press should agree on the current target immediately after
each rerender, without requiring the pointer to leave and re-enter.

Separately, complete an ordinary accepted point move first and then activate Offset on the edited
face without refreshing. Hover/selection, preview and Apply should work immediately against that
current scene; consumed drag guidance must not make the new Offset session appear stale.

After a successful Apply, Offset should remain active and remember the last valid distance only.
The previous face/chain must not remain secretly selected. Repeat all core checks at both viewport
sizes and zoom levels, including keyboard focus, concise disabled reasons and absence of clipped or
blur-dismissed controls.

Before Apply, hover a provisional target edge and its grouped distance presentation. Both should
communicate the same distance-drag owner without becoming ordinarily selectable. Press without
moving, move less than 3 px, then drag normally: only the normal drag should change Distance and the
whole provisional result. Try a topology-invalid distance, return to a valid position, and release;
the last complete valid preview must stay visible and recover without restarting. Repeat and cancel
through Escape, capture loss and zoom/tool change: the pointer-down distance/preview must return and
history must remain unchanged. A normal release retains the final candidate but adds no history;
Apply must remain unavailable while the drag is captured, then the later Apply adds exactly one
step and must publish the geometry that was visible at release.
For a circular face, drag the grouped annotation along its displayed source-to-target radial line;
the distance must increase toward the target side rather than responding from the opposite side.
Immediately begin a second distance drag after releasing the first, then repeat and cancel the
second drag. A delayed update, release or cancel from the first gesture must never stop, overwrite
or restore the second gesture; its own cancel must return exactly to the second pointer-down
candidate without refresh.

## U7 — explicit native line-line Fillet and unchanged Offset consumption

Create exactly two standalone untrimmed native Profile Lines sharing one endpoint and author one
ordinary line-line Fillet preview. Confirm the primary action is labelled **Apply computed** and the
separate action is labelled **Apply native profile**. Cancel, re-enter Fillet, recreate the same
preview and choose Apply native profile. It should visibly complete as one atomic edit: both
original lines are shortened, one ordinary circular arc appears, the two tangent joins are shown
consistently, one editable driving Radius owns the arc, the Fillet panel closes and focus returns to
Select.

Repeat the same manual two-line corner collection with the line parents picked first in forward
order and then in reverse order. Before Apply, each preview must keep the canonical parent order
while its arc contacts and tangent directions remain attached to their matching parents. Apply
native profile from both orders, then edit Radius once: both resulting ordinary corners must retain
the same visible tangent branches without a swapped contact, branch jump or blank scene.

On another eligible preview, begin dragging the provisional Radius, move far enough to show a
different valid radius, then press Escape to restore the exact pointer-down preview. Without
leaving Fillet or recreating the corner, **Apply native profile** must remain enabled and publish
that visibly restored radius. It must not report a stale-preview error or reuse the discarded
sample's generated identity.

While an eligible native action is visible, attempt to add a second corner whose trims cross the
first and are rejected. The last valid preview should remain visible, and its native action must
still apply directly rather than becoming stale because the rejected replacement consumed hidden
revision state.

Edit the Radius and an eligible outer line endpoint; the visible accepted result should remain
finite and tangent without a branch jump or blank scene. One Undo must restore the original shared
line-line corner and remove the arc/tangency/Radius output together; one Redo must restore the same
rounded result in one step.

Activate Modify → Offset and collect the resulting native line-arc-line path as one ordered open
chain, including once in reverse traversal. Preview, distance drag and Apply should behave exactly
like any other mixed native chain, with no Fillet-specific label, dependency or refusal. Separately
create a normal computed Fillet and confirm its generated arc/discarded fragments remain visibly
unavailable to Offset.

Try a line-circle corner, more than one pending Fillet corner and a shared endpoint with a third
incident Profile edge. Apply native profile should be disabled with the concise reason `shared
corner must be owned only by the two selected source lines` for the third-owner case, without a
persistent ID or document-error prefix. Attempting an unavailable path must leave the visible
geometry and Undo stack unchanged and must not leave a stale global Problem. Cancel and re-enter
Fillet to confirm an eligible corner recovers without refresh.

## Acceptance record

Pending clean committed-source release qualification, frozen replacement nomination and explicit
supervising-human decision.
Mechanical qualification or a live Tailscale server does not itself pass any UAT row and does not
close M80. The superseded `b83dad2` snapshot cannot receive acceptance. After acceptance, record the
exact replacement source, tree, immutable manifest and decision here; then perform the normal
GitHub Pages publication and hosted-byte verification before marking the milestone complete.
