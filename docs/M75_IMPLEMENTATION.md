<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M75 implementation — hover and primary pointer-owner parity

Status: **complete (2026-08-16); the post-F002 replacement is accepted for scoped closure and
exact-verified on GitHub Pages**. The initial candidate and
clean-qualified M75-F001 replacement remain historical evidence only. The caller accepted the
current candidate, focused F001/F002 hover recheck and U1-U12 without claiming a separately logged
step-by-step replay. Exact publication evidence is recorded below. M76 subsequently reached scoped
approval and final qualification; its publication closeout supersedes M75 as current authority
without changing any M75 evidence.

Architecture decision: no new ADR is currently required. M75 consolidates existing editor-owned
picking, annotation visibility and hover presentation within the accepted-scene boundary. It adds
no equation, persistence language or browser-owned semantic policy.

## 1. Owning boundaries

### Headless constraint editor

The presentation-independent editor owns one private Select candidate collector and resolver.
Both pointer-move prediction and primary pointer-down call it with the same accepted scene,
viewport, visibility, current problem set, active tool and finite pointer sample. Existing semantic
selection items remain the result identities.

The additive public surface is limited to problem-aware Select and domain-aware authoring
pointer-move wrappers over existing scene, selection, authoring-state and normalized-input DTOs.
Existing pointer-leave, cancellation and retained-state paths revoke host-side camera, scene or
input-ownership remaps. Candidate enumeration, comparison and precedence remain private; M75 does
not publish a general-purpose hit-test framework or new public ownership type hierarchy.

### Demo web adapter

The web workbench continues to normalize browser coordinates and forward tool/camera/overlay
transitions. It renders only the hover target, related operands and context supplied by the
headless result. During uncaptured computed-Fillet authoring, one move/down translation helper may
enumerate the complete `elementsFromPoint` paint stack and retain the exact `FeatureCorner` already
identified by a headless `SceneFilletHit::Radius`; otherwise it retains the top painted item. That
item remains only an intent hint: the coordinator independently reauthenticates retained preview
ownership, scene provenance, geometry policy and proximity. Browser DOM/SVG targets, paint order,
CSS `:hover` and local geometry checks cannot add or retain semantic hover state.
The pointer-active grip, rail and spoke inherit the same `FeatureCorner` identity from their
radius-affordance group, so every visible headless radius surface can participate in translation.

### Unchanged domain owners

`geosolve-sketch`, `geosolve-core`, computed-feature evaluation and persistence remain unchanged.
No residual, Jacobian, branch, solve, rank/DOF, constraint/dimension, history or schema behavior is
part of the implementation cut.

## 2. Shared Select resolution

For one authenticated input, the resolver chooses the first applicable class below:

| Priority | Candidate class | Existing owner returned |
| ---: | --- | --- |
| 1 | Current applicable Fillet radius surface or grip | Existing feature-corner/radius action owner |
| 2 | Draggable stored point or semantic center | Existing point/center geometry owner |
| 3 | Visible annotation occurrence | Existing constraint or dimension item plus occurrence |
| 4 | Other native or computed geometry | Existing curve/feature selection item |
| 5 | Visible intrinsic datum | Existing `SketchDatum` selection item |
| 6 | No applicable hit | `None` |

Applicability, visibility and provenance checks remain those of the existing owner. Within a class,
all established hit tolerances and native/computed/Profile/Construction ordering remain unchanged.
Selection modifiers are applied only after primary resolution and therefore cannot reorder hits.

Pointer-move is prediction only: it must not mutate selection, accepted geometry, history,
authoring, Fillet state, problem state or scene authority. Given the same inputs, an immediately
following pointer-down must consume the predicted primary owner.

## 3. Annotation and context rules

A current problem may force an otherwise contextual constraint or dimension occurrence visible.
That occurrence enters the same visible-annotation candidate list as every other painted
occurrence. When the problem or accepted scene changes, its visibility and hit eligibility change
together.

Multiple eligible annotation occurrences will compare:

1. finite screen-space distance;
2. stable semantic item identity;
3. stable occurrence identity.

The comparator will not depend on source insertion, map iteration or browser paint order.

