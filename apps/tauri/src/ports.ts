import type { Edge } from "@xyflow/react";
import type { NodeTypeId } from "imp-core-types";
import {
  inferPortsFromEdges,
  type CatalogInputPort,
  type CatalogOutputPort,
} from "imp-graph-view";
import { getNodeDefinition, type Registry } from "imp-registry";

export type PortLists = {
  inputs: CatalogInputPort[];
  outputs: CatalogOutputPort[];
};

export { inferPortsFromEdges };

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
      inputs: Object.values(definition.inputs).map((port) => ({
        id: port.id,
        variadic: port.variadic === true,
        type: port.type,
        defaultValue: port.defaultValue,
      })),
      outputs: Object.values(definition.outputs).map((port) => ({
        id: port.id,
      })),
    };
  }
  return inferPortsFromEdges(nodeId, edges);
}
