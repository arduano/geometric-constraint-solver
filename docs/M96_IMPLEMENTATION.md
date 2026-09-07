<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M96 manifold implementation and development evidence

The PC liquid-cooling manifold now has three **12 mm** passages joined to one
reservoir, a separate **12 mm two-bend stair passage** between two ports, and a
**2.4 mm** perimeter silicone groove. The user positively reviewed the initial
6 mm version and requested this 2026-09-08 amendment. M95 remains the accepted
product; the amended M96 layout is not yet human-accepted or clean-qualified.

## Files and API

- `packages/geosolve-sketch-code/src/authoring.ts` exposes the patch-private
  `p.computed.polylineChannel` recipe. The Rust declaration catalog owns its fixed
  six-output shape: left/right wall features and four endpoint points. The
  TypeScript compiler/runtime and generated frontend declarations share that shape.
  `PatchApplication<Project, Result>` preserves typed outputs while allowing
  whole patch invocations in groups with a private invariant project brand.
- `crates/geosolve-sketch-code/src/expansion/channel.rs` composes existing native
  supporting-line offset dimensions, wall segments, computed corner Fillets,
  midpoint/perpendicular endpoint relations and Center Arc caps. Generated IDs
  derive from reconciled patch identity and stable source keys.
- `crates/geosolve-sketch-code/src/composition/channel_validation.rs` validates the
  final accepted finite line/arc boundary through a validation-only host request.
  Cold, incremental and restored publication all reject incorrect connectivity,
  self-intersection, overlapping/touching branches and missing joins.
- `crates/geosolve-sketch-code/assets/bundled-samples/pc-water-manifold/` contains
  `water-channel.patch.ts`, `point-to-point-channel.patch.ts`,
  `silicone-groove.patch.ts`, their pinned artifacts and
  the rebuilt sample source. The trusted-source generator in
  `packages/geosolve-sketch-code/scripts/manifold-patches.mjs` authenticates all three.
- `crates/geosolve-demo-web/src/workbench/code_projects/navigation.rs` includes
  native straight spans and computed bends when a generated wall is selected,
  while excluding borrowed source geometry and the opposite wall.

## Mathematical behavior

A width `w` produces supporting lines at signed distances `±w/2`. A centreline
bend radius `R` produces concentric inner/outer wall radii `R−w/2` and `R+w/2`,
with explicit turn/contact state. Open ends may have tangent semicircles of
radius `w/2`. The manifold uses end caps at its three outlets and leaves inlet
mouths open to the reservoir. Seven native segments complete the reservoir wall.
The separate stair uses both caps and retains two independent R3 bores. The seal
is closed, so it has two closed rounded boundaries and no caps.

For the amended sample, water centreline radius R8 gives R2/R14 walls and R6
caps; the seal retains R3.8/R6.2 walls. Independent expected areas are
**10794.42469 mm²** for the reservoir and its three passages, **1230.69023 mm²**
for the separate stair, and **1515.39822 mm²** for the groove. The native test
checks widths, endpoint incidence, loop counts, enclosure, and separation from
the plate, five bores and eight screws. Intended wet/groove clearance is 2.8 mm
and stair/shared-circuit clearance is 8 mm. The amended native render confirms
finite accepted geometry, normalized hard residuals at most `2.89e-15`, Current
features and zero numerical/effective DOF.

M96-F001 fixes native keyed-polyline offset alias resolution. M96-F002 corrects
Fillet polishing at representable floating-point accuracy, with unchanged final
geometry and branch validation; [its record](M96_F002.md) retains exact before/after
evidence. M96-F003 rejects a cap that intersects an earlier wall. M96-F004 fixes
mixed native/computed wall navigation. [M96-F005](M96_F005.md) ensures boundary
validation uses solved geometry after dimension edits. [M96-F006](M96_F006.md)
recognizes fully constrained large hard components before preference optimization,
preserving rank policy and all hard/soft validation. A malformed artifact
missing a required wall output also rejects before any unchecked lookup.
No new solver equations, persistent computed-feature kinds, unsafe code or FFI
were introduced. F006 adds a rank factorization for large square/tall hard
components before the preference pass; wide underconstrained components skip it.
This preserves the existing policy for actual free motion and does not promise
faster solves for every rank-deficient model.

## Initial 6 mm implementation checks

Commands ran from the repository through `nix-shell shell.nix --run '…'`.
Logs are under `target/m96/`; exact failure and subsequent repair records remain.
This table records the initial 6 mm implementation. The amended sample needs
the fresh results below; earlier timings are not claims about the amended layout.

