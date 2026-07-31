# M65 focused UAT

Status: `M65-F003` corrective source is present, but the preceding candidate remains withdrawn and
a mechanically requalified replacement has not yet been published.

Withdrawn candidate code source: `eee2134` (with positive-Temporary prerequisite `fc88264`).

Replacement candidate code source: Pending final qualification.

Tailscale endpoint: Pending refreshed deployment. Do not use the previous endpoint as evidence for
this scorecard.

When the replacement is recorded, use its ordinary workbench for this focused human
usability/behavior check after direct native, WASM and release qualification. UAT is not a
replacement for those tests.

## UAT scorecard

### M65-U1 — continuation without branch jumps

1. Open **Mechanisms → Scotch yoke · 1 DOF**.
2. Delete **Yoke slider on horizontal guide** so the lower point has two freedoms.
3. Drag the lower point through a smooth curved path, including small reversals.
4. Repeat on **Scissor jack** and **Five-stage scissor tower**.

Expected: motion follows the current local configuration. A rejected/ambiguous sample leaves the
last valid preview in place; later motion continues from it. No point jumps to an unrelated valid
assembly merely because another root exists.

Result: Pending.

### M65-U2 — pantograph interaction work

1. Open **Mechanisms → Pantograph linkage · 2 DOF**.
2. Drag **Pantograph input A** through several short arcs, including natural cursor motion away
   from its exact fixed-radius path.
3. Drag the independent guide arm and then alternate between both controls.

Expected: the preview remains responsive and locally continuous. The prior multi-second/tab-lock
behavior is absent. Both freedoms remain usable. This is the ordinary canonical-v4 sample with
its original two `Parallel` relations; the test does not rely on an affine replacement
construction or draft-v5 exception.

Result: Pending.

### M65-U3 — independent twin rollers and rejection recovery

1. Open **Mechanisms → Twin-roller cam · 2 DOF**.
2. Note the starting positions of both roller centers.
3. Drag the left roller by its visible circumference through short horizontal, vertical and
   diagonal moves, including a reversal, while watching the right center.
4. Continue the same gesture for several samples, then repeat symmetrically with the right roller
   while watching the left center.
5. Push one roller toward an invalid or difficult position, then return to a nearby valid
   position without starting a new gesture.

Expected: only the dragged roller follows the local cam-contact branch; the other center remains
visually stationary, with no delayed snap after several samples. Motion remains responsive and
bounded. A failed sample leaves the complete last valid preview unchanged, and the next valid
sample recovers on the same continuation chain. Direct headless qualification holds passive-center
movement to `1e-8`; this UAT confirms the corresponding interaction behavior rather than measuring
that tolerance visually.

Result: Pending.

### M65-U4 — locked elbow explicit branch

1. Open **Mechanisms → Locked elbow · open/crossed branches**.
2. Select the elbow point.
3. In **Assembly branch**, choose **Preview alternate**.
4. Inspect the gold dashed ghost, then Cancel.
5. Preview again and Accept; exercise Undo and Redo.

Expected: Preview never mutates the authoritative sketch. Cancel restores the unchanged view.
Accept switches the representable open/crossed branch atomically, and Undo/Redo treat it as one
edit.

Result: Pending.

### M65-U5 — locked four-bar branch evidence

1. Open **Mechanisms → Locked four-bar · open/crossed branches**.
2. Select the output joint named by the sample.
3. Preview and accept the alternate branch.

Expected: the inspector reports inspected/maximum deterministic seeds. Only the gold ghost changes
before Accept. Accepted geometry is finite, satisfies the constraints and remains editable.

Result: Pending.

### M65-U6 — ordinary workflow regression

1. Return to a normal movable mechanism.
2. Drag, release, Undo and Redo.
3. Create/delete a constraint and save/reload the workspace.

Expected: M65 adds no modal branch behavior to ordinary drag, no sample read-only state and no
regression in ordinary history or persistence.

Result: Pending.

## Finding ledger

### M65-F001 — natural pantograph input motion could still lock the tab

- Reproduction: drag the input/corner point away from its exact fixed-radius manifold.
- Root cause: positive-cost Temporary projection was followed by recursively rerunning the full
  Temporary optimizer for every lower Preference line-search and curvature trial.
- Disposition: recursive Temporary reoptimization remains removed. `M65-F003` first optimizes and
  certifies a Preference baseline while protecting every attained Temporary residual row, then
  permits only bounded optional scalar-level refinement. Failed refinement retains the certified
  row baseline.
- Direct regression: the original two-`Parallel` pantograph accepts the current input path at a
  per-sample peak of 116 factorizations / 93 nonlinear iterations and total `328/265`; the guide
  path peaks at `103/84`, total `291/242`. Wide output and center paths each peak at `67/57` and
  total `598/502`. Every sample stays inside the production pointer vector.
- Human retest: Pending under M65-U2.

### M65-F002 — twin rollers appeared immovable and a difficult drag could freeze

- Reproduction: press a roller circumference rather than its small center point, then drag toward
  a difficult cam location.
- Root cause: curve selection did not own a point gesture, so the circumference was selectable but
  inert; projected pointer work was also unlimited.
