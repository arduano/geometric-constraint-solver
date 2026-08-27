<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 focused UAT — Optional code/GUI sketch authoring

Status: **M84-F011 withdraws the clean-qualified immutable F010 nomination while its ninth PC water
manifold project and PNG export undergo replacement qualification; refreshed human UAT U1-U16 is
pending**. No row is accepted. Exact source
`cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, and snapshot
`/tmp/geosolve-m84-f010-uat.7R5eXQoz` are historical rollback authority and remain served only
until an exact F011 replacement is verified. F009 source `c74651c`, tree
`a904584`, and snapshot `/tmp/geosolve-m84-f009-uat.q8cKIN3v` are withdrawn historical defect
evidence. F007 source `cc2f05e`, the direct-authoring `41e65a4` snapshot, combined F005/F006 source
`ff2e142` and the initial/F003/F004 snapshots are likewise historical. M84 remains active and
unaccepted; Pages remains on accepted M83.

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

The later frozen candidate at source `ff2e142` is also withdrawn. M84-F007 reproduced a legitimate
multi-frame rectangle drag rejection when terminal classification copied coupled solver motion into
multiple same-tier semantic writes. The provisional correction authenticates one point lens at
pointer-down and historically rematerialized only that seed at release. M84-F010 supersedes this
single-seed durability rule while retaining the exact authentication rule. Its pending route stores the exact
`CodeSessionIdentity`, pointer and lens; only that pointer's dedicated terminal publisher consumes
it. Generic saves reject without consumption; foreign/reentrant preparation and foreign terminals
preserve the route. Every durable sidebar/code/history route cancels internal capture before
mutation even if the viewport is unavailable, and a defensive generic save preserves an unexpected
live route without restoring beneath it. No-motion release/cancel is history-neutral; exact stored-
session mismatch and Apply/Undo cannot revert newer authority. The earlier release-WASM/browser
matrix passed 14/14. Post-audit web library 270/270,
sketch-code suites and focused Clippy/WASM checks pass. This is historical focused evidence; the
clean replacement nomination is recorded below and no UAT row is claimed by automation.

## Historical F007 replacement nomination

Exact product source `cc2f05ed97500f4bae4c0da6839362dbbc8c2e53`, tree
`6b8fc417ac9464843ac14fb350a8e5f794b1cdb1`, passes the complete clean gate from 00:52:15 through
01:08:51 AEST on 2026-08-27. The 6,194-line, 419,126-byte log
`/tmp/geosolve-m84-f007-release-gate.fn6ZHE.log` has SHA-256
`40c73a8856df0905e85e2b877a82db0e0e81da583764ccc18d729e268f01763b` and ends with the Trunk
0.21.14 success marker. It includes formatting/diff hygiene, warnings-denied workspace
Clippy/Rustdoc, locked tests/doctests, unchanged 271-row golden, actual WASM, both TypeScript
packages, package closure, benchmarks/performance, licensing and release Trunk. The unchanged
golden and four-demo ledger hashes are
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact gate output was frozen without rebuilding at
`/tmp/geosolve-m84-f007-uat.KgW8fpLf`. Source, copied and frozen manifests match; the directory is
`0555` and exactly seven regular non-symlink files are `0444`. Ordered-manifest aggregate is
`8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`; complete evidence is at
`/tmp/geosolve-m84-f007-freeze-evidence.rP5rQcTG`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
1cdaa482960063be48a3175941caa61b2120dfa36c61bfd87190efef3629ab5d  geosolve-demo-web-82d5c542ca4d046b.js
6ae0f1208912c7a9050c7cc5729381aad992242b213989bbea0787bc69fd37c4  geosolve-demo-web-82d5c542ca4d046b_bg.wasm
92cbb926b4dfb36af12ae4d7e549006fd0f699e8b99662062db101e73b0f800f  index.html
81e24b427990f181b099e7b751dee6739d68024001e3cef592a8be3faed435db  styles-8eda25752cd33a23.css
```

