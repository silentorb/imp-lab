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
  CMD_DOCUMENT_CLOSE_ACTIVE,
  CMD_DOCUMENT_NEW_GRAPH,
  CMD_DOCUMENT_OPEN_FILE,
  CMD_DOCUMENT_SAVE,
  CMD_DOCUMENT_SAVE_AS,
  CMD_EDIT_REDO,
  CMD_EDIT_UNDO,
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
import { getLabRegistry } from "./registry";
import classes from "./App.module.css";

type ExtensionsLoadDto = {
  warnings?: string[];
  packRoot?: string | null;
  catalogs?: {
    nodeLibraries?: unknown[];
    typeConstraintLibraries?: unknown[];
    graphLibraries?: unknown[];
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

type SaveResult = {
  origin: string;
  title: string;
};

export default function App() {
  const { colorScheme, setColorScheme, toggleColorScheme } =
    useMantineColorScheme();
  const [documents, setDocuments] = useState<EditorDocument[]>([]);
  const [histories, setHistories] = useState<Record<string, GraphHistory>>(
    {},
  );
  const documentsRef = useRef(documents);
  documentsRef.current = documents;
  const historiesRef = useRef(histories);
  historiesRef.current = histories;
  const [activeId, setActiveId] = useState("");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [titleDraft, setTitleDraft] = useState("");
  const titleCommitTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [status, setStatus] = useState(
    "Prototype · Open / Save bridged · Imp graphs editable",
  );
  const [registry, setRegistry] = useState<Registry>(() => getLabRegistry());

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
        const n = loaded.catalogs?.nodeLibraries?.length ?? 0;
        if (n > 0) {
          setStatus((prev) =>
            prev.includes("catalogs")
              ? prev
              : `${prev} · ${n} extension catalog libraries`,
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
      const doc = documentFromDto(result);
      const existing = documents.find((d) => d.origin === doc.origin);
      if (existing) {
        // Activate in the same turn so the graph pane is not 0x0 during fitView.
        setActiveId(existing.id);
        setStatus(`Activated · ${existing.title}`);
        return;
      }
      setDocuments((prev) => [...prev, doc]);
      setHistories((prev) => ({
        ...prev,
        [doc.id]: createHistory(doc.graph ?? emptyImpGraph()),
      }));
      setActiveId(doc.id);
      setStatus(`Opened · ${doc.title}`);
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

  const saveActive = useCallback(async () => {
    const doc = documents.find((d) => d.id === activeId) ?? documents[0];
    if (!doc?.graph) {
      setStatus("Nothing to save");
      return;
    }
    try {
      const payload = { graph: doc.graph, path: doc.origin };
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
    if (!doc?.graph) {
      setStatus("Nothing to save");
      return;
    }
    try {
      const payload = { graph: doc.graph, path: doc.origin };
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
    const history = histories[doc.id];
    if (!history || !canUndo(history)) {
      setStatus("Nothing to undo");
      return;
    }
    const next = undo(history);
    syncDocFromHistory(doc.id, next);
    setStatus("Undo");
  }, [activeId, documents, histories, syncDocFromHistory]);

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
    const history = histories[doc.id];
    if (!history || !canRedo(history)) {
      setStatus("Nothing to redo");
      return;
    }
    const next = redo(history);
    syncDocFromHistory(doc.id, next);
    setStatus("Redo");
  }, [activeId, documents, histories, syncDocFromHistory]);

  const addNodeToActive = useCallback(
    (typeId: string) => {
      const doc = documents.find((d) => d.id === activeId) ?? documents[0];
      if (!doc) {
        setStatus("Open or create a graph first");
        return;
      }
      const next = addImpNode(doc.graph ?? emptyImpGraph(), typeId);
      updateDocGraph(doc.id, next);
      setStatus(`Added · ${typeId}`);
    },
    [activeId, documents, updateDocGraph],
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
        default:
          setStatus(`Unhandled command · ${id}`);
          break;
      }
    },
    [
      closeActive,
      openFile,
      redoActive,
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
          <Title order={6}>Properties</Title>
          {!active ? (
            <Text size="xs" c="dimmed">
              No document
            </Text>
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
                File → Open File… or New graph
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
                    <GraphErrorBoundary resetKey={doc.id}>
                      <LabGraphPanel
                        graph={doc.graph ?? emptyImpGraph()}
                        registry={registry}
                        fitViewKey={doc.id}
                        onGraphChange={(graph) => updateDocGraph(doc.id, graph)}
                        onSelectionChange={
                          doc.id === (activeId || documents[0]?.id)
                            ? onGraphSelectionChange
                            : undefined
                        }
                      />
                    </GraphErrorBoundary>
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
