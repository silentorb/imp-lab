import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AppShell,
  Badge,
  Box,
  Button,
  Checkbox,
  Group,
  SegmentedControl,
  Stack,
  Tabs,
  Text,
  TextInput,
  Title,
  useMantineColorScheme,
} from "@mantine/core";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Graph, Port, PrimitiveValue } from "imp-core-types";
import {
  addImpNode,
  formatLiteralValue,
  isLiteralEditableSignalType,
  literalEditorKind,
  parseLiteralText,
  setNodeInputLiteral,
  setNodeTitle,
  type GraphSelection,
} from "imp-graph-view";
import {
  getNodeDefinition,
  listNodeDefinitions,
  type Registry,
} from "imp-registry";
import {
  resolveKeyEvent,
  type BindingDto,
} from "./bindings";
import {
  CMD_DOCUMENT_CLOSE_ACTIVE,
  CMD_DOCUMENT_NEW_GRAPH,
  CMD_DOCUMENT_NEW_NOTATION,
  CMD_DOCUMENT_OPEN_FILE,
  CMD_DOCUMENT_SAVE,
  CMD_DOCUMENT_SAVE_AS,
  CMD_EDIT_REDO,
  CMD_EDIT_UNDO,
  CMD_NOTATION_SNAP_TOGGLE,
  CMD_NOTATION_TOOL_DRAW,
  CMD_NOTATION_TOOL_ERASE,
  CMD_NOTATION_TOOL_SELECT,
  CMD_VIEW_TOGGLE_COLOR_SCHEME,
  EDITOR_COMMAND_EVENT,
  type EditorCommandPayload,
} from "./commands";
import {
  canRedo,
  canUndo,
  createHistory,
  isDirty,
  markSaved,
  push,
  redo,
  undo,
  type GraphHistory,
} from "./documentHistory";
import {
  documentFromDto,
  emptyImpGraph,
  newUntitledDocument,
  type DocumentSummaryDto,
  type EditorDocument,
} from "./documents";
import { GraphErrorBoundary } from "./GraphErrorBoundary";
import { LabGraphPanel } from "./LabGraphPanel";
import {
  canRedoNotation,
  canUndoNotation,
  createNotationHistory,
  emptyNotation,
  isNotationDirty,
  markNotationSaved,
  notationFromUnknown,
  PianoRollPanel,
  pushNotation,
  redoNotation,
  secondsToBeats,
  summarizeNotation,
  undoNotation,
  type Notation,
  type NotationHistory,
  type PianoRollUiPrefs,
} from "./piano-roll";
import { getLabRegistry } from "./registry";
import {
  collectOpenDocumentsEntry,
  ENTRY_OPEN_DOCUMENTS,
  ENTRY_PIANO_ROLL_UI,
  getUserStateEntry,
  pianoEntryToPrefs,
  pianoPrefsToEntry,
  setUserStateEntry,
  type OpenDocumentsEntry,
  type PianoRollUiEntry,
} from "./userState";
import classes from "./App.module.css";

type ExtensionsLoadDto = {
  warnings?: string[];
  packRoot?: string | null;
  catalogs?: {
    nodeLibraries?: unknown[];
    typeConstraintLibraries?: unknown[];
    graphLibraries?: unknown[];
  };
  contributions?: {
    commandIds?: string[];
    menuItems?: Array<{
      commandId: string;
      label: string;
      menu: string;
      order: number;
    }>;
  };
};

function summarizeGraph(graph: Graph): string {
  const nodeCount = Object.keys(graph.nodes).length;
  const edgeCount = Object.keys(graph.edges).length;
  return `Imp graph · ${nodeCount} nodes · ${edgeCount} edges`;
}

function isUntitledOrigin(origin: string): boolean {
  return !origin || origin === "untitled" || origin.startsWith("untitled");
}

function isTextEditingTarget(el: Element | null): boolean {
  if (!el || !(el instanceof HTMLElement)) {
    return false;
  }
  if (el.isContentEditable) {
    return true;
  }
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA";
}

function documentDirty(history: GraphHistory, origin: string): boolean {
  return isDirty(history) || isUntitledOrigin(origin);
}

function notationDocDirty(history: NotationHistory, origin: string): boolean {
  return isNotationDirty(history) || isUntitledOrigin(origin);
}

type SaveResult = {
  origin: string;
  title: string;
};

type PlaybackStatusDto = {
  playing: boolean;
  positionSecs: number;
};

const DEFAULT_PIANO_PREFS: PianoRollUiPrefs = {
  tool: "draw",
  snapEnabled: true,
  snapResolution: "1/16",
};

/** Build tab + history state for a host-opened document DTO. */
function materializeOpenedDocument(dto: DocumentSummaryDto): {
  doc: EditorDocument;
  graphHistory?: GraphHistory;
  notationHistory?: NotationHistory;
  playhead?: number;
} {
  const doc = documentFromDto(dto);
  if (doc.kind === "notation") {
    const parsed = notationFromUnknown(
      doc.notation ?? emptyNotation(doc.id),
    );
    const opened = {
      ...doc,
      notation: parsed,
      summary: summarizeNotation(parsed),
    };
    return {
      doc: opened,
      notationHistory: createNotationHistory(parsed),
      playhead: 0,
    };
  }
  return {
    doc,
    graphHistory: createHistory(doc.graph ?? emptyImpGraph()),
  };
}

