/** Typed client for the host's project / record commands (Tome data sources). */

import { invoke } from "@tauri-apps/api/core";
import type { Node as ImpNode } from "imp-core-types";
import type { RecordDocumentDto, RecordRef } from "./documents";

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

export type RecordSummary = {
  nodeId: string;
  title: string;
  /** Node type Lab opens it as; `null` for nodes Lab lists but cannot open. */
  nodeType: string | null;
  corpusId: string | null;
  modifiedAt: string | null;
};

/** Explorer filter: every node type Lab can open. */
export const FILTER_OPENABLE = "openable";
/** Explorer filter: every Tome node (others are listed, not openable). */
export const FILTER_ALL = "all";

/** A node type id, {@link FILTER_OPENABLE} or {@link FILTER_ALL}. */
export type ExplorerTypeFilter = string;

export type RecordsFilter = {
  nodeType?: ExplorerTypeFilter;
  corpusId?: string | null;
  text?: string | null;
};

export type RecordPayload = {
  nodeType: string;
  value: unknown;
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
  invoke<RecordDocumentDto>("record_open", { nodeId });
export const recordCreate = (title: string, corpusId: string, payload: RecordPayload) =>
  invoke<SaveResult>("record_create", { title, corpusId, payload });
export const recordSave = (nodeId: string, payload: RecordPayload) =>
  invoke<SaveResult>("record_save", { nodeId, payload });

/**
 * Imp nodes a graph references but does not hold, resolved from the project's Tome stores.
 * `nodeTypes` are the registered node type ids (a record is a node when its type hub is one).
 */
export const resolveRecordNodes = (ids: string[], nodeTypes: string[]) =>
  invoke<ImpNode[]>("records_resolve_nodes", { ids, nodeTypes });
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
