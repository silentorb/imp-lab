/** Typed client for the host's project / record commands (Tome data sources). */

import { invoke } from "@tauri-apps/api/core";
import type { DocumentKind, DocumentSummaryDto, RecordRef } from "./documents";

/** Host event fired after local writes and external file edits. */
export const RECORDS_CHANGED_EVENT = "records://changed";

export type StoreAccess = "readwrite" | "readonly";

export type StoreInfo = {
  id: string;
  label: string;
  access: StoreAccess;
  contentPath: string;
  primary: boolean;
  recordCount: number;
};

export type ProjectInfo = {
  configPath: string;
  name: string;
  stores: StoreInfo[];
};

export type RecordKind = DocumentKind | "other";

export type RecordSummary = {
  nodeId: string;
  title: string;
  kind: RecordKind;
  corpusId: string | null;
  modifiedAt: string | null;
};

/** `lab` = graphs and notations; `all` also lists other Tome nodes (read-only). */
export type ExplorerKindFilter = "lab" | DocumentKind | "all";

export type RecordsFilter = {
  kind?: ExplorerKindFilter;
  corpusId?: string | null;
  text?: string | null;
};

export type RecordPayload = {
  kind: DocumentKind;
  graph?: unknown;
  notation?: unknown;
};

export type SaveResult = { record: RecordRef; title: string };

export type RecordsChangedEvent = {
  origin: "local" | "external";
  nodeIds: string[];
  deletedNodeIds: string[];
  modelChanged: boolean;
};

/** Host errors are `code: message`. */
export function errorCode(err: unknown): string {
  const text = String(err);
  const idx = text.indexOf(":");
  return idx > 0 ? text.slice(0, idx).trim() : "";
}

export function errorMessage(err: unknown): string {
  const text = String(err);
  const idx = text.indexOf(":");
  return idx > 0 ? text.slice(idx + 1).trim() : text;
}

export const projectCurrent = () => invoke<ProjectInfo | null>("project_current");
export const projectOpen = (path: string) => invoke<ProjectInfo>("project_open", { path });
export const projectPickAndOpen = () => invoke<ProjectInfo | null>("project_pick_and_open");
export const projectPickAndCreate = () => invoke<ProjectInfo | null>("project_pick_and_create");
export const projectPickStoreFolder = () => invoke<string | null>("project_pick_store_folder");
export const projectAddStore = (id: string, contentPath: string, access: StoreAccess) =>
  invoke<ProjectInfo>("project_add_store", { id, contentPath, access });
export const projectRemoveStore = (id: string) =>
  invoke<ProjectInfo>("project_remove_store", { id });

export const recordsList = (filter: RecordsFilter) =>
  invoke<RecordSummary[]>("records_list", { filter });
export const recordOpen = (nodeId: string) =>
  invoke<DocumentSummaryDto>("record_open", { nodeId });
export const recordCreate = (title: string, corpusId: string, payload: RecordPayload) =>
  invoke<SaveResult>("record_create", { title, corpusId, payload });
export const recordSave = (nodeId: string, payload: RecordPayload) =>
  invoke<SaveResult>("record_save", { nodeId, payload });
export const recordRename = (nodeId: string, title: string) =>
  invoke<void>("record_rename", { nodeId, title });
export const recordArchive = (nodeId: string) => invoke<void>("record_archive", { nodeId });

/** Writable stores, primary first. */
export function writableStores(project: ProjectInfo | null): StoreInfo[] {
  return (project?.stores ?? []).filter((s) => s.access === "readwrite");
}

/** Default save target: last used if still writable, else primary, else first writable. */
export function defaultSaveStore(
  project: ProjectInfo | null,
  lastUsed: string | null | undefined,
): string | null {
  const writable = writableStores(project);
  if (lastUsed && writable.some((s) => s.id === lastUsed)) return lastUsed;
  return (writable.find((s) => s.primary) ?? writable[0])?.id ?? null;
}
