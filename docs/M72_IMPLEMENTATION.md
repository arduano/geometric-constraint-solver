<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M72 implementation — Public workbench bulk fixes

Status: **complete and explicitly approved on 2026-08-15**. M72-F001 through M72-F004 are
implemented, clean-qualified and published. The complete-history secret scan, exact final hosted
artifact byte verification and public Chromium qualification pass. The supervising caller
approved the focused UAT and requested milestone closure.

Implementation commits:

- `9e38032` — clear stale retained Problems;
- `bcbaf6f` and `392637d` — keep interactive rectangles free-size and prove construction/resize
  history;
- `a0758db` — unify canvas tool options and exact-set Problems dismissal;
- `a1663d0` — add the pinned repository-prefixed Pages pipeline;
- `dc09b01` — make the rectangle regression warnings-denied and nominate the clean source;
- `6eb2c63` — build the Pages artifact inside the pinned Nix environment;
- `b700313` — remove split chevrons and make the unified options overlay persistent until a tool
  switch or explicit close-to-Select action.

## 1. Files and APIs

- `crates/geosolve-constraint-editor/src/coordinator.rs` clears prior computed setup errors at the
  start of each durable refresh. Same-sketch Undo/Redo and reload shortcuts now require accepted
  authority for the current input; a rejected live attempt is rebuilt from the selected checkpoint.
  Focused tests cover rejected-dimension recovery, suppression repair, current parameter/external
  failures, feature-only Undo and same-sketch reload after failed no-history attempts.
- `crates/geosolve-constraint-editor/src/lib.rs` changes only the interactive
  `ConstructionProposal::Rectangle` lowering. It removes the macro-generated fixed-point source
  and width/height dimensions through their ordinary owned-state removal path. The canonical
  `SketchDocument::add_rectangle` macro is unchanged.
- `crates/geosolve-constraint-editor/tests/m72_free_rectangle.rs` owns the exact rectangle
  topology, finite accepted geometry, independently validated residual, rank/DOF, projected resize
  and construction/resize Undo/Redo contract.
- `crates/geosolve-demo-web/index.html`, `styles.css` and `src/workbench/mod.rs` replace sidebar
  flyouts with one nonmodal bottom-left canvas surface. An internal `OptionOverlayKind` owns Equal,
  Tangent, Continuity, all five dimensions, Fillet, five conic-family tools, NURBS and Construction
  display. Option-bearing palette tools have no separate split-chevron: their centered main button
  implicitly opens the relevant family, and invoking the same family again is idempotent. The
  surface provides mutual exclusion, family-local controls and parsing, bounded scrolling and
  deterministic focus entry. It persists across blur, outside/canvas clicks, zoom and ordinary
  controls; switching tools closes or replaces it, while its `×` and Escape exit to Select and
  focus Select.
- The Problems card uses a presentation-only exact problem-set identity. Dismissal hides only the
  currently rendered set; a changed set opens automatically, and recovery removes the card without
  changing headless diagnostics, tree state or canvas markers.
- `.github/workflows/pages.yml` runs the complete release gate, performs a separate Trunk build
  using the Pages base path, validates exactly seven regular non-symlink files and deploys that
  artifact. Every third-party action and the Nixpkgs source is commit-pinned.
- The workbench footer exposes Source and License links. `README.md` links the approved public URL.

No new public Rust API, solver equation, residual, Jacobian, branch rule, priority, persistence
schema or golden case was added.

## 2. Mathematical and lifecycle behavior

F001 is an accepted-authority correction. Computed refresh state from a previous failed attempt is
discarded before evaluating the durable current input. A genuine failure produced again by the
current parameters or external snapshots remains visible. Undo, Redo, repair and reload retain the
ordinary transactional rule: rejected design intent may remain in history, but only geometry
accepted for the restored current input is authoritative.

