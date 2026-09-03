// SPDX-License-Identifier: GPL-3.0-or-later
import * as Dialog from "@radix-ui/react-dialog";
import { Panel, PanelGroup, PanelResizeHandle, type PanelGroupStorage } from "react-resizable-panels";
import { Activity, AlertTriangle, ChevronDown, Code2, Download, FileJson, FolderOpen, Menu, PackageOpen, PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, Play, Redo2, RotateCcw, Save, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { WorkbenchAdapter, WorkbenchSnapshot, WorkspaceMode } from "./lib/adapter";
import { assertWorkbenchSnapshot } from "./lib/adapter";
import { readBrowserStorage, removeBrowserStorage, writeBrowserStorage } from "./lib/browser-storage";
import { createProjectStore, type ProjectStore } from "./lib/project-storage";
import { readRecentSamples, rememberRecentSample } from "./lib/recent-samples";
import { resolvePendingManagedMutationSnapshot } from "./lib/pending-managed-mutation";
import { utf8ByteSpanToUtf16Range, type Utf16SourceRange } from "./lib/source-navigation";
import { MockWorkbenchAdapter } from "./lib/mock-adapter";
import { useTransientSurface } from "./hooks/use-transient-surface";
import { Button } from "./components/ui/button";
import { ToolRail } from "./components/tool-rail";
import { ToolIcon } from "./components/tool-icon";
import { TransientPopover } from "./components/transient-popover";
import { CanvasViewport } from "./components/canvas-viewport";
import { CanvasControls } from "./components/canvas-controls";
import { DeclarationPanel, DetailsPanel, Explorer, ParametersView, ProblemsView, type DeclarationPanelActions } from "./components/side-panels";
import { CodeEditor, type EditorNavigation } from "./components/code-editor";
import { OpenSurface, type SampleEntry } from "./components/open-surface";
import type { ToolCatalog } from "./lib/tool-catalog";
import { toolLabel, toolSection } from "./lib/tool-catalog";

const FALLBACK = new MockWorkbenchAdapter();
const DEFAULT_PROJECT_STORE = createProjectStore();
const PRESENTATION_KEY = "geosolve.presentation.v1";
const DRAFT_KEY = "geosolve.source-draft.v1";
const SPLIT_CODE_DEFAULT_PX = 560;
const SPLIT_CODE_MINIMUM_PX = 520;
const DURABLE_REPLACEMENT_COMMANDS = new Set(["project.new", "project.new-code", "project.import", "sample.open"]);
type CodeSurface = "source" | "parameters" | "problems" | "generated" | "artifacts";
type ProjectSaveIntent = "auto" | "manual" | "replacement";

function readPresentation() {
  const fallback = { mode: "design" as WorkspaceMode, explorer: true, details: true, splitCodeWidth: SPLIT_CODE_DEFAULT_PX, codeSurface: "source" as CodeSurface };
  const stored = readBrowserStorage(PRESENTATION_KEY, "workspace presentation");
  if (stored.issue) return { ...fallback, storageIssue: stored.issue };
  try {
    const value = JSON.parse(stored.value ?? "null") as { mode?: unknown; explorer?: unknown; details?: unknown; splitCodeWidth?: unknown; codeSurface?: unknown } | null;
    const codeSurface = ["source", "parameters", "problems", "generated", "artifacts"].includes(String(value?.codeSurface)) ? value?.codeSurface as CodeSurface : "source";
    const splitCodeWidth = typeof value?.splitCodeWidth === "number" && Number.isFinite(value.splitCodeWidth) ? Math.max(SPLIT_CODE_MINIMUM_PX, Math.min(960, value.splitCodeWidth)) : SPLIT_CODE_DEFAULT_PX;
    return { mode: value && ["design", "split", "code"].includes(String(value.mode)) ? value.mode as WorkspaceMode : "design", explorer: typeof value?.explorer === "boolean" ? value.explorer : true, details: typeof value?.details === "boolean" ? value.details : true, splitCodeWidth, codeSurface, storageIssue: null };
  } catch { return { ...fallback, storageIssue: null }; }
}

export interface AppProps { adapter?: WorkbenchAdapter; projectStore?: ProjectStore; }

export default function App({ adapter = FALLBACK, projectStore = DEFAULT_PROJECT_STORE }: AppProps) {
  const [initialPresentation] = useState(readPresentation);
  const [initialRecents] = useState(readRecentSamples);
  const [snapshot, setSnapshot] = useState<WorkbenchSnapshot | null>(null);
  const [toolCatalog, setToolCatalog] = useState<ToolCatalog | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [mode, setMode] = useState<WorkspaceMode>(initialPresentation.mode);
  const activeTool = snapshot?.presentation.activeTool ?? "select";
  const [capturedGesture, setCapturedGesture] = useState(false);
  const [reproOpen, setReproOpen] = useState(false);
  const [explorerOpen, setExplorerOpen] = useState(initialPresentation.explorer);
  const [detailsOpen, setDetailsOpen] = useState(initialPresentation.details);
  const [splitCodeWidth, setSplitCodeWidth] = useState(initialPresentation.splitCodeWidth);
  const [codeSurface, setCodeSurface] = useState<CodeSurface>(initialPresentation.codeSurface);
  const [recentSamples, setRecentSamples] = useState(initialRecents.entries);
  const [draft, setDraft] = useState("");
  const [actionError, setActionError] = useState<string | null>([initialPresentation.storageIssue, initialRecents.issue].filter(Boolean).join("; ") || null);
  const [editorNavigation, setEditorNavigation] = useState<EditorNavigation | null>(null);
  const snapshotRef = useRef<WorkbenchSnapshot | null>(null);
  const draftRef = useRef("");
  const navigationRequest = useRef(0);
  const startupRequest = useRef(0);
  const projectSaveTail = useRef<Promise<void>>(Promise.resolve());
  const projectSaveEpoch = useRef(0);
  const projectAutosaveSafe = useRef(false);
  const lastSavedProject = useRef<string | null | undefined>(undefined);
  const suppressInstalledSnapshotAutosave = useRef<WorkbenchSnapshot | null>(null);
  const fileButtonRef = useRef<HTMLButtonElement | null>(null);
  const importInputRef = useRef<HTMLInputElement | null>(null);
  const transient = useTransientSurface();

  useEffect(() => { draftRef.current = draft; }, [draft]);
  const reportError = useCallback((error: unknown) => setActionError(errorText(error)), []);
  const panelStorage = useMemo<PanelGroupStorage>(() => ({
    getItem(name) {
      const stored = readBrowserStorage(name, "workspace pane layout");
      if (stored.issue) queueMicrotask(() => reportError(stored.issue));
      return stored.value;
    },
    setItem(name, value) {
      const stored = writeBrowserStorage(name, value, "workspace pane layout");
      if (stored.issue) queueMicrotask(() => reportError(stored.issue));
    },
  }), [reportError]);

  const resolveAdapterSnapshot = useCallback(
    (next: WorkbenchSnapshot) => resolvePendingManagedMutationSnapshot(adapter, next),
    [adapter],
  );

  const queueProjectSave = useCallback((intent: ProjectSaveIntent, clearError: boolean) => {
    const epoch = projectSaveEpoch.current;
    const save = projectSaveTail.current.then(async () => {
      if (projectSaveEpoch.current !== epoch || (intent !== "manual" && !projectAutosaveSafe.current)) return;
      try {
        const payload = await adapter.persistProject();
        if (projectSaveEpoch.current !== epoch) return;
        if (intent !== "manual" && payload.contents === lastSavedProject.current && projectAutosaveSafe.current) {
          if (clearError) setActionError(null);
          return;
        }
        const stored = await projectStore.write(payload.contents);
        if (!stored.value) {
          reportError(stored.issue ?? "Browser storage could not write the saved project");
          return;
        }
        if (projectSaveEpoch.current === epoch) {
          lastSavedProject.current = payload.contents;
          projectAutosaveSafe.current = true;
        }
        if (stored.issue) reportError(stored.issue);
        else if (clearError) setActionError(null);
      } catch (error) { reportError(error); }
    });
    projectSaveTail.current = save.catch(() => undefined);
    return save;
  }, [adapter, projectStore, reportError]);

  const start = useCallback(() => {
    const request = startupRequest.current + 1;
    startupRequest.current = request;
    projectSaveEpoch.current += 1;
    projectAutosaveSafe.current = false;
    lastSavedProject.current = undefined;
    setStartupError(null);
    const install = async (next: WorkbenchSnapshot) => {
      const checked = await resolveAdapterSnapshot(next);
      const catalog = await adapter.toolCatalog();
      if (startupRequest.current !== request) return false;
      snapshotRef.current = checked;
      suppressInstalledSnapshotAutosave.current = checked;
      setSnapshot(checked);
      setToolCatalog(catalog);
      const file = checked.source.files.find((candidate) => candidate.path === checked.source.selectedPath);
      const restored = restoredBrowserDraft(checked, file);
      if (restored.issue) reportError(restored.issue);
      setDraft(restored.contents ?? file?.contents ?? "");
      return true;
    };
    void projectStore.read().then(async (storedProject) => {
      if (startupRequest.current !== request) return;
      lastSavedProject.current = storedProject.value;
      projectAutosaveSafe.current = storedProject.autosaveSafe;
      if (storedProject.issue) {
        reportError(storedProject.autosaveSafe
          ? storedProject.issue
          : `${storedProject.issue}; Automatic project saving is paused until you use Save in browser.`);
      }
      const persistedProject = storedProject.value ?? undefined;
      try {
        await install(await adapter.construct({ version: 1, persistedProject }));
      }
      catch (error) {
        if (startupRequest.current !== request) return;
        if (!persistedProject) {
          setStartupError([errorText(error), storedProject.issue].filter(Boolean).join("; "));
          return;
        }
        projectAutosaveSafe.current = false;
        try {
          const installed = await install(await adapter.construct({ version: 1 }));
          if (!installed) return;
          if (!storedProject.autosaveSafe) {
            setActionError(`The fallback saved workspace could not be restored: ${errorText(error)}; IndexedDB authority was unread, so the existing browser data was retained and automatic project saving remains paused until you use Save in browser.`);
            return;
          }
          if (startupRequest.current !== request) return;
          const removed = await projectStore.remove();
          if (startupRequest.current !== request) return;
          lastSavedProject.current = removed.value ? null : undefined;
          projectAutosaveSafe.current = true;
          projectSaveEpoch.current += 1;
          void queueProjectSave("replacement", false);
          if (startupRequest.current === request) {
            setActionError([`Saved workspace could not be restored and was reset: ${errorText(error)}`, removed.issue].filter(Boolean).join("; "));
          }
        }
        catch (fallbackError) {
          if (startupRequest.current === request) setStartupError(errorText(fallbackError));
        }
      }
    }).catch((error: unknown) => {
      if (startupRequest.current === request) setStartupError(errorText(error));
    });
  }, [adapter, projectStore, queueProjectSave, reportError, resolveAdapterSnapshot]);
  useEffect(() => {
    start();
    return () => { startupRequest.current += 1; };
  }, [start]);
  useEffect(() => {
    const stored = writeBrowserStorage(PRESENTATION_KEY, JSON.stringify({ mode, explorer: explorerOpen, details: detailsOpen, splitCodeWidth, codeSurface }), "workspace presentation");
    if (stored.issue) reportError(stored.issue);
  }, [codeSurface, detailsOpen, explorerOpen, mode, reportError, splitCodeWidth]);

  const acceptSnapshot = useCallback(async (next: WorkbenchSnapshot) => {
    const checked = await resolveAdapterSnapshot(next);
    const before = snapshotRef.current;
    const beforeFile = before?.source.files.find((file) => file.path === before.source.selectedPath);
    const localDirty = Boolean(beforeFile && draftRef.current !== beforeFile.contents);
    const nextFile = checked.source.files.find((file) => file.path === checked.source.selectedPath) ?? checked.source.files[0];
    if (!localDirty || before?.source.selectedPath !== checked.source.selectedPath) setDraft(nextFile?.contents ?? "");
    snapshotRef.current = checked;
    setSnapshot(checked);
    return checked;
  }, [resolveAdapterSnapshot]);
  const acceptCanvasSnapshot = useCallback((next: WorkbenchSnapshot) => {
    void acceptSnapshot(next).catch(reportError);
  }, [acceptSnapshot, reportError]);
  const command = useCallback(async (name: string, payload?: unknown) => {
    try {
      const next = await adapter.dispatch({ version: 1, command: name, payload });
      if (projectAutosaveSafe.current) setActionError(null);
      const accepted = await acceptSnapshot(next);
      if (DURABLE_REPLACEMENT_COMMANDS.has(name)) {
        suppressInstalledSnapshotAutosave.current = accepted;
        projectSaveEpoch.current += 1;
        void queueProjectSave("replacement", false);
      }
      return accepted;
    } catch (error) {
      reportError(error);
      const current = snapshotRef.current;
      if (!current) throw error;
      return current;
    }
  }, [acceptSnapshot, adapter, queueProjectSave, reportError]);
  const verifiedCommand = useCallback(async (name: string, payload?: unknown) => {
    try {
      const next = assertWorkbenchSnapshot(await adapter.dispatch({ version: 1, command: name, payload }));
      if (projectAutosaveSafe.current) setActionError(null);
      return await acceptSnapshot(next);
    } catch (error) {
      reportError(error);
      return null;
    }
  }, [acceptSnapshot, adapter, reportError]);
  const saveBrowserProject = useCallback(async () => {
    await queueProjectSave("manual", true);
  }, [queueProjectSave]);
  const chooseTool = useCallback((id: string, origin?: HTMLElement) => { transient.close(false); void command("tool.select", { id }); if (origin?.getAttribute("role") === "menuitem") queueMicrotask(() => document.querySelector<HTMLElement>('[role="application"]')?.focus()); }, [command, transient]);

  useEffect(() => {
    if (!snapshot) return;
    if (suppressInstalledSnapshotAutosave.current === snapshot) {
      suppressInstalledSnapshotAutosave.current = null;
      return;
    }
    void queueProjectSave("auto", false);
  }, [queueProjectSave, snapshot?.project.sampleKey, snapshot?.project.title, snapshot?.revision]);

  useEffect(() => {
    if (!snapshot) return;
    const file = snapshot.source.files.find((candidate) => candidate.path === snapshot.source.selectedPath);
    if (!file || file.readOnly || draft === file.contents) {
      const removed = removeBrowserStorage(DRAFT_KEY, "the unapplied source draft");
      if (removed.issue) reportError(removed.issue);
      return;
    }
    const stored = writeBrowserStorage(DRAFT_KEY, JSON.stringify({ version: 1, title: snapshot.project.title, sampleKey: snapshot.project.sampleKey ?? null, path: file.path, base: file.contents, contents: draft }), "the unapplied source draft");
    if (stored.issue) reportError(stored.issue);
  }, [draft, reportError, snapshot]);

  useEffect(() => {
    const onWorkspaceKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const modifier = (event.ctrlKey || event.metaKey) && !event.altKey;
      if (modifier && event.key.toLowerCase() === "o") {
        event.preventDefault();
        if (fileButtonRef.current) transient.toggle("open", fileButtonRef.current);
        return;
      }
      if (modifier && event.key.toLowerCase() === "s") {
        event.preventDefault();
        transient.close(false);
        void saveBrowserProject();
        return;
      }
      if (event.key === "Enter" && snapshotRef.current?.presentation.canFinish && event.target instanceof Element && event.target.closest('[role="application"]')) {
        event.preventDefault();
        void command("tool.finish");
        return;
      }
      if (event.key !== "Escape") return;
      if (reproOpen) { event.preventDefault(); setReproOpen(false); return; }
      if (transient.active) { event.preventDefault(); transient.close(true); return; }
      if (capturedGesture) { event.preventDefault(); setCapturedGesture(false); void adapter.cancel({ version: 1, reason: "escape" }).then((next) => next && acceptSnapshot(next)).catch(reportError); return; }
      if (activeTool !== "select") { event.preventDefault(); chooseTool("select"); queueMicrotask(() => document.querySelector<HTMLElement>('[aria-label="Select"]')?.focus()); }
    };
    document.addEventListener("keydown", onWorkspaceKey);
    return () => document.removeEventListener("keydown", onWorkspaceKey);
  }, [acceptSnapshot, activeTool, adapter, capturedGesture, chooseTool, command, reportError, reproOpen, saveBrowserProject, transient]);

  if (!snapshot || !toolCatalog) return <main className="grid h-dvh place-items-center bg-canvas text-sm text-muted">{startupError ? <section role="alert" className="max-w-md rounded-lg border border-danger/50 bg-raised p-5 text-center"><p className="font-medium text-foreground">Workbench unavailable</p><p className="mt-2 text-xs leading-relaxed">{startupError}</p><Button className="mt-4" onClick={start}>Try again</Button></section> : <span role="status">Starting GeoSolve…</span>}</main>;
  const selectedFile = snapshot.source.files.find((file) => file.path === snapshot.source.selectedPath) ?? snapshot.source.files[0];
  const localDraftDirty = Boolean(selectedFile && draft !== selectedFile.contents);
  const historyBlocked = localDraftDirty || snapshot.source.dirty;
  const replacementBlocked = localDraftDirty || snapshot.source.dirty;
  const reportReplacementBlocked = () => reportError("Apply or Revert the current source draft before replacing this project.");
  const openSample = (sample: SampleEntry) => { if (replacementBlocked) { reportReplacementBlocked(); return; } transient.close(false); void command("sample.open", { key: sample.key, title: sample.title }).then((next) => {
    setDraft(next.source.files.find((file) => file.path === next.source.selectedPath)?.contents ?? "");
    if (next.revision !== snapshot.revision && next.project.sampleKey === sample.key) {
      const remembered = rememberRecentSample(recentSamples, sample);
      setRecentSamples(remembered.entries);
      if (remembered.issue) reportError(remembered.issue);
    }
  }); setMode(sample.kind === "code" ? "split" : "design"); };
  const replaceProject = (name: "project.new" | "project.new-code", nextMode: WorkspaceMode) => { if (replacementBlocked) { reportReplacementBlocked(); return; } transient.close(false); void command(name).then((next) => setDraft(next.source.files.find((file) => file.path === next.source.selectedPath)?.contents ?? "")); setMode(nextMode); };
  const importProject = () => { if (replacementBlocked) { reportReplacementBlocked(); return; } transient.close(false); importInputRef.current?.click(); };
  const importSelectedFile = (file?: File) => {
    if (!file) return;
    void file.text().then((contents) => command("project.import", { path: file.name, contents })).then((next) => {
      setDraft(next.source.files.find((candidate) => candidate.path === next.source.selectedPath)?.contents ?? "");
      setMode(next.source.files.some((candidate) => candidate.path === "sketch.ts") ? "split" : "design");
      if (importInputRef.current) importInputRef.current.value = "";
    }).catch(reportError);
  };
  const navigateToEditor = (path: string, locate: (contents: string) => Utf16SourceRange | string) => {
    if (localDraftDirty) { reportError("Apply or Revert the current draft before navigating to an authenticated source span."); return; }
    const initialFile = snapshot.source.files.find((candidate) => candidate.path === path);
    if (!initialFile) { reportError(`Source file \`${path}\` is unavailable.`); return; }
    const initialRange = locate(initialFile.contents);
    if (typeof initialRange === "string") { reportError(initialRange); return; }
    const navigate = (next: WorkbenchSnapshot) => {
      const file = next.source.files.find((candidate) => candidate.path === path);
      if (!file) { reportError(`Source file \`${path}\` is unavailable.`); return; }
      const range = file.contents === initialFile.contents ? initialRange : locate(file.contents);
      if (typeof range === "string") { reportError(range); return; }
      setDraft(file.contents);
      navigationRequest.current += 1;
      setEditorNavigation({ request: navigationRequest.current, ...range });
      setCodeSurface("source");
      setMode("code");
    };
    if (snapshot.source.selectedPath === path) navigate(snapshot);
    else void command("source.select", { path }).then(navigate);
  };
  const navigateToSource = (path: string, from: number, to = from) => {
    navigateToEditor(path, (contents) => {
      const converted = utf8ByteSpanToUtf16Range(contents, from, to);
      return converted.ok
        ? converted.range
        : `Authenticated source span in \`${path}\` is invalid: ${converted.reason}.`;
    });
  };
  const openSelectionInCode = () => {
    const source = snapshot.selection?.source;
    if (source) navigateToSource(source.path, source.from, source.to);
    else reportError("This selection has no exact authenticated managed-source owner.");
  };
  const openProblem = (problem: WorkbenchSnapshot["problems"][number]) => {
    if (!problem.file) return;
    navigateToEditor(problem.file, (contents) => {
      const offset = lineColumnOffset(contents, problem.line ?? 1, problem.column ?? 1);
      return { from: offset, to: offset };
    });
  };
  const declarationActions: DeclarationPanelActions = {
    onSelect: (id) => { void command("declaration.select", { id }); },
    onNavigate: (row) => {
      if (!row.source) { reportError("This declaration has no authenticated managed-source span."); return; }
      void verifiedCommand("declaration.source.open", { id: row.id, from: row.source.from, to: row.source.to }).then((next) => { if (next) navigateToSource(row.source!.path, row.source!.from, row.source!.to); });
    },
    onMove: (id, move) => { void command("declaration.move", { id, ...move }); },
    onSuppress: (id, suppressed) => { void command("declaration.suppression.set", { id, suppressed }); },
    onDelete: (id) => { void command("declaration.delete", { id }); },
  };

  return (
    <div className="flex h-dvh min-h-[720px] min-w-[1024px] flex-col overflow-hidden bg-canvas text-foreground">
      <input ref={importInputRef} type="file" accept=".json,.txt,application/json,text/plain" hidden onChange={(event) => importSelectedFile(event.currentTarget.files?.[0])} />
      <header className="relative z-50 flex h-11 shrink-0 items-center border-b border-border bg-surface px-2">
        <div className="relative flex items-center gap-1">
          <Button ref={fileButtonRef} aria-label="File menu" aria-haspopup="menu" aria-expanded={transient.active === "file"} size="compact" variant="ghost" onClick={(event) => transient.toggle("file", event.currentTarget)}><Menu className="size-4" />File<ChevronDown className="size-3" /></Button>
          {transient.active === "file" && <TransientPopover surfaceRef={transient.contentRef} label="File menu" className="left-0 top-9 w-56"><MenuButton icon={<FolderOpen />} label="Open…" shortcut="Ctrl O" onClick={() => { if (fileButtonRef.current) transient.toggle("open", fileButtonRef.current); }} /><MenuButton icon={<Save />} label="Save in browser" shortcut="Ctrl S" onClick={() => { transient.close(); void saveBrowserProject(); }} /><MenuButton icon={<Download />} label="Export canonical project…" onClick={() => { transient.close(); if (localDraftDirty) { reportError("Apply or Revert the current source draft before exporting a canonical project."); return; } void exportProject(adapter).catch(reportError); }} />{selectedFile.path === "sketch.ts" && <MenuButton icon={<Code2 />} label={localDraftDirty || snapshot.source.dirty ? "Download raw draft…" : "Download sketch.ts…"} onClick={() => { transient.close(); downloadText(localDraftDirty || snapshot.source.dirty ? "sketch-draft.ts" : "sketch.ts", draft, "text/typescript"); }} />}<div className="my-1 border-t border-border" /><MenuButton icon={<FileJson />} label="Import project or repro…" onClick={importProject} /></TransientPopover>}
          <div className="mx-1 h-5 w-px bg-border" />
          <Button aria-label="Undo" aria-description={historyBlocked ? "Apply or Revert the source draft before moving history." : undefined} disabled={historyBlocked || !snapshot.presentation.canUndo} size="icon" variant="ghost" className="size-8" title={historyBlocked ? "Apply or Revert the source draft before moving history" : snapshot.presentation.canUndo ? "Undo last change" : "Nothing to undo"} onClick={() => void command("history.undo")}><Undo2 className="size-4" /></Button>
          <Button aria-label="Redo" aria-description={historyBlocked ? "Apply or Revert the source draft before moving history." : undefined} disabled={historyBlocked || !snapshot.presentation.canRedo} size="icon" variant="ghost" className="size-8" title={historyBlocked ? "Apply or Revert the source draft before moving history" : snapshot.presentation.canRedo ? "Redo last change" : "Nothing to redo"} onClick={() => void command("history.redo")}><Redo2 className="size-4" /></Button>
        </div>
        <div className="pointer-events-none absolute left-1/2 top-1/2 max-w-[34vw] -translate-x-1/2 -translate-y-1/2 text-center"><p className="truncate text-xs font-medium text-foreground">{snapshot.project.title}</p><p className="flex items-center justify-center gap-1 text-[9px] uppercase tracking-wider text-muted"><span className={`size-1.5 rounded-full ${snapshot.project.status === "failed" ? "bg-danger" : localDraftDirty || snapshot.project.status === "dirty" ? "bg-accent" : "bg-emerald-400"}`} />{snapshot.project.status === "failed" ? "failed" : localDraftDirty || snapshot.project.status === "dirty" ? "dirty draft" : "accepted"} · r{snapshot.revision}</p></div>
        <div className="ml-auto flex items-center gap-1">
          <div role="group" aria-label="Workspace layout" className="flex rounded-md border border-border bg-canvas p-0.5">{(["design", "split", "code"] as const).map((value) => <button key={value} aria-pressed={mode === value} onClick={() => { transient.close(false); setMode(value); }} className="h-6 rounded px-2 text-[11px] capitalize text-muted outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent aria-pressed:bg-raised aria-pressed:text-foreground">{value}</button>)}</div>
          <Button aria-label={mode === "code" ? "Explorer unavailable in Code layout" : explorerOpen ? "Hide Explorer" : "Show Explorer"} disabled={mode === "code"} size="icon" variant="ghost" className="size-8" onClick={() => setExplorerOpen((open) => !open)} title={mode === "code" ? "Explorer is available in Design and Split layouts" : undefined}>{explorerOpen ? <PanelLeftClose className="size-4" /> : <PanelLeftOpen className="size-4" />}</Button>
          <Button aria-label={mode === "code" ? "Details unavailable in Code layout" : detailsOpen ? "Hide details" : "Show details"} disabled={mode === "code"} size="icon" variant="ghost" className="size-8" onClick={() => setDetailsOpen((open) => !open)} title={mode === "code" ? "Details are available in Design and Split layouts" : undefined}>{detailsOpen ? <PanelRightClose className="size-4" /> : <PanelRightOpen className="size-4" />}</Button>
          <div className="relative"><Button aria-label="Diagnostics" aria-haspopup="menu" aria-expanded={transient.active === "diagnostics"} size="icon" variant="ghost" className="size-8" onClick={(event) => transient.toggle("diagnostics", event.currentTarget)}><Activity className="size-4" /></Button>{transient.active === "diagnostics" && <TransientPopover surfaceRef={transient.contentRef} label="Diagnostics" className="right-0 top-9 w-60"><MenuButton icon={<Activity />} label="Interaction trace…" onClick={() => { transient.close(); setReproOpen(true); }} /><MenuButton icon={<Download />} label="Download reproduction" onClick={() => { transient.close(); if (replacementBlocked) { reportError("Apply or Revert the current source draft before downloading an exact reproduction."); return; } void adapter.exportReproduction().then(downloadExport).catch(reportError); }} /><MenuButton icon={<FileJson />} label="Download diagnostics" onClick={() => { transient.close(); downloadText("geosolve-diagnostics.json", JSON.stringify({ version: 1, project: snapshot.project, revision: snapshot.revision, problems: snapshot.problems }, null, 2), "application/json"); }} /></TransientPopover>}</div>
        </div>
      </header>

      {actionError && <div role="alert" className="fixed bottom-3 left-1/2 z-[55] flex max-h-24 min-h-10 w-[min(42rem,calc(100vw-2rem))] -translate-x-1/2 items-center gap-2 overflow-auto rounded-lg border border-danger/60 bg-surface/95 px-3 py-1.5 text-xs text-foreground shadow-panel backdrop-blur-sm"><AlertTriangle className="size-3.5 shrink-0 text-red-300" /><span className="min-w-0 flex-1 leading-relaxed">{actionError}</span><Button aria-label="Dismiss action error" size="icon" variant="ghost" className="size-7 shrink-0" onClick={() => setActionError(null)}><X className="size-3.5" /></Button></div>}

      <div className="relative flex min-h-0 flex-1">
        {mode !== "code" && <ToolRail catalog={toolCatalog} activeTool={activeTool} activeSurface={transient.active} surfaceRef={transient.contentRef} toggleSurface={transient.toggle} selectTool={chooseTool} />}
        <PanelGroup direction="horizontal" autoSaveId="geosolve-workbench-shell-v2" storage={panelStorage}>
          {explorerOpen && mode !== "code" && <><Panel id="explorer" defaultSize={9} minSize={6} maxSize={22}><Explorer snapshot={snapshot} actions={declarationActions} blockedReason={localDraftDirty ? "Apply or Revert the source draft before structured declaration actions." : undefined} /></Panel><ResizeHandle /></>}
          <Panel id="workspace" defaultSize={70} minSize={45}>
            <main className="relative flex h-full min-h-0 flex-col">
              <StableWorkspace mode={mode} splitCodeWidth={splitCodeWidth} onSplitCodeWidth={setSplitCodeWidth} canvas={<DesignWorkspace adapter={adapter} snapshot={snapshot} catalog={toolCatalog} onSnapshot={acceptCanvasSnapshot} onError={reportError} activeTool={activeTool} onFinish={() => void command("tool.finish")} onCancel={() => chooseTool("select")} onGeometryRole={(selected) => void command(selected ? "geometry.role.toggle" : "geometry.authoring-role.toggle")} onViewCommand={(viewCommand) => void command(viewCommand)} captured={setCapturedGesture} />} code={<CodeWorkspace mode={mode} surface={codeSurface} setSurface={setCodeSurface} snapshot={snapshot} selectedFile={selectedFile} draft={draft} setDraft={setDraft} command={command} navigation={editorNavigation} declarationActions={declarationActions} declarationBlockedReason={localDraftDirty ? "Apply or Revert the source draft before structured declaration actions." : undefined} onParameterEdit={(id, value) => { if (localDraftDirty) return; void command("parameter.edit", { id, value }); }} onProblemOpen={openProblem} />} />
              {transient.active === "open" && <div ref={transient.contentRef as React.RefObject<HTMLDivElement>} role="dialog" aria-modal="false" aria-label="Open project" className="absolute inset-5 z-40 overflow-hidden rounded-xl border border-border bg-raised shadow-panel"><OpenSurface recents={recentSamples} onOpen={openSample} onNewSketch={() => replaceProject("project.new", "design")} onNewCode={() => replaceProject("project.new-code", "code")} onImport={importProject} onDismiss={() => transient.close(true)} /></div>}
            </main>
          </Panel>
          {detailsOpen && mode !== "code" && <><ResizeHandle /><Panel id="details" defaultSize={21} minSize={18} maxSize={34}><DetailsPanel snapshot={snapshot} onOpenCode={openSelectionInCode} onParameterEdit={(id, value) => { if (localDraftDirty) return; void command("parameter.edit", { id, value }); }} onProblemOpen={openProblem} parametersBlocked={localDraftDirty} /></Panel></>}
        </PanelGroup>
      </div>

      <ReproDialog adapter={adapter} open={reproOpen} onOpenChange={setReproOpen} onError={reportError} />
    </div>
  );
}

