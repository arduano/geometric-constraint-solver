<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M89 UAT: executed, reversible managed sketches

Status: **Historical, superseded by M90's closed typed clean-break contract.** M89-F004/F005
implementation, provisional dirty-tree mechanical qualification and its immutable F005 nomination
completed, but the Compass Rose retest, targeted preflight and M89-U1 through M89-U8 were never run
and are not retrospectively passed or waived. Every row below remains **Not run**; this scorecard
does not constitute clean-source qualification or human acceptance.

Do **not** run this scorecard against `/tmp/geosolve-m89-uat.TIPyWl`. That snapshot's aggregate
`5b842c6541261a9386c6a060475350712326a467137f166b4c02d8782635ff6a` and release-WASM SHA-256
`04a8b6504db1e8ce68f9c8cec864d293d95e07394dff302bc877782b4ef4ce04` are withdrawn M89-F001
reproduction evidence; its old PID `825513` is retired while the snapshot remains preserved.

Do **not** use the withdrawn F001 replacement snapshot
`/tmp/geosolve-m89-f001-replacement-uat.IwvrBh8x`. Its nine regular files, two directories and zero
symlinks use `0444`/`0555`; ordered-manifest aggregate is
`7d40c31c0eba5aae0d1b6e7febf16a0b5fa44a541c82a9ddbbb0125cd8707424` and optimized release-WASM
SHA-256 is `a2b2fd4ec4852e3d16b10ab0fa41eb8e4d64b52db430447bdbf28bb8d82f3241`.
Freeze and HTTP/browser evidence is
`/tmp/geosolve-m89-f001-replacement-freeze-evidence.yhuZYse1`. M89-F002 withdraws those bytes from
continuing UAT because an ordinary Polyline expanded to transport-level source hundreds of lines
long. Former PID `1589113`, invocation `d05d85152a4547f7b32e316b6a3d4b9a`, has been replaced; the
historical snapshot remains preserved.

The pending scorecard would have applied only to the now-historical immutable F005 candidate at
`http://100.94.63.83:18089/`. Unit `geosolve-m89-react-uat-current.service`, PID `1007459`,
invocation `a3fea6dddb59438795e52c6a8fab136f`, started
`Wed 2026-09-02 20:21:15 AEST` with `WorkingDirectory=/tmp/geosolve-m89-f005-uat.hzNuDxF0` and
serves only that snapshot. Its external manifest is
`/tmp/geosolve-m89-f005-uat.hzNuDxF0.sha256`, aggregate
`fb488ad2bf29e8897cf9811c002b748693e5d211bae4bb54c83ed060db5db668`, and freeze/HTTP/browser
evidence is `/tmp/geosolve-m89-f005-freeze-evidence.VAoQDl8n`. The distribution contains nine
regular files, including three JavaScript, one CSS and one WASM, two directories and no
symlinks/other entries. Its `16,333,537`-byte
`assets/geosolve_demo_web_bg-52ybei8k.wasm` has SHA-256
`3e6f515ff1e5de0f668c13e86c02d280c0dc085bbd89b314bf9ece6c82aae575`.

Strict staging and live ten-route HTTP ledgers are byte-identical at SHA-256
`4de184eb1eb237f98997b70702567a2b110b40d96df5d0d9653f024ac5f23e8d`. Focused Fillet lifecycle
passes `1/1` and the normal frozen product passes `15/15` on both endpoints; the normal product
intentionally excludes the compiler-parity-only harness test. Staging PID `995137`, invocation
`efe4de2264774f68bf5852284fa69887`, is retired. The F003 snapshot and its former PID `3637680` /
invocation `73bdb82cf2a64436ab6f65fe76b08ccf` remain historical rollback evidence, not current authority.
The preliminary F003 snapshot `/tmp/geosolve-m89-f003-uat.W5GIfQi7`, F002 snapshots
`/tmp/geosolve-m89-f002-uat.bSM6Nn59` and `/tmp/geosolve-m89-f002-uat.Taocj5bI`, and every F001
snapshot remain preserved but withdrawn. Every human row remains **Not run**.

M89-F001 was reproduced exactly by opening Compass Rose, choosing Sketch → Polyline, clicking
three right-angle points and pressing Finish. The deterministic regression uses normalized canvas
positions `(0.35, 0.40)`, `(0.60, 0.40)`, `(0.60, 0.65)`. The withdrawn build left `sketch.ts`
unchanged because normalized expansion could not resolve `core.markers`. The repair publishes the
Compass patch's named `markers` and `ring` collections, preserves accepted authority on failure,
and atomically publishes the successful active-copy upgrade.

