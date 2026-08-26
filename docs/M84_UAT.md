<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 focused UAT — Optional code/GUI sketch authoring

Status: **M84-F005 and M84-F006 are implemented and focused-qualified but not accepted; refreshed
UAT U1-U14 is pending**. No row is accepted and no replacement snapshot is nominated. The direct-
authoring `41e65a4` snapshot, together with the initial, F003 and F004 snapshots, is withdrawn
historical evidence. Pages remains on accepted M83.

## Withdrawn candidate evidence

Qualified product source: `79078eca44a5af4de5cccd92bf6fee570c473624`; tree:
`05aefb0cbd3972d423f1713df1e58628b24ec216`. The clean Nix release gate passed on 2026-08-25;
its 6,107-line, 414,758-byte log `/tmp/geosolve-m84-release-gate.GOEuXP.log` has SHA-256
`0f50e6bcdf019c71d70497acc301dcdfd194db1142b248bcd469d0f3ed9efda0`. This includes
workspace Clippy/tests/Rustdoc, clean 271-row golden, actual WASM, both TypeScript suites, package
closure, benchmarks/performance, licensing and Trunk 0.21.14. M84-F001 and M84-F002 are closed by
owning-layer regressions.

The gate output was frozen without rebuilding at `/tmp/geosolve-m84-uat.aHw5ePSW`, with directory
mode `0555`, seven regular non-symlink files at `0444`, freeze evidence at
`/tmp/geosolve-m84-freeze-evidence.7loLImo2`, and ordered-manifest aggregate
`99beaf51ebb314aa26689427f970a75a516891efd20f68587c2a33c1b3a64f34`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
57918914596207c2a3aa27abae8cbce1e321a54d797620e0ea7e66900d940732  geosolve-demo-web-e0d2c05fe4bed1f4.js
15cdf9f3ecabd8c0b42e419009af43065686a2d66a8f6209a48ae57449bc172e  geosolve-demo-web-e0d2c05fe4bed1f4_bg.wasm
2afe27a4143da8521f07945cb0671e3412985b05d3ab035a26255196079776fe  index.html
9c4cc19e4ead8b15276095c81983cd0824f792e580e5929adc75f979264e2952  styles-5c4359127dc3a0bb.css
```

Local and Tailscale eight-path ledgers both have SHA-256
`dd8e6c1350f56cb6e7a483892a188187edc68ddcb63ee8ba9335401432ba8895`: every request returns
HTTP 200 with zero redirects, exact MIME/length/hash, no `Location`/`Content-Encoding`, and `/`
equals `index.html`. Focused Playwright passes 4/4 locally and 4/4 on Tailscale, including its
bounded-surface check at both required sizes; log hashes are
`54858c5a1f75cc2e286d07360ed8342c7f8a09290a8f3e7ee1c4d295afe91d9e` and
`37f289b62adc02362e8c34a1ea23377c2a3fc5446d43ac262a5b075c1652f4c0`.

Historical `geosolve-m84-uat.service` PID `2426265` served the immutable snapshot above at
`http://100.94.63.83:8080/`; it was retired only after replacement temporary byte and browser
verification passed. Those facts preserve historical mechanical evidence and do not accept any
row.

M84-F003 reproduction: draw an aligned rectangle, then a Segment between two rectangle corners.
The old source view shows a serialized `{ declaration, output, kind }` dependency rather than a
lexical `frame.corners.*` expression. This disproves the code-authoring claim and withdraws the
complete snapshot despite its earlier mechanical qualification.

## Withdrawn F003 replacement evidence

Qualified product source: `b9e67bad7f4935b1e0591ea4f149fae478b32675`; tree:
`7062806695e1e134c339cfa47903145d321f6350`. The exact clean gate

```bash
env -u GEOSOLVE_ALLOW_DIRTY -u NO_COLOR \
  nix-shell shell.nix --run ./scripts/release-gate.sh
```

