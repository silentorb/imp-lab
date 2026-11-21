/** Per-document Imp graph undo/redo snapshot stack. */

import type { Graph } from "imp-core-types";

export const DEFAULT_HISTORY_CAP = 100;

export type GraphHistory = {
  past: Graph[];
  present: Graph;
  future: Graph[];
  saved: Graph;
  coalesceKey?: string;
};

function cloneGraph(graph: Graph): Graph {
  return structuredClone(graph);
}

/** Stable serialization for equality (sorted object keys). */
export function graphFingerprint(graph: Graph): string {
  const nodes = Object.keys(graph.nodes)
    .sort()
    .map((id) => {
      const node = graph.nodes[id]!;
      const inputs = Object.keys(node.inputs)
        .sort()
        .map((port) => [port, node.inputs[port]]);
      return [id, node.type, node.title ?? null, node.typeArgs ?? null, inputs];
    });
  const edges = Object.keys(graph.edges)
    .sort()
    .map((id) => {
      const edge = graph.edges[id]!;
      return [
        id,
        edge.from.node,
        edge.from.port,
        edge.to.node,
        edge.to.port,
        edge.order ?? null,
      ];
    });
  return JSON.stringify({ nodes, edges });
}

export function graphsEqual(a: Graph, b: Graph): boolean {
  return graphFingerprint(a) === graphFingerprint(b);
}

export function isDirty(history: GraphHistory): boolean {
  return !graphsEqual(history.present, history.saved);
}

export function createHistory(graph: Graph): GraphHistory {
  const present = cloneGraph(graph);
  return {
    past: [],
    present,
    future: [],
    saved: cloneGraph(present),
  };
}

export function push(
  history: GraphHistory,
  next: Graph,
  coalesceKey?: string,
  cap: number = DEFAULT_HISTORY_CAP,
): GraphHistory {
  if (graphsEqual(history.present, next)) {
    return history;
  }

  const nextPresent = cloneGraph(next);

  if (
    coalesceKey != null &&
    coalesceKey === history.coalesceKey &&
    history.past.length > 0
  ) {
    return {
      ...history,
      present: nextPresent,
      future: [],
      coalesceKey,
    };
  }

  const past = [...history.past, cloneGraph(history.present)];
  while (past.length > cap) {
    past.shift();
  }

  return {
    past,
    present: nextPresent,
    future: [],
    saved: history.saved,
    coalesceKey,
  };
}

export function undo(history: GraphHistory): GraphHistory {
  if (history.past.length === 0) {
    return history;
  }
  const past = [...history.past];
  const previous = past.pop()!;
  return {
    past,
    present: previous,
    future: [cloneGraph(history.present), ...history.future],
    saved: history.saved,
    coalesceKey: undefined,
  };
}

export function redo(history: GraphHistory): GraphHistory {
  if (history.future.length === 0) {
    return history;
  }
  const [next, ...future] = history.future;
  return {
    past: [...history.past, cloneGraph(history.present)],
    present: next!,
    future,
    saved: history.saved,
    coalesceKey: undefined,
  };
}

export function markSaved(history: GraphHistory): GraphHistory {
  return {
    ...history,
    saved: cloneGraph(history.present),
  };
}

/** Replace present without growing past (e.g. external open into tab). */
export function replacePresent(_history: GraphHistory, graph: Graph): GraphHistory {
  const present = cloneGraph(graph);
  return {
    past: [],
    present,
    future: [],
    saved: cloneGraph(present),
    coalesceKey: undefined,
  };
}

export function canUndo(history: GraphHistory): boolean {
  return history.past.length > 0;
}

export function canRedo(history: GraphHistory): boolean {
  return history.future.length > 0;
}
