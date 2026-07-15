# ADR 0002: Local 2D workplanes and SE(2) mechanism bodies

Status: accepted

## Decision

Planar sketches and mechanisms use local 2D coordinates embedded in world space through a `PlaneFrame`. Mechanism links use redundant rigid-body `SE(2)` poses with body-local joint features.

## Reason

This keeps planar numerical systems small and supports arbitrary 3D placement. Rigid-body coordinates preserve link rigidity and joint semantics without distance webs. ADR 0006 defines the common `SE(2)`/`SE(3)` convention used by M11 and the spatial linkage milestones.

## Consequence

The M1-M7 baseline does not add world-space `z = 0` residuals for every point. M11 adds manifold spatial state, and M16/M18 add spatial features and joints without changing the local-workplane decision. Physics remains outside M8-M22.