ran on 2026-08-26 from 00:24:02 through 00:46:20.940662 AEST, approximately 22m19s, and exited 0.
Its 6,192-line, 414,397-byte log `/tmp/geosolve-m84-f003b-release-gate.log` has SHA-256
`eb05d3c1e460f5cb7be410dc44d0af0c4b4eaf4fd775423b676c77a84a433f90`. The unchanged 271-row
golden remains SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`;
the separate M84 ledger remains
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The gate output was frozen without rebuilding at `/tmp/geosolve-m84-f003-uat.mO67NI`. Source,
copied and frozen manifests are identical. The directory is `0555`; exactly seven regular non-
symlink files are `0444`. Complete evidence is retained at
`/tmp/geosolve-m84-f003-freeze-evidence.Ue4SCM`, and the ordered-manifest aggregate is
`38d356e9f727a4b690c1166dee3a36b1e0a5e59a2ad8bad1ca7243d889c6a617`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
8c06f0303535f09aa7bec53703136f5e29625c0aa193abff945bc867ab13af99  geosolve-demo-web-c14103084aedc965.js
178016828122190de25998de41a9a1991028a0180da0ff665a843fc170c9b85f  geosolve-demo-web-c14103084aedc965_bg.wasm
d8faa1ccc37a0758aaf8ba94d45cfd12661ba42bc4f80b35d754ae59897311fe  index.html
5b30ea9a86e4be495437705c2b2a9cb30800068ff5a27bcd61a408e13d7e1700  styles-4a93ed51c1144512.css
```

Temporary `:18084` and retained `:8080` eight-path HTTP ledgers were byte-identical at SHA-256
`438d747641522dd567e5790c56663f9bcb596147d48b843e1fe3367830822d0c`: every request returns
HTTP 200 with zero redirects, exact MIME/length/hash, no `Location`/`Content-Encoding`, and `/`
equals `index.html`. Existing browser qualification passes 4/4 and the dedicated F003 flow passes
1/1 on both endpoints. Temporary PID `3728373` and worktree PID `2872083` are retired. Retained
`geosolve-m84-uat.service`, PID `3736900`, served only this immutable snapshot at
`http://100.94.63.83:8080/`. M84-F004 withdraws these bytes from current UAT even if that endpoint
remains reachable.

The replacement emits lexical variable/member expressions, aliases exact native rectangle points,
labels low-level transport as Intent IR, provides a read-only Code preview and promotes into a real
persistent project whose dependent line follows managed rectangle edits. It omits only the exact
canonical fresh document foundation and normalizes only expansion-owned Segment branches. Raw
strings, DTOs, foreign-project and forged reserved-project references, wrong kinds and misspelled
members fail closed. These are historical automated qualification facts, not accepted human rows
or a current candidate.

M84-F004 reproduction: in an ordinary sketch, draw two connected lines and place a Fillet between
them. The complete managed projection rejected the unsupported computed Fillet, and presentation
responded to that conversion error by omitting Code entirely. The F004 contract adds lexical
Segment-to-Segment endpoint reuse and direct `$.computed.filletSet` with two ordered lexical
`NativeCurveSpanRef` parents plus complete persisted contact/branch state. The central descriptors
brand only line spans, rectangle edges and Polyline segments; a computed host Fillet arc rejects as
a direct parent in TypeScript and Rust. Radius accepts a positive finite model-unit number or
branded `mm(...)`. The checked-in managed-v1 line/Horizontal/Vertical/line/Fillet fixture is shared
by TypeScript compile, Rust parse and cold materialization. Unsupported complete projections still
fail closed,
but Code remains discoverable and displays an escaped read-only diagnostic, Intent IR remains
available and Promote is absent. Focused Rust owner/bootstrap/direct-lowering/workbench,
TypeScript and UI tests pass. Exact browser replay also found the ordinary axis-aligned mouse path's
inferred Horizontal and Vertical declarations were outside bootstrap closure. They now project as
lexical managed constraint calls over `line.span`/`line2.span`, preserve suppression and lower to
the existing constraint kinds; neither relation is omitted and no new equation is introduced.
Retained-failed intent is also authenticated against the prior accepted semantic identity before
serialization; a mismatch keeps Code visible as unavailable and removes Promote rather than
constructing hybrid source.

## Withdrawn historical F004 candidate evidence