Contextual corridors remain a separate output. They may reveal related annotations and operands
without creating a primary target. A context-only sample returns `None` for the target, and the
same pointer-down cannot select the revealed item unless another real hit class owns that sample.

## 4. Hover invalidation and presentation

The retained coordinator/web seam clears the current hover target and related context when any
of these owning inputs changes:

- active tool;
- selection or another annotation-visibility input;
- camera/viewport transform, including pan, zoom, Fit and Origin recentering;
- accepted scene or visibility/problem composition;
- overlay, dialog, input or other non-canvas pointer ownership.

Existing pointer-leave and cancellation paths will share the same revocation behavior. Returning
to the prior state cannot restore cached hover; a fresh valid canvas pointer sample is required.
The browser will rerender the cleared headless state immediately and will not preserve a DOM-only
highlight. Ordinary and feature authoring suppress uncaptured Select movement. A captured
Fillet-radius gesture remains an editor-owned exception until its matching terminal sample.
Uncaptured authoring movement is not discarded: the ordinary or grouped-Fillet owner publishes
only the compatible native item that its unchanged press would consume. A current painted
computed corner publishes its radius owner only after the same retained-preview/provenance checks
as pointer-down and the same exact editor hit resolution as the radius gesture. When native SVG
paint lies above that computed corner, the uncaptured Fillet-only adapter reconciliation supplies
the matching headless radius owner to both paths. With no matching headless radius owner it leaves
the top painted item as an untrusted hint rather than inventing browser-side precedence.

## 5. Focused regression plan

### Editor owner tests

- Freeze every adjacent precedence edge and both candidate insertion orders.
- Cover ordinary points and semantic centers separately.
- Cover ordinary-visible and problem-forced constraint/dimension occurrences.
- Freeze annotation distance/item/occurrence comparison, including exact ties and repeated scene
  construction.
- Separate context-only corridor reveal from primary target selection.
- Exercise tool, camera, scene, visibility/problem and overlay-ownership invalidation.
- Cover ordinary and grouped-Fillet point/curve operands, wrong-kind overlap fallback, empty and
  inapplicable clearing, computed-radius ownership and stale painted-hint rejection.
- Assert pointer-move is mutation-free and immediate pointer-down consumes the same owner.

Run the semantic cases natively and with `wasm-bindgen-test-runner`. Exact hit-envelope boundary
tests will reuse existing tolerances rather than update them.

### Thin web tests

- Verify browser coordinates and current problems reach the headless wrapper unchanged.
- Verify the workbench paints only the returned target/context and clears it synchronously on each
  invalidation trigger.
- Verify a native point painted over the correct computed radius grip cannot hide the exact
  headless `SceneFilletHit::Radius` owner from either move or unchanged down, that grip/rail/spoke
  markup all extract the same `FeatureCorner`, and that a missing or foreign owner preserves the
  top paint item only as an untrusted hint.
- Verify overlay/focus and uncaptured letterbox routes jointly revoke the pending animation-frame
  sample, stationary pointer input and current headless context, while captured edge crossings are
  preserved.
- Verify uncaptured ordinary/Fillet authoring movement reaches its own resolver rather than Select,
  while a captured Fillet-radius gesture remains editor-owned.
- Verify RAF coalescing keeps the latest painted intent hint paired with the latest pointer sample;
  the hint alone cannot authenticate a computed owner.
- Verify a DOM/SVG target or CSS hover cannot manufacture a semantic canvas owner.
- Preserve existing keyboard focus, accessible names and non-colour focus/selection cues while
  overlay ownership suppresses canvas hover.

### Compatibility checks

- Keep the authoring/scene golden byte-identical; no new row is expected.
- Re-run existing point/curve/annotation/Fillet/datum picking, M68 radius ownership, M69 role
  ordering, M72 overlay and M74 datum/reference regressions.
- Independently compare accepted document/geometry, history, rank/DOF, branches, problem set and
  persistence bytes before and after hover-only sequences.

## 6. Finding M75-F001 — authoring hover was suppressed

