<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M95 implementation and qualification

Status: **in progress** from accepted M94 closure source `d4438a9`.
[M95_GOALS.md](M95_GOALS.md) records the approved contract. M94 remains served at port 18096;
M95 has only a provisional development build, with no clean nomination or user acceptance yet.

## Reproduction and implementation

**M95-F001 — Explorer selection did not select canvas outputs.** The exact public native setter
regression at `d4438a9` expected a Point selection but received an empty list. The existing
`set_selected_declaration` intentionally clears native selection because editing and restoration
rely on that contract. The separate `select_navigation_declarations` route now resolves complete
accepted owned outputs without changing mutation/drag semantics. The bridge reproduction
`m95_explorer_selection_paints_accepted_outputs_without_history` independently failed before
integration and passes afterward, with byte-identical complete persistence.

New native APIs resolve accepted outputs, exact authenticated output bindings, visible declaration
owners and reverse selection ownership. Curves expand to all spans; exact generated points/spans
stay exact. Borrowed input operands remain excluded. Computed Feature/FeatureCorner selection,
captured-gesture rejection, modifiers and truthful unique Inspector ownership have owner tests.

The bridge caches Explorer/provenance by accepted Intent/code/source/visibility plus a transient
instance token. It emits multi-owner navigation and selection deltas without resending source,
parameters or project state. Direct source ranges remain separate from hierarchy membership;
partial parent selection cannot highlight unrelated helper expressions. The compiler-authenticated
statement sites currently cover builder expressions (for example `$.geometry.segment(...)`),
not `const name =`, comments or arbitrary TypeScript references. Selecting a full declaration line
intersects that expression; placing the cursor only in its binding prefix is unmatched.

Frontend CodeMirror decorations retain text selection and focus. Automatic reveal keys selected
identities rather than source offsets. Ordinary selection preserves layout; explicit Show in
canvas/Show in code opens Split when needed. Dirty/retained-invalid source and active tools/gestures
are guarded. Hidden/suppressed rows remain browsable. Group/member toggles, multiple empty owners,
Unicode, stale replacement and Apply/history reconciliation have focused integration coverage.

Audit refinements prevent sibling action strips, honor construction/annotation visibility, avoid
an arbitrary Inspector for multi-owner selection, and use the existing single-node Inspector API
instead of rebuilding the full workbench projection on each selection. Selection-delta decoding
rejects malformed/stale owners and preserves unchanged object references and presentation order.
No solver mathematics, residuals, tolerances, branch state, persistence formats or golden bytes change.

## Executed development checks

All Cargo commands below ran through `nix-shell shell.nix --run`, with `CARGO_BUILD_JOBS=4` for root
integration builds. They are focused development evidence, not a clean integrated qualification.

- Native exact old-setter reproduction: confirmed red, new route green. Full editor/Fillet/Offset
  collateral suites passed 15/13/12 at that revision; later focused editor M95 cases and exact-output
  helper passed. Strict owning Clippy passed. Logs: `target/m95-native-*.log`.
- `cargo test --locked -p geosolve-demo-web --lib code_projects::navigation -- --nocapture`:
  **8 passed**, including retained failure, host Fillets, helper ownership, polyline members,
  borrowed operands and deletion/history/restore. No golden expansion.
- `cargo test --locked -p geosolve-demo-web --lib m95_ -- --nocapture`: final focused root pass
  **10/10** in `target/m95/bridge-audit-r5.log`; exact persistence checked after navigation.
- `cargo clippy --locked -p geosolve-demo-web --lib --tests -- -D warnings`: passed after the final
  owner/Fillet refinements in `target/m95/clippy-final-r2.log`; integrated qualification remains.
- Frontend types passed; App/editor/source conversion/adapter suites passed **85 tests**. The
  subsequently repaired selection-delta/adapter validation suites passed **16 tests**. Logs:
  `/tmp/m95-frontend-final-unit.log`, `/tmp/m95-wasm-adapter.log`, and source-agent transport logs.
