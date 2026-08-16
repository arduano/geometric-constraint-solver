<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 focused UAT — production-quality annotations

Status: prepared, not executed. The exact clean-qualified immutable candidate below is live for
review. Mechanical evidence disposes no UAT item. GitHub Pages remains on accepted M75 until
explicit M76 approval.

Candidate product source: `37eade50b566f62905a395655bc80c17d9b6bef4`

Candidate product tree: `d6ad2f453d672accbcc3848a1a16d2039b3511d1`

Current endpoint: `http://100.94.63.83:8080/`

Current server PID: `1077092` (retained command-runner session `11404`)

Server log: `/tmp/geosolve-m76-uat.puiPgO.server.log`

Immutable snapshot: `/tmp/geosolve-m76-uat.puiPgO` (directory `0555`, files `0444`)

Ordered-manifest aggregate:
`fb18b7c2387b9cea4bb681cac124f6ef9e63180ff071a734e80d27ac8cd83bdf`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 19,296 | `ab28a050a47a4b64fd20f6b821658246444528eee9b0c4499627af381427b72f` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-bcd530e5985506f3.js` | 33,221 | `d8974bcac131556374933e638799af9ee854a913e819e5ce492c9c0707547e0a` |
| `geosolve-demo-web-bcd530e5985506f3_bg.wasm` | 6,273,503 | `daf31f0a65459d3a6158feaa95d4c91c844417755a5071a9b8195c3ef9e02809` |
| `index.html` | 28,226 | `426d1c517dceebfdd80d48e5efa8a5931010bfd66b41264d655626d84c5d15f1` |
| `styles-c6136ab1b6e8294a.css` | 32,376 | `f1d2d6327d3e2af504d4e3d3a9e371b25040b114e23cf138d2e415a5835b539a` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 after 503
seconds at the source above. Editor passes 353/353, demo-web 122/122, M76 native/WASM parity 4/4
each, M75 native/WASM 11/11 each, M74 native/WASM 5/5 each, and the reviewed golden remains
unchanged at 270/270. The 149.39-second sparse crossover, Rustdoc, licensing/package-content,
benchmarks and Trunk 0.21.14 assembly also pass. The seven files above are the gate output frozen
without rebuilding.

Proxy/cache-bypassed identity requests for `/` and every file return HTTP 200 with exact media
types, lengths and bytes, no redirects or content encoding; `/` equals `index.html`, and the
fetched aggregate matches. Evidence is retained at `/tmp/geosolve-m76-http-verify.noWQO3`. Old
M75 server PID `37152` was retired before this candidate was served.

Run at `1440x900` and approximately `1024x720`, at coarse and fine zoom.

## U1 — dimension readability

Open the dimension sampler and inspect point distance, affine line/polyline-span length, radius,
diameter, angle, supporting offset and exact translated offset. Values must be compact, unambiguous and
visually attached to truthful geometry. Reference values must remain distinguishable without
colour, while tooltip/inspector/accessibility text stays descriptive.

## U2 — constraint readability and density

Open the constraint sampler. Hover/select operands and annotations, then enable Display “show all”.
Check all twenty symbol families, paired marks, local rotation, leaders and the fixed right-angle
square. Dense scenes should remain deterministic and legible without obvious collisions.

## U3 — move, cancel and reset

Move examples from every dimension family and several compact glyphs. Confirm the 3 px threshold,
that line/leader clicks select without unexpectedly moving, and that Escape, tool/camera change and
capture loss restore the original position. Test selected reset and reset-all.

## U4 — persistence and editing neutrality

Reload after moving annotations, then Delete/Undo/Redo nearby sketch content. Surviving placement
should persist and sketch history should contain no annotation-only step. Load a new sample and
confirm old offsets do not transfer. A deliberately incompatible cache must still restore the valid
sketch with deterministic automatic layout.

## Acceptance record

- U1: Pending.
- U2: Pending.
- U3: Pending.
- U4: Pending.
- Final supervising approval: Pending.
- GitHub Pages publication: Deferred until approval.
