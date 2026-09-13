<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 focused UAT — Optional code/GUI sketch authoring

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

Status: **complete and closed 2026-08-27; exact clean-qualified immutable M84-F012 is accepted,
exact-verified on GitHub Pages and preserved as frozen UAT evidence**. Exact F011
source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, and snapshot
`geosolve-m84-f011-uat.ps736NLh` are historical rollback evidence. Exact F012 source
`84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, and snapshot
`geosolve-m84-f012-uat.nMOymIIM` are current mechanical nomination authority; no UAT acceptance
is claimed by that automation alone. The maintainer's later milestone-level decision accepts
U1-U16 without claiming a separately logged row-by-row hands-on replay. F010 source `cf463838`,
tree `992e587`, and snapshot `geosolve-m84-f010-uat.7R5eXQoz` are historical rollback
evidence. F009 source `c74651c`, tree
`a904584`, and snapshot `geosolve-m84-f009-uat.q8cKIN3v` are withdrawn historical defect
evidence. F007 source `cc2f05e`, the direct-authoring `41e65a4` snapshot, combined F005/F006 source
`ff2e142` and the initial/F003/F004 snapshots are likewise historical. M84 is accepted, publicly
verified and closed; Pages is final M84 public-byte authority.

## Withdrawn candidate evidence

Qualified product source: `79078eca44a5af4de5cccd92bf6fee570c473624`; tree:
`05aefb0cbd3972d423f1713df1e58628b24ec216`. The clean Nix release gate passed on 2026-08-25;
its 6,107-line, 414,758-byte log `geosolve-m84-release-gate.GOEuXP.log` has SHA-256
`0f50e6bcdf019c71d70497acc301dcdfd194db1142b248bcd469d0f3ed9efda0`. This includes
workspace Clippy/tests/Rustdoc, clean 271-row golden, actual WASM, both TypeScript suites, package
closure, benchmarks/performance, licensing and Trunk 0.21.14. M84-F001 and M84-F002 are closed by
owning-layer regressions.

The gate output was frozen without rebuilding at `geosolve-m84-uat.aHw5ePSW`, with directory
mode `0555`, seven regular non-symlink files at `0444`, freeze evidence at
`geosolve-m84-freeze-evidence.7loLImo2`, and ordered-manifest aggregate
`99beaf51ebb314aa26689427f970a75a516891efd20f68587c2a33c1b3a64f34`:

The archived manifest records the individual asset checksums.

Local and preview eight-path ledgers both have SHA-256
`dd8e6c1350f56cb6e7a483892a188187edc68ddcb63ee8ba9335401432ba8895`: every request returns
HTTP 200 with zero redirects, exact MIME/length/hash, no `Location`/`Content-Encoding`, and `/`
equals `index.html`. Focused Playwright passes 4/4 locally and 4/4 on preview, including its
bounded-surface check at both required sizes; log hashes are
`54858c5a1f75cc2e286d07360ed8342c7f8a09290a8f3e7ee1c4d295afe91d9e` and
`37f289b62adc02362e8c34a1ea23377c2a3fc5446d43ac262a5b075c1652f4c0`.

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
Its 6,192-line, 414,397-byte log `geosolve-m84-f003b-release-gate.log` has SHA-256
`eb05d3c1e460f5cb7be410dc44d0af0c4b4eaf4fd775423b676c77a84a433f90`. The unchanged 271-row
golden remains SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`;
the separate M84 ledger remains
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The gate output was frozen without rebuilding at `geosolve-m84-f003-uat.mO67NI`. Source,
copied and frozen manifests are identical. The directory is `0555`; exactly seven regular non-
symlink files are `0444`. Complete evidence is retained at
`geosolve-m84-f003-freeze-evidence.Ue4SCM`, and the ordered-manifest aggregate is
`38d356e9f727a4b690c1166dee3a36b1e0a5e59a2ad8bad1ca7243d889c6a617`:

