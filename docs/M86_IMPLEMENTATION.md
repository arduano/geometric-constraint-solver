<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 implementation ledger — Focused bug fixes and UAT follow-up

Status: **accepted and clean-qualified on 2026-08-29; public closeout remains pending**.
M86-F001-F003 and the bounded interaction trace are implemented and mechanically
qualified. The supervising user's explicit “looks good, let's close the milestone” decision
accepts M86-U1-U8 without claiming a separately logged row-by-row replay. Accepted descendant
`88d1b5e` / tree `09018e5` passes clean qualification and no-rebuild HTTP verification. The former
F002 nomination remains withdrawn.
`docs/M86_GOALS.md` owns the contract.

## Finding ledger

### M86-F001 — Code-owned direct dimension edits reject after nested Intent mutation

Disposition: **repaired, clean-qualified, immutably nominated and accepted by the supervising
user's scoped “Looks good” assessment**.

Reproduction baseline is M85 closeout head `4b69a57`. Open PC Water Manifold, select
`code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`,
and change its Inspector value from `16` to `8`. Authenticated expansion provenance resolves the
alias to managed declaration `topScrewRail3Length` and exact source `target: mm(16)`. The displayed
failure says changed leaves lack semantic GUI-draft provenance; source, annotation and accepted
native target remain `16`, and no outer code-history entry is created.

The generic Inspector dispatcher edits the nested Intent scalar. Code-project checkpoint
publication then classifies the delegated editor delta and rejects because the only semantic GUI
draft provenance currently modeled there is for point-placement overlays. Existing managed scalar
lenses already know how to rewrite a declaration's `target`; the missing boundary is an
authenticated Inspector-to-managed-source route.

First regression owner: `geosolve-demo-web`'s optional code-project workbench composition. The
crossed browser adapter receives one thin dispatch test. No core/sketch residual, Jacobian or broad
golden change is warranted.

### M86-F002 — Computed Fillet radius surface hides its persistent parent endpoints

Disposition: **expanded during UAT, repaired, provisionally qualified and accepted by final
milestone approval; the prior nomination remains withdrawn**.

Reproduction baseline is F001 documentation head `bcc5ae4`. In Select mode, use two joined native
line/polyline spans with a computed Fillet, expose the selected Fillet radius affordance and sample
their retained shared endpoint where the radius rail also hits. `EditorScene::hit_test` returns the
persistent point, but both `pointer_move` and `pointer_down` return the computed `FeatureCorner`, so
the original corner cannot be selected or dragged.

The headless Select resolver unconditionally asked the blended Fillet resolver first. That resolver
correctly treats grip, spoke, continuation rail and arc as one radius surface for M75 painted-item
parity, but could not express a point-versus-Fillet specificity hierarchy. First regression owner is
`geosolve-constraint-editor`; no browser-only or solver-layer correction is warranted.

The first repair and candidate covered a persistent endpoint shared by both parents. During UAT the
supervising user supplied the broader exact workflow: draw one right-angle two-span `Polyline` with
both legs length `2`, then request a Fillet radius of `2`. Exact radius `2` is the evaluator's
tangent-at-endpoint fold boundary; accepted radius `1.99` robustly retains the same visible broad
Fillet surface and proves that it covers both remote parent endpoints. Hover/down still returned the
Fillet over the middle of each point marker. This has the same owner, symptom and root cause, so it
expands M86-F002 and withdraws source `dbe94da` rather than opening another ID.

### M86-F003 — Typed Panel terminal snaps back after valid release

Disposition: **confirmed, repaired, provisionally qualified and accepted by final milestone
approval**.

Open **Typed Panel · keyed Fillets**, drag the upper-left rectangle corner from `[0, 40]` through
accepted previews to `[3, 38]`, and release. Native pointer-up keeps the exact last preview, but
retained code publication previously rejected with
`terminal code drag differs from its independently staged native authority in computed features`
and restored the pre-drag durable scene. This looked like nondeterministic snapping but is a
synchronous retained-terminal authority rejection, not drafting inference or a solver race.

