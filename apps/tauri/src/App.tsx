import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AppShell,
  Badge,
  Box,
  Button,
  Checkbox,
  Group,
  Modal,
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
import { impFileFromGraph, impFileGraph, parseImpFile, serializeImpFile } from "imp-file";
import {
  formatLiteralValue,
  isLiteralEditableType,
  literalEditorKind,
  parseLiteralText,
  setNodeInputLiteral,
  setNodeTitle,
  type GraphSelection,
  type ImpGraphViewHandle,
} from "imp-graph-view";
import {
  getNodeDefinition,
  type Registry,
} from "imp-registry";
import {
  resolveKeyEvent,
  type BindingDto,
} from "./bindings";
import {
  CMD_DOCUMENT_CLOSE_ACTIVE,
  CMD_DOCUMENT_DUPLICATE,
  CMD_DOCUMENT_NEW_GRAPH,
  CMD_DOCUMENT_SAVE,
  CMD_PROJECT_MANAGE_STORES,
  CMD_PROJECT_NEW,
  CMD_PROJECT_OPEN,
  CMD_RECORDS_FOCUS_FILTER,
  CMD_RECORDS_SEARCH,
  CMD_GRAPH_ADD_NODE,
  CMD_EDIT_REDO,
  CMD_EDIT_UNDO,
  CMD_PLAY_START,
  CMD_PLAY_TOGGLE,
  CMD_SETTINGS_OPEN,
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
  emptyImpGraph,
  IMP_GRAPH_NODE_TYPE,
  IMP_GRAPH_TYPE_INFO,
  isImpGraph,
  newUntitledDocument,
  summarizeGraph,
  type DocumentProperty,
  type EditorDocument,
  type NodeTypeInfo,
  type RecordDocumentDto,
  type RecordRef,
} from "./documents";
import {
  EMPTY_UI,
  importModuleSource,
  installSharedModules,
  loadUi,
  newRecordCommand,
  type LoadedEditor,
  type LoadedUi,
  type UiExtensionsDto,
} from "./extensionUi";
import { ExtensionEditorPanel, type CommandHandler } from "./ExtensionEditorPanel";
import {
  canRedoValue,
  canUndoValue,
  createValueHistory,
  isValueDirty,
  jsonEquals,
  markValueSaved,
  pushValue,
  redoValue,
  undoValue,
  type ValueHistory,
} from "./valueHistory";
import {
  defaultSaveStore,
  errorMessage,
  FILTER_ALL,
  FILTER_OPENABLE,
  projectAddStore,
  projectCurrent,
  projectOpen,
  projectPickAndCreate,
  projectPickAndOpen,
  projectPickStoreFolder,
  projectRemoveStore,
  RECORDS_CHANGED_EVENT,
  recordArchive,
  recordCreate,
  recordOpen,
  recordRename,
  recordSave,
  recordsList,
  type ExplorerTypeFilter,
  type ProjectInfo,
  type RecordPayload,
  type RecordsChangedEvent,
  type RecordSummary,
  type SaveResult,
} from "./records";
import { RecordExplorer } from "./RecordExplorer";
import { RecordSearch } from "./RecordSearch";
import {
  ManageStoresModal,
  SaveTargetModal,
  type SaveTargetRequest,
} from "./StoreDialogs";
import { GraphErrorBoundary } from "./GraphErrorBoundary";
import { LabGraphPanel } from "./LabGraphPanel";
import { getLabRegistry } from "./registry";
import {
  collectOpenDocumentsEntry,
  ENTRY_OPEN_DOCUMENTS,
  ENTRY_RECORDS_UI,
  getUserStateEntry,
  setUserStateEntry,
  type OpenDocumentsEntry,
  type RecordsUiEntry,
} from "./userState";
import { connectTopics, topicStore } from "./extensionTopics";
import {
  actionsFor,
  applyNodeEdit,
  createEditSequencer,
  editCoalesceKey,
  editsTopics,
  EMPTY_NODE_UI,
  literalOptionsFor,
  parseNodeEdit,
  type NodeUi,
} from "./nodeUi";
import { LiteralOptionsSelect, NodeActions } from "./NodeUiControls";
import { SettingsModal } from "./settings/SettingsModal";
import { StatusItems } from "./settings/StatusItems";
import {
  allSettings,
  deliverSettings,
  effectiveValues,
  EMPTY_LAYOUT,
  ENTRY_EXTENSION_SETTINGS,
  type SettingsLayout,
  type SettingValues,
  type StatusItemDecl,
} from "./settings/settingsStore";
import classes from "./App.module.css";