| Command | Result |
| --- | --- |
| `npm --prefix packages/geosolve-sketch-code test` | Passed runtime, source/artifact fixtures, Deno parity and type checks |
| `cargo test --locked -p geosolve-sketch-code --test declaration_descriptor_parity --test typescript_artifact_interop` | 4 + 5 passed |
| `cargo test --locked -p geosolve-sketch-code --test m96_polyline_channel` | 5 passed, including real native restoration, equivalent length units, invalid units and malformed artifact rejection |
| `cargo test --locked -p geosolve-sketch-code --test channel_boundary` | 3 passed, including crossing rejection and driven edit/replay/restore |
| `cargo test --locked -p geosolve-sketch-code --test m92_fabrication_wave_a fabrication_wave_a_is_source_authoritative_grouped_and_fully_constrained -- --exact` | Passed all five fabrication samples; manifold native checkpoint/host ownership restore exactly |
| `cargo test --locked -p geosolve-sketch-features --lib` | 49 passed |
| `cargo test --locked -p geosolve-demo-web --lib workbench::code_projects::navigation::tests` | 11 passed, including all eight manifold walls |
| `cargo test --locked -p geosolve-headless --test m92_product_design_intent manifold_seal_encloses_the_shared_reservoir_and_routes -- --nocapture` | Passed independent complete sample geometry |
| `cargo test --locked -p geosolve-headless --test m92_product_design_intent manifold_reservoir_and_outlet_edits_retain_the_shared_circuit -- --ignored --nocapture` | 1 passed in 789.51 seconds on the initial 6 mm sample |
| `cargo run --locked -p geosolve-headless -- render --sample pc-water-manifold --out target/m96/manifold-render` | Passed native SVG/PNG/project export |
| `npm --prefix crates/geosolve-demo-web/frontend run wasm:release` | Passed optimized WASM build |
| `GEOSOLVE_BROWSER_COMPILER_HARNESS=1 VITE_GEOSOLVE_MOCK=0 npm --prefix crates/geosolve-demo-web/frontend run build:ui` | Passed browser compiler-harness build |
| `GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18101 GEOSOLVE_CHROMIUM_PATH=$(command -v google-chrome) M92_BROWSER_AUDIT_OUTPUT=/home/arduano/programming/geometric-constraint-solver/target/m96/browser-audit npm --prefix crates/geosolve-demo-web/frontend run test:e2e -- tests/e2e/m92-sample-audit.spec.ts --grep "M92 visual workflow: pc-water-manifold" --workers=1` | 1 passed in 2.3 minutes: both edits, Undo/Redo/reload, selection and group visibility |

Focused warnings-denied Clippy passed for features, channel lowering/validation,
restoration tests and navigation. `cargo fmt --all -- --check`, generated fixture
checks, language-service declaration generation and `git diff --check` passed.
The integrated gate owns final workspace, golden, browser and performance coverage;
these focused checks alone are not a release qualification claim.

The initial browser view was inspected with construction hidden. Native export
`target/m96/manifold-geometry.svg` additionally hides annotations, point markers,
construction and reference/grid layers for a clear geometry-only preview;
`target/m96/manifold-geometry.png` is its rasterization. No geometry was redrawn.

## Amendment and qualification status

Integrated run `20260908T012505-e9d6c999` passes all 243 stages (20 fresh and
223 authenticated reused successes). Source remained unchanged throughout.
This dirty-tree run is provisional development evidence, not a clean release claim.
The exact production artifact is frozen at `target/m96/preview-20260908T010723-6a0a9164/` and
served at `http://100.94.63.83:18101/`. Fresh HTTP byte/MIME and actual-WASM
readiness verification passes. The independent receipt/evidence audit is
`target/m96/qualification-audit.json`; the served artifact receipt is
`target/m96/preview-20260908T010723-6a0a9164-verification.json`. The audit checks
9,892 evidence files and 224 output roots. The served manifest SHA-256 is
`802e70ec15c4ad5aa2d0d8f08e99bf38282d682097dc0830f46082b09e0b90df`;
`target/m96/preview-qualification-binding.json` binds every frozen product file
to the authenticated prepared output and successful transport receipt.

```bash
nix-shell shell.nix --run 'GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh --resume 20260908T010723-6a0a9164'
python3 target/m96/audit-qualification.py 20260908T012505-e9d6c999
node crates/geosolve-demo-web/frontend/scripts/verify-artifact.mjs --manifest target/m96/preview-20260908T010723-6a0a9164/production.json --directory target/m96/preview-20260908T010723-6a0a9164/geosolve-production --url http://100.94.63.83:18101/ --receipt target/m96/preview-20260908T010723-6a0a9164-verification.json
```

