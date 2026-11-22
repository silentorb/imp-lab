/** Document tab state for Imp Lab (Imp graphs + Aura notation stored as Tome records). */

import type { Graph } from "imp-core-types";
import {
  emptyNotation,
  summarizeNotation,
  type Notation,
} from "./piano-roll";

export type DocumentKind = "imp-graph" | "notation";

export type DocumentProperty = { label: string; value: string };

/** Where a saved document lives: a Tome node in one of the project's data stores. */
export type RecordRef = {
  nodeId: string;
  corpusId: string;
  /** Content root of the owning store. */
  contentDir: string;
};

export type EditorDocument = {
  /** Node id once saved; a session-local UUID before the first save. */
  id: string;
  title: string;
  kind: DocumentKind;
  /** `null` until the document is first saved to a store. */
  record: RecordRef | null;
  dirty: boolean;
  summary: string;
  properties: DocumentProperty[];
  /** Bare Imp graph for `imp-graph` docs. */
  graph?: Graph;
  /** Aura notation tip for `notation` docs. */
  notation?: Notation;
};

/** Host DTO from `record_open`. */
export type DocumentSummaryDto = {
  id: string;
  title: string;
  kind: DocumentKind;
  record: RecordRef;
  summary: string;
  properties: DocumentProperty[];
  graph?: Graph;
  notation?: Notation;
};

export function documentFromDto(dto: DocumentSummaryDto): EditorDocument {
  return { ...dto, dirty: false };
}

export function isUnsaved(doc: Pick<EditorDocument, "record">): boolean {
  return doc.record === null;
}

export function emptyImpGraph(): Graph {
  return { nodes: {}, edges: {} };
}

export function newUntitledDocument(
  kind: DocumentKind = "imp-graph",
): EditorDocument {
  const id = crypto.randomUUID();
  if (kind === "notation") {
    const notation = emptyNotation(id);
    const title = "Untitled notation";
    return {
      id,
      title,
      kind,
      record: null,
      dirty: true,
      summary: summarizeNotation(notation),
      properties: [
        { label: "Type", value: kind },
        { label: "Title", value: title },
        { label: "Store", value: "Not saved yet" },
      ],
      notation,
    };
  }

  const title = "Untitled graph";
  return {
    id,
    title,
    kind,
    record: null,
    dirty: true,
    summary: "Untitled Imp graph · empty",
    properties: [
      { label: "Type", value: kind },
      { label: "Title", value: title },
      { label: "Store", value: "Not saved yet" },
    ],
    graph: emptyImpGraph(),
  };
}
