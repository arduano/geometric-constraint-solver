<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 implementation ledger — Projectional sketch design intent

Status: **implementation, clean mechanical qualification and immutable Tailscale nomination
complete; human UAT pending**. This ledger records implementation and qualification against ADR
0040 and `docs/M83_GOALS.md`. Accepted M81 GitHub Pages bytes remain public authority.

## Baseline and disposition

- Accepted product baseline: M81 closeout plus the M82 exact rollback on `main`.
- Rejected chronological research: branch `archive/m83-chronological-lineage-2026-08-23`, tip
  `be62a1c`.
- Replacement implementation begins after rollback commit `af77877` and reuses only independently
  justified native seams. It does not restore the archived ledger, JSON owner rewrite or mirrored
  history.
- Nominated product source: `1b4f4558688e1bd32be075793e11885f892a3245`; tree:
  `ce09e010dc74f2e97b19d52d15433eef8f2f78d3`.

## Implementation slices

### I1 — native materialization prerequisites

Completed preparatory commits expose canonical parameter batches, atomic scalar edits, neutral
continuation seeding, independently authenticated accepted rematerialization, native-Fillet intent
continuation and transient-only pointer rendering. These APIs remain useful below any lineage
model and keep accepted-state validation inside `geosolve-sketch`.

### I2 — `geosolve-sketch-intent`

Complete. The new pure-Rust crate owns stable graph/port/child/reservation identities, closed
catalog schemas, separate instance/organization/external identities, unordered patches, exact CAS,
retained accepted/failure authority, canonical persistence and one bounded Undo/Redo history. It
contains no solver equation and depends on neither the sketch domain nor the editor.

The first review-hardening pass adds a durable reservation/tombstone ledger to semantic and
session identity, stable developer symbols independent of mutable display names, non-writable
logical handles, accurately named host materialization artifacts and deterministic clock-free
transaction descriptors for the read-only History projection. Focused lifecycle coverage includes
suppression, deletion, retained failure, Undo/Redo, divergent edits, bounded-history eviction and
canonical reload. The equation-free branded TypeScript package consumes the same closed wire
vocabulary and carries no geometry or residual implementation.

### I3 — editor-owned materializer

Complete. `geosolve-constraint-editor` lowers the closed geometry, relation, dimension, operation,
computed-Fillet, parameter and external catalogs in dependency order; translates exact native
reservations; records logical/native ownership and reverse free-leaf bindings; decodes canonical
host inputs; and uses the existing authenticated native Fillet/Profile Offset paths. Accepted
publication requires the existing independent solve validation, and canonical cold reconstruction
is covered against warm/editor materialization.

### I4 — coordinator integration and bootstrap

Complete. One projectional coordinator owns intent, accepted materialization and bounded composite
history. Fresh and legacy flat workspaces normalize into typed per-object bootstrap declarations
with exact existing native bindings, and supported point ejection continues identity in place.
Canvas construction, contextual relations/dimensions, Inspector/source edits, native role changes,
computed Fillet, Profile Offset, operation deletion and live RPC all publish through the same patch
authority. Pointer preview state remains transient; terminal release publishes at most one exact
instance/property transaction.

### I5 — projections, RPC and workspace v8

Complete. Commit `50d2ec6` installed the Design-panel shell; subsequent slices add Rust-backed
`Outline | Structured source | History`, schema-derived Inspector edits, organization-only button
and drag/drop reorder, deterministic TypeScript-shaped source tokens, read-only History, DOM-free
WASM/RPC, a branded TypeScript client and strict workspace-v8 persistence. Version 7 rejects and
v1-v6 migrate through their original strict decoder before per-object bootstrap normalization.

The M76 annotation-layout cache remains an explicitly separate presentation-only workspace field:
it is compatibility-filtered and recomputable, omitted from reproduction authority, and never
enters intent identity, dependency scheduling, materialization, solver input or composite history.

Final interaction hardening at `62378c9` projects a unique canvas/tree owner to the same stable
declaration selected by Outline/source; ambiguous, protected, unowned or multi-owner native
selection clears declaration targeting. Toolbar and Delete/Backspace share editor-owned deletion,
authoring retains keyboard precedence, failed deletion does not save, retained-invalid Profile
Offset deletes by stable declaration with its exact private aggregate closure, and grouped private
helper source rows cannot become invisible selection/reorder targets while recognized token edits
remain typed.

### I6 — qualification and nomination

Mechanical qualification and nomination are complete. Inventory, order-independence,
reservation/tombstone, retained-failure, cold/warm differential,
construction/application/operation, Fillet/Offset, persistence, native/WASM RPC, TypeScript and
split pointer/terminal performance suites pass. The exact clean-gate release output was frozen
without rebuilding, verified first on a temporary Tailscale listener and independently verified
again at the retained UAT endpoint. Human M83-U1 through M83-U8 review remains pending. Do not
publish Pages before human approval.

## Findings

No replacement-architecture finding is open. The final selection/deletion review found and closed
one pre-nomination interaction seam before assigning a public `M83-Fxxx` identity: browser and
editor selection could name different mutation targets, and private Offset helpers could be
selected from source despite being grouped out of Outline. Exact owner regressions now freeze the
corrected contract; no solver equation or mathematical behavior changed.

