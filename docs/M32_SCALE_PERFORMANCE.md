# M32 scale and performance evidence

## Scope and correctness boundary

This evidence extends the M29 envelope with representative M30 construction/NURBS
load-and-edit work and M31 all-family/NURBS-self profile analysis. It does not change
the supported scale points, solver configuration, profile completeness rules, or any
correctness tolerance.

The correctness envelope remains uniform model scales `1e-6`, `1`, and `1e6`, with
explicit topology/branch state and rank/mobility classification preserved. Every
accepted solve must still receive fresh independent hard/domain validation with a
maximum normalized hard residual of `1e-9` or less. An invalid or non-finite result is
never timed as success. Visual-profile `Complete` still requires all relevant roots,
tangent order, area sign, and containment decisions to resolve within the unchanged
explicit work limits; elapsed time cannot convert `Truncated` or `Skipped` into
`Complete`.

The native timing fixtures use model scale `1`. Scale correctness remains covered by
the M1-M31 acceptance corpus; in the relevant run below,
`all_family_profile_scenario_is_scale_invariant` and the broader M31 suite exercised
the required transformed/scale profile cases. These timings are not a claim that
every finite or ill-conditioned input has interactive performance.

## Native harness

Run from the workspace root:

```bash
cargo run --locked --release -p geosolve-sketch --example m32_performance
```

`m32_performance` uses two warmups and 12 measured samples. It reports the
nearest-rank p95 used by the existing M14 harness; with 12 samples this is the largest
sample. Setup excluded from a timed edit consists only of cloning the already accepted
session. Output assertions and canonical re-import checks run after the timer.

The measured boundaries are:

| Measurement | Start state | Timed work | Untimed postcondition |
| --- | --- | --- | --- |
| Construction load/solve | Allocated canonical JSON | Strict JSON import plus `SketchDocumentSession::new`, including solve, diagnostics, and independent acceptance validation | Accepted finite report, `HardValidity::Valid`, residual `<= 1e-9`, valid rank |
| Construction edit/solve | Clone of accepted supporting-offset session | Public target-endpoint command, solve, and atomic accepted publication | Associated target geometry moved, one history entry, canonical round trip |
| NURBS load/solve | Allocated canonical JSON | Strict JSON import plus independently validated session creation | Accepted finite report, residual `<= 1e-9`, valid rank |
| NURBS knot edit/solve | Clone of accepted local-support NURBS session | Public homogeneous knot-insertion command, solve, and atomic accepted publication | One new semantic span, one history entry, canonical round trip |
| Profile analysis | Accepted immutable document | Complete `analyze_visual_profiles` call and result allocation using the scene's existing M31 UAT options | Expected status/families/faces, finite publication, exact deterministic work signature, unchanged canonical JSON |

Representative document resources from the run were:

| Workload | Points | Scalars | Curves | Contacts | Constraints | Dimensions | Trim views | Canonical JSON |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `construction-supporting-offset` | 4 | 1 | 2 | 0 | 2 | 1 | 0 | 2,289 B |
| `nurbs-local-support` | 6 | 6 | 1 | 0 | 2 | 0 | 0 | 3,012 B |
| `profile-all-families` | 80 | 30 | 42 | 8 | 8 | 0 | 0 | 28,713 B |
| `profile-nurbs-self-intersection` | 4 | 4 | 1 | 0 | 0 | 0 | 0 | 1,896 B |

## Reference environment

