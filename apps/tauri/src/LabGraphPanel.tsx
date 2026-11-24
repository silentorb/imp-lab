import { useCallback, useEffect, useMemo, useRef, useState, type Ref } from "react";
import { useMantineColorScheme } from "@mantine/core";
import type { Edge } from "@xyflow/react";
import {
  edgeMapProvider,
  graphSource,
  layeredNodes,
  nodeMapProvider,
  type Graph,
  type GraphSource,
} from "imp-core-types";
import {
  ImpGraphView,
  applyGraphEdits,
  graphWithUnresolvedStubs,
  unresolvedNodeIds,
  type AddableNodeType,
  type GraphEdit,
  type GraphSelection,
  type ImpGraphViewHandle,
  type LayoutBox,
  type ResolvePorts,
} from "imp-graph-view";
import "@xyflow/react/dist/style.css";
import "imp-graph-view/styles.css";
import { listNodeDefinitions, type Registry } from "imp-registry";
import { estimateNodeSize, layoutGraph } from "./layoutGraph";
import { resolvePorts } from "./ports";
import { resolveRecordNodes } from "./records";
import classes from "./LabGraphPanel.module.css";

type LabGraphPanelProps = {
  graph: Graph;
  registry: Registry;
  onGraphChange: (graph: Graph) => void;
  onSelectionChange?: (selection: GraphSelection) => void;
  /** Stable per document tab — fits once on open, not on every topology edit. */
  fitViewKey?: string;
  /** Host commands (e.g. `graph.add-node`) drive the view through this. */
  ref?: Ref<ImpGraphViewHandle>;
};

/** Registry node types for the add-node picker, one entry per id. */
function addableNodeTypes(registry: Registry): AddableNodeType[] {
  const byId = new Map<string, AddableNodeType>();
  for (const definition of listNodeDefinitions(registry)) {
    if (!byId.has(definition.id)) {
      byId.set(definition.id, { id: definition.id, name: definition.name });
    }
  }
  return [...byId.values()];
}

/**
 * Nodes the document references but does not hold, resolved from the open project's Tome
 * stores. Empty while loading or when no project is open.
 */
function useRemoteNodes(graph: Graph, registry: Registry): Graph["nodes"] {
  const [remote, setRemote] = useState<Graph["nodes"]>({});
  const wantedKey = useMemo(
    () => unresolvedNodeIds(graphSource(graph)).sort().join("\n"),
    [graph],
  );
  useEffect(() => {
    const ids = wantedKey ? wantedKey.split("\n") : [];
    if (ids.length === 0) {
      setRemote({});
      return;
    }
    let cancelled = false;
    const nodeTypes = [...new Set(listNodeDefinitions(registry).map((d) => d.id))];
    resolveRecordNodes(ids, nodeTypes)
      .then((nodes) => {
        if (!cancelled) setRemote(Object.fromEntries(nodes.map((n) => [n.id, n])));
      })
      .catch(() => {
        if (!cancelled) setRemote({});
      });
    return () => {
      cancelled = true;
    };
  }, [wantedKey, registry]);
  return remote;
}

export function LabGraphPanel({
  graph,
  registry,
  onGraphChange,
  onSelectionChange,
  fitViewKey,
  ref,
}: LabGraphPanelProps) {
  const { colorScheme } = useMantineColorScheme();
  const nodeTypes = useMemo(() => addableNodeTypes(registry), [registry]);
  const [layout, setLayout] = useState<Map<string, LayoutBox>>(new Map());
  const remoteNodes = useRemoteNodes(graph, registry);

  // The document's own nodes first, then nodes from the project's other records.
  const source: GraphSource = useMemo(
    () => ({
      edges: edgeMapProvider(graph.edges),
      nodes: layeredNodes(nodeMapProvider(graph.nodes), nodeMapProvider(remoteNodes)),
    }),
    [graph, remoteNodes],
  );

  // Edits apply to this document. Edits to nodes another record holds are skipped by
  // applyGraphEdits; edit those in their own record.
  const latestGraph = useRef(graph);
  latestGraph.current = graph;
  const onEdit = useCallback(
    (edits: GraphEdit[]) => {
      const next = applyGraphEdits(latestGraph.current, edits);
      if (next === latestGraph.current) return;
      latestGraph.current = next;
      onGraphChange(next);
    },
    [onGraphChange],
  );

  const resolve: ResolvePorts = useCallback(
    (nodeType, nodeId, edges) => {
      return resolvePorts(registry, nodeType, nodeId, edges as Edge[]);
    },
    [registry],
  );

  const layoutInput = useMemo(() => graphWithUnresolvedStubs(source), [source]);
  const topologyKey = useMemo(() => {
    const nodes = Object.keys(layoutInput.nodes).sort().join(",");
    const edges = Object.entries(layoutInput.edges)
      .map(
        ([id, e]) =>
          `${id}:${e.from.node}.${e.from.port}->${e.to.node}.${e.to.port}:${e.order ?? ""}`,
      )
      .sort()
      .join("|");
    return `${nodes}#${edges}`;
  }, [layoutInput]);

  useEffect(() => {
    let cancelled = false;
    if (Object.keys(layoutInput.nodes).length === 0) {
      setLayout(new Map());
      return;
    }

    const sizes = new Map<string, { width: number; height: number }>();
    for (const [id, node] of Object.entries(layoutInput.nodes)) {
      const ports = resolvePorts(registry, node.type, id, []);
      let inputSlots = 0;
      let hasSingletonInputs = false;
      for (const port of ports.inputs) {
        if (!port.variadic) {
          inputSlots += 1;
          hasSingletonInputs = true;
        } else {
          const fanIn = Object.values(layoutInput.edges).filter(
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
    }

    void layoutGraph(layoutInput, sizes).then((laid) => {
      if (!cancelled) {
        setLayout(laid);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [topologyKey, layoutInput, registry]);

  const rfColorMode = colorScheme === "light" ? "light" : "dark";

  return (
    <div className={classes.root}>
      <ImpGraphView
        ref={ref}
        source={source}
        addableNodeTypes={nodeTypes}
        onEdit={onEdit}
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