export default function App() {
  const { colorScheme, setColorScheme, toggleColorScheme } =
    useMantineColorScheme();
  const [documents, setDocuments] = useState<EditorDocument[]>([]);
  const [histories, setHistories] = useState<Record<string, GraphHistory>>(
    {},
  );
  const [notationHistories, setNotationHistories] = useState<
    Record<string, NotationHistory>
  >({});
  const [playheads, setPlayheads] = useState<Record<string, number>>({});
  const [pianoPrefs, setPianoPrefs] =
    useState<PianoRollUiPrefs>(DEFAULT_PIANO_PREFS);
  const [notationPlayingId, setNotationPlayingId] = useState<string | null>(
    null,
  );
  const [livePlayheadBeats, setLivePlayheadBeats] = useState<number | null>(
    null,
  );
  const documentsRef = useRef(documents);
  documentsRef.current = documents;
  const historiesRef = useRef(histories);
  historiesRef.current = histories;
  const notationHistoriesRef = useRef(notationHistories);
  notationHistoriesRef.current = notationHistories;
  const playheadsRef = useRef(playheads);
  playheadsRef.current = playheads;
  const [activeId, setActiveId] = useState("");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const titleCommitTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [status, setStatus] = useState(
    "Prototype · Open / Save bridged · Imp graphs + notation",
  );
  const [registry, setRegistry] = useState<Registry>(() => getLabRegistry());
  const [extensionCommandIds, setExtensionCommandIds] = useState<Set<string>>(
    () => new Set(),
  );
  const extensionCommandIdsRef = useRef(extensionCommandIds);
  extensionCommandIdsRef.current = extensionCommandIds;
  const [inputBindings, setInputBindings] = useState<BindingDto[]>([]);
  const inputBindingsRef = useRef(inputBindings);
  inputBindingsRef.current = inputBindings;
  /** False until startup restore finishes so sync effects skip the empty initial frame. */
  const uiStateReadyRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const loaded = await invoke<ExtensionsLoadDto>("get_extensions_load");
        if (cancelled) {
          return;
        }
        for (const warning of loaded.warnings ?? []) {
          console.warn("imp-lab extensions:", warning);
        }
        setRegistry(getLabRegistry(loaded.catalogs ?? null));
        const commandIds = new Set(loaded.contributions?.commandIds ?? []);
        setExtensionCommandIds(commandIds);
        const n = loaded.catalogs?.nodeLibraries?.length ?? 0;
        const parts: string[] = [];
        if (n > 0) {
          parts.push(`${n} extension catalog libraries`);
        }
        if (commandIds.size > 0) {
          parts.push(`${commandIds.size} extension commands`);
        }
        if (parts.length > 0) {
          setStatus((prev) =>
            parts.every((p) => prev.includes(p.split(" ")[0]!))
              ? prev
              : `${prev} · ${parts.join(" · ")}`,
          );
        }
      } catch (err) {
        console.warn("get_extensions_load failed", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const dto = await invoke<{ bindings: BindingDto[] }>(
          "get_input_bindings",
        );
        if (!cancelled) {
          setInputBindings(dto.bindings ?? []);
        }
      } catch (err) {
        console.warn("get_input_bindings failed", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const pianoEntry = await getUserStateEntry<PianoRollUiEntry>(
          ENTRY_PIANO_ROLL_UI,
        );
        if (cancelled) {
          return;
        }
        if (pianoEntry) {
          setPianoPrefs(pianoEntryToPrefs(pianoEntry, DEFAULT_PIANO_PREFS));
        }

        const openEntry = await getUserStateEntry<OpenDocumentsEntry>(
          ENTRY_OPEN_DOCUMENTS,
        );
        if (cancelled) {
          return;
        }
        const paths = openEntry?.openPaths ?? [];
        if (paths.length === 0) {
          return;
        }

        const nextDocs: EditorDocument[] = [];
        const nextHistories: Record<string, GraphHistory> = {};
        const nextNotationHistories: Record<string, NotationHistory> = {};
        const nextPlayheads: Record<string, number> = {};
        const warnings: string[] = [];

        for (const path of paths) {
          try {
            const dto = await invoke<DocumentSummaryDto>("open_document_file", {
              path,
            });
            const material = materializeOpenedDocument(dto);
            nextDocs.push(material.doc);
            if (material.graphHistory) {
              nextHistories[material.doc.id] = material.graphHistory;
            }
            if (material.notationHistory) {
              nextNotationHistories[material.doc.id] = material.notationHistory;
            }
            if (material.playhead !== undefined) {
              nextPlayheads[material.doc.id] = material.playhead;
            }
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            warnings.push(`${path}: ${message}`);
            console.warn("user-state: skip restore open", path, err);
          }
        }

        if (cancelled || nextDocs.length === 0) {
          return;
        }

        setDocuments(nextDocs);
        setHistories(nextHistories);
        setNotationHistories(nextNotationHistories);
        setPlayheads(nextPlayheads);

        const preferred = openEntry?.activePath ?? null;
        const active =
          (preferred && nextDocs.find((d) => d.origin === preferred)?.id) ||
          nextDocs[0]?.id ||
          "";
        setActiveId(active);
        const skipped = warnings.length;
        setStatus(
          skipped > 0
            ? `Restored ${nextDocs.length} tab(s) · ${skipped} skipped`
            : `Restored ${nextDocs.length} tab(s)`,
        );
      } catch (err) {
        console.warn("user-state restore failed", err);
      } finally {
        if (!cancelled) {
          uiStateReadyRef.current = true;
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!uiStateReadyRef.current) {
      return;
    }
    const entry = collectOpenDocumentsEntry(
      documents,
      activeId,
      isUntitledOrigin,
    );
    void setUserStateEntry(ENTRY_OPEN_DOCUMENTS, entry).catch((err) => {
      console.warn("user-state: persist open-documents failed", err);
    });
  }, [documents, activeId]);

  useEffect(() => {
    if (!uiStateReadyRef.current) {
      return;
    }
    void setUserStateEntry(
      ENTRY_PIANO_ROLL_UI,
      pianoPrefsToEntry(pianoPrefs),
    ).catch((err) => {
      console.warn("user-state: persist piano-roll-ui failed", err);
    });
  }, [pianoPrefs]);

  const paletteTypes = useMemo(() => {
    const ids = new Set(
      listNodeDefinitions(registry).map((definition) => definition.id),
    );
    return [...ids].sort();
  }, [registry]);

  const active = useMemo(
    () => documents.find((doc) => doc.id === activeId) ?? documents[0],
    [documents, activeId],
  );

  const selectedLocalNode = useMemo(() => {
    if (!selectedNodeId || !active?.graph) {
      return null;
    }
    return active.graph.nodes[selectedNodeId] ?? null;
  }, [active?.graph, selectedNodeId]);

  const selectedIsMissing =
    selectedNodeId != null &&
    active?.graph != null &&
    !(selectedNodeId in active.graph.nodes);

  const selectedDefinition = useMemo(() => {
    if (!selectedLocalNode) {
      return undefined;
    }
    return getNodeDefinition(registry, selectedLocalNode.type);
  }, [registry, selectedLocalNode]);

  const selectedWiredPorts = useMemo(() => {
    const wired = new Set<string>();
    if (!selectedNodeId || !active?.graph) {
      return wired;
    }
    for (const edge of Object.values(active.graph.edges)) {
      if (edge.to.node === selectedNodeId) {
        wired.add(edge.to.port);
      }
    }
    return wired;
  }, [active?.graph, selectedNodeId]);

  const selectedLiteralPorts = useMemo(() => {
    if (!selectedDefinition) {
      return [] as Port[];
    }
    return Object.values(selectedDefinition.inputs).filter(
      (port) =>
        port.variadic !== true && isLiteralEditableSignalType(port.type),
    );
  }, [selectedDefinition]);

  const syncDocFromHistory = useCallback(
    (docId: string, history: GraphHistory) => {
      setHistories((prev) => ({ ...prev, [docId]: history }));
      setDocuments((prev) =>
        prev.map((doc) =>
          doc.id === docId
            ? {
                ...doc,
                graph: history.present,
                dirty: documentDirty(history, doc.origin),
                summary: summarizeGraph(history.present),
              }
            : doc,
        ),
      );
    },
    [],
  );

  const syncNotationFromHistory = useCallback(
    (docId: string, history: NotationHistory) => {
      setNotationHistories((prev) => ({ ...prev, [docId]: history }));
      setDocuments((prev) =>
        prev.map((doc) =>
          doc.id === docId
            ? {
                ...doc,
                notation: history.present,
                dirty: notationDocDirty(history, doc.origin),
                summary: summarizeNotation(history.present),
                title: history.present.title ?? doc.title,
              }
            : doc,
        ),
      );
    },
    [],
  );

  const updateDocGraph = useCallback(
    (docId: string, graph: Graph, coalesceKey?: string) => {
      const baseline =
        documentsRef.current.find((d) => d.id === docId)?.graph ??
        emptyImpGraph();
      const current = historiesRef.current[docId] ?? createHistory(baseline);
      syncDocFromHistory(docId, push(current, graph, coalesceKey));
    },
    [syncDocFromHistory],
  );

  const updateDocNotation = useCallback(
    (docId: string, notation: Notation, coalesceKey?: string) => {
      const baseline =
        documentsRef.current.find((d) => d.id === docId)?.notation ??
        emptyNotation(docId);
      const current =
        notationHistoriesRef.current[docId] ?? createNotationHistory(baseline);
      syncNotationFromHistory(
        docId,
        pushNotation(current, notation, coalesceKey),
      );
    },
    [syncNotationFromHistory],
  );

  const clearTitleCommitTimer = useCallback(() => {
    if (titleCommitTimer.current != null) {
      clearTimeout(titleCommitTimer.current);
      titleCommitTimer.current = null;
    }
  }, []);

  const commitSelectedTitle = useCallback(
    (raw: string) => {
      clearTitleCommitTimer();
      const doc = documents.find((d) => d.id === activeId) ?? documents[0];
      if (!doc?.graph || !selectedNodeId) {
        return;
      }
      if (!(selectedNodeId in doc.graph.nodes)) {
        return;
      }
      const next = setNodeTitle(doc.graph, selectedNodeId, raw);
      if (next === doc.graph) {
        return;
      }
      updateDocGraph(doc.id, next, `title:${doc.id}:${selectedNodeId}`);
    },
    [
      activeId,
      clearTitleCommitTimer,
      documents,
      selectedNodeId,
      updateDocGraph,
    ],
  );

  const scheduleTitleCommit = useCallback(
    (raw: string) => {
      clearTitleCommitTimer();
      titleCommitTimer.current = setTimeout(() => {
        titleCommitTimer.current = null;
        commitSelectedTitle(raw);
      }, 400);
    },
    [clearTitleCommitTimer, commitSelectedTitle],
  );

  const commitSelectedLiteral = useCallback(
    (portId: string, value: PrimitiveValue | undefined) => {
      const doc = documents.find((d) => d.id === activeId) ?? documents[0];
      if (!doc?.graph || !selectedNodeId) {
        return;
      }
      if (!(selectedNodeId in doc.graph.nodes)) {
        return;
      }
      const next = setNodeInputLiteral(
        doc.graph,
        selectedNodeId,
        portId,
        value,
      );
      if (next === doc.graph) {
        return;
      }
      updateDocGraph(
        doc.id,
        next,
        `literal:${doc.id}:${selectedNodeId}:${portId}`,
      );
    },
    [activeId, documents, selectedNodeId, updateDocGraph],
  );

  const onGraphSelectionChange = useCallback(
    (selection: GraphSelection) => {
      clearTitleCommitTimer();
      const nodeId =
        selection.nodeIds.length === 1 ? (selection.nodeIds[0] ?? null) : null;
      setSelectedNodeId(nodeId);
      const doc = documents.find((d) => d.id === activeId) ?? documents[0];
      const node = nodeId && doc?.graph ? doc.graph.nodes[nodeId] : undefined;
      setTitleDraft(node?.title ?? "");
    },
    [activeId, clearTitleCommitTimer, documents],
  );

  useEffect(() => {
    clearTitleCommitTimer();
    setSelectedNodeId(null);
    setTitleDraft("");
  }, [activeId, clearTitleCommitTimer]);

  useEffect(() => {
    return () => clearTitleCommitTimer();
  }, [clearTitleCommitTimer]);

  const openFile = useCallback(async () => {
    try {
      const result = await invoke<DocumentSummaryDto | null>(
        "pick_and_open_document",
      );
      if (!result) {
        setStatus("Open File… cancelled");
        return;
      }
      const existing = documents.find((d) => d.origin === result.origin);
      if (existing) {
        // Activate in the same turn so the graph pane is not 0x0 during fitView.
        setActiveId(existing.id);
        setStatus(`Activated · ${existing.title}`);
        return;
      }
      const material = materializeOpenedDocument(result);
      const opened = material.doc;
      if (material.notationHistory) {
        setNotationHistories((prev) => ({
          ...prev,
          [opened.id]: material.notationHistory!,
        }));
        setPlayheads((prev) => ({
          ...prev,
          [opened.id]: material.playhead ?? 0,
        }));
      }
      if (material.graphHistory) {
        setHistories((prev) => ({
          ...prev,
          [opened.id]: material.graphHistory!,
        }));
      }
      setDocuments((prev) => [...prev, opened]);
      setActiveId(opened.id);
      setStatus(`Opened · ${opened.title}`);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setStatus(`Open failed · ${message}`);
    }
  }, [documents]);

  const applySaveResult = useCallback((docId: string, result: SaveResult) => {
    setHistories((prev) => {
      const current = prev[docId];
      if (!current) {
        return prev;
      }
      const { [docId]: _removed, ...rest } = prev;
      return { ...rest, [result.origin]: markSaved(current) };
    });
    setNotationHistories((prev) => {
      const current = prev[docId];
      if (!current) {
        return prev;
      }
      const { [docId]: _removed, ...rest } = prev;
      return { ...rest, [result.origin]: markNotationSaved(current) };
    });
    setPlayheads((prev) => {
      if (!(docId in prev)) {
        return prev;
      }
      const { [docId]: parked, ...rest } = prev;
      return { ...rest, [result.origin]: parked ?? 0 };
    });
    setDocuments((prev) =>
      prev.map((d) =>
        d.id === docId
          ? {
              ...d,
              dirty: false,
              origin: result.origin,
              title: result.title,
              id: result.origin,
              properties: d.properties.map((row) => {
                if (row.label === "Path") {
                  return { ...row, value: result.origin };
                }
                if (row.label === "Title") {
                  return { ...row, value: result.title };
                }
                return row;
              }),
            }
          : d,
      ),
    );
    setActiveId(result.origin);
    setStatus(`Saved · ${result.title}`);
  }, []);

  const savePayloadFor = (doc: EditorDocument) => {
    if (doc.kind === "notation") {
      return {
        kind: "notation" as const,
        notation: doc.notation ?? emptyNotation(doc.id),
        path: doc.origin,
      };
    }
    return {
      kind: "imp-graph" as const,
      graph: doc.graph ?? emptyImpGraph(),
      path: doc.origin,
    };
  };

  const saveActive = useCallback(async () => {
    const doc = documents.find((d) => d.id === activeId) ?? documents[0];
    if (!doc || (doc.kind === "imp-graph" && !doc.graph) || (doc.kind === "notation" && !doc.notation)) {
      setStatus("Nothing to save");
      return;
    }
    try {
      const payload = savePayloadFor(doc);
      const needsSaveAs =
        !doc.origin ||
        doc.origin === "untitled" ||
        doc.origin.startsWith("untitled");
      const result = needsSaveAs
        ? await invoke<SaveResult | null>("pick_and_save_document", {
            payload,
          })
        : await invoke<SaveResult>("save_document_file", { payload });
      if (!result) {
        setStatus("Save cancelled");
        return;
      }
      applySaveResult(doc.id, result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setStatus(`Save failed · ${message}`);
    }
  }, [activeId, applySaveResult, documents]);

  const saveActiveAs = useCallback(async () => {
    const doc = documents.find((d) => d.id === activeId) ?? documents[0];
    if (!doc || (doc.kind === "imp-graph" && !doc.graph) || (doc.kind === "notation" && !doc.notation)) {
      setStatus("Nothing to save");
      return;
    }
    try {
      const payload = savePayloadFor(doc);
      const result = await invoke<SaveResult | null>("pick_and_save_document", {
        payload,
      });
      if (!result) {
        setStatus("Save cancelled");
        return;
      }
      applySaveResult(doc.id, result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setStatus(`Save failed · ${message}`);
    }
  }, [activeId, applySaveResult, documents]);

  const closeActive = useCallback(() => {
    setDocuments((prev) => {
      if (prev.length === 0) {
        queueMicrotask(() => setStatus("No document to close"));
        return prev;
      }
      const currentId = activeId || prev[0]?.id;
      const next = prev.filter((d) => d.id !== currentId);
      const nextActive = next[0]?.id ?? "";
      queueMicrotask(() => {
        setHistories((h) => {
          if (!currentId || !(currentId in h)) {
            return h;
          }
          const { [currentId]: _removed, ...rest } = h;
          return rest;
        });
        setNotationHistories((h) => {
          if (!currentId || !(currentId in h)) {
            return h;
          }
          const { [currentId]: _removed, ...rest } = h;
          return rest;
        });
        setPlayheads((h) => {
          if (!currentId || !(currentId in h)) {
            return h;
          }
          const { [currentId]: _removed, ...rest } = h;
          return rest;
        });
        setActiveId(nextActive);
        setStatus(next.length ? "Closed tab" : "Closed last tab");
      });
      return next;
    });
  }, [activeId]);

  const undoActive = useCallback(() => {
    if (isTextEditingTarget(document.activeElement)) {
      document.execCommand("undo");
      return;
    }
    const doc = documents.find((d) => d.id === activeId) ?? documents[0];
    if (!doc) {
      setStatus("Nothing to undo");
      return;
    }
    if (doc.kind === "notation") {
      const history = notationHistories[doc.id];
      if (!history || !canUndoNotation(history)) {
        setStatus("Nothing to undo");
        return;
      }
      syncNotationFromHistory(doc.id, undoNotation(history));
      setStatus("Undo");
      return;
    }
    const history = histories[doc.id];
    if (!history || !canUndo(history)) {
      setStatus("Nothing to undo");
      return;
    }
    const next = undo(history);
    syncDocFromHistory(doc.id, next);
    setStatus("Undo");
  }, [
    activeId,
    documents,
    histories,
    notationHistories,
    syncDocFromHistory,
    syncNotationFromHistory,
  ]);

  const redoActive = useCallback(() => {
    if (isTextEditingTarget(document.activeElement)) {
      document.execCommand("redo");
      return;
    }
    const doc = documents.find((d) => d.id === activeId) ?? documents[0];
    if (!doc) {
      setStatus("Nothing to redo");
      return;
    }
    if (doc.kind === "notation") {
      const history = notationHistories[doc.id];
      if (!history || !canRedoNotation(history)) {
        setStatus("Nothing to redo");
        return;
      }
      syncNotationFromHistory(doc.id, redoNotation(history));
      setStatus("Redo");
      return;
    }
    const history = histories[doc.id];
    if (!history || !canRedo(history)) {
      setStatus("Nothing to redo");
      return;
    }
    const next = redo(history);
    syncDocFromHistory(doc.id, next);
    setStatus("Redo");
  }, [
    activeId,
    documents,
    histories,
    notationHistories,
    syncDocFromHistory,
    syncNotationFromHistory,
  ]);

  const addNodeToActive = useCallback(
    (typeId: string) => {
      const doc = documents.find((d) => d.id === activeId) ?? documents[0];
      if (!doc || doc.kind !== "imp-graph") {
        setStatus("Open or create a graph first");
        return;
      }
      const next = addImpNode(doc.graph ?? emptyImpGraph(), typeId);
      updateDocGraph(doc.id, next);
      setStatus(`Added · ${typeId}`);
    },
    [activeId, documents, updateDocGraph],
  );

  const runExtensionCommand = useCallback(
    async (id: string) => {
      const docs = documentsRef.current;
      const doc = docs.find((d) => d.id === activeId) ?? docs[0];
      const neighborPath =
        doc && !isUntitledOrigin(doc.origin) ? doc.origin : null;

      let graphJson: string | null = null;
      if (doc?.kind === "notation" && doc.notation) {
        if (id === "play.start" || id === "play.toggle" || id === "play.stop") {
          const parked = playheadsRef.current[doc.id] ?? 0;
          graphJson = JSON.stringify({
            ...doc.notation,
            labStartBeats: parked,
          });
        }
      } else if (doc?.graph) {
        graphJson = JSON.stringify(doc.graph);
      }

      if (
        !graphJson &&
        (id === "play.start" || id === "play.toggle")
      ) {
        setStatus("Open a graph or notation document before playing");
        return;
      }

      try {
        await invoke("run_extension_command", {
          id,
          graphJson,
          neighborPath,
        });
        if (id === "play.stop") {
          setNotationPlayingId(null);
          setLivePlayheadBeats(null);
          setStatus("Playback stopped");
        } else if (id === "play.start") {
          if (doc?.kind === "notation") {
            setNotationPlayingId(doc.id);
          } else {
            setNotationPlayingId(null);
            setLivePlayheadBeats(null);
          }
          setStatus("Playback started");
        } else if (id === "play.toggle") {
          try {
            const status = await invoke<PlaybackStatusDto>(
              "get_playback_status",
            );
            // Status is post-toggle: playing means we just started.
            if (status.playing && doc?.kind === "notation") {
              setNotationPlayingId(doc.id);
            } else {
              setNotationPlayingId(null);
              setLivePlayheadBeats(null);
            }
          } catch {
            if (doc?.kind === "notation") {
              setNotationPlayingId(doc.id);
            } else {
              setNotationPlayingId(null);
              setLivePlayheadBeats(null);
            }
          }
          setStatus("Playback toggled");
        } else {
          setStatus(`Ran · ${id}`);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setStatus(`Command failed · ${id}: ${message}`);
      }
    },
    [activeId],
  );

  const dispatchCommand = useCallback(
    (id: string) => {
      switch (id) {
        case CMD_DOCUMENT_OPEN_FILE:
          void openFile();
          break;
        case CMD_DOCUMENT_SAVE:
          void saveActive();
          break;
        case CMD_DOCUMENT_SAVE_AS:
          void saveActiveAs();
          break;
        case CMD_DOCUMENT_NEW_GRAPH: {
          const doc = newUntitledDocument("imp-graph");
          setDocuments((prev) => [...prev, doc]);
          setHistories((prev) => ({
            ...prev,
            [doc.id]: createHistory(doc.graph ?? emptyImpGraph()),
          }));
          setActiveId(doc.id);
          setStatus(`New graph · ${doc.title}`);
          break;
        }
        case CMD_DOCUMENT_NEW_NOTATION: {
          const doc = newUntitledDocument("notation");
          setDocuments((prev) => [...prev, doc]);
          setNotationHistories((prev) => ({
            ...prev,
            [doc.id]: createNotationHistory(
              doc.notation ?? emptyNotation(doc.id),
            ),
          }));
          setPlayheads((prev) => ({ ...prev, [doc.id]: 0 }));
          setActiveId(doc.id);
          setStatus(`New notation · ${doc.title}`);
          break;
        }
        case CMD_DOCUMENT_CLOSE_ACTIVE:
          closeActive();
          break;
        case CMD_EDIT_UNDO:
          undoActive();
          break;
        case CMD_EDIT_REDO:
          redoActive();
          break;
        case CMD_VIEW_TOGGLE_COLOR_SCHEME:
          toggleColorScheme();
          break;
        case CMD_NOTATION_TOOL_DRAW:
          setPianoPrefs((p) => ({ ...p, tool: "draw" }));
          setStatus("Tool · Draw");
          break;
        case CMD_NOTATION_TOOL_SELECT:
          setPianoPrefs((p) => ({ ...p, tool: "select" }));
          setStatus("Tool · Select");
          break;
        case CMD_NOTATION_TOOL_ERASE:
          setPianoPrefs((p) => ({ ...p, tool: "erase" }));
          setStatus("Tool · Erase");
          break;
        case CMD_NOTATION_SNAP_TOGGLE:
          setPianoPrefs((p) => {
            const snapEnabled = !p.snapEnabled;
            setStatus(snapEnabled ? "Snap on" : "Snap off");
            return { ...p, snapEnabled };
          });
          break;
        default:
          if (extensionCommandIdsRef.current.has(id)) {
            void runExtensionCommand(id);
          } else {
            setStatus(`Unhandled command · ${id}`);
          }
          break;
      }
    },
    [
      closeActive,
      openFile,
      redoActive,
      runExtensionCommand,
      saveActive,
      saveActiveAs,
      toggleColorScheme,
      undoActive,
    ],
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void listen<EditorCommandPayload>(EDITOR_COMMAND_EVENT, (event) => {
      if (!cancelled) {
        dispatchCommand(event.payload.id);
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [dispatchCommand]);

  // Keydown owns chord → command resolve (bindings table). Menu clicks / OS
  // accelerators still emit editor://command for the same ids.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) {
        return;
      }
      if (isTextEditingTarget(document.activeElement)) {
        return;
      }
      const command = resolveKeyEvent(event, inputBindingsRef.current);
      if (!command) {
        return;
      }
      event.preventDefault();
      dispatchCommand(command);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [dispatchCommand]);

  // Notation playhead follows the Lab playback clock; snaps back when stopped.
  useEffect(() => {
    if (!notationPlayingId) {
      setLivePlayheadBeats(null);
      return;
    }
    let cancelled = false;
    const tick = async () => {
      try {
        const status = await invoke<PlaybackStatusDto>("get_playback_status");
        if (cancelled) {
          return;
        }
        const doc = documentsRef.current.find(
          (d) => d.id === notationPlayingId,
        );
        if (!doc?.notation) {
          return;
        }
        if (!status.playing) {
          setNotationPlayingId(null);
          setLivePlayheadBeats(null);
          return;
        }
        setLivePlayheadBeats(
          secondsToBeats(status.positionSecs, doc.notation.tempoBpm),
        );
      } catch {
        // Pack may be absent; ignore tick errors.
      }
    };
    const handle = window.setInterval(() => {
      void tick();
    }, 33);
    return () => {
      cancelled = true;
      window.clearInterval(handle);
    };
  }, [notationPlayingId]);

  return (
    <AppShell
      header={{ height: 40 }}
      footer={{ height: 28 }}
      aside={{ width: 280, breakpoint: "sm" }}
      padding={0}
      className={classes.shell}
    >
      <AppShell.Header className={classes.header}>
        <Group h="100%" px="sm" justify="space-between" gap="sm" wrap="nowrap">
          <Group gap="xs" wrap="nowrap">
            <Text size="sm" fw={600}>
              Imp Lab
            </Text>
            <Badge size="xs" variant="light" color="gray">
              Tauri experiment
            </Badge>
          </Group>
          <Group gap={6} wrap="nowrap">
            <SegmentedControl
              size="xs"
              value={colorScheme === "auto" ? "dark" : colorScheme}
              onChange={(value) =>
                setColorScheme(value === "light" ? "light" : "dark")
              }
              data={[
                { label: "Dark", value: "dark" },
                { label: "Light", value: "light" },
              ]}
            />
            <Button
              variant="default"
              onClick={() => dispatchCommand(CMD_DOCUMENT_OPEN_FILE)}
            >
              Open…
            </Button>
            <Button
              variant="default"
              onClick={() => dispatchCommand(CMD_DOCUMENT_SAVE)}
            >
              Save
            </Button>
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Aside className={classes.aside} p="sm">
        <Stack gap="xs">
          {active?.kind !== "notation" ? (
            <>
              <Title order={6}>Add node</Title>
              <Group gap={6}>
                {paletteTypes.map((typeId) => (
                  <Button
                    key={typeId}
                    size="compact-xs"
                    variant="light"
                    onClick={() => addNodeToActive(typeId)}
                  >
                    {typeId}
                  </Button>
                ))}
              </Group>
            </>
          ) : null}
          <Title order={6}>Properties</Title>
          {!active ? (
            <Text size="xs" c="dimmed">
              No document
            </Text>
          ) : active.kind === "notation" ? (
            <Stack gap={4}>
              <Text size="xs" c="dimmed">
                No graph selection
              </Text>
              {active.properties.map((row) => (
                <Group
                  key={row.label}
                  gap="xs"
                  justify="space-between"
                  wrap="nowrap"
                >
                  <Text size="xs" c="dimmed">
                    {row.label}
                  </Text>
                  <Text size="xs" ta="right" lineClamp={1}>
                    {row.value}
                  </Text>
                </Group>
              ))}
            </Stack>
          ) : selectedLocalNode ? (
            <Stack gap={6}>
              <TextInput
                size="xs"
                label="Title"
                value={titleDraft}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  setTitleDraft(value);
                  scheduleTitleCommit(value);
                }}
                onBlur={() => commitSelectedTitle(titleDraft)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    commitSelectedTitle(titleDraft);
                  }
                }}
              />
              <Group gap="xs" justify="space-between" wrap="nowrap">
                <Text size="xs" c="dimmed">
                  Id
                </Text>
                <Text size="xs" ta="right" lineClamp={1}>
                  {selectedLocalNode.id}
                </Text>
              </Group>
              <Group gap="xs" justify="space-between" wrap="nowrap">
                <Text size="xs" c="dimmed">
                  Type
                </Text>
                <Text size="xs" ta="right" lineClamp={1}>
                  {selectedLocalNode.type}
                </Text>
              </Group>
              {selectedLiteralPorts.length > 0 ? (
                <Text size="xs" c="dimmed" mt={4}>
                  Inputs
                </Text>
              ) : null}
              {selectedLiteralPorts.map((port) => {
                const kind = literalEditorKind(port.type);
                const wired = selectedWiredPorts.has(port.id);
                const stored = selectedLocalNode.inputs[port.id];
                const label = wired ? `${port.id} (wired)` : port.id;
                if (kind === "boolean") {
                  return (
                    <Checkbox
                      key={port.id}
                      size="xs"
                      label={label}
                      checked={stored === true}
                      onChange={(event) => {
                        commitSelectedLiteral(
                          port.id,
                          event.currentTarget.checked,
                        );
                      }}
                    />
                  );
                }
                return (
                  <TextInput
                    key={port.id}
                    size="xs"
                    label={label}
                    value={formatLiteralValue(stored)}
                    placeholder={
                      port.defaultValue !== undefined
                        ? formatLiteralValue(port.defaultValue)
                        : undefined
                    }
                    type={kind === "number" ? "number" : "text"}
                    onChange={(event) => {
                      commitSelectedLiteral(
                        port.id,
                        parseLiteralText(
                          event.currentTarget.value,
                          port.type,
                        ),
                      );
                    }}
                  />
                );
              })}
            </Stack>
          ) : selectedIsMissing ? (
            <Stack gap={6}>
              <TextInput
                size="xs"
                label="Title"
                value=""
                disabled
                placeholder="(missing)"
              />
              <Group gap="xs" justify="space-between" wrap="nowrap">
                <Text size="xs" c="dimmed">
                  Id
                </Text>
                <Text size="xs" ta="right" lineClamp={1}>
                  {selectedNodeId}
                </Text>
              </Group>
              <Group gap="xs" justify="space-between" wrap="nowrap">
                <Text size="xs" c="dimmed">
                  Type
                </Text>
                <Text size="xs" ta="right" lineClamp={1}>
                  (missing)
                </Text>
              </Group>
            </Stack>
          ) : selectedNodeId ? (
            <Text size="xs" c="dimmed">
              Select a single node
            </Text>
          ) : (
            <Stack gap={4}>
              <Text size="xs" c="dimmed">
                Select a node…
              </Text>
              {active.properties.map((row) => (
                <Group
                  key={row.label}
                  gap="xs"
                  justify="space-between"
                  wrap="nowrap"
                >
                  <Text size="xs" c="dimmed">
                    {row.label}
                  </Text>
                  <Text size="xs" ta="right" lineClamp={1}>
                    {row.value}
                  </Text>
                </Group>
              ))}
            </Stack>
          )}
        </Stack>
      </AppShell.Aside>

      <AppShell.Main className={classes.main}>
        {documents.length === 0 ? (
          <Box className={classes.emptyState}>
            <Stack gap="xs" align="center">
              <Text size="sm" c="dimmed" ta="center">
                No documents open
              </Text>
              <Text size="xs" c="dimmed" ta="center">
                File → Open File…, New graph, or New notation
              </Text>
            </Stack>
          </Box>
        ) : (
          <Tabs
            value={activeId || documents[0]?.id}
            onChange={(value) => value && setActiveId(value)}
            className={classes.tabs}
          >
            <Tabs.List className={classes.tabList}>
              {documents.map((doc) => (
                <Tabs.Tab key={doc.id} value={doc.id}>
                  {doc.dirty ? `${doc.title} *` : doc.title}
                </Tabs.Tab>
              ))}
            </Tabs.List>

            {documents.map((doc) => (
              <Tabs.Panel key={doc.id} value={doc.id} className={classes.panel}>
                <Box p="md" className={classes.content}>
                  <Group gap="xs" mb="sm">
                    <Badge size="sm" variant="outline">
                      {doc.kind}
                    </Badge>
                    <Text size="xs" c="dimmed" lineClamp={1}>
                      {doc.origin}
                    </Text>
                  </Group>
                  <Title order={4} mb="xs">
                    {doc.title}
                  </Title>
                  <Text size="sm">{doc.summary}</Text>
                  <Box className={classes.canvas}>
                    {doc.kind === "notation" ? (
                      <PianoRollPanel
                        notation={doc.notation ?? emptyNotation(doc.id)}
                        playheadBeats={
                          notationPlayingId === doc.id &&
                          livePlayheadBeats != null
                            ? livePlayheadBeats
                            : (playheads[doc.id] ?? 0)
                        }
                        prefs={pianoPrefs}
                        onPrefsChange={setPianoPrefs}
                        onNotationChange={(notation, coalesceKey) =>
                          updateDocNotation(doc.id, notation, coalesceKey)
                        }
                        onPlayheadChange={(beats) =>
                          setPlayheads((prev) => ({
                            ...prev,
                            [doc.id]: beats,
                          }))
                        }
                      />
                    ) : (
                      <GraphErrorBoundary resetKey={doc.id}>
                        <LabGraphPanel
                          graph={doc.graph ?? emptyImpGraph()}
                          registry={registry}
                          fitViewKey={doc.id}
                          onGraphChange={(graph) =>
                            updateDocGraph(doc.id, graph)
                          }
                          onSelectionChange={
                            doc.id === (activeId || documents[0]?.id)
                              ? onGraphSelectionChange
                              : undefined
                          }
                        />
                      </GraphErrorBoundary>
                    )}
                  </Box>
                </Box>
              </Tabs.Panel>
            ))}
          </Tabs>
        )}
      </AppShell.Main>

      <AppShell.Footer className={classes.footer}>
        <Group h="100%" px="sm" justify="space-between" wrap="nowrap">
          <Text size="xs" lineClamp={1}>
            {status}
          </Text>
          <Text size="xs" c="dimmed">
            {active?.dirty ? "dirty" : "saved"}
          </Text>
        </Group>
      </AppShell.Footer>
    </AppShell>
  );
}
