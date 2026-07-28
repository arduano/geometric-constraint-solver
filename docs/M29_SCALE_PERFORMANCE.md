# M29 scale and performance envelope

## Correctness envelope

The release acceptance corpus runs representative sketch and linkage models at
uniform model scales `1e-6`, `1` and `1e6`. At those tested scales, topology,
explicit branch state, source ordering and rank/mobility classifications are
required to remain invariant. Every accepted solve independently validates a
maximum normalized hard residual of `1e-9` or less.

This is a tested set of scale points, not a claim that every finite coordinate or
every intermediate condition number is solvable. Named mixed-scale, cancellation,
rational-pole, spline and spatial fixtures in `docs/SCENARIOS.md` extend the tested
set. Unrepresentable products, unresolved normalization, invalid geometry and
non-finite values reject rather than becoming zero or convergence.

The 16 MiB JSON and 100,000-object import limits are defensive resource ceilings,
not interactive-performance promises.

## Reproducible workloads

| Workload | Supported release evidence |
| --- | --- |
| Core dense | Criterion `2x2`, `4x4`, `8x8` systems |
| Core representative | CAD-like 100/1,000/10,000 and linkage-like 99/999/9,999 tangent coordinates |
| Sparse crossover | Connected 64/128/256-column hard systems; `Auto` threshold remains 256 columns/rows/entries at density `<= 1/128` |
| Sketch interaction | M14 small and medium documents with current native import/first/incremental solve ceilings; browser-render evidence is historical |
| Advanced sketch | 1,000-control/128-contact NURBS locality and M28 105-pair correctness; no general interactive timing claim |
| Spatial release | 256 moving bodies, 1,536 active coordinates, validated `SparseQr`, dense-authoritative rank and 180-second ceiling |

Historical M14 and M16 measurements and exact workload boundaries remain in
`docs/M14_PERFORMANCE.md` and `docs/M16_SPARSE_CROSSOVER.md`. Their numbers are
reference observations, not tolerances used to weaken correctness.

## M29 reference environment

- date: 2026-07-21;
- tree: M28/M29 release work based on `4d2b74b`;
- OS: NixOS Linux x86-64, kernel `7.1.1`;
- native toolchain: `rustc 1.97.1`, `cargo 1.97.1`;
- declared MSRV: Rust `1.89`;
- browser build shell: Nix `rustc/cargo 1.95.0`, Trunk `0.21.14`,
  wasm-bindgen CLI `0.2.121`.

The final release record must identify a clean commit. Measurements from a dirty
tree are development evidence only.

## M29 development measurements

The 2026-07-21 release-candidate tree produced these p95 results. Native M14 rows
remain enforced; the browser row is a retired historical measurement:

| Measurement | Small | Medium | Budget |
| --- | ---: | ---: | ---: |
| JSON import | 0.449 ms | 0.519 ms | 20 / 150 ms |
| First solve | 1.849 ms | 51.550 ms | 500 / 4,000 ms |
| Incremental edit/solve | 4.558 ms | 111.778 ms | 300 / 1,500 ms |
| Browser render (historical) | 9.300 ms | 61.400 ms | 75 / 400 ms at the M29 checkpoint |

The 256-moving-body spatial release fixture solved and independently validated
1,536 active coordinates with the `SparseQr` step path in `88.29s`, below its
`180s` ceiling. Historically, the desktop five-stage scissor burst coalesced 40
pointer events to one render in `37ms`, below its then-current `100ms` ceiling.

## Commands

```bash
cargo bench --locked -p geosolve-core --bench small_dense
cargo bench --locked -p geosolve-core --bench representative_sparse -- --test
cargo bench --locked -p geosolve-core --bench connected_sparse
cargo run --locked --release -p geosolve-sketch --example m14_performance
cargo test --locked --release -p geosolve-linkage --test m23_performance \
  exact_auto_sparse_crossover_solves_and_validates_256_moving_body_chain \
  -- --exact --ignored --nocapture
```

Surviving release WASM build, from `crates/geosolve-demo-web`:

```bash
nix-shell ../../shell.nix --run 'trunk build --release'
```

The former browser timing invocation was deleted by M50 after its retained semantics received
direct owners. Its recorded measurements remain historical evidence, not a current gate.

## Budgets and rebaselining

The native M14 import/solve/edit ceilings and the M23 180-second release ceiling
remain enforced gates. The M14 browser-render/burst ceilings were retired with the
browser harness at M50. Criterion measurements are observational. A change outside normal noise requires
investigation but does not fail solely by percentage. Rebaselining an enforced
budget requires a documented workload or reference-environment change, preserved
before/after measurements and confirmation that residual, rank, branch and
validation policy did not change.
