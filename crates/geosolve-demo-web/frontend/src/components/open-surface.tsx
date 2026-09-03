// SPDX-License-Identifier: GPL-3.0-or-later
import { useMemo, useRef, useState } from "react";
import { FileCode2, FilePlus2, FolderOpen, Search } from "lucide-react";
import { cn } from "../lib/cn";
import { samples, type SampleEntry } from "../lib/sample-catalog";

export type { SampleEntry } from "../lib/sample-catalog";

interface OpenSurfaceProps {
  recents: SampleEntry[];
  onOpen: (sample: SampleEntry) => void;
  onNewSketch: () => void;
  onNewCode: () => void;
  onImport: () => void;
  onDismiss: () => void;
}

export function OpenSurface({ recents, onOpen, onNewSketch, onNewCode, onImport, onDismiss }: OpenSurfaceProps) {
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const filtered = useMemo(() => {
    const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
    return samples.filter((sample) => terms.every((term) => `${sample.key} ${sample.title} ${sample.group} ${sample.kind}`.toLowerCase().includes(term)));
  }, [query]);
  const grouped = filtered.reduce<Record<string, SampleEntry[]>>((groups, sample) => {
    (groups[sample.group] ??= []).push(sample);
    return groups;
  }, {});

  return (
    <div className="flex h-full min-h-0 flex-col" onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); onDismiss(); } }}>
      <header className="border-b border-border p-5">
        <h2 className="text-xl font-semibold text-foreground">Open</h2>
        <p className="mt-1 text-sm text-muted">Start cleanly or inspect one of the complete bundled examples.</p>
        <div className="relative mt-4">
          <Search className="pointer-events-none absolute left-3 top-2.5 size-4 text-muted" />
          <input ref={searchRef} autoFocus value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search 37 samples…" className="h-9 w-full rounded-md border border-border bg-canvas pl-9 pr-3 text-sm text-foreground outline-none placeholder:text-muted focus:border-accent focus:ring-1 focus:ring-accent" />
        </div>
        <div className="mt-3 grid grid-cols-3 gap-2">
          <QuickStart icon={<FilePlus2 />} label="New sketch" onClick={onNewSketch} />
          <QuickStart icon={<FileCode2 />} label="Start from code" onClick={onNewCode} />
          <QuickStart icon={<FolderOpen />} label="Import project" onClick={onImport} />
        </div>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        {!query.trim() && <section aria-labelledby="recent-workspaces" className="mb-5">
          <h3 id="recent-workspaces" className="mb-2 text-xs font-semibold uppercase tracking-[.14em] text-muted">Recent</h3>
          {recents.length > 0
            ? <div className="grid grid-cols-2 gap-1">{recents.map((sample) => <SampleButton key={`recent-${sample.stableId}`} sample={sample} onOpen={onOpen} />)}</div>
            : <p className="rounded-md border border-dashed border-border px-3 py-3 text-xs text-muted">No recent workspace yet</p>}
        </section>}
        <p className="mb-3 text-xs font-semibold uppercase tracking-[.14em] text-muted">{filtered.length} samples</p>
        {Object.entries(grouped).map(([group, entries]) => (
          <section key={group} aria-labelledby={`sample-group-${group}`} className="mb-5">
            <h3 id={`sample-group-${group}`} className="sticky top-0 z-10 border-b border-border bg-raised/95 py-2 text-xs font-semibold uppercase tracking-wide text-muted backdrop-blur">{group}</h3>
            <div className="grid grid-cols-2 gap-1 pt-2">
              {entries?.map((sample) => <SampleButton key={sample.stableId} sample={sample} onOpen={onOpen} />)}
            </div>
          </section>
        ))}
        {filtered.length === 0 && <p role="status" className="py-12 text-center text-sm text-muted">No samples match “{query}”.</p>}
      </div>
    </div>
  );
}

function SampleButton({ sample, onOpen }: { sample: SampleEntry; onOpen: (sample: SampleEntry) => void }) {
  return <button onClick={() => onOpen(sample)} className="group rounded-md border border-transparent px-3 py-2 text-left outline-none hover:border-border hover:bg-surface focus-visible:border-accent focus-visible:ring-1 focus-visible:ring-accent"><span className="block text-sm text-foreground">{sample.title}</span><span className={cn("mt-1 inline-block rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase", sample.kind === "code" ? "bg-amber-400/15 text-accent" : "bg-sky-400/10 text-sky-300")}>{sample.kind}</span></button>;
}

function QuickStart({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return <button onClick={onClick} className="flex h-14 items-center gap-2 rounded-md border border-border bg-surface px-3 text-left text-sm text-foreground outline-none hover:border-neutral-500 hover:bg-neutral-700 focus-visible:ring-2 focus-visible:ring-accent">{<span className="[&>svg]:size-4 text-accent">{icon}</span>} {label}</button>;
}
