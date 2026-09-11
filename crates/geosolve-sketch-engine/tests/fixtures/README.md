<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Recorded generator acceptance fixtures

`channel.json` and `manifold.json` are outputs of the real M98 TypeScript SDK
runtime recorder, supplied by the recorder implementation agent on 2026-09-09.
The manifold executes the existing source-native M97 water manifold and its local
patch definitions through ordinary TypeScript. Neither fixture contains managed
source spans, fabricated compiler receipts or solver equations.

Tests independently assert geometry and compare the full manifold against its
accepted managed counterpart; these JSON bytes are input, not a golden answer.
Construction compiler snapshots in `construction-compilations.json` come from real
native insertion tickets. Regenerate explicitly with
`cargo test --locked -p geosolve-sketch-engine --test construction write_real_compiler_requests -- --exact --ignored`,
then `node crates/geosolve-sketch-engine/tests/fixtures/generate-construction.mjs`.
Ordinary native tests use these checked-in compiler outputs and require no Node process.

`construction-parity-compilations.json` likewise contains actual compiler outputs for
all 25 native construction recipes, including a Tangent Arc referencing the accepted
Segment. Regenerate using the explicitly ignored native
`construction_parity::write_all_recipe_compiler_requests` test and then
`node crates/geosolve-sketch-engine/tests/fixtures/generate-construction-parity.mjs`.
The engine test requires independent source replay, compiler/native parity, finite
accepted geometry, cold reconstruction and one Undo/Redo transaction for every recipe.
These receipts are never synthesized in a Rust test.

`tool-operation-basis.json`, `tool-operation-feature.json`, and
`tool-operation-rejection.json` are genuine managed compiler outputs for the existing
tool families, a minimal open Polyline with omitted `closed`, and an impossible
Horizontal attempt followed by a valid retry. Regenerate them with
`node crates/geosolve-sketch-engine/tests/fixtures/generate-tool-operations.mjs --basis`,
`--feature`, or `--rejection`, respectively. `tool-operation-compilations.json`
contains genuine terminal compiler results for all 20 relation/dimension/feature
tools and selected geometry role. Regenerate terminal requests with
`cargo test --locked -p geosolve-sketch-engine --test tool_operations write_real_compiler_requests -- --exact --ignored`,
then run the generator with no flag. Native tests authenticate receipts and compare
native previews, independent validation, source replay, cold restore and history.