M84-F010 already permits tightly bounded finite normalization of redundant rectangle aliases in
design and current accepted documents. Cold rematerialization of Typed Panel's keyed Fillets can
inherit ULP-scale differences from exactly that alias normalization, while the public computed
snapshot comparison remained bit-exact. M86-F003 is therefore an independent M84-F010 scope
recurrence owned by `geosolve-demo-web`'s retained code-workbench terminal parity boundary. It does
not reopen or renumber historical M84-F010.

## Implemented design

### I1 — Exact owner regression

- [x] Load the checked-in PC Water Manifold through the ordinary code-project constructor.
- [x] Authenticate the exact alias and assert it resolves to `topScrewRail3Length`, direct
  `dimension.curveLength`, target leaf and source `target: mm(16)`.
- [x] Drive one Inspector-equivalent managed edit to `8`; assert only that source token changes,
  native target is `8`, accepted geometry is finite and independently Hard-valid, the stable alias
  is immediately reselected and exactly one outer code-history entry is added.
- [x] Assert Undo/Redo restore exact source, target and accepted checkpoint. Redo additionally proves
  the stable semantic alias remains present; durable UI selection across the history step is not
  asserted.
- [x] Add an accepted direct-diameter `5 -> 8` fixture with exact Undo, plus manifold diameter
  retained-invalid `5 -> 0`, byte-exact accepted-authority retention and exact Undo. No golden row
  is added.

Owning tests:

- `m86_f001_manifold_dimension_inspector_rewrites_managed_target_atomically`
- `m86_f001_direct_diameter_inspector_rewrites_managed_target_and_exact_undo`

### I2 — Managed Inspector route

- [x] Add private `CodeInspectorEditRoute::{NotClaimed, Claimed}` and
  `CodeProjectWorkbench::apply_managed_dimension_inspector_edit`, accepting the exact authenticated
  Inspector identity, selected alias, semantic target leaf and finite scalar.
- [x] Authenticate the current selected Inspector projection, materialized session identity,
  accepted alias-to-managed-declaration provenance, direct non-patch builder family and exact
  `Target[0]/Value` leaf. Only `dimension.curveLength` and `dimension.diameter` are claimed; opaque
  alias text is never decoded.
- [x] Require the managed `target` scalar and accepted Intent literal to agree, then reuse the
  existing `apply_scalar_lens(..., "target", value)` path. That ordinary path rewrites source and
  parses, expands, materializes, solves and independently validates before publication; native
  authority is never edited directly.
- [x] On accepted rematerialization, look up and immediately reselect the stable semantic alias.
  The adapter installs the complete returned `WorkbenchDocumentAuthority`, including revision
  high-water metadata. Save-time checkpoint publication observes the already-published checkpoint
  and adds no duplicate outer history row.
- [x] Reject dirty, retained-failed, pending semantic-point gesture, stale, foreign, unsupported,
  wrong-leaf, wrong-unit and non-finite work before nested Intent mutation.

### I3 — Adapter and failure retention

- [x] Route only authenticated code-owned direct dimension controls through I2 before the generic
  Inspector mutation path; preserve the existing generic dispatcher for GUI-owned targets and
  every unrelated Inspector field.
- [x] Add thin adapter regression
  `m86_f001_projectional_browser_inspector_routes_managed_dimension_to_source`. It proves exact
  browser dispatch, complete authority metadata installation, idempotent save/no duplicate outer
  history, and byte-exact accepted-authority retention after a zero target.
- [x] Prove failed rematerialization retains prior accepted native authority while retaining failed
  managed source and one exact outer Undo row. While a retained code failure is active, a further
  Inspector rewrite rejects until source correction or Undo. No partial accepted editor or
  selection publication occurs.
- [x] Keep existing GUI-owned reference-dimension and generic Inspector Undo/Redo tests passing;
  GUI-owned declarations return `NotClaimed` from the optional code route.

### I4 — Qualification and release

- [x] Run focused owner, diameter, adapter and GUI-owned collateral tests.
- [x] Pass format, diff hygiene, warnings-denied affected-crate Clippy and the relevant WASM target
  check.
