/** Thin client for host-owned UI-state persistence (tabs, piano prefs). */

import { invoke } from "@tauri-apps/api/core";
import type { PianoRollUiPrefs } from "./piano-roll";

export const ENTRY_OPEN_DOCUMENTS = "open-documents";
export const ENTRY_PIANO_ROLL_UI = "piano-roll-ui";

export type OpenDocumentsEntry = {
  openPaths: string[];
  activePath?: string | null;
};

export type PianoRollUiEntry = {
  activeTool: string;
  snapEnabled: boolean;
  snapResolution: string;
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
  };
}

export function collectOpenDocumentsEntry(
  documents: Array<{ id: string; origin: string }>,
  activeId: string,
  isUntitled: (origin: string) => boolean,
): OpenDocumentsEntry {
  const openPaths = documents
    .map((d) => d.origin)
    .filter((origin) => !isUntitled(origin));
  const activeDoc = documents.find((d) => d.id === activeId) ?? documents[0];
  const activePath =
    activeDoc && !isUntitled(activeDoc.origin) ? activeDoc.origin : null;
  return {
    openPaths,
    activePath: activePath ?? undefined,
  };
}
