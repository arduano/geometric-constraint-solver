# ADR 0002: Local 2D workplanes and SE(2) mechanism bodies

Status: accepted

## Decision

Planar sketches and mechanisms use local 2D coordinates embedded in world space through a `PlaneFrame`. Mechanism links use redundant rigid-body `SE(2)` poses with body-local joint features.

## Reason

This keeps the initial numerical system small and naturally supports arbitrary 3D placement. Rigid-body coordinates preserve link rigidity and joint semantics without distance webs. It also provides a clean path to `SE(3)` variable blocks later.

## Consequence

The MVP does not add world-space `z = 0` residuals for every point. Cross-plane constraints and spatial joints are deferred until the planar solver is robust.
