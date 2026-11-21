import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AppShell,
  Badge,
  Box,
  Button,
  Group,
  SegmentedControl,
  Stack,
  Tabs,
  Text,
  Title,
  useMantineColorScheme,
} from "@mantine/core";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Graph } from "imp-core-types";
import { addImpNode } from "imp-graph-view";
import { listNodeDefinitions } from "imp-registry";
import {
  CMD_DOCUMENT_CLOSE_ACTIVE,
  CMD_DOCUMENT_NEW_GRAPH,
  CMD_DOCUMENT_OPEN_FILE,
  CMD_DOCUMENT_SAVE,
  CMD_DOCUMENT_SAVE_AS,
  CMD_VIEW_TOGGLE_COLOR_SCHEME,
  EDITOR_COMMAND_EVENT,
  type EditorCommandPayload,
} from "./commands";
import {
  documentFromDto,
  emptyImpGraph,
  newUntitledDocument,
  type DocumentSummaryDto,
  type EditorDocument,
} from "./documents";
import { GraphErrorBoundary } from "./GraphErrorBoundary";
import { LabGraphPanel } from "./LabGraphPanel";
import { labRegistry } from "./registry";
import classes from "./App.module.css";

function summarizeGraph(graph: Graph): string {
  const nodeCount = Object.keys(graph.nodes).length;
  const edgeCount = Object.keys(graph.edges).length;
  return `Imp graph · ${nodeCount} nodes · ${edgeCount} edges`;
}

type SaveResult = {
  origin: string;
  title: string;
};

export default function App() {
  const { colorScheme, setColorScheme, toggleColorScheme } =
    useMantineColorScheme();
  const [documents, setDocuments] = useState<EditorDocument[]>([]);
  const [activeId, setActiveId] = useState("");
  const [status, setStatus] = useState(
    "Prototype · Open / Save bridged · Imp graphs editable",
  );

  const paletteTypes = useMemo(() => {
    const ids = new Set(
      listNodeDefinitions(labRegistry).map((definition) => definition.id),
    );
    return [...ids].sort();
  }, []);

  const active = useMemo(
    () => documents.find((doc) => doc.id === activeId) ?? documents[0],
    [documents, activeId],
  );

  const updateDocGraph = useCallback((docId: string, graph: Graph) => {
    setDocuments((prev) =>
      prev.map((doc) =>
        doc.id === docId
          ? {
              ...doc,
              graph,
              dirty: true,
              summary: summarizeGraph(graph),
            }
          : doc,
      ),
    );
  }, []);

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
      setActiveId(doc.id);
      setStatus(`Opened · ${doc.title}`);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setStatus(`Open failed · ${message}`);
    }
  }, [documents]);

  const applySaveResult = useCallback((docId: string, result: SaveResult) => {
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
        setActiveId(nextActive);
        setStatus(next.length ? "Closed tab" : "Closed last tab");
      });
      return next;
    });
  }, [activeId]);

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
          setActiveId(doc.id);
          setStatus(`New graph · ${doc.title}`);
          break;
        }
        case CMD_DOCUMENT_CLOSE_ACTIVE:
          closeActive();
          break;
        case CMD_VIEW_TOGGLE_COLOR_SCHEME:
          toggleColorScheme();
          break;
        default:
          setStatus(`Unhandled command · ${id}`);
          break;
      }
    },
    [closeActive, openFile, saveActive, saveActiveAs, toggleColorScheme],
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
          {active ? (
            <Stack gap={4}>
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
          ) : (
            <Text size="xs" c="dimmed">
              No document
            </Text>
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
                        onGraphChange={(graph) => updateDocGraph(doc.id, graph)}
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