- [x] Pass complete affected-crate/workspace tests, unchanged 271-case clean golden and the clean
  Nix release gate.
- [x] Freeze the exact no-rebuild candidate and exact-verify local/Tailscale bytes.
- [x] Record the supervising user's scoped “Looks good” assessment as acceptance of M86-U1 through
  M86-U5 without claiming a separately logged row-by-row replay.
- [ ] Publish and exact-verify Pages only after explicit supervising-user approval; retire retained
  services and close M86 afterward.

### I5 — Fillet Select specificity hierarchy

- [x] Retain exact native/WASM hover/down regression
  `m86_f002_fillet_source_corner_remains_selectable_through_its_radius_surface`, including ordinary
  Point hit, simultaneous broad Fillet hit, Point hover/selection and ordinary Point gesture.
- [x] Add the real two-span right-angle Polyline fixture with two length-2 legs and accepted radius
  `1.99`, immediately below the reported radius-2 tangent-at-endpoint fold boundary. Prove that both
  remote endpoints lie inside the same broad Fillet surface and remain reachable.
- [x] Separate the compact radius grip from the blended broad radius surface in the private Select
  resolver. Resolve in this order: compact grip; visible persistent endpoint belonging to either
  current parent; broad Fillet arc/spoke/rail; ordinary geometry.
- [x] Reuse the feature-authoring owner's exact line/polyline span-endpoint query. Exact shared
  identity and active Coincident representatives retain semantic-corner handling. A one-parent
  remote endpoint also qualifies. When distinct opposite-parent point halos both hit, choose the
  nearer point; an exact cross-parent distance tie returns to the Fillet rather than using coordinate
  overlap as topology.
- [x] Make active-Coincident representative construction request-local and lazy. A broad Fillet-only
  hit returns before document-wide incidence work; one `OnceCell` shares at most one construction
  across every overlapping owner and the final semantic collapse after an endpoint candidate hits.
  The exact unit regression samples a real broad arc outside every point halo and proves the cell
  remains uninitialized.
- [x] Keep `EditorScene::resolve_fillet_hit_with_policy` unchanged for active Fillet authoring,
  painted-radius reconciliation and public Fillet-aware picking. Unrelated overlapping points and
  passive curves remain below the broad surface.
- [x] Expand `m75_hover_pointer_parity` to 18 rows. Native and WASM pass 18/18, including the named
  M86-F002 rows plus compact-grip, nearer-endpoint, cross-Fillet and disconnected-tie assertions.
- [x] Pass current warnings-denied affected-crate Clippy and unchanged 271-row golden authority.
- [x] Pass the complete provisional dirty-worktree gate, freeze its exact no-rebuild output and
  exact-verify temporary/local/Tailscale bytes. Preserve the patch as build identity; do not call it
  a clean-source nomination.
- [x] Complete focused replacement UAT under the supervising user's 2026-08-29 milestone-level
  close decision without claiming a separately logged row-by-row replay.

### I6 — Causally bounded Typed Panel computed parity

- [x] Add exact owner regression
  `typed_panel_upper_left_terminal_keeps_the_last_native_preview`. For targets `[3, 38]`, `[5, 37]`
  and `[-2, 36]` sequentially in one Typed Panel session, it drives real pointer frames and requires
  exact preview, native terminal, published and restored coordinates; exactly two canonical
  semantic drafts; one outer revision;
  finite accepted geometry; Current keyed Fillets; and independently validated Hard residual
  `<= 1e-9`.
- [x] Replace the Boolean document-parity result with an exact/mismatch/normalized-set result. A
  derived computed tolerance can activate only when design and current accepted document parity
  both normalize the same nonempty set of redundant rectangle alias point IDs. An exact side,
  unequal sets or mismatch keeps computed parity exact.
- [x] Derive the bounded source-curve set from accepted public curve definitions that reference one
  of those normalized points. Admit finite scalar roundoff only on public computed edges and
  construction fragments causally sourced by those curves. Unrelated edges/fragments remain
  bit-exact.