Temporary `:18087` and retained `:8080` eight-path ledgers are byte-identical at SHA-256
`efa609c6bac127753336c3634730b81bed04699a25c6394ab039c7f06b0b2b64`: every path returns HTTP
200, zero redirects, exact MIME/length/hash, no `Location`/`Content-Encoding`, and `/` equals
`index.html`. Baseline 4/4, direct-authored 3/3, F003 1/1, F004 2/2 and F005-F007 4/4 browser
suites pass on both endpoints. Browser coverage directly exercises both desktop sizes,
no-selection producer drag, selected producer, selected-consumer detachment, repeated drag,
Undo/Redo/reload/Reset, managed deletion and generated-child suppression. Generic/foreign/stale
terminal and bit-conflict adversarial internals remain Rust/WASM-owned because no public browser
gesture exists for them.

Only after temporary byte/browser verification passed was withdrawn PID `4081080` retired.
Temporary PID `34895` is retired. `geosolve-m84-uat.service`, PID `62376`, served only the exact
snapshot at `http://100.94.63.83:8080/`. The F009 replacement below supersedes it; PID `62376` is
retired. This historical evidence accepts no UAT row.

## Withdrawn F009 replacement nomination

Exact product source `c74651cc82506e31926042df65a1eeec08a6af9d`, tree
`a904584410ca9a8cd3112d17ad70c0e84c29e8d9`, passes the complete clean gate. The 6,218-line,
421,590-byte log has SHA-256
`c9b743c8f95d6df7706b04e2d820ac67426f1b11ec447d2bffdc08cd1fe0f6f1`. The unchanged 271-row
golden and eight-demo ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

The exact no-rebuild seven-file output is frozen at
`/tmp/geosolve-m84-f009-uat.q8cKIN3v`, directory/files `0555`/`0444`, ordered-manifest aggregate
`23f2f839f2a3be6b722ae26cb548f0a19ce2f3d6afac90d5f913938a042d1c1f`. Complete evidence is at
`/tmp/geosolve-m84-f009-freeze-evidence.3FoVTQ6m`. Temporary `:18089` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`add827e88d17735cfb6cb0bbecec885f5680db0bd11b67bb591673d566b90676`; baseline 4/4,
direct-authored 3/3, F003 1/1, F004 2/2 and F005-F007 4/4 browser suites pass on both endpoints.
Every sample card opens fitted finite geometry at `1440x900` and `1024x720`.

Temporary PID `3943194` and superseded F007 PID `62376` are retired. M84-F010 withdraws those
bytes. Historical retained PID `3965271` and temporary development PID `238809` are retired; the
immutable F009 snapshot remains preserved. This evidence does not accept U1-U14.

## Historical F010 replacement nomination

Exact product source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passed the clean release gate from
15:58:23.055857854 through 16:17:18.303733569 AEST on 2026-08-27, exit 0. Its 6,209-line,
420,425-byte log `/tmp/geosolve-m84-f010-gate.9NvAi3z5/release-gate.log` has SHA-256
`bf57345266005a85b6da20f1105c3cf126d2492ba91e413ef0f07c5d38d3b28a` and ends with final Trunk
success. The unchanged 271-row golden and eight-demo M84 ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

The exact seven-file gate output was frozen without rebuilding at
`/tmp/geosolve-m84-f010-uat.7R5eXQoz`, directory/files `0555`/`0444`, ordered-manifest aggregate
`ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`. Complete evidence is at
`/tmp/geosolve-m84-f010-freeze-evidence.sXWXNG0Z`. Temporary `:18091` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`57f2f4c2b47a11db8fc76a7f6a2e3d30555cb36e96b191081454a4c47fb85cbe`; all eight paths have exact
bytes and MIME. Focused Compass 1/1 and the carried 14/14 browser matrix pass on both endpoints.
The Compass scenario performs six center drags, proves exact release and
+50/+250/+500/+1000 ms positions, keeps all four spokes attached, requires finite accepted
authority and verifies exact reload. Retained focused spec/config SHA-256 values are
`4b97f570d5122a353b4ee104b26ea46427ca1c3b79aa5a8d35fee7875302dab0` and
`c0900c1132352ed9471321a2cf5727baf2c004d8eebf1a1bcd8a43146289df9c`.

`geosolve-m84-uat.service`, PID `650971`, serves only the immutable F010 snapshot from that
snapshot working directory at `http://100.94.63.83:8080/`. Historical F009 PID `3965271` and
temporary F010 PIDs `238809`/`621532` are retired; the F009 snapshot is preserved. These automated
facts historically nominated F010 but accept no UAT row. M84-F011 withdraws that nomination.