- date: 2026-07-22;
- tree: clean M32 release candidate `8d6f648`;
- OS: NixOS Linux x86-64, kernel `7.1.1`;
- CPU: Intel Core i5-14400F, 10 cores / 16 logical CPUs, maximum 4.7 GHz;
- native toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`,
  `cargo 1.97.1 (c980f4866 2026-06-30)`;
- build: Cargo `release` profile, locked dependency graph;
- browser runtime: Chromium `149.0.7827.196`, headless with GPU disabled, at the
  desktop `1440x1000` CSS-pixel viewport;
- browser harness: Node.js `24.16.0`, Trunk `0.21.14`, release WASM bundle;
- process peak resident set: 9,348 KiB from Linux `VmHWM`, observational and
  process-wide rather than a per-workload allocation measurement.

The complete clean release gate reran these measurements on the same candidate before
M32 completion.

## Native measurements

The 2026-07-22 run produced:

| Measurement | Median | p95 | Conservative gate |
| --- | ---: | ---: | ---: |
| Supporting-offset load/solve | 0.104 ms | 0.113 ms | 1,000 ms |
| Supporting-offset edit/solve | 0.483 ms | 0.506 ms | 1,000 ms |
| Local-support NURBS load/solve | 0.205 ms | 0.318 ms | 2,000 ms |
| Local-support NURBS knot insert/solve | 0.256 ms | 0.340 ms | 2,000 ms |
| All-family profile analysis | 23.232 ms | 24.796 ms | 10,000 ms |
| NURBS self-profile analysis | 15.812 ms | 16.486 ms | 5,000 ms |

The ceilings are release fail-fast limits, not optimization targets. They are
deliberately well above the reference observations to tolerate shared CI and
toolchain noise. The harness always performs the full operation first and validates
correctness/completeness independently before checking elapsed time. It does not
change solver tolerances, iteration limits, rank policy, profile options, branch
state, or accepted-state validation to meet a gate.

## Deterministic profile resources

Every warmup and measured sample had the exact same count/work signature as the
baseline analysis. All-family output was `Complete` with 15 families, 30 faces, 30
contours, 98 directed edges, 31 intersections, and no issues. NURBS self-analysis was
`Complete` with one family, one face, one contour, two directed edges, one certified
self-intersection, and no issues.

| Counter | All-family consumed / limit | NURBS self consumed / limit |
| --- | ---: | ---: |
| Candidate pairs | 1,445 / 100,000 | 1 / 100,000 |
| Intersection subdivisions | 60 / 500,000 | 18 / 500,000 |
| Intersection roots | 31 / 100,000 | 1 / 100,000 |
| Fragments | 113 / 100,000 | 4 / 100,000 |
| Integration subdivisions | 206 / 500,000 | 126 / 500,000 |
| Containment tests | 420 / 100,000 | 0 / 100,000 |
| Faces | 30 / 10,000 | 1 / 10,000 |

These deterministic counters are the authoritative resource evidence for bounded
profile work. Peak RSS is reported for observation only because allocator, loader,
platform, and process history make it unsuitable as a portable correctness gate.

## Historical browser harness and measurements

Before M50, `m32BrowserPerformanceSuite` ran in both a focused mode and the full browser
suite. Each workload received two warmups followed by 12 measured samples.
The timer starts immediately before the existing **Load accepted example** action and
stops only after `#playground-root` publishes a strictly newer `renderSequence`.
Selection of the scene key and scale `1` is untimed. The timed work therefore includes
public scenario construction, session solve and independent acceptance validation,
profile analysis where applicable, and publication of the resulting DOM/SVG render.

Every sample is checked after the timer for accepted finite geometry and the exact
M30/M31 UAT status. Profile samples additionally recheck the existing family/face
expectations and every unchanged native consumed/limit profile counter. No solver
request, solver tolerance, profile option, rendering tolerance, branch state, or
completeness rule is changed for timing. Samples are sorted and p95 uses the same
nearest-rank rule as the native and legacy M14 harnesses; with 12 samples it is the
largest observation.

The 2026-07-22 focused release run produced:

| Scene load/solve/profile/render | Median | p95 | Conservative gate |
| --- | ---: | ---: | ---: |
| `construction-supporting-offset` | 2.050 ms | 4.300 ms | 1,000 ms |
| `nurbs-local-support` | 3.500 ms | 6.200 ms | 2,000 ms |
| `profile-all-families` | 64.600 ms | 81.400 ms | 10,000 ms |
| `profile-nurbs-self-intersection` | 29.000 ms | 35.300 ms | 5,000 ms |

The browser ceilings match the conservative native fail-fast classes and are not
optimization targets. They tolerate shared headless-CI and browser scheduling noise
without weakening any solver or profile correctness requirement.

## Verification

Commands run for this evidence:

```bash
cargo check --locked -p geosolve-sketch --example m32_performance
cargo run --locked --release -p geosolve-sketch --example m32_performance
cargo test --locked -p geosolve-sketch --test m30 --test m31
(cd crates/geosolve-demo-web && nix-shell ../../shell.nix --run 'trunk build --release')
```

The focused Chromium command is intentionally omitted because M50 deleted the old browser
harness after direct semantic replacement. The table above remains historical evidence.

The example check passed, the release harness passed all six timing gates and all
correctness/completeness/resource assertions, and the relevant native tests passed:
7 M30 tests and 31 M31 tests, with no failures or ignored tests. The release Trunk
build and focused M32 Chromium suite passed all functional retention checks and all
four browser timing gates on the first invocation.
