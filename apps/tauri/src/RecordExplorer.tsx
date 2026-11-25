/** Left panel: the project's data stores and the lab records in them. */

import { useMemo, useState, type RefObject } from "react";
import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Group,
  Menu,
  Modal,
  ScrollArea,
  SegmentedControl,
  Stack,
  Text,
  TextInput,
  Tooltip,
  UnstyledButton,
} from "@mantine/core";
import type { NodeTypeInfo } from "./documents";
import {
  FILTER_ALL,
  FILTER_OPENABLE,
  type ExplorerTypeFilter,
  type ProjectInfo,
  type RecordSummary,
  type StoreInfo,
} from "./records";
import classes from "./RecordExplorer.module.css";

export type RecordExplorerProps = {
  project: ProjectInfo | null;
  records: RecordSummary[];
  /** Node types Lab can open (Imp graphs first, then extension editors). */
  nodeTypes: NodeTypeInfo[];
  typeFilter: ExplorerTypeFilter;
  onTypeFilterChange: (filter: ExplorerTypeFilter) => void;
  text: string;
  onTextChange: (text: string) => void;
  filterRef: RefObject<HTMLInputElement | null>;
  /** Node ids open in tabs (highlighted) and the active one. */
  openNodeIds: Set<string>;
  activeNodeId: string | null;
  onOpen: (record: RecordSummary) => void;
  onNewInStore: (storeId: string, nodeType: string) => void;
  onRename: (nodeId: string, title: string) => void;
  onArchive: (nodeId: string) => void;
  onDuplicate: (record: RecordSummary) => void;
  onOpenProject: () => void;
  onNewProject: () => void;
  onManageStores: () => void;
};

function StoreHeader({
  store,
  count,
  nodeTypes,
  onNew,
}: {
  store: StoreInfo;
  count: number;
  nodeTypes: NodeTypeInfo[];
  onNew: (nodeType: string) => void;
}) {
  const readonly = store.access === "readonly";
  return (
    <Group gap={4} wrap="nowrap" className={classes.storeHeader}>
      <Tooltip label={store.contentPath} openDelay={400} multiline maw={360}>
        <Text size="xs" fw={600} lineClamp={1} className={classes.storeName}>
          {store.label}
        </Text>
      </Tooltip>
      {store.primary ? (
        <Badge size="xs" variant="light">
          primary
        </Badge>
      ) : null}
      {readonly ? (
        <Badge size="xs" variant="outline" color="gray">
          read-only
        </Badge>
      ) : null}
      <Text size="xs" c="dimmed">
        {count}
      </Text>
      <Box style={{ flex: 1 }} />
      {readonly ? null : (
        <Menu position="bottom-end" withinPortal>
          <Menu.Target>
            <ActionIcon
              size="xs"
              variant="subtle"
              aria-label={`New record in ${store.label}`}
            >
              +
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Label>New in {store.label}</Menu.Label>
            {nodeTypes.map((t) => (
              <Menu.Item key={t.id} onClick={() => onNew(t.id)}>
                {t.label}
              </Menu.Item>
            ))}
          </Menu.Dropdown>
        </Menu>
      )}
    </Group>
  );
}

