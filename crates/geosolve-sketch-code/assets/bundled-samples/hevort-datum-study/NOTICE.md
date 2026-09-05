<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# HevORT HD9/MGN9 reference notice

The adjacent managed sketch was informed by the HevORT project at
<https://github.com/MirageC79/HevORT>, immutable commit
`7e26a9320f825ada57a145dfcf1a3d6c77488576`. The consulted sources are:

- `files/STL/HD9/Option_HD9_CFx/XYHD9_CFx_YCarriage_LH_LowerBody v1.step`;
- `bom/Option_HD9_CFx_MGN9.xlsx`.

That upstream snapshot is licensed under the GNU General Public License version 3 only
(`GPL-3.0-only`). GeoSolve independently redraws a narrow 2D envelope and selected mounting datums;
it does not redistribute the solid, reproduce an exact part, or provide a manufacturing guarantee.

## Audited interpretation

The STEP vertex bounds X=-35..29.1 and Z=143.8..176.2 yield 64.1 x 32.4 mm. Its Y-normal R1.6 mounting axes at X=5/25 and Z=150/170 form a 20 x 20 mm square; redraw coordinates are (X-15,Z-160). The right rail study is a separate interface comparison, not the same mounting pattern.

The BOM identifies the MGN9 rail and MGN9H block option. Illustrative rail length and nominal component envelopes are construction datums; manufacture is outside this study.

The separate MGN9H mounting comparison additionally consults Printers for Ants Micron
<https://github.com/PrintersForAnts/Micron>, commit
`61a229ba01c3febbfb9b994e07302d9ddac68bfb`,
`CAD/SubAssemblies/CNC_Carriage.step` (`GPL-3.0-only`). Its selected 16 x 15 mm
mounting pattern is rotated ninety degrees in the adjacent rail datum study.