The verification command ran in the pinned Nix shell with absolute filesystem
paths and `GEOSOLVE_CHROMIUM_PATH` set to the pinned `google-chrome`; relative
paths above are shown from the repository root.

Browser coverage includes 17 fresh opening/pixel witnesses and 44 fresh full
cases with zero failures, retries or skips. All 45 inventory cases are covered
by these overlapping batches; no browser leaf was reused. Browser batch duration
was 1055.4 s.

The integrated run covers format, warnings-denied workspace Clippy, Rust suites,
optimized WASM, generated packages, 271 unchanged golden cases, browser
selection/edit/history/reload/canvas workflows and performance checks. Exact
stage commands, timings and evidence hashes are retained in the authenticated
qualification manifest. No commit was made and final user acceptance is pending.
The artifact was prepared
in run `20260908T010723-6a0a9164`, whose browser batch could not start because the
interrupted previous run left its test server on 127.0.0.1:4173. The stale server
was verified as this checkout's previous harness and stopped. The replacement
run reuses only authenticated completed successes; browser qualification runs
on the same prepared files. No test assertion or source was changed for this retry.

- `cargo test --locked -p geosolve-headless --test m92_product_design_intent manifold_seal_encloses_the_shared_reservoir_and_routes -- --exact --nocapture`
  passes (1/1, 93.05 s): all four widths and cap/bend radii, exact closed areas,
  separation, five bores, groove enclosure, plate and screw clearances.
  Evidence: `target/m96/thick-channel-geometry-test.log`.
- `cargo fmt --all -- --check` and `git diff --check` pass on the amendment.

- The 12 mm amended sample and all imported patches type-check with zero
  diagnostics (`target/m96/thick-channel-types.log`).
- Native SVG/PNG/project export passes (`target/m96/thick-channel-render.log`);
  `target/m96/manifold-thick-geometry.png` is the native geometry with annotations,
  construction, points and grid hidden, without redrawing geometry.
- The initial integrated run `20260907T222650-701c4508` was interrupted by a host
  update. Run `20260907T230839-b30645b9` passed 241 stages, but one browser pixel
  witness failed; it is not a complete result. The witness sampled a ring obscured
  by later opaque point markers and annotation masks in the narrow split layout.
  It now selects exposed markers using native paint order, retaining nonempty
  sampling, real screenshot pixels and the same seven-sector threshold. The
  original layout case and four M94 canvas cases pass (5/5 in 1.2 minutes).
- Screenshot review found four TS2739 diagnostics because whole patch invocations
  were not typed as groupable declarations. The branded `PatchApplication` type
  fixes that without changing runtime grouping; plain records and foreign-project
  applications reject. An attempted member-reference grouping was rejected by the
  flat-root catalog contract in `20260907T235147-7aa9c0b7` and was reverted. The
  browser layout case now also requires zero TypeScript diagnostics.
- Run `20260907T235737-86172414` passed 241 stages, including the unchanged 271-case
  golden and amended typing, before being deliberately interrupted during browser
  checks for the user’s 12 mm/stair amendment. No completed integrated qualification
  is claimed for any interrupted or failed attempt.

- Run `20260908T003435-00543906` failed the unchanged reservoir-width edit
  after the amendment enlarged the main hard component past the projected-priority
  threshold. M96-F006 fixes the independently reproduced core failure. The core
  threshold regression passes at 127, 128 and 161 coordinates; the complete
  49-test M16 core suite passes, including actual-free-motion and bounded controls.
  The final focused Clippy check passes.
  The exact exported native design now accepts in 0.79 s with hard residual
  maximum 7.63e-12. The full bundled-sample edit/Undo/Redo/reload test passes in 71.58 s.
  The integrated seven-test product-design suite also passes in 65.74 s, including
  full manifold reservoir/outlet edits and independent geometry after both changes.
  Replacement integrated run `20260908T012505-e9d6c999` passes, including the amended
  manifold checks. Diagnostic-only source changes were removed.

## Input and product limits

Qualified inputs are directly authored native keyed polylines of straight spans;
generated and nested inputs are not qualified. Width must be positive, radius
must exceed half width, and corners must be nonzero, nonreversing and fit adjacent
spans. Collinear intermediate vertices must be removed first. The final boundary
is bounded to 512 line/arc edges. Independent sample checks cover separation from
other objects; the helper does not perform a general Boolean union. This is planar
geometry with no channel depth, fluid-flow calculation or seal-compression model.
