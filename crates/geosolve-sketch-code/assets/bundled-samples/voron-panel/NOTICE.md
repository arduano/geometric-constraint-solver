<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Voron V0.2 motor-panel reference notice

The adjacent managed sketch was informed by VoronDesign's Voron-0 project at
<https://github.com/VoronDesign/Voron-0>, immutable commit
`a53fc87562fd630c846af38d7de850c894dc3d85`. The consulted sources are:

- `Drawings/Motor_Panel_v0.2.PDF`;
- `DXF/Motor_Panel.dxf`.

That upstream snapshot is licensed under the GNU General Public License version 3 only
(`GPL-3.0-only`). GeoSolve independently redraws a simplified dimensional study. Its single central and
edge cutout boundary has explicitly squared transitions; it omits bend and production detail and is not the
upstream DXF, a cut-ready file, or a manufacturing guarantee.

## Audited interpretation

The 122 x 37 mm bounds inform the independently redrawn panel. The design study retains bilateral symmetry and an edge-open T passage; it omits bends, production details, curved transitions and manufacturing instructions.

The single closed DXF polyline establishes an edge-open T passage and paired side notches. Redraw coordinates translate DXF Y by +211.49999618536208; the PDF presents the part rotated 180 degrees, so its passage opens downward while this DXF view opens upward. Source dimensions include a 10 mm neck, 30 x 10 crossbar extent and 3.5 mm notch mouth. Curved lobes and transitions are deliberately squared off, so this is not a cut-ready profile.
