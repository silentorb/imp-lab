/** Document tab state for Imp Lab: one open Tome record (an Imp graph, or a node type an extension edits). */

import type { Graph } from "imp-core-types";

/** Type-table id of Imp graph records (built in). */
export const IMP_GRAPH_NODE_TYPE = "01M80M9GRAPH00000000000001";

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
  /** Tome node type (type-table id). */
  nodeType: string;
  /** `null` until the document is first saved to a store. */
  record: RecordRef | null;
  dirty: boolean;
  /** Bare Imp graph for Imp graph documents. */
  graph?: Graph;
  /** Editor value for documents of an extension node type. */
  value?: unknown;
};

/** Host DTO from `record_open`. */
export type RecordDocumentDto = {
  id: string;
  title: string;
  nodeType: string;
  record: RecordRef;
  value: unknown;
};

export function isImpGraph(doc: Pick<EditorDocument, "nodeType">): boolean {
  return doc.nodeType === IMP_GRAPH_NODE_TYPE;
}

export function isUnsaved(doc: Pick<EditorDocument, "record">): boolean {
  return doc.record === null;
}

export function emptyImpGraph(): Graph {
  return { nodes: {}, edges: {} };
}

export function summarizeGraph(graph: Graph): string {
  const nodeCount = Object.keys(graph.nodes).length;
  const edgeCount = Object.keys(graph.edges).length;
  return `Imp graph · ${nodeCount} nodes · ${edgeCount} edges`;
}

/** A new, unsaved document. */
export function newUntitledDocument(
  nodeType: string,
  label: string,
  init: { graph?: Graph; value?: unknown },
): EditorDocument {
  return {
    id: crypto.randomUUID(),
    title: `Untitled ${label.toLowerCase()}`,
    nodeType,
    record: null,
    dirty: true,
    ...init,
  };
}

/** A node type Lab can open, as the explorer and search list it. */
export type NodeTypeInfo = {
  id: string;
  /** One record ("Graph"). */
  label: string;
  /** Many records ("Graphs"). */
  title: string;
};

export const IMP_GRAPH_TYPE_INFO: NodeTypeInfo = {
  id: IMP_GRAPH_NODE_TYPE,
  label: "Graph",
  title: "Graphs",
};