function StableWorkspace({ mode, splitCodeWidth, onSplitCodeWidth, canvas, code }: { mode: WorkspaceMode; splitCodeWidth: number; onSplitCodeWidth: (width: number) => void; canvas: React.ReactNode; code: React.ReactNode }) {
  const host = useRef<HTMLDivElement>(null);
  const drag = useRef<{ pointer: number; x: number; width: number } | null>(null);
  const clamp = useCallback((width: number) => {
    const available = host.current?.clientWidth ?? (SPLIT_CODE_MINIMUM_PX + 280);
    return Math.round(Math.max(SPLIT_CODE_MINIMUM_PX, Math.min(Math.max(SPLIT_CODE_MINIMUM_PX, available - 280), width)));
  }, []);
  return <div ref={host} className="flex h-full min-h-0 min-w-0 overflow-hidden">
    <div aria-hidden={mode === "code"} className={`${mode === "code" ? "hidden" : "flex"} min-h-0 min-w-[280px] flex-1 flex-col overflow-hidden`}>{canvas}</div>
    <div
      role="separator"
      aria-label="Resize canvas and code"
      aria-orientation="vertical"
      aria-valuemin={SPLIT_CODE_MINIMUM_PX}
      aria-valuemax={960}
      aria-valuenow={Math.round(splitCodeWidth)}
      tabIndex={mode === "split" ? 0 : -1}
      className={`${mode === "split" ? "block" : "hidden"} group relative z-20 w-1 shrink-0 cursor-col-resize bg-border outline-none focus-visible:bg-accent`}
      onDoubleClick={() => onSplitCodeWidth(clamp(SPLIT_CODE_DEFAULT_PX))}
      onKeyDown={(event) => {
        const delta = event.key === "ArrowLeft" ? 20 : event.key === "ArrowRight" ? -20 : 0;
        if (delta) { event.preventDefault(); onSplitCodeWidth(clamp(splitCodeWidth + delta)); }
        else if (event.key === "Home") { event.preventDefault(); onSplitCodeWidth(clamp(SPLIT_CODE_DEFAULT_PX)); }
      }}
      onPointerDown={(event) => { event.currentTarget.setPointerCapture(event.pointerId); drag.current = { pointer: event.pointerId, x: event.clientX, width: splitCodeWidth }; }}
      onPointerMove={(event) => { if (drag.current?.pointer === event.pointerId) onSplitCodeWidth(clamp(drag.current.width + drag.current.x - event.clientX)); }}
      onPointerUp={(event) => { if (drag.current?.pointer === event.pointerId) drag.current = null; }}
      onLostPointerCapture={() => { drag.current = null; }}
    ><span className="absolute inset-y-0 -left-1 w-3" /></div>
    <div aria-hidden={mode === "design"} inert={mode === "design" ? true : undefined} style={mode === "split" ? { flexBasis: `${splitCodeWidth}px` } : undefined} className={`${mode === "design" ? "hidden" : "flex"} min-h-0 min-w-0 flex-col overflow-hidden ${mode === "split" ? "shrink-0 grow-0" : "flex-1"}`}>{code}</div>
  </div>;
}