export function RecordExplorer(props: RecordExplorerProps) {
  const { project, records, nodeTypes, typeFilter, text } = props;
  const typeLabel = (id: string | null) =>
    (id && nodeTypes.find((t) => t.id === id)?.label.toLowerCase()) || "node";
  const [renaming, setRenaming] = useState<RecordSummary | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [archiving, setArchiving] = useState<RecordSummary | null>(null);

  const byStore = useMemo(() => {
    const groups = new Map<string, RecordSummary[]>();
    for (const store of project?.stores ?? []) {
      groups.set(store.id, []);
    }
    for (const record of records) {
      const key = record.corpusId ?? "";
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(record);
    }
    return groups;
  }, [project, records]);

  if (!project) {
    return (
      <Stack gap="xs" p="sm" className={classes.empty}>
        <Text size="sm" fw={600}>
          No project open
        </Text>
        <Text size="xs" c="dimmed">
          A project is a set of data stores. Graphs and other documents are saved as records in
          them.
        </Text>
        <Button size="xs" onClick={props.onNewProject}>
          New Project…
        </Button>
        <Button size="xs" variant="default" onClick={props.onOpenProject}>
          Open Project…
        </Button>
      </Stack>
    );
  }

  const storeById = new Map(project.stores.map((s) => [s.id, s]));
  const writable = (corpusId: string | null) =>
    corpusId != null && storeById.get(corpusId)?.access === "readwrite";

  return (
    <Stack gap={6} p="xs" className={classes.root}>
      <Group gap={4} wrap="nowrap">
        <Menu position="bottom-start" withinPortal>
          <Menu.Target>
            <UnstyledButton className={classes.projectButton}>
              <Text size="sm" fw={600} lineClamp={1}>
                {project.name} ▾
              </Text>
            </UnstyledButton>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Label>{project.configPath}</Menu.Label>
            <Menu.Item onClick={props.onManageStores}>Manage data stores…</Menu.Item>
            <Menu.Divider />
            <Menu.Item onClick={props.onOpenProject}>Open project…</Menu.Item>
            <Menu.Item onClick={props.onNewProject}>New project…</Menu.Item>
          </Menu.Dropdown>
        </Menu>
      </Group>
      <TextInput
        ref={props.filterRef}
        size="xs"
        placeholder="Filter records (Ctrl+P)"
        value={text}
        onChange={(event) => props.onTextChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            props.onTextChange("");
            event.currentTarget.blur();
          }
        }}
      />
      <SegmentedControl
        size="xs"
        fullWidth
        value={typeFilter}
        onChange={(value) => props.onTypeFilterChange(value)}
        data={[
          { label: "Lab", value: FILTER_OPENABLE },
          ...nodeTypes.map((t) => ({ label: t.title, value: t.id })),
          { label: "All", value: FILTER_ALL },
        ]}
      />
      <ScrollArea className={classes.scroll} type="auto" offsetScrollbars>
        <Stack gap={8}>
          {[...byStore.entries()].map(([storeId, items]) => {
            const store = storeById.get(storeId);
            return (
              <Stack key={storeId || "unknown"} gap={2}>
                {store ? (
                  <StoreHeader
                    store={store}
                    count={items.length}
                    nodeTypes={nodeTypes}
                    onNew={(nodeType) => props.onNewInStore(store.id, nodeType)}
                  />
                ) : (
                  <Text size="xs" fw={600} c="dimmed">
                    {storeId || "Unknown store"}
                  </Text>
                )}
                {items.length === 0 ? (
                  <Text size="xs" c="dimmed" pl="xs">
                    {text ? "No matches" : "Empty"}
                  </Text>
                ) : (
                  items.map((record) => {
                    const openable = record.nodeType !== null;
                    const isActive = props.activeNodeId === record.nodeId;
                    const isOpen = props.openNodeIds.has(record.nodeId);
                    return (
                      <Group
                        key={record.nodeId}
                        gap={4}
                        wrap="nowrap"
                        className={[
                          classes.row,
                          isActive ? classes.rowActive : "",
                          openable ? "" : classes.rowDisabled,
                        ].join(" ")}
                      >
                        <UnstyledButton
                          className={classes.rowButton}
                          disabled={!openable}
                          onClick={() => openable && props.onOpen(record)}
                          title={openable ? record.title : "No editor for this node's type"}
                        >
                          <Group gap={6} wrap="nowrap">
                            <Text size="xs" lineClamp={1} fw={isOpen ? 600 : 400}>
                              {record.title}
                            </Text>
                            <Text size="10px" c="dimmed">
                              {typeLabel(record.nodeType)}
                            </Text>
                          </Group>
                        </UnstyledButton>
                        {openable ? (
                          <Menu position="bottom-end" withinPortal>
                            <Menu.Target>
                              <ActionIcon
                                size="xs"
                                variant="subtle"
                                className={classes.rowMenu}
                                aria-label={`Actions for ${record.title}`}
                              >
                                ⋯
                              </ActionIcon>
                            </Menu.Target>
                            <Menu.Dropdown>
                              <Menu.Item onClick={() => props.onOpen(record)}>Open</Menu.Item>
                              <Menu.Item
                                disabled={!writable(record.corpusId)}
                                onClick={() => {
                                  setRenameDraft(record.title);
                                  setRenaming(record);
                                }}
                              >
                                Rename…
                              </Menu.Item>
                              <Menu.Item onClick={() => props.onDuplicate(record)}>
                                Duplicate to store…
                              </Menu.Item>
                              <Menu.Divider />
                              <Menu.Item
                                color="red"
                                disabled={!writable(record.corpusId)}
                                onClick={() => setArchiving(record)}
                              >
                                Archive
                              </Menu.Item>
                            </Menu.Dropdown>
                          </Menu>
                        ) : null}
                      </Group>
                    );
                  })
                )}
              </Stack>
            );
          })}
        </Stack>
      </ScrollArea>

      <Modal
        opened={renaming !== null}
        onClose={() => setRenaming(null)}
        title="Rename record"
        size="sm"
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            if (renaming && renameDraft.trim()) {
              props.onRename(renaming.nodeId, renameDraft.trim());
              setRenaming(null);
            }
          }}
        >
          <Stack gap="sm">
            <TextInput
              data-autofocus
              label="Title"
              value={renameDraft}
              onChange={(event) => setRenameDraft(event.currentTarget.value)}
            />
            <Group justify="flex-end">
              <Button variant="default" onClick={() => setRenaming(null)}>
                Cancel
              </Button>
              <Button type="submit" disabled={!renameDraft.trim()}>
                Rename
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>

      <Modal
        opened={archiving !== null}
        onClose={() => setArchiving(null)}
        title="Archive record"
        size="sm"
      >
        <Stack gap="sm">
          <Text size="sm">
            Archive “{archiving?.title}”? It moves to the store’s archive and disappears from
            the explorer; the file is kept.
          </Text>
          <Group justify="flex-end">
            <Button variant="default" onClick={() => setArchiving(null)}>
              Cancel
            </Button>
            <Button
              color="red"
              onClick={() => {
                if (archiving) props.onArchive(archiving.nodeId);
                setArchiving(null);
              }}
            >
              Archive
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