The archived manifest records the individual asset checksums.

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
412,411-byte log `geosolve-m84-f004b-release-gate.log` with SHA-256
`f4c005912392c11cab6600706876c37dab855f0a59c0b7c06565c242b014d6fd`. It includes workspace
tests/Clippy/Rustdoc, unchanged golden, actual WASM, TypeScript, package closure, benchmarks,
licensing and Trunk. The 271-row golden remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate M84 ledger
remains `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact no-rebuild seven-file output is frozen at
`geosolve-m84-f004-hv-uat.FF5RFBZe`, with directory mode `0555`, regular files at `0444`,
complete evidence at `geosolve-m84-f004-hv-freeze-evidence.FiCfIvir`, and ordered-manifest
aggregate `f34c46ee5876c4bdb458863cc90c6c6b25281cc8e44f89c8d00eba0f16ca5bbc`. Temporary and retained
eight-path HTTP ledgers are byte-identical at SHA-256
`54efcd30699a8632b868d753af88b1f17433c284201688834f7a0f35e2598153`: every path returns HTTP
200 with zero redirects, exact MIME/length/body, no `Location` or `Content-Encoding`, and `/`
equals `index.html`. Baseline 4/4, F003 1/1 and F004 2/2 browser suites pass on both endpoints.

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
  tee geosolve-m84-authored-release-gate.log
```

from 14:55:50.676940562 through 15:12:22.475576111 AEST on 2026-08-26, exit 0 in
991.798635549 seconds. Its final Trunk `INFO ✅ success` marker was checked independently of the
tee pipeline. The 6,146-line, 415,754-byte log
`geosolve-m84-authored-release-gate.log` has SHA-256
`34bf408f6a565dec5705745f916eb628397a549b4a7002d865269e8d167e179f`. It includes formatting and
diff hygiene, warnings-denied Clippy/Rustdoc, locked workspace tests/doctests, unchanged 271-row
golden, separate four-demo M84 ledger, actual WASM, both TypeScript packages, package closure,
performance/benchmarks, licensing and Trunk 0.21.14. The worktree was clean before and after the
gate. The golden remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the M84 ledger remains
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact gate output was copied without rebuilding to
`geosolve-m84-authored-uat.ZYQQyBQQ`. Source, copied and frozen manifests match; the directory
is `0555` and exactly seven regular non-symlink files are `0444`. Complete evidence is retained at
`geosolve-m84-authored-freeze-evidence.ngNf7jxZ`; its ordered-manifest aggregate is
`6f82bb261057916f737110cb6533da128d1afde72d1b2f5584f937acdd1a54b1`:

The archived manifest records the individual asset checksums.

Temporary `:18086` and retained `:8080` eight-path HTTP ledgers are byte-identical at SHA-256
`1450e4c6d8585ba17dee56feafaf96869c45f764f3400280a0dc37581f9b4eee`. `/` plus all seven files
return HTTP 200, zero redirects, exact MIME/length/hash, no `Location` or `Content-Encoding`, and
`/` equals `index.html`. Against those same frozen bytes, direct-authored lifecycle passes 3/3,
baseline passes 4/4, F003 passes 1/1 and F004 passes 2/2 on both endpoints. The direct suite covers
both `1440x900` and `1024x720`, zero landing overflow, Start/edit/Apply, retained-invalid canvas,
Undo, reload/repro and sample-card routing. Automated browser checks are mechanical nomination
evidence, not maintainer UAT.

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
`geosolve-m84-f007-release-gate.fn6ZHE.log` has SHA-256
`40c73a8856df0905e85e2b877a82db0e0e81da583764ccc18d729e268f01763b` and ends with the Trunk
0.21.14 success marker. It includes formatting/diff hygiene, warnings-denied workspace
Clippy/Rustdoc, locked tests/doctests, unchanged 271-row golden, actual WASM, both TypeScript
packages, package closure, benchmarks/performance, licensing and release Trunk. The unchanged
golden and four-demo ledger hashes are
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact gate output was frozen without rebuilding at
`geosolve-m84-f007-uat.KgW8fpLf`. Source, copied and frozen manifests match; the directory is
`0555` and exactly seven regular non-symlink files are `0444`. Ordered-manifest aggregate is
`8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`; complete evidence is at
`geosolve-m84-f007-freeze-evidence.rP5rQcTG`:

The archived manifest records the individual asset checksums.

Temporary `:18087` and retained `:8080` eight-path ledgers are byte-identical at SHA-256
`efa609c6bac127753336c3634730b81bed04699a25c6394ab039c7f06b0b2b64`: every path returns HTTP
200, zero redirects, exact MIME/length/hash, no `Location`/`Content-Encoding`, and `/` equals
`index.html`. Baseline 4/4, direct-authored 3/3, F003 1/1, F004 2/2 and F005-F007 4/4 browser
suites pass on both endpoints. Browser coverage directly exercises both desktop sizes,
no-selection producer drag, selected producer, selected-consumer detachment, repeated drag,
Undo/Redo/reload/Reset, managed deletion and generated-child suppression. Generic/foreign/stale
terminal and bit-conflict adversarial internals remain Rust/WASM-owned because no public browser
gesture exists for them.

