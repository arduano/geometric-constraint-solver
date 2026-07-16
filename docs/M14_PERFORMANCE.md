# M14 playground performance

## Scope

M14 enforces responsiveness budgets without changing solver tolerances, rank policy, branch validation, diagnostic work, or accepted-state validation. The fixtures are deterministic public `SketchDocument` graphs built by `alpha_performance_document`:

| Workload | Points | Scalars | Curves | Contacts | Constraints | Dimensions | Canonical JSON |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Small | 16 | 16 | 11 | 5 | 14 | 6 | 13,866 bytes |
| Medium | 128 | 128 | 88 | 40 | 112 | 48 | 109,626 bytes |

Each mixed tile contains A1 constrained rectangle, A3 line-circle tangency, A4 free-radius circle-arc tangency, and A5 Bezier tangent-line geometry. Medium contains eight translated disconnected tiles. Persistent IDs and insertion order are fixed.

## Boundaries

- Import starts with canonical JSON already allocated and measures `SketchDocument::from_json`.
- First solve starts with a parsed document and measures `SketchDocumentSession::new`, including independent residual/domain validation and diagnostics.
- Incremental edit/solve starts with an accepted session clone and measures one public width-target command through accepted projection and independent validation.
- Browser render starts with accepted WASM state and measures one zoom action, complete SVG/inspector DOM replacement, and synchronous event handling.
- Native measurements use two warmups and 12 samples. Browser measurements use 12 alternating zoom samples. The reported and enforced statistic is p95.

## Budgets

| Measurement | Small p95 budget | Medium p95 budget |
| --- | ---: | ---: |
| JSON import | 20 ms | 150 ms |
| First solve | 500 ms | 4,000 ms |
| Incremental edit/solve | 300 ms | 1,500 ms |
| Browser render | 75 ms | 400 ms |

The limits are alpha interaction ceilings rather than solver microbenchmark targets. A budget failure must be fixed by implementation or by explicitly reducing the supported alpha document envelope, never by weakening correctness policy.

## Reference Run

Recorded 2026-07-16 on Linux x86-64, Intel Core i5-14400F, 16 logical CPUs, Rust 1.94.0, Node 24.16.0, Chromium 149.0.7827.196, and Trunk 0.21.14:

| Measurement | Small median / p95 | Medium median / p95 |
| --- | ---: | ---: |
| JSON import | 0.044 / 0.301 ms | 0.377 / 0.558 ms |
| First solve | 0.889 / 1.289 ms | 42.174 / 44.067 ms |
| Incremental edit/solve | 1.922 / 2.680 ms | 85.815 / 89.777 ms |
| Browser render | p95 11.200 ms | p95 82.600 ms |

Run the native gate from the workspace root:

```bash
cargo run --locked --release -p geosolve-sketch --example m14_performance
```

Build and run the browser gate from `crates/geosolve-demo-web`:

```bash
nix-shell ../../shell.nix --run 'trunk build --release'
node e2e/m14.mjs
```

The Chromium suite uses only Node standard-library APIs, including the global `WebSocket`, and raw DevTools Protocol. It requires Node 22 or newer, an external Chromium executable, and no npm packages, WebDriver, or browser-test framework.
