<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M87 focused UAT — Managed controls and browser-free authoring

Status: **accepted and closed on 2026-08-31**. The supervising user's explicit “Close off this
milestone” decision accepts M87-U9/U10 at milestone level without claiming a separately logged
row-by-row visual replay. The earlier U1-U8 scoped disposition remains historical. The retained
sound work and complete adaptive-detail/LOD removal are unchanged. M88 followed and is now
complete.

## Candidate identity

Exact accepted product source is `32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`. It passes the complete clean release gate at exit `0`
with `NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`. No immutable no-rebuild
candidate or public publication was created; the mutable Tailscale development server remains
collaboration evidence, not publication authority. Historical pre-F003 dirty evidence used:

```bash
TMPDIR=/home/arduano/.cache/geosolve-m87-tmp GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run 'TMPDIR=/home/arduano/.cache/geosolve-m87-tmp ./scripts/release-gate.sh'
```

That historical dirty gate does not qualify the revised fully constrained samples. The reviewed twelve-row
code-project ledger has SHA-256
`f6ecd037cef8befc59f9a057fef499a14f0851f8ec5a0d3a1468a69e66a9d1bc`. The later clean gate qualifies
that exact ledger and complete post-F003 source but does not create a no-rebuild freeze, public
deployment or service-retirement claim. The manufacturing owner suite passes 3/3, native composition passes 13/13, the
reviewed-ledger check passes 1/1 and the all-demo headless/deterministic-product suite passes 10/10.
Fresh mutable review bundles are:

- M87-U9: `/tmp/geosolve-m87-post-f003.FPHP3b/cnc`;
- M87-U10: `/tmp/geosolve-m87-post-f003.FPHP3b/gridfinity`.

The mutable development distribution was rebuilt from the current dirty source and all seven files
served at `http://100.94.63.83:8080/` byte-match the local Trunk output. This is collaboration
infrastructure only, not a clean nomination, immutable candidate or publication result.

## Scorecard

