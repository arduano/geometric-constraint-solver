<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B implementation — Bounded workspace reproduction capsules

Status: active after `M70B-F002`. The bounded transport and restore remain qualified, and
`M70B-F001` retains complete replacement evidence. The second human-supplied payload finding has a
direct headless-authoring correction, accepted-scene fallback regression and complete replacement
qualification; publication, supervising-human UAT and approval remain pending. This document
records no human pass or milestone closure.

Architecture owners: the existing `geosolve-demo-web` workspace-persistence boundary owns the
transport, `geosolve-sketch` owns the pre-existing Local contact-branch lowering corrected by
`M70B-F001`, and `geosolve-constraint-editor` owns radial-Normal authoring semantics corrected by
`M70B-F002`. The thin web scene composer may present an older accepted document beneath a rejected
design but gains no solver, branch or inference-publication authority. No new ADR is required.

Prior withdrawn candidate source: `6a0d05246a3fbca7487ffd614c1d48bf5bdc9c8b`

Prior `M70B-F001` replacement source: `b4ec279e221df38816b7376a6978712e21df02c2`

`M70B-F002` replacement source: `2e0f6c348ea0d3d9ee0bc2fd556f402a29d7059b`

`M70B-F002` integrated release-gate result: **PASS**

Prior `M70B-F001` integrated release-gate result: **PASS**

Prior `M70B-F001` Tailscale distribution: `/tmp/geosolve-m70b-f001-uat.A2G9KJ` at
`http://100.94.63.83:8080/`

Prior `M70B-F001` release manifest aggregate:
`b91f25a600e09f99c67f7b8a77d2bc6a38d7a1517fead2b70942ed5681337c28`

Prior withdrawn Tailscale distribution: `/tmp/geosolve-m70b-uat.Oj9SZT`, manifest aggregate
`35ca7410d92aaf074dde7fc6265ad2f99beaea9b082169a7f0fb4ff87d153969`

## 1. Files and APIs

M70B deliberately reuses the sole workbench's complete application checkpoint instead of creating
a second scene model.

- `crates/geosolve-demo-web/src/reproduction.rs` owns the public, pure transformation between
  opaque workspace JSON bytes and `GEOSOLVE_REPRO_V1` text. Its typed errors distinguish envelope,
  version, codec, length, checksum, base64url, compressed-stream, resource and UTF-8 failures.
- `crates/geosolve-demo-web/src/lib.rs` exposes that bounded codec so a reproduction payload can be
  recognized and decoded without browser storage or a solver shortcut.
- `crates/geosolve-demo-web/src/bin/geosolve-repro.rs` is a narrow native diagnostic decoder: it
  reads one payload from standard input and writes decoded workspace JSON to standard output. It
  grants no publication authority; browser restore still owns strict workspace validation and
  complete coordinator reconstruction.
- `crates/geosolve-demo-web/src/workbench/persistence.rs` first obtains a fresh
  `WorkspaceSnapshot::from_coordinator(...).encode()` value. Restore performs transport decode,
  strict `WorkspaceSnapshot::decode` and `coordinator_from_snapshot` in that order and returns a
  complete replacement coordinator; it never mutates an existing coordinator in place.
- `crates/geosolve-demo-web/src/workbench/mod.rs`, `index.html` and `styles.css` own the thin
  visible copy/paste overlay, clipboard attempt/manual-copy fallback, error presentation and final
  all-or-nothing workbench swap. The same module composes current computed/native scenes and now
  retains detached historical accepted presentation when current publication authority is absent.
  Geometry, validation and workspace interpretation remain below that browser adapter.
- `base64 0.23.1`, `miniz_oxide 0.9.1` and transitive `adler2 2.0.1` provide pure-Rust strict
  URL-safe text encoding and zlib stream handling. Their licence expressions are recorded in
  `THIRD_PARTY_LICENSES.md`; no native library, FFI or `unsafe` exception is added.
- `PLAN.md`, `ACCEPTANCE.md`, `ARCHITECTURE.md`, `docs/SCENARIOS.md` and
  `docs/M70B_UAT.md` own the qualified scope and pending human gate.
