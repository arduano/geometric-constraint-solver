<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 focused UAT — Retained authoring semantic consolidation

Status: **the clean, byte-verified M73-F004 replacement candidate is current UAT authority; focused
human UAT and explicit approval remain pending, so M73 is still open**. Direct Rust/WASM tests
remain authoritative for semantic dispatch, candidate identity, accepted state and mutation-free
rejection; human review now uses only the candidate below.

Candidate source: `4c93ac5dd102fd52c78665a75997bcaf3d1d6f99`

Candidate tree: `fe9897153baa974b3c5c06e7a3bf5eee76e920f2`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server PID: `3870531`

Immutable snapshot: `/tmp/geosolve-m73-uat.JKAWtJ` (directory `0555`, files `0444`)

Ordered-manifest aggregate:
`3153f3b7b75e55ecc27c8798f4f26c6368c5b1e8db8422ee44c8840612d7ba8e`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 15,490 | `c3ef0cedd4de5968e36d2917daaf463c450fbe2266a06bc45b0cfae2dc20b935` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-92f14bb278c26c6b.js` | 33,093 | `5647aeac2f7852f1bf4015722528386b67c7c31208f9f5ce2cccbbb7171f2988` |
| `geosolve-demo-web-92f14bb278c26c6b_bg.wasm` | 6,021,403 | `bc1a23dd0f7917152c69a1f94e9858ceaf0d912a955db4bd68d77bca5a268342` |
| `index.html` | 26,345 | `a2cf744c5daea9cea42c5dbd7dd58c6a27d9e508841f54e5589a4256ef7b3f40` |
| `styles-437727272832bc26.css` | 27,010 | `9e4b1c6985f119cff35366119fbeef8abb2096b386a8db78a4cd730915316344` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` passes completely on the
candidate source: editor 325/325 plus every integration, public M73 3/3, unchanged golden 234/234,
native/WASM parity, workspace formatting/Clippy/tests/Rustdoc, benchmark compilation, M14/M32
performance, licensing/packaging, the 256-moving-body sparse crossover in 135.18 seconds and Trunk
0.21.14 all pass.

The distribution was copied from that gate without rebuilding. PID `3870531` has exact argv:

```text
python3 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m73-uat.JKAWtJ
```

Its executable is
`/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14`. Proxy/cache-bypassed,
identity-encoded requests for all seven files and `/` return HTTP 200 with expected media types and
match the frozen bytes; `/` equals `index.html`, and the fetched aggregate matches.

Historical F001-F003 source `efde645345577f44e0d6b691f7ca27eb587c4b53` and snapshot
`/tmp/geosolve-m73-uat.5EhWNL` remain preserved, but PID `3403533` has exited and those bytes are not
current UAT authority.

## Completed mechanical prerequisite — M73-F004 span-axis precedence

An eligible live world Horizontal span whose inference policy both adjusts coordinates and
persists the constraint must suppress durable same-axis `HorizontalPoints` and
`HorizontalPointToMidpoint` tracking before guide publication, candidate accounting, latch
acquisition or cross-axis pairing. Live world Vertical must symmetrically suppress durable
`VerticalPoints` and `VerticalPointToMidpoint`. Generic tracking-only cues remain visible and awake
without contributing a competing relation. Orthogonal durable point/native-midpoint plus
world-axis bundles remain available; remembered Parallel/Perpendicular/Collinear behavior and
solver redundancy rejection remain unchanged.