- [x] Keep edge identity/role/source, Fillet sweep, tangent orientations, contact winding,
  provenance, topology/public evaluation mapping, feature definitions, persistent identities,
  logical and native ownership and allocator high-water exact. Revision/digest and evaluation
  lifecycle stamps may refresh during canonical rematerialization and remain outside parity. More
  than 8 ULP, out-of-cell near-zero or non-finite differences reject.
- [x] Add focused guards
  `rectangle_terminal_derived_roundoff_is_causal_and_bounded`,
  `rectangle_terminal_derived_roundoff_keeps_public_fillet_branch_state_exact` and
  `computed_roundoff_requires_matching_design_and_accepted_alias_normalization`; retain the
  rectangle scalar contract and Compass Rose terminal collateral.
- [x] Keep private continuation certificates, including transverse-orientation metadata, outside
  this demo-workbench public computed DTO parity comparison. They are not implicated by the Current
  Typed Panel case and do not block this repair.
- [x] Pass focused owner/boundary/collateral tests, warnings-denied affected-crate Clippy and the
  unchanged 271-row golden.
- [x] Pass the fresh complete demo-web suite: 307/307 library tests plus the binary, integration
  and doc-test targets.
- [x] Pass the complete provisional workspace/release gate and serve immutable replacement bytes
  for UAT. At that provisional checkpoint, clean committed-source nomination remained pending; the
  final record below completes it.

### I7 — Bounded interaction trace

- [x] Add a memory-only `InteractionTrace` with a 192-row working bound and hard 128 KiB export
  bound. Overflow preserves pointer-down plus the newest terminal, rejection and rollback evidence;
  all fields are single-line, UTF-8-safe and control-character sanitized.
- [x] Trace raw/coalesced/queued/animation-frame pointer samples, native pointer-up, authenticated
  semantic route, staged overlay, first computed mismatch with exact bits/ULPs, parity result,
  publication, accepted-authority restore, persistence and presentation work.
- [x] Keep the trace absent from code source, retained documents, workspace history, reproduction
  payloads and local storage. Empty traces remain empty until a real managed-code pointer gesture.
- [x] Add the managed-code-only `Copy trace` command and reuse the reproduction overlay in read-only
  trace mode. Hide Load, preselect text before insecure-context clipboard access and return focus to
  the trace command; flat/non-code workspaces keep it disabled.
- [x] Pass six focused trace-bound tests, forced 9-ULP mismatch and full parity-rejection tests, the
  real three-release Typed Panel causal trace assertions, full demo-web 316/316, warnings-denied
  demo-web Clippy, formatting, locked WASM check, release Trunk build, browser smoke and unchanged
  golden `--check`.

## Files and API surface

- `crates/geosolve-demo-web/src/workbench/code_projects.rs` adds the private closed ownership enum,
  authenticated code-project route and focused owner regressions.
- `crates/geosolve-demo-web/src/workbench/mod.rs` asks that route before generic Inspector mutation,
  installs complete returned authority on acceptance, retains current authority on failure and owns
  the thin adapter regression.
- `crates/geosolve-constraint-editor/src/feature_authoring.rs` exposes its existing private
  line/polyline span-endpoint query to the parent module.
- `crates/geosolve-constraint-editor/src/lib.rs` separates the compact grip from the broad Fillet
  radius surface, expands the private parent-endpoint resolver, applies the shared Select-only
  specificity order and carries one request-local lazy Coincident-representative cell. Its focused
  unit regression proves broad arc-only motion initializes no document-wide topology state.
- `crates/geosolve-constraint-editor/tests/m75_hover_pointer_parity.rs` carries seven named
  F002 rows covering shared identity, both remote parent endpoints, Coincident equivalence,
  coordinate-only overlap, nearer endpoint selection, cross-Fillet arbitration and an applied
  persistent Fillet. The fixtures also prove compact-grip precedence and disconnected-tie fallback.
- `crates/geosolve-demo-web/src/workbench/code_projects.rs` adds the F003 typed terminal regression,
  set-valued document normalization evidence, causal public source-curve filtering, bounded public
  computed-edge/fragment comparison and exact discrete-state guards.
