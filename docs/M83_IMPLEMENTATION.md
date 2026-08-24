<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 implementation ledger — Projectional sketch design intent

Status: **implementation, M83-F001 through M83-F007 repair, clean post-F007 qualification and
immutable Tailscale nomination complete; human UAT pending**. This ledger records implementation
and qualification against ADR 0040 and `docs/M83_GOALS.md`. Accepted M81 GitHub Pages bytes remain
public authority.

## Baseline and disposition

- Accepted product baseline: M81 closeout plus the M82 exact rollback on `main`.
- Rejected chronological research: branch `archive/m83-chronological-lineage-2026-08-23`, tip
  `be62a1c`.
- Replacement implementation begins after rollback commit `af77877` and reuses only independently
  justified native seams. It does not restore the archived ledger, JSON owner rewrite or mirrored
  history.
- Withdrawn initial nomination: documentation commit `232b83a627a1f6e6aea9121cd1768a551072ba1f`
  advertised product source `1b4f4558688e1bd32be075793e11885f892a3245`, tree
  `ce09e010dc74f2e97b19d52d15433eef8f2f78d3`. M83-F001 through M83-F005 make those bytes
  historical evidence only.
- Post-F005 product source, since withdrawn by F006/F007: `a621cddc0a3b8687d6b7686bc850619332c73779`;
  tree `f6d77b447552d0d120c48be4bfdeb96bc5f2da59`.
- Current post-F007 product source: `fafea4ebeddc295ca898258ac604858bcdd4db5f`;
  tree `ff75c36aacdabe601f34bb59baaba01f6c91687a`.

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

M83-F001 keeps latest attempted and latest accepted point samples distinct. A rejected terminal
attempt commits the newest visible accepted preview, an exact release reuses that preview, and a
delayed duplicate capture-loss terminal is inert. Suppressed declarations expose no reverse
binding to an absent native object, and retained-invalid current intent rejects manipulation at
pointer-down. M83-F003 cold-restores retained-invalid migrated/bootstrap v8 state from its exact
authenticated accepted graph/prefix while retaining current failed intent, history and Undo.

M83-F006 keeps exact preview/cold comparison while canonicalizing only recomputable derived line
branch metadata for Polyline and the four rectangle recipes, and only within the same positive
branch cell. Segment and Midpoint Line retain exact explicit branch intent; the Midpoint Line
lowerer now reads that field instead of deriving a replacement from coordinates. Repeated shared-
corner rectangle/diagonal drags publish the visible preview, validate residuals, preserve the
explicit diagonal branch and Undo exactly.

### I5 — projections, RPC and workspace v8

Complete. Commit `50d2ec6` installed the Design-panel shell; subsequent slices add Rust-backed
`Outline | Structured source | History`, schema-derived Inspector edits, organization-only button
and drag/drop reorder, deterministic TypeScript-shaped source tokens, read-only History, DOM-free
WASM/RPC, a branded TypeScript client and strict workspace-v8 persistence. Version 7 rejects and
v1-v6 migrate through their original strict decoder before per-object bootstrap normalization.

The M76 annotation-layout cache remains an explicitly separate presentation-only workspace field:
it is compatibility-filtered and recomputable, omitted from reproduction authority, and never
enters intent identity, dependency scheduling, materialization, solver input or composite history.

M83-F007 restores New on the projectional toolbar. Startup and New share one canonical empty
projectional authority; the action clears durable authored geometry/declarations/history plus
transient authoring, selection-adjacent preview, camera and disclosure state, returns to Select,
and autosaves a canonical workspace-v8 snapshot. It does not fall back to the retired flat event
authority.

Final interaction hardening at `62378c9` projects a unique canvas/tree owner to the same stable
declaration selected by Outline/source; ambiguous, protected, unowned or multi-owner native
selection clears declaration targeting. Toolbar and Delete/Backspace share editor-owned deletion,
authoring retains keyboard precedence, failed deletion does not save, retained-invalid Profile
Offset deletes by stable declaration with its exact private aggregate closure, and grouped private
helper source rows cannot become invisible selection/reorder targets while recognized token edits
remain typed.

