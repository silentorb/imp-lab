/** Thin client for host-owned UI-state persistence (tabs, piano prefs). */

import { invoke } from "@tauri-apps/api/core";
import type { PianoRollUiPrefs } from "./piano-roll";

export const ENTRY_OPEN_DOCUMENTS = "open-documents";
export const ENTRY_PIANO_ROLL_UI = "piano-roll-ui";
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
  explorerKindFilter?: string | null;
};

export type PianoRollUiEntry = {
  activeTool: string;
  snapEnabled: boolean;
  snapResolution: string;
  /** Absent in entries saved before audition existed. */
  audition?: boolean;
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

export function pianoPrefsToEntry(prefs: PianoRollUiPrefs): PianoRollUiEntry {
  return {
    activeTool: prefs.tool,
    snapEnabled: prefs.snapEnabled,
    snapResolution: prefs.snapResolution,
    audition: prefs.audition,
  };
}

export function pianoEntryToPrefs(
  entry: PianoRollUiEntry | null | undefined,
  fallback: PianoRollUiPrefs,
): PianoRollUiPrefs {
  if (!entry) {
    return fallback;
  }
  const tool =
    entry.activeTool === "draw" ||
    entry.activeTool === "select" ||
    entry.activeTool === "erase"
      ? entry.activeTool
      : fallback.tool;
  const snapResolution =
    entry.snapResolution === "1/4" ||
    entry.snapResolution === "1/8" ||
    entry.snapResolution === "1/16" ||
    entry.snapResolution === "1/32"
      ? entry.snapResolution
      : fallback.snapResolution;
  return {
    tool,
    snapEnabled: Boolean(entry.snapEnabled),
    snapResolution,
    audition:
      typeof entry.audition === "boolean" ? entry.audition : fallback.audition,
  };
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