Qualified product source: `c2cf160d3a7d5065e582f2ba982881380d2b871c`; tree:
`94a178699f9b2e8bd2a6497c9b0334d43cad2b20`. The complete clean gate ran on 2026-08-26 from
12:37:30.923 through 12:55:14.714 AEST, exited 0 in 1,064 seconds, and produced the 6,118-line,
412,411-byte log `/tmp/geosolve-m84-f004b-release-gate.log` with SHA-256
`f4c005912392c11cab6600706876c37dab855f0a59c0b7c06565c242b014d6fd`. It includes workspace
tests/Clippy/Rustdoc, unchanged golden, actual WASM, TypeScript, package closure, benchmarks,
licensing and Trunk. The 271-row golden remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate M84 ledger
remains `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact no-rebuild seven-file output is frozen at
`/tmp/geosolve-m84-f004-hv-uat.FF5RFBZe`, with directory mode `0555`, regular files at `0444`,
complete evidence at `/tmp/geosolve-m84-f004-hv-freeze-evidence.FiCfIvir`, and ordered-manifest
aggregate `f34c46ee5876c4bdb458863cc90c6c6b25281cc8e44f89c8d00eba0f16ca5bbc`. Temporary and retained
eight-path HTTP ledgers are byte-identical at SHA-256
`54efcd30699a8632b868d753af88b1f17433c284201688834f7a0f35e2598153`: every path returns HTTP
200 with zero redirects, exact MIME/length/body, no `Location` or `Content-Encoding`, and `/`
equals `index.html`. Baseline 4/4, F003 1/1 and F004 2/2 browser suites pass on both endpoints.

Historical `geosolve-m84-uat.service` PID `3316682` served only those immutable bytes at
`http://100.94.63.83:8080/`. It was retired only after the direct-authoring replacement passed
temporary exact-byte and browser verification. The later amendment keeps this snapshot withdrawn
because it predates amended scope; it does not accept a row or authorize M84 Pages publication.

## Withdrawn direct code-authoring nomination

A canonical fresh workspace now exposes one **Start from code** action and four genuine project
cards. Starting creates **Untitled code sketch** as a distinct artifact-free `Authored` project;
its complete editable source declares a rectangle and diagonal through lexical
`frame.corners.*` references. Focused Rust owner tests pass valid Apply, whole-source replacement,
exact native endpoint aliasing, retained-invalid canvas/persistence, Undo/Redo and hostile origin
rejection. The fresh landing additionally requires exact current/accepted semantic parity and
independently validated empty native authority. No fifth bundled project, golden row, custom
artifact, JavaScript execution or solver equation is introduced.

Exact product source `41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, passes the clean command

```bash
env -u GEOSOLVE_ALLOW_DIRTY -u NO_COLOR \
  nix-shell shell.nix --run './scripts/release-gate.sh' 2>&1 | \
  tee /tmp/geosolve-m84-authored-release-gate.log
```

from 14:55:50.676940562 through 15:12:22.475576111 AEST on 2026-08-26, exit 0 in
991.798635549 seconds. Its final Trunk `INFO ✅ success` marker was checked independently of the
tee pipeline. The 6,146-line, 415,754-byte log
`/tmp/geosolve-m84-authored-release-gate.log` has SHA-256
`34bf408f6a565dec5705745f916eb628397a549b4a7002d865269e8d167e179f`. It includes formatting and
diff hygiene, warnings-denied Clippy/Rustdoc, locked workspace tests/doctests, unchanged 271-row
golden, separate four-demo M84 ledger, actual WASM, both TypeScript packages, package closure,
performance/benchmarks, licensing and Trunk 0.21.14. The worktree was clean before and after the
gate. The golden remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the M84 ledger remains
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact gate output was copied without rebuilding to
`/tmp/geosolve-m84-authored-uat.ZYQQyBQQ`. Source, copied and frozen manifests match; the directory
is `0555` and exactly seven regular non-symlink files are `0444`. Complete evidence is retained at
`/tmp/geosolve-m84-authored-freeze-evidence.ngNf7jxZ`; its ordered-manifest aggregate is
`6f82bb261057916f737110cb6533da128d1afde72d1b2f5584f937acdd1a54b1`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
26f7601677f65392bcde98f30cbb94f01f50d9ed45895c856a30d459ff38e715  geosolve-demo-web-c98440de375793fc.js
89e70d8dcd84b648a7b0c7a252db7f927fa3d6cd81937bc4744c011b1d738241  geosolve-demo-web-c98440de375793fc_bg.wasm
0b2996e0da06a9a57ae42d977bff1a4798833c53db34fa97fdb424e589ae7f14  index.html
81e24b427990f181b099e7b751dee6739d68024001e3cef592a8be3faed435db  styles-8eda25752cd33a23.css
```