M83-F002 resolves native row/header drop targets into explicit before/after slots, including
adjacent and end moves, while retaining organization-only semantics. M83-F004 stamps every source
token edit with exact `IntentSessionIdentity` before token lookup across DOM, Rust RPC and branded
TypeScript. M83-F005 exposes canonical input-slot-to-stable-port bindings in Structured Source and
Inspector; rebind updates both deterministically and Inspector references remain read-only.

### I6 — qualification and nomination

Post-F005 mechanical qualification and nomination completed. Inventory, order-independence,
reservation/tombstone, retained-failure, cold/warm differential,
construction/application/operation, Fillet/Offset, persistence, native/WASM RPC, TypeScript and
split pointer/terminal performance suites pass. The exact clean-gate release output was frozen
without rebuilding, verified first on a temporary Tailscale listener and independently verified
again at the retained UAT endpoint. F006/F007 subsequently withdraw that candidate. The clean
post-F007 gate, no-rebuild freeze, 5/5 frozen-browser checks and temporary/retained served-byte
verification pass; human M83-U1 through M83-U9 review remains pending. Do not publish Pages before
human approval; neither earlier nomination supplies current UAT authority.

## Findings

All seven replacement findings are mechanically resolved; their human rechecks remain pending.

- **M83-F001 — deterministic accepted drag identity and exact-once terminal capture.** The
  withdrawn candidate could let a rejected newer sample or duplicate capture terminal obscure the
  last accepted visible drag. Attempted/accepted identities are now separate; release commits the
  newest authenticated accepted preview once, while suppressed bindings, retained-invalid intent,
  foreign/stale terminals and cancellation fail closed.
- **M83-F002 — exact Outline/cell insertion semantics.** Painted targets map their upper/lower
  halves to before/after slots after excluding the moving identity. Adjacent, end, self, stale and
  cross-cell cases are deterministic and organization-only with exact Undo.
- **M83-F003 — retained-invalid bootstrap reload authority.** Cold v8 restore reconstructs the
  authenticated prior accepted migrated canvas rather than blanking it, while current failed
  intent, history and Undo remain inspectable. Corrupt authority still rejects.
- **M83-F004 — exact-CAS Structured Source tokens.** Browser, Rust RPC and TypeScript requests
  carry the exact originating session/revision/digest before numeric token lookup. Reorder-stale
  tokens reject atomically instead of editing a different declaration.
- **M83-F005 — complete stable-input projection.** Structured Source and Inspector expose each
  canonical input slot as its exact stable typed port reference and update on rebind; Inspector
  references carry no edit control.
- **M83-F006 — shared recipe drag preview/cold parity.** A rectangle point shared with a diagonal
  could solve and preview correctly, then snap back because native continuation retained a
  round-off-different derived rectangle edge branch vector from cold materialization. Preview/cold
  comparison now canonicalizes only schema-owned recomputable Polyline/rectangle branch metadata
  after proving the same positive branch cell; all unrelated draft-v5 state remains exact. Segment
  and Midpoint Line branches are never normalized, and Midpoint Line lowering preserves its
  explicit branch field.
- **M83-F007 — projectional New workspace reset.** New is enabled and builds the canonical fresh
  projectional authority, clears populated authored geometry/declarations/history and transient
  interaction state, returns to Select and persists workspace v8 instead of using the retired flat
  path.

No finding changes a solver equation, residual, priority, tolerance or accepted M81 public
behavior. F006 preserves existing explicit branch rules; it narrows comparison normalization to
derived same-cell metadata and corrects Midpoint Line materialization to honor its stored branch.

## Withdrawn initial qualification record

The initially nominated source passed these owner suites and its clean gate. M83-F001 through
M83-F005 later withdrew it; the record remains historical rather than current qualification:

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

The repository's default test-thread stack limitation was handled with
`RUST_MIN_STACK=16777216` inside that initial gate. HEAD, tree and worktree remained exact and
clean throughout its qualification.