- Disposition: circles publish their semantic center as a headless drag handle, gestures preserve
  the initial pointer-to-center offset, and ordinary projected samples use deterministic work
  limits. A rejected difficult sample retains its last valid preview and the next valid sample
  resumes continuation.
- Direct regression: both circle circumferences request movement of their own center without a
  pointer jump; difficult twin-roller rejection and valid recovery are bounded and transactional.
  The complete production vector permits 16,384 items each for document validation,
  dependency/locality and lowering; 256 nonlinear iterations; 512 rejected trials; 1,024
  component linearizations; `256 x 256` dense dimensions and 33,554,432 additive dense-kernel
  work units; 256 factorizations; 256 rank kernels; 512 diagnostic candidates; and 1,024
  diagnostic trials.
- Human retest: Pending under M65-U3.

### M65-F003 — an accepted twin-roller drag moved the untouched roller

- Reproduction: in **Twin-roller cam · 2 DOF**, drag one roller circumference through natural
  short horizontal or vertical cursor deltas. The preceding candidate could accept the sample
  while relocating the other independent roller; continued samples could preserve and accumulate
  that unintended motion.
- Lifecycle root cause: the preceding generic path did not own a gesture-stable, rank-derived
  passive-freedom contract. Broad previous-state intent could be reconstructed from already-solved
  output, allowing passive drift to become the next apparent target.
- Solver root cause: the positive-Temporary Preference path attempted scalar-level retraction. If
  that strict correction failed, it could fall back to the raw post-Temporary state, label the
  lower level `Acceptable` and publish a hard-valid but avoidably drifted passive freedom.
- Corrective design: accepted hard-nullspace evidence selects only the passive point anchors needed
  to cover freedom outside the active point response. Their targets are copied from the accepted
  visible geometry at gesture start. The resulting `DocumentDragLocalityPlan` is stamped with
  design/accepted identity, exact process-local design-publication and accepted-state provenance,
  and the persistent active point, and carries hard-equality DOF, active/passive ranks and anchors
  before being frozen through the complete gesture. Ordinary clones share both private tokens;
  every retained-design or accepted-state publication respectively renews its token. Equal
  revisions from divergent lifecycle clones therefore cannot validate one another's plan or
  preview. The cursor is the sole Temporary target and only those anchors become `PreviousState`
  Preferences.
- Solver disposition: Preference work first optimizes and certifies an exact-complete-Temporary-row
  baseline. Bounded scalar-level refinement is optional; failure retains the baseline. If no
  finite hard-valid and priority-valid baseline certifies, the sample rejects and retains the
  complete last accepted preview.
- Lifecycle/history disposition: current-design and branch previews prove exact design lineage in
  addition to accepted-parent lineage. Signed-zero-distinct same-revision design/branch evidence
  rejects atomically; interaction-free cross-process restore preserves exact canonical bytes.
  Undo/Redo/reload retain the candidate topology and fall back to the untouched candidate if
  imported compatible numerical values invalidate candidate-only topology. Exact release uses
  no-motion certification whose materialization, candidate validation, audit discovery and audit
  refresh all charge the shared controller; cancellation or late exhaustion leaves every
  lifecycle identity, canonical payload and revision high-water unchanged.
- Linear solve disposition: rank-deficient `2 x N` pointer projection uses a stable
  row-space/fixed-SVD solve with the same unsquared rank threshold and returns success only after
  stationarity and minimum-norm/nullspace-orthogonality certification. Exact A5 drag/round-trip
  coverage is restored at `1e-6`, `1` and `1e6`.
- Cleanup: the obsolete `stability_target` / `with_stability_target` request API and
  persistent-ID-ordered passive retry are removed.
- Direct regression gate: accepted-visible target capture, plan stamps/staleness, fixed-point empty
  plans, natural horizontal and vertical deltas for both rollers, continued circumference
  gestures, exact-row baseline retention, difficult rejection and same-chain recovery are all
  direct headless owners. Continued samples hold the passive center within `1e-8` and expose the
  unchanged gesture-start plan. Natural roller work peaks globally at 155 factorizations /
  147 nonlinear iterations; difficult rejection/recovery peaks at `120/89`.
- Qualification: focused provenance, locality, priority, history and work characterization pass.
  The latest focused snapshot (2026-07-31) is core lib 46 passed / 1 ignored; M5 28 passed; M15
  13 passed; M16 49 passed; lifecycle 32 passed; `m65_history` 2 passed; and
  `m65_interaction_lifecycle` 6 passed. The complete format, warnings-denied workspace Clippy,
  locked workspace, WASM and release Trunk gate remains pending.
- Human retest: Pending under the expanded M65-U3 after a refreshed exact-source deployment.

M65-U3 is the targeted `M65-F003` recheck, not standalone milestone approval. Because the previous
candidate was withdrawn, the replacement candidate must still complete M65-U1 through M65-U6;
every result above remains Pending until that exact-source deployment is mechanically qualified.

## Approval

M65 remains open. Approval requires a replacement candidate source and endpoint recorded above,
`M65-F003` mechanical qualification, all scorecard items marked Pass (or an explicitly accepted
scoped limitation), every finding to have a disposition, and an explicit supervising-human
approval statement.