Reproduction against initial frozen candidate `f3affff1b62b1cb484a59647c4072c94c3b12ada` used the
ordinary editable `fillet-workshop` sample. After activating Fillet, hovering an applicable point
or curve left its class as `wb-point` or `wb-curve` and published zero `.geometry-hovered`
elements. Clicking curve `66000000000000000000000000000038` immediately made that same curve
`authoring-pending`. This is a confirmed presentation-independent interaction defect, not a CSS,
solver, tolerance, persistence or geometry failure.

Root cause: the adapter deliberately discarded all uncaptured ordinary and feature-authoring
pointer moves. Clicks separately called domain-aware candidate resolvers, so routing those moves
through Select would still have produced wrong overlap/applicability semantics.

Correction:

- `AuthoringState` and `FeatureAuthoringState` now resolve click and read-only hover through the
  same ordered candidate path, preserving warning and fallback precedence.
- The retained coordinator publishes authoring hover through the existing `EditorHoverState` and
  `EditorEffect::HoverChanged` contract. No selection, authoring, preview, history/transcript,
  gesture, accepted scene or geometry state changes.
- Computed preview-radius hover and pointer-down share candidate/preview/provenance validation and
  exact headless radius-hit resolution. A stale painted corner fails closed without native
  fallback.
- The browser routes uncaptured moves to Editor, ordinary authoring or feature authoring according
  to the current owner; captured movement remains Editor-owned. RAF samples coalesce pointer input
  and painted intent together.

Focused correction evidence on the implementation tree:

```text
cargo fmt --all -- --check                                      # pass
git diff --check                                                # pass
cargo test --locked -p geosolve-constraint-editor \
  --test m75_hover_pointer_parity                               # 11 passed
env NO_COLOR=true nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor \
   --test m75_hover_pointer_parity --target wasm32-unknown-unknown'
                                                               # same 11 passed
cargo test --locked -p geosolve-demo-web --lib                 # 116 passed
cargo clippy --locked -p geosolve-constraint-editor \
  -p geosolve-demo-web --all-targets --all-features \
  -- -D warnings                                               # pass
```

The initial immutable snapshot stays untouched as historical evidence. It is no longer a valid
human-UAT candidate; the clean-qualified F001 replacement below has also been withdrawn by F002,
and at that checkpoint GitHub Pages remained on accepted M74.

## 7. M75-F001 replacement qualification ledger (superseded by M75-F002)

Exact clean product source `57f407ada2eb8a16f8162d1db4126d5c5024f1b4`, tree
`7bff59c5d4d36d1acb687a93d78707b32e323d65`, was qualified with:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The complete run exited 0. M75 parity passes 11/11 natively and 11/11 under WASM, demo-web tests
pass 116/116, the reviewed golden remains unchanged at 270/270, and the sparse 256-moving-body
crossover completes in 143.27 seconds. All remaining formatting, diff, warnings-denied Clippy,
locked workspace, WASM, Rustdoc, benchmark, performance, licence/package and Trunk release-assembly
steps in the gate pass.

The gate-produced distribution was copied without rebuilding to
`/tmp/geosolve-m75-f001-uat.2Ju7gq`. The directory is mode `0555`; its seven regular non-symlink
files are mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 18,270 | `b2c503a0ca2ad33c0fcc137666a349a773630fb712a4cdd50f8fea64454614d0` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-41f4150de02af486.js` | 33,221 | `39eebb2d778b7470d0b2bd552ab7716cb12e38fe072bca05905e1f936fc81f09` |
| `geosolve-demo-web-41f4150de02af486_bg.wasm` | 6,117,357 | `cc194398055211d420a82b058fb83cf3d3e2e54bcded5c6c5116cca086be3d7d` |
| `index.html` | 27,478 | `fa50308533c8a98f2c8f37b63a72414ddba2f33d9a2f4339157779a7a2e875bc` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

Its C-locale ordered-manifest aggregate is
`9ecf1dde82ca777ae8de6dc380606512008b3bf088808e995fd0c4b2b8896967`. PID `4026985` served that
snapshot at `http://100.94.63.83:8080/` before being retired; its historical log is
`/tmp/geosolve-m75-f001-uat.2Ju7gq.server.log`. The exact server argv is:

```text
/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m75-f001-uat.2Ju7gq
```

