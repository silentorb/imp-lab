/** Document tab state for the Tauri shell (summaries + Imp graph body when present). */

import type { Graph } from "imp-core-types";

export type DocumentKind = "imp-graph";

export type DocumentProperty = { label: string; value: string };

export type EditorDocument = {
  id: string;
  title: string;
  kind: DocumentKind;
  /** Filesystem path or untitled label */
  origin: string;
  dirty: boolean;
  summary: string;
  properties: DocumentProperty[];
  /** Bare Imp graph for `imp-graph` docs. */
  graph?: Graph;
};

/** Host DTO from `pick_and_open_document` / `open_document_file`. */
export type DocumentSummaryDto = {
  id: string;
  title: string;
  kind: DocumentKind;
  origin: string;
  dirty: boolean;
  summary: string;
  properties: DocumentProperty[];
  graph?: Graph;
};

export function documentFromDto(dto: DocumentSummaryDto): EditorDocument {
  return { ...dto };
}

export function emptyImpGraph(): Graph {
  return { nodes: {}, edges: {} };
}

export function newUntitledDocument(kind: DocumentKind = "imp-graph"): EditorDocument {
  const id = crypto.randomUUID();
  const title = "Untitled graph";
  return {
    id,
    title,
    kind,
    origin: "untitled",
    dirty: true,
    summary: "Untitled Imp graph · empty",
    properties: [
      { label: "Id", value: id },
      { label: "Type", value: kind },
      { label: "Title", value: title },
    ],
    graph: emptyImpGraph(),
  };
}
