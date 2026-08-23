<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Authoritative sketch lineage

Status: **replacement editable-lineage/predictive-drag candidate mechanically qualified, frozen
and byte-verified; M83-U1–U12 human UAT pending**. M81 remains accepted public product authority;
M83 is not accepted or published.

M83 deliberately preserves existing sketch mathematics and visible authoring behavior while
replacing persistent workbench authority. The amendment exposes a selectable projection and routes
program edits back through that real retained authority. Human review should cover end-to-end
editing/history/reload behavior, whether legal and blocked reordering feels intelligible, and
whether predictive dragging improves expensive scenes without making intent look accepted.

## Replacement amendment candidate authority

This is the sole current M83-U1–U12 candidate:

- Product source: `d8137543fecf4a09433471e16383db5069de0d41`.
- Product tree: `031a8c95ddc99f1bf52847c0485ac857b0af1079`.
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0
  from committed clean source on 2026-08-23, 09:52:01Z–10:17:36Z. The 308,679-byte log
  `/tmp/geosolve-m83-amendment-release-gate.log` has SHA-256
  `015af2b3fea2c64677920b3c590df8c5812b7662900a11e4ba156633845cbd17`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-amendment-uat.AVC9ce` (directory `0555`; exactly
  seven regular non-symlink files `0444`).
- C-locale ordered-manifest aggregate:
  `260445142878a35c4e5cfed3934438c58c1e47b09a462cec4030e34f68da11e2`.
- Temporary verification: `geosolve-m83-amendment-temp-uat.service`, PID `1728899`, served only
  that snapshot at `100.94.63.83:18080`; it was retired after the complete pass.
- Current UAT endpoint: `http://100.94.63.83:8080/`, served only from that snapshot by
  `geosolve-m83-lineage-uat.service`, nomination PID `1733554`.
- Exact served-byte evidence:
  `/tmp/geosolve-m83-amendment-temp-verify.8Z630V/results.tsv` and
  `/tmp/geosolve-m83-amendment-final-verify.BK5m0F/results.tsv`; the ledgers are byte-identical,
  each with SHA-256
  `7f7f1f3fa813dca1ee99e5a9fe4f6d622babbf65bfb342a6a1a37470874b34ee`.

Both verification passes cover `/` plus all seven files; each returned HTTP 200 from the direct
Tailscale address with zero redirects, no `Location` or `Content-Encoding`, exact media type,
`Content-Length`, downloaded length, SHA-256 and body bytes; `/` exactly equals `index.html`.
Focused architecture/API/interaction and predictive-terminal reviews found no release blocker.
One future hardening note is
recorded without weakening this candidate: Point terminal publication is already protected by its
exact retained preview, identity, coordinate, provenance, cold-reproduction and atomic-swap checks,
while a private Point scene seal would add fail-earlier symmetry with CurveControl.

GitHub Pages remains deliberately unchanged at remote source
`f55f226e28e0bbbb9d1b1c509cb68322be0a2240` until explicit M83 approval.

## Withdrawn read-only candidate authority

- Product source: `bb888cc68c00ad3a3823a9f2215528dfb357f9f9` (withdrawn from current UAT).
- Product tree: `dff5ebebbfe024c00f88ba231362a3ea29d6e0bc`.
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 on
  2026-08-23 from an isolated clean worktree pinned to the exact product commit; 332,496-byte log
  `/tmp/geosolve-m83-lineage-stable-release-gate.log`, SHA-256
  `d1529363d8fc382c0767adbcfca24e4827f5a280d17090fd83c31b6910e791c1`.
