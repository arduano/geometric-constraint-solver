// SPDX-License-Identifier: GPL-3.0-or-later
import { readBrowserStorage, writeBrowserStorage } from "./browser-storage";
import { samples, type SampleEntry } from "./sample-catalog";

const RECENT_SAMPLES_KEY = "geosolve-workbench-recents-v1";
const MAX_RECENT_SAMPLES = 6;
const MAX_RECENT_SAMPLE_STORAGE_BYTES = 8 * 1024;

interface RecentSampleEnvelope {
  version: 1;
  entries: Array<{ kind: SampleEntry["kind"]; key: string; title: string }>;
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
    if (envelope?.version !== 1 || !Array.isArray(envelope.entries)) return { entries: [], issue: null };
    return { entries: canonicalEntries(envelope.entries), issue: null };
  } catch {
    return { entries: [], issue: null };
  }
}

export function rememberRecentSample(current: SampleEntry[], sample: SampleEntry): RecentSamplesResult {
  const entries = canonicalEntries([
    { kind: sample.kind, key: sample.key, title: sample.title },
    ...current.map(({ kind, key, title }) => ({ kind, key, title })),
  ]);
  const encoded = JSON.stringify({
    version: 1,
    entries: entries.map(({ kind, key, title }) => ({ kind, key, title })),
  } satisfies RecentSampleEnvelope);
  const stored = writeBrowserStorage(RECENT_SAMPLES_KEY, encoded, "recent sample shortcuts");
  return { entries, issue: stored.issue };
}

function canonicalEntries(entries: unknown[]): SampleEntry[] {
  const canonical: SampleEntry[] = [];
  for (const entry of entries) {
    if (!entry || typeof entry !== "object") continue;
    const shortcut = entry as { kind?: unknown; key?: unknown };
    if (typeof shortcut.key !== "string" || (shortcut.kind !== "native" && shortcut.kind !== "code")) continue;
    const sample = samples.find((candidate) => candidate.kind === shortcut.kind && candidate.key === shortcut.key);
    if (!sample || canonical.some((candidate) => candidate.kind === sample.kind && candidate.key === sample.key)) continue;
    canonical.push(sample);
    if (canonical.length === MAX_RECENT_SAMPLES) break;
  }
  return canonical;
}