Proxy/cache-bypassed identity requests verify `/` and all seven files with HTTP 200, exact media
types, lengths and bytes, no redirect or content encoding, `/ == index.html`, and a matching fetched
aggregate. Evidence is retained at `/tmp/geosolve-m75-f001-http-verify.kXc5g5`. The unchanged M72
and M74 Chromium scripts pass at `1440x900` and `1024x720`; their SHA-256 hashes are
`4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`.

M75-F002 below withdraws this otherwise clean candidate from human UAT. Its snapshot and served
bytes remain historical transport evidence only; at that checkpoint GitHub Pages remained
accepted M74 authority.

## 8. Finding M75-F002 — top paint target hid the computed radius owner

Exact reproduction uses the ordinary `fillet-workshop` scene. Collect point
`6600000000000000000000000000004f` and curve `66000000000000000000000000000038`, then hover the
computed-radius grip where a native point is painted above the correct `FeatureCorner`. The
headless scene resolves that computed corner as `SceneFilletHit::Radius`, but the top-target-only
adapter supplied native curve `66000000000000000000000000000052`. Hover therefore promised the
wrong owner; pressing without moving destroyed the valid preview and did not capture a radius
gesture. This is an adapter paint-stack defect, not a solver, feature, tolerance, scene-authority or
coordinator-authentication failure.

Independent adapter review reproduced a second surface of the same defect before commit: the
pointer-active radius rail and spoke had no `data-editor-item` owner, so stack extraction returned
no `FeatureCorner` even when the headless radius resolver accepted those visible surfaces.

Correction:

- Only while Fillet authoring owns uncaptured canvas input, the adapter enumerates the complete
  `Document::elementsFromPoint` stack inside the workbench viewport.
- One helper used by both pointer-move and pointer-down asks the headless scene for the exact
  `SceneFilletHit::Radius` owner and selects its matching painted `FeatureCorner` wherever it occurs
  in that stack.
- The shared radius-affordance SVG group carries the existing `FeatureCorner` identity, allowing
  grip, rail and spoke targets to extract the same owner without duplicating semantic policy.
- If there is no matching headless radius owner, the helper returns the top painted item. It does
  not promote a foreign computed item or create browser-side semantic precedence.
- The coordinator remains final authority and repeats candidate, preview, accepted/design/computed
  provenance, geometry-policy and exact-radius validation before hover or press can consume the
  hint. Captured movement stays editor-owned.
- Native presentation coverage freezes the radius-affordance owner attributes and all three
  pointer-active surfaces in addition to the exact paint-order reconciliation regression.

Focused correction evidence on the provisional implementation tree:

```text
cargo fmt --all -- --check                                      # pass
git diff --check                                                # pass
cargo test --locked -p geosolve-constraint-editor \
  --test m75_hover_pointer_parity                               # 11 passed
env NO_COLOR=true nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor \
   --test m75_hover_pointer_parity --target wasm32-unknown-unknown'
                                                               # same 11 passed
cargo test --locked -p geosolve-demo-web --lib                 # 117 passed
cargo clippy --locked -p geosolve-constraint-editor \
  -p geosolve-demo-web --all-targets --all-features \
  -- -D warnings                                               # pass
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown                              # pass
./scripts/golden-authoring-scene-oracle.sh --check              # unchanged 270 rows
```

Focused Chromium script `/tmp/m75_f001_browser_check.mjs`, SHA-256
`1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e`, passes 6/6 against the
provisional corrected local build. It covers Fillet point/curve hover and unchanged clicks,
ordinary relation/dimension compatibility and fallback, empty/inapplicable clearing, and the
computed-radius grip, visible spoke and rail hover/capture/release promise. This was provisional
presentation evidence; the clean nomination below repeats it over the frozen candidate.

### Post-F002 clean qualification and accepted candidate nomination