- `cargo fmt --all` and `git diff --check`: passed during development.
- Optimized WASM plus harness/production build passed into `/tmp/geosolve-m95-dev-r1`, using
  `CARGO_PROFILE_RELEASE_INCREMENTAL=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 npm --prefix
  crates/geosolve-demo-web/frontend run build:release-artifacts -- --out /tmp/geosolve-m95-dev-r1`.
  Log: `target/m95/dev-build-r1.log`. Subsequent source refinements are not in that provisional build.

All three new browser workflows now pass on the provisional candidate: explicit source/dirty/layout
in `/tmp/m95-browser-r2.log`, canvas/Explorer/pixels and dense generated navigation in
`/tmp/m95-browser-r3.log`. Initial failures were harness assumptions: unnormalized source, builder
expression ranges, partial whole-declaration coverage after a span-only pick, and a generated scalar
port incorrectly expected to paint a curve. Corrected assertions keep actual selected-stroke pixels,
exact saved bytes and geometry, cursor/focus/layout and Unicode navigation requirements.

Independent final review additionally found alternate Feature/FeatureCorner coverage and a group
with one visible and one deselected hidden descendant. Focused regressions now require a picked
Fillet to toggle off completely and a mixed-visibility group to complete its partial selection.
Multiple empty generated siblings resolve to their common declaration rather than a lexical child.

Clean integrated qualification, final dense comparison, frozen Tailscale candidate and
supervising-user acceptance remain outstanding.


## Visual review and preflight interruption

The independent browser pass also qualified Jansen's existing authoring→Open source workflow;
it explicitly returns to Select before navigation. Four targeted workflows pass in 30.3 seconds.
Visual review then found that the declaration selected-background CSS was absent, and generated
centre/curve/radius ports shared a label. Explicit state classes now paint the background; colliding
member labels append their authenticated output path. Inspector navigation uses the same friendly
row label instead of an internal generated alias. A browser computed-color assertion joins the
actual selected-canvas-pixel witness. Final polished candidate-r2 checks pass **4/4** in 32.2s,
with screenshots in `target/m95/browser-polished`; the subsequent Inspector-label-only refinement
is covered by the next integrated build. Strict Clippy passes in `target/m95/label-clippy-r2.log`.

The clean `e2abf18` nomination started run `20260907T181340-9f40bbb2`, then was deliberately
interrupted during `preflight.frontend` to incorporate those visual findings. Its three completed
preflight stages are recorded successes, not complete product qualification. Resume will authenticate
any reusable inputs; no domain/golden/browser gate passed is claimed from that interrupted attempt.

Preliminary repeated direct-WASM selection in the robotic harness (12 member/invocation pairs)
measures 14.6 ms median for one exact member and 9.7 ms for its 18-output invocation. M94's prior
Explorer route takes 40.4/39.8 ms while publishing complete snapshots and lacking canvas-output
selection. M95 delta payloads are about 457–481 thousand UTF-16 characters versus 1.069 million;
both runs preserve exact complete persistence. These are synchronous bridge phase measurements,
not end-to-end FPS. The richer new behavior and differing payloads are disclosed; final artifact
pan/zoom/drag checks remain. Raw evidence: `target/m95/performance-{m94,provisional}/selection.json`.


**M95-F002 — retained hidden selection lost its partial group marker.** With two hidden circles,
select both rows, toggle the second off, then create a dirty source draft. Accepted logical
selection survives index reconciliation, but the ancestor row omitted that logical coverage.
The extended `m95_multiple_hidden_rows_keep_browsing_without_inventing_an_inspector` regression
fails on `9acc023` (`target/m95/hidden-draft-red.log`). Counting that retained logical coverage in
the partial-row case repairs the projection without changing selection, source ownership or history.
The complete focused `m95_` suite passes **10/10**, then strict Clippy passes in
`target/m95/hidden-draft-green.log`.

Run `20260907T182205-867074e5` was deliberately interrupted in `prepare.workspace` for this
one-line correction after all six preflight stages passed. Its incomplete qualification is
retained honestly; the next nomination resumes with input-authenticated successes. No completed
domain/golden/browser suite was repeated due to either preflight interruption.
