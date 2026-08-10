<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B implementation — Bounded workspace reproduction capsules

Status: active human UAT. The bounded transport, restore, ordinary-workbench delivery, focused
qualification, complete integrated release gate, frozen-candidate publication and served-byte
verification pass. Supervising-human UAT and approval remain pending; this document records no
human pass or milestone closure.

Architecture owner: the existing `geosolve-demo-web` workspace-persistence boundary; M70B adds no
solver or domain authority and requires no new ADR.

Candidate source: `6a0d05246a3fbca7487ffd614c1d48bf5bdc9c8b`

Integrated release-gate result: **PASS**

Tailscale release distribution: `/tmp/geosolve-m70b-uat.Oj9SZT` at
`http://100.94.63.83:8080/`

Release manifest aggregate: `35ca7410d92aaf074dde7fc6265ad2f99beaea9b082169a7f0fb4ff87d153969`

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
  all-or-nothing workbench swap. Geometry, validation and workspace interpretation remain below
  that browser adapter.
- `base64 0.23.1`, `miniz_oxide 0.9.1` and transitive `adler2 2.0.1` provide pure-Rust strict
  URL-safe text encoding and zlib stream handling. Their licence expressions are recorded in
  `THIRD_PARTY_LICENSES.md`; no native library, FFI or `unsafe` exception is added.
- `PLAN.md`, `ACCEPTANCE.md`, `ARCHITECTURE.md`, `docs/SCENARIOS.md` and
  `docs/M70B_UAT.md` own the qualified scope and pending human gate.

The canonical single-line envelope is:

```text
GEOSOLVE_REPRO_V1:zlib-base64url:<workspace_bytes>:<fnv1a64>:<body>
```

`workspace_bytes` is canonical unsigned decimal, `fnv1a64` is exactly sixteen lowercase
hexadecimal digits and `body` is strict unpadded URL-safe base64. Incompatible future transport
semantics require another version prefix.

## 2. Mathematical behavior

M70B changes no residual, Jacobian, scaling, priority, solve status, independent validation, rank
classification, geometry branch or sketch/feature definition. It transports persisted application
input and accepted-state evidence only.

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

The exact implementation tree later committed unchanged as the nominated source passed:

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

Outcomes on 2026-08-10:

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

## 4. Acceptance criteria

- [x] focused codec, persistence and thin-adapter tests pass;
- [x] warnings-denied native Clippy and the explicit WASM check pass;
- [x] the locked complete workspace/release gate passes without weakening an existing threshold;
- [x] dependency licence inventory, package contents and release Trunk assembly pass;
- [x] one clean source and read-only distribution are frozen and byte-verified over Tailscale;
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
the focused human UAT is explicitly approved. M71 stays deferred throughout that work.
