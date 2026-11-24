/**
 * Node UI declared by extensions (`get_node_ui`): dropdown options for a node type's literal
 * input, and action buttons in Properties that send the owning service a request.
 *
 * Lab does not know what the node types mean. A service may edit a node's literals afterwards
 * through the action's edits topic; Lab applies each edit to the open graph that holds the node,
 * merging the edits of one session into one undo step.
 */

import type { Graph, PrimitiveValue } from "imp-core-types";

export type NodeLiteralOptionsDecl = {
  id: string;
  nodeType: string;
  port: string;
  optionsTopic: string;
};

export type NodeActionDecl = {
  id: string;
  label: string;
  nodeType: string;
  serviceId: string;
  action: string;
  editsTopic: string | null;
  order: number;
};

export type NodeUi = {
  literalOptions: NodeLiteralOptionsDecl[];
  actions: NodeActionDecl[];
  warnings: string[];
};

export const EMPTY_NODE_UI: NodeUi = { literalOptions: [], actions: [], warnings: [] };

/** One edit from an edits topic: replace literals of `node` (`null` removes one). */
export type NodeEdit = {
  seq: number;
  session: string;
  node: string;
  inputs: Record<string, PrimitiveValue | null>;
};

export function literalOptionsFor(
  ui: NodeUi,
  nodeType: string,
  port: string,
): NodeLiteralOptionsDecl | undefined {
  return ui.literalOptions.find((o) => o.nodeType === nodeType && o.port === port);
}

export function actionsFor(ui: NodeUi, nodeType: string): NodeActionDecl[] {
  return ui.actions.filter((a) => a.nodeType === nodeType);
}

/** Every edits topic the declared actions use, once each. */
export function editsTopics(ui: NodeUi): string[] {
  return [...new Set(ui.actions.flatMap((a) => (a.editsTopic ? [a.editsTopic] : [])))];
}

function isPrimitive(value: unknown): value is PrimitiveValue | null {
  return (
    value === null ||
    typeof value === "string" ||
    typeof value === "number" ||
    typeof value === "boolean"
  );
}

/** A topic value as a node edit; anything malformed is `null`. */
export function parseNodeEdit(value: unknown): NodeEdit | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const v = value as Record<string, unknown>;
  if (
    typeof v.seq !== "number" ||
    typeof v.session !== "string" ||
    typeof v.node !== "string" ||
    !v.inputs ||
    typeof v.inputs !== "object" ||
    Array.isArray(v.inputs)
  ) {
    return null;
  }
  const inputs: Record<string, PrimitiveValue | null> = {};
  for (const [port, input] of Object.entries(v.inputs as Record<string, unknown>)) {
    if (!isPrimitive(input)) {
      return null;
    }
    inputs[port] = input;
  }
  return { seq: v.seq, session: v.session, node: v.node, inputs };
}

/**
 * `graph` with the edit's literals applied, or the same graph when the node is not in it or
 * nothing changes.
 */
export function applyNodeEdit(graph: Graph, edit: NodeEdit): Graph {
  const node = graph.nodes[edit.node];
  if (!node) {
    return graph;
  }
  const inputs = { ...node.inputs };
  let changed = false;
  for (const [port, value] of Object.entries(edit.inputs)) {
    const had = Object.prototype.hasOwnProperty.call(inputs, port);
    if (value === null) {
      if (had) {
        delete inputs[port];
        changed = true;
      }
    } else if (!had || inputs[port] !== value) {
      inputs[port] = value;
      changed = true;
    }
  }
  if (!changed) {
    return graph;
  }
  return { ...graph, nodes: { ...graph.nodes, [edit.node]: { ...node, inputs } } };
}

/** Undo coalescing key: one GUI session (per topic) is one undo step. */
export function editCoalesceKey(topic: string, edit: NodeEdit): string {
  return `node-edit:${topic}:${edit.session}`;
}

/**
 * Tracks the last edit seen per topic, so a repeated topic value (a snapshot, a re-render)
 * applies once.
 */
export function createEditSequencer() {
  const last = new Map<string, number>();
  return {
    /** Whether `edit` on `topic` is new; records it when it is. */
    accept(topic: string, edit: NodeEdit): boolean {
      const seen = last.get(topic);
      if (seen !== undefined && edit.seq <= seen) {
        return false;
      }
      last.set(topic, edit.seq);
      return true;
    },
  };
}

export type ActionInvoke = (
  command: "service_request",
  args: { serviceId: string; requestJson: string },
) => Promise<unknown>;

export type ActionNode = { id: string; type: string; inputs: Partial<Record<string, PrimitiveValue>> };

/**
 * Sends an action's request: `{type: "node_action", action, node}`. Resolves to an error
 * message to show, or `null` on success (a reply of `{error}` is a failure).
 */
export async function runNodeAction(
  invoke: ActionInvoke,
  action: NodeActionDecl,
  node: ActionNode,
): Promise<string | null> {
  try {
    const reply = await invoke("service_request", {
      serviceId: action.serviceId,
      requestJson: JSON.stringify({ type: "node_action", action: action.action, node }),
    });
    const parsed = typeof reply === "string" && reply ? JSON.parse(reply) : reply;
    if (parsed && typeof parsed === "object" && typeof (parsed as { error?: unknown }).error === "string") {
      return (parsed as { error: string }).error;
    }
    return null;
  } catch (err) {
    return err instanceof Error ? err.message : String(err);
  }
}