## Qualification record

Focused owner suites passed before nomination and the final clean gate reran their complete parent
suites from the exact nominated source:

```text
cargo test --locked -p geosolve-constraint-editor --test m83_projectional_editor
8 passed

cargo test --locked -p geosolve-constraint-editor --test m83_projectional_offset
11 passed

RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-demo-web --lib
189 passed

cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
pass
```

The first provisional documentation candidate exposed one strict Clippy-only test spelling and was
not nominated. Commit `1b4f455` changes only `assert!(x == 0)` to `assert_eq!(x, 0)` in that
regression. From the resulting clean committed source, this exact command completed with exit 0:

```bash
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate ran from 2026-08-24 14:31:13 to 14:52:37 AEST. Its 376,943-byte, 5,554-line log is
`/tmp/geosolve-m83-gate.0So8w1Mo/release-gate.log`, SHA-256
`b6547c1bbbb99175d108c5a2a506f6146a8b01b133a3953c472c9705dd5caeae`. It passed formatting and
diff hygiene, warnings-denied locked all-target/all-feature workspace Clippy, locked all-feature
workspace tests, exact 271-row golden `--require-clean`, the M70/M71/M74/M75/M76/M77/M79 and demo
native/WASM parity cuts, the all-feature WASM check, TypeScript clean install/build/runtime tests,
warnings-denied Rustdoc, benchmark compilation, M14/M32/M83 and split interaction performance,
the ignored 256-moving-body sparse crossover in 128.19 seconds, licence/package checks and Trunk
0.21.14 release assembly. M83 retained-preview p95 was 11.504 ms against 16 ms; exact terminal was
122.985 ms against 750 ms. The three interaction p95 values were 4.674 ms, 7.912 ms and 18.192 ms,
all within their independent frame budgets.

The repository's default test-thread stack limitation remains handled with
`RUST_MIN_STACK=16777216` inside the authoritative gate. HEAD, tree and worktree remained exact and
clean throughout qualification.

## Immutable candidate and served-byte evidence

Without rebuilding, the gate-produced `crates/geosolve-demo-web/dist` was copied to
`/tmp/geosolve-m83-uat.DFamHN` and byte-compared before and after freezing. The directory is `0555`;
all seven entries are regular non-symlink files at `0444`. Complete source/freeze metadata is in
`/tmp/geosolve-m83-freeze-evidence.QX4H66`. The C-locale ordered `sha256sum *` manifest aggregate is
`4bb4bf4f22caefae429b514cfffda1d92ac3704e6f3474c5022413ec0f242989`:

```text
7acf06ec28c181468f26a92f6978af0f4b9d4f3205e076e602c517f00923d07f  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
2b04bb132eb756898a0d313442a86019949b8d2b3ef7e302798039864ef6d657  geosolve-demo-web-fdac61e779feddb4.js
9eb17134bdd1c43fe02594f34b291c533b6dd391092260b1c54426b05c151498  geosolve-demo-web-fdac61e779feddb4_bg.wasm
0c48bd6d73d0420d767d5cfbe761902cec1debfe9462f05ebef9c16323e2817d  index.html
b1311b3d99e27cecee11beb86a8cb32251f7132a615cbcead5be9815aa4c49f4  styles-105fdd951cbee1a1.css
```

| File | Bytes |
| --- | ---: |
| `API_COMPATIBILITY.md` | 28,079 |
| `LICENSE` | 35,148 |
| `THIRD_PARTY_LICENSES.md` | 3,120 |
| `geosolve-demo-web-fdac61e779feddb4.js` | 37,314 |
| `geosolve-demo-web-fdac61e779feddb4_bg.wasm` | 11,599,582 |
| `index.html` | 32,577 |
| `styles-105fdd951cbee1a1.css` | 45,114 |

Temporary service `geosolve-m83-temp-uat.service`, PID `2740811`, first served only that snapshot
at `100.94.63.83:18083`. Proxy-disabled, cache-bypassed identity requests for `/` and all seven
files returned HTTP 200 with zero redirects, no `Location` or `Content-Encoding`, exact expected
media type and length, and snapshot-identical bytes; `/` equals `index.html`. Evidence is
`/tmp/geosolve-m83-temp-verify.2ogZzJ/results.tsv`, SHA-256
`6d57de9beadd0114afb2d1f101b0f9a542c876ea45aa9f9efda11724b351ce97`.

Only after that ledger passed, `geosolve-m83-uat.service`, PID `2747514`, began serving the same
immutable directory at `http://100.94.63.83:8080/`. The same eight checks passed independently;
final evidence is `/tmp/geosolve-m83-final-verify.GG1MfU/results.tsv`, with the same result-ledger
SHA-256 because every asserted path/status/type/length/body hash is identical. The temporary
listener is retired and the retained `:8080` service remains live for focused human UAT. The
unrelated loopback VS Code listener was not changed.

The documentation-only descendant recording this evidence does not replace nominated product
source/tree or rebuild its artifact. GitHub Pages deliberately remains on accepted M81 bytes until
explicit M83 human approval.