- `crates/geosolve-demo-web/src/workbench/interaction_trace.rs` adds the bounded memory-only trace;
  `workbench/mod.rs`, `index.html` and `styles.css` add its managed gesture checkpoints and read-only
  copy surface.
- No public crate API, persistence/wire version, code grammar, Intent schema, solver API, residual or
  Jacobian changes.

## Commands and focused evidence

The following focused commands genuinely pass on committed M86 product source `9050424`:

```bash
cargo test --locked -p geosolve-demo-web --lib \
  workbench::code_projects::tests::m86_f001_manifold_dimension_inspector_rewrites_managed_target_atomically \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::code_projects::tests::m86_f001_direct_diameter_inspector_rewrites_managed_target_and_exact_undo \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::m86_f001_projectional_browser_inspector_routes_managed_dimension_to_source \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::code_projects::tests::braced_frame_gui_reference_dimension_survives_overlay_drag_and_history \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::projectional_browser_inspector_commits_accepted_edits_once_and_undo_redo_restores_them \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib --no-run
cargo fmt --all -- --check
git diff --check
cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
```

The exact manifold owner test covers accepted curve length `16 -> 8`, independently validated
finite geometry, one outer row, exact Undo, executed Redo and retained-invalid diameter Undo. The
small accepted-diameter fixture separately covers `5 -> 8`. The adapter and two existing collateral
tests prove browser routing, authority replacement, GUI-owned fallback and generic history remain
coherent. The complete qualification and freeze evidence below supersede the earlier focused-only
checkpoint; Pages publication is intentionally not claimed before human approval.

The following expanded F002 and affected-crate commands genuinely passed on the saved gate-time
served-build worktree identity (`feafcc2a…`):

```bash
cargo test --locked -p geosolve-constraint-editor --test m75_hover_pointer_parity -- --nocapture
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor --test m75_hover_pointer_parity \
   --target wasm32-unknown-unknown'
cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings
cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings
```

The parity target passes 18/18 on native and WASM.
Focused demo-web owner and guard tests also
pass for the three Typed Panel targets, causal source scoping, the 8-ULP boundary, matching nonempty
design/accepted alias sets, exact public Fillet sweep/tangent/winding/provenance, the existing
rectangle scalar contract and Compass Rose terminal collateral. The unchanged 271-row golden clean
check passes. The fresh complete demo-web run passes 307/307 library tests plus its binary,
integration and doc-test targets.

The complete provisional dirty-worktree gate and no-rebuild served-byte verification now supersede
the earlier focused-only checkpoint. This evidence is still not clean-source nomination evidence.

## Historical qualification and provisional replacement UAT

### Historical F001 qualification and nomination

Exact product source `90504245e19858f986d5f506f6e42d237e9665b5`, tree
`65e092540dab82618d1129229b566a2e791aa40c`, was clean when the complete Nix gate ran from
2026-08-28 12:04:44 through 12:24:38 AEST. The gate exits `0`; its 6,534-line, 436,494-byte log is
`/tmp/geosolve-m86-nix-gate.CSQkgk/release-gate.log`, SHA-256
`4b81a1d12df503ef220b10645f3f4266891edc1eeaf87f3647071e2a77c33584`. It passes warnings-
denied workspace Clippy, all-feature workspace tests and doc tests, the unchanged 271-case golden,
native/WASM parity, both TypeScript packages, warnings-denied Rustdoc, benchmark compilation,
release performance sentinels, cargo-deny licences, package verification and final Trunk release
assembly.

An earlier direct-PATH clean-gate attempt passed native workspace tests and the golden check, then
stopped with status `101` before executing its first WASM test because
`wasm-bindgen-test-runner` was absent from that shell's PATH. This is recorded as a harness error,
not product evidence, at `/tmp/geosolve-m86-gate.sMkCi9/release-gate.log` (SHA-256
`13c924c7286e248269802145d8f175dae838dec9baa7d7e5e30bf0f2d6382195`). The complete clean Nix
gate above supplies the actual WASM and release authority.

