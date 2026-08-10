<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B implementation — Bounded workspace reproduction capsules

Status: active implementation. The bounded transport, restore and ordinary-workbench delivery
scope is fixed below. Focused/direct qualification, the complete integrated release gate, frozen
candidate publication, served-byte verification and supervising-human UAT are all pending. This
document records no pass or approval.

Architecture owner: the existing `geosolve-demo-web` workspace-persistence boundary; M70B adds no
solver or domain authority and requires no new ADR.

Candidate source: pending

Integrated release-gate result: **PENDING**

Tailscale release distribution and byte manifest: **PENDING**

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
  `docs/M70B_UAT.md` own the active scope and pending gate.

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

No command below is recorded as passing yet. The nominated source must run at least:

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-demo-web --all-features
cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown
cargo deny check licenses
git diff --check
nix-shell shell.nix --run './scripts/release-gate.sh'
```

The intended recipient-side diagnostic workflow is:

```text
cargo run --locked -p geosolve-demo-web --bin geosolve-repro < payload.txt
```

That command only exposes decoded workspace JSON for inspection; it is not a qualification result
or a coordinator-publication route.

Pending direct coverage must prove:

- deterministic exact bytes, empty and repetitive workspace round trips and fixed checksum
  convention;
- canonical five-field envelope and strict version, codec, decimal, lowercase checksum and
  unpadded-base64 rules;
- corruption, truncation, trailing zlib bytes, declared-length mismatch, invalid UTF-8 and all
  three resource limits;
- transport bombs stop at the declared bounded output rather than allocating unbounded memory;
- a representative workspace containing computed Fillets, Construction roles and allocator
  high-water restores exact v5 content;
- transport-valid but workspace-invalid text cannot construct or publish a coordinator;
- a corrupt or semantically invalid payload leaves the live workspace byte-identical; and
- native tests cover codec behavior and the same codec path compiles for
  `wasm32-unknown-unknown`.

## 4. Acceptance criteria pending

- [ ] focused codec, persistence and thin-adapter tests pass;
- [ ] warnings-denied native Clippy and the explicit WASM check pass;
- [ ] the locked complete workspace/release gate passes without weakening an existing threshold;
- [ ] dependency licence inventory, package contents and release Trunk assembly pass;
- [ ] one clean source and read-only distribution are frozen and byte-verified over Tailscale;
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
the pending direct/release gate produces one candidate and the focused human UAT is explicitly
approved. M71 stays deferred throughout that work.
