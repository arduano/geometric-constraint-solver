# M33 deterministic representative baselines

## Scope

This document freezes six deterministic benchmark workload shapes for the current
sketch-v4 implementation. The fixtures are benchmark-only support in
`crates/geosolve-sketch/benches/support/m33_representative.rs`; they add no library
API, persisted variant, solver equation, host callback, or target-only behavior.

The benchmark keys are exactly:

1. `connected`;
2. `disconnected`;
3. `spline_heavy`;
4. `parameter_heavy`;
5. `external_reference`;
6. `activation_heavy`.

`parameter_heavy`, `external_reference`, and `activation_heavy` are explicitly
**current-v4 workload-shape proxies, not APIs**. They use existing v4 driving
scalars, fixed local geometry, and source suppression respectively. They do not
claim host-parameter batches, immutable external snapshots, effective activation,
input revisions, digests, provenance, or stale-commit behavior. Those contracts
remain future milestones.

## Exact shapes

All coordinates use model scale `10`. Construction is formulaic, contains no random
input, and uses fixed document namespaces `0x33010000` through `0x33060000` in the
key order above.

| Key | Exact representative shape | Warm edit |
| --- | --- | --- |
| `connected` | One unanchored chain of 64 directed lines over 65 points `P_i = (1.25i, 0.4 sin(0.31i))`; every line has one driving length scalar/dimension. This is one reduced hard component. Previous-state preferences are disabled for this deliberately underconstrained shape. | Increase segment 32's driving length by 1%. |
| `disconnected` | 32 ordinary rectangle macros in an 8-column grid. Width is `2 + 0.05(i mod 5)` and height is `1.25 + 0.05(i mod 3)`. Every rectangle retains its ordinary anchor, four axis constraints, and two driving dimensions. | Increase rectangle 0 width by 2%. |
| `spline_heavy` | 16 disjoint clamped degree-3 NURBS, each with eight distinct controls, eight positive weights, five support spans, knot vector `[0,0,0,0,1,2,3,4,5,5,5,5]`, and its first control fixed. Curves occupy separate y bands; previous-state preferences are disabled. | Move curve 0's middle control by `+0.2` in y through the public point command/temporary target path. |
| `parameter_heavy` | **Current-v4 parameter-heavy workload-shape proxy (not an API):** 64 independent horizontal line cells. Each cell has a fixed first point, horizontal relation, positive driving length scalar, and curve-length dimension. | Increase cell 32's current-v4 driving scalar by 1%. |
| `external_reference` | **Current-v4 external-reference workload-shape proxy (not an API):** 32 cells in an 8-column grid. Each cell has a two-point fixed support line standing in for immutable input shape, one local point, and two driving point-distance dimensions from support endpoints to that local point. | Increase cell 0's first distance target by 1%. |
| `activation_heavy` | **Current-v4 activation-heavy workload-shape proxy (not an API):** 32 ordinary rectangles. For each rectangle the right-vertical relation, top-horizontal relation, and height dimension are retained but suppressed, yielding 96 suppressed and 128 active sources. | Unsuppress rectangle 0's right-vertical source. |

The connected and spline-heavy requests intentionally omit previous-state
preferences. Their warm edits still use ordinary public commands, and every measured
solve still performs the production hard/domain validation path. This keeps those
two shapes focused on connected hard solving and spline definition/profile work
rather than a large secondary minimum-motion objective.

## Frozen signatures

Canonical checksum is 64-bit FNV-1a over exact canonical v4 JSON bytes, with offset
basis `0xcbf29ce484222325` and prime `0x100000001b3`. These are deterministic drift
detectors, not security digests and not future external-snapshot digests.