Historical F001 automated repair qualification passes the complete `geosolve-sketch-code` crate,
demo-web `357/357`, frontend `54/54`, warnings-denied Clippy, the locked WASM check, unchanged clean
golden qualification, optimized release-WASM Playwright `13/13` and the complete provisional
dirty-tree release gate. The exact no-build Compass test passed `1/1` against both temporary
`:18189` and the then-final `:18089` frozen bytes, with identical ten-route HTTP ledgers at SHA-256
`62ae4d760bb40fb9031ef53a8ec9ee6c8f239572242ed6ee730282e1a0f50e0b`. Human UAT should still
repeat the exact Compass Rose flow before the general rows: require managed-v2 `sketch.ts`, one
geometry plus two inferred constraint declarations under `Canvas additions`, source navigation for
all three, two added accepted curves, accepted revision `r2`, no remaining draft, one Undo route and
no console/network/runtime error. This human retest is **Not run**.

M89-F002 retains the same geometry/publication contract while replacing the verbose generic form.
Its captured one-axis and two-axis sources were respectively `4,238` bytes/`172` lines and `4,943`
bytes/`199` lines. On the historical F002 bytes, the exact Compass retest contains one readable
`$.geometry.polyline(...)` with deterministic `v0`, `v1`, `v2` keys,
`representation: "singleCurve"`, and direct inferred constraints such as
`curve: geometry1.segments.byKey.v0`. It must contain no `$.intent.recipe`; the complete source must
be at most `75` lines and `2,500` bytes. Source navigation, two added accepted spans, revision `r2`,
one Undo route and clean runtime remain mandatory. The isolated compact fixture is `24` lines/`722`
bytes. The frozen-browser run produced a normalized Compass source of `64` lines/`1,841` bytes
(`65`/`1,842` including CodeMirror's trailing blank), and the exact no-build regression passed
`1/1` first on frozen staging `:18189` and again on live `:18089`. The optimized release-WASM
harness passed `13/13`, the frozen normal product passed `12/12` without rebuilding, and both
strict ten-route HTTP ledgers have SHA-256
`2a9a939d073f63df095ebc01542eb95443f4e3b86d7df2980c0b586a571b2ba1`.

M89-F003 generalizes that compactness contract to every geometry tool. The expected source surface
contains exactly these three categories:

- Segment uses direct `$.geometry.line(...)`;
- exact native Polyline uses direct `$.geometry.polyline(...)`;
- Sketch Point, Midpoint Line, all four rectangle variants, all three circle variants, all three
  arc variants, both ellipse variants, both elliptical-arc variants, Quadratic and Cubic Bezier,
  Rational Quadratic Conic, Parabola, Hyperbola, Open-Control NURBS and Periodic-Control NURBS use
  one lossless `$.geometry.recipe(...)` declaration each.

For a representative Quadratic or Cubic Bezier and one high-cardinality NURBS, complete the canvas
gesture and inspect `sketch.ts`. Require one readable declaration under `Canvas additions`, no
`$.intent.recipe`/`geosolve-intent-recipe-v1`, exact source navigation and accepted finite geometry.
Edit a source-owned control point, reload, and require the same recipe/result ownership. For
Tangent Arc, also require its source-span dependency and contact/branch values to survive cold
replay. No TypeScript geometry equation or hand-copied result schema may appear.

Focused automated coverage has passed all 25 variants. Every single generated compact declaration
is at most `1,024` bytes; exact TypeScript examples are Cubic Bezier `387`, Tangent Arc `921` and
Periodic NURBS `872` bytes, with their normalized three-shape sketch at `3,087` bytes. The Rust
all-feature sketch-code suite passes `109` unit tests plus every integration/doc test; compact
round-trip passes `3/3`, editor insertion `13/13`, TypeScript runtime `27/27`, mutation `18/18`,
pinned-Deno `2/2`, frontend `54/54`, focused sketch-code Clippy and diff hygiene. The exact Compass
integration regression also passes after the final logical-root correction. Final qualification
passes demo-web `357/357`, formatting, affected warnings-denied Clippy, the locked WASM check,
optimized nine-file distribution validation and release-WASM Playwright `14/14`. The unchanged
normal product passes `13/13` before freezing and against frozen staging and live endpoints.

Historical F003 nomination identity: frozen path `/tmp/geosolve-m89-f003-uat.uF6Yjsc3`;
ordered-manifest aggregate
`c37d832a6dc076982e3fda8f2ffbcc8b2f26ed99ad6ffd0da0b7ba4f96d705ed`; optimized release-WASM
SHA-256 `99eeaa5668276282201cb1971b042d3d0904a13b7a75d40e2357ce08a34c748a`; evidence directory
`/tmp/geosolve-m89-f003-freeze-evidence.5TcRPlem`; endpoint
`http://100.94.63.83:18089/`; service/PID/invocation
`geosolve-m89-react-uat-current.service` / `3637680` /
`73bdb82cf2a64436ab6f65fe76b08ccf`. Staging and live strict ten-route HTTP ledgers are identical
at SHA-256 `c5efab20409773cc7f43cecdd9c6fca5b961402611d073f6bf0e8f40cca0ce1e`.

M89-F004/F005 targeted preflight is **Not run** and would apply only to the historical immutable
F005 replacement:

All 25 canvas geometry variants and all 33 standalone persistent constraint kinds reverse-project to compact, cold-replayable managed source. The two host-external schema variants remain compile-visible and schema-authenticated, but require separately supplied host snapshots.
Those variants are `ExternalPointCoincident` and `ExternalLineCollinear`. F004/F005 implement no
standalone host-snapshot execution path, so they remain gated and are not UAT replay rows.

Final provisional dirty-tree pre-UAT evidence passes exact gate
`nix-shell shell.nix --run 'GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh'` with sketch-code unit
`114`, compact geometry `4/4`, constraint matrix `1/1`, direct Fillet lowering `7/7`,
`m89_editor_insertion` `19/19`, demo-web `358/358`, TypeScript runtime `28/28`, mutation `19/19`,
pinned-Deno parity `2/2` and frontend `55/55`. The ambient attempt built through workspace and
golden checks before its first WASM parity leg reported `HARNESS_ERROR` because
`wasm-bindgen-test-runner` was absent after reboot. The pinned shell with runner `0.2.121` passed
the complete gate, so the ambient result was a harness/environment failure, not a product defect.
These final F004/F005 counts append to rather than replace the historical F003 evidence above.

- Author direct Horizontal and Vertical constraints, then representative non-axis and contact
  constraints. Require direct axis declarations and compact `$.constraint.recipe(...)` source for
  each standalone case, finite accepted state and no `$.intent.recipe(...)`. The automated catalog
  boundary is 33 standalone-replayable variants out of 35; `ExternalPointCoincident` and
  `ExternalLineCollinear` remain honestly host-external gated and are not manual replay rows.
- Author a right-angle computed Fillet and require one direct
  `$.computed.filletSet(...)` declaration with lexical parent spans, display name, radius,
  suppression and explicit branch/contact metadata. Require no generic transport declaration.
- Select that Fillet and require navigation to the whole authenticated declaration plus
  `Modifiable in source`. Edit the radius, confirm selection and accepted geometry survive, then
  suppress/restore, delete/Undo and reload. Each step must reproduce source, Explorer ownership and
  accepted computed geometry together with no runtime error.
- Keep the existing Profile Offset M89-U3 contract unchanged. F004/F005 introduce no new Offset
  work and no broader Offset acceptance claim.

The Compass Rose human retest, all targeted preflight observations and M89-U1 through M89-U8 remain
**Not run**. The immutable F005 identity above is current mechanical evidence, but there is no
human acceptance result or explicit milestone closeout yet.

1. Open a legacy managed sample, confirm it renders unchanged, then make one structured edit. Only
   the active copy should normalize to v2; reopening the bundled sample must restore its original
   bytes.
2. In Typed Panel, edit one shared radius without any `editLens`. Every runtime-recorded consumer
   must update once and unrelated controls must remain unchanged.
3. Draw one common recipe and Profile Offset on a code project. Each completed gesture must add one
   readable **user-facing declaration closure** under `Canvas additions`, with no hidden GUI
   duplicate. Exercise at least one Bezier or advanced curve as well as Polyline: Segment and
   Polyline use their direct compact builders, while the other 23 geometry variants use the compact
   descriptor-authenticated `$.geometry.recipe(...)` form above rather than a transport dump.
   Profile Offset retains an explicit aggregate helper declaration plus its root; the helper must
   remain selectable, editable and
   source-navigable while the root atomically owns closure reorder/delete. A shared aggregate must
   remain an independent root. Undo/Redo must restore source, closure order and canvas together.
4. Reorder independent declarations and suppress/restore a top-level declaration plus a generated
   patch member. Source order and explicit suppression must match the panel after reload. A
   dependency-invalid move must explain its refusal and change nothing.
5. Create an invalid candidate edit. The previous accepted canvas must remain visible, one source-
   positioned diagnostic must appear, and no accepted history entry may publish. Correcting it must
   publish all authorities together.
6. Use Unicode identifiers/comments before an editable value. Navigation, selection and rewrite
   must cover the exact CodeMirror characters, proving UTF-8 byte to UTF-16 conversion.
7. Export the project and run native inspect/render twice. Reports and SVG must be deterministic,
   finite and independently validated with normalized hard residual at most `1e-9`, without a web
   UI or Deno for the compiled artifact.
8. Compile the same managed source in browser and through the pinned native sidecar. Normalized
   source, IR, executed artifact and all digests must match exactly.
