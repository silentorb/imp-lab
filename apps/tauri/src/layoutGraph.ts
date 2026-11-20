import ELK from "elkjs/lib/elk.bundled.js";
import type { Graph } from "imp-core-types";

export type NodeSize = { width: number; height: number };

export type LaidOutBox = {
  x: number;
  y: number;
  width: number;
  height: number;
};

const DEFAULT_SIZE: NodeSize = { width: 168, height: 56 };

const elk = new ELK();

/** Estimate ImpNode box size from resolved port counts. */
export function estimateNodeSize(
  inputCount: number,
  outputCount: number,
): NodeSize {
  const rows = Math.max(inputCount, outputCount, 1);
  return {
    width: 168,
    height: Math.max(56, 36 + rows * 18),
  };
}

/**
 * Host-only ELK layered left-to-right layout for an Imp graph.
 * Collapses port-parallel edges to one ELK edge; skips self-loops.
 */
export async function layoutGraph(
  graph: Graph,
  nodeSizes: Map<string, NodeSize>,
): Promise<Map<string, LaidOutBox>> {
  const children = Object.keys(graph.nodes).map((id) => {
    const size = nodeSizes.get(id) ?? DEFAULT_SIZE;
    return {
      id,
      width: size.width,
      height: size.height,
    };
  });

  const seenPairs = new Set<string>();
  const edges: { id: string; sources: string[]; targets: string[] }[] = [];
  for (const [edgeId, edge] of Object.entries(graph.edges)) {
    const source = edge.from.node;
    const target = edge.to.node;
    if (source === target) {
      continue;
    }
    if (!graph.nodes[source] || !graph.nodes[target]) {
      continue;
    }
    const pairKey = `${source}\0${target}`;
    if (seenPairs.has(pairKey)) {
      continue;
    }
    seenPairs.add(pairKey);
    edges.push({
      id: edgeId,
      sources: [source],
      targets: [target],
    });
  }

  const laid = await elk.layout({
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.spacing.nodeNode": "36",
      "elk.layered.spacing.nodeNodeBetweenLayers": "72",
    },
    children,
    edges,
  });

  const result = new Map<string, LaidOutBox>();
  for (const child of laid.children ?? []) {
    if (child.x == null || child.y == null) {
      continue;
    }
    const width = child.width ?? DEFAULT_SIZE.width;
    const height = child.height ?? DEFAULT_SIZE.height;
    result.set(child.id, {
      x: child.x,
      y: child.y,
      width,
      height,
    });
  }
  return result;
}
