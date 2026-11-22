/** Store pickers: "Save new record" / "Duplicate to store", and "Manage data stores". */

import { useEffect, useState } from "react";
import {
  ActionIcon,
  Badge,
  Button,
  Group,
  Modal,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
} from "@mantine/core";
import type { ProjectInfo, StoreAccess } from "./records";

export type SaveTargetRequest = {
  /** `save`: first save of an unsaved doc; `duplicate`: copy into another store. */
  mode: "save" | "duplicate";
  docId: string;
  title: string;
  defaultStoreId: string | null;
};

export function SaveTargetModal({
  request,
  project,
  onCancel,
  onConfirm,
}: {
  request: SaveTargetRequest | null;
  project: ProjectInfo | null;
  onCancel: () => void;
  onConfirm: (title: string, storeId: string) => void;
}) {
  const [title, setTitle] = useState("");
  const [storeId, setStoreId] = useState<string | null>(null);

  useEffect(() => {
    if (request) {
      setTitle(request.title);
      setStoreId(request.defaultStoreId);
    }
  }, [request]);

  const stores = project?.stores ?? [];
  const data = stores.map((s) => ({
    value: s.id,
    label: `${s.label}${s.primary ? " · primary" : ""}${s.access === "readonly" ? " · read-only" : ""}`,
    disabled: s.access === "readonly",
  }));
  const chosen = stores.find((s) => s.id === storeId);
  const canSave = Boolean(title.trim() && chosen && chosen.access === "readwrite");

  return (
    <Modal
      opened={request !== null}
      onClose={onCancel}
      title={request?.mode === "duplicate" ? "Duplicate to store" : "Save new record"}
      size="md"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (canSave && storeId) onConfirm(title.trim(), storeId);
        }}
      >
        <Stack gap="sm">
          <TextInput
            data-autofocus
            label="Title"
            value={title}
            onChange={(event) => setTitle(event.currentTarget.value)}
          />
          <Select
            label="Data store"
            description="Where the record is written. Read-only stores can't take new records."
            data={data}
            value={storeId}
            onChange={setStoreId}
            allowDeselect={false}
            comboboxProps={{ withinPortal: true }}
          />
          {chosen ? (
            <Text size="xs" c="dimmed" lineClamp={2}>
              {chosen.contentPath}
            </Text>
          ) : null}
          {data.every((d) => d.disabled) ? (
            <Text size="xs" c="red">
              This project has no writable store. Add one in Manage data stores.
            </Text>
          ) : null}
          <Group justify="flex-end">
            <Button variant="default" onClick={onCancel}>
              Cancel
            </Button>
            <Button type="submit" disabled={!canSave}>
              {request?.mode === "duplicate" ? "Duplicate" : "Save"}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}

export function ManageStoresModal({
  opened,
  project,
  onClose,
  onPickFolder,
  onAdd,
  onRemove,
}: {
  opened: boolean;
  project: ProjectInfo | null;
  onClose: () => void;
  onPickFolder: () => Promise<string | null>;
  onAdd: (id: string, contentPath: string, access: StoreAccess) => Promise<boolean>;
  onRemove: (id: string) => void;
}) {
  const [id, setId] = useState("");
  const [path, setPath] = useState("");
  const [access, setAccess] = useState<StoreAccess>("readwrite");
  const [busy, setBusy] = useState(false);
  const idValid = /^[A-Za-z0-9_-]+$/.test(id);
  const idTaken = (project?.stores ?? []).some((s) => s.id === id);

  return (
    <Modal opened={opened} onClose={onClose} title="Data stores" size="lg">
      <Stack gap="md">
        <Text size="xs" c="dimmed">
          Each store is a Tome content folder. Records live in exactly one store; the primary
          store also holds the shared model (type tables). Removing a store only drops it from
          the project; its files stay on disk.
        </Text>
        <Table withRowBorders verticalSpacing={4} fz="xs">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Store</Table.Th>
              <Table.Th>Folder</Table.Th>
              <Table.Th>Records</Table.Th>
              <Table.Th />
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {(project?.stores ?? []).map((store) => (
              <Table.Tr key={store.id}>
                <Table.Td>
                  <Group gap={4} wrap="nowrap">
                    <Text size="xs" fw={600}>
                      {store.id}
                    </Text>
                    {store.primary ? (
                      <Badge size="xs" variant="light">
                        primary
                      </Badge>
                    ) : null}
                    {store.access === "readonly" ? (
                      <Badge size="xs" variant="outline" color="gray">
                        read-only
                      </Badge>
                    ) : null}
                  </Group>
                </Table.Td>
                <Table.Td>
                  <Text size="xs" lineClamp={1} title={store.contentPath}>
                    {store.contentPath}
                  </Text>
                </Table.Td>
                <Table.Td>{store.recordCount}</Table.Td>
                <Table.Td>
                  {store.primary ? null : (
                    <ActionIcon
                      size="sm"
                      variant="subtle"
                      color="red"
                      aria-label={`Remove ${store.id}`}
                      onClick={() => onRemove(store.id)}
                    >
                      ×
                    </ActionIcon>
                  )}
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>

        <form
          onSubmit={async (event) => {
            event.preventDefault();
            if (!idValid || idTaken || !path.trim()) return;
            setBusy(true);
            const ok = await onAdd(id, path.trim(), access);
            setBusy(false);
            if (ok) {
              setId("");
              setPath("");
              setAccess("readwrite");
            }
          }}
        >
          <Stack gap="xs">
            <Text size="sm" fw={600}>
              Add a store
            </Text>
            <Group gap="xs" align="flex-end" wrap="nowrap">
              <TextInput
                size="xs"
                label="Id"
                placeholder="samples"
                value={id}
                onChange={(event) => setId(event.currentTarget.value)}
                error={id && !idValid ? "letters, digits, - or _" : idTaken ? "already used" : null}
                w={140}
              />
              <TextInput
                size="xs"
                label="Folder"
                placeholder="Content folder (created if missing)"
                value={path}
                onChange={(event) => setPath(event.currentTarget.value)}
                style={{ flex: 1 }}
              />
              <Button
                size="xs"
                variant="default"
                onClick={async () => {
                  const picked = await onPickFolder();
                  if (picked) setPath(picked);
                }}
              >
                Browse…
              </Button>
            </Group>
            <Group gap="xs" justify="space-between">
              <Select
                size="xs"
                w={160}
                data={[
                  { value: "readwrite", label: "Read / write" },
                  { value: "readonly", label: "Read-only" },
                ]}
                value={access}
                onChange={(value) => setAccess((value as StoreAccess) ?? "readwrite")}
                allowDeselect={false}
                comboboxProps={{ withinPortal: true }}
              />
              <Button
                size="xs"
                type="submit"
                loading={busy}
                disabled={!idValid || idTaken || !path.trim()}
              >
                Add store
              </Button>
            </Group>
          </Stack>
        </form>
      </Stack>
    </Modal>
  );
}
