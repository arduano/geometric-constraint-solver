<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M79 focused UAT — inference cycling and recovery

Status: **accepted 2026-08-18; final GitHub Pages publication pending**. The supervising caller
approved the clean-qualified frozen candidate and requested milestone closure without reporting a
new finding.

Use only the exact source, immutable snapshot and Tailscale endpoint recorded below. Findings must
name the candidate and receive an M79 finding ID before replacement work begins.

Product source: `6874aa1961798f4838fcda8b5fbedc4e39abfa7a`

Candidate tree: `f2b70c0b5a3bd8d759479c42bf742f7f288c821d`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server: `geosolve-m79-uat.service`, PID `40049`

Immutable snapshot: `/tmp/geosolve-m79-uat.I5TJTx` (directory `0555`, seven regular non-symlink
files `0444`)

Ordered-manifest aggregate:
`1da8503f4d9ab535bbe3b9ce2972e05d742b2928ad8c54b59596bbac240e9ebf`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 26,922 | `0106c01a41c0a227da03dd3f389a92070119ff81178cd9ff4f621e73198edd3a` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-6ab7a24d44cc82f6.js` | 33,333 | `075079f4bb9ae65c52e728c49fdca8df0dfa200fc3e7a8623ead8781cbe8d840` |
| `geosolve-demo-web-6ab7a24d44cc82f6_bg.wasm` | 6,554,076 | `7aa509532cd544a6cf6410652a9e7f3c8d9df5c36de036401b1a9c9641c1f3ec` |
| `index.html` | 29,143 | `10f178a075eb4e7a6ee8e3a28de0e2f20d2f961d0bce47c6ffdee5663b9446b6` |
| `styles-a83e80383c7972df.css` | 35,731 | `cc0f03992191c1952bc4242fc951eac0e4c1d3a6bce0965a2290f2892cbe6572` |

The exact clean release command
`env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from 17:43:28 through 17:54:19 AEST on 2026-08-18 and exited successfully without changing
candidate HEAD, tree or worktree. Its 253,043-byte, 3,307-line retained log is
`/tmp/geosolve-m79-clean-gate.HpmSbd.log`, SHA-256
`838fafa04c67a64f75a9c38d40ea4b3cbb5825dbe8ccb59e339bcee462db21b3`.

The gate-produced `dist` was copied without rebuilding, compared byte-for-byte and frozen above.
Freeze evidence is `/tmp/geosolve-m79-freeze-evidence.M5PXPY`. Proxy-disabled, cache-bypassed,
identity-encoded requests first passed on temporary port `18079`, after which the old M78 listener
was retired and this candidate started on `8080`. Final requests for `/` and all seven files return
HTTP 200 with zero redirects, no content encoding, exact media types/lengths and snapshot-identical
bodies; `/` equals `index.html`. Temporary evidence is
`/tmp/geosolve-m79-temp-verify.K5bILD`; final evidence is
`/tmp/geosolve-m79-final-verify.LExe3F`. Both `results.tsv` files have SHA-256
`be0531a9306c1b3582af87f23b8e2725e1d85ba0b96d4a145c8954c50c5791ab`.

## U1 — exact reported reproduction

1. Draw a Center Rectangle with its centre snapped to Origin and a corner away from both axes.
2. Activate Midpoint Line and choose Origin as its centre.
3. Hover the midpoint of the rectangle's right edge.
4. Press Tab through every displayed candidate at least twice, including wraparound.

Every press must immediately update the guide/preview, must never leave a stale/dead state and must
continue to work without pointer movement. The candidate list and order must stay stable for that
stationary sample.

Return to the default `Midpoint + Horizontal` choice and place the line. Placement must succeed as
one Undo/Redo step. The retained scene must show the associative endpoint Midpoint relation and no
duplicate auto Horizontal relation; the line remains horizontal as a consequence of the accepted
geometry.

## U2 — movement and candidate refresh

At the same target, select a non-default candidate with Tab, move clearly away until snapping
disappears, then return. Normal candidates must immediately return and Tab must start from the
fresh ranked cohort. Repeat with two close points or midpoints so two same-position semantic
alternatives can be cycled A → B → A without either disappearing.

## U3 — modifiers and lifecycle

After choosing a candidate with Tab, exercise each transition before returning to the target:

- hold/release Ctrl or Cmd to suppress/restore inference;
- hold/release Shift on a recipe that supports regularization;
- Backspace, first/second Escape and a geometry-tool switch;
- Undo and Redo;
- pan, zoom and fit/reset camera;
- browser blur/focus and pointer leave/re-entry; and
- open/close an authoring overlay or otherwise transfer canvas ownership.

No old choice may leak into the new context. A stale click must never place another candidate
silently, and the next ordinary hover must recover without refresh/reload.

## U4 — queued movement and click truth

Move quickly between two different snap targets and press Tab before the pointer appears settled.
The selected guide must belong to the latest visible coordinate, never the previous target. Click
the unchanged stationary target and confirm the displayed choice is consumed exactly once. Move or
change tools before another click and confirm that choice is no longer active.

## U5 — adjacent candidate families

Spot-check candidate cycling for persistent points, line/polyline midpoints, point-on-curve,
Origin/axes, semantic centres, horizontal/vertical, parallel/perpendicular/collinear and a
two-reference Cartesian intersection. All compatible ranked alternatives remain cycleable; tied
alternatives require an explicit choice rather than guessing. Resource-limited, suppressed or
genuinely stale states remain noncommittable.

## Acceptance record

On 2026-08-18 the supervising caller stated, “UAT approved, please close off.” That explicit
milestone-level decision accepts U1-U5 for exact frozen product source `6874aa1`, tree `f2b70c0`,
snapshot `/tmp/geosolve-m79-uat.I5TJTx` and aggregate
`1da8503f4d9ab535bbe3b9ce2972e05d742b2928ad8c54b59596bbac240e9ebf`. No M79 UAT finding was
reported. Final GitHub Pages publication and hosted-byte verification remain before closure.
