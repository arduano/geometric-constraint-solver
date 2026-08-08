<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M67 implementation — legacy surface and harness cleanup

Status: complete and explicitly approved by the supervising human on 2026-08-08.

## 1. Files and APIs

M67 removes only audited legacy or unowned surfaces:

- the sole workbench's Production topology, Host-state evidence and Accepted redundancy cards;
- the frozen M40 browser-evidence/transition qualification harness after all fourteen cases have
  reviewed dispositions and every retained executed semantic has a direct owner;
- private generic local-AD and sketch-lowering code with no production caller; and
- orphan presentation styles, tombstone scans and duplicate release-gate invocations.

The retained M32 supporting-offset performance workload now validates movement of the endpoint it
edits. Its previous assertion required incidental movement from the other free endpoint of a valid
two-DOF solve, contradicting the current direct M30 behavior owner and predictable-motion policy.

The separately routed `/#/dev/lab` application was already deleted in M50. M67 adds no router,
alternate workbench or browser E2E replacement. The doc-hidden, post-`0.2.0`
`M40Qualification*`, `m40_qualification_corpus`, `run_m40_qualification` and
`validate_m40_qualification_matrix` evidence surface is intentionally removed. No released domain
API is replaced by a browser or presentation API.

## 2. Mathematical behavior

M67 adds and changes no residual equation, Jacobian formula, hard/temporary/preference semantics,
rank or mobility rule, branch/orientation state, convergence tolerance or success-publication
condition. The unused general local-AD prototype and its unused normalized-tangent storage branch
are removed; live Pose2/Pose3 local-difference AD and independent finite-difference coverage
remain.

The M32 change is an oracle correction only: the same edit, solve request, timing boundary and hard
validation execute, while the witness no longer treats one arbitrary valid allocation of passive
motion as mathematical behavior.

Topology, lifecycle, redundancy, diagnostic and audit DTOs remain reusable domain behavior even
though the workbench no longer renders their raw developer cards. Problems, canvas attribution and
global-error fallback remain the user-facing diagnostic surface.

## 3. Commands and outcomes

Nominated source: `3d52b29fc11f5cef572fe86f58a95897ec8c8214` on `main`. It was the sole
registered worktree, with no stash, untracked file or tracked modification.

The exact clean command

```text
nix-shell shell.nix --run './scripts/release-gate.sh'
```

passed on 2026-08-08. It completed locked offline metadata, format and diff checks;
warnings-denied workspace Clippy; the locked all-feature workspace suite; all-feature WASM;
warnings-denied rustdoc; benchmark compilation; M14 and M32 release workloads; the explicit
256-moving-body sparse release test; dependency licences; package contents for all eight
publishable crates; and Trunk 0.21.14's optimized release build.

The test inventory command

```text
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features -- --list'
```

listed 1,193 tests across 96 suite binaries. The normal full suite passed with its declared manual
and release-only ignored tests left ignored; the release gate separately ran the ignored
1,536-coordinate spatial case, which passed in 114.79 seconds. Focused retained-consumer counts
were editor 183, demo-web 65, renamed sketch 3 and renamed linkage 2, all passing.

The corrected M32 workload passed with p95 `0.193/0.689 ms` for supporting-offset load/edit,
`0.291/0.446 ms` for NURBS load/knot insertion, and `17.760/11.144 ms` for all-family/NURBS-self
profile analysis, all below their respective `1/1/2/2/10/5 s` budgets.

Additional literal boundary checks passed:

```text
nix-shell shell.nix --run 'RUSTFLAGS="-D dead-code" cargo check --locked -p geosolve-core -p geosolve-sketch -p geosolve-constraint-editor --lib'
nix-shell shell.nix --run 'RUSTFLAGS="-D warnings" cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
```

Static inventory reported one `#[wasm_bindgen(start)]`, one `workbench-root`, zero forbidden
runtime references and zero E2E directories. The seven-file distribution manifest is
`e9d410c71290e7200595aaf9be6327523a812a1fa7d23abfa9d12c8279c176ac`; all seven HTTP
responses matched their local SHA-256 values at the recorded Tailscale endpoint.

## 4. Acceptance criteria

Mechanically passed:

- one workbench startup/root and no alternate lab runtime, developer card or browser harness;
- reviewed dispositions for all fourteen former M40 transition cases and direct current owners for
  every retained executed semantic;
- ordinary authoring, editable Samples, persistence, camera, Problems and computed Fillets to
  remain qualified;
- canonical sketch v1-v4 and workspace v1-v4 migration behavior to remain unchanged; and
- the full locked release gate.

The supervising human explicitly approved all four focused areas in `docs/M67_UAT.md` on
2026-08-08 and requested M67 closure.

## 5. Known limitations or next blocker

M67 intentionally does not address `M66-KL001`, add Offset/Mirror, redesign computed-feature
branches, add topology presentation elsewhere, or harden new solver behavior. Cargo continues to
emit its pre-existing advisory that workspace packages specify both `license` and `license-file`;
licence validation and all package-content gates pass. No blocker remains within the approved M67
scope. M68 is an empty placeholder awaiting supervising-user scope.