## Withdrawn F009 replacement nomination

Exact product source `c74651cc82506e31926042df65a1eeec08a6af9d`, tree
`a904584410ca9a8cd3112d17ad70c0e84c29e8d9`, passes the complete clean gate. The 6,218-line,
421,590-byte log has SHA-256
`c9b743c8f95d6df7706b04e2d820ac67426f1b11ec447d2bffdc08cd1fe0f6f1`. The unchanged 271-row
golden and eight-demo ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

The exact no-rebuild seven-file output is frozen at
`geosolve-m84-f009-uat.q8cKIN3v`, directory/files `0555`/`0444`, ordered-manifest aggregate
`23f2f839f2a3be6b722ae26cb548f0a19ce2f3d6afac90d5f913938a042d1c1f`. Complete evidence is at
`geosolve-m84-f009-freeze-evidence.3FoVTQ6m`. Temporary `:18089` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`add827e88d17735cfb6cb0bbecec885f5680db0bd11b67bb591673d566b90676`; baseline 4/4,
direct-authored 3/3, F003 1/1, F004 2/2 and F005-F007 4/4 browser suites pass on both endpoints.
Every sample card opens fitted finite geometry at `1440x900` and `1024x720`.

## Historical F010 replacement nomination

Exact product source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passed the clean release gate from
15:58:23.055857854 through 16:17:18.303733569 AEST on 2026-08-27, exit 0. Its 6,209-line,
420,425-byte log `geosolve-m84-f010-gate.9NvAi3z5/release-gate.log` has SHA-256
`bf57345266005a85b6da20f1105c3cf126d2492ba91e413ef0f07c5d38d3b28a` and ends with final Trunk
success. The unchanged 271-row golden and eight-demo M84 ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

The exact seven-file gate output was frozen without rebuilding at
`geosolve-m84-f010-uat.7R5eXQoz`, directory/files `0555`/`0444`, ordered-manifest aggregate
`ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`. Complete evidence is at
`geosolve-m84-f010-freeze-evidence.sXWXNG0Z`. Temporary `:18091` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`57f2f4c2b47a11db8fc76a7f6a2e3d30555cb36e96b191081454a4c47fb85cbe`; all eight paths have exact
bytes and MIME. Focused Compass 1/1 and the carried 14/14 browser matrix pass on both endpoints.
The Compass scenario performs six center drags, proves exact release and
+50/+250/+500/+1000 ms positions, keeps all four spokes attached, requires finite accepted
authority and verifies exact reload. Retained focused spec/config SHA-256 values are
`4b97f570d5122a353b4ee104b26ea46427ca1c3b79aa5a8d35fee7875302dab0` and
`c0900c1132352ed9471321a2cf5727baf2c004d8eebf1a1bcd8a43146289df9c`.

Exact F011 source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, passes the clean gate from 18:38:08.068586771 through
18:55:55.205076185 AEST, exit 0. Its 6,251-line, 422,664-byte log has SHA-256
`ceea545929196981e2790a822b384596642f63a6ef93ecea98f76799cdd7e353`; golden/nine-demo-ledger
hashes are `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`. The exact no-rebuild
snapshot `geosolve-m84-f011-uat.ps736NLh`, modes `0555`/`0444`, aggregate
`056193f4af17437da5430dc86059ad4c4b73ec62e959a461935ca29153b10fc2`, has complete evidence at
`geosolve-m84-f011-freeze-evidence.4deymgss`. Temporary `:18093` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`9339301ea57feb293a27256795344f88805046426e3659bae0f750b67b251b94`.

Exact F012 source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, passes the clean gate from 20:36:27.840305756 through
21:01:30.826307821 AEST, exit 0. Its 6,274-line, 424,393-byte log has SHA-256
`04e35c73fe92ca3e089b87bd13b5221c60835b72c9eeba5ed38916b51150004a`; golden/nine-demo-ledger
hashes are `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`. The exact no-rebuild
snapshot `geosolve-m84-f012-uat.nMOymIIM`, modes `0555`/`0444`, seven regular files and zero
symlinks, aggregate `166abc1298220090ba4c8b0a37a176fb4f945cceae68771efbd601acc1970169`, has complete evidence at
`geosolve-m84-f012-freeze-evidence.qua6ci1b`. Temporary and retained eight-path ledgers are
byte-identical at SHA-256 `66fcd4c852baab5290605066ec856239af7c4f033cef55a4dfd5fb86058645ba`.

