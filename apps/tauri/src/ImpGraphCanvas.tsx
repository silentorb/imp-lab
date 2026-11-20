import { useMemo } from "react";
import { Text, useMantineColorScheme } from "@mantine/core";
import {
  Background,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  type Edge,
  type NodeTypes,
} from "@xyflow/react";
import type { Graph, InputValues } from "imp-core-types";
import { impToReactFlow } from "imp-graph-view";
import "@xyflow/react/dist/style.css";
import classes from "./ImpGraphCanvas.module.css";
import { ImpNode, type ImpFlowNode } from "./ImpNode";
import { resolvePorts } from "./ports";
import { labRegistry } from "./registry";

const GRID_COLS = 4;
const CELL_W = 240;
const CELL_H = 140;

const nodeTypes = { imp: ImpNode } satisfies NodeTypes;

type ImpGraphCanvasProps = {
  graph: Graph;
};

/** Host-only grid so Imp→RF default `{0,0}` positions do not stack. */
function toHostNodes(
  convertedNodes: ReturnType<typeof impToReactFlow>["nodes"],
  edges: Edge[],
): ImpFlowNode[] {
  return convertedNodes.map((node, index) => {
    const impType = node.type ?? "";
    const inputValues = (node.data?.inputValues ?? {}) as InputValues;
    const { inputPorts, outputPorts } = resolvePorts(
      labRegistry,
      impType,
      node.id,
      edges,
    );
    return {
      id: node.id,
      type: "imp",
      position: {
        x: (index % GRID_COLS) * CELL_W,
        y: Math.floor(index / GRID_COLS) * CELL_H,
      },
      data: {
        impType,
        inputValues,
        inputPorts,
        outputPorts,
      },
    };
  });
}

function ImpGraphCanvasInner({ graph }: ImpGraphCanvasProps) {
  const { colorScheme } = useMantineColorScheme();
  const { nodes, edges } = useMemo(() => {
    const converted = impToReactFlow(graph);
    const rfEdges = converted.edges as Edge[];
    return {
      nodes: toHostNodes(converted.nodes, rfEdges),
      edges: rfEdges,
    };
  }, [graph]);

  const empty = nodes.length === 0;
  const rfColorMode = colorScheme === "light" ? "light" : "dark";

  return (
    <div className={classes.root}>
      {empty ? (
        <div className={classes.empty}>
          <Text size="sm" c="dimmed" ta="center">
            Empty Imp graph
          </Text>
        </div>
      ) : (
        <ReactFlow
          className={classes.flow}
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          colorMode={rfColorMode}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.1}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={false}
          edgesFocusable={false}
          nodesFocusable={false}
          panOnDrag
          zoomOnScroll
          proOptions={{ hideAttribution: true }}
        >
          <Background gap={16} size={1} />
          <Controls showInteractive={false} />
        </ReactFlow>
      )}
    </div>
  );
}

export function ImpGraphCanvas(props: ImpGraphCanvasProps) {
  return (
    <ReactFlowProvider>
      <ImpGraphCanvasInner {...props} />
    </ReactFlowProvider>
  );
}