- Immutable no-rebuild snapshot: `/tmp/geosolve-m83-lineage-uat.1KL8gG` (directory `0555`; seven
  regular non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `d5d51fcb07352f96e39518941d59e41491a25106563c34700fdff6536060bd27`.
- Historical Tailscale endpoint: `http://100.94.63.83:8080/`, formerly served only from that
  snapshot by `geosolve-m83-lineage-uat.service` (nomination PID `4152505`, retired).
- Exact served-byte verification: temporary
  `/tmp/geosolve-m83-lineage-temp-verify.jbCyQE/results.tsv` and retained
  `/tmp/geosolve-m83-lineage-final-verify.2LxtXE/results.tsv`, each SHA-256
  `5cbce667ead67c909d54a97f4db5b39866bb560c2c98876f3a34390a737be186`.

Both historical verification passes cover `/` plus all seven files with HTTP 200, direct
Tailscale address, zero redirects, no `Location` or `Content-Encoding`, exact media
type/length/SHA/body and root equality with `index.html`. The historical temporary listener and PID
`4152505` are retired; the snapshot remains recoverable but is no longer served. These bytes do not
contain editable Lineage or predictive drag and no longer satisfy this UAT. GitHub Pages is
intentionally unchanged until approval.

Independent browser review reports zero root/Inspector overflow at the 1600, 1100, 952, 940, 929
and 928px boundary cases, zero bounded-history overflow for `1025 / 2049`, and 6.24:1 Lineage-count
contrast. This is qualification evidence, not a substitute for the scorecard below.

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
- Historical Tailscale endpoint: `http://100.94.63.83:8080/`, formerly served only from that
  snapshot by `geosolve-m83-uat.service` (nomination PID `1273798`, retired).
- Exact served-byte verification: temporary
  `/tmp/geosolve-m83-temp-verify.GLWzwt/results.tsv` and retained
  `/tmp/geosolve-m83-final-verify.yMsi3f/results.tsv`, each SHA-256
  `3217083aa1a3f9e56d02dcdb30f8c518b35d27767676abc531aa2356f1632ba1`.

Both historical verification passes covered `/` plus all seven files exactly. Those bytes are
withdrawn from current UAT because they do not contain the Lineage panel, and their service is
retired.

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
| M83-U9 | Watch **Lineage** while creating independent and dependent geometry/constraints. Select rows from pointer and keyboard, inspect graph/ownership/identity details, then select canvas and tree geometry. Repeat at wide and compact desktop widths. | Selection always opens a fresh authority-derived Inspector; geometry and Lineage selections clear each other; stable developer/output/reservation identity is legible; listbox focus and wide/compact layout remain bounded. No selection state survives reload. | pending |
| M83-U10 | Reorder independent and dependent rows with buttons, position menu, `Alt+Up/Down` and desktop drag/drop; then directly move an output, suppress/delete, Undo/Redo and reload. Try an independent constraint/dimension pair plus a suppressed row, imported root and deleted row. | Independent actions—including the constraint/dimension pair—move; dependency-blocked moves stop at an explained legal boundary; imported/deleted rows are pinned; suppressed rows remain movable. Stable step/developer/output/materialized identities survive, direct movement rewrites the same owner without a Move event, and one ordinary history controls reorder/Undo/Redo/reload. | pending |
| M83-U11 | Open the selected row's collapsed debug editor. Preserve an unapplied change while selecting controls/rerendering, Reset it, apply a harmless label change, and try malformed or schema-changing JSON. | Compatible raw edits publish atomically; hostile/stale edits leave program, accepted scene and history unchanged; draft and selection are not restored after reload. | pending |
| M83-U12 | Drag a point and an advanced curve control in a complex scene continuously, pause, release, Undo/Redo and reload; switch tool or focus while one drag is pending. Finish once with a newer deliberately invalid terminal sample. Repeat around ordinary Fillet/Offset/authoring controls. | Slow dragging shows a distinct responsive cursor-intent marker while verified geometry catches up and never blanks. Exact release lands at the newest terminal position as one owner rewrite; invalid release cancels instead of committing an older preview or saving; context changes revoke stale prediction; Fillet/Offset/authoring controls remain live. | pending |

## Acceptance rule

Any lost object/feature, branch flip, changed authoring behavior, partial multi-owner drag, stale or
nested history, accepted-scene substitution, failed recovery, reload mismatch or blank scene opens
an exact owner regression under the GeoSolve defect-hardening workflow and withdraws the
candidate. Cosmetic requests unrelated to M83 authority are deferred rather than folded into the
milestone.

M83 closes only after the replacement exact candidate passes this scorecard or receives an explicit
scoped disposition, the supervising caller approves it, and the accepted bytes are subsequently
published and exactly verified on GitHub Pages.
