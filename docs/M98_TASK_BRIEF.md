<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 original prototype brief

This historical brief records the initial local-folder experiment. It was later
expanded into the [M98 implementation plan](M98_IMPLEMENTATION_PLAN.md), native
engine and multi-editor collaboration. Use [Getting started](GETTING_STARTED.md)
for current commands and [M98 qualification](M98_QUALIFICATION.md) for final evidence.

The purpose was to make an ordinary folder of editable TypeScript the durable
sketch source. An external editor or AI agent could save source while a person used
the existing canvas and Inspector. Both paths used the same managed-source
transactions, Rust materializer and independently validated solver.

The initial workflow was:

1. Initialize a new folder without overwriting existing files.
2. Serve it through the existing demo workbench.
3. Observe external saves, compile and validate them, then update accepted geometry.
4. Write supported GUI edits back to source without watcher loops.
5. Reject stale edits while preserving disk contents and pending intent.
6. Reopen from disk and retain useful diagnostics when current source is invalid.

The prototype deliberately covered one entry file and one editing browser, with
loopback transport, bounded paths and per-session authorization. It reused the
managed authoring subset and manual project export. Arbitrary TypeScript reverse
editing, collaboration, multi-file imports and a 3D kernel were outside that first
cut. Later M98 work added imports, editable semantic sidecars, profile export,
headless generators and collaborative authoring; it did not add a solid modeler.

Initial delivery required actual two-way browser/file evidence and focused checks
for atomic external saves, invalid source retention, stale writes and ordinary demo
startup. Prototype readiness was distinct from release qualification and human
acceptance. The final M98 product is mechanically qualified; its human UAT remains
open as recorded in [M98 UAT](M98_UAT.md).