| Release state | Status |
| --- | --- |
| M84-F007 clean qualification and historical freeze | superseded |
| Eight-demo/M84-F008/F009 clean replacement qualification and immutable freeze | withdrawn by F010 |
| M84-F010 proportional owner/WASM/mutable-browser qualification | complete |
| M84-F010 clean qualification and immutable replacement freeze | complete; withdrawn by F011 |
| M84-F011 manifold/PNG focused qualification | complete on mutable local build; no UAT row accepted |
| M84-F011 clean qualification and immutable replacement freeze | pending |
| Supervising-user refreshed M84-U1 through M84-U16 | pending |
| GitHub Pages publication, service retirement and M84 closure | pending |

Run the ordinary desktop workbench at approximately
`1440x900` and `1024x720`. Use actual code-project samples rather than importing equivalent flat
scenes. Direct tests, not visual judgment, own exact identities, generations, residuals, payload
bounds and byte parity.

M84-F006, M84-F007, M84-F009 and M84-F010 are automated authority hardening rather than additional
hands-on rows. Imported-session and
draft bounds, typed Reset/Restore tokens, exact direct/generated owner pruning, generated
detachment/rebinding/cancellation, repeated-drag Undo/Redo and bit-exact duplicate/conflict handling
remain acceptance prerequisites for U14 and the clean gate. U14 must additionally exercise
multi-frame no-selection and selected-producer drags and verify that only the pointer-down semantic
lens authorizes terminal overlay publication while the complete solver-coupled movement closure is
persisted atomically and selected-consumer detachment remains local. The Compass center must remain
at the exact accepted release through at least +500 ms and reload, with every spoke attached. These
requirements remain part of U14. F011 adds distinct U15 manifold-dogfood and U16 PNG-export rows.
M84-F009 additionally requires exact renamed/nested and mapped `result_output` regressions plus the
all-eight automated output-kind audit: each declared reference kind and expanded target kind must
match the reviewed catalog, including Mounting Plate `plate.profile` as Profile rather than the
`ne` Point. It adds no hands-on row.

## Scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M84-U1 | Open **Rounded polyline · dynamic corners**. Inspect six keyed vertices, five spans and four visibly distinct radius-4 Fillets; insert `crest`, reorder/remove it and Undo/Redo. | Every Fillet is visibly clear of point markers. Counts become seven/six/five after insertion; unaffected members keep their visible identity; reorder is non-semantic; removal/tombstone and Undo are exact. | pending |
| M84-U2 | Edit the adaptive Fillet radius through its lens, drag a permitted generated/free point, Reset to code, toggle open/closed and enter an impossible radius. | Lens edits the managed invocation; drag creates one explicit override; reset removes it; cardinality adapts; invalid geometry retains source/diagnostic above the prior accepted canvas. | pending |
| M84-U3 | Open **Typed panel · keyed Fillets** and inspect rectangle outputs plus `fillets({ lowerLeft, upperRight })`; inspect `wire.vertices`/`wire.segments` in **Lantern garland**. Run the shipped type fixtures. | Corners/edges/profile, both Fillets and Polyline keyed Point/CurveSpan roots have meaningful typed fields; misspelling, raw IDs, cross-project refs and point/curve/corner mismatch fail compilation. | pending |
| M84-U4 | Open **Braced frame · GUI → code → GUI**. Edit/drag the GUI rectangle and code-generated brace, then constrain or dimension `brace.diagonals.rising`. | Both authoring directions update one accepted scene and one managed source; ownership/selection is truthful; ordinary native constraints remain fast and editable. | pending |
| M84-U5 | Mix code Apply, Inspector/canvas edits, organization changes, lens edits, overrides and Reset; walk Undo/Redo after each. | Every action creates exactly one coherent history entry; source, expansion, nested intent, accepted scene and overrides restore together with no mirrored editor history. | pending |
| M84-U6 | Edit whitespace/comments/unowned formatting, then attempt unsupported syntax, stale-span Apply, duplicate keys and a dangling generated-output removal. | Preserved spans remain byte-identical; unsupported/stale/duplicate/dangling edits diagnose exactly and publish no unintended source, graph or scene change. | pending |
| M84-U7 | Tour **Mounting plate**, **Lantern garland**, **Suspension bridge**, **Compass rose** and **Neon manifold**. Inspect Mounting Plate `plate.profile`, edit representative managed dimensions/placements and compare custom helpers before/after. | `plate.profile` is the Profile rather than a Point; rounded profile/holes, adaptive bulbs/Fillets, cables/stays, ring/markers and explicit Neon bends remain finite and understandable; `patches/*.patch.ts` stays byte-identical and artifact/ownership/lens state is truthful. | pending |
| M84-U8 | Save/reload and Copy/New/Load repro for valid and retained-invalid code projects; remove/tamper an artifact and try a corrupt/oversized payload. | Complete offline authority/history returns; invalid intent keeps its accepted scene; missing/tampered/corrupt/oversized inputs reject before atomic replacement. | pending |
| M84-U9 | Drag repeatedly in **Suspension bridge** while watching source/History, including rapid reversals and a rejected final pointer sample. | Preview remains at the M83 interaction quality; pointer frames do not parse/expand or churn panels; release commits the newest accepted preview once and exact Undo restores it. | pending |
| M84-U10 | Start a plain M83 workspace/build without code support, then a code-enabled one. Inspect the browser file/artifact UI at both sizes. | Plain solver/editor behavior and persistence remain available without code-module linkage; code UI is polished and bounded, custom files are clearly read-only, and it does not pretend to be a general IDE. | pending |
| M84-U11 | In an ordinary sketch, draw an aligned rectangle and a line between two rectangle corners; inspect Intent IR and Code, then promote the preview and edit the rectangle. | Intent IR is labelled as transport data; Code uses lexical `frame.corners.*` values with no serialized dependency DTO; promotion creates one real managed project and the dependent line follows later edits. | pending |
| M84-U12 | In an ordinary sketch, draw a Horizontal line, continue it with a Vertical line and Fillet their corner. Inspect Code, promote and reload; edit radius once as a model-unit number and once with `mm(...)`. Then separately try a computed Fillet arc as a direct parent and add another unsupported declaration. | The `line2` declaration contains lexical `start: line.end`; the preview contains `$.constraint.horizontal` over `line.span`, `$.constraint.vertical` over `line2.span`, and `$.computed.filletSet` with those lexical parent spans, no raw ID/DTO and complete explicit branch/contact fields. Both existing axis constraints survive promotion. Line spans are native-branded; a computed host arc cannot be a parent. Positive finite model-unit and branded-mm radii work; forged/other-unit/nonpositive/nonfinite values reject. Promote/reload retains finite Current Fillet geometry with independently validated Hard residual `<= 1e-9`. Unsupported scenes keep Code visible with an escaped truthful diagnostic and Intent IR fallback, while Promote is absent. | pending |
| M84-U13 | Click **New**, open **Code**, inspect the starter and nine example cards, then choose **Start from code**. Edit the starter rectangle, Apply, enter a collapsed invalid rectangle, Undo/Redo, reload/repro, return to New, and open every sample card. Check both desktop sizes. | The fresh surface has one direct starter action and nine genuine projects without overflow. Every card opens finite fitted visible geometry. Start creates **Untitled code sketch** with editable artifact-free `sketch.ts`, lexical `frame.corners.*` dependencies and no Promote/fabricated GUI history. Valid edits update finite accepted geometry; invalid edits retain the prior canvas and diagnostic; Undo/Redo/reload/repro preserve authored authority. | pending |
| M84-U14 | In code projects, drag a literal point and each rectangle-corner role through at least two preview frames; also drag a Lantern vertex, Bridge tower peak, Compass center/spoke and Neon shared endpoint. On a shared rectangle point, repeat a multi-frame drag with no semantic selection and with its producer selected, then drag it with its referenced consumer selected. During one pending route try a generic save, foreign terminal and foreign/reentrant preparation; also verify no-motion release/cancel and stale terminal after Apply/Undo. Repeat the detached-consumer drag, Undo/Redo and Reset. Then select/delete a managed declaration with dependents and one generated child. Repeat deletion with a dirty source draft, retained code failure, a target retained across another revision and a GUI-owned selection. Finally remove a drafted owner in a source edit that also fails native publication, then reload and Undo. | Each accepted release records one bounded point-only semantic overlay/history entry and preserves a finite independently validated accepted scene. Lantern bulbs/Fillets, Bridge cables/stays, Compass ring/markers and Neon bends remain attached/current. The Compass center remains at the exact release through at least +500 ms and reload. Pointer-down authenticates exactly one semantic point lens and stores its exact `CodeSessionIdentity` plus pointer; only the dedicated authenticated terminal publisher may consume it. Generic save, foreign terminal and foreign/reentrant preparation reject while preserving the route. Non-pointer durable code actions invalidate it; no-motion release/cancel is history-neutral; stale terminal after Apply/Undo cannot revert newer accepted authority. Terminal publication atomically persists the complete authenticated solver-coupled semantic point closure; ordinary aliases remain exact and only redundant rectangle aliases receive bounded numerical canonicalization, so incidental roundoff cannot create competing writes while material/signed-zero conflicts still reject. Point-seed precedence is typed overlay draft > legacy generated override > managed source seed; Reset restores the coupled bundle/lower tier. No preference chooses the unique producer and producer selection keeps consumers attached; unique consumer selection detaches only it, truthfully permits Segment identity replacement, rebinds surviving code/GUI dependents and remains repeat-draggable/Undoable. Ordinary GUI points stay delegated. Rectangle coupling is atomic. Selection/deletion resolves through an exact session + accepted alias + semantic-owner token, not a hashed `code.*` alias. Managed deletion rewrites the exact source/code-owned closure; generated-child deletion is reversible suppression. Dirty/failed/stale/GUI-owned cases reject without mutation. A retained structural/native failure keeps its deterministic owner-pruned attempted overlay above the exact accepted overlay/canvas; reload and Undo preserve both. Persisted optional envelopes report `geosolve-sketch-code-session-v2` and `geosolve-code-workbench-v2`, while plain M83 workspace-v8 is unchanged. | pending |
| M84-U15 | Open **PC water manifold · fully constrained dogfood**. Inspect the plate, reservoir clearance, three restrained routes, three enclosing O-ring groove loops and eight screw holes. In Code, inspect the three `waterChannel` invocations and custom patch; add and then remove/reorder one keyed route corner and Undo/Redo. Inspect diagnostics and attempt ordinary selection/edits without changing the one absolute anchor. | The fitted sketch reads as a plausible acrylic distribution plate rather than a synthetic corpus. All screw circles visibly remain 5 mm; grooves surround their corresponding channels; only one FixedPoint supplies absolute placement and all remaining geometry is relationally constrained. The patch stays read-only, `p.each` adapts Fillet cardinality to current keyed corners, unaffected keyed identities survive, accepted geometry stays finite/Current and diagnostics report zero numerical/equality/bidirectional DOF. | pending |
| M84-U16 | With the manifold open, click **Export PNG** and inspect the downloaded file; repeat from an ordinary non-code sketch after changing selection/tool state. | Each download is a readable 2000 × 1400 `geosolve-sketch.png` with the accepted scene, dark standalone styling, datums/construction/annotations and no hit targets, provisional controls or error overlay. Export leaves project, accepted geometry, selection, Undo/Redo, repro and persistence state unchanged and remains available without code-project authority. | pending |

Any JavaScript runtime solve, browser `eval`, raw code-facing ID, cross-project retarget, ordinal
identity churn, silent cascade, duplicate history, blank accepted scene or pointer-frame expansion
withdraws the candidate and opens an owning-layer regression.

## Final disposition

- Supervising-user UAT: refreshed M84-U1 through M84-U16 pending; every previous candidate,
  including direct-authoring `41e65a4`, remains withdrawn historical evidence.
- M84 GitHub Pages publication: prohibited before explicit approval.
- Tailscale state: rollback F010 snapshot `/tmp/geosolve-m84-f010-uat.7R5eXQoz` remains served at
  `http://100.94.63.83:8080/` by `geosolve-m84-uat.service`, PID `650971`. Withdrawn F009 snapshot
  `/tmp/geosolve-m84-f009-uat.q8cKIN3v` is preserved as historical defect evidence; PID `3965271`
  is retired.
