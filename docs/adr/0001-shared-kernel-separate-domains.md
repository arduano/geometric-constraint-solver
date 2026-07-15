# ADR 0001: Shared numerical kernel, separate domain models

Status: accepted

## Decision

CAD sketches and mechanisms compile into the same variable/residual/Jacobian kernel, but retain separate high-level object models and constraint libraries.

## Reason

Sketch entities are independently editable design geometry. Mechanism geometry belongs to rigid bodies and carries joint, driver and assembly-mode semantics. Treating a rigid link as a web of sketch distances creates unnecessary variables and redundant constraints and obscures position/velocity kinematic semantics.

## Consequence

`geosolve-core` cannot depend on either domain crate. Deliverable 1 remains in `geosolve-sketch`, Deliverable 2 remains in `geosolve-linkage`, and the web smoke consumer exercises both public frontends rather than constructing raw residuals. No shared kernel API introduces physics semantics.
