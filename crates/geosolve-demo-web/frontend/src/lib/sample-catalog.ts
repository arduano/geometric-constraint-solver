// SPDX-License-Identifier: GPL-3.0-or-later
import samplesJson from "../data/samples.json";

export interface SampleEntry {
  ordinal: number;
  stableId: string;
  key: string;
  title: string;
  category: "mechanism" | "product_fabrication" | "reference_lab" | "scale_study";
  group: string;
  summary: string;
}

export const samples = samplesJson as SampleEntry[];
