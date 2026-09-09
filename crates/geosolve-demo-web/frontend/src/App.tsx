// SPDX-License-Identifier: GPL-3.0-or-later
import * as Dialog from "@radix-ui/react-dialog";
import { Panel, PanelGroup, PanelResizeHandle, type PanelGroupStorage } from "react-resizable-panels";
import { Activity, AlertTriangle, ChevronDown, Code2, Download, FileJson, FolderOpen, Focus, Menu, PackageOpen, PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, Play, Redo2, RotateCcw, Save, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { DimensionDisplayMode, WorkbenchAdapter, WorkbenchSnapshot, WorkspaceMode } from "./lib/adapter";
import { assertWorkbenchSnapshot } from "./lib/adapter";
import { readBrowserStorage, removeBrowserStorage, writeBrowserStorage } from "./lib/browser-storage";
import { createProjectStore, type ProjectStore } from "./lib/project-storage";
import { readRecentSamples, rememberRecentSample } from "./lib/recent-samples";
import { resolvePendingManagedMutationSnapshot } from "./lib/pending-managed-mutation";
import { utf8ByteSpanToUtf16Range, utf8ByteSpansToUtf16Ranges, utf16RangeToUtf8ByteSpan, type Utf16SourceRange } from "./lib/source-navigation";
import { MockWorkbenchAdapter } from "./lib/mock-adapter";
import { useTransientSurface } from "./hooks/use-transient-surface";
import { Button } from "./components/ui/button";
import { ToolRail } from "./components/tool-rail";
import { ToolIcon } from "./components/tool-icon";
import { TransientPopover } from "./components/transient-popover";
import { CanvasViewport } from "./components/canvas-viewport";
import { CanvasControls } from "./components/canvas-controls";
import { AuthoringDocumentProperties, type AuthoringMetadataActions } from "./components/authoring-metadata";
import { DeclarationPanel, DetailsPanel, Explorer, ParametersView, ProblemsView, type DeclarationPanelActions } from "./components/side-panels";
import { CodeEditor, type EditorNavigation } from "./components/code-editor";
import { OpenSurface, type SampleEntry } from "./components/open-surface";
import type { ToolCatalog } from "./lib/tool-catalog";
import type { FolderWorkbenchAdapter } from "./lib/folder-adapter";
import { toolLabel, toolSection } from "./lib/tool-catalog";

const FALLBACK = new MockWorkbenchAdapter();
const DEFAULT_PROJECT_STORE = createProjectStore();
const PRESENTATION_KEY = "geosolve.presentation.v1";
const DRAFT_KEY = "geosolve.source-draft.v1";
const SPLIT_CODE_DEFAULT_PX = 560;
const SPLIT_CODE_MINIMUM_PX = 520;
const DURABLE_REPLACEMENT_COMMANDS = new Set(["project.new", "project.new-code", "project.import", "sample.open"]);
const PRESENTATION_SAVE_COMMANDS = new Set(["explorer.visibility.set", "explorer.visibility.isolate", "explorer.visibility.restore", "view.construction.toggle", "dimensions.mode", "dimensions.pin", "dimensions.clearPins"]);
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

export interface AppProps { adapter?: WorkbenchAdapter; projectStore?: ProjectStore; folder?: FolderWorkbenchAdapter; }

export default function App({ adapter = FALLBACK, projectStore = DEFAULT_PROJECT_STORE, folder }: AppProps) {
  const [, updateFolderStatus] = useState(0);
  const [initialPresentation] = useState(readPresentation);
  const [initialRecents] = useState(readRecentSamples);
  const [snapshot, setSnapshot] = useState<WorkbenchSnapshot | null>(null);
  const [toolCatalog, setToolCatalog] = useState<ToolCatalog | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [mode, setMode] = useState<WorkspaceMode>(initialPresentation.mode);
  const [replacementFitRequest, setReplacementFitRequest] = useState(0);
  const fittedReplacement = useRef(0);
  const activeTool = snapshot?.presentation.activeTool ?? "select";
  const [capturedGesture, setCapturedGesture] = useState(false);
  const [metadataPending, setMetadataPending] = useState(false);
  const metadataInFlight = useRef(false);
  const metadataFocus = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (metadataPending || metadataInFlight.current) return;
    const origin = metadataFocus.current;
    metadataFocus.current = null;
    if (origin?.isConnected && (document.activeElement === document.body || document.activeElement === origin)) origin.focus({ preventScroll: true });
  }, [metadataPending]);
  const [reproOpen, setReproOpen] = useState(false);
  const [explorerOpen, setExplorerOpen] = useState(initialPresentation.explorer);
  const [detailsOpen, setDetailsOpen] = useState(initialPresentation.details);
  const [splitCodeWidth, setSplitCodeWidth] = useState(initialPresentation.splitCodeWidth);
  const [codeSurface, setCodeSurface] = useState<CodeSurface>(initialPresentation.codeSurface);
  const [recentSamples, setRecentSamples] = useState(initialRecents.entries);
  const [draft, setDraft] = useState("");
  const [draftStorageSafe, setDraftStorageSafe] = useState(false);
  const [actionError, setActionError] = useState<string | null>([initialPresentation.storageIssue, initialRecents.issue].filter(Boolean).join("; ") || null);
  const [editorNavigation, setEditorNavigation] = useState<EditorNavigation | null>(null);
  const snapshotRef = useRef<WorkbenchSnapshot | null>(null);
  const draftRef = useRef("");
  const navigationRequest = useRef(0);
  const lastAutomaticReveal = useRef<string | null>(null);
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
    if (folder) return folder.refresh().then(() => undefined).catch(reportError);
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
          setDraftStorageSafe(true);
        }
        if (stored.issue) reportError(stored.issue);
        else if (clearError) setActionError(null);
      } catch (error) { reportError(error); }
    });
    projectSaveTail.current = save.catch(() => undefined);
    return save;
  }, [adapter, folder, projectStore, reportError]);

  const start = useCallback(() => {
    const request = startupRequest.current + 1;
    startupRequest.current = request;
    projectSaveEpoch.current += 1;
    projectAutosaveSafe.current = false;
    setDraftStorageSafe(false);
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
      const restored = folder ? { contents: file?.contents, issue: null } : restoredBrowserDraft(checked, file);
      if (restored.issue) reportError(restored.issue);
      setDraft(restored.contents ?? file?.contents ?? "");
      setDraftStorageSafe(projectAutosaveSafe.current);
      return true;
    };
    if (folder) {
      void adapter.construct({ version: 2 }).then(install).catch((error) => setStartupError(errorText(error)));
      return;
    }
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
        await install(await adapter.construct({ version: 2, persistedProject }));
      }
      catch (error) {
        if (startupRequest.current !== request) return;
        if (!persistedProject) {
          setStartupError([errorText(error), storedProject.issue].filter(Boolean).join("; "));
          return;
        }
        projectAutosaveSafe.current = false;
        try {
          const installed = await install(await adapter.construct({ version: 2 }));
          if (!installed) return;
          if (!storedProject.autosaveSafe) {
            setActionError(`The fallback saved workspace could not be restored: ${errorText(error)}; IndexedDB authority was unread, so the existing browser data was retained and automatic project saving remains paused until you use Save in browser.`);
            return;
          }
          if (startupRequest.current !== request) return;
          setActionError(`Saved workspace could not be restored: ${errorText(error)}; your saved data was retained. Automatic project saving is paused until you use Save in browser.`);
        }
        catch (fallbackError) {
          if (startupRequest.current === request) setStartupError(errorText(fallbackError));
        }
      }
    }).catch((error: unknown) => {
      if (startupRequest.current === request) setStartupError(errorText(error));
    });
  }, [adapter, folder, projectStore, queueProjectSave, reportError, resolveAdapterSnapshot]);
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
  useEffect(() => folder?.subscribe((next) => {
    updateFolderStatus((value) => value + 1);
    if (next && snapshotRef.current) void acceptSnapshot(next).catch(reportError);
  }), [acceptSnapshot, folder, reportError]);
  const command = useCallback(async (name: string, payload?: unknown) => {
    try {
      const previousDimensionMode = snapshotRef.current?.dimensions?.mode ?? "focused";
      const next = await adapter.dispatch({ version: 2, command: name, payload });
      if (projectAutosaveSafe.current) setActionError(null);
      const accepted = await acceptSnapshot(next);
      if (DURABLE_REPLACEMENT_COMMANDS.has(name)) {
        suppressInstalledSnapshotAutosave.current = accepted;
        projectSaveEpoch.current += 1;
        setReplacementFitRequest((request) => request + 1);
        void queueProjectSave("replacement", false);
      } else if (PRESENTATION_SAVE_COMMANDS.has(name) || (name === "dimensions.focus" && previousDimensionMode !== accepted.dimensions?.mode)) {
        void queueProjectSave("auto", false);
      }
      return accepted;
    } catch (error) {
      reportError(error);
      const current = snapshotRef.current;
      if (!current) throw error;
      return current;
    }
  }, [acceptSnapshot, adapter, queueProjectSave, reportError]);

  useLayoutEffect(() => {
    if (replacementFitRequest === fittedReplacement.current) return;
    // Project replacement can also switch Design/Code to Split. Fit only after
    // the panels finish their layout effects, before the next paint. A hidden
    // canvas keeps the request until it is shown.
    let cancelled = false;
    const epoch = projectSaveEpoch.current;
    const frame = requestAnimationFrame(() => {
      const bounds = document.querySelector<HTMLElement>('[role="application"]')?.getBoundingClientRect();
      if (!bounds || bounds.width <= 0 || bounds.height <= 0) return;
      void (async () => {
        await adapter.resize({ version: 2, width: bounds.width, height: bounds.height, pixelRatio: window.devicePixelRatio || 1 });
        if (cancelled || epoch !== projectSaveEpoch.current) return;
        const next = await adapter.dispatch({ version: 2, command: "view.fit" });
        if (cancelled || epoch !== projectSaveEpoch.current) return;
        fittedReplacement.current = replacementFitRequest;
        await acceptSnapshot(next);
      })().catch(reportError);
    });
    return () => { cancelled = true; cancelAnimationFrame(frame); };
  }, [acceptSnapshot, adapter, mode, replacementFitRequest, reportError]);
  const verifiedCommand = useCallback(async (name: string, payload?: unknown) => {
    try {
      const next = assertWorkbenchSnapshot(await adapter.dispatch({ version: 2, command: name, payload }));
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
    if (!snapshot || folder) return;
    if (suppressInstalledSnapshotAutosave.current === snapshot) {
      suppressInstalledSnapshotAutosave.current = null;
      return;
    }
    void queueProjectSave("auto", false);
  }, [folder, queueProjectSave, snapshot?.project.sampleKey, snapshot?.project.title, snapshot?.revision]);

  useEffect(() => {
    // A failed/uncertain project restore also protects its separately saved
    // unapplied source. Only a successful explicit save releases that protection.
    if (!snapshot || !draftStorageSafe || folder) return;
    const file = snapshot.source.files.find((candidate) => candidate.path === snapshot.source.selectedPath);
    if (!file || file.readOnly || draft === file.contents) {
      const removed = removeBrowserStorage(DRAFT_KEY, "the unapplied source draft");
      if (removed.issue) reportError(removed.issue);
      return;
    }
    const stored = writeBrowserStorage(DRAFT_KEY, JSON.stringify({ version: 1, title: snapshot.project.title, sampleKey: snapshot.project.sampleKey ?? null, path: file.path, base: file.contents, contents: draft }), "the unapplied source draft");
    if (stored.issue) reportError(stored.issue);
  }, [draft, draftStorageSafe, folder, reportError, snapshot]);

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
      if (capturedGesture) { event.preventDefault(); setCapturedGesture(false); void adapter.cancel({ version: 2, reason: "escape" }).then((next) => next && acceptSnapshot(next)).catch(reportError); return; }
      if (activeTool !== "select") { event.preventDefault(); chooseTool("select"); queueMicrotask(() => document.querySelector<HTMLElement>('[aria-label="Select"]')?.focus()); }
    };
    document.addEventListener("keydown", onWorkspaceKey);
    return () => document.removeEventListener("keydown", onWorkspaceKey);
  }, [acceptSnapshot, activeTool, adapter, capturedGesture, chooseTool, command, reportError, reproOpen, saveBrowserProject, transient]);

  useEffect(() => {
    const navigation = snapshot?.navigation;
    const primary = navigation?.sources[0];
    if (!primary) lastAutomaticReveal.current = null;
    const currentFile = snapshot?.source.files.find((file) => file.path === snapshot.source.selectedPath);
    if (!navigation || !primary || !navigation.canNavigateSource || snapshot?.source.dirty || draft !== currentFile?.contents) return;
    if (mode === "design" || (mode === "code" && codeSurface !== "source")) return;
    if (lastAutomaticReveal.current === navigation.selectionKey) return;
    lastAutomaticReveal.current = navigation.selectionKey;
    const reveal = (next: WorkbenchSnapshot) => {
      if (snapshotRef.current?.navigation?.authority !== navigation.authority || snapshotRef.current?.navigation?.selectionKey !== navigation.selectionKey) return;
      const file = next.source.files.find((candidate) => candidate.path === primary.path);
      if (!file) return;
      const converted = utf8ByteSpanToUtf16Range(file.contents, primary.from, primary.to);
      if (!converted.ok) { reportError(converted.reason); return; }
      navigationRequest.current += 1;
      setEditorNavigation({ request: navigationRequest.current, ...converted.range, focus: false });
    };
    if (currentFile.path === primary.path) reveal(snapshot!);
    else void command("source.select", { path: primary.path }).then(reveal);
  }, [snapshot, draft, mode, codeSurface, command, reportError]);

  if (!snapshot || !toolCatalog) return <main className="grid h-dvh place-items-center bg-canvas text-sm text-muted">{startupError ? <section role="alert" className="max-w-md rounded-lg border border-danger/50 bg-raised p-5 text-center"><p className="font-medium text-foreground">Workbench unavailable</p><p className="mt-2 text-xs leading-relaxed">{startupError}</p><Button className="mt-4" onClick={start}>Try again</Button></section> : <span role="status">Starting GeoSolve…</span>}</main>;
  const selectedFile = snapshot.source.files.find((file) => file.path === snapshot.source.selectedPath) ?? snapshot.source.files[0];
  const localDraftDirty = Boolean(selectedFile && draft !== selectedFile.contents);
  const historyBlocked = localDraftDirty || snapshot.source.dirty;
  const metadataBlockedReason = metadataPending ? "Applying document properties…" : localDraftDirty || snapshot.source.dirty
    ? "Apply or Revert the source draft before editing document properties."
    : capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation)
      ? "Finish the current tool or gesture before editing document properties."
      : undefined;
  const mutateMetadata = (name: string, payload: unknown) => {
    if (metadataBlockedReason || metadataInFlight.current) return;
    metadataFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    metadataInFlight.current = true;
    setMetadataPending(true);
    void command(name, payload).finally(() => {
      metadataInFlight.current = false;
      setMetadataPending(false);
    });
  };
  const metadataActions: AuthoringMetadataActions = {
    onEdit: (payload) => mutateMetadata("authoring.metadata.set", payload),
    onExtract: (payload) => mutateMetadata("authoring.parameter.extract", payload),
  };
  const replacementBlocked = localDraftDirty || snapshot.source.dirty;
  const reportReplacementBlocked = () => reportError("Apply or Revert the current source draft before replacing this project.");
  const openSample = (sample: SampleEntry) => { if (replacementBlocked) { reportReplacementBlocked(); return; } transient.close(false); void command("sample.open", { key: sample.key, title: sample.title }).then((next) => {
    setDraft(next.source.files.find((file) => file.path === next.source.selectedPath)?.contents ?? "");
    if (next.revision !== snapshot.revision && next.project.sampleKey === sample.key) {
      const remembered = rememberRecentSample(recentSamples, sample);
      setRecentSamples(remembered.entries);
      if (remembered.issue) reportError(remembered.issue);
    }
  }); setMode("split"); };
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
    if (capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation)) { reportError("Finish the current tool or gesture before navigating between views."); return; }
    if (localDraftDirty || snapshot.source.dirty) { reportError("Apply or Revert the current draft before navigating to an authenticated source span."); return; }
    const initialFile = snapshot.source.files.find((candidate) => candidate.path === path);
    if (!initialFile) { reportError(`Source file \`${path}\` is unavailable.`); return; }
    const initialRange = locate(initialFile.contents);
    if (typeof initialRange === "string") { reportError(initialRange); return; }
    const authority = snapshot.navigation?.authority;
    const navigate = (next: WorkbenchSnapshot) => {
      if (authority && next.navigation?.authority !== authority) { reportError("The accepted source changed; select the object again."); return; }
      const file = next.source.files.find((candidate) => candidate.path === path);
      if (!file) { reportError(`Source file \`${path}\` is unavailable.`); return; }
      if (file.contents !== initialFile.contents) { reportError("The accepted source changed; select the object again."); return; }
      const range = initialRange;
      setDraft(file.contents);
      navigationRequest.current += 1;
      setEditorNavigation({ request: navigationRequest.current, ...range, focus: true });
      setCodeSurface("source");
      setMode((current) => current === "design" ? "split" : current);
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
    const source = snapshot.navigation?.sources[0] ?? snapshot.selection?.source;
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
  const showSourceInCanvas = (range: Utf16SourceRange) => {
    const navigation = snapshot.navigation;
    if (capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation) || localDraftDirty || snapshot.source.dirty || !navigation?.canNavigateSource) return;
    const converted = utf16RangeToUtf8ByteSpan(draft, range.from, range.to);
    if (!converted.ok) { reportError(converted.reason); return; }
    void verifiedCommand("navigation.source.select", { authority: navigation.authority, path: selectedFile.path, ...converted.range }).then((next) => {
      if (!next) return;
      if (next.navigation?.notice) return;
      setMode((current) => current === "code" ? "split" : current);
    });
  };
  const declarationActions: DeclarationPanelActions = {
    navigationBlockedReason: (capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation)) ? "Finish the current tool or gesture before navigating between views." : undefined,
    onSelect: (id, mode = "replace") => {
      if (capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation)) return;
      if (snapshot.navigation) void command("navigation.rows.select", { authority: snapshot.navigation.authority, ids: [id], mode });
      else void command("declaration.select", { id });
    },
    onNavigate: (row) => {
      if (capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation)) return;
      if (!row.source) { reportError("This declaration has no authenticated managed-source span."); return; }
      if (snapshot.navigation) {
        if (!snapshot.navigation.canNavigateSource) { reportError(snapshot.navigation.unavailableReason ?? "Source navigation is unavailable."); return; }
        navigateToSource(row.source.path, row.source.from, row.source.to);
        return;
      }
      void verifiedCommand("declaration.source.open", { id: row.id, from: row.source.from, to: row.source.to }).then((next) => { if (next) navigateToSource(row.source!.path, row.source!.from, row.source!.to); });
    },
    onMove: (id, move) => { void command("declaration.move", { id, ...move }); },
    onVisibility: (id, visible) => { void command("explorer.visibility.set", { id, visible }); },
    onIsolate: (id) => { void command("explorer.visibility.isolate", { id }); },
    onRestoreVisibility: () => { void command("explorer.visibility.restore"); },
    onConstructionVisibility: () => { void command("view.construction.toggle"); },
    onSuppress: (id, suppressed) => { void command("declaration.suppression.set", { id, suppressed }); },
    onDelete: (id) => { void command("declaration.delete", { id }); },
  };

  return (
    <div className="flex h-dvh min-h-[720px] min-w-[1024px] flex-col overflow-hidden bg-canvas text-foreground" onInputCapture={(event) => {
      if (event.target instanceof HTMLInputElement) folder?.fieldChanged(event.target.getAttribute("aria-label") ?? "Inspector input", event.target.value);
    }}>
      {folder && <div className="flex h-11 min-w-0 shrink-0 items-center gap-2 border-b border-border bg-raised px-3 text-xs" role="region" aria-label="Local folder">
        <strong className="shrink-0">Local folder · </strong><span className="max-w-[30%] truncate" title={folder.state?.paths.source}>{folder.state?.paths.source}</span>
        <span className={`min-w-0 flex-1 truncate ${folder.state?.ok ? "" : "text-danger"}`} title={folder.notice} role="status">{folder.notice}</span>
        <Button className="shrink-0" onClick={() => { void folder.refresh().catch(reportError); }}>Refresh from disk</Button>
        {folder.pending && <Button className="shrink-0" onClick={() => downloadText("geosolve-pending-intent.json", folder.pending, "application/json")}>Download pending intent</Button>}
      </div>}
      <input ref={importInputRef} type="file" accept=".json,.txt,application/json,text/plain" hidden onChange={(event) => importSelectedFile(event.currentTarget.files?.[0])} />
      <header className="relative z-50 flex h-11 shrink-0 items-center border-b border-border bg-surface px-2">
        <div className="relative flex items-center gap-1">
          <Button ref={fileButtonRef} aria-label="File menu" aria-haspopup="menu" aria-expanded={transient.active === "file"} size="compact" variant="ghost" onClick={(event) => transient.toggle("file", event.currentTarget)}><Menu className="size-4" />File<ChevronDown className="size-3" /></Button>
          {transient.active === "file" && <TransientPopover surfaceRef={transient.contentRef} label="File menu" className="left-0 top-9 w-56"><MenuButton icon={<FolderOpen />} label="Open…" shortcut="Ctrl O" onClick={() => { if (fileButtonRef.current) transient.toggle("open", fileButtonRef.current); }} /><MenuButton icon={<Save />} label={folder ? "Refresh saved file" : "Save in browser"} shortcut="Ctrl S" onClick={() => { transient.close(); void saveBrowserProject(); }} /><MenuButton icon={<Download />} label="Export canonical project…" onClick={() => { transient.close(); if (localDraftDirty) { reportError("Apply or Revert the current source draft before exporting a canonical project."); return; } void exportProject(adapter).catch(reportError); }} />{selectedFile.path === "sketch.ts" && <MenuButton icon={<Code2 />} label={localDraftDirty || snapshot.source.dirty ? "Download raw draft…" : "Download sketch.ts…"} onClick={() => { transient.close(); downloadText(localDraftDirty || snapshot.source.dirty ? "sketch-draft.ts" : "sketch.ts", draft, "text/typescript"); }} />}<div className="my-1 border-t border-border" /><MenuButton icon={<FileJson />} label="Import project or repro…" onClick={importProject} /></TransientPopover>}
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
          {explorerOpen && mode !== "code" && <><Panel id="explorer" defaultSize={16} minSize={12} maxSize={26}><Explorer snapshot={snapshot} actions={declarationActions} blockedReason={localDraftDirty ? "Apply or Revert the source draft before structured declaration actions." : undefined} /></Panel><ResizeHandle /></>}
          <Panel id="workspace" defaultSize={63} minSize={45}>
            <main className="relative flex h-full min-h-0 flex-col">
              <StableWorkspace mode={mode} splitCodeWidth={splitCodeWidth} onSplitCodeWidth={setSplitCodeWidth} canvas={<DesignWorkspace adapter={adapter} snapshot={snapshot} catalog={toolCatalog} onSnapshot={acceptCanvasSnapshot} onError={reportError} activeTool={activeTool} onFinish={() => void command("tool.finish")} onCancel={() => chooseTool("select")} onGeometryRole={(selected) => void command(selected ? "geometry.role.toggle" : "geometry.authoring-role.toggle")} onViewCommand={(viewCommand) => void command(viewCommand)} onDimensionMode={(dimensionMode) => void command("dimensions.mode", { mode: dimensionMode })} captured={setCapturedGesture} />} code={<CodeWorkspace metadataActions={metadataActions} metadataBlockedReason={metadataBlockedReason} mode={mode} surface={codeSurface} setSurface={setCodeSurface} snapshot={snapshot} selectedFile={selectedFile} draft={draft} setDraft={(value) => { folder?.draftChanged(value !== selectedFile.contents); setDraft(value); }} command={command} navigation={editorNavigation} onShowInCanvas={showSourceInCanvas} navigationBlockedReason={(capturedGesture || activeTool !== "select" || Boolean(snapshot.pendingManagedMutation)) ? "Finish the current tool or gesture before navigating between views." : undefined} declarationActions={declarationActions} declarationBlockedReason={localDraftDirty ? "Apply or Revert the source draft before structured declaration actions." : undefined} onParameterEdit={(id, value) => { if (localDraftDirty) return; void command("parameter.edit", { id, value }); }} onProblemOpen={openProblem} />} />