function DesignWorkspace({ adapter, snapshot, catalog, onSnapshot, onError, activeTool, onFinish, onCancel, onGeometryRole, onViewCommand, captured }: { adapter: WorkbenchAdapter; snapshot: WorkbenchSnapshot; catalog: ToolCatalog; onSnapshot: (snapshot: WorkbenchSnapshot) => void; onError: (error: unknown) => void; activeTool: string; onFinish: () => void; onCancel: () => void; onGeometryRole: (selected: boolean) => void; onViewCommand: (command: "view.grid.toggle" | "view.fit" | "view.origin") => void; captured: (value: boolean) => void }) {
  const section = toolSection(catalog, activeTool);
  const command = activeTool === catalog.select.toolId ? catalog.select : section?.commands.find((candidate) => candidate.toolId === activeTool) ?? catalog.select;
  const authoring = activeTool !== catalog.select.toolId;
  const authoringRoleVisible = section?.id === "sketch" && activeTool !== "sketch-point";
  const selectedRole = activeTool === catalog.select.toolId ? snapshot.presentation.selectedGeometryRole : undefined;
  const selectedRoleVisible = selectedRole !== undefined;
  const displayedRole = selectedRole ?? snapshot.presentation.geometryRole;
  const constructionRole = displayedRole === "construction";
  const mixedRole = displayedRole === "mixed";
  const geometryRoleLabel = selectedRoleVisible ? `Selected curves: ${mixedRole ? "Mixed roles" : constructionRole ? "Construction" : "Profile"}` : `New curves: ${constructionRole ? "Construction" : "Profile"}`;
  const nextGeometryRole = constructionRole ? "Profile" : "Construction";
  return <section className="flex h-full min-h-0 flex-1 flex-col">
    <div className="flex h-9 shrink-0 items-center gap-2 border-b border-border bg-raised/95 px-2 text-xs text-muted">
      <ToolIcon className="size-4 text-accent" icon={command.icon} />
      {section && <><span className="text-[10px] uppercase tracking-wider text-muted">{section.label}</span><span aria-hidden="true" className="text-border">/</span></>}
      <span className="font-medium text-foreground">{toolLabel(catalog, activeTool)}</span>
      {authoring && <span className="min-w-0 truncate text-[11px]">{snapshot.presentation.canFinish ? "Click the canvas to continue · Enter or Finish completes · Esc cancels" : "Click the canvas to continue · Esc cancels"}</span>}
      <span className="ml-auto flex shrink-0 items-center gap-1">
        {(authoringRoleVisible || selectedRoleVisible) && <Button aria-label={`${geometryRoleLabel}. Change to ${nextGeometryRole}`} aria-pressed={mixedRole ? "mixed" : constructionRole} className="h-7 text-[11px] aria-pressed:bg-cyan-400/15 aria-pressed:text-cyan-100" onClick={() => onGeometryRole(selectedRoleVisible)} size="compact" title={selectedRoleVisible ? `Selected curves have ${mixedRole ? "mixed" : constructionRole ? "Construction" : "Profile"} roles. Click to change them to ${nextGeometryRole}.` : `New curves are ${constructionRole ? "Construction" : "Profile"} geometry. Click to change the authoring role.`} variant="ghost"><ToolIcon className="size-3.5 shrink-0 text-cyan-300" icon={catalog.geometryRole.icon} />{geometryRoleLabel}</Button>}
        {authoring && <><span aria-hidden="true" className="mx-0.5 h-4 w-px bg-border" /><Button size="compact" variant="ghost" className="h-7" onClick={onCancel}>Cancel</Button><Button size="compact" variant="default" className="h-7" disabled={!snapshot.presentation.canFinish} onClick={onFinish}>Finish</Button></>}
      </span>
    </div>
    <div className="relative min-h-0 flex-1">
      <CanvasViewport adapter={adapter} snapshot={snapshot} onSnapshot={onSnapshot} onCaptureChange={captured} onError={onError} />
      <CanvasControls gridVisible={snapshot.presentation.gridVisible} onCommand={onViewCommand} />
    </div>
  </section>;
}

