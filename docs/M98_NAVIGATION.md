<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 folder navigation measurements

These are development measurements on the current main-pc, with other development work
running. They establish behavior and report observed costs; isolated integrated qualification
is still pending and no new latency target is inferred.

`workspace-navigation.test.mjs` sends 21 actual HTTP requests per accepted folder: wheel
batches, hover, Center on origin, Fit and grid toggles. It checks every authored/cache file's
contents and filesystem identity, instruments write syscalls, records worker starts and
refuses full persistence serialization during navigation. All four sample checks pass with
zero writes, source recompiles or rollback serialization.

| Folder | Scene items | Median request | p95 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Small circle | 80 | 5.82 ms | 8.21 ms | 15.98 ms |
| Gridfinity | 281 | 16.62 ms | 91.94 ms | 92.60 ms |
| Manifold | 364 | 27.91 ms | 2,640.73 ms | 2,698.98 ms |
| Dense backplane | 534 | 40.24 ms | 73.26 ms | 91.26 ms |

The manifold's two Fit requests dominate its tail: 2.64 and 2.70 seconds. Its wheel/hover
requests in this capture range from 8.81 to 35.28 ms. These measurements do not establish
smooth full-browser rendering or a new Fit performance improvement. Existing renderer and
solver performance obligations remain in the integrated gate.

A generator Center on origin control separately passes with unchanged source/input state.
The former origin classification defect is [M98-F008](M98_HARDENING.md).

A deterministic queue check holds one watcher scan, invokes 1,001 watcher callbacks and
sends 140 concurrent real HTTP snapshot requests. Exactly one scan remains pending; 127
requests queue with it and 13 promptly refuse admission at the 128 total bound. Accepted
requests finish after release, later admission succeeds, the actual native worker remains
responsive, and unauthorized requests still receive 403. There are no writes or new workers.

Small owning regression command executed by the reviewer:

```bash
node --test --test-name-pattern='small folder navigation' scripts/workspace-navigation.test.mjs
```

Logs: `target/m98/workspace-navigation-small-r5.log` and
`target/m98/workspace-navigation-large-r3.log`; detailed JSON measurements live below
`target/m98/navigation/`. The same owning test file includes the queue and generator checks;
the review ledger records their focused runs separately.
