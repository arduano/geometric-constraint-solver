<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M60 implementation report

Date: 2026-07-29

Status: complete

## 1. Files and APIs added

M60 keeps the public solver and companion APIs unchanged and completes their presentation in the
sole desktop workbench:

- `RestoreCheckpoint::{design_uses_draft_v5, accepted_uses_draft_v5}` expose the encoding of each
  checkpoint document without asking the web crate to inspect sketch JSON;
- the workbench workspace envelope advances to version 2 with explicit `canonical_v4` or
  `draft_v5` document payloads and deterministic legacy-v1 migration;
- `geosolve-demo-web` consumes the public `geosolve-sketch-ops` and
  `geosolve-sketch-topology` crates directly;
- the inspector gains a production-topology card with complete, skipped, truncated, cancelled,
  exhausted and unavailable presentation;
- the reusable right-expanding scenario catalog adds four stable M61 leaves under
  `m61-advanced-topology`; and
- deterministic scenario evidence now includes advanced-family, NURBS, companion-operation and
  production-topology summaries.

The ten existing M53/M55 stable scenario IDs and their subtrees are unchanged. Scenario state
remains ephemeral and suppresses ordinary workspace persistence until exit.

## 2. Mathematical and transaction behavior

M60 adds no residual equation, Jacobian, rank rule, branch heuristic or independent geometry
implementation.

Advanced scenarios are constructed through the existing public alpha scenario builders.
Periodic NURBS movement uses `DocumentEdit::TransitionNurbsContact`; refinement uses
`DocumentEdit::InsertNurbsKnot`. Split, exact mirror and linear pattern are prepared through
`SketchOperationSnapshot`, then applied to a cloned retained session through the operation
proposal's exact compare-and-swap transaction boundary. The resulting retained session becomes
the scenario coordinator only after ordinary sketch acceptance.

Production-topology presentation captures the current complete retained input, executes a public
controlled query and exposes a consumable profile only when
`TopologyCompleteness::Complete` carries `TopologyProductionProfile`. An added open eligible line
produces typed incomplete evidence and no consumable profile. A pre-cancelled query changes no
sketch input or accepted state. Recovery reconstructs the deterministic initial fixture rather
than reusing stale output.

Workspace-v2 serialization records the actual checkpoint encoding. Canonical v4 and draft-v5
documents are decoded only by their owning public sketch codecs. Draft-v5 multi-interval state
therefore survives desktop save/reload without pretending it is frozen canonical v4.

## 3. Commands run and outcomes

Focused qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all && cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings && cargo test --locked -p geosolve-constraint-editor && cargo test --locked -p geosolve-demo-web && cargo test --locked -p geosolve-sketch-ops --all-features && cargo test --locked -p geosolve-sketch-topology --all-features'
```

Outcome: pass. Editor tests pass 60 unit plus 7 M55 integration cases; demo-web passes 40 direct
tests; operations pass 18; production topology passes 15.

WASM and release consumer qualification:

```bash
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-ops --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-topology --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Outcome: pass with Trunk 0.21.14.

Complete release qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-ops --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-topology --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Outcome: pass. Existing Cargo warnings about both inherited SPDX `license` and `license-file`
remain non-failing repository-wide metadata warnings; no licence metadata was removed.

## 4. Acceptance criteria passed

- The complete M55 action surface and all ten pre-M60 stable scenario identities remain intact.
- Accepted all-family curves, explicit periodic NURBS branch/refinement behavior, associative
  fillet state and companion operations are reachable through one nested selector.
- Companion operations publish only through public proposal and retained-session boundaries.
- The production-topology card never presents skipped, truncated, cancelled, exhausted,
  unavailable or stale evidence as consumable.
- Complete/incomplete/cancelled/recovered topology behavior has direct deterministic regression
  coverage.
- Workspace v2 round-trips canonical v4 and draft-v5 multi-interval state, migrates version 1 and
  rejects malformed, unknown-version, unknown-field and unknown-encoding payloads.
- Scenario capture is deterministic and ordinary persistence remains suppressed during scenario
  review.
- Native, WASM and release Trunk consumers pass without a browser harness, `/#/dev/lab`, CDP or
  mobile claim.

No new residual was introduced, so M60 requires no new residual Jacobian implementation or
finite-difference Jacobian test.

## 5. Known limitations and next blocker

- The workbench remains a desktop diagnostic consumer, not a production renderer or B-rep host.
- Advanced authoring in this cut is scenario-led; it does not claim a general-purpose control for
  every advanced curve property.
- Scenario operation replacement deliberately uses reset/transcript as its UAT history boundary;
  it does not claim ordinary coordinator undo across an ephemeral fixture replacement.
- Production topology is recomputed from the current accepted input for presentation. No stale
  profile cache is consumed.
- Mobile and responsive behavior remain outside acceptance.
- The next blocker is supervising-human M61 UAT. Objective qualification is complete, but no M61
  human approval is recorded yet.