const CODE_SURFACES: Array<{ id: CodeSurface; label: string }> = [
  { id: "source", label: "Source" },
  { id: "parameters", label: "Parameters" },
  { id: "problems", label: "Problems" },
  { id: "generated", label: "Generated" },
  { id: "artifacts", label: "Artifacts" },
];

function CodeWorkspace({ mode, surface, setSurface, snapshot, selectedFile, draft, setDraft, command, navigation, declarationActions, declarationBlockedReason, onParameterEdit, onProblemOpen }: { mode: WorkspaceMode; surface: CodeSurface; setSurface: (surface: CodeSurface) => void; snapshot: WorkbenchSnapshot; selectedFile: WorkbenchSnapshot["source"]["files"][number]; draft: string; setDraft: (value: string) => void; command: (name: string, payload?: unknown) => Promise<WorkbenchSnapshot>; navigation: EditorNavigation | null; declarationActions: DeclarationPanelActions; declarationBlockedReason?: string; onParameterEdit: (id: string, value: string) => void; onProblemOpen: (problem: WorkbenchSnapshot["problems"][number]) => void }) {
  const dirty = snapshot.source.dirty || draft !== selectedFile.contents;
  const revert = () => { setDraft(selectedFile.contents); if (snapshot.source.dirty) void command("source.revert"); };
  const selectFile = (path: string) => { if (dirty || path === selectedFile.path) return; void command("source.select", { path }).then((next) => setDraft(next.source.files.find((file) => file.path === path)?.contents ?? "")); };
  return <section aria-label="Code workspace" className="flex h-full min-h-[500px] min-w-[520px] flex-col bg-canvas">
    <div role="tablist" aria-label="Code surfaces" className={`${mode === "code" ? "flex" : "hidden"} h-9 shrink-0 items-end gap-1 border-b border-border bg-raised px-2`}>{CODE_SURFACES.map((item) => <button key={item.id} type="button" role="tab" aria-selected={surface === item.id} onClick={() => setSurface(item.id)} className="h-8 rounded-t border-b-2 border-transparent px-3 text-xs text-muted outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent aria-selected:border-accent aria-selected:bg-surface aria-selected:text-foreground">{item.label}{item.id === "problems" && snapshot.problems.length > 0 ? ` (${snapshot.problems.length})` : ""}</button>)}</div>
    <header className="flex h-10 shrink-0 items-center border-b border-border bg-surface px-2"><div role="tablist" aria-label="Source files" className="flex min-w-0 flex-1 gap-1 overflow-x-auto">{snapshot.source.files.map((file) => <button type="button" role="tab" aria-selected={file.path === selectedFile.path} aria-controls="source-editor" disabled={dirty && file.path !== selectedFile.path} onClick={() => { setSurface("source"); selectFile(file.path); }} key={file.path} className="h-8 max-w-48 shrink-0 truncate rounded-t border-b-2 border-transparent px-3 text-xs text-muted outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40 aria-selected:border-accent aria-selected:bg-raised aria-selected:text-foreground"><Code2 className="mr-1.5 inline size-3" />{file.path}</button>)}</div><span className={`mx-2 shrink-0 text-[10px] uppercase tracking-wider ${dirty ? "text-accent" : "text-emerald-300"}`}>{dirty ? "Unapplied changes" : "Accepted source"}</span><Button size="compact" variant="ghost" disabled={!dirty} onClick={revert}><RotateCcw className="size-3" />Revert</Button><Button size="compact" variant="default" disabled={!dirty || selectedFile.readOnly} onClick={() => void command("source.prepare", { path: selectedFile.path, contents: draft })}><Play className="size-3" />Apply</Button></header>
    <div aria-hidden={surface !== "source" && mode === "code"} inert={surface !== "source" && mode === "code" ? true : undefined} className={`${surface !== "source" && mode === "code" ? "hidden" : "flex"} min-h-0 flex-1 flex-col`}>
      <div id="source-editor" role="tabpanel" className="flex min-h-0 flex-1"><CodeEditor value={draft} readOnly={selectedFile.readOnly} onChange={setDraft} navigation={navigation} /></div>
    </div>
    {mode === "code" && surface !== "source" && <div role="tabpanel" className="min-h-0 flex-1 overflow-auto p-5">{surface === "parameters" && <ParametersView snapshot={snapshot} onEdit={onParameterEdit} blocked={dirty} />}{surface === "problems" && <ProblemsView snapshot={snapshot} onOpen={onProblemOpen} />}{surface === "generated" && <DeclarationPanel rows={snapshot.explorer} actions={declarationActions} blockedReason={declarationBlockedReason} className="mx-auto w-full max-w-3xl" />}{surface === "artifacts" && (snapshot.source.files.filter((file) => file.path !== "sketch.ts").length ? <ul className="grid gap-2">{snapshot.source.files.filter((file) => file.path !== "sketch.ts").map((file) => <li key={file.path}><button className="flex w-full items-center gap-2 rounded border border-border bg-surface p-3 text-left text-sm outline-none hover:bg-raised focus-visible:ring-1 focus-visible:ring-accent" onClick={() => { setSurface("source"); selectFile(file.path); }}><PackageOpen className="size-4 text-accent" /><span className="truncate">{file.path}</span><span className="ml-auto text-[10px] uppercase text-muted">read-only</span></button></li>)}</ul> : <CodeEmpty icon={<PackageOpen />} title="No custom artifacts" detail="Pinned data-only project artifacts appear here." />)}</div>}
  </section>;
}