| ID | Human action | Pass condition | Status |
|---|---|---|---|
| M87-U1 | Open **Typed Panel · keyed Fillets**, select either generated Fillet and change radius from `4` to `2` in Inspector. Undo/Redo, then repeat with the selected Fillet's radius grip while observing multiple pointer frames before release. | Inspector shows one shared source owner and two consumers. One exact source token changes, both Fillets become radius `2`, the stable selection remains usable and exactly one outer history row is added. During the grip gesture both Fillets preview the same radius without source/history publication; release performs the same one outer source transaction. | accepted by historical scoped disposition; not separately replayed |
| M87-U2 | Open the Code panel with no relevant canvas selection and inspect/edit a managed value that has no currently selected one-to-one Inspector property, then Undo/Redo it. | The Code panel exposes the complete current manifest rather than only selected-property controls. The edit changes its exact source value and accepted geometry once, adds one outer history row and round-trips through Undo/Redo without requiring canvas selection or an `EditLens`. | accepted by historical scoped disposition; not separately replayed |
| M87-U3 | Open **PC Water Manifold**, inspect `channelBendRadius`, and change it from `5` to `4`. Review the disclosed upper, middle and lower invocation consumers. Then give one invocation a direct local `bendRadius` literal through managed source, Apply, and edit that local control. | The shared binding is one control whose complete fan-out names all six Fillets across the three exact invocations; its edit updates all three groups atomically. After one invocation receives a direct literal, its local control updates only that invocation, while the shared control retains exactly the other two groups. No route falls back to the first invocation or copied native identity. | accepted by historical scoped disposition; not separately replayed |
| M87-U4 | Without a browser, server, DOM, network or Node runtime, run the complete `geosolve-headless` loop: inspect a bundled demo, copy a complete current control token into an exact-CAS edit batch, publish to a new directory, inspect the emitted `project.json`, and render it to another new directory. Open the resulting SVG and PNG. | Inspect/edit/render complete successfully from cold native authority; the edited source and consumers are correct; reports show finite accepted geometry, independently valid Hard residuals and Current active features. Output directories are atomic and non-overwriting. The SVG is useful deterministic design evidence and the `2000 x 1400` PNG visibly contains the expected geometry and annotations without external resources. | accepted by historical scoped disposition; not separately replayed |
| M87-U5 | Load the dense multi-model reproduction or **PC Water Manifold**, then wheel/pan continuously while maximizing, restoring and resizing the window. Stop navigating, click geometry, and repeat one wheel gesture. | The complete scene remains painted throughout. No “retained/required viewport presentation unavailable” state strands the canvas, no blank viewport remains after resize, and the later click and wheel gesture still render. No adaptive-detail control or reduced-paint state is present. | accepted by historical scoped disposition; not separately replayed |
| M87-U6 | In **Typed Panel · keyed Fillets**, repeatedly drag the upper-left rectangle corner through short, long and fractional movements, including several releases at nearly the same screen position. Select both generated Fillets and inspect radius plus contact/branch fields; also select a code-owned point and an ordinary GUI-owned item. | Every accepted release remains at its final native preview with no intermittent snap-back. Radius says **Modifiable in sketch.ts**, names `cornerFillets.radius`, shows `mm(4)` and two generated consumers. Generated contact/branch fields and the code-owned Display name say **Encoded** and cannot be edited; solver point coordinates say **Modifiable instance**; ordinary GUI-owned controls remain editable. A dirty managed-source draft changes code-owned metadata/actions to **Blocked** until Apply/Revert. | accepted by historical scoped disposition; not separately replayed |
| M87-U7 | Open **Robotic cable-harness routing board** and inspect the full fitted scene. Pan/zoom, select several clips and bends, inspect `sharedClipRadius`/`sharedBendRadius`, then drag `serviceRoute.serviceLoop` through two distinct releases followed by Undo twice, Redo twice and reload. Compare the native view with a fresh headless SVG/PNG render. | All eight routes remain complete and readable inside the 360 x 220 mm board; 80 clips and 64 rounded bends are present with no truncated chain, missing corner or partial scene. The shared controls disclose all 80/64 consumers. Each service-loop release remains at its terminal, moves only the expected route-local geometry, rewrites no `sketch.ts`, and survives history/reload with all Fillets Current. Headless vector/raster evidence matches the same accepted design. | accepted by historical scoped disposition; not separately replayed |
| M87-U8 | In the routing-board source, localize only `serviceHarness` clip/bend radii, Apply, then add keyed vertex `inspectionClip` between `strainReliefB` and `sink`; Undo/Redo the insertion and remove/reinsert it once. | Only the service route changes when its local controls are edited; the other seven retain their shared values and geometry. The service fan-out is 10 local clips/8 local Fillets while shared fan-out is 70/56. Insertion adds one point, segment, clip and rounded bend without disturbing unrelated routes; Undo/Redo/reload remain stable and reinsertion is accepted without stale-owner leakage. | accepted by historical scoped disposition; not separately replayed |
| M87-U9 | Open **CNC joinery fit coupon · keyed corner reliefs** and inspect the complete fitted view, then compare it with a fresh post-F003 headless SVG/PNG. Inspect the shared cutter/handling radii, minimal datum authority and loose/nominal/press stations. | One 120 x 140 mm blank shows three complete 70 mm-wide mortises with loose/nominal/press heights of 18.4/18.0/17.6 mm, three 95 x 18 mm tabs, all twelve conservative corner-relief circles and ten smooth handling Fillets. The stations are coherent, unclipped and visually distinct, and shared/local control fan-out is understandable. The accepted report has one `FixedPoint`, no `FixedCoordinate`, numerical/structural nullity zero and DOF zero. The view is explicitly a 2D/2.5D fit drawing, not a CAM/toolpath preview. | accepted by 2026-08-31 milestone-level close decision; not separately replayed |
| M87-U10 | Open **Gridfinity 1×1×3U section · keyed standard profile**, fit the complete scene and compare it with a fresh post-F003 headless SVG/PNG. Inspect the floor- and lip-radius controls and symmetric construction authority independently. | One symmetric closed material contour visibly contains the full base, cavity floor, both walls and both stacking lips. The 41.5 mm outer width, 35.6 mm base bottom, 4.75/7 mm base levels, 21 mm body, 0.95 mm walls and 4.4 mm nominal lip are represented coherently; two floor and two lip Fillets are present and independently understandable. The accepted report has no `FixedPoint`, one Y `FixedCoordinate`, numerical/structural nullity zero and DOF zero. It is clearly a cross-section sketch, not a solid or print-fit claim. | accepted by 2026-08-31 milestone-level close decision; not separately replayed |

## Disposition

On 2026-08-30 the supervising user stated that further LOD work was putting the milestone in a
weird, risky state and explicitly requested complete LOD removal, a graphics audit and retention of
the good code. That earlier scoped disposition remains historical acceptance of U1-U8 without a
fresh row-by-row replay; M87-F002 remains resolved at the renderer owner. The later CNC/Gridfinity
amendment created two additional visual rows. M87-F003 then replaced literal/fixed-lock authority
with fully constrained relational designs. On 2026-08-31 the supervising user explicitly requested
M87 closure; that milestone-level disposition accepts U9/U10 without inventing a separate replay.
Exact source `32c7289`, tree `38f7175`, passes the complete clean gate. No immutable artifact,
public deployment or service-retirement result is inferred; the existing Tailscale listener remains
a mutable development service. Diagnosed Gridfinity performance/stack work is carried into active
M88's ordered stability prerequisite.
