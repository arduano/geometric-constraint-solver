<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M79 implementation — stable inference cycling and recovery

Status: **implementation and pre-nomination qualification complete; clean release qualification,
immutable Tailscale nomination and human UAT pending**.

## Ownership and interfaces

`geosolve-constraint-editor` owns the exact stationary candidate cohort, candidate ordering,
explicit-choice authentication, guide selection and retained mixed-candidate publication.
`geosolve-demo-web` owns only physical keyboard/pointer translation, animation-frame coalescing and
browser-context invalidation.

The existing `DraftInferenceInput::preferred_candidate` and
`DraftInferenceStatus::StalePreferredCandidate` wire remain source-compatible. The only additive
public convenience is:

```rust
DraftInferenceResolution::next_cycle_candidate_id()
    -> Option<DraftInferenceCandidateId>
```

It follows the already-published deterministic candidate order, wraps a resolved/ambiguous cohort
with at least two entries and returns `None` for None, Suppressed, ResourceLimited,
StalePreferredCandidate or a singleton.

No canonical document or workspace schema changes.

## Headless candidate lifecycle

The inference engine retains one bounded internal cohort keyed to the exact normalized
`DraftInferenceFrame`. An unpreferred sample performs ordinary bounded generation, ID allocation,
ranking and automatic hysteresis, then seals its candidates and non-candidate guides. A preferred
sample is accepted only when its complete frame and candidate ID match that seal. Selection is
rebuilt from the sealed candidate and guides without regenerating candidates or updating automatic
latches/reference memory.

Suppression and normal stage/session invalidation clear the seal. A foreign frame/ID clears stage
state and returns fail-closed stale output with no cycleable replacement. Invalid/non-finite work
remains transactional through the engine's existing clone-before-publication transition.

## Browser adapter lifecycle

The pointer queue binds a stationary candidate to pointer identity, exact screen position and
semantic modifiers. Ordinary observation retires a mismatched choice before constructing the next
headless input. Modifier replay clears the choice and forwards one unpreferred sample only when
geometry drafting owns the stationary pointer. Blur, pointer leave/cancel, tool/stage/history,
camera, scene and overlay ownership transitions use the same retirement path.

Tab first drains the pending animation-frame sample through the ordinary headless move path, then
asks the published resolution for its next ID and immediately resolves that stationary choice.
Non-owner contexts cannot store a choice. Pointer-down may forward an exact choice once and clears
it before any success or failure result can be retried. A hover-only stale result clears and
refreshes once without a preference; stale pointer-down remains noncommittable.

## Mixed positional/directional publication

The private pending construction records only the auto-direction relation indexes whose original
candidate also carried a stronger positional anchor (or complete two-axis intersection). The
public plan shape remains unchanged.

The authenticated effect path trials the original plan. A retry is eligible only when accepted-
state redundancy evidence reports every problematic auto source as fully redundant and every such
source maps to one recorded direction index. It filters those definitions, starts again from the
original retained session, and runs the normal independent acceptance and redundancy validation.
The effective plan is recorded for replay; publication marks the original authenticated pending
plan before positive acknowledgement can consume the draft. Any partial/positional/unrecognized
redundancy or retry failure publishes nothing and preserves the original typed error.

## Findings and evidence

### M79-F001 — stationary candidate cohort churn

Owner: `DraftInferenceEngine`. Explicit preference previously updated the same automatic latches
that filter the next candidate generation. Candidate A could therefore remove candidate B that had
just been advertised at the identical frame. The sealed-cohort path separates explicit cycling
from automatic hover memory.

### M79-F002 — browser preference poisoning

Owner: demo pointer adapter. One bare candidate ID survived pointer movement, modifier replay,
blur and non-drafting ownership, while headless invalidation had already retired its ID map. Exact
choice context plus centralized retirement removes that cross-frame authority.

### M79-F003 — queued-move/Tab race

Owner: demo event ordering. Tab previously read the old coordinator resolution, cancelled the
scheduled frame and combined that ID with the queue's newer input. Draining the latest sample
before cycling makes coordinate and cohort one ordered transition.

### M79-F004 — unpublishable ranked mixed bundle

Owner: authenticated retained construction publication. The centre-rectangle/right-edge-midpoint
case produces useful Midpoint intent plus a Horizontal relation already implied by the accepted
graph. Exact redundancy-guided direction pruning retains the stronger associative anchor without
weakening generic rejection or inventing a priority weight.

## Qualification record

The implementation was developed from source
`077b428effb18958928531cd27c284b513f845fa`. The retained independent reproduction log
`/tmp/m79_exact_repro.log` has SHA-256
`0a898a60b62a229d5ddfa1917c8b9bef3151b3ede18e107d76dd0f9e95d1fdf2` and preserves the exact
five-candidate cohort reported in `docs/M79_GOALS.md`.

Pre-nomination qualification passes:

```text
cargo test --locked -p geosolve-constraint-editor --lib
  # 364 passed
cargo test --locked -p geosolve-constraint-editor --test m79_inference_lifecycle
  # 1 passed natively
nix-shell shell.nix --run 'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m79_inference_lifecycle --target wasm32-unknown-unknown'
  # 1 passed in the WASM harness
cargo test --locked -p geosolve-demo-web --lib
  # 144 passed
cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity --test m71_transition_parity --test m71_f003_midpoint_axis --test m71_f004_axis_bundle --test m71_f005_cross_axis --test m73_f004_span_axis_precedence
  # 11 passed
cargo test --locked -p geosolve-constraint-editor --test golden_authoring_oracle golden_oracle_inventory_and_tsv_schema_are_exhaustive -- --exact
  # 1 passed
./scripts/golden-authoring-scene-oracle.sh --survey
  # 270/270 PASS
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
  # exact reviewed match; no non-pass dispositions
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
cargo doc --locked --workspace --all-features --no-deps
  # all passed
```

The one reviewed M70 transition-fixture change removes the candidate and guides formerly exposed
by a stale preference; all other rows are byte-identical. No broad golden dimension was added
because M79 changes transient candidate/coordinator lifecycle rather than the durable authoring-
family matrix. The only diagnostics are the longstanding non-failing Cargo notices for packages
declaring both `license` and `license-file`.

The clean release log, exact nominated commit/tree, no-rebuild frozen manifest and served-byte
evidence will be appended after committing this implementation and running the release gate from a
clean tree. Until then this file does not claim an accepted release candidate.
