<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M68 implementation — headless Fillet direct manipulation

Status: implementation, focused direct qualification and complete release qualification are
complete on frozen candidate `25211e5`. Explicit supervising-human UAT, including the `M68-F001`
through `M68-F003` retests, remains pending; M68 is not yet accepted.

## 1. Files and APIs

The approved vertical slice changes three existing ownership layers without adding a crate or
schema:

- `geosolve-sketch-features` adds
  `ComputedFeatureAuthoringSnapshot::{continue_fillet_corner,continue_fillet_corners,
  continue_fillet_corners_numeric,reseed_fillet_contact,local_fillet_corner_alternatives}`,
  `ComputedFeatureDocument::replace_fillet_set` and the typed sensitivity/contact/alternative
  DTOs. These APIs operate on completed absolute `NewComputedFilletCorner` intent.
- `geosolve-constraint-editor` adds the `SceneFillet*` rail, contact, action, target and typed-limit
  DTOs; Fillet radius/contact gestures and branch-preview state in `ConstraintEditor`; and
  coordinator-owned affordance population, exact preview acknowledgement, numeric/gesture
  publication and explicit action transactions. Feature-only publication continues through the
  existing checkpoint/history owner.
- `geosolve-demo-web` renders the returned DTOs, translates pointer/focus events through exact
  render stamps, captures/releases point/Fillet/pan pointers and exposes the same actions in a
  compact accessible panel. The canvas renders one central radius handle per selected corner and
  no endpoint contact circles; typed contact/branch data remains in the headless interface, while
  the compact panel exposes the branch actions. Its production terminal-route and pan-admission
  policies own only browser capture bookkeeping; they do not select geometry or branches.

The ordinary editable **2D Fillet playground** now contains a friendly line-circle island away
from a fold and a separately labelled radius-`0.5` fold stress island. Both use the ordinary
coordinator and save-like scene data, with no guidance state or protected geometry.

ADR 0032, this ledger and `docs/M68_UAT.md` are the durable records. Workspace v4 and the
separately versioned feature document are unchanged.

## 2. Mathematical behavior

M68 replaces moving-centre distance tracking with the local radius sensitivity of the selected
offset-curve intersection. For persisted normal sides `s_i`, parent parameters `t_i` and offset
points `O_i(t_i,r) = p_i(t_i) + s_i r n_i(t_i)`, the feature layer solves

```text
[ O_1,t  -O_2,t ] [dt_1/dr, dt_2/dr]^T = s_2 n_2 - s_1 n_1
dC/dr = O_1,t (dt_1/dr) + s_1 n_1.
```

The second-parent expression is checked independently. Non-finite, singular, ill-conditioned or
disagreeing sensitivities are rejected. A pointer-down rail maps pointer displacement by
`dot(dp,dC/dr) / |dC/dr|^2`; transverse motion is a no-op. Central finite differences over the
same absolute branch are the independent derivative oracle.

Continuation preserves each completed corner's normal sides, retained endpoints, contact
neighbourhoods/windings, endpoint order, sweep and local root. A fold or domain/regularity limit
retains the last exact current result and reports a typed stop; it never silently changes roots.
Only an explicit contact, retained-direction or bounded local alternative action may re-anchor
absolute corner intent. One successful action publishes radius and any replacement corner intent
atomically in one feature revision/history step.

After a native-source edit, static evaluation and continuation now derive their search domains
through one `current_branch_root_parents` policy. Two affine supports retain their complete
certified cells because fixed-side transverse lines have one unique offset intersection; a case
with any non-affine parent retains the narrow persisted-seed neighbourhood that prevents remote
root hops. This keeps a valid moved line-line Fillet adjustable without weakening real fold,
near-parallel or curved-parent branch protection.

Ordinary pointer continuation deliberately withholds a rail at an exact fold. An explicit numeric
edit may leave that fold only after independently validating the exact affine/non-affine origin,
then resolving the target inside the persisted seed-connected branch cell. An absent, tied or
remote result rejects; numeric editing does not make the fold draggable or globally enumerate
roots.

Ordinary sketch geometry remains outside the feature edit. Every direct test must prove unchanged
sketch identity, accepted coordinates, residuals, rank and DOF. The solver success contract,
priority semantics, residual catalog and M27/M28 advanced Fillet APIs are unchanged.

## 3. Commands and outcomes

The implementation and its UAT repairs are split into eight reviewable source commits:

- `807d2f4` — feature-domain absolute continuation, analytic rail, bounded alternatives and
  atomic configuration replacement;
- `0954e97` — workbench affordances, accessible actions, exact render stamps, friendly/fold
  specimens and point/Fillet/pan pointer capture;
- `240a174` — coordinator/editor Current-only radius/contact transactions, exact terminal-sample
  checks and stale/invalid/foreign-pointer hardening;
- `c82d420` — shared affine-source re-anchoring for evaluation and continuation plus the
  `M68-F001` feature/editor regressions; and
- `227cc9a` — one central Fillet radius handle, no endpoint contact elements or invisible contact
  hit zones, and focused editor/web regressions for `M68-F002`; and
- `5355162` — icon-only branch actions without circular handle-like backplates, preserving their
  canvas and accessible-panel semantics while leaving the central radius grip as the sole handle;
