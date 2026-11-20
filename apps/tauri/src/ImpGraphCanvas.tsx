import { useEffect, useMemo, useState } from "react";
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
import { estimateNodeSize, layoutGraph } from "./layoutGraph";
import { resolvePorts } from "./ports";
import { labRegistry } from "./registry";

const nodeTypes = { imp: ImpNode } satisfies NodeTypes;

type ImpGraphCanvasProps = {
  graph: Graph;
};

type PreparedNode = {
  id: string;
  impType: string;
  inputValues: InputValues;
  inputPorts: string[];
  outputPorts: string[];
};

function prepareHostNodes(
  graph: Graph,
  convertedNodes: ReturnType<typeof impToReactFlow>["nodes"],
  edges: Edge[],
): { prepared: PreparedNode[]; sizes: Map<string, { width: number; height: number }> } {
  const sizes = new Map<string, { width: number; height: number }>();
  const prepared = convertedNodes.map((node) => {
    const impType = node.type ?? "";
    const inputValues = (node.data?.inputValues ?? {}) as InputValues;
    const { inputPorts, outputPorts } = resolvePorts(
      labRegistry,
      impType,
      node.id,
      edges,
    );
    sizes.set(node.id, estimateNodeSize(inputPorts.length, outputPorts.length));
    return {
      id: node.id,
      impType,
      inputValues,
      inputPorts,
      outputPorts,
    };
  });
  // Ensure size keys exist for every Imp node even if converter omitted one.
  for (const id of Object.keys(graph.nodes)) {
    if (!sizes.has(id)) {
      sizes.set(id, estimateNodeSize(1, 1));
    }
  }
  return { prepared, sizes };
}

function ImpGraphCanvasInner({ graph }: ImpGraphCanvasProps) {
  const { colorScheme } = useMantineColorScheme();
  const edges = useMemo(() => {
    return impToReactFlow(graph).edges as Edge[];
  }, [graph]);

  const [nodes, setNodes] = useState<ImpFlowNode[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    const converted = impToReactFlow(graph);
    const rfEdges = converted.edges as Edge[];

    if (converted.nodes.length === 0) {
      setNodes([]);
      return;
    }

    setNodes(null);
    const { prepared, sizes } = prepareHostNodes(
      graph,
      converted.nodes,
      rfEdges,
    );

    void layoutGraph(graph, sizes).then((laid) => {
      if (cancelled) {
        return;
      }
      setNodes(
        prepared.map((node) => {
          const box = laid.get(node.id) ?? {
            x: 0,
            y: 0,
            ...estimateNodeSize(node.inputPorts.length, node.outputPorts.length),
          };
          return {
            id: node.id,
            type: "imp" as const,
            position: { x: box.x, y: box.y },
            style: { width: box.width, height: box.height },
            data: {
              impType: node.impType,
              inputValues: node.inputValues,
              inputPorts: node.inputPorts,
              outputPorts: node.outputPorts,
            },
          };
        }),
      );
    });

    return () => {
      cancelled = true;
    };
  }, [graph]);

  const empty = Object.keys(graph.nodes).length === 0;
  const rfColorMode = colorScheme === "light" ? "light" : "dark";
  const waiting = !empty && nodes === null;

  return (
    <div className={classes.root}>
      {empty ? (
        <div className={classes.empty}>
          <Text size="sm" c="dimmed" ta="center">
            Empty Imp graph
          </Text>
        </div>
      ) : waiting ? (
        <div className={classes.empty}>
          <Text size="sm" c="dimmed" ta="center">
            Laying out graph…
          </Text>
        </div>
      ) : (
        <ReactFlow
          className={classes.flow}
          nodes={nodes ?? []}
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