| Key | Points | Scalars | Curves | Contacts | Constraints | Dimensions | Trim views | Active / suppressed sources | JSON bytes | FNV-1a64 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `connected` | 65 | 64 | 64 | 0 | 0 | 64 | 0 | 64 / 0 | 57,080 | `06952493bd1a274e` |
| `disconnected` | 128 | 64 | 128 | 0 | 160 | 64 | 0 | 224 / 0 | 124,903 | `32d8108e5878277e` |
| `spline_heavy` | 128 | 128 | 16 | 0 | 16 | 0 | 0 | 16 / 0 | 54,942 | `d7ab4c46461cae67` |
| `parameter_heavy` | 128 | 64 | 64 | 0 | 128 | 64 | 0 | 192 / 0 | 101,294 | `0bf10acdd03aa4e1` |
| `external_reference` | 96 | 64 | 32 | 0 | 64 | 64 | 0 | 128 / 0 | 72,037 | `7f3f8ab12cf0d451` |
| `activation_heavy` | 128 | 64 | 128 | 0 | 160 | 64 | 0 | 128 / 96 | 132,961 | `d08a5a2d314a0ef9` |

Fresh accepted solve/report signatures are:

| Key | Tangent coordinates | Active hard rows | Reduced components | Numerical rank | Right nullity | Audit sources / rows |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `connected` | 130 | 64 | 1 | 64 | 66 | 64 / 64 |
| `disconnected` | 256 | 192 | 64 | 192 | 0 | 352 / 512 |
| `spline_heavy` | 368 | 0 | 240 | 0 | 336 | 16 / 32 |
| `parameter_heavy` | 256 | 128 | 128 | 128 | 0 | 320 / 512 |
| `external_reference` | 192 | 64 | 96 | 64 | 0 | 224 / 384 |
| `activation_heavy` | 256 | 96 | 128 | 96 | 96 | 256 / 416 |

Fixed elimination explains why several reported reduced-component and active-row
counts differ from the visible domain-cell count. The spline shape has 16 fixed
control sources, but those rows are eliminated from the active hard systems. All
remaining unconstrained controls and non-gauge weights are still represented in the
tangent-coordinate and right-nullity evidence.

Default visual-profile analysis is `Complete` with no issue for all six inputs:

| Key | Families | Faces | Intersections | Candidate pairs | Fragments |
| --- | ---: | ---: | ---: | ---: | ---: |
| `connected` | 1 | 0 | 0 | 2,016 | 64 |
| `disconnected` | 1 | 32 | 0 | 8,128 | 128 |
| `spline_heavy` | 1 | 0 | 0 | 3,240 | 80 |
| `parameter_heavy` | 1 | 0 | 0 | 2,016 | 64 |
| `external_reference` | 1 | 0 | 0 | 496 | 32 |
| `activation_heavy` | 1 | 32 | 0 | 8,128 | 128 |

## Measurement boundaries

Criterion groups and IDs are exactly the four group names below crossed with the six
keys above, for 24 cases. Every group uses sample size 10, 200 ms warmup, and 750 ms
target measurement time. Criterion may lengthen collection to obtain its minimum
sample count.

| Group | Setup excluded from elapsed time | Included timed work | Work after elapsed time |
| --- | --- | --- | --- |
| `production_cold_compile` | Build and validate the deterministic document; clone one input for the iteration. | `SketchDocument::lower` followed by `Sketch::compile`, including document validation, deterministic runtime remapping, residual declaration, bounds, audit descriptors, and core problem construction. | Black-box and destroy lowering/compile results. |
| `production_warm_edit_solve` | Build, solve, independently validate, and clone one accepted `SketchDocumentSession`; construct its revision-checked command. | `SketchDocumentSession::apply`, including its internal candidate clone, edit, lowering/compile, solve/diagnostics, mandatory independent validation, accepted projection, and atomic publication. | Black-box and destroy the result and cloned session. |
| `production_solve_diagnostics` | Build and validate the deterministic document; clone one input for the iteration. | `SketchDocumentSession::new` with the shape's request and default solver policy, including document validation, lowering/compile, solve, rank and bounded diagnostics, audit, independent domain validation, and accepted-state projection. | Black-box and destroy the result. |
| `production_visual_profile` | Retain an already independently accepted immutable document and default profile options. | Complete `SketchDocument::analyze_visual_profiles` call and output allocation. | Black-box and destroy the analysis. |

