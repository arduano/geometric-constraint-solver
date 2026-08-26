<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 focused UAT — Optional code/GUI sketch authoring

Status: **M84-F004 is mechanically qualified and its immutable replacement is nominated; focused
UAT U1-U12 is pending**. No row is accepted. The F003 snapshot is withdrawn historical evidence.
Pages remains on accepted M83. Both earlier candidates are preserved only as historical defect
evidence.

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

## Current F004 candidate evidence

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

Retained `geosolve-m84-uat.service`, PID `3316682`, serves only those immutable bytes at
`http://100.94.63.83:8080/`. The temporary replacement and obsolete pre-axis F004 services are
retired. This nominates the candidate for the scorecard below; it does not accept a human row or
authorize M84 Pages publication.

Run the ordinary desktop workbench at approximately
`1440x900` and `1024x720`. Use actual code-project samples rather than importing equivalent flat
scenes. Direct tests, not visual judgment, own exact identities, generations, residuals, payload
bounds and byte parity.

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

Any JavaScript runtime solve, browser `eval`, raw code-facing ID, cross-project retarget, ordinal
identity churn, silent cascade, duplicate history, blank accepted scene or pointer-frame expansion
withdraws the candidate and opens an owning-layer regression.

## Final disposition

- Supervising-user UAT: M84-U1 through M84-U12 pending against the nominated F004 replacement;
  both earlier candidates remain withdrawn historical evidence.
- M84 GitHub Pages publication: prohibited before explicit approval.
- Replacement Tailscale nomination: complete at `http://100.94.63.83:8080/`; retirement is
  prohibited until accepted Pages bytes are independently verified.
