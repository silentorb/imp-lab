import { describe, expect, test } from "bun:test";
import type { Graph } from "imp-core-types";
import { estimateNodeSize, layoutGraph } from "./layoutGraph";

function sizeMap(
  graph: Graph,
  size = estimateNodeSize(1, 1),
): Map<string, { width: number; height: number }> {
  return new Map(Object.keys(graph.nodes).map((id) => [id, size]));
}

describe("estimateNodeSize", () => {
  test("grows with port rows", () => {
    expect(estimateNodeSize(0, 0).height).toBe(56);
    expect(estimateNodeSize(2, 1).height).toBe(36 + 2 * 18);
    expect(estimateNodeSize(1, 1).width).toBe(168);
  });
});

describe("layoutGraph", () => {
  test("places sine-style chain left to right", async () => {
    const graph: Graph = {
      nodes: {
        time: { id: "time", type: "time", inputs: {} },
        freq: { id: "freq", type: "parameter", inputs: { value: 440 } },
        osc: { id: "osc", type: "sine", inputs: {} },
        out: { id: "out", type: "output", inputs: {} },
      },
      edges: {
        freq_to_osc: {
          from: { node: "freq", port: "value" },
          to: { node: "osc", port: "frequency" },
        },
        time_to_osc: {
          from: { node: "time", port: "time" },
          to: { node: "osc", port: "time" },
        },
        osc_to_out: {
          from: { node: "osc", port: "audio" },
          to: { node: "out", port: "value" },
        },
      },
    };

    const laid = await layoutGraph(graph, sizeMap(graph));
    const freq = laid.get("freq")!;
    const time = laid.get("time")!;
    const osc = laid.get("osc")!;
    const out = laid.get("out")!;

    expect(freq.x).toBeLessThan(osc.x);
    expect(time.x).toBeLessThan(osc.x);
    expect(osc.x).toBeLessThan(out.x);
  });

  test("does not overlap same-rank boxes", async () => {
    const graph: Graph = {
      nodes: {
        a: { id: "a", type: "parameter", inputs: {} },
        b: { id: "b", type: "parameter", inputs: {} },
        c: { id: "c", type: "output", inputs: {} },
      },
      edges: {
        a_c: {
          from: { node: "a", port: "value" },
          to: { node: "c", port: "value" },
        },
        b_c: {
          from: { node: "b", port: "value" },
          to: { node: "c", port: "value" },
        },
      },
    };

    const laid = await layoutGraph(graph, sizeMap(graph));
    const boxes = [...laid.values()];
    for (let i = 0; i < boxes.length; i++) {
      for (let j = i + 1; j < boxes.length; j++) {
        const left = boxes[i]!;
        const right = boxes[j]!;
        const xOverlap =
          left.x < right.x + right.width && right.x < left.x + left.width;
        const yOverlap =
          left.y < right.y + right.height && right.y < left.y + left.height;
        expect(xOverlap && yOverlap).toBe(false);
      }
    }
  });

  test("places dangling edge endpoints as missing cards", async () => {
    const graph: Graph = {
      nodes: {
        loop: { id: "loop", type: "loop", inputs: {} },
      },
      edges: {
        dangling: {
          from: { node: "ghost", port: "score" },
          to: { node: "loop", port: "value" },
        },
      },
    };

    const laid = await layoutGraph(graph, sizeMap(graph));
    expect(laid.has("loop")).toBe(true);
    expect(laid.has("ghost")).toBe(true);
  });

  test("collapses port-parallel edges and still places LR", async () => {
    const graph: Graph = {
      nodes: {
        src: { id: "src", type: "pack", inputs: {} },
        dst: { id: "dst", type: "output", inputs: {} },
      },
      edges: {
        e1: {
          from: { node: "src", port: "a" },
          to: { node: "dst", port: "value" },
        },
        e2: {
          from: { node: "src", port: "b" },
          to: { node: "dst", port: "value" },
          order: 1,
        },
      },
    };

    const laid = await layoutGraph(graph, sizeMap(graph));
    expect(laid.get("src")!.x).toBeLessThan(laid.get("dst")!.x);
  });
});