F002 preserves four shared rectangle corners, four explicit line branches and four hard H/V rows.
It publishes no fixed-point source, dimension or private scalar. The focused accepted state has
finite coordinates, normalized hard residual at most `1e-9`, numerical rank four and right nullity
four. Projected corner movement changes width and height, and construction plus resize each owns
one normal history step. The separately public constrained rectangle macro retains its historical
anchor and dimensions.

F003 and the Problems disclosure are browser presentation changes over existing public coordinator
and audit data. Only the active option family is read or validated, so malformed hidden C2, conic
or NURBS fields cannot block an unrelated tool. Overlay lifetime follows tool intent rather than
transient DOM focus: non-tool interaction leaves the active family visible, another tool closes or
replaces it, and explicit close returns to Select. No browser-owned equation or accepted-state path
was introduced.

## 3. Commands run and outcomes

The following development qualification commands have run successfully on the committed M72
implementation:

```text
cargo fmt --all -- --check
git diff --check
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo clippy --locked -p geosolve-demo-web --lib --all-features -- -D warnings
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release --locked'
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
/tmp/tmp.ywhCssvJw5/actionlint .github/workflows/pages.yml
```

Outcomes:

- constraint-editor: **321/321 unit tests** and every integration/doc test pass; the focused
  `m72_interactive_rectangle_has_free_size_and_retained_history` regression passes **1/1**;
- demo-web: **104/104 library tests**, the decoder test and doc tests pass;
- focused warnings-denied web Clippy and the WASM target check pass;
- the unchanged canonical authoring/scene oracle remains **234/234 `PASS`** and passes survey,
  check and require-clean modes;
- formatting and diff hygiene pass;
- Trunk 0.21.14 produces seven files;
- a separate build with `--public-url /geometric-constraint-solver/` produced exactly seven regular
  non-symlink files with prefixed JavaScript, WASM and stylesheet references;
- `actionlint` passes for the Pages workflow.

Initial local Chromium checks using the ordinary release build passed at `1440x900` and
`1024x720`. They opened every option family and verified one active panel, subtype-specific
controls, containment, focus handling, invalid inactive-field isolation, Problems close/reopen,
Source/License links and browser-local scene persistence across reload. The same checks later
passed against the public endpoint. The focused 2026-08-15 UAT follow-up replaces the initial
dismissal-on-outside-interaction contract with the implicit, persistent overlay contract recorded
above and in `docs/M72_UAT.md`; that interaction passed local automated qualification and the
scoped human close decision. Its replacement publication and final public checks are recorded
below. Historical initial-run screenshots were recorded at `/tmp/m72-overlay-1440x900.png` and
`/tmp/m72-overlay-1024x720.png`, SHA-256
`96a3f2ed0fa845688d9acf7b9e24443e41d1511e2dbf448fc0e2f329533f0024` and
`77b7043953654cf2efab36f0e02928afcd571b7489a9cc2ad3aa3456ba2c3bd2` respectively.

### Historical initial qualification and publication checkpoint

The nominated clean source is commit `dc09b019704fe4a5cd48aff1ae838dfa52f36813`, tree
`38d79f5e05cb5274cc7eeb6bc6c0c2fac7d6f624`. The complete release gate ran from
`2026-08-14T22:20:18+10:00` through `2026-08-14T22:28:29+10:00` and exited successfully. Its
retained log is `/tmp/geosolve-m72-clean-gate.upGsYJ.log`, SHA-256
`7758b84585c28761414efaa20422d95c4e7f9717966bb173583e06244f6b6471`. The gate includes the
unchanged **234/234 `PASS`** golden at SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`, all locked workspace
tests, warnings-denied Clippy and rustdoc, native/WASM parity, performance, licence/package and
Trunk release assembly checks. The 256-moving-body sparse crossover passed in **152.55 seconds**.

Gitleaks 8.30.1 then scanned all 266 commits reachable across the complete Git history and reported
no leaks. Its empty `[]` report is `/tmp/geosolve-m72-gitleaks.SfZwXM.json`, SHA-256
`37517e5f3dc66819f61f5a7bb8ace1921282415f10551d2defa5c3eb0985b570`.

`https://github.com/arduano/geometric-constraint-solver` is public, its default branch is `main`,
and its homepage is `https://arduano.github.io/geometric-constraint-solver/`. The Pages API reports
`build_type: workflow`, `public: true` and `https_enforced: true`. The first push run,
`31800607957`, passed the complete release gate and Pages configuration, then failed only in
`Build the repository-prefixed Pages artifact` because `trunk` was invoked outside `nix-shell`
(`exit 127`); no deployment occurred. Commit `6eb2c63f6349851e70200570c9c2db07631acd3a`
changes only that workflow step to run the same build inside `nix-shell shell.nix`; its tree is
`fba3427e5e17023150a8252a154f097f56eb5964`.

