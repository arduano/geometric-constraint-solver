<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Recorded generator acceptance fixtures

`channel.json` and `manifold.json` are outputs of the real M98 TypeScript SDK
runtime recorder, supplied by the recorder implementation agent on 2026-09-09.
The manifold executes the existing source-native M97 water manifold and its local
patch definitions through ordinary TypeScript. Neither fixture contains managed
source spans, fabricated compiler receipts or solver equations.

Tests independently assert geometry and compare the full manifold against its
accepted managed counterpart; these JSON bytes are input, not a golden answer.
