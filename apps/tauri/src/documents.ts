/** Document tab state for Imp Lab (Imp graphs + Aura notation). */

import type { Graph } from "imp-core-types";
import {
  emptyNotation,
  summarizeNotation,
  type Notation,
} from "./piano-roll";

export type DocumentKind = "imp-graph" | "notation";

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
  /** Aura notation tip for `notation` docs. */
  notation?: Notation;
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
  notation?: Notation;
};

export function documentFromDto(dto: DocumentSummaryDto): EditorDocument {
  return { ...dto };
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
      origin: "untitled",
      dirty: true,
      summary: summarizeNotation(notation),
      properties: [
        { label: "Id", value: id },
        { label: "Type", value: kind },
        { label: "Title", value: title },
      ],
      notation,
    };
  }

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
