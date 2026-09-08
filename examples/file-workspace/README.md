<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Local sketch folder prototype

M98 is a prototype for human testing, pinned to the committed pre-metadata M97 base.
Run everything from the allocated M98 worktree. Node provides local file/HTTP transport;
the existing pure Rust WASM workbench owns compilation receipts, solving and the scene.

One-time build (Nix-friendly, two Cargo jobs, private outputs):

```bash
export CARGO_BUILD_JOBS=2
export BINARYEN_CORES=2
export npm_config_cache="$PWD/target/m98/npm-cache"
export DENO_DIR="$PWD/target/m98/deno"
nix-shell shell.nix
npm ci --prefix packages/geosolve-intent
npm run build --prefix packages/geosolve-intent
npm ci --prefix packages/geosolve-sketch-code
npm ci --prefix crates/geosolve-demo-web/frontend
npm run wasm:release --prefix crates/geosolve-demo-web/frontend
node crates/geosolve-demo-web/frontend/scripts/build-workspace.mjs
npm run build:ui --prefix crates/geosolve-demo-web/frontend
```

Then, from the worktree root:

```bash
node scripts/file-workspace.mjs init target/my-sketch
node scripts/file-workspace.mjs serve target/my-sketch
# Open the exact printed URL (includes a per-session token).
node scripts/file-workspace.mjs status target/my-sketch
node scripts/file-workspace.mjs check target/my-sketch
```

`init` accepts a new or empty folder and refuses to overwrite either project file.
`serve`/`open` choose an unused loopback port by default; `--port N` requests one explicitly
and fails if occupied. `check` independently compiles/solves disk, reports JSON diagnostics,
and exits 1 for invalid source. `status` queries the running bridge and includes current and
accepted content hashes/revisions, paths, errors, write and external-apply counts.

Edit `value: mm(10)` in `sketch.ts` externally. After save, the open canvas and `ringRadius`
Inspector value update. Edit the same Inspector field and press Enter: the file changes.
Switch to Split to see the accepted plaintext. Source edits in the Code editor use Apply.
Invalid disk saves retain their text and the prior geometry with an explicit error status.
On a conflict, download pending intent, Refresh from disk, and retry the intended edit manually.
Source drafts remain in the editor until Revert. The bridge does not auto-merge.

File → Export canonical project… and Download sketch.ts… remain the existing manual exports.
Open the server's bare URL without `?folder=1` for the ordinary standalone demo, samples,
browser saving and import. Folder mode does not replace its chosen sketch with a sample/import.

Only `sketch.ts` is watched (100 ms content polling, 120 ms settle debounce, including rename).
The manifest is read at startup and fixed to that entry. SDK imports and the current managed
authoring subset are supported; local dependencies and custom patch modules are not. The existing
writer preserves supported comments/labels, not arbitrary TypeScript ASTs.

Source-backed geometry creation, dimensions, source Apply and Undo/Redo are persisted. Point/grip
dragging is blocked because the existing native instance overlays are outside this source-only
prototype. Camera, selection, pins, visibility and history are session-only; restart reconstructs
the accepted source and fits the view. Ordinary external edits keep the running camera and
meaningful selection using the existing workbench transaction.

`.geosolve/last-good.ts` is a derived plaintext recovery cache, never startup design authority.
`.geosolve/session.json` records the bridge URL/token/PID. `before-*.ts` files retain displaced
source during UI writes, including external writes through old open file descriptors. Publication
uses an exclusive same-filesystem hard link after claiming the prior file, so a racing external
rename cannot be overwritten. There is a brief missing-entry interval; interruption can leave the
old source at the reported recovery path. This is a Linux/local-filesystem prototype, not a general
filesystem transaction layer. Recovery files accumulate and may be removed manually after review.
Never treat these caches as a competing authoritative geometry snapshot.

The preview is loopback only. For a remote machine use, for example,
`ssh -L 18108:127.0.0.1:18108 user@host`, then open the printed URL locally (same port/token).
Choose the actual printed port on both sides; no public deployment is implied.
