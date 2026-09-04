// SPDX-License-Identifier: GPL-3.0-or-later
import { readBrowserStorage, writeBrowserStorage } from "./browser-storage";
import { samples, type SampleEntry } from "./sample-catalog";

const RECENT_SAMPLES_KEY = "geosolve-workbench-recents-v2";
const MAX_RECENT_SAMPLES = 6;
const MAX_RECENT_SAMPLE_STORAGE_BYTES = 8 * 1024;

interface RecentSampleEnvelope {
  version: 2;
  entries: Array<{ key: string }>;
}

export interface RecentSamplesResult {
  entries: SampleEntry[];
  issue: string | null;
}

export function readRecentSamples(): RecentSamplesResult {
  const stored = readBrowserStorage(RECENT_SAMPLES_KEY, "recent sample shortcuts");
  if (stored.issue || !stored.value || stored.value.length > MAX_RECENT_SAMPLE_STORAGE_BYTES) {
    return { entries: [], issue: stored.issue };
  }
  try {
    const envelope = JSON.parse(stored.value) as Partial<RecentSampleEnvelope> | null;
    if (envelope?.version !== 2 || !Array.isArray(envelope.entries)) return { entries: [], issue: null };
    return { entries: canonicalEntries(envelope.entries), issue: null };
  } catch {
    return { entries: [], issue: null };
  }
}

export function rememberRecentSample(current: SampleEntry[], sample: SampleEntry): RecentSamplesResult {
  const entries = canonicalEntries([
    { key: sample.key },
    ...current.map(({ key }) => ({ key })),
  ]);
  const encoded = JSON.stringify({
    version: 2,
    entries: entries.map(({ key }) => ({ key })),
  } satisfies RecentSampleEnvelope);
  const stored = writeBrowserStorage(RECENT_SAMPLES_KEY, encoded, "recent sample shortcuts");
  return { entries, issue: stored.issue };
}

function canonicalEntries(entries: unknown[]): SampleEntry[] {
  const canonical: SampleEntry[] = [];
  for (const entry of entries) {
    if (!entry || typeof entry !== "object") continue;
    const shortcut = entry as { key?: unknown };
    if (typeof shortcut.key !== "string") continue;
    const sample = samples.find((candidate) => candidate.key === shortcut.key);
    if (!sample || canonical.some((candidate) => candidate.key === sample.key)) continue;
    canonical.push(sample);
    if (canonical.length === MAX_RECENT_SAMPLES) break;
  }
  return canonical;
}