## Withdrawn initial candidate and served-byte evidence

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
listener was retired and the retained `:8080` service was live for focused human UAT at that
checkpoint. The unrelated loopback VS Code listener was not changed.

This initial artifact is no longer UAT authority and its former process is retired. Its bytes and
ledgers remain historical evidence only.

## Replacement qualification record

Focused F001-F005 owner suites and their complete parents pass: projectional coordinator 6/6,
projectional editor 9/9, intent projection 6/6, intent RPC 6/6, demo-web library 193/193, packaged
TypeScript runtime 10/10, focused warnings-denied Clippy, formatting, diff hygiene and the
all-feature WASM check. An independent read-only review approved F005's deterministic projection
and found no mutation, materialization or solver authority change.

```text
cargo test --locked -q -p geosolve-constraint-editor --test m83_projectional_coordinator
6 passed
cargo test --locked -q -p geosolve-constraint-editor --test m83_projectional_editor
9 passed
cargo test --locked -q -p geosolve-constraint-editor --test m83_intent_projection
6 passed
cargo test --locked -q -p geosolve-constraint-editor --test m83_intent_rpc
6 passed
RUST_MIN_STACK=16777216 cargo test --locked -q -p geosolve-demo-web --lib
193 passed
cargo clippy --locked -p geosolve-constraint-editor --test m83_intent_projection -- -D warnings
pass
cargo clippy --locked -p geosolve-demo-web --lib -- -D warnings
pass
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
pass
```

From clean committed source `a621cddc0a3b8687d6b7686bc850619332c73779`, this exact command
completed with exit 0:

```bash
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The replacement gate ran from 2026-08-24 17:09:15 to 17:24:39 AEST in 924 seconds. Its
372,195-byte, 5,519-line log is
`/tmp/geosolve-m83-f001-final-gate.uR6Ei0/release-gate.log`, SHA-256
`1043356cb2480944314566eab7fb92e5d560f48ee606ce3ca15ea4753f16af1d`. It passed formatting,
diff hygiene, warnings-denied locked all-target/all-feature workspace Clippy, locked all-feature
workspace tests, exact 271-row golden `--require-clean`, native/WASM parity, all-feature WASM,
TypeScript clean install/build/runtime tests, warnings-denied Rustdoc, benchmark compilation,
licence/package checks and Trunk 0.21.14 release assembly. The 256-moving-body sparse crossover
passed in 127.40 seconds.

M83 retained-preview p95 was 11.608 ms against 16 ms and exact terminal publication was 127.645 ms
against 750 ms. Fillet-radius, Offset-distance and relation-heavy-curve-control frame p95 values
were 4.918 ms, 6.931 ms and 16.639 ms, all within their independent 250/400/150 ms ceilings. The
exact terminal values were 5.315 ms, 7.666 ms and 92.326 ms against 4000/6000/3000 ms ceilings.

## Post-F005 immutable candidate and served-byte evidence (withdrawn)

Without rebuilding, the gate-produced `dist` was copied to
`/tmp/geosolve-m83-f005-uat.ge07gw`, byte-compared before and after freezing, and made immutable for
UAT: directory mode `0555`, seven regular non-symlink files at `0444`. Complete freeze metadata is
`/tmp/geosolve-m83-f005-freeze-evidence.EdVqUj`. The ordered manifest aggregate is
`720ea687a7002a9f1dbc818147263d9b003cc03bd79cefb2cf8e6aca2597b89b`:

```text
7acf06ec28c181468f26a92f6978af0f4b9d4f3205e076e602c517f00923d07f  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
670ea0707236efb16588039cd12c2b9c8edae74b92f4b1f51951138d95ec8244  geosolve-demo-web-27a5256c4c16b7b9.js
6d6f13d0017e6f41ff79dbbebb571d7ad5a8da87c39e86ca5b1d1d7332339765  geosolve-demo-web-27a5256c4c16b7b9_bg.wasm
68a079db5b5a48c0916a16ebcd8521d2ea92bda8093651878c7be792c476e032  index.html
368d50605a0f14d4e67784e711cd5d920cd56a715fe82d09e68f2bd974caaf24  styles-4251c5b53d199c44.css
```

| File | Bytes |
| --- | ---: |
| `API_COMPATIBILITY.md` | 28,079 |
| `LICENSE` | 35,148 |
| `THIRD_PARTY_LICENSES.md` | 3,120 |
| `geosolve-demo-web-27a5256c4c16b7b9.js` | 37,314 |
| `geosolve-demo-web-27a5256c4c16b7b9_bg.wasm` | 11,638,103 |
| `index.html` | 32,577 |
| `styles-4251c5b53d199c44.css` | 45,555 |

The frozen artifact passed 3/3 Playwright checks: six repeated accepted point drops with delayed
capture loss and Undo/Redo; adjacent Outline before/after drops with invariant geometry; and three
curve-property drops with exact Undo restoration.

```bash
NODE_PATH=/home/arduano/.npm/_npx/420ff84f11983ee5/node_modules \
  node /home/arduano/.npm/_npx/420ff84f11983ee5/node_modules/@playwright/test/cli.js \
  test --config=/tmp/m83-pw/playwright.config.js