function CodeEmpty({ icon, title, detail }: { icon: React.ReactNode; title: string; detail: string }) { return <div className="grid h-full min-h-64 place-content-center justify-items-center text-center"><span className="mb-3 text-muted [&>svg]:size-6">{icon}</span><p className="text-sm text-foreground">{title}</p><p className="mt-1 max-w-sm text-xs leading-relaxed text-muted">{detail}</p></div>; }

function ResizeHandle({ horizontal = false }: { horizontal?: boolean }) { return <PanelResizeHandle aria-label={horizontal ? "Resize workspace rows" : "Resize workspace columns"} aria-valuemin={0} aria-valuemax={100} aria-valuenow={50} className={`${horizontal ? "h-1 cursor-row-resize" : "w-1 cursor-col-resize"} group relative z-20 shrink-0 bg-border outline-none focus-visible:bg-accent`}><span className={`absolute ${horizontal ? "inset-x-0 -top-1 h-3" : "inset-y-0 -left-1 w-3"}`} /></PanelResizeHandle>; }
function MenuButton({ icon, label, shortcut, onClick }: { icon: React.ReactNode; label: string; shortcut?: string; onClick: (event: React.MouseEvent<HTMLButtonElement>) => void }) { return <button role="menuitem" onClick={onClick} className="flex h-8 w-full items-center gap-2 rounded px-2 text-left text-sm text-foreground outline-none hover:bg-neutral-700 focus-visible:bg-neutral-700 focus-visible:ring-1 focus-visible:ring-accent"><span className="text-muted [&>svg]:size-3.5">{icon}</span>{label}{shortcut && <kbd className="ml-auto text-[10px] text-muted">{shortcut}</kbd>}</button>; }
function ReproDialog({ adapter, open, onOpenChange, onError }: { adapter: WorkbenchAdapter; open: boolean; onOpenChange: (open: boolean) => void; onError: (error: unknown) => void }) {
  const [payload, setPayload] = useState<{ filename: string; contents: string }>({ filename: "geosolve-interaction-trace.txt", contents: "No interaction trace events recorded." });
  useEffect(() => {
    if (!open) return;
    let current = true;
    setPayload({ filename: "geosolve-interaction-trace.txt", contents: "Loading the current interaction trace…" });
    void adapter.exportInteractionTrace().then((next) => { if (current) setPayload(next); }).catch((error) => { if (current) onError(error); });
    return () => { current = false; };
  }, [adapter, onError, open]);
  return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Portal><Dialog.Overlay className="fixed inset-0 z-[60] bg-black/70 backdrop-blur-sm" /><Dialog.Content aria-describedby="trace-description" className="fixed left-1/2 top-1/2 z-[61] flex h-[min(620px,80vh)] w-[min(820px,80vw)] -translate-x-1/2 -translate-y-1/2 flex-col rounded-xl border border-border bg-raised p-4 shadow-panel outline-none"><div className="flex items-start"><div><Dialog.Title className="text-base font-semibold">Interaction trace</Dialog.Title><Dialog.Description id="trace-description" className="mt-1 text-xs text-muted">Bounded diagnostic transport. Downloads remain available when clipboard payloads grow too large.</Dialog.Description></div><Dialog.Close asChild><Button className="ml-auto" size="icon" variant="ghost" aria-label="Close interaction trace"><X className="size-4" /></Button></Dialog.Close></div><textarea readOnly aria-label="Interaction trace payload" value={payload.contents} className="mt-4 min-h-0 flex-1 resize-none rounded border border-border bg-canvas p-3 font-mono text-xs text-muted outline-none focus:border-accent" /><footer className="mt-3 flex justify-end gap-2"><Button onClick={() => downloadText(payload.filename, payload.contents, "text/plain")}><Download className="size-3.5" />Download</Button><Button variant="default" onClick={() => void navigator.clipboard?.writeText(payload.contents).catch(onError)}>Copy trace</Button></footer></Dialog.Content></Dialog.Portal></Dialog.Root>;
}
async function exportProject(adapter: WorkbenchAdapter) { const payload = await adapter.exportProject(); const url = URL.createObjectURL(new Blob([payload.contents], { type: "application/json" })); const anchor = document.createElement("a"); anchor.href = url; anchor.download = payload.filename; anchor.click(); URL.revokeObjectURL(url); }
function downloadText(filename: string, contents: string, type: string) { const url = URL.createObjectURL(new Blob([contents], { type })); const anchor = document.createElement("a"); anchor.href = url; anchor.download = filename; anchor.click(); URL.revokeObjectURL(url); }
function downloadExport(payload: { filename: string; contents: string }) { downloadText(payload.filename, payload.contents, "text/plain"); }
function errorText(error: unknown) { return error instanceof Error ? error.message : String(error); }
function lineColumnOffset(source: string, line: number, column: number) { let offset = 0; const lines = source.split("\n"); for (let index = 1; index < Math.max(1, line) && index <= lines.length; index += 1) offset += (lines[index - 1]?.length ?? 0) + 1; return Math.min(source.length, offset + Math.max(0, column - 1)); }
function restoredBrowserDraft(snapshot: WorkbenchSnapshot, file?: WorkbenchSnapshot["source"]["files"][number]) {
  if (!file || file.readOnly) return { contents: null, issue: null };
  const stored = readBrowserStorage(DRAFT_KEY, "the unapplied source draft");
  if (stored.issue) return { contents: null, issue: stored.issue };
  try {
    const saved = JSON.parse(stored.value ?? "null") as { version?: unknown; title?: unknown; sampleKey?: unknown; path?: unknown; base?: unknown; contents?: unknown } | null;
    if (saved?.version !== 1 || saved.title !== snapshot.project.title || (saved.sampleKey ?? null) !== (snapshot.project.sampleKey ?? null) || saved.path !== file.path || saved.base !== file.contents || typeof saved.contents !== "string") return { contents: null, issue: null };
    return { contents: saved.contents, issue: null };
  } catch {
    const removed = removeBrowserStorage(DRAFT_KEY, "the invalid unapplied source draft");
    return { contents: null, issue: removed.issue };
  }
}
