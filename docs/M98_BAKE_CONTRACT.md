<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Baked 2D profile interchange, v1

`geosolve-baked-profile-v1` transports accepted, sampled geometry from GeoSolve to
an external modeling application. It contains geometry and provenance, not a
modeling program. The original consumer integration was introduced in M98.

```json
{
  "format": "geosolve-baked-profile-v1",
  "units": "mm",
  "source": { "sha256": "<64 lowercase hexadecimal characters>" },
  "plane": {
    "origin": [0, 0, 0],
    "x_axis": [1, 0, 0],
    "y_axis": [0, 1, 0]
  },
  "sampling": { "max_chord_error_mm": 0.02 },
  "regions": [
    {
      "id": "region-0",
      "outer": [[0, 0], [85, 0], [85, 56], [0, 56]],
      "holes": []
    }
  ]
}
```

The required field meanings are fixed; optional provenance may accompany them.
The source hash identifies the exact UTF-8 entry-source bytes. Current exporters
also record the complete dependency/input/toolchain provenance. Plane coordinates
and local XY coordinates use millimeters; the plane frame is orthonormal and
right-handed. Loops are implicitly closed, with no repeated first point. Outer
loops are counterclockwise; holes are clockwise.

Region IDs are local to one export and provide no persistent topological naming.
A consumer must select a region explicitly when several are present. Production
arrangements include interior faces as well as enclosing faces with holes; extruding
every exported face can fill intended holes.

`sampling.max_chord_error_mm` bounds source-curve polygonization only. It does not
cover later Boolean, STL or manufacturing error. Unsupported geometry, incomplete
or ambiguous topology, nonfinite coordinates and unrepresentable precision targets
reject. An invalid current source cannot fall back to cached last-good geometry.

From a prepared checkout:

```bash
node packages/geosolve-cli/bin/geosolve.mjs bake <folder> \
  --out <outside-file.json> --chord-error-mm 0.02
```

No browser or running server is required. The output parent must exist and remain
outside the source folder. Publication occurs only after accepted topology and
sampling validate; failure returns a diagnostic and leaves an existing output
intact. The CLI contains no geometry equations.

The [profile examples](../examples/file-workspace-bake/README.md) exercise the
nominal 85 × 56 mm board footprint, four diameter-2.7 mm holes and solved circle/arc
geometry. [The manifold](../examples/file-workspace-manifold/README.md) exercises
computed boundaries. Nominal hardware dimensions and downstream solid examples
are demonstrations, not physical fit or manufacturing validation.

The consumer proof checks actual export → import → extrusion → STL → independent
bounds/topology/volume validation, then changes a source dimension and repeats.
[M99 qualification](M99_QUALIFICATION.md#exact-artifacts-and-consumer-use) records
the final installed-package integration.
