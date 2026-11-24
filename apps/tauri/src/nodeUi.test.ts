import { describe, expect, test } from "bun:test";
import type { Graph } from "imp-core-types";
import {
  actionsFor,
  applyNodeEdit,
  createEditSequencer,
  editCoalesceKey,
  editsTopics,
  literalOptionsFor,
  parseNodeEdit,
  runNodeAction,
  type NodeActionDecl,
  type NodeUi,
} from "./nodeUi";

const openGui: NodeActionDecl = {
  id: "gui",
  label: "Open plugin GUI",
  nodeType: "synth",
  serviceId: "playback",
  action: "open",
  editsTopic: "a.edits",
  order: 10,
};

const ui: NodeUi = {
  literalOptions: [
    { id: "opts", nodeType: "synth", port: "plugin", optionsTopic: "a.plugins" },
  ],
  actions: [openGui, { ...openGui, id: "fx", nodeType: "fx", editsTopic: "a.edits" }],
  warnings: [],
};

const graph: Graph = {
  nodes: {
    n1: { id: "n1", type: "synth", inputs: { plugin: "p", params: "{}" } },
  },
  edges: {},
} as unknown as Graph;

describe("node UI declarations", () => {
  test("look up options and actions by node type", () => {
    expect(literalOptionsFor(ui, "synth", "plugin")?.optionsTopic).toBe("a.plugins");
    expect(literalOptionsFor(ui, "synth", "state")).toBeUndefined();
    expect(actionsFor(ui, "synth").map((a) => a.id)).toEqual(["gui"]);
    expect(editsTopics(ui)).toEqual(["a.edits"]);
  });
});

describe("node edits", () => {
  test("parse rejects malformed values", () => {
    expect(parseNodeEdit({ seq: 1, session: "s", node: "n1", inputs: { state: "x" } })).not.toBeNull();
    expect(parseNodeEdit({ seq: "1", session: "s", node: "n1", inputs: {} })).toBeNull();
    expect(parseNodeEdit({ seq: 1, session: "s", node: "n1", inputs: [] })).toBeNull();
    expect(parseNodeEdit({ seq: 1, session: "s", node: "n1", inputs: { a: {} } })).toBeNull();
    expect(parseNodeEdit(null)).toBeNull();
  });

  test("apply sets and removes literals, leaving other nodes and graphs alone", () => {
    const edit = parseNodeEdit({
      seq: 1,
      session: "s",
      node: "n1",
      inputs: { state: "AAA=", params: null },
    })!;
    const next = applyNodeEdit(graph, edit);
    expect(next.nodes.n1!.inputs).toEqual({ plugin: "p", state: "AAA=" });
    expect(graph.nodes.n1!.inputs).toEqual({ plugin: "p", params: "{}" });
    expect(applyNodeEdit(next, edit)).toBe(next);
    expect(applyNodeEdit(graph, { ...edit, node: "elsewhere" })).toBe(graph);
  });

  test("one session is one coalesce key; each seq applies once", () => {
    const edit = parseNodeEdit({ seq: 2, session: "s", node: "n1", inputs: {} })!;
    expect(editCoalesceKey("a.edits", edit)).toBe(editCoalesceKey("a.edits", { ...edit, seq: 3 }));
    const sequencer = createEditSequencer();
    expect(sequencer.accept("a.edits", edit)).toBe(true);
    expect(sequencer.accept("a.edits", edit)).toBe(false);
    expect(sequencer.accept("a.edits", { ...edit, seq: 3 })).toBe(true);
    expect(sequencer.accept("b.edits", edit)).toBe(true);
  });
});

describe("running an action", () => {
  const node = { id: "n1", type: "synth", inputs: { plugin: "p" } };

  test("sends a node_action request to the action's service", async () => {
    const sent: unknown[] = [];
    const error = await runNodeAction(
      async (_command, args) => {
        sent.push({ serviceId: args.serviceId, request: JSON.parse(args.requestJson) });
        return "{}";
      },
      openGui,
      node,
    );
    expect(error).toBeNull();
    expect(sent).toEqual([
      { serviceId: "playback", request: { type: "node_action", action: "open", node } },
    ]);
  });

  test("an error reply or a failed call is reported", async () => {
    expect(await runNodeAction(async () => '{"error":"no GUI"}', openGui, node)).toBe("no GUI");
    expect(
      await runNodeAction(
        async () => {
          throw new Error("service gone");
        },
        openGui,
        node,
      ),
    ).toBe("service gone");
  });
});