This narrowly supersedes M71-F004's same-axis-alternative rule only for eligible live world-axis
span constraints. Initial implementation `4fb9a7dd67ea86cd268028b5fa5c7842c56f2a88`, hardening
follow-up `0153fc0` and final durable-tracker boundary follow-up `89e409a` produce final focused
source `89e409a6ebe12c640ae2f313f95de67430dfa8d0`. It passes public regression
`m73_f004_span_axis_precedence` 3/3, including finite accepted geometry/residual and exact retained
history. The inference-owner early-suppression, budget/latch, cross-axis, orthogonal-bundle,
generic-tracking and remembered-direction controls; complete editor suite at 325 unit tests plus
every integration; M71 F003/F004/F005 and transition parity; warnings-denied Clippy; and unchanged
234/234 golden survey/check/clean gate all pass. The complete clean replacement qualification and
byte-verified publication recorded above also pass; only this focused human UAT remains pending.

Regression-hardening follow-up `f41e398d00b7a7ca1e12a12a285408a0b7bd3566` puts the complete
point/native-midpoint by Horizontal/Vertical matrix in the focused `same_axis_span` run and checks
the exact published guide set and empty durable tracker latch. The public midpoint case proves the
midpoint wake before testing suppression. Those focused commands pass 5/5 and 3/3 respectively.

## M73-U1 — Line and polyline stage continuity

1. Draw an ordinary Line and a multi-segment Polyline with point reuse, H/V inference and one
   remembered-reference inference.
2. Confirm each staged preview, retained relation and final segment refers to the point/span the
   cursor indicated.
3. Undo/Redo the completed Line and Polyline, then cancel a partial Polyline, reactivate the tool
   and redraw it.

Pass when stage ownership, segment numbering, references and history feel unchanged from M71/M72.

## M73-U2 — Contextual relation authoring

Exercise representative point, point/curve, line/line, center-bearing and curve/curve selections,
including Horizontal/Vertical, Coincident, Point on curve, Parallel/Perpendicular, Equal,
Concentric, Collinear, Tangent and Continuity. Confirm compatible selections apply once, invalid or
incomplete selections show their normal typed warning, and Undo/Redo preserves ordinary selection
and accepted-scene behavior.

Pass when the contextual tool surface retains its existing availability, operand order, branch
choices and error presentation. The retired direct Rust compatibility API has no browser control
and should create no visible omission.

## M73-U3 — Compound inference provenance and recovery

In the retained drafting-relations playground, check one line and one polyline endpoint using:

- a point-axis plus perpendicular span-direction bundle;
- Horizontal alignment to one stored point plus Vertical alignment to another;
- an ambiguous alternative followed by a deliberate candidate choice or cursor retreat.

Confirm the preview guides, snapped point and retained relations describe the same choice. Cancel,
Undo and retry; no stale guide or relation may survive.

Pass when compound candidates remain predictable and every rejection/recovery leaves the last
accepted scene intact.

## M73-U4 — Live world-axis precedence

On the replacement candidate, exercise both Line and Polyline endpoints:

1. Wake a stored point on the same Horizontal axis as a live Horizontal span, then repeat with a
   native midpoint. Confirm only the live Horizontal candidate/constraint-backed guide survives;
   no Horizontal point or point-to-midpoint guide remains.
2. Repeat symmetrically for live Vertical with a stored point and native midpoint.
3. Wake a point and midpoint on the orthogonal axis and confirm each still composes with the live
   world-axis span into the expected two-guide, two-relation bundle.
4. Check remembered Parallel, Perpendicular and Collinear alternatives on Cartesian supports, then
   attempt an actually redundant retained relation and confirm ordinary solver rejection remains.
5. Where a generic tracking-only cue is presented, confirm it remains visible but creates no
   competing retained relation.
6. Commit, Undo/Redo, cancel and retry representative cases; no suppressed durable guide, stale
   relation or extra history step may survive.

Pass when live world H/V direction intent clearly owns its same-axis coordinate while orthogonal
bundles, remembered-direction behavior and retained solver authority remain unchanged.

## Approval record

- M73-U1: pending human review on the current candidate above.
- M73-U2: pending human review on the current candidate above.
- M73-U3: pending human review on the current candidate above.
- M73-U4: mechanical prerequisite passed; current-candidate human review pending.
- Final M73 approval: pending.
