<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 focused UAT — production-quality annotations

Status: **accepted for scoped closure (2026-08-17); separate post-refinement hands-on UAT was
explicitly waived**. The caller reviewed the initial candidate, reported that it looked good,
requested two final feature refinements and authorized closure after they were implemented. U1-U4
are accepted under that authority; no individual step-by-step replay was logged or is claimed.
GitHub Pages publication and exact public-byte verification remain pending.

Accepted clean-qualified source: `9b4e7f72dcacefdf4d7847a22eb675c711068d26`

Accepted clean-qualified tree: `e0591664fbeb2e353bc880dd826dc39ac1caeec9`

M76 feature-refinement commit: `a9fd6f6a71edf5be9d9fb5856074d291192a898d`

M76 feature-refinement tree: `2627e1d0ffdc500166bbcee50626fc9d65e05b67`

Current endpoint: `http://100.94.63.83:8080/`

Current server PID: `1455071` (retained command-runner session `70653`)

Immutable snapshot: `/tmp/geosolve-m76-uat.ctgYzp` (directory `0555`, files `0444`)

Ordered-manifest aggregate:
`337b0e6a2ce2b6a9aed979d0a4849e2d0887c092df66efa345d4917929d01dd4`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 19,296 | `ab28a050a47a4b64fd20f6b821658246444528eee9b0c4499627af381427b72f` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-55053c1ba5c6df34.js` | 33,221 | `d8974bcac131556374933e638799af9ee854a913e819e5ce492c9c0707547e0a` |
| `geosolve-demo-web-55053c1ba5c6df34_bg.wasm` | 6,273,395 | `ceed21fb5467d43e0ca603521c4c54602458c85f0736f6c129ae63303c01b53b` |
| `index.html` | 28,226 | `92a5e926448e82d05e3d84f1c0044513c6b70125d798374ec03fc205149ae1a5` |
| `styles-c2e1aed7dc61439c.css` | 31,750 | `69e4241bdcafc260ec6248ecc0a94f0cdb6420155419dd103a30d49ee1d467ee` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 after 569
seconds at the accepted source above. Editor passes 353/353, demo-web 122/122, M76 native/WASM
parity 5/5 each, M75 native/WASM 11/11 each, M74 native/WASM 5/5 each, and the reviewed golden
remains unchanged at 270/270. The 127.63-second sparse crossover, formatting, warnings-denied
Clippy and Rustdoc, locked workspace tests, benchmarks, licensing/package-content checks and Trunk
0.21.14 assembly also pass. The seven files above are the gate output frozen without rebuilding.

Proxy/cache-bypassed identity requests for `/` and every file return HTTP 200 with exact media
types, lengths and bytes, no redirects or content encoding; `/` equals `index.html`, and the
fetched aggregate matches. Evidence is retained at `/tmp/geosolve-m76-http-verify.CqEufj`. The
superseded server PID `1077092` was retired before this candidate was served.

## Refinements included after initial review

- Shared-endpoint acute/right line-angle annotations place arc, label, arrowheads and hit geometry
  in the wedge formed by the actual finite rays; the vertically opposite wedge misses. Obtuse
  finite joins retain the accepted acute supporting-line presentation. This pre-existing behavior
  was improved as an M76 feature, not recorded as an `M76-Fxxx` defect.
- Origin remains a protected/selectable/inference-capable headless datum and a Reference tree and
  inspector target, but its redundant canvas ring, cross, text and focus target are absent. The
  permanent X/Y axis intersection communicates zero.

The initial reviewed nomination at source `37eade50b566f62905a395655bc80c17d9b6bef4`, tree
`d6ad2f453d672accbcc3848a1a16d2039b3511d1`, snapshot
`/tmp/geosolve-m76-uat.puiPgO`, aggregate
`fb18b7c2387b9cea4bb681cac124f6ef9e63180ff071a734e80d27ac8cd83bdf`, is superseded evidence
only. It is not the accepted final candidate.

## Prepared review scope

The following sections preserve the intended review scope. They are accepted under the caller's
explicit scoped closure, not presented as separately observed post-refinement transcripts.

### U1 — dimension readability

At `1440x900` and approximately `1024x720`, and at coarse and fine zoom, inspect point distance,
affine line/polyline-span length, radius, diameter, angle, supporting offset and exact translated
offset. Values should be compact, unambiguous and visually attached to truthful geometry.
For a shared-endpoint acute/right line angle, the annotation should occupy the finite-ray interior
wedge and the vertically opposite arc position should not select it.
Reference values should remain distinguishable without colour while tooltip, inspector and
accessibility text stay descriptive.

### U2 — constraint readability and density

Inspect the constraint sampler, hover/select operands and annotations, then enable Display “show
all”. Check all twenty symbol families, paired marks, local rotation, leaders and the fixed
right-angle square. Dense scenes should remain deterministic and legible without obvious
collisions.

### U3 — move, cancel and reset

Move examples from every dimension family and several compact glyphs. Check the 3 px threshold,
that line/leader clicks select without unexpectedly moving, and that Escape, tool/camera change
and capture loss restore the original position. Exercise selected reset and reset-all.

### U4 — persistence and editing neutrality

Reload after moving annotations, then Delete/Undo/Redo nearby sketch content. Surviving placement
should persist and sketch history should contain no annotation-only step. A new sample should not
inherit old offsets. An incompatible cache should still restore valid sketch data with
deterministic automatic layout.

## Acceptance record

- U1: Accepted under the caller's scoped closure; not individually replayed/logged after the final
  refinements.
- U2: Accepted under the caller's scoped closure; not individually replayed/logged after the final
  refinements.
- U3: Accepted under the caller's scoped closure; not individually replayed/logged after the final
  refinements.
- U4: Accepted under the caller's scoped closure; not individually replayed/logged after the final
  refinements.
- Final supervising approval: Received 2026-08-17, including explicit waiver of separate
  post-refinement UAT.
- GitHub Pages publication: Pending as the remaining standard closeout step.

GitHub Pages remains on accepted M75 until this approved M76 candidate is published and exact
public bytes are verified. No later milestone is active.
