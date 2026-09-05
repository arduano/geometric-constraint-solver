<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Original Prusa MINI X-carriage reference notice

The adjacent managed sketch was informed by Prusa Research's Original Prusa MINI project at
<https://github.com/prusa3d/Original-Prusa-MINI>, immutable commit
`853bc30c4b10190f1d669ed6d0a567e333c28f21`. The consulted sources are:

- `STEP/PRINTED PARTS/MINI-x-carriage.stp`;
- `STL/MINI-x-carriage.stl`.

That upstream snapshot is licensed under the GNU General Public License version 3 only
(`GPL-3.0-only`). GeoSolve independently redraws a narrow front-envelope and datum study. It does
not import the solid or mesh, reproduce a printable part, or provide a dimensional guarantee or
manufacturing guarantee; selected mount coordinates are independently extracted below.

## Audited interpretation

Selected STEP cylindrical interfaces normal to Z are redrawn in coordinates (Y,X-17): two R7.5 bearing seats at X=0/34,Y=0; R1.6 at X=1.75,Y=12; R1.65 at X=46.5,Y=4. Orthogonal bores and 3D details are omitted. Parameter edits explore an adapter variant rather than modifying OEM specifications.

STL axis-aligned bounds are 31.423 x 69.407 x 29 mm. Only the independently centred first-two-axis bounding box is shown as construction; it is not a registration of the differently oriented STL and STEP or a printable outline.
