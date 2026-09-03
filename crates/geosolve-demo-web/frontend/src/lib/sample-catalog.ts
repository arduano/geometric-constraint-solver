// SPDX-License-Identifier: GPL-3.0-or-later
import samplesJson from "../data/samples.json";

export interface SampleEntry {
  stableId: string;
  key: string;
  title: string;
  group: string;
  kind: "native" | "code";
}

export const samples = samplesJson as SampleEntry[];
