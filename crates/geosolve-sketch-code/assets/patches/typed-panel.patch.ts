// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Preserve every caller-owned corner key in the mapped Fillet result. */
export const fillets = definePatch(
  { corners: t.record(t.corner()), radius: t.length() },
  (p, { corners, radius }) => ({
    fillets: p.mapRecord(corners, (corner) => p.fillet({ corner, radius })),
  }),
);