- `crates/geosolve-sketch/src/compiler.rs` maps each existing semantic-open Local contact interval
  to closed effective core bounds one representable value inward. Persistent branch metadata and
  independent validation remain unchanged.
- `crates/geosolve-sketch/tests/m70b_open_contact_bounds.rs` directly certifies the two effective
  active endpoints; existing M12 and M27 bound expectations cover Bezier, tangency and Fillet
  consumers.
- `crates/geosolve-constraint-editor/tests/m70b_projected_drag.rs` reconstructs the exact payload
  graph/accepted state and owns the continued projected-drag regression.
- `crates/geosolve-constraint-editor/src/coordinator.rs` owns the `M70B-F002` radial-Normal
  SupportingLine/Interior contract and unique retained-accepted-geometry projection seed. It
  rejects the former bounded or Local request before retained mutation.
- `crates/geosolve-constraint-editor/tests/m70b_radial_normal.rs` reconstructs the supplied
  circle/perimeter-line geometry, covers circle/arc external supports in both operand orders and
  freezes historical-accepted seeding beneath visibly different rejected design coordinates. It
  verifies accepted residual validation plus mutation-free rejection of invalid metadata.
- `crates/geosolve-demo-web/src/workbench/mod.rs` directly tests both scene-composition authority
  rows affected by F002: a rejected constraint keeps historical accepted SVG paths visible while
  the detached scene fails retained-session authentication, and a current computed Fillet preview
  remains exact-stamped, composite and authenticated rather than silently falling back to native
  geometry.

The canonical single-line envelope is:

```text
GEOSOLVE_REPRO_V1:zlib-base64url:<workspace_bytes>:<fnv1a64>:<body>
```

`workspace_bytes` is canonical unsigned decimal, `fnv1a64` is exactly sixteen lowercase
hexadecimal digits and `body` is strict unpadded URL-safe base64. Incompatible future transport
semantics require another version prefix.

## 2. Mathematical behavior

The reproduction transport changes no residual, Jacobian, scaling, priority, solve status,
independent validation, rank classification, geometry branch or sketch/feature definition. It
transports persisted application input and accepted-state evidence only. The separately recorded
`M70B-F001` UAT correction changes only the effective core-bound lowering of an existing Local
contact branch; it adds no branch kind or persisted state. `M70B-F002` changes only headless
authoring metadata/initialization for the pre-existing radial centre-on-line relation and the thin
choice between current-bound and detached accepted presentation. It changes no residual,
Jacobian, solver tolerance, priority, independent validation or retained-session authority rule.

Copy follows one authority-preserving path:

1. capture the current retained coordinator through the existing workspace checkpoint API;
2. encode the resulting complete `WorkspaceSnapshot` v5 JSON deterministically;
3. calculate FNV-1a over those exact decoded bytes;
4. compress them as one zlib stream and encode that stream with strict unpadded base64url; and
5. publish the bounded text in the visible overlay and attempt a clipboard copy.

Paste reverses only the transport first. A correct checksum proves accidental-corruption
detection, not authenticity or acceptable geometry. The decoded UTF-8 text must independently
pass strict workspace version/schema/high-water validation, after which a complete
`RetainedEditorCoordinator` is reconstructed through the ordinary restore path. The browser swaps
that fully built coordinator into the sole workbench only after every step succeeds. A failure at
any layer retains the exact live coordinator and accepted scene.

Resource limits are independent:

| Layer | Maximum |
| --- | ---: |
| Complete input/output text | 16 MiB |
| Decoded compressed zlib body | 12 MiB |
| Inflated workspace JSON | 64 MiB |

Decode requires one fully consumed zlib stream with exactly the declared output length. Padded or
noncanonical base64, truncated/corrupt input, trailing compressed bytes and over-expansion all fail
before workspace validation or publication.

Workspace v5 already owns design and accepted document payloads, whether accepted state belongs to
the current design, sketch identity high-water, computed-feature JSON, feature/evaluation allocator
high-water and lifecycle revisions. The capsule adds none of those concepts. It intentionally
excludes current authoring/tool progress, pointer capture, selection/hover state, camera, sample
identity/guidance and native command-history cursor. Successful load therefore restores the
persisted workspace, not an old browser interaction.

## 3. Commands and outcomes