Corrected run `31802816639` attempt 1 reached the final 256-moving-body performance check with all
geometry, residual, rank, sparse-backend and convergence assertions passing, but a shared runner
took `209.045026946s` against the unchanged `180s` ceiling. No artifact was published. The same
source and unmodified threshold passed on attempt 2 in **176.27 seconds**. The complete run then
passed its repository-prefixed build, seven-file validation, artifact upload and deployment jobs.
Run URL: `https://github.com/arduano/geometric-constraint-solver/actions/runs/31802816639`.

Uploaded Pages artifact `9221899077` (`github-pages`) was downloaded to
`/tmp/geosolve-m72-pages-artifact.CaJToc`; its ordered SHA-256 manifest aggregate is
`34c647dd29e6eee31cd58111db4082a2593b67f10b2d6735a26512a617889254`. The public root and all
seven files return HTTP 200 and byte-match that exact uploaded artifact. The HTML uses
`/geometric-constraint-solver/` asset URLs and the WASM response is `application/wasm`.

| Hosted artifact file | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `12279bff40f678cb04cafc11c09911ed9d76b164d690eb7c69a683d397da24cb` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-b2bffc86c277791a.js` | `7f7b472a8afd81ec6256d7b8ba9fc6f720b4d083d33e6ef2899a678f87fad088` |
| `geosolve-demo-web-b2bffc86c277791a_bg.wasm` | `4ade9ca0b5b01e4f58af731f5767332554a08484c8776ecc617215bfeaed9020` |
| `index.html` | `da90d24d05aec1ec992915cc69977409fb3b1b01d2974fa14286fbd4b7aa3528` |
| `styles-ab0ad01a4ddeccd5.css` | `ca9ee05bb09623f0f5814e9cdf1fe9711fe0d687c434998bec94fff5ae8cc9e7` |

The local and hosted WASM builds have environment-dependent WASM bytes and therefore different
Trunk asset names. Publication authority is the exact artifact built after the hosted complete
gate, uploaded by the workflow and matched above; JavaScript, stylesheet and legal/API document
content hashes are unchanged from local preflight. Finally, the public command
`M72_BASE_URL=https://arduano.github.io/geometric-constraint-solver/ node /tmp/m72_full_browser_check.mjs`
passed the initial candidate's option families, containment, focus/dismissal, Problems disclosure
and browser-local reload persistence at `1440x900` and `1024x720` with no console or page errors.
That run remains historical evidence for the initial candidate. The final run below supersedes it
for the revised overlay lifetime and close-to-Select contract.

### Final overlay follow-up qualification and publication

Accepted product source `b7003137960afb1b9d29c990d595df44bcd7c2d4`, tree
`80a0cb1c65ca3dd723968b5bf3a518f7dcbdca35`, passed the complete local release gate from a clean
worktree with:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

Every gate stage passed, including the unchanged **234/234 `PASS`** golden and the
256-moving-body sparse crossover in **128.40 seconds**. No solver equation, public API,
persistence format, residual, branch, priority or mathematical semantic changed in the overlay
follow-up.

