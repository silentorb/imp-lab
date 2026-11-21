import { useCallback, useEffect, useMemo, useState } from "react";
import { useMantineColorScheme } from "@mantine/core";
import type { Edge } from "@xyflow/react";
import type { Graph } from "imp-core-types";
import {
  ImpGraphView,
  type GraphSelection,
  type LayoutBox,
  type ResolvePorts,
} from "imp-graph-view";
import "@xyflow/react/dist/style.css";
import "imp-graph-view/styles.css";
import { missingNodeIds } from "imp-graph-view";
import type { Registry } from "imp-registry";
import { estimateNodeSize, layoutGraph } from "./layoutGraph";
import { resolvePorts } from "./ports";
import classes from "./LabGraphPanel.module.css";

type LabGraphPanelProps = {
  graph: Graph;
  registry: Registry;
  onGraphChange: (graph: Graph) => void;
  onSelectionChange?: (selection: GraphSelection) => void;
  /** Stable per document tab — fits once on open, not on every topology edit. */
  fitViewKey?: string;
};

export function LabGraphPanel({
  graph,
  registry,
  onGraphChange,
  onSelectionChange,
  fitViewKey,
}: LabGraphPanelProps) {
  const { colorScheme } = useMantineColorScheme();
  const [layout, setLayout] = useState<Map<string, LayoutBox>>(new Map());

  const resolve: ResolvePorts = useCallback(
    (nodeType, nodeId, edges) => {
      return resolvePorts(registry, nodeType, nodeId, edges as Edge[]);
    },
    [registry],
  );

  const topologyKey = useMemo(() => {
    const nodes = Object.keys(graph.nodes).sort().join(",");
    const edges = Object.entries(graph.edges)
      .map(
        ([id, e]) =>
          `${id}:${e.from.node}.${e.from.port}->${e.to.node}.${e.to.port}:${e.order ?? ""}`,
      )
      .sort()
      .join("|");
    return `${nodes}#${edges}`;
  }, [graph]);

  useEffect(() => {
    let cancelled = false;
    if (Object.keys(graph.nodes).length === 0) {
      setLayout(new Map());
      return;
    }

    const sizes = new Map<string, { width: number; height: number }>();
    const sizeFor = (id: string, type: string) => {
      const ports = resolvePorts(registry, type, id, []);
      let inputSlots = 0;
      let hasSingletonInputs = false;
      for (const port of ports.inputs) {
        if (!port.variadic) {
          inputSlots += 1;
          hasSingletonInputs = true;
        } else {
          const fanIn = Object.values(graph.edges).filter(
            (e) => e.to.node === id && e.to.port === port.id,
          ).length;
          inputSlots += fanIn + 1;
        }
      }
      sizes.set(
        id,
        estimateNodeSize(inputSlots, Math.max(ports.outputs.length, 1), {
          hasSingletonInputs,
        }),
      );
    };
    for (const [id, node] of Object.entries(graph.nodes)) {
      sizeFor(id, node.type);
    }
    for (const id of missingNodeIds(graph)) {
      sizeFor(id, "(missing)");
    }

    void layoutGraph(graph, sizes).then((laid) => {
      if (!cancelled) {
        setLayout(laid);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [topologyKey, graph, registry]);

  const rfColorMode = colorScheme === "light" ? "light" : "dark";

  return (
    <div className={classes.root}>
      <ImpGraphView
        graph={graph}
        onGraphChange={onGraphChange}
        onSelectionChange={onSelectionChange}
        resolvePorts={resolve}
        layout={layout}
        nodesDraggable={false}
        colorMode={rfColorMode}
        fitViewKey={fitViewKey}
        className={classes.flow}
      />
    </div>
  );
}
