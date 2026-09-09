// SPDX-License-Identifier: GPL-3.0-or-later
import type { ProfileExport } from "@geosolve/engine";
import type { FootprintInputs } from "./generator";

export interface GenerateRequest { revision: number; inputs: FootprintInputs; }
export type GenerateResponse = {
  revision: number; status: "accepted"; profiles: ProfileExport; inputDigest: string;
  inputs: FootprintInputs; elapsedMs: number; curveCount: number;
} | { revision: number; status: "rejected"; detail: string };

export const EVALUATION_TIMEOUT_MS = 20_000;