Exact clean product source `553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
`83df4efb99ca66cf0cebc0caec4515b61afd33cf`, was qualified with:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The complete gate exited 0 in 480.94 seconds. Demo-web passes 117/117, M75 parity passes 11/11
natively and 11/11 under WASM, the reviewed golden remains unchanged at 270/270, the sparse
256-moving-body crossover completes in 141.82 seconds, and Trunk release assembly passes. The
gate's remaining formatting, diff, warnings-denied Clippy, locked workspace, WASM, Rustdoc,
benchmark, performance and licence/package checks also pass.

The exact gate-produced distribution was copied without rebuilding to
`/tmp/geosolve-m75-f002-uat.hlSQYT`. The directory is mode `0555`; all seven entries are regular
non-symlink files at mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 18,564 | `b99a56b9c1aa8679538726c95b1ed29729174ff2945a44be1ea07b08d6f22cf2` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-35551745a5e20011.js` | 33,221 | `ade1f75e65ca2636f29259c7b3716d375e0b3886a6ba1bdf61817686b2dad2d2` |
| `geosolve-demo-web-35551745a5e20011_bg.wasm` | 6,117,030 | `9d01af2fee2d7ce3884020579187037eb617fe73ede243e491842ba044adf9dc` |
| `index.html` | 27,478 | `9bff14da5388601e8d48a175e65c033141f383736fcd9da4065350eb9baebf33` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

The C-locale ordered-manifest aggregate is
`eae64913c29d760f6eb64d7681212facca0c6d8869dee9631aeb9d77b059a139`. PID `37152` served only
this snapshot at `http://100.94.63.83:8080/`; log
`/tmp/geosolve-m75-f002-uat.hlSQYT.server.log` records the exact historical process. Its argv was:

```text
/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m75-f002-uat.hlSQYT
```

Old PID `4026985` was retired only after the new immutable snapshot and listener were verified. PID
`37152` was subsequently retired before M76 took the shared endpoint. The F001 snapshot remains
read-only and unserved. Proxy/cache-bypassed identity requests for `/` and all
seven files return HTTP 200 with exact media types, content lengths and bytes, no redirect or
content encoding; `/` equals `index.html` and the fetched aggregate matches. Evidence is retained
at `/tmp/geosolve-m75-f002-http-verify.1nRxtz`.

The unchanged M72 and M74 scripts pass over Tailscale at both `1440x900` and `1024x720`; their
SHA-256 values remain `4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44`
and `e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`. M75 script
`/tmp/m75_f001_browser_check.mjs`, SHA-256
`1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e`, passes 6/6 against these
exact frozen bytes, including native authoring and grip/spoke/rail hover/capture/release.

This snapshot was the current mechanical UAT authority at nomination. Mechanical qualification
alone disposed no human item. The supervising caller subsequently reported the candidate looking
good and authorized closure on 2026-08-16. That scoped approval accepts the exact candidate,
focused F001/F002 hover recheck and U1-U12; it does not claim that the detailed scorecard was
individually executed or logged. At that approval checkpoint, GitHub Pages still served accepted
M74, so publication and exact hosted-byte verification remained the sole open closeout step.

### Final GitHub Pages publication

Accepted product source `553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
`83df4efb99ca66cf0cebc0caec4515b61afd33cf`, is deployed from documentation-only approval
descendant `f80235978fbcdccd58c45a08bccf3969a20110c9`, tree
`eb05b6496aa5c761e005a40da78d8fb96e84c16a`. GitHub Pages workflow run `31939764951` passes in
25m59s. Qualify-and-assemble job `95147135584` passes in 25m40s, including the 24m24s complete
hosted release gate, native/WASM M75 11/11, demo-web 117/117, unchanged 270/270 golden,
70.90-second 256-moving-body sparse crossover and 26-second repository-prefixed build. Deploy job
`95149802628` passes in 11s; deployment `5929879555` reports success with HTTPS enforcement at
`https://arduano.github.io/geometric-constraint-solver/`.

Artifact `9261974799`, name `github-pages`, was downloaded to
`/tmp/geosolve-m75-pages-verify.NkQwem/github-pages.zip`. The ZIP is 2,108,111 bytes with SHA-256
`8c031953dec4975c9b701a5ba30f060a95d5e0772286396f3c03ac74fb665fc0`, matching GitHub's digest,
and contains only `artifact.tar`. The 6,277,120-byte inner tar has SHA-256
`8ac419fbea39c306e6ee529309f2d3965c93d4ff0459fd2e21179714e9b89c1d` and extracts to exactly
seven regular files with no links:

| Final hosted artifact file | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 19,087 | `6f9dbd39c3698b4ba8fbfd4e3a8d6006fc69f1078eeb90a6943df0468b46f4e9` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-3a692386adf1e085.js` | 33,221 | `ade1f75e65ca2636f29259c7b3716d375e0b3886a6ba1bdf61817686b2dad2d2` |
| `geosolve-demo-web-3a692386adf1e085_bg.wasm` | 6,116,943 | `ff5939f6e52483b3815a141371ccf3e929c089f192f72b6ed47d607e93de924a` |
| `index.html` | 27,618 | `80ba86454751c3d8a73e1dd1138a369fff0763b01c1ed70ad8acbe31880bb720` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

The C-locale manifest aggregate is
`4c2da7d7860ac0bcadc64722007b5accb01aa999aa79f3046ba9d2868e86ef3b`. The public root and all
seven paths return HTTP 200 with zero redirects, no content encoding and exact media types,
content lengths and bytes; `/` equals `index.html`. Application asset URLs use only the
`/geometric-constraint-solver/` prefix. Public M72 and M74 checks pass at both supported desktop
sizes, and M75 passes 6/6. GitHub Pages is final public-byte authority; the frozen Tailscale
snapshot remains accepted candidate evidence.

## 9. Initial qualification ledger (superseded by M75-F001)

Prequalification evidence completed on the dirty implementation tree:

```text
cargo fmt --all -- --check                                      # pass
git diff --check                                                # pass
cargo test --locked -p geosolve-constraint-editor \
  --test m75_hover_pointer_parity                               # 9 passed
env NO_COLOR=true nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor \
   --test m75_hover_pointer_parity --target wasm32-unknown-unknown'
                                                               # same 9 passed
cargo test --locked -p geosolve-demo-web --lib --all-features  # 116 passed
cargo clippy --locked --workspace --all-targets --all-features \
  -- -D warnings                                               # pass
cargo test --locked --workspace --all-features                 # pass
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown                              # pass
./scripts/golden-authoring-scene-oracle.sh --check              # unchanged 270 rows
env NO_COLOR=true nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && env -u NO_COLOR \
   trunk build --release --locked'                             # pass
```

Independent review then closed three concrete gaps before candidate nomination: portable WASM
coverage now includes exact annotation ties and the lifecycle matrix; selection changes revoke a
prediction evaluated under old annotation visibility; and the thin adapter has executable current
problem forwarding, queued-context revocation and Fillet feature-authoring ownership tests. The
focused post-review matrix passes 9/9 natively, 9/9 under WASM and 116/116 in the demo-web crate;
focused warnings-denied Clippy and the demo-web WASM check pass.

### Initial clean candidate qualification

Exact product source `f3affff1b62b1cb484a59647c4072c94c3b12ada`, tree
`7662abc8b7c71130f54fbf2745afa60f0d286431`, was clean-qualified with:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The final complete run exited 0. It includes formatting and diff checks, warnings-denied workspace
M70/M71/M74/M75 WASM suites, the demo-web WASM check, warnings-denied Rustdoc, benchmark
compilation, M14/M32 performance, licensing and package-content checks, the release 256-moving-body
sparse crossover in 138.09 seconds, and Trunk 0.21.14 release assembly. M75 parity passes 9/9
natively and 9/9 under WASM; editor unit tests pass 339/339 and demo-web tests pass 116/116.

Final M14 p95 measurements were 0.270/1.219/3.212 ms for small import/first solve/incremental edit
and 1.165/61.579/140.789 ms for the corresponding medium operations, all below their budgets. M32
p95 measurements were 0.192/0.479 ms for construction-offset load/edit, 0.187/0.358 ms for NURBS
load/knot insertion, 20.017 ms for all-family profile analysis and 15.799 ms for the NURBS
self-intersection profile; observed peak RSS was 10,488 KiB.

The first clean-gate attempt passed every step through the 140.70-second crossover and package
checks, then Trunk reported a transient `ENOENT` while copying its optimized WASM into `dist`. This
was a build-pipeline `HARNESS_ERROR`, not candidate evidence: the same isolated Trunk command
passed on the unchanged clean source, and the complete release gate above was rerun from the start
and passed before any artifact was frozen.

### Initial immutable Tailscale candidate

The final successful gate's exact seven files were copied without rebuilding to
`/tmp/geosolve-m75-uat.hUSaG7`. The directory is mode `0555`; every entry is a regular,
non-symlink file at mode `0444`.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 17,616 | `be4769bf0f57d1f27d7068e6e1e47a41305a320d08948fa306a38ca620db92b3` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-fc3fd24fd70a16aa.js` | 33,221 | `1e24182d7c61f3681b5fd62591a2f33b4ada6e3a1d3fd2fe884ad3484a2060bc` |
| `geosolve-demo-web-fc3fd24fd70a16aa_bg.wasm` | 6,109,194 | `76944eddca4ca6c95ad967c0b5b8dc215d292ca07515740fe3914588c1f4f70b` |
| `index.html` | 27,478 | `e00a829f0f954422fd9c5454110fd67d979b5fde42934ac230fbf34822c18430` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

