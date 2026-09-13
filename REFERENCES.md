<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Libraries and reference projects

The mathematical and implementation references below informed GeoSolve's design.
[Cargo.toml](Cargo.toml) and [Cargo.lock](Cargo.lock) record current Rust dependencies;
frontend/package lockfiles record JavaScript dependencies. Review compatibility
and qualification before upgrading them.

## Rust libraries

### Primary dependencies

- `nalgebra 0.35.0` — small fixed-size geometry, transformations, dense QR/SVD and test-sized linear algebra. Used in shared geometry and dense numerical paths.
  https://docs.rs/nalgebra/latest/nalgebra/
- `faer 0.24.4` — pure Rust sparse matrix storage, QR/least-squares and optional Cholesky fast paths; numerical and dense/sparse agreement contracts remain independently tested.
  https://docs.rs/faer/latest/faer/sparse/linalg/solvers/
- `slotmap 1.1.1` — stable generational IDs for entities, variables and constraints.
  https://docs.rs/slotmap/latest/slotmap/
- `indexmap 2.14.0` — deterministic insertion order where user-visible/report ordering matters.
  https://docs.rs/indexmap/latest/indexmap/
- `petgraph 0.8.3` — variable/residual incidence graph, connected components and structural matching/decomposition.
  https://docs.rs/petgraph/latest/petgraph/
- `num-dual 0.14.1` — internal dependency for component-local forward-mode automatic differentiation. The local AD formula/adapter remains private, and analytic or AD derivatives must still pass the independent central finite-difference oracle.
  https://docs.rs/num-dual/latest/num_dual/
- `thiserror 2` — structured construction/validation errors; do not use errors to hide ordinary solve outcomes.

### Testing and benchmarking

- `approx 0.5` — simple floating-point assertions; geometry acceptance should still use explicit scale-aware bounds.
- `proptest 1.11` — construct-valid/perturb/recover, invariance and invalid-input properties.
- `criterion 0.8` — M3 small-dense and M8+ representative benchmarks; performance is not a substitute for validity.
- central finite differences — mandatory in-house Jacobian oracle for analytic and `num-dual` derivatives.

### Browser and authoring layers

- `wasm-bindgen` exposes the Rust engine to JavaScript. Its CLI must match the exact
  crate version in `Cargo.lock`; the [build guide](docs/GETTING_STARTED.md) explains setup.
- React and Vite host the workbench; PixiJS supplies its accelerated canvas renderer.
  CodeMirror provides the source editor. Trunk and the earlier direct DOM/SVG
  application are historical implementations.
- TypeScript powers the typed authoring/compiler packages. Deno runs restricted
  source execution checks; Node hosts project files, generators and collaboration.
- Automerge underlies shared text in the Rust collaboration layer. Accepted geometry
  still goes through independent native validation and server authority.

### Libraries to study, not adopt as the foundation

- `levenberg-marquardt` — clean dense LM API and reporting ideas, but not sparse/CAD-aware.
  https://docs.rs/levenberg-marquardt/latest/levenberg_marquardt/
- `argmin` — optimization framework and trust-region references, but it does not supply CAD decomposition/diagnostics.
  https://docs.rs/argmin/latest/argmin/
- `tiny-solver`, `sophus_opt`, `apex-solver` — block/factor-graph and manifold-solving architecture references.
- `solverang 0.1` — feature claims are directly relevant, but it was too new and had too little public provenance/adoption at scaffold time. It was not adopted as a foundational dependency.
- `inari 2` — interval-arithmetic research reference; not an adopted solver dependency.

## Reference implementations

### SolveSpace

Repository and revision inspected during architecture research:

- repository: https://github.com/solvespace/solvespace
- inspected revision: `32158eb6d270bdabcbd225dcc2dc3b9cd606a2d1`
- solver overview: https://solvespace.github.io/solvespace-web/tech.html
- linkage tutorial: https://solvespace.com/linkage.pl

Most relevant source files:

- `src/system.cpp` — nonlinear iteration, sparse QR, rank/DOF and bad-constraint diagnosis;
- `src/constrainteq.cpp` — high-level constraints to residual equations;
- `src/expr.*` — symbolic expression DAG and exact symbolic partial derivatives;
- `src/sketch.h`, `src/solvespace.h` — handles, parameters and entities;
- `src/generate.cpp` — workplanes, prior-state reuse and branch-sensitive angle behavior.

Borrow:

- stable handles;
- separation of high-level constraints and scalar equations;
- exact derivative generation concept;
- previous-state/local branch behavior;
- independent rank/DOF reporting.

Do not copy blindly:

- fixed unknown limits;
- limited graph decomposition;
- modified Newton as the only nonlinear strategy;
- distributed implicit branch rules.

SolveSpace is GPL-3.0-or-later. Preserve attribution/notices for any directly translated code; prefer independently written implementations of published numerical techniques.

### FreeCAD Sketcher / PlaneGCS

- repository: https://github.com/FreeCAD/FreeCAD
- inspected revision: `a7b6badda77eb1f741bc7332e83c051673cf4772`
- source directory: https://github.com/FreeCAD/FreeCAD/tree/main/src/Mod/Sketcher/App/planegcs
- API docs: https://freecad.github.io/SourceDoc/d2/d69/classGCS_1_1System.html

Most relevant source files:

- `GCS.cpp`, `GCS.h` — system assembly, algorithms and diagnostics;
- `SubSystem.*` — dense/sparse Jacobians and subsystem behavior;
- `Constraints.*` — residuals, gradients, branch/step limits;
- `Geo.h` — derivative-carrying geometry helpers.

Borrow:

- bipartite connected-component decomposition;
- equality substitution;
- source tags mapping multiple residuals to one UI constraint;
- temporary drag constraints separated from normal constraints;
- DogLeg/LM fallback strategy;
- QR-based conflict/redundancy/dependent-parameter diagnostics.

Do not copy blindly:

- raw `double*` parameter ownership;
- large hand-coded derivative surface without a checker;
- branch policy distributed invisibly across constraint classes.

PlaneGCS files are LGPL-2.1-or-later. Preserve notices and check licence obligations for direct translations.

### Ceres Solver

Use as a conceptual/API reference for residual blocks, local parameterizations/manifolds, robust nonlinear least squares and solver reports—not as a dependency because this project requires pure Rust.

- modeling and manifolds: https://ceres-solver.readthedocs.io/latest/nnls_modeling.html
- solving: https://ceres-solver.readthedocs.io/latest/nnls_solving.html

### Mechanism references

- Modern Robotics closed-chain kinematics: https://modernrobotics.northwestern.edu/nu-gm-book-resource/7-2-closed-chains/
- Modern Robotics singularities: https://modernrobotics.northwestern.edu/nu-gm-book-resource/5-3-singularities/
- Simbody multibody graph/tree and loop constraints: https://simbody.github.io/3.8.0/classSimTK_1_1MultibodyGraphMaker.html
- Project Chrono revolute-joint constraint definition: https://api.projectchrono.org/9.0.0/classchrono_1_1_ch_link_revolute.html
- Baraff constrained dynamics notes as a reference for the explicitly excluded force/dynamics boundary: https://www.cs.cmu.edu/~baraff/sigcourse/notesf.pdf

### Constraint decomposition/research

- witness-configuration decomposition survey/paper: https://arxiv.org/abs/1811.11472

Structural matching and current-configuration numerical Jacobian rank remain
separate reports: a structural count is not proof of numerical independence.