| Release state | Status |
| --- | --- |
| M84-F007 clean qualification and historical freeze | superseded |
| Eight-demo/M84-F008/F009 clean replacement qualification and immutable freeze | withdrawn by F010 |
| M84-F010 proportional owner/WASM/mutable-browser qualification | complete |
| M84-F010 clean qualification and immutable replacement freeze | complete; withdrawn by F011 |
| M84-F011 manifold/PNG focused qualification | complete on mutable and both frozen endpoints; no UAT row accepted |
| M84-F011 clean qualification and immutable replacement freeze | complete; withdrawn by F012 and retained as historical rollback evidence |
| M84-F012 annotation paint/pick and WYSIWYG export amendment | clean-qualified, immutably frozen and accepted; automation alone accepted no UAT row |
| maintainer milestone-level acceptance of M84-U1 through M84-U16 | complete |
| GitHub Pages publication, service retirement and M84 closure | complete |

## Maintainer acceptance

On 2026-08-27 the maintainer approved M84 and asked for the milestone to be closed before
performance optimization begins. That milestone-level decision accepts M84-U1 through M84-U16
against the exact qualified F012 candidate. It does **not** claim a separate row-by-row hands-on
replay or invent observations that were not logged. The scorecard statuses therefore say
`accepted by milestone-level approval`, not `manually passed`.

## Final GitHub Pages publication

Approval descendant `e6e960d7ac297eb099ba80c19148393e46427606`, tree
`173c65d5c39c9cb371869fccf22d5b41ddec6ceb`, passes Pages run `33068058169`, build/deploy jobs
`98503000701`/`98504823968`, deployment `6121981526` and artifact `9644770095` (API size
4,890,131 bytes). Downloaded 15,032,320-byte `artifact.tar` has SHA-256
`da43ee8d85f81461d579cafa24b9e884451c97a58909955ae188ef9281f7412e`; it contains exactly seven
regular files, no symlinks, with ordered-manifest aggregate
`1f2228abcb163e09ff50db2f19d79b13d9c74e638731d6cd26d5ade324b27835`.

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
requirements remain part of U14. F011 adds distinct U15 manifold-dogfood and U16 PNG-export rows;
F012 extends U16 with the shared annotation paint/pick toggle and WYSIWYG cleanup without adding a
seventeenth row.
M84-F009 additionally requires exact renamed/nested and mapped `result_output` regressions plus the
all-eight automated output-kind audit: each declared reference kind and expanded target kind must
match the reviewed catalog, including Mounting Plate `plate.profile` as Profile rather than the
`ne` Point. It adds no hands-on row.

## Scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M84-U1 | Open **Rounded polyline · dynamic corners**. Inspect six keyed vertices, five spans and four visibly distinct radius-4 Fillets; insert `crest`, reorder/remove it and Undo/Redo. | Every Fillet is visibly clear of point markers. Counts become seven/six/five after insertion; unaffected members keep their visible identity; reorder is non-semantic; removal/tombstone and Undo are exact. | accepted by milestone-level approval |
| M84-U2 | Edit the adaptive Fillet radius through its lens, drag a permitted generated/free point, Reset to code, toggle open/closed and enter an impossible radius. | Lens edits the managed invocation; drag creates one explicit override; reset removes it; cardinality adapts; invalid geometry retains source/diagnostic above the prior accepted canvas. | accepted by milestone-level approval |
| M84-U3 | Open **Typed panel · keyed Fillets** and inspect rectangle outputs plus `fillets({ lowerLeft, upperRight })`; inspect `wire.vertices`/`wire.segments` in **Lantern garland**. Run the shipped type fixtures. | Corners/edges/profile, both Fillets and Polyline keyed Point/CurveSpan roots have meaningful typed fields; misspelling, raw IDs, cross-project refs and point/curve/corner mismatch fail compilation. | accepted by milestone-level approval |
| M84-U4 | Open **Braced frame · GUI → code → GUI**. Edit/drag the GUI rectangle and code-generated brace, then constrain or dimension `brace.diagonals.rising`. | Both authoring directions update one accepted scene and one managed source; ownership/selection is truthful; ordinary native constraints remain fast and editable. | accepted by milestone-level approval |
| M84-U5 | Mix code Apply, Inspector/canvas edits, organization changes, lens edits, overrides and Reset; walk Undo/Redo after each. | Every action creates exactly one coherent history entry; source, expansion, nested intent, accepted scene and overrides restore together with no mirrored editor history. | accepted by milestone-level approval |
| M84-U6 | Edit whitespace/comments/unowned formatting, then attempt unsupported syntax, stale-span Apply, duplicate keys and a dangling generated-output removal. | Preserved spans remain byte-identical; unsupported/stale/duplicate/dangling edits diagnose exactly and publish no unintended source, graph or scene change. | accepted by milestone-level approval |
| M84-U7 | Tour **Mounting plate**, **Lantern garland**, **Suspension bridge**, **Compass rose** and **Neon manifold**. Inspect Mounting Plate `plate.profile`, edit representative managed dimensions/placements and compare custom helpers before/after. | `plate.profile` is the Profile rather than a Point; rounded profile/holes, adaptive bulbs/Fillets, cables/stays, ring/markers and explicit Neon bends remain finite and understandable; `patches/*.patch.ts` stays byte-identical and artifact/ownership/lens state is truthful. | accepted by milestone-level approval |
| M84-U8 | Save/reload and Copy/New/Load repro for valid and retained-invalid code projects; remove/tamper an artifact and try a corrupt/oversized payload. | Complete offline authority/history returns; invalid intent keeps its accepted scene; missing/tampered/corrupt/oversized inputs reject before atomic replacement. | accepted by milestone-level approval |
| M84-U9 | Drag repeatedly in **Suspension bridge** while watching source/History, including rapid reversals and a rejected final pointer sample. | Preview remains at the M83 interaction quality; pointer frames do not parse/expand or churn panels; release commits the newest accepted preview once and exact Undo restores it. | accepted by milestone-level approval |
| M84-U10 | Start a plain M83 workspace/build without code support, then a code-enabled one. Inspect the browser file/artifact UI at both sizes. | Plain solver/editor behavior and persistence remain available without code-module linkage; code UI is polished and bounded, custom files are clearly read-only, and it does not pretend to be a general IDE. | accepted by milestone-level approval |
| M84-U11 | In an ordinary sketch, draw an aligned rectangle and a line between two rectangle corners; inspect Intent IR and Code, then promote the preview and edit the rectangle. | Intent IR is labelled as transport data; Code uses lexical `frame.corners.*` values with no serialized dependency DTO; promotion creates one real managed project and the dependent line follows later edits. | accepted by milestone-level approval |
| M84-U12 | In an ordinary sketch, draw a Horizontal line, continue it with a Vertical line and Fillet their corner. Inspect Code, promote and reload; edit radius once as a model-unit number and once with `mm(...)`. Then separately try a computed Fillet arc as a direct parent and add another unsupported declaration. | The `line2` declaration contains lexical `start: line.end`; the preview contains `$.constraint.horizontal` over `line.span`, `$.constraint.vertical` over `line2.span`, and `$.computed.filletSet` with those lexical parent spans, no raw ID/DTO and complete explicit branch/contact fields. Both existing axis constraints survive promotion. Line spans are native-branded; a computed host arc cannot be a parent. Positive finite model-unit and branded-mm radii work; forged/other-unit/nonpositive/nonfinite values reject. Promote/reload retains finite Current Fillet geometry with independently validated Hard residual `<= 1e-9`. Unsupported scenes keep Code visible with an escaped truthful diagnostic and Intent IR fallback, while Promote is absent. | accepted by milestone-level approval |
| M84-U13 | Click **New**, open **Code**, inspect the starter and nine example cards, then choose **Start from code**. Edit the starter rectangle, Apply, enter a collapsed invalid rectangle, Undo/Redo, reload/repro, return to New, and open every sample card. Check both desktop sizes. | The fresh surface has one direct starter action and nine genuine projects without overflow. Every card opens finite fitted visible geometry. Start creates **Untitled code sketch** with editable artifact-free `sketch.ts`, lexical `frame.corners.*` dependencies and no Promote/fabricated GUI history. Valid edits update finite accepted geometry; invalid edits retain the prior canvas and diagnostic; Undo/Redo/reload/repro preserve authored authority. | accepted by milestone-level approval |
| M84-U14 | In code projects, drag a literal point and each rectangle-corner role through at least two preview frames; also drag a Lantern vertex, Bridge tower peak, Compass center/spoke and Neon shared endpoint. On a shared rectangle point, repeat a multi-frame drag with no semantic selection and with its producer selected, then drag it with its referenced consumer selected. During one pending route try a generic save, foreign terminal and foreign/reentrant preparation; also verify no-motion release/cancel and stale terminal after Apply/Undo. Repeat the detached-consumer drag, Undo/Redo and Reset. Then select/delete a managed declaration with dependents and one generated child. Repeat deletion with a dirty source draft, retained code failure, a target retained across another revision and a GUI-owned selection. Finally remove a drafted owner in a source edit that also fails native publication, then reload and Undo. | Each accepted release records one bounded point-only semantic overlay/history entry and preserves a finite independently validated accepted scene. Lantern bulbs/Fillets, Bridge cables/stays, Compass ring/markers and Neon bends remain attached/current. The Compass center remains at the exact release through at least +500 ms and reload. Pointer-down authenticates exactly one semantic point lens and stores its exact `CodeSessionIdentity` plus pointer; only the dedicated authenticated terminal publisher may consume it. Generic save, foreign terminal and foreign/reentrant preparation reject while preserving the route. Non-pointer durable code actions invalidate it; no-motion release/cancel is history-neutral; stale terminal after Apply/Undo cannot revert newer accepted authority. Terminal publication atomically persists the complete authenticated solver-coupled semantic point closure; ordinary aliases remain exact and only redundant rectangle aliases receive bounded numerical canonicalization, so incidental roundoff cannot create competing writes while material/signed-zero conflicts still reject. Point-seed precedence is typed overlay draft > legacy generated override > managed source seed; Reset restores the coupled bundle/lower tier. No preference chooses the unique producer and producer selection keeps consumers attached; unique consumer selection detaches only it, truthfully permits Segment identity replacement, rebinds surviving code/GUI dependents and remains repeat-draggable/Undoable. Ordinary GUI points stay delegated. Rectangle coupling is atomic. Selection/deletion resolves through an exact session + accepted alias + semantic-owner token, not a hashed `code.*` alias. Managed deletion rewrites the exact source/code-owned closure; generated-child deletion is reversible suppression. Dirty/failed/stale/GUI-owned cases reject without mutation. A retained structural/native failure keeps its deterministic owner-pruned attempted overlay above the exact accepted overlay/canvas; reload and Undo preserve both. Persisted optional envelopes report `geosolve-sketch-code-session-v2` and `geosolve-code-workbench-v2`, while plain M83 workspace-v8 is unchanged. | accepted by milestone-level approval |
| M84-U15 | Open **PC water manifold · fully constrained dogfood**. Inspect the plate, reservoir clearance, three restrained routes, three enclosing O-ring groove loops and eight screw holes. In Code, inspect the three `waterChannel` invocations and custom patch; add and then remove/reorder one keyed route corner and Undo/Redo. Inspect diagnostics and attempt ordinary selection/edits without changing the one absolute anchor. | The fitted sketch reads as a plausible acrylic distribution plate rather than a synthetic corpus. All screw circles visibly remain 5 mm; grooves surround their corresponding channels; only one FixedPoint supplies absolute placement and all remaining geometry is relationally constrained. The patch stays read-only, `p.each` adapts Fillet cardinality to current keyed corners, unaffected keyed identities survive, accepted geometry stays finite/Current and diagnostics report zero numerical/equality/bidirectional DOF. | accepted by milestone-level approval |
| M84-U16 | In both the manifold code project and an ordinary flat sketch, confirm **Annotations** starts on. Overlap an annotation with geometry or the Origin, turn annotations off/on, exercise a Fillet radius handle, and export once in each annotation state. Also export while a draft/inference/provisional authoring cue is visible. | The toggle is session-local and controls constraint/dimension paint and picking together. Hidden selected/problem annotations have no invisible hover/click corridor and never steal pointer ownership from underlying geometry/datums; Fillet handles remain available. Existing annotation layout and selection do not reset, and toggling creates no history or document/code/Intent-IR/repro/persistence change. Each readable 2000 × 1400 `geosolve-sketch.png` is WYSIWYG for annotation visibility, retains accepted standalone paint, omits all hit/error/draft/inference/provisional paint, and leaves live project, layout, selection, accepted geometry, Undo/Redo and durable authority unchanged. | accepted by milestone-level approval |

Any JavaScript runtime solve, browser `eval`, raw code-facing ID, cross-project retarget, ordinal
identity churn, silent cascade, duplicate history, blank accepted scene or pointer-frame expansion
withdraws the candidate and opens an owning-layer regression.

## Final disposition