Without rebuilding, that gate's seven-file `dist` was copied and frozen read-only at
`/tmp/geosolve-m86-uat.vdEFAxsF`. It contains exactly seven regular files, zero symlinks or other
non-regular entries at directory/file modes `0555`/`0444`; source, copied, frozen and post-serve
manifests are identical. The ordered-manifest aggregate is
`1f872c6b51317ff810b48ab8965e1e0a0f6cb45feb01f5cbfe654eafbedd5882`; complete evidence is
`/tmp/geosolve-m86-freeze-evidence.6fU7WpCl`.

Local service PID `3879694`, invocation `c00b3e911ce54733be0b4b6a47756de1`, served only that
snapshot at `http://127.0.0.1:18101/`. Retained Tailscale service PID `3879933`, invocation
`e56fde25abab4481a9bd7c825d4279b6`, served the same snapshot at
`http://100.94.63.83:8080/`. Both historical eight-path HTTP ledgers are byte-identical at SHA-256
`b5ad691e14198791fa801281724fc3186161b2d2aae7358affea402e9a5f0acd`; every path returns 200,
zero redirects, exact MIME/length/body, no `Location` or `Content-Encoding`, and `/` equals
`index.html`. Those services were replaced only after temporary verification of the F002 bytes.

### Withdrawn pre-expansion F002 qualification and nomination

Historical source `dbe94daf152515169b78a310cf2286f9ea04c80b`, tree
`77f86c0a198af12e10537dc4d6d7d90066ba48e8`, was clean when the complete Nix release gate ran from
2026-08-28 13:46:55 through 14:20:45 AEST. The gate exits `0`; its 6,570-line, 440,856-byte log is
`/tmp/geosolve-m86-f002-gate.wSztT0Bb/release-gate.log`, SHA-256
`34ac3e398953398495d22d480d9a11d88b03020c0235c07db61c443534fa4278`. It passes warnings-denied
workspace Clippy, all-feature workspace tests/doc tests, unchanged 271-case golden, every native/
WASM parity target including all then-current 15 M75 rows, both TypeScript packages, warnings-denied Rustdoc,
benchmark compilation, release performance sentinels, cargo-deny licences, package verification
and final Trunk release assembly.

Without rebuilding, the gate's exact seven-file `dist` was copied and frozen at
`/tmp/geosolve-m86-f002-uat.CPfe9QD8`. It contains seven regular files, zero symlinks or other
non-regular entries at directory/file modes `0555`/`0444`; source, copied, frozen, post-serve and
source-after manifests are identical. Its ordered-manifest aggregate is
`e3f9581a05a8cbf5731b33625fa63f2b35e62f4ebcfdacb6d75a4486f80fc850`; complete evidence is
`/tmp/geosolve-m86-f002-freeze-evidence.mSh9iwrm`.

The frozen bytes first passed all eight paths on temporary local port `18102`. Local service PID
`597410`, invocation `7cf7cbbae81a48ee8f492e1dab6e592d`, then replaced the prior candidate at
`http://127.0.0.1:18101/`; retained Tailscale service PID `597412`, invocation
`1a777ee174764f3cbb35f7b4863e5e95`, served the same snapshot at
`http://100.94.63.83:8080/`. Temporary-local, final-local and Tailscale eight-path ledgers were
identical at SHA-256 `e5513ab3e36262f2ccedf175006f1283d5504180c8d0be46e1e90dded999a3df`.
Every path returned 200 with zero redirects, exact MIME/length/body, no `Location` or
`Content-Encoding`, and `/` equalled `index.html`. Expanded M86-F002 withdraws those bytes from
current UAT; the snapshot and ledgers remain immutable historical defect evidence. No current
service identity or replacement nomination is inferred from those historical PIDs after reboot.

### Historical provisional combined F002/F003 UAT candidate

The served build identity is the saved pre-gate 160,117-byte, 2,976-line binary patch over HEAD
`4730e156e17cf3df88b9681a22961d41b686c2ff`, tree
`23a76c3b7141f10064d899113b97135932d23033`. Its patch has SHA-256
`feafcc2a717a9c1bf9ff7a708b705903b2e18c6ef67327f533d66819a8784a57`; the complete status has
SHA-256 `945ef3534016a5735c42c6fedaf72e66be2acc41ce9dc764db6e5896c3636b5a` and no untracked files.
Saved pre/post-gate patch and status files are byte-identical. Subsequent documentation-only
worktree edits are outside that served-build patch and do not alter the frozen seven-file candidate.