The original transport implementation tree, before `M70B-F001`, passed:

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-demo-web --all-features
cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown
cargo deny check licenses
git diff --check
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

Historical transport-candidate outcomes on 2026-08-10:

- `cargo fmt --all -- --check` and `git diff --check`: pass;
- locked all-feature `geosolve-demo-web`: 94/94 library tests plus 1/1 native decoder test pass;
- warnings-denied demo-web Clippy and the explicit `wasm32-unknown-unknown` check: pass;
- both native and WASM `cargo license` inventories include the recorded M70B packages and only
  recorded GPL-compatible expressions;
- `cargo deny check licenses`: pass;
- the complete integrated release gate: pass, including all locked workspace tests, cross-target
  M70 transition parity, rustdoc, benchmark compilation, package contents, performance budgets,
  the required 256-moving-body sparse crossover and Trunk 0.21.14 release assembly; and
- every one of the seven frozen files and served `/` byte-matches the read-only local snapshot.

An earlier development gate reached Trunk and correctly exposed that the new native diagnostic
binary made the WASM artifact selection ambiguous. The final source explicitly selects
`geosolve_demo_web` in the Trunk link; both a focused release build and the complete replacement
gate pass with that fix.

The historical results above do not nominate the withdrawn distribution for further human UAT.

Replacement source `b4ec279e221df38816b7376a6978712e21df02c2` then passed:

```text
cargo test --locked -p geosolve-sketch --test m70b_open_contact_bounds
cargo test --locked -p geosolve-constraint-editor --test m70b_projected_drag
cargo test --locked -p geosolve-sketch --test m12 --test m27
cargo test --locked -p geosolve-sketch --test m22_differential_constraints \
  --test m22_nurbs_runtime
cargo test --locked -p geosolve-sketch --test m28
cargo test --locked -p geosolve-sketch --test m10 --test m14
cargo clippy --locked -p geosolve-sketch -p geosolve-constraint-editor \
  --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

Focused outcomes are 1/1 direct-bound, 1/1 exact-payload, M12 10/10, M27 10/10,
M22 differential 3/3, M22 NURBS runtime 2/2, M28 18/18, M10 17/17 and M14 12/12.
Warnings-denied focused Clippy, formatting and diff checks pass. The complete clean integrated gate
exited zero: workspace warnings-denied Clippy and locked tests, native/WASM M70 transition parity,
the demo-web WASM check, warnings-denied rustdoc, benchmark compilation, performance budgets,
package/licence checks and Trunk 0.21.14 release assembly all pass. The required 256-moving-body
sparse crossover passes in 146.60 seconds. Only the pre-existing non-failing Cargo `license` plus
`license-file` advisories were emitted.

The release distribution contains exactly seven read-only files:

```text
af91333ed578f05ec49c76fd10c18dd0ead0f9f845b8ff45279de5a6cbc7b80e  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
922e9e87046394256a701436b9991ad25cd2ef28786b69e0a70d8eaa6163993a  geosolve-demo-web-8a79f3f16d3cefbf.js
0bb7882b6b4928fce6f6d4bc9ba55955e4f7889659e4af7270b1f67bbf6c48ef  geosolve-demo-web-8a79f3f16d3cefbf_bg.wasm
dae08a4c361e72668e09e3687352d88fa50baf778e345cc6e54349c4fd3beae6  index.html
49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048  styles-36c74d05d21a90c9.css
```

All seven assets were fetched through the actual Tailscale address with proxy/cache bypass and
compared byte-for-byte to the frozen snapshot. `/` also matches `index.html`; independently
calculated local and served aggregates both equal the replacement manifest above.

The intended recipient-side diagnostic workflow is:

```text
cargo run --locked -p geosolve-demo-web --bin geosolve-repro < payload.txt
```

That command only exposes decoded workspace JSON for inspection; it is not a qualification result
or a coordinator-publication route.

Direct coverage proves:

- deterministic exact bytes, empty and repetitive workspace round trips and fixed checksum
  convention;
- canonical five-field envelope and strict version, codec, decimal, lowercase checksum and
  unpadded-base64 rules;
- corruption, truncation, trailing zlib bytes, declared-length mismatch, invalid UTF-8 and all
  three resource limits, including exact-equality acceptance at each bound;
- transport bombs stop at the declared bounded output rather than allocating unbounded memory;
- a representative workspace containing computed Fillets, Construction roles and allocator
  high-water restores exact v5 content;
- transport-valid but workspace-invalid text cannot construct or publish a coordinator;
- transport- and workspace-valid state whose retained lifecycle exhausts coordinator
  reconstruction also rejects through the complete payload path; and
- a corrupt or semantically invalid payload leaves the live workspace byte-identical; and
- native tests cover codec behavior and the same codec path compiles for
  `wasm32-unknown-unknown`.

The `M70B-F002` focused worktree passes:

```text
cargo test --locked -p geosolve-constraint-editor --test m70b_radial_normal --test m55
cargo test --locked -p geosolve-demo-web canvas_scene
cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web \
  --all-targets --all-features -- -D warnings
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown
cargo fmt --all -- --check
git diff --check
```

Focused outcomes are 4/4 radial-Normal regressions, M55 17/17 and 2/2 accepted-scene composition
authority rows. Warnings-denied focused Clippy, explicit WASM compilation, formatting and diff
checks pass.

Clean nominated source `2e0f6c348ea0d3d9ee0bc2fd556f402a29d7059b` then passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The complete integrated gate exited zero: formatting, static single-workbench and Git-hygiene
checks, workspace warnings-denied Clippy and locked all-feature tests, native/WASM M70 transition
parity, the demo-web WASM check, warnings-denied rustdoc, benchmark compilation, performance
budgets, package/licence checks and Trunk 0.21.14 release assembly all pass. The required
256-moving-body sparse crossover passes in 147.45 seconds. Only the pre-existing non-failing Cargo
`license` plus `license-file` advisories were emitted. Replacement distribution freezing,
publication and served-byte verification remain pending.

## 3.1 `M70B-F001` owning-layer correction

Payload identity `8446:ea81c82137d5b13c` restores a free line endpoint, the other line endpoint
on a circle and an ellipse major-axis point on the line. The restored state is neither singular
nor overconstrained: numerical rank is four, equality and bidirectional bounded mobility are ten,
and the free endpoint's locality plan has five passive freedoms and three anchors.

The bounded point-on-line contact persists a Local neighbourhood
`(0.17362649353483556, 0.5736264935348356)` and an accepted parameter
`0.5268478331756027`. Local neighbourhoods are strict/open branch state, but the compiler had
given the closed core optimizer those same endpoints. A secondary optimum could therefore become
active at an endpoint accepted by core and then be rejected by independent sketch validation as
`AmbiguousContactNeighborhood`.

The compiler now gives only Local contacts the effective closed interval
`[lower.next_up(), upper.next_down()]`. Persisted branch metadata and strict independent `>`/`<`
validation remain unchanged; bounded `Interior`, exact `Start`/`End`, tolerances, rank rules and
drag policy are untouched. A direct sketch regression certifies both effective active endpoints,
independent validity and unchanged branch metadata. The exact retained-editor graph reaches six
formerly failing horizontal, vertical, diagonal and reversal targets within `1e-8`, in one bounded
attempt each, with independently validated normalized hard residual at most `1e-9` and all ten
freedoms preserved.

## 3.2 `M70B-F002` owning-layer correction

Payload identity `6037:eecc886c0e61208f` restores an accepted circle and line whose end point is
already on the circumference, then retains a rejected radial Normal source. Resolution was
correctly `RadialLine`, and its persistent definition correctly reused centre-on-line
`PointOnCurve`, but generic authoring selected the line's first advertised contact domain:
bounded `[0,1]`/Interior at the picked parameter `0.5237281588081177`.

That metadata imposed unintended segment containment. The circle centre's unique affine
projection is about `1.6632787580742947`, outside the segment. The nonlinear attempt therefore
drove the positive radius toward the degenerate zero branch and stalled after 17 iterations at
maximum normalized hard residual about `1.53e-2`; the supplied workspace correctly retained the
prior accepted document rather than publishing that candidate.

The coordinator now gives radial Normal a dedicated closed authoring contract:

1. select the affine line and circle/arc centre independently of operand order;
2. use compatible retained accepted geometry to calculate the unique finite supporting-line
   projection, including historical accepted geometry beneath a rejected design, and fail if the
   selected identities are absent rather than reading attempted coordinates;
3. expose and persist only `ContactDomain::SupportingLine` with
   `ContactNeighborhood::Interior`, winding zero and no tangent/normal-side branch; and
4. reject a bounded or Local direct radial request before any retained design/attempt identity can
   advance.

The supplied-coordinate regression goes through `AuthoringState` and `apply_authoring`, publishes
independently hard-valid finite geometry at normalized residual at most `1e-9`, directly rejects
both bounded and Local restrictions without retained mutation, freezes the visibly non-collapsed
radius and evaluates the accepted contact through its supporting-line domain. A fixed line segment
`(2,0)->(3,0)` with circle/arc centre `(0,0)` additionally freezes parameter `-2`, both curve
families and both operand orders. A separate rejected-move fixture puts the attempted centre at
`(100,0)` while the retained accepted centre stays at `(0,0)`, then proves both action metadata and
compact authoring still use accepted parameter `-2`. This remains radial centre-on-support
incidence, not a claim that the selected circumference point is the normal contact.

The visible disappearance was a separate presentation-authority error. Beneath a newer rejected
design, `accepted_state()` intentionally remains available while
`accepted_state_for_current_input()` and `accepted_prepared_input()` do not. The WASM adapter had
treated that missing current authority as `scene=None`. Scene composition now attempts the
historical native accepted document only for that detached presentation row; current computed
output still fails closed if its provenance or affordances cannot compose, while explicit
Withheld/Absent state keeps its existing authenticated native path. Direct demo-web regressions
prove both affected rows: old accepted SVG paths remain without inferred-construction authority,
and a current Fillet preview stays composite and authenticated. The rejected fixture deliberately
puts one attempted point at `(40,40)` while its accepted coordinate stays `(0,0)`, compares the
complete scene point/curve vectors and requires visible problem-marker markup. Attempted geometry
is never substituted.

## 4. Acceptance criteria

- [x] focused codec, persistence and thin-adapter tests pass;
- [x] warnings-denied native Clippy and the explicit WASM check pass;
- [x] the prior `M70B-F001` locked complete workspace/release gate passes without weakening an
  existing threshold;
- [x] the prior `M70B-F001` dependency licence inventory, package contents and release Trunk
  assembly pass;
- [x] the `M70B-F001` replacement source and read-only distribution are completely requalified,
  frozen and byte-verified over Tailscale;
- [x] `M70B-F002` payload, circle/arc external-support, operand-order, historical-seed,
  invalid-request and scene-authority regressions pass with no solver or authority weakening;
- [x] the `M70B-F002` source passes the complete integrated release gate;
- [ ] its replacement distribution is frozen and byte-verified over Tailscale;
- [ ] every prepared area in `docs/M70B_UAT.md` is exercised; and
- [ ] the supervising human explicitly approves M70B.

## 5. Known limitations and next blocker

`GEOSOLVE_REPRO_V1` is a diagnostic application-workspace interchange, not canonical sketch JSON,
a host interchange standard, encryption, authentication or a long-term substitute for a future
product file format. It reproduces no browser-local interaction or command history. Very large
payloads remain unsuitable for chat even when below the defensive limits; the UI must report their
size honestly rather than silently dropping content.

The removed M32 `GEOSOLVE_SCENE_V1` LZSS/profile-budget capsule, `/#/dev/lab`, file picker,
download flow, raw browser-storage handoff and browser E2E remain retired. M70B cannot close until
the focused human UAT and targeted `M70B-F001`/`M70B-F002` rechecks are explicitly approved. A
Local interval whose semantic endpoint is exactly zero has no valid solution at that endpoint;
exact edge pressure can therefore still fail closed before the one-ULP effective endpoint becomes
active. Ordinary near-edge contacts and the supplied positive-bound payload pass, but exact
bound-event application for extreme zero/tiny Local intervals remains a future hardening topic
rather than being folded into this repair. Radial Normal deliberately means that the circle/arc
centre lies on the complete affine line support. True contact-bearing normal authoring at a picked
circumference location remains a future retained-primitive/UX decision and is not smuggled into
M70B. M71 stays deferred throughout that work.
