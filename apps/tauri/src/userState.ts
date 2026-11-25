/** Thin client for host-owned UI-state persistence (tabs, explorer prefs, component prefs). */

import { invoke } from "@tauri-apps/api/core";

export const ENTRY_OPEN_DOCUMENTS = "open-documents";
export const ENTRY_RECORDS_UI = "records-ui";

/** Open project and record tabs (pre-project `openPaths` entries are ignored). */
export type OpenDocumentsEntry = {
  projectPath?: string | null;
  openNodeIds?: string[];
  activeNodeId?: string | null;
};

/** Record explorer prefs. */
export type RecordsUiEntry = {
  lastNewRecordStore?: string | null;
  /** A node type id, `openable` or `all`. */
  explorerNodeType?: string | null;
};

export async function getUserStateEntry<T>(
  entryId: string,
): Promise<T | null> {
  return invoke<T | null>("get_user_state_entry", { entryId });
}

export async function setUserStateEntry(
  entryId: string,
  value: unknown,
): Promise<void> {
  await invoke("set_user_state_entry", { entryId, value });
}

export function collectOpenDocumentsEntry(
  projectPath: string | null,
  documents: Array<{ id: string; record: { nodeId: string } | null }>,
  activeId: string,
): OpenDocumentsEntry {
  const openNodeIds = documents.flatMap((d) => (d.record ? [d.record.nodeId] : []));
  const activeDoc = documents.find((d) => d.id === activeId) ?? documents[0];
  return {
    projectPath: projectPath ?? undefined,
    openNodeIds,
    activeNodeId: activeDoc?.record?.nodeId ?? undefined,
  };
}