Temporary `:18086` and retained `:8080` eight-path HTTP ledgers are byte-identical at SHA-256
`1450e4c6d8585ba17dee56feafaf96869c45f764f3400280a0dc37581f9b4eee`. `/` plus all seven files
return HTTP 200, zero redirects, exact MIME/length/hash, no `Location` or `Content-Encoding`, and
`/` equals `index.html`. Against those same frozen bytes, direct-authored lifecycle passes 3/3,
baseline passes 4/4, F003 passes 1/1 and F004 passes 2/2 on both endpoints. The direct suite covers
both `1440x900` and `1024x720`, zero landing overflow, Start/edit/Apply, retained-invalid canvas,
Undo, reload/repro and sample-card routing. Automated browser checks are mechanical nomination
evidence, not supervising-user UAT.

The temporary service and historical F004 PID `3316682` are retired. The `41e65a4` service/snapshot
record is historical only: M84-F005 withdraws it from UAT because it predates collaborative draft
overlay and semantic-authority scope. It is not a replacement candidate and must not be used to
claim a UAT row, acceptance or Pages authority.

| Release state | Status |
| --- | --- |
| M84-F005/F006 clean qualification, immutable freeze and exact Tailscale replacement | pending |
| Supervising-user refreshed M84-U1 through M84-U14 | pending |
| GitHub Pages publication, service retirement and M84 closure | pending |

Run the ordinary desktop workbench at approximately
`1440x900` and `1024x720`. Use actual code-project samples rather than importing equivalent flat
scenes. Direct tests, not visual judgment, own exact identities, generations, residuals, payload
bounds and byte parity.

M84-F006 is automated audit hardening rather than an additional hands-on row. Imported-session and
draft bounds, typed Reset/Restore tokens, exact direct/generated owner pruning, generated
detachment/rebinding/cancellation, repeated-drag Undo/Redo and bit-exact duplicate/conflict handling
remain acceptance prerequisites for U14 and the clean gate. They do not reduce U1-U14 or create a
new U15.

## Scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M84-U1 | Open **Rounded polyline · dynamic corners**. Inspect six keyed vertices, five spans and four Fillets; insert `crest`, reorder/remove it and Undo/Redo. | Counts become seven/six/five after insertion; unaffected members keep their visible identity; reorder is non-semantic; removal/tombstone and Undo are exact. | pending |
| M84-U2 | Edit the adaptive Fillet radius through its lens, drag a permitted generated/free point, Reset to code, toggle open/closed and enter an impossible radius. | Lens edits the managed invocation; drag creates one explicit override; reset removes it; cardinality adapts; invalid geometry retains source/diagnostic above the prior accepted canvas. | pending |
| M84-U3 | Open **Typed panel · keyed Fillets** and inspect rectangle outputs plus `fillets({ lowerLeft, upperRight })`. Run the shipped type fixtures. | Corners/edges/profile and both Fillets have meaningful typed fields; misspelling, raw IDs, cross-project refs and point/curve/corner mismatch fail compilation. | pending |
| M84-U4 | Open **Braced frame · GUI → code → GUI**. Edit/drag the GUI rectangle and code-generated brace, then constrain or dimension `brace.diagonals.rising`. | Both authoring directions update one accepted scene and one managed source; ownership/selection is truthful; ordinary native constraints remain fast and editable. | pending |
| M84-U5 | Mix code Apply, Inspector/canvas edits, organization changes, lens edits, overrides and Reset; walk Undo/Redo after each. | Every action creates exactly one coherent history entry; source, expansion, nested intent, accepted scene and overrides restore together with no mirrored editor history. | pending |
| M84-U6 | Edit whitespace/comments/unowned formatting, then attempt unsupported syntax, stale-span Apply, duplicate keys and a dangling generated-output removal. | Preserved spans remain byte-identical; unsupported/stale/duplicate/dangling edits diagnose exactly and publish no unintended source, graph or scene change. | pending |
| M84-U7 | Inspect **Mounting plate · reusable AI-authored module**, edit its managed dimensions/placement, and compare the helper before/after. | Rounded profile and `nw/ne/se/sw` holes adapt; `patches/*.patch.ts` stays byte-identical; artifact/ownership/lens state remains understandable. | pending |
| M84-U8 | Save/reload and Copy/New/Load repro for valid and retained-invalid code projects; remove/tamper an artifact and try a corrupt/oversized payload. | Complete offline authority/history returns; invalid intent keeps its accepted scene; missing/tampered/corrupt/oversized inputs reject before atomic replacement. | pending |
| M84-U9 | Drag repeatedly in the largest code sample while watching source/History, including rapid reversals and a rejected final pointer sample. | Preview remains at the M83 interaction quality; pointer frames do not parse/expand or churn panels; release commits the newest accepted preview once and exact Undo restores it. | pending |
| M84-U10 | Start a plain M83 workspace/build without code support, then a code-enabled one. Inspect the browser file/artifact UI at both sizes. | Plain solver/editor behavior and persistence remain available without code-module linkage; code UI is polished and bounded, custom files are clearly read-only, and it does not pretend to be a general IDE. | pending |
| M84-U11 | In an ordinary sketch, draw an aligned rectangle and a line between two rectangle corners; inspect Intent IR and Code, then promote the preview and edit the rectangle. | Intent IR is labelled as transport data; Code uses lexical `frame.corners.*` values with no serialized dependency DTO; promotion creates one real managed project and the dependent line follows later edits. | pending |
| M84-U12 | In an ordinary sketch, draw a Horizontal line, continue it with a Vertical line and Fillet their corner. Inspect Code, promote and reload; edit radius once as a model-unit number and once with `mm(...)`. Then separately try a computed Fillet arc as a direct parent and add another unsupported declaration. | The `line2` declaration contains lexical `start: line.end`; the preview contains `$.constraint.horizontal` over `line.span`, `$.constraint.vertical` over `line2.span`, and `$.computed.filletSet` with those lexical parent spans, no raw ID/DTO and complete explicit branch/contact fields. Both existing axis constraints survive promotion. Line spans are native-branded; a computed host arc cannot be a parent. Positive finite model-unit and branded-mm radii work; forged/other-unit/nonpositive/nonfinite values reject. Promote/reload retains finite Current Fillet geometry with independently validated Hard residual `<= 1e-9`. Unsupported scenes keep Code visible with an escaped truthful diagnostic and Intent IR fallback, while Promote is absent. | pending |
| M84-U13 | Click **New**, open **Code**, inspect the starter and four example cards, then choose **Start from code**. Edit the starter rectangle, Apply, enter a collapsed invalid rectangle, Undo/Redo, reload/repro, return to New, and open a sample card. Check both desktop sizes. | The fresh surface has one direct starter action and four genuine projects without overflow. Start creates **Untitled code sketch** with editable artifact-free `sketch.ts`, lexical `frame.corners.*` dependencies and no Promote/fabricated GUI history. Valid edits update finite accepted geometry; invalid edits retain the prior canvas and diagnostic; Undo/Redo/reload/repro preserve authored authority; New restores the starter and cards open genuine projects. | pending |
| M84-U14 | In a code project, drag a literal point, each rectangle-corner role, then a shared point with no semantic selection, with its producer selected and with its referenced consumer selected. Repeat the detached-consumer drag, Undo/Redo and Reset. Then select/delete a managed declaration with dependents and one generated child. Repeat deletion with a dirty source draft, retained code failure, a target retained across another revision and a GUI-owned selection. Finally remove a drafted owner in a source edit that also fails native publication, then reload and Undo. | Each accepted release records one bounded point-only semantic overlay/history entry and preserves a finite independently validated accepted scene. Point-seed precedence is typed overlay draft > legacy generated override > managed source seed; Reset restores the coupled bundle/lower tier. No preference or producer selection keeps consumers attached; unique consumer selection detaches only it, truthfully permits Segment identity replacement, rebinds surviving code/GUI dependents and remains repeat-draggable/Undoable. Rectangle coupling is atomic. Selection/deletion resolves through an exact session + accepted alias + semantic-owner token, not a hashed `code.*` alias. Managed deletion rewrites the exact source/code-owned closure; generated-child deletion is reversible suppression. Dirty/failed/stale/GUI-owned cases reject without mutation. A retained structural/native failure keeps its deterministic owner-pruned attempted overlay above the exact accepted overlay/canvas; reload and Undo preserve both. Persisted optional envelopes report `geosolve-sketch-code-session-v2` and `geosolve-code-workbench-v2`, while plain M83 workspace-v8 is unchanged. | pending |

Any JavaScript runtime solve, browser `eval`, raw code-facing ID, cross-project retarget, ordinal
identity churn, silent cascade, duplicate history, blank accepted scene or pointer-frame expansion
withdraws the candidate and opens an owning-layer regression.

## Final disposition

- Supervising-user UAT: refreshed M84-U1 through M84-U14 pending; every previous candidate,
  including direct-authoring `41e65a4`, remains withdrawn historical evidence.
- M84 GitHub Pages publication: prohibited before explicit approval.
- Replacement Tailscale nomination: pending after clean F005/F006 qualification and no-rebuild
  freeze.
