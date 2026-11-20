import type { Edge } from "@xyflow/react";
import type { NodeTypeId } from "imp-core-types";
import { getNodeDefinition, type Registry } from "imp-registry";

export type PortLists = {
  inputPorts: string[];
  outputPorts: string[];
};

/** Unique non-empty handle ids from edges incident on `nodeId`. */
export function inferPortsFromEdges(nodeId: string, edges: Edge[]): PortLists {
  const inputs = new Set<string>();
  const outputs = new Set<string>();
  for (const edge of edges) {
    if (edge.target === nodeId && edge.targetHandle) {
      inputs.add(edge.targetHandle);
    }
    if (edge.source === nodeId && edge.sourceHandle) {
      outputs.add(edge.sourceHandle);
    }
  }
  return {
    inputPorts: [...inputs],
    outputPorts: [...outputs],
  };
}

/** Prefer catalog ports; fall back to edge-inferred handles for unknown types. */
export function resolvePorts(
  registry: Registry,
  impType: NodeTypeId | undefined,
  nodeId: string,
  edges: Edge[],
): PortLists {
  const definition =
    impType != null && impType !== ""
      ? getNodeDefinition(registry, impType)
      : undefined;
  if (definition) {
    return {
      inputPorts: Object.keys(definition.inputs),
      outputPorts: Object.keys(definition.outputs),
    };
  }
  return inferPortsFromEdges(nodeId, edges);
}