type ExtensionsLoadDto = {
  warnings?: string[];
  packRoot?: string | null;
  catalogs?: {
    nodeLibraries?: unknown[];
    impFiles?: unknown[];
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

function documentDirty(history: GraphHistory, record: RecordRef | null): boolean {
  return isDirty(history) || record === null;
}

function editorEquals(editor: LoadedEditor | undefined) {
  return editor?.implementation.equals ?? jsonEquals;
}

function valueDocDirty(
  history: ValueHistory<unknown>,
  record: RecordRef | null,
  editor: LoadedEditor | undefined,
): boolean {
  return isValueDirty(history, editorEquals(editor)) || record === null;
}

/** Editor component for a node type (extension node types only; Imp graphs are built in). */
function editorFor(ui: LoadedUi, nodeType: string): LoadedEditor | undefined {
  return ui.editors.find((e) => e.nodeTypeId === nodeType);
}

/** Node types Lab can open, for the explorer and search. */
function nodeTypeInfos(ui: LoadedUi): NodeTypeInfo[] {
  return [
    IMP_GRAPH_TYPE_INFO,
    ...ui.editors.map((e) => ({ id: e.nodeTypeId, label: e.label, title: e.typeTitle })),
  ];
}

/** Body to persist for a document. */
function payloadFor(ui: LoadedUi, doc: EditorDocument): RecordPayload {
  if (isImpGraph(doc)) {
    return { nodeType: IMP_GRAPH_NODE_TYPE, value: impFileFromGraph(doc.graph ?? emptyImpGraph()) };
  }
  const serialize = editorFor(ui, doc.nodeType)?.implementation.serialize;
  return { nodeType: doc.nodeType, value: serialize ? serialize(doc.value) : doc.value };
}

/** Body of a new, empty record of `nodeType`. */
function emptyPayload(ui: LoadedUi, nodeType: string): RecordPayload {
  if (nodeType === IMP_GRAPH_NODE_TYPE) {
    return { nodeType, value: impFileFromGraph(emptyImpGraph()) };
  }
  const editor = editorFor(ui, nodeType);
  if (!editor) {
    throw new Error(`no editor for node type ${nodeType}`);
  }
  const value = editor.implementation.empty();
  const serialize = editor.implementation.serialize;
  return { nodeType, value: serialize ? serialize(value) : value };
}

/** One-line summary under the tab title. */
function summarizeDoc(ui: LoadedUi, doc: EditorDocument): string {
  if (isImpGraph(doc)) {
    return doc.record || Object.keys(doc.graph?.nodes ?? {}).length > 0
      ? summarizeGraph(doc.graph ?? emptyImpGraph())
      : "Untitled Imp graph · empty";
  }
  const editor = editorFor(ui, doc.nodeType);
  return editor ? editor.implementation.summarize(doc.value) : doc.nodeType;
}

/** Properties pane rows for a document without a selection. */
function docProperties(
  ui: LoadedUi,
  doc: EditorDocument,
  storeLabel: string,
): DocumentProperty[] {
  const editor = editorFor(ui, doc.nodeType);
  const rows: DocumentProperty[] = [
    { label: "Type", value: isImpGraph(doc) ? "Imp graph" : (editor?.label ?? doc.nodeType) },
    { label: "Title", value: doc.title },
    { label: "Store", value: storeLabel },
  ];
  if (doc.record) {
    rows.push({ label: "Node id", value: doc.record.nodeId });
  }
  if (isImpGraph(doc)) {
    const graph = doc.graph ?? emptyImpGraph();
    rows.push(
      { label: "Nodes", value: String(Object.keys(graph.nodes).length) },
      { label: "Edges", value: String(Object.keys(graph.edges).length) },
    );
  } else if (editor?.implementation.properties) {
    rows.push(...editor.implementation.properties(doc.value));
  }
  return rows;
}

/** Pending store picker: what is being saved, plus the dialog request. */
type SaveTargetState = {
  request: SaveTargetRequest;
  payload: RecordPayload;
};

/** Build tab + history state for a host-opened record. */
function materializeOpenedDocument(
  ui: LoadedUi,
  dto: RecordDocumentDto,
): {
  doc: EditorDocument;
  graphHistory?: GraphHistory;
  valueHistory?: ValueHistory<unknown>;
} {
  const base = {
    id: dto.id,
    title: dto.title,
    nodeType: dto.nodeType,
    record: dto.record,
    dirty: false,
  };
  if (dto.nodeType === IMP_GRAPH_NODE_TYPE) {
    const graph = dto.value ? impFileGraph(parseImpFile(dto.value)) : emptyImpGraph();
    return { doc: { ...base, graph }, graphHistory: createHistory(graph) };
  }
  const editor = editorFor(ui, dto.nodeType);
  if (!editor) {
    throw new Error("no editor for this record's node type");
  }
  const value = editor.implementation.parse(dto.value);
  return { doc: { ...base, value }, valueHistory: createValueHistory(value) };
}

const UI_IO = {
  getUiExtensions: () => invoke<UiExtensionsDto>("get_ui_extensions"),
  readLibrary: (extensionId: string) => invoke<string>("read_ui_library", { extensionId }),
  importModule: importModuleSource,
};

/** Loads extension UI once per page; every caller awaits the same load. */
let uiLoad: Promise<LoadedUi> | null = null;
function loadExtensionUi(): Promise<LoadedUi> {
  if (!uiLoad) {
    installSharedModules();
    uiLoad = loadUi(UI_IO).catch((err: unknown) => {
      console.warn("extension UI unavailable", err);
      return EMPTY_UI;
    });
  }
  return uiLoad;
}

export default function App() {
  const { colorScheme, setColorScheme, toggleColorScheme } =
    useMantineColorScheme();
  const [documents, setDocuments] = useState<EditorDocument[]>([]);
  const [histories, setHistories] = useState<Record<string, GraphHistory>>(
    {},
  );
  const [valueHistories, setValueHistories] = useState<
    Record<string, ValueHistory<unknown>>
  >({});
  /** Extension UI: editors for node types extensions add. */
  const [ui, setUi] = useState<LoadedUi>(EMPTY_UI);
  const uiRef = useRef(ui);
  uiRef.current = ui;
  const nodeTypes = useMemo(() => nodeTypeInfos(ui), [ui]);
  /** Document id → command handler of its extension editor panel. */
  const commandHandlers = useRef(new Map<string, CommandHandler>());
  const setCommandHandler = useCallback((docId: string, handler: CommandHandler | null) => {
    if (handler) commandHandlers.current.set(docId, handler);
    else commandHandlers.current.delete(docId);
  }, []);
  const documentsRef = useRef(documents);
  documentsRef.current = documents;
  const historiesRef = useRef(histories);
  historiesRef.current = histories;
  const valueHistoriesRef = useRef(valueHistories);
  valueHistoriesRef.current = valueHistories;
  const [activeId, setActiveId] = useState("");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const titleCommitTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [status, setStatus] = useState("Imp Lab · records in Tome data stores");
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const projectRef = useRef(project);
  projectRef.current = project;
  const [records, setRecords] = useState<RecordSummary[]>([]);
  const [typeFilter, setTypeFilter] = useState<ExplorerTypeFilter>(FILTER_OPENABLE);
  const [filterText, setFilterText] = useState("");
  const filterRef = useRef<HTMLInputElement | null>(null);
  const [recordSearchOpen, setRecordSearchOpen] = useState(false);
  /** Graph view per open graph tab, for commands that act on the active canvas. */
  const graphViewHandles = useRef(new Map<string, ImpGraphViewHandle>());
  const [lastNewRecordStore, setLastNewRecordStore] = useState<string | null>(null);
  const [saveTarget, setSaveTarget] = useState<SaveTargetState | null>(null);
  const [manageStoresOpen, setManageStoresOpen] = useState(false);
  const [closePromptId, setClosePromptId] = useState<string | null>(null);
  const [registry, setRegistry] = useState<Registry>(() => getLabRegistry());
  const [extensionCommandIds, setExtensionCommandIds] = useState<Set<string>>(
    () => new Set(),
  );
  const extensionCommandIdsRef = useRef(extensionCommandIds);
  extensionCommandIdsRef.current = extensionCommandIds;

  // Extension topics (pushed state) feed status items, live setting options, and editors.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void connectTopics(
      (command) => invoke(command),
      (event, handler) => listen(event, handler),
    ).then((fn) => {
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
  }, []);

  /** Extension settings: declarations, current values, and the dialog. */
  const [settingsLayout, setSettingsLayout] = useState<SettingsLayout>(EMPTY_LAYOUT);
  const settingsLayoutRef = useRef(settingsLayout);
  settingsLayoutRef.current = settingsLayout;
  const [settingValues, setSettingValues] = useState<SettingValues>({});
  const settingValuesRef = useRef(settingValues);
  settingValuesRef.current = settingValues;
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsCategory, setSettingsCategory] = useState<string | null>(null);
  const [statusItemDecls, setStatusItemDecls] = useState<StatusItemDecl[]>([]);
  const [nodeUi, setNodeUi] = useState<NodeUi>(EMPTY_NODE_UI);
  /** Node id → the document a node action was last run from, where its edits go first. */
  const actionDocs = useRef(new Map<string, string>());

  useEffect(() => {
    let cancelled = false;
    invoke<NodeUi>("get_node_ui")
      .then((ui) => {
        if (cancelled) {
          return;
        }
        for (const warning of ui.warnings ?? []) {
          console.warn("imp-lab node UI:", warning);
        }
        setNodeUi(ui);
      })
      .catch((err) => console.warn("extension node UI unavailable", err));
    return () => {
      cancelled = true;
    };
  }, []);

  const openSettings = useCallback((categoryId: string | null) => {
    setSettingsCategory(categoryId);
    setSettingsOpen(true);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [layout, items, stored] = await Promise.all([
          invoke<SettingsLayout>("get_settings_layout"),
          invoke<StatusItemDecl[]>("get_status_items"),
          getUserStateEntry<SettingValues>(ENTRY_EXTENSION_SETTINGS),
        ]);
        if (cancelled) {
          return;
        }
        for (const warning of layout.warnings ?? []) {
          console.warn("imp-lab settings:", warning);
        }
        const values = effectiveValues(layout, stored);
        setSettingsLayout(layout);
        setStatusItemDecls(items);
        setSettingValues(values);
        await deliverSettings((command, args) => invoke(command, args), layout, values);
      } catch (err) {
        console.warn("extension settings unavailable", err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const changeSetting = useCallback((settingId: string, value: unknown) => {
    const layout = settingsLayoutRef.current;
    const values = { ...settingValuesRef.current, [settingId]: value };
    settingValuesRef.current = values;
    setSettingValues(values);
    void setUserStateEntry(ENTRY_EXTENSION_SETTINGS, values).catch((err: unknown) =>
      console.warn("saving settings failed", err),
    );
    const serviceId = allSettings(layout).find((s) => s.id === settingId)?.serviceId;
    if (serviceId) {
      void deliverSettings((command, args) => invoke(command, args), layout, values, [
        serviceId,
      ]);
    }
  }, []);

  const statusItems = (
    <StatusItems items={statusItemDecls} onOpenCategory={openSettings} />
  );

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
        // Records of extension node types need their editors before tabs are restored.
        const loadedUi = await loadExtensionUi();
        if (cancelled) {
          return;
        }
        for (const warning of loadedUi.warnings) {
          console.warn("imp-lab extension UI:", warning);
        }
        setUi(loadedUi);
        uiRef.current = loadedUi;

        const recordsUi = await getUserStateEntry<RecordsUiEntry>(ENTRY_RECORDS_UI);
        if (cancelled) {
          return;
        }
        if (recordsUi?.lastNewRecordStore) {
          setLastNewRecordStore(recordsUi.lastNewRecordStore);
        }
        const savedFilter = recordsUi?.explorerNodeType;
        if (
          savedFilter &&
          (savedFilter === FILTER_OPENABLE ||
            savedFilter === FILTER_ALL ||
            nodeTypeInfos(loadedUi).some((t) => t.id === savedFilter))
        ) {
          setTypeFilter(savedFilter);
        }

        const openEntry = await getUserStateEntry<OpenDocumentsEntry>(
          ENTRY_OPEN_DOCUMENTS,
        );
        if (cancelled) {
          return;
        }
        let opened: ProjectInfo | null = await projectCurrent().catch(() => null);
        if (!opened && openEntry?.projectPath) {
          try {
            opened = await projectOpen(openEntry.projectPath);
          } catch (err) {
            setStatus(`Could not reopen project · ${errorMessage(err)}`);
          }
        }
        if (cancelled) {
          return;
        }
        setProject(opened);
        const nodeIds = opened ? (openEntry?.openNodeIds ?? []) : [];
        if (nodeIds.length === 0) {
          if (opened) setStatus(`Opened project · ${opened.name}`);
          return;
        }

        const nextDocs: EditorDocument[] = [];
        const nextHistories: Record<string, GraphHistory> = {};
        const nextValueHistories: Record<string, ValueHistory<unknown>> = {};
        const warnings: string[] = [];

        for (const nodeId of nodeIds) {
          try {
            const dto = await recordOpen(nodeId);
            const material = materializeOpenedDocument(loadedUi, dto);
            nextDocs.push(material.doc);
            if (material.graphHistory) {
              nextHistories[material.doc.id] = material.graphHistory;
            }
            if (material.valueHistory) {
              nextValueHistories[material.doc.id] = material.valueHistory;
            }
          } catch (err) {
            warnings.push(`${nodeId}: ${errorMessage(err)}`);
            console.warn("user-state: skip restore record", nodeId, err);
          }
        }

        if (cancelled || nextDocs.length === 0) {
          return;
        }

        setDocuments(nextDocs);
        setHistories(nextHistories);
        setValueHistories(nextValueHistories);

        const preferred = openEntry?.activeNodeId ?? null;
        const active =
          (preferred && nextDocs.find((d) => d.id === preferred)?.id) ||
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
      project?.configPath ?? null,
      documents,
      activeId,
    );
    void setUserStateEntry(ENTRY_OPEN_DOCUMENTS, entry).catch((err) => {
      console.warn("user-state: persist open-documents failed", err);
    });
  }, [documents, activeId, project?.configPath]);

  useEffect(() => {
    if (!uiStateReadyRef.current) {
      return;
    }
    const entry: RecordsUiEntry = {
      lastNewRecordStore: lastNewRecordStore ?? undefined,
      explorerNodeType: typeFilter,
    };
    void setUserStateEntry(ENTRY_RECORDS_UI, entry).catch((err) => {
      console.warn("user-state: persist records-ui failed", err);
    });
  }, [typeFilter, lastNewRecordStore]);

  /** Re-query the explorer list (and store counts) for the open project. */
  const refreshRecords = useCallback(async () => {
    if (!projectRef.current) {
      setRecords([]);
      return;
    }
    try {
      const [list, info] = await Promise.all([
        recordsList({ nodeType: typeFilter, text: filterText || null }),
        projectCurrent(),
      ]);
      setRecords(list);
      if (info) setProject(info);
    } catch (err) {
      setStatus(`Listing records failed · ${errorMessage(err)}`);
    }
  }, [typeFilter, filterText]);
  const refreshRecordsRef = useRef(refreshRecords);
  refreshRecordsRef.current = refreshRecords;

  useEffect(() => {
    const handle = window.setTimeout(() => void refreshRecords(), 120);
    return () => window.clearTimeout(handle);
  }, [refreshRecords, project?.configPath]);

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

  // Selected node this document does not hold: another record's node, or an unresolved endpoint.
  const selectedIsElsewhere =
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
        port.variadic !== true && isLiteralEditableType(port.type),
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
                dirty: documentDirty(history, doc.record),
              }
            : doc,
        ),
      );
    },
    [],
  );

  const syncValueFromHistory = useCallback(
    (docId: string, history: ValueHistory<unknown>) => {
      setValueHistories((prev) => ({ ...prev, [docId]: history }));
      setDocuments((prev) =>
        prev.map((doc) =>
          doc.id === docId
            ? {
                ...doc,
                value: history.present,
                dirty: valueDocDirty(
                  history,
                  doc.record,
                  editorFor(uiRef.current, doc.nodeType),
                ),
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

  const updateDocValue = useCallback(
    (docId: string, value: unknown, coalesceKey?: string) => {
      const doc = documentsRef.current.find((d) => d.id === docId);
      if (!doc) {
        return;
      }
      const current = valueHistoriesRef.current[docId] ?? createValueHistory(doc.value);
      const equals = editorEquals(editorFor(uiRef.current, doc.nodeType));
      syncValueFromHistory(docId, pushValue(current, value, coalesceKey, equals));
    },
    [syncValueFromHistory],
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

  // Services edit nodes they acted on (a plugin GUI saving its state) through edits topics.
  // Each new edit applies to the open graph holding the node; one session is one undo step.
  useEffect(() => {
    const sequencer = createEditSequencer();
    const unsubscribes = editsTopics(nodeUi).map((topic) => {
      // An edit published before Lab subscribed is not replayed.
      const current = parseNodeEdit(topicStore.get(topic));
      if (current) {
        sequencer.accept(topic, current);
      }
      return topicStore.subscribe(topic, () => {
        const edit = parseNodeEdit(topicStore.get(topic));
        if (!edit || !sequencer.accept(topic, edit)) {
          return;
        }
        const docs = documentsRef.current;
        const preferred = actionDocs.current.get(edit.node);
        const doc =
          docs.find((d) => d.id === preferred && d.graph?.nodes[edit.node]) ??
          docs.find((d) => d.graph?.nodes[edit.node]);
        if (!doc?.graph) {
          return;
        }
        const next = applyNodeEdit(doc.graph, edit);
        if (next !== doc.graph) {
          updateDocGraph(doc.id, next, editCoalesceKey(topic, edit));
        }
      });
    });
    return () => {
      for (const unsubscribe of unsubscribes) {
        unsubscribe();
      }
    };
  }, [nodeUi, updateDocGraph]);

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

  /** Add an opened record as a tab (or activate it when already open). */
  const addOpenedDocument = useCallback((dto: RecordDocumentDto) => {
    const existing = documentsRef.current.find(
      (d) => d.record?.nodeId === dto.record.nodeId,
    );
    if (existing) {
      // Activate in the same turn so the graph pane is not 0x0 during fitView.
      setActiveId(existing.id);
      return existing;
    }
    const material = materializeOpenedDocument(uiRef.current, dto);
    const opened = material.doc;
    if (material.valueHistory) {
      setValueHistories((prev) => ({ ...prev, [opened.id]: material.valueHistory! }));
    }
    if (material.graphHistory) {
      setHistories((prev) => ({ ...prev, [opened.id]: material.graphHistory! }));
    }
    setDocuments((prev) => [...prev, opened]);
    setActiveId(opened.id);
    return opened;
  }, []);

  const openRecord = useCallback(
    async (nodeId: string) => {
      const existing = documentsRef.current.find((d) => d.record?.nodeId === nodeId);
      if (existing) {
        setActiveId(existing.id);
        return;
      }
      try {
        const opened = addOpenedDocument(await recordOpen(nodeId));
        setStatus(`Opened · ${opened.title}`);
      } catch (err) {
        setStatus(`Open failed · ${errorMessage(err)}`);
      }
    },
    [addOpenedDocument],
  );

  /** Replace a clean open tab with the record's current content (external edits). */
  const reloadOpenRecord = useCallback(async (docId: string, nodeId: string) => {
    try {
      const material = materializeOpenedDocument(uiRef.current, await recordOpen(nodeId));
      setDocuments((prev) => prev.map((d) => (d.id === docId ? material.doc : d)));
      if (material.graphHistory) {
        setHistories((prev) => ({ ...prev, [docId]: material.graphHistory! }));
      }
      if (material.valueHistory) {
        setValueHistories((prev) => ({ ...prev, [docId]: material.valueHistory! }));
      }
    } catch (err) {
      console.warn("reload record failed", nodeId, err);
    }
  }, []);

  /** Re-key a tab to its record after the first save (history / playhead follow). */
  const applySaveResult = useCallback((docId: string, result: SaveResult) => {
    const nextId = result.record.nodeId;
    const rekey = <T,>(prev: Record<string, T>, map: (v: T) => T) => {
      if (!(docId in prev)) return prev;
      const { [docId]: current, ...rest } = prev;
      return { ...rest, [nextId]: map(current as T) };
    };
    setHistories((prev) => rekey(prev, markSaved));
    setValueHistories((prev) => rekey(prev, markValueSaved));
    const storeLabel =
      projectRef.current?.stores.find((s) => s.id === result.record.corpusId)?.label ??
      result.record.corpusId;
    setDocuments((prev) =>
      prev.map((d) =>
        d.id === docId
          ? { ...d, id: nextId, dirty: false, record: result.record, title: result.title }
          : d,
      ),
    );
    setActiveId((current) => (current === docId ? nextId : current));
    setStatus(`Saved · ${result.title} · ${storeLabel}`);
  }, []);

  const activeDoc = () =>
    documentsRef.current.find((d) => d.id === activeId) ?? documentsRef.current[0];

  /** Save: overwrite the record, or ask which store a new record goes to. */
  const saveDocument = useCallback(
    async (doc: EditorDocument): Promise<boolean> => {
      if (!projectRef.current) {
        setStatus("Open or create a project to save records");
        return false;
      }
      if (doc.record) {
        try {
          applySaveResult(
            doc.id,
            await recordSave(doc.record.nodeId, payloadFor(uiRef.current, doc)),
          );
          return true;
        } catch (err) {
          setStatus(`Save failed · ${errorMessage(err)}`);
          return false;
        }
      }
      setSaveTarget({
        request: {
          mode: "save",
          docId: doc.id,
          title: doc.title,
          defaultStoreId: defaultSaveStore(projectRef.current, lastNewRecordStore),
        },
        payload: payloadFor(uiRef.current, doc),
      });
      return false;
    },
    [applySaveResult, lastNewRecordStore],
  );

  const saveActive = useCallback(async () => {
    const doc = activeDoc();
    if (!doc) {
      setStatus("Nothing to save");
      return;
    }
    await saveDocument(doc);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, saveDocument]);

  const requestDuplicate = useCallback(
    (title: string, payload: RecordPayload, docId: string) => {
      if (!projectRef.current) {
        setStatus("Open or create a project first");
        return;
      }
      setSaveTarget({
        request: {
          mode: "duplicate",
          docId,
          title: `${title} copy`,
          defaultStoreId: defaultSaveStore(projectRef.current, lastNewRecordStore),
        },
        payload,
      });
    },
    [lastNewRecordStore],
  );

  const duplicateActive = useCallback(() => {
    const doc = activeDoc();
    if (!doc) {
      setStatus("Nothing to duplicate");
      return;
    }
    requestDuplicate(doc.title, payloadFor(uiRef.current, doc), doc.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, requestDuplicate]);

  const duplicateRecord = useCallback(
    async (record: RecordSummary) => {
      try {
        const dto = await recordOpen(record.nodeId);
        requestDuplicate(dto.title, { nodeType: dto.nodeType, value: dto.value }, dto.id);
      } catch (err) {
        setStatus(`Duplicate failed · ${errorMessage(err)}`);
      }
    },
    [requestDuplicate],
  );

  const confirmSaveTarget = useCallback(
    async (title: string, storeId: string) => {
      const pending = saveTarget;
      setSaveTarget(null);
      if (!pending) return;
      try {
        const result = await recordCreate(title, storeId, pending.payload);
        setLastNewRecordStore(storeId);
        if (pending.request.mode === "save") {
          applySaveResult(pending.request.docId, result);
        } else {
          await openRecord(result.record.nodeId);
          setStatus(`Duplicated · ${result.title} · ${storeId}`);
        }
      } catch (err) {
        setStatus(`Save failed · ${errorMessage(err)}`);
      }
    },
    [applySaveResult, openRecord, saveTarget],
  );

  /** Explorer "+": create an empty record directly in a store and open it. */
  const newRecordInStore = useCallback(
    async (storeId: string, nodeType: string) => {
      const label = (
        nodeTypeInfos(uiRef.current).find((t) => t.id === nodeType)?.label ?? "record"
      ).toLowerCase();
      try {
        const result = await recordCreate(
          `Untitled ${label}`,
          storeId,
          emptyPayload(uiRef.current, nodeType),
        );
        setLastNewRecordStore(storeId);
        await openRecord(result.record.nodeId);
        setStatus(`New ${label} · ${storeId}`);
      } catch (err) {
        setStatus(`Create failed · ${errorMessage(err)}`);
      }
    },
    [openRecord],
  );

  const renameRecord = useCallback(async (nodeId: string, title: string) => {
    try {
      await recordRename(nodeId, title);
      setDocuments((prev) =>
        prev.map((d) =>
          d.record?.nodeId === nodeId ? { ...d, title } : d,
        ),
      );
      setStatus(`Renamed · ${title}`);
    } catch (err) {
      setStatus(`Rename failed · ${errorMessage(err)}`);
    }
  }, []);

  /** Drop a tab and its history / playhead. */
  const closeDocument = useCallback((docId: string) => {
    const drop = <T,>(prev: Record<string, T>) => {
      if (!(docId in prev)) return prev;
      const { [docId]: _removed, ...rest } = prev;
      return rest;
    };
    setHistories(drop);
    setValueHistories(drop);
    const remaining = documentsRef.current.filter((d) => d.id !== docId);
    setDocuments(remaining);
    setActiveId((current) => (current === docId ? (remaining[0]?.id ?? "") : current));
    setStatus(remaining.length ? "Closed tab" : "Closed last tab");
  }, []);

  const archiveRecord = useCallback(
    async (nodeId: string) => {
      try {
        await recordArchive(nodeId);
        const open = documentsRef.current.find((d) => d.record?.nodeId === nodeId);
        if (open && !open.dirty) {
          closeDocument(open.id);
        } else if (open) {
          // Keep unsaved edits: the tab becomes a new, unsaved document.
          setDocuments((prev) =>
            prev.map((d) => (d.id === open.id ? { ...d, record: null, dirty: true } : d)),
          );
        }
        setStatus("Archived record");
      } catch (err) {
        setStatus(`Archive failed · ${errorMessage(err)}`);
      }
    },
    [closeDocument],
  );

  const closeActive = useCallback(() => {
    const doc = activeDoc();
    if (!doc) {
      setStatus("No document to close");
      return;
    }
    if (doc.dirty) {
      setClosePromptId(doc.id);
      return;
    }
    closeDocument(doc.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeId, closeDocument]);

  /** Switch projects: record tabs belong to the old session, unsaved tabs are kept. */
  const activateProject = useCallback(
    async (open: () => Promise<ProjectInfo | null>) => {
      const dirtyRecords = documentsRef.current.filter((d) => d.record && d.dirty);
      if (
        dirtyRecords.length > 0 &&
        !window.confirm(
          `Discard unsaved changes in ${dirtyRecords.length} record(s) and switch project?`,
        )
      ) {
        return;
      }
      try {
        const info = await open();
        if (!info) {
          setStatus("Cancelled");
          return;
        }
        for (const doc of documentsRef.current) {
          if (doc.record) closeDocument(doc.id);
        }
        setProject(info);
        setStatus(`Opened project · ${info.name}`);
      } catch (err) {
        setStatus(`Project failed · ${errorMessage(err)}`);
      }
    },
    [closeDocument],
  );

  const addStore = useCallback(
    async (id: string, contentPath: string, access: "readwrite" | "readonly") => {
      try {
        setProject(await projectAddStore(id, contentPath, access));
        setStatus(`Added store · ${id}`);
        void refreshRecordsRef.current();
        return true;
      } catch (err) {
        setStatus(`Add store failed · ${errorMessage(err)}`);
        return false;
      }
    },
    [],
  );

  const removeStore = useCallback(
    async (id: string) => {
      const affected = documentsRef.current.filter((d) => d.record?.corpusId === id);
      if (
        affected.some((d) => d.dirty) &&
        !window.confirm(`Discard unsaved changes in records from "${id}"?`)
      ) {
        return;
      }
      try {
        setProject(await projectRemoveStore(id));
        for (const doc of affected) closeDocument(doc.id);
        setStatus(`Removed store · ${id}`);
        void refreshRecordsRef.current();
      } catch (err) {
        setStatus(`Remove store failed · ${errorMessage(err)}`);
      }
    },
    [closeDocument],
  );

  // Local writes and external edits (git pull, other editors, TS Tome) refresh the explorer;
  // clean tabs follow external edits, dirty tabs keep the user's work.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<RecordsChangedEvent>(RECORDS_CHANGED_EVENT, (event) => {
      if (cancelled) return;
      void refreshRecordsRef.current();
      const change = event.payload;
      if (change.origin !== "external") return;
      const touched = new Set(change.nodeIds);
      const deleted = new Set(change.deletedNodeIds);
      for (const doc of documentsRef.current) {
        const nodeId = doc.record?.nodeId;
        if (!nodeId) continue;
        if (deleted.has(nodeId)) {
          setStatus(`“${doc.title}” was removed outside Lab`);
          setDocuments((prev) =>
            prev.map((d) => (d.id === doc.id ? { ...d, record: null, dirty: true } : d)),
          );
        } else if (touched.has(nodeId) || change.modelChanged) {
          if (doc.dirty) {
            setStatus(`“${doc.title}” changed outside Lab · your unsaved edits are kept`);
          } else {
            void reloadOpenRecord(doc.id, nodeId);
          }
        }
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [reloadOpenRecord]);

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
    if (!isImpGraph(doc)) {
      const history = valueHistories[doc.id];
      if (!history || !canUndoValue(history)) {
        setStatus("Nothing to undo");
        return;
      }
      syncValueFromHistory(doc.id, undoValue(history));
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
    valueHistories,
    syncDocFromHistory,
    syncValueFromHistory,
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
    if (!isImpGraph(doc)) {
      const history = valueHistories[doc.id];
      if (!history || !canRedoValue(history)) {
        setStatus("Nothing to redo");
        return;
      }
      syncValueFromHistory(doc.id, redoValue(history));
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
    valueHistories,
    syncDocFromHistory,
    syncValueFromHistory,
  ]);

  /** Opens the active graph's node-type picker (the graph view also opens it on Shift+A). */
  const openAddNodeInActive = useCallback(() => {
    const docs = documentsRef.current;
    const doc = docs.find((d) => d.id === activeId) ?? docs[0];
    if (!doc || !isImpGraph(doc)) {
      setStatus("Open or create a graph first");
      return;
    }
    graphViewHandles.current.get(doc.id)?.openAddNode();
  }, [activeId]);

  const runExtensionCommand = useCallback(
    async (id: string) => {
      const docs = documentsRef.current;
      const doc = docs.find((d) => d.id === activeId) ?? docs[0];
      // Extensions get the project config, so they can look up referenced records (a graph
      // using another record) across every data store of the project.
      const neighborPath = project?.configPath ?? null;

      // The active document's value: the graph, or an extension node type's stored value.
      const graphJson = doc
        ? isImpGraph(doc)
          ? serializeImpFile(impFileFromGraph(doc.graph ?? emptyImpGraph(), { format: "imp" }))
          : JSON.stringify(payloadFor(uiRef.current, doc).value)
        : null;

      if (!graphJson && (id === CMD_PLAY_START || id === CMD_PLAY_TOGGLE)) {
        setStatus("Open a document before playing");
        return;
      }

      try {
        await invoke("run_extension_command", {
          id,
          graphJson,
          neighborPath,
        });
        if (id === "play.stop") {
          setStatus("Playback stopped");
        } else if (id === CMD_PLAY_START) {
          setStatus("Playback started");
        } else if (id === CMD_PLAY_TOGGLE) {
          setStatus("Playback toggled");
        } else {
          setStatus(`Ran · ${id}`);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setStatus(`Command failed · ${id}: ${message}`);
      }
    },
    [activeId, project],
  );

  /** New, unsaved document of an extension editor's node type. */
  const newEditorDocument = useCallback((editor: LoadedEditor) => {
    const value = editor.implementation.empty();
    const doc = newUntitledDocument(editor.nodeTypeId, editor.label, { value });
    setDocuments((prev) => [...prev, doc]);
    setValueHistories((prev) => ({ ...prev, [doc.id]: createValueHistory(value) }));
    setActiveId(doc.id);
    setStatus(`New ${editor.label.toLowerCase()} · ${doc.title}`);
  }, []);

  const dispatchCommand = useCallback(
    (id: string) => {
      // The active document's extension editor gets first refusal.
      const activeDocId =
        documentsRef.current.find((d) => d.id === activeId)?.id ?? documentsRef.current[0]?.id;
      if (activeDocId && commandHandlers.current.get(activeDocId)?.(id)) {
        return;
      }
      switch (id) {
        case CMD_PROJECT_OPEN:
          void activateProject(projectPickAndOpen);
          break;
        case CMD_PROJECT_NEW:
          void activateProject(projectPickAndCreate);
          break;
        case CMD_PROJECT_MANAGE_STORES:
          if (projectRef.current) {
            setManageStoresOpen(true);
          } else {
            setStatus("Open or create a project first");
          }
          break;
        case CMD_RECORDS_FOCUS_FILTER:
          filterRef.current?.focus();
          filterRef.current?.select();
          break;
        case CMD_RECORDS_SEARCH:
          setRecordSearchOpen(true);
          break;
        case CMD_GRAPH_ADD_NODE:
          openAddNodeInActive();
          break;
        case CMD_DOCUMENT_SAVE:
          void saveActive();
          break;
        case CMD_DOCUMENT_DUPLICATE:
          duplicateActive();
          break;
        case CMD_DOCUMENT_NEW_GRAPH: {
          const doc = newUntitledDocument(IMP_GRAPH_NODE_TYPE, "graph", {
            graph: emptyImpGraph(),
          });
          setDocuments((prev) => [...prev, doc]);
          setHistories((prev) => ({
            ...prev,
            [doc.id]: createHistory(doc.graph ?? emptyImpGraph()),
          }));
          setActiveId(doc.id);
          setStatus(`New graph · ${doc.title}`);
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
        case CMD_SETTINGS_OPEN:
          openSettings(null);
          break;
        default: {
          const editor = uiRef.current.editors.find((e) => newRecordCommand(e.id) === id);
          if (editor) {
            newEditorDocument(editor);
          } else if (extensionCommandIdsRef.current.has(id)) {
            void runExtensionCommand(id);
          } else {
            setStatus(`Unhandled command · ${id}`);
          }
          break;
        }
      }
    },
    [
      activateProject,
      activeId,
      closeActive,
      duplicateActive,
      newEditorDocument,
      openAddNodeInActive,
      openSettings,
      redoActive,
      runExtensionCommand,
      saveActive,
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

  const storeLabelOf = (doc: EditorDocument) =>
    doc.record
      ? (project?.stores.find((s) => s.id === doc.record?.corpusId)?.label ??
        doc.record.corpusId)
      : "Not saved yet";

  const propertyRows = (doc: EditorDocument) =>
    docProperties(ui, doc, storeLabelOf(doc)).map((row) => (
      <Group key={row.label} gap="xs" justify="space-between" wrap="nowrap">
        <Text size="xs" c="dimmed">
          {row.label}
        </Text>
        <Text size="xs" ta="right" lineClamp={1}>
          {row.value}
        </Text>
      </Group>
    ));

  return (
    <AppShell
      header={{ height: 40 }}
      footer={{ height: 28 }}
      navbar={{ width: 260, breakpoint: "sm" }}
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
            {project ? (
              <Text size="xs" c="dimmed" lineClamp={1}>
                {project.name}
              </Text>
            ) : null}
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
              disabled={!active}
              onClick={() => dispatchCommand(CMD_DOCUMENT_SAVE)}
            >
              Save
            </Button>
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar className={classes.navbar}>
        <RecordExplorer
          project={project}
          records={records}
          nodeTypes={nodeTypes}
          typeFilter={typeFilter}
          onTypeFilterChange={setTypeFilter}
          text={filterText}
          onTextChange={setFilterText}
          filterRef={filterRef}
          openNodeIds={
            new Set(documents.flatMap((d) => (d.record ? [d.record.nodeId] : [])))
          }
          activeNodeId={active?.record?.nodeId ?? null}
          onOpen={(record) => void openRecord(record.nodeId)}
          onNewInStore={(storeId, nodeType) => void newRecordInStore(storeId, nodeType)}
          onRename={(nodeId, title) => void renameRecord(nodeId, title)}
          onArchive={(nodeId) => void archiveRecord(nodeId)}
          onDuplicate={(record) => void duplicateRecord(record)}
          onOpenProject={() => dispatchCommand(CMD_PROJECT_OPEN)}
          onNewProject={() => dispatchCommand(CMD_PROJECT_NEW)}
          onManageStores={() => dispatchCommand(CMD_PROJECT_MANAGE_STORES)}
        />
        <RecordSearch
          open={recordSearchOpen}
          onClose={() => setRecordSearchOpen(false)}
          project={project}
          nodeTypes={nodeTypes}
          onOpen={(record) => void openRecord(record.nodeId)}
        />
      </AppShell.Navbar>

      <AppShell.Aside className={classes.aside} p="sm">
        <Stack gap="xs">
          <Title order={6}>Properties</Title>
          {!active ? (
            <Text size="xs" c="dimmed">
              No document
            </Text>
          ) : !isImpGraph(active) ? (
            <Stack gap={4}>{propertyRows(active)}</Stack>
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
                const options = literalOptionsFor(
                  nodeUi,
                  selectedLocalNode.type,
                  port.id,
                );
                if (options && !wired) {
                  return (
                    <LiteralOptionsSelect
                      key={port.id}
                      label={label}
                      topic={options.optionsTopic}
                      value={typeof stored === "string" ? stored : ""}
                      onChange={(value) =>
                        commitSelectedLiteral(
                          port.id,
                          value === "" ? undefined : value,
                        )
                      }
                    />
                  );
                }
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
              {actionsFor(nodeUi, selectedLocalNode.type).length > 0 && active.graph ? (
                <NodeActions
                  actions={actionsFor(nodeUi, selectedLocalNode.type)}
                  node={{
                    id: selectedLocalNode.id,
                    type: selectedLocalNode.type,
                    inputs: selectedLocalNode.inputs,
                  }}
                  invoke={(command, args) => invoke(command, args)}
                  onRun={() =>
                    actionDocs.current.set(selectedLocalNode.id, active.id)
                  }
                />
              ) : null}
            </Stack>
          ) : selectedIsElsewhere ? (
            <Stack gap={6}>
              <TextInput
                size="xs"
                label="Title"
                value=""
                disabled
                placeholder="(not in this graph)"
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
                  (defined elsewhere)
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
              {propertyRows(active)}
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
                {project
                  ? "Open a record from the explorer, or File → New graph"
                  : "File → New Project… or Open Project… to work with data stores"}
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
                      {nodeTypes.find((t) => t.id === doc.nodeType)?.label.toLowerCase() ??
                        doc.nodeType}
                    </Badge>
                    <Text size="xs" c="dimmed" lineClamp={1}>
                      {storeLabelOf(doc)}
                    </Text>
                  </Group>
                  <Title order={4} mb="xs">
                    {doc.title}
                  </Title>
                  <Text size="sm">{summarizeDoc(ui, doc)}</Text>
                  <Box className={classes.canvas}>
                    {!isImpGraph(doc) ? (
                      (() => {
                        const editor = editorFor(ui, doc.nodeType);
                        return editor ? (
                          <ExtensionEditorPanel
                            editor={editor}
                            docId={doc.id}
                            value={doc.value}
                            active={doc.id === (activeId || documents[0]?.id)}
                            neighborPath={project?.configPath ?? null}
                            extensionCommandIds={extensionCommandIds}
                            statusItems={statusItems}
                            setStatus={setStatus}
                            onChange={(value, coalesceKey) =>
                              updateDocValue(doc.id, value, coalesceKey)
                            }
                            setCommandHandler={setCommandHandler}
                          />
                        ) : (
                          <Text size="sm" c="dimmed">
                            No editor for this node type.
                          </Text>
                        );
                      })()
                    ) : (
                      <GraphErrorBoundary resetKey={doc.id}>
                        <LabGraphPanel
                          ref={(handle) => {
                            if (handle) graphViewHandles.current.set(doc.id, handle);
                            else graphViewHandles.current.delete(doc.id);
                          }}
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
          <Group gap="sm" wrap="nowrap">
            {statusItems}
            <Text size="xs" c="dimmed">
              {active?.dirty ? "dirty" : "saved"}
            </Text>
          </Group>
        </Group>
      </AppShell.Footer>

      <SettingsModal
        opened={settingsOpen}
        layout={settingsLayout}
        values={settingValues}
        initialCategory={settingsCategory}
        onChange={changeSetting}
        onClose={() => setSettingsOpen(false)}
      />

      <SaveTargetModal
        request={saveTarget?.request ?? null}
        project={project}
        onCancel={() => {
          setSaveTarget(null);
          setStatus("Save cancelled");
        }}
        onConfirm={(title, storeId) => void confirmSaveTarget(title, storeId)}
      />

      <ManageStoresModal
        opened={manageStoresOpen}
        project={project}
        onClose={() => setManageStoresOpen(false)}
        onPickFolder={() => projectPickStoreFolder().catch(() => null)}
        onAdd={addStore}
        onRemove={(id) => void removeStore(id)}
      />

      <Modal
        opened={closePromptId !== null}
        onClose={() => setClosePromptId(null)}
        title="Unsaved changes"
        size="sm"
      >
        <Stack gap="sm">
          <Text size="sm">
            Save changes to “
            {documents.find((d) => d.id === closePromptId)?.title ?? "document"}” before
            closing?
          </Text>
          <Group justify="flex-end">
            <Button variant="default" onClick={() => setClosePromptId(null)}>
              Cancel
            </Button>
            <Button
              variant="default"
              color="red"
              onClick={() => {
                if (closePromptId) closeDocument(closePromptId);
                setClosePromptId(null);
              }}
            >
              Discard
            </Button>
            <Button
              onClick={async () => {
                const doc = documents.find((d) => d.id === closePromptId);
                setClosePromptId(null);
                if (doc && (await saveDocument(doc))) {
                  closeDocument(doc.record?.nodeId ?? doc.id);
                }
              }}
            >
              Save
            </Button>
          </Group>
        </Stack>
      </Modal>
    </AppShell>
  );
}
