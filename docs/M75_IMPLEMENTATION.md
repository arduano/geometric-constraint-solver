<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M75 implementation plan — Select hover and primary pointer-owner parity

Status: **implemented and prequalified as of 2026-08-16; clean candidate qualification, immutable
Tailscale nomination and human UAT remain pending**. This ledger records the implemented ownership,
compatibility boundary and evidence without claiming milestone acceptance or public publication.

Architecture decision: no new ADR is currently required. M75 consolidates existing editor-owned
picking, annotation visibility and hover presentation within the accepted-scene boundary. It adds
no equation, persistence language or browser-owned semantic policy.

## 1. Owning boundaries

### Headless constraint editor

The presentation-independent editor owns one private Select candidate collector and resolver.
Both pointer-move prediction and primary pointer-down call it with the same accepted scene,
viewport, visibility, current problem set, active tool and finite pointer sample. Existing semantic
selection items remain the result identities.

The only additive public surface is problem-aware pointer-move wrappers over the existing
scene/selection DTOs. Existing pointer-leave, cancellation and retained-state paths revoke
host-side camera, scene or input-ownership remaps. Candidate enumeration, comparison and
precedence remain private; M75 does not publish a general-purpose hit-test framework or new public
ownership type hierarchy.

### Demo web adapter

The web workbench continues to normalize browser coordinates and forward tool/camera/overlay
transitions. It renders only the hover target, related operands and context supplied by the
headless result. Browser DOM/SVG targets, paint order, CSS `:hover` and local geometry checks will
not add or retain semantic hover state.

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

## 5. Focused regression plan

### Editor owner tests

- Freeze every adjacent precedence edge and both candidate insertion orders.
- Cover ordinary points and semantic centers separately.
- Cover ordinary-visible and problem-forced constraint/dimension occurrences.
- Freeze annotation distance/item/occurrence comparison, including exact ties and repeated scene
  construction.
- Separate context-only corridor reveal from primary target selection.
- Exercise tool, camera, scene, visibility/problem and overlay-ownership invalidation.
- Assert pointer-move is mutation-free and immediate pointer-down consumes the same owner.

Run the semantic cases natively and with `wasm-bindgen-test-runner`. Exact hit-envelope boundary
tests will reuse existing tolerances rather than update them.

### Thin web tests

- Verify browser coordinates and current problems reach the headless wrapper unchanged.
- Verify the workbench paints only the returned target/context and clears it synchronously on each
  invalidation trigger.
- Verify overlay/focus and uncaptured letterbox routes jointly revoke the pending animation-frame
  sample, stationary pointer input and current headless context, while captured edge crossings are
  preserved.
- Verify uncaptured Fillet feature-authoring movement cannot enter Select hover resolution while a
  captured Fillet-radius gesture still can.
- Verify a DOM/SVG target or CSS hover cannot manufacture a semantic canvas owner.
- Preserve existing keyboard focus, accessible names and non-colour focus/selection cues while
  overlay ownership suppresses canvas hover.

### Compatibility checks

- Keep the authoring/scene golden byte-identical; no new row is expected.
- Re-run existing point/curve/annotation/Fillet/datum picking, M68 radius ownership, M69 role
  ordering, M72 overlay and M74 datum/reference regressions.
- Independently compare accepted document/geometry, history, rank/DOF, branches, problem set and
  persistence bytes before and after hover-only sequences.

## 6. Qualification ledger

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

The exact complete clean release gate and `--require-clean` qualification remain pending until the
implementation/prequalification commit is clean. No immutable distribution or human result is
claimed by these provisional results.

## 7. Completion gates

- Implement the shared resolver, problem-aware pointer-move wrappers, invalidation and browser
  translation without changing the frozen semantics.
- Pass focused native/WASM/web and proportional compatibility qualification with unchanged golden
  bytes.
- Pass the complete clean release gate and freeze its exact output as a read-only, byte-verified
  Tailscale candidate kept live through follow-up UAT.
- Complete `docs/M75_UAT.md`, including every deferred M74-U1 through M74-U8 item, the new ownership
  matrix, two desktop sizes, zoom/tolerance fringes and accessibility.
- Receive explicit supervising-human approval, deploy the exact accepted source through GitHub
  Pages, verify every hosted byte/media type and close M75.

## 8. Compatibility and limitations

M75 is an additive pre-1.0 interaction correction. Public API growth is limited to problem-aware
pointer-move wrappers over existing DTOs. It does not activate sketch v5, change canonical v1-v4,
retune hit or drafting-inference tolerances, change annotation placement, support mobile/tablet or
add hover semantics to non-Select authoring tools.