The harness preflights every representative input before registering timed work. It
validates canonical document structure, successful lowering/compile, accepted warm
edit, finite `HardValidity::Valid` reports, independent normalized residual at most
`1e-9`, valid rank, evaluated finite audit rows, `Complete` profiles, finite profile
publication, unchanged resource limits, and the frozen signatures above. This
harness-level validation is outside timed loops. Mandatory validation performed by
the production operation itself remains inside the operation boundary and is never
bypassed.

No benchmark has a performance threshold. Criterion intervals are observations, not
acceptance tolerances, crossover policy, or permission to weaken numerical/profile
correctness.

Memory is separate from the four timing boundaries. On Linux the harness reads
`VmHWM` from `/proc/self/status` after all groups. That value is process-wide and
includes Criterion, all prepared fixtures, allocator retention, and every group; it
is not a per-workload allocation count or portable budget.

Cancellation latency is unavailable in M33 because the current public sketch-v4
operations have no cancellation mechanism or checkpoint contract. No cancellation
number or surrogate polling benchmark is reported. Cooperative cancellation and its
latency boundary begin at M35.

## Reference observation

Observed on 2026-07-22 from a dirty M33 development tree based on commit
`11dec459b600`; these values are not release-candidate evidence:

- OS: Linux x86-64, kernel `7.1.1`;
- CPU: Intel Core i5-14400F, 10 cores / 16 logical CPUs, maximum 4.7 GHz;
- toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, LLVM `22.1.6`,
  `cargo 1.97.1 (c980f4866 2026-06-30)`;
- build: Cargo `bench`/release profile with `--locked`, Criterion plotters backend
  because gnuplot was unavailable.

Criterion's bracketed estimates from the observational run were:

| Key | Cold compile | Warm edit/solve | Solve/diagnostics | Visual profile |
| --- | ---: | ---: | ---: | ---: |
| `connected` | 0.125 ms `[0.122, 0.127]` | 27.773 ms `[27.367, 28.224]` | 12.223 ms `[12.145, 12.298]` | 0.154 ms `[0.146, 0.161]` |
| `disconnected` | 12.145 ms `[12.058, 12.274]` | 68.307 ms `[66.872, 69.994]` | 32.362 ms `[31.861, 32.938]` | 4.802 ms `[4.706, 4.928]` |
| `spline_heavy` | 8.388 ms `[8.143, 8.596]` | 30.040 ms `[29.648, 30.428]` | 14.562 ms `[14.412, 14.760]` | 47.045 ms `[46.435, 47.800]` |
| `parameter_heavy` | 26.368 ms `[25.925, 26.862]` | 120.220 ms `[119.180, 121.200]` | 57.913 ms `[57.200, 58.656]` | 0.471 ms `[0.464, 0.478]` |
| `external_reference` | 16.515 ms `[16.182, 16.859]` | 60.830 ms `[60.291, 61.477]` | 31.122 ms `[30.748, 31.557]` | 0.143 ms `[0.139, 0.146]` |
| `activation_heavy` | 10.566 ms `[10.467, 10.735]` | 65.102 ms `[64.206, 65.995]` | 31.539 ms `[30.880, 32.375]` | 4.863 ms `[4.816, 4.896]` |

The same run reported process-wide peak RSS `70,212 KiB`. It is observational only
under the memory boundary above.

## Commands

Run from the workspace root:

```bash
cargo test --locked -p geosolve-sketch --test m33_benchmarks
cargo bench --locked -p geosolve-sketch --bench m33_representative --no-run
cargo bench --locked -p geosolve-sketch --bench m33_representative -- --test
cargo bench --locked -p geosolve-sketch --bench m33_representative
```

The normal test uses smaller versions of all six shapes for complete
compile/solve/edit/diagnostic/profile validation and separately checks the exact
representative document counts and canonical checksums. Criterion test mode validates
all 24 exact representative cases without imposing timing thresholds.