The C-locale `sha256sum * | sha256sum` aggregate is
`69425a504453eda6645c96b6163b5b899ab455f40828f3cdecc73b90ff3c41d9`. Source `dist` was hashed
before and after the copy, every source/snapshot pair compared equal, and the read-only snapshot
aggregate was rechecked after its modes changed.

PID `3801058`, retained in command-runner session `47845`, served only this snapshot at
`http://100.94.63.83:8080/` with exact argv:

```text
/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m75-uat.hUSaG7
```

The exact old M74 PID `2599593` was retired only after the M75 snapshot was complete; historical
snapshot `/tmp/geosolve-m74-uat.jFfAm4` remains read-only and unserved. An initial detached M75
server launch was reaped with its launcher, so its first verifier failed closed with `curl` status
7 before transferring a file. The retained foreground server above was then started and every
verification was rerun in a fresh evidence directory; candidate bytes were unchanged. The first
retained process was subsequently reaped when its delegated command session ended before human
handoff. PID `3801058` then served the same revalidated read-only snapshot from the root session;
it has since exited after the F001 replacement occupied the endpoint.

Proxy/cache-bypassed, identity-encoded requests for `/` and all seven files return HTTP 200 with
exact media types, content lengths and bytes, zero redirects and no content encoding. `/` equals
`index.html`, and the fetched ordered aggregate matches the frozen aggregate. Successful HTTP
evidence is retained at `/tmp/geosolve-m75-http-verify.sQGN1B`.

The unchanged compatibility scripts pass directly over Tailscale at `1440x900` and `1024x720`:

```text
M72_BASE_URL=http://100.94.63.83:8080/ node /tmp/m72_full_browser_check.mjs
M74_BASE_URL=http://100.94.63.83:8080/ node /tmp/m74_browser_check.mjs
```

Their SHA-256 values are
`4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc` respectively. These are M72
and M74 regression smoke results, not synthetic M75 human UAT. At that checkpoint GitHub Pages
continued to serve the accepted M74 artifact and was not M75 authority during UAT.

## 10. Completion gates

- **Pass:** the Select and authoring shared resolvers, pointer-move wrappers, invalidation and
  browser translation are implemented without changing the frozen semantics.
- **Pass:** focused native/WASM/web and proportional compatibility qualification passes with
  unchanged golden bytes.
- **Pass:** the complete clean release gate passes and its exact output is the read-only,
  byte-verified Tailscale candidate kept live through follow-up UAT.
- **Pass for scoped closure:** U1-U12 and the focused F001/F002 hover recheck are accepted under the
  supervising caller's 2026-08-16 approval. This is not a claim that every detailed step, desktop
  size, zoom fringe or accessibility path was separately executed and logged.
- **Pass:** exact accepted product source `553fd912730b1de3b39736c49b669e94cabdd2c3`
  is deployed through GitHub Pages; every hosted byte/media type and the public browser contracts
  verify. M75 is complete.

## 11. Compatibility and limitations

M75 is an additive pre-1.0 interaction correction. Public API growth is limited to problem-aware
Select and domain-aware authoring pointer-move wrappers over existing DTOs. It does not activate
sketch v5, change canonical v1-v4, retune hit or drafting-inference tolerances, change annotation
placement, support mobile/tablet or add a second hover system to geometry drafting tools.