`env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from 18:44:26 through 19:10:14 AEST on 2026-08-28. Both pipeline statuses are `0`. The
6,582-line, 441,920-byte log
`/tmp/geosolve-m86-f002-f003-gate.yDrJlI8n/release-gate.log` has SHA-256
`93b645c2a2f1850f589b406943f3618da4fc833a42ff6066884602ec3e31ddb6`. Pre/post status and binary
patch files compare exactly. The gate passes warnings-denied workspace Clippy; all-feature native
tests/doc tests; unchanged 271-row golden; native/WASM parity including F002 18/18; demo-web
307/307; both TypeScript packages; Rustdoc; benchmark compilation; release-performance sentinels;
cargo-deny licences; package verification; and final Trunk 0.21.14 assembly. Because the source is
dirty, this is provisional UAT evidence and never a clean nomination.

Without rebuilding, the gate's seven-file `dist` was copied to
`/tmp/geosolve-m86-f002-f003-uat.yGY3Nvly` and frozen with directory/files `0555`/`0444`. It has
exactly seven top-level regular files, zero symlinks, nested or other entries; source, copied,
frozen and every post-serve manifest are identical. The ordered-manifest aggregate is
`8f5a4ffcd96819b986ba81a9467d0c83a64365b2d21338cd134e164fa4444ce4`; complete evidence is
`/tmp/geosolve-m86-f002-f003-freeze-evidence.EsMzxE2v`.

The frozen bytes first passed all eight paths on temporary `127.0.0.1:18102`, PID/invocation
`965128`/`a06c89f580744568b0d39677ee776da1`, while both historical listeners remained live. Only then
did local PID/invocation
`969297`/`c1681e5beb234ce487dbf9b639cbd9dd` replace `127.0.0.1:18101`, followed by Tailscale
PID/invocation `973390`/`c77a5b3abbe94752b864af9bda53c355` at
`100.94.63.83:8080`. Temporary/local/Tailscale eight-path ledgers are byte-identical at SHA-256
`dca3e6eeba66e12c873ba4b5ba9b6cadd489060f0e4c7d5ce1070ed3344ec96f`; every path returns 200,
zero redirects, exact MIME/length/body, no `Location` or `Content-Encoding`, and `/` equals
`index.html`. The temporary service is stopped; the two combined-candidate services subsequently
served only that frozen snapshot and were later replaced by the trace-enabled services below. The
withdrawn snapshot remains intact as rollback evidence.

### Accepted trace-enabled UAT descendant

After the snap-back remained observable in human UAT and the complete Copy repro payload became
impractical to paste, the supervising user requested exact gesture logging. The bounded I7 trace
was added without changing F003's terminal-parity semantics. Focused trace tests, the real
three-terminal regression with causal stage assertions, full demo-web 316/316, warnings-denied
Clippy, formatting, locked WASM check, release Trunk build, browser smoke and unchanged golden
`--check` pass.

The trace-enabled release output is frozen read-only at
`/tmp/geosolve-m86-trace-uat.U1C0QPSf`. It contains exactly seven regular files, zero symlinks or
other entries, with directory/files `0555`/`0444` and ordered-manifest aggregate
`f5f429f70e42e3b39a8f22696c19ff81f358cfb10c43f7910baf386c9d82fd44`. `index.html`, JavaScript
and WASM SHA-256 values are
`b5a36925ee1edd8af7dbbe9d4127b129184be131f84414af8e4ceac128e2111c`,
`3bb6b395a6f053e5172063474a974dbd98a10163b45963bb736381ead6a02837` and
`1626b3a9163f265dcaf7db0f2f0260930c9fe0ac3b9695ac749ed559e5fc435b`.

Local PID/invocation `2433761`/`3f829abfff0a46eba586c07fed507d8e` serves `127.0.0.1:18101`;
Tailscale PID/invocation `2433763`/`5f2c7c3eddac43119f380ec2e87b47c8` serves
`100.94.63.83:8080`. Both report the frozen snapshot as their working directory. `/` plus every
file on both endpoints match it byte-for-byte; evidence
`/tmp/geosolve-m86-trace-http-verify.fXS06h` has results SHA-256
`b2de59e63fc30a2dcbef108e671b1038103983bb95fea53d7410bb5799d080f3`. The temporary diagnostic
listener on `18103` is retired. The supervising user's 2026-08-29 close decision accepts this
trace-enabled descendant and M86-U1-U8 without inventing a separate row-by-row replay.

### Final clean committed-source nomination

Accepted source `88d1b5e06a7ce8ffe38931f792492f6f837a1d74`, tree
`09018e5aeb7e824396ae2ee2c70a3e30912414fa`, passes the complete clean Nix release gate from
13:16:02 to 13:35:50 AEST with pipeline statuses `0 0` and identical empty pre/post worktree
status. The 6,573-line, 438,432-byte log
`/tmp/geosolve-m86-clean-gate.w0UKa8fu/release-gate.log` has SHA-256
`e3adef1b33f1b840d9bc44ea7e30a5c76248766187d705eb1bdeb682cfc3bad0`. It passes workspace
Clippy/tests/doc tests, unchanged 271-row golden, native/WASM parity including F002 18/18,
demo-web 316/316, both TypeScript packages, Rustdoc, licences, package verification, benchmarks,
release-performance sentinels and final Trunk assembly.

Without rebuilding, the exact seven-file output is frozen at
`/tmp/geosolve-m86-clean-uat.d7DF9hcM`, directory/files `0555`/`0444`, zero symlinks/nested entries
and ordered-manifest aggregate
`d9d88bfb8ac4acd3f8d45f2cbbc965694297d76b61192be12c4fe65b9deb557e`. The JS/WASM/index/CSS
SHA-256 values are respectively
`1fb90f90f647d7313f0a8f01cef1bd1d71f81594a26526a4266a3f9e031895f9`,
`5816ea743cfeeb696ff7ee0656994a7742f02d55aacdc8d3939230ce7ecc26a7`,
`f4f59579d86518d51b2cc3f6ac43ef2e177d94f297792c081e6f3fb252ae38c2` and
`92059496edc2cc939c436b3361d13729e314af15a94e2e677fd8a559a3e281d1`.

Isolated temporary HTTP PID/invocation `3502269`/`c3a6eeec322d454ab97b246fbd67e8e1` at
`127.0.0.1:18104` exact-verifies `/` and all seven files; results SHA-256 is
`cda649921ad08469b50642f23afda334c3fff821848a8365718e0713ca9f909b`, with complete evidence at
`/tmp/geosolve-m86-clean-freeze-evidence.MDl33z2L`. It is retired, inactive/dead and `MainPID=0`;
curl exits `7` with HTTP `000`. The accepted UAT services are deliberately retained until the
separately rebuilt Pages artifact and hosted paths pass exact verification.

## Semantic-preservation ledger

F001 remains an optional code-authoring transaction adapter. F002 remains a private headless
Select-priority rule over already accepted scene semantics. F003 remains a retained-terminal parity
correction after independent acceptance: its bounded scalar comparison activates only from the same
nonempty redundant-alias normalization set in both design and accepted documents and only for public
computed edges/fragments causally sourced by curves referencing those points. Unrelated computed
geometry and every compared public discrete semantic field remain exact. Revision/digest and
evaluation lifecycle stamps are intentionally refreshed/excluded. Private continuation certificates
remain outside this public DTO comparison and are not implicated by Typed Panel's Current Fillets.

None of F001-F003 changes a solver equation, dimension residual, Jacobian, hard/soft policy,
computed branch state, persistence format or ordinary GUI Inspector behavior. F001 success still
requires ordinary Intent materialization, native solve and independent residual validation; F002
uses persistent parent endpoints and active Coincident incidence without treating solved coordinate
proximity as topology; F003 cannot turn a non-finite, out-of-cell or discrete mismatch into parity.
