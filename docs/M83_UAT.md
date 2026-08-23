<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Authoritative sketch lineage

Status: **Lineage-panel amendment implemented; replacement immutable candidate pending**. The
previous clean-gate source, no-rebuild snapshot and byte-verified Tailscale endpoint below are
superseded because they predate the requested panel. M81 remains accepted public product authority
meanwhile.

M83 deliberately preserves existing sketch mathematics and visible authoring behavior while
replacing persistent workbench authority. The amendment now exposes a read-only projection of that
real retained program, so human review should cover both end-to-end editing/history/reload behavior
and whether the panel makes program/history changes legible without implying row editability.

## Superseded pre-panel candidate

- Product source: `d378f7b31f56b43af787202dc1ebb92b7d199f84`.
- Product tree: `25a47e821cc80ff62d1891cfc7095d10fb2ec87f`.
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 on
  2026-08-23 at 02:06:03 AEST; log `/tmp/geosolve-m83-release-gate.log`, SHA-256
  `b38c7c5a46408e5237109b3ac64d6c75f340479920baefaa933189c90ae7692e`.
- Immutable no-rebuild snapshot: `/tmp/geosolve-m83-uat.RwXTfs` (directory `0555`; seven regular
  non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `ee2695ca55e803cbdeb8f6cd5a1ff632e59fe583428807f36e29ad5f2fbebd51`.
- Tailscale endpoint: `http://100.94.63.83:8080/`, served only from that snapshot by
  `geosolve-m83-uat.service` (nomination PID `1273798`).
- Exact served-byte verification: temporary
  `/tmp/geosolve-m83-temp-verify.GLWzwt/results.tsv` and retained
  `/tmp/geosolve-m83-final-verify.yMsi3f/results.tsv`, each SHA-256
  `3217083aa1a3f9e56d02dcdb30f8c518b35d27767676abc531aa2356f1632ba1`.

Both historical verification passes cover `/` plus all seven files with HTTP 200, direct Tailscale address,
zero redirects, no `Location` or `Content-Encoding`, exact media type/length/SHA/body, and root
equality with `index.html`. The temporary `:18080` listener is retired; the retained endpoint stays
live only for continuity until replacement. These bytes are withdrawn from current UAT because
they do not contain the Lineage panel. GitHub Pages is intentionally unchanged until approval.

Replacement product source, frozen snapshot and exact served-byte evidence will be recorded here
after the amended committed source passes the complete clean release gate.

## Focused scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create and edit representative Point, Line/Polyline, Rectangle, Circle/Arc, Ellipse, Bezier/conic and open/periodic NURBS variants, including Shift variants and snapped point reuse. | Recipe behavior, intrinsic constraints, branches and stable selection remain indistinguishable from the accepted workbench; no blank/partial scene appears. | pending |
| M83-U2 | Add representative point, direction, symmetry, contact/tangency/continuity constraints and Driving/Reference distance, radius and angle dimensions; edit, suppress, delete, Undo and Redo them. | One ordinary history controls the complete scene; annotations, target values, source identity and explicit branches survive history. | pending |
| M83-U3 | Move simple and advanced curve handles, including a drag that changes several owning points/parameters; edit radius/trim/rational/NURBS properties and Profile/Construction role. | The accepted projection persists after release and reload; all affected owners change atomically, with no partial motion or hidden generic Move action. | pending |
| M83-U4 | Create, edit and remove a computed multi-corner Fillet; exercise radius manipulation, an explicit branch action and an invalid/recovery path. | Stable feature/corner intent survives while generated edges remain current-only; failure retains the last complete scene and later valid input recovers. | pending |
| M83-U5 | Apply two native-profile Fillets (including consecutive corners if convenient), then create a face or open-chain Profile Offset and drag/edit its distance; Undo/Redo and delete the operations. | Native topology and Offset association rebuild exactly, both Fillets remain independently editable, operation-owned objects are removed/restored as one history action, and unrelated identities remain stable. | pending |
| M83-U6 | Deliberately create a structurally valid but unsolved/conflicting edit, then Undo, Redo, repair it and continue authoring. | The retained failure is visible while the prior complete accepted scene remains authoritative; history never becomes stuck or publishes a partial scene. | pending |
| M83-U7 | Save/reload the workspace with moved annotations, branches, features and history; repeat once after a failed retained edit and continue Undo/Redo. | Workspace v7 restores retained-versus-accepted authority, exact history and annotation placement, then continues editing without ID reuse or stale Problems. | pending |
| M83-U8 | Import one older reproduction/workspace payload from the existing v1-v6 corpus; edit and delete one imported entity, Undo/Redo that deletion, save as v7 and reload. | The imported scene remains one truthful baseline with no invented history; deletion behaves as a later retirement action, Undo restores the exact identity, Redo retires it again, and neither history nor reload reuses or silently removes that baseline identity. | pending |
| M83-U9 | Watch **Lineage** while creating geometry and a constraint, directly moving an owner, suppressing/deleting an action, and using Undo/Redo. Repeat once with an imported sample and at both a wide and compact desktop width. | New authored intent adds ordered rows; direct movement rewrites the owning row without adding a Move event; revision advances; suppressed/deleted rows remain explicit and readable; stable step/developer IDs survive Undo/Redo; the separate history position and Undo/Redo availability track the command bar. Rows are visibly read-only. At wide size Lineage sits beside Sketch Tree; at compact size it stacks below without squeezing or covering the canvas. | pending |

## Acceptance rule

Any lost object/feature, branch flip, changed authoring behavior, partial multi-owner drag, stale or
nested history, accepted-scene substitution, failed recovery, reload mismatch or blank scene opens
an exact owner regression under the GeoSolve defect-hardening workflow and withdraws the
candidate. Cosmetic requests unrelated to M83 authority are deferred rather than folded into the
milestone.

M83 closes only after the replacement exact candidate passes this scorecard or receives an explicit
scoped disposition, the supervising caller approves it, and the accepted bytes are subsequently
published and exactly verified on GitHub Pages.