- `8e3ee5d` — independently validated branch-action priority, icon-free retained-direction arrows,
  a 24-pixel action corridor and headless-authoritative preview emphasis; and
- `25211e5` — SVG-stack reconciliation so overlapping painted corridors cannot suppress the
  unique headless-nearest action or fall through to a Fillet drag.

The latest integrated focused gate passed in the project Nix shell:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check &&
  cargo test --locked -p geosolve-sketch-features -p geosolve-constraint-editor \
    -p geosolve-demo-web --all-features &&
  cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web \
    --all-targets --all-features -- -D warnings'
```

Recorded direct results are 35/35 `geosolve-sketch-features` tests, 169/169 editor unit tests,
17/17 M55 integration tests, 14/14 `m66_feature_authoring` tests, 15/15
`m66_feature_authoring_matrix` tests (46/46 editor integration tests in total) and 68/68 demo-web
tests. Formatting and strict native Clippy passed. After the final editor transition fix, the
following WASM commands also passed:

```text
nix-shell shell.nix --run 'RUSTFLAGS="-D warnings" cargo check --locked \
  -p geosolve-demo-web --all-features --target wasm32-unknown-unknown &&
  cargo clippy --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown -- -D warnings'
```

Two independent read-only audits found no remaining editor-transaction or browser-capture
blocker. The editor audit verified invalid-sample token consumption, same-position retry, exact
terminal-coordinate matching, stale acknowledgements, foreign pointers and cancellation. The web
audit verified exact-once release ownership, outside-canvas terminal delivery, non-stealing pan,
and camera/workspace cancellation ordering.

An additional independent review of `M68-F001` found no blocker: the repair aligns only the
affine/affine reevaluation and continuation domains, while genuine fold conditioning and every
non-affine seed-local guard remain unchanged.

The complete release-gate command sequence then passed from clean source `25211e5`:

```text
nix-shell shell.nix --run './scripts/release-gate.sh'
```

The wrapper invocation passed formatting, warnings-denied workspace Clippy, locked all-feature
tests, all-feature WASM, rustdoc, benchmarks and the normal performance suites before the execution
host terminated it while the unchanged long regression was running. On the same untouched clean
source, the exact release-only 256-moving-body spatial sparse-crossover command passed in
`136.32s`; the remaining licence/package and release Trunk commands then passed. Cargo's repeated
`license` plus `license-file` advisory is pre-existing and non-blocking; the explicit licence gate
passed.

The frozen release distribution is identified by:

```text
sha256sum crates/geosolve-demo-web/dist/* | sha256sum
24438f7019d58628ca3c34814be890c6a7a6687f233545d7b6ef03ee84664e05  -
```

The focused candidate is served from that exact distribution at
`http://100.94.63.83:8080/` for `docs/M68_UAT.md`. All seven served HTTP responses match their
local release files by SHA-256.

## 4. Acceptance criteria

Focused direct evidence now covers:

- analytic rail sensitivity against central finite differences across line-line, line-circle and
  line-Bezier families, scales/transforms, motion directions and folds;
- absolute branch preservation, explicit bounded alternatives/contact reseeding and atomic
  grouped-radius conflicts;
- exhaustive headless pointer and action transitions, including invalid release, cancellation,
  multiple pointers, stale/foreign owners, hover/click parity and one-step history;
- identical Current-only semantics for authoring, published dragging and numeric editing;
- pointer capture/release and camera-cancel event translation in the thin workbench adapter;
- persistence, Undo/Redo/reload and the complete `M66-PF001` through `M66-PF004` regression set;
- large native source-point edits followed by grouped-Fillet rail exposure, preview and
  publication without a false fold; and
- exactly one visible central radius handle per selected Fillet corner, with no endpoint contact
  element, invisible contact-hit zone or circular branch-action backplate;
- independently validated arrows outranking overlapping Fillet surfaces, deterministic
  headless-nearest resolution across overlapping SVG corridors, no adjacent redundant arrow glyph
  and strong headless-authoritative hover emphasis; and
- unchanged native sketch identity, coordinates, residuals, rank and DOF for every feature-only
  edit.

The bounded radius reference model has 28 reachable states—one idle, 23 live, one cancelled and
three released—and exercises all 240 applicable state/event transitions from canonical prefixes
of at most five events. It compares exact durable feature JSON/radius/stable IDs, history length
and cursor, held preview revision, active pointer ownership and native sketch coordinates,
residual/rank/DOF invariants after every transition.

The complete objective mechanical gate now passes. `docs/M68_UAT.md` still requires explicit
supervising-human approval over the frozen build served through Tailscale. M68 remains open until
that approval is recorded.

## 5. Known limitations or next blocker

The sole remaining blocker is focused human UAT of the frozen candidate, including explicit
retest of `M68-F001` through `M68-F003`. This is a scoped active milestone, not a completion record.
M68 intentionally excludes Offset/Mirror authoring,
two-non-affine-parent Fillets, computed-on-computed chaining, Bake/Explode, profile/topology
consumption, cross-revision topological naming, computed arcs as constraint operands, schema
changes, global
root enumeration, browser E2E, mobile and legacy UI.

The interaction foundations may be reusable for later computed geometry, including variable-
topology offsets, but no such feature or acceptance claim belongs to M68.