```

Temporary Tailscale service `geosolve-m83-f005-temp-uat.service` first served the frozen snapshot
at `100.94.63.83:18085`. Proxy-disabled, cache-bypassed checks of `/` and all seven files returned
HTTP 200, zero redirects, no `Location`/`Content-Encoding`, exact type/length/body and root equality
with `index.html`. Evidence `/tmp/geosolve-m83-f005-temp-verify.YYwD99/results.tsv` has SHA-256
`585aa1571a1bc4440fcce609afa17435382ad1ee8d8f9d60e128229f18313a87`.

Only after that pass, `geosolve-m83-uat.service`, PID `3331431`, began serving the same directory at
`http://100.94.63.83:8080/`. Independent final evidence
`/tmp/geosolve-m83-f005-final-verify.EGsGQD/results.tsv` has the identical ledger SHA-256. Temporary
Tailscale and loopback browser listeners were retired; at that checkpoint `:8080` remained live for
focused human UAT. F006/F007 later retired and replaced that service.

The documentation-only descendant recording that evidence did not replace its product source/tree
or rebuild its artifact. GitHub Pages deliberately remains on accepted M81 bytes until explicit
M83 human approval.

## Post-F007 qualification record

Focused F006/F007 owner and collateral suites pass: materializer 27/27, projectional coordinator
6/6, curve controls 5/5, projectional editor 10/10 and demo-web library 195/195, including the
three derived-branch unit regressions. Focused warnings-denied Clippy, formatting/diff hygiene and
the all-feature WASM check pass. No equation, residual, solver priority or tolerance changed.

From clean committed source `fafea4ebeddc295ca898258ac604858bcdd4db5f`, tree
`ff75c36aacdabe601f34bb59baaba01f6c91687a`, this exact command completed with exit 0:

```bash
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate ran from 2026-08-24 20:14:40 to 20:30:47 AEST in 967 seconds. Its 373,070-byte,
5,530-line log is `/tmp/geosolve-m83-f007-gate.aGkBUu/release-gate.log`, SHA-256
`20807697a2df8941d59c9033c1604063940284e7b89f10ad19b1aeddcf25a7f4`. It passed formatting,
diff hygiene, warnings-denied locked all-target/all-feature workspace Clippy, locked all-feature
workspace tests, the exact unchanged 271-row golden `--require-clean`, native/WASM parity,
all-feature WASM, TypeScript clean install/build/runtime tests, warnings-denied Rustdoc, benchmark
compilation, licence/package checks and Trunk 0.21.14 release assembly. The 256-moving-body sparse
crossover passed in 107.98 seconds.

M83 retained-preview p95 was 13.880 ms against 16 ms and exact terminal publication was 130.692 ms
against 750 ms. Fillet-radius, Offset-distance and relation-heavy-curve-control frame p95 values
were 4.827 ms, 6.654 ms and 17.961 ms; their exact terminal values were 5.680 ms, 8.143 ms and
95.628 ms, all within their independent ceilings.

Without rebuilding, the gate-produced `dist` was copied to
`/tmp/geosolve-m83-f007-uat.52r7H7`, byte-compared and frozen. Freeze metadata is in
`/tmp/geosolve-m83-f007-freeze-evidence.y5GbCJ`; the ordered manifest aggregate is
`bc04955f52ac14f3eba96637b23210772ab339e1a2f3ac60be59558bcb4c5973`. The directory is `0555`
and all seven entries are regular non-symlink files at `0444`:

```text
7acf06ec28c181468f26a92f6978af0f4b9d4f3205e076e602c517f00923d07f  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
f8519fe21e7edc0229555df8c89f280840583bc72c2208f691ad9b6cba93d9dd  geosolve-demo-web-119c7106611e4e18.js
112f69b748b8ba807f7a927dddcc64cbb34ade35260810442342582d111caa8a  geosolve-demo-web-119c7106611e4e18_bg.wasm
879f052749c88707c26ae42b417ecac4923b42590d4ec8297efe7f6a50809697  index.html
368d50605a0f14d4e67784e711cd5d920cd56a715fe82d09e68f2bd974caaf24  styles-4251c5b53d199c44.css
```

The existing 3/3 frozen Playwright suite passes against those bytes. Temporary served-byte
verification passed at the replacement listener;
`/tmp/geosolve-m83-f007-temp-verify.3CjjqM/results.tsv` has SHA-256
`9573901313adf09b23e93e27857639fa7bf96eb969121a6f22277cadb275b9ec`.

The new external frozen-browser harness passes 2/2 on both the temporary and retained endpoints:
six rectangle/shared-diagonal drops remain committed with exact Undo, and New clears populated
durable/transient state, selects Select and persists the empty workspace. The retained run used:

```bash
set -o pipefail
env -u FORCE_COLOR NO_COLOR=1 M83_BASE_URL=http://100.94.63.83:8080/ \
  NODE_PATH=/home/arduano/.npm/_npx/420ff84f11983ee5/node_modules \
  node /home/arduano/.npm/_npx/420ff84f11983ee5/node_modules/@playwright/test/cli.js \
  test --config=/tmp/m83-f007-pw/playwright.config.js 2>&1 | \
  tee /tmp/m83-f007-pw/final-artifact-run.log
```

The configuration SHA-256 is
`bfdd1d9920e8bcd49f4fe9853085450f7365b2546220712fc6f50a4431c8e8ad`, the specification SHA-256
is `7b36eaeef0600b07e6da410485ff8bbc8521da2da2a17984c6eeee1fd2df2b33`, and the final 554-byte,
9-line log SHA-256 is `a97160b3b4e57f0c822454f318210798dadb41c0e804e216acba1dbb5f64e9b7`.
Both rows passed in 6.2 seconds. A control run against the old F005 endpoint failed both rows with
the reported defects, proving harness sensitivity. This remains thin frozen-adapter parity; Rust
owner regressions are the semantic and solver oracle, and no repository E2E harness is restored.

Only after the temporary byte and 2/2 browser passes, `geosolve-m83-uat.service`, PID `4006665`,
began serving the frozen directory at `http://100.94.63.83:8080/`. Independent final evidence
`/tmp/geosolve-m83-f007-final-verify.nHhejs/results.tsv` has the same SHA-256 as the temporary
ledger, `9573901313adf09b23e93e27857639fa7bf96eb969121a6f22277cadb275b9ec`. Both ledgers cover `/`
and all seven files with HTTP 200, zero redirects, no `Location` or `Content-Encoding`, exact media
type/length/body, and root equality with `index.html`. Temporary services are retired; `:8080`
remains live for focused human UAT. GitHub Pages remains on accepted M81.