Documentation-only approval source `2d1513912787445ff825836705158c2b563dc7ff`, tree
`01d58d1e926508692fdf6d8422f62f904bd0e388`, changes no product byte or semantic. GitHub Pages
workflow run
`https://github.com/arduano/geometric-constraint-solver/actions/runs/31862218764` passed against
that descendant. Its qualify-and-assemble job completed in **26m12s**, including the unchanged
256-body ceiling in **160.40 seconds**, and its deployment job completed in **9s**. Uploaded Pages
artifact `9241248173` was downloaded as
`/tmp/geosolve-m72-final-pages.zMCyo0/artifact.tar` and extracted to
`/tmp/geosolve-m72-final-artifact.FhTP9h`. The tar SHA-256 is
`e9b874809a2f93deae19b6d7ca435e45bda92bf861adcf2ca54786f4ee2b2702`; the ordered file-manifest
aggregate is `4a48f3cbc0269fdad2c4be91da015a6751eddbe4bdfe9bf97b5814674b9c7ff6`.

| Final hosted artifact file | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `12279bff40f678cb04cafc11c09911ed9d76b164d690eb7c69a683d397da24cb` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-b693b69337dd1646.js` | `39dbb8eb0693bc652dc8d4ee3c3327cc0ed7ac71b436e2b8bc593b64a09d0035` |
| `geosolve-demo-web-b693b69337dd1646_bg.wasm` | `351c338dc8595bfdcc3f6fc449d920c714e68bcca3d17314c53c5c4c1bd53f8d` |
| `index.html` | `ee841571a93e9b04c66830b8599fa3f2fb843539cfee3c93ea3baf21c73a0061` |
| `styles-437727272832bc26.css` | `9e4b1c6985f119cff35366119fbeef8abb2096b386a8db78a4cd730915316344` |

The public root and all seven files return HTTP 200 and byte-match artifact `9241248173` exactly.
JavaScript is served as `application/javascript`, WASM as `application/wasm` and CSS as `text/css`.
The final public command
`M72_BASE_URL=https://arduano.github.io/geometric-constraint-solver/ node /tmp/m72_full_browser_check.mjs`
passes at `1440x900` and `1024x720`. It covers all 16 option families, centered implicit buttons,
overlay persistence, idempotent re-invocation, tool-switch replacement, explicit close-to-Select,
containment, internal scrolling, hidden-field isolation, Source/License links and browser-local
reload persistence, with no console or page errors. Final screenshots
`/tmp/m72-overlay-1440x900.png` and `/tmp/m72-overlay-1024x720.png` have SHA-256
`b1130166a8107a913e7d4bdfc5147e587e75db8a0027951024efffc76fbaa97e` and
`6db6f54b3688165cf098898d33242955f017f3b94f041460e7c5f0035d02ba4a` respectively.

## 4. Acceptance passed

- F001 stale native/computed Problems recovery and genuine-current-failure retention pass at the
  retained coordinator owner.
- F002 free-size interactive rectangle topology, residual, rank/DOF, resize and history pass; the
  canonical macro is unchanged.
- F003's unified option surface, exact Problems dismissal and compact/ordinary desktop containment
  pass native/web tests and local Chromium review.
- The reviewed 234-row golden and all mathematical/persistence meanings remain unchanged.
- The complete clean release gate and full-history Gitleaks scan pass on the nominated source.
- The repository is public and workflow-based HTTPS Pages is configured.
- The Pages workflow and repository-prefixed seven-file artifact pass local static validation.
- F004's hosted complete gate, artifact validation/upload/deployment, public byte/media checks and
  public Chromium preflight pass.
- The accepted `b700313` overlay follow-up passes the final local complete gate; its docs-only
  `2d15139` approval descendant passes the hosted gate, exact public artifact verification and
  two-size Chromium contract.
- The supervising caller accepted the focused M72-U1 through M72-U4 scope and explicitly requested
  closure on 2026-08-15.

## 5. No remaining gate

M72 is complete with no blocker. The later `[skip ci]` closure record changes documentation only
and does not replace the product or publication authority above. M73 remains a proposed, inactive
milestone.
