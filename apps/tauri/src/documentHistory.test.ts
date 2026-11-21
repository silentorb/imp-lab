import { describe, expect, test } from "bun:test";
import type { Graph } from "imp-core-types";
import {
  canRedo,
  canUndo,
  createHistory,
  graphsEqual,
  isDirty,
  markSaved,
  push,
  redo,
  replacePresent,
  undo,
} from "./documentHistory";

function graph(nodes: Graph["nodes"], edges: Graph["edges"] = {}): Graph {
  return { nodes, edges };
}

describe("graphsEqual", () => {
  test("ignores key insertion order", () => {
    const a = graph({
      b: { id: "b", type: "sine", inputs: {} },
      a: { id: "a", type: "time", inputs: {} },
    });
    const b = graph({
      a: { id: "a", type: "time", inputs: {} },
      b: { id: "b", type: "sine", inputs: {} },
    });
    expect(graphsEqual(a, b)).toBe(true);
  });
});

describe("createHistory / isDirty / markSaved", () => {
  test("starts clean matching present", () => {
    const g = graph({ n: { id: "n", type: "time", inputs: {} } });
    const h = createHistory(g);
    expect(isDirty(h)).toBe(false);
    expect(canUndo(h)).toBe(false);
    expect(canRedo(h)).toBe(false);
  });

  test("push makes dirty; markSaved clears", () => {
    const start = graph({});
    let h = createHistory(start);
    h = push(h, graph({ n: { id: "n", type: "time", inputs: {} } }));
    expect(isDirty(h)).toBe(true);
    h = markSaved(h);
    expect(isDirty(h)).toBe(false);
  });
});

describe("push / undo / redo", () => {
  test("undo restores previous; redo restores next", () => {
    const empty = graph({});
    const one = graph({ a: { id: "a", type: "time", inputs: {} } });
    const two = graph({
      a: { id: "a", type: "time", inputs: {} },
      b: { id: "b", type: "sine", inputs: {} },
    });

    let h = createHistory(empty);
    h = push(h, one);
    h = push(h, two);
    expect(Object.keys(h.present.nodes)).toEqual(["a", "b"]);

    h = undo(h);
    expect(Object.keys(h.present.nodes)).toEqual(["a"]);
    expect(canRedo(h)).toBe(true);

    h = undo(h);
    expect(Object.keys(h.present.nodes)).toEqual([]);
    expect(canUndo(h)).toBe(false);

    h = redo(h);
    expect(Object.keys(h.present.nodes)).toEqual(["a"]);
    h = redo(h);
    expect(Object.keys(h.present.nodes)).toEqual(["a", "b"]);
    expect(canRedo(h)).toBe(false);
  });

  test("push after undo clears future", () => {
    const empty = graph({});
    const one = graph({ a: { id: "a", type: "time", inputs: {} } });
    const other = graph({ z: { id: "z", type: "output", inputs: {} } });

    let h = createHistory(empty);
    h = push(h, one);
    h = undo(h);
    expect(canRedo(h)).toBe(true);
    h = push(h, other);
    expect(canRedo(h)).toBe(false);
    expect(Object.keys(h.present.nodes)).toEqual(["z"]);
  });

  test("identical push is a no-op", () => {
    const g = graph({ a: { id: "a", type: "time", inputs: {} } });
    let h = createHistory(g);
    const same = graph({ a: { id: "a", type: "time", inputs: {} } });
    const next = push(h, same);
    expect(next).toBe(h);
  });
});

describe("coalesce", () => {
  test("same coalesce key replaces present without growing past", () => {
    const empty = graph({});
    let h = createHistory(empty);
    h = push(h, graph({ a: { id: "a", type: "parameter", inputs: { value: 1 } } }), "lit");
    expect(h.past.length).toBe(1);
    h = push(
      h,
      graph({ a: { id: "a", type: "parameter", inputs: { value: 2 } } }),
      "lit",
    );
    expect(h.past.length).toBe(1);
    expect(h.present.nodes.a?.inputs.value).toBe(2);

    h = undo(h);
    expect(Object.keys(h.present.nodes)).toEqual([]);
  });

  test("different coalesce key grows past", () => {
    const empty = graph({});
    let h = createHistory(empty);
    h = push(h, graph({ a: { id: "a", type: "parameter", inputs: { value: 1 } } }), "a");
    h = push(h, graph({ a: { id: "a", type: "parameter", inputs: { value: 2 } } }), "b");
    expect(h.past.length).toBe(2);
  });
});

describe("cap", () => {
  test("drops oldest past entries", () => {
    let h = createHistory(graph({}));
    for (let i = 0; i < 5; i++) {
      h = push(
        h,
        graph({ [`n${i}`]: { id: `n${i}`, type: "time", inputs: {} } }),
        undefined,
        3,
      );
    }
    expect(h.past.length).toBe(3);
  });
});

describe("replacePresent", () => {
  test("resets stacks and saved baseline", () => {
    let h = createHistory(graph({}));
    h = push(h, graph({ a: { id: "a", type: "time", inputs: {} } }));
    const loaded = graph({ b: { id: "b", type: "sine", inputs: {} } });
    h = replacePresent(h, loaded);
    expect(canUndo(h)).toBe(false);
    expect(isDirty(h)).toBe(false);
    expect(Object.keys(h.present.nodes)).toEqual(["b"]);
  });
});

describe("undo past saved baseline", () => {
  test("dirty clears when undoing to saved graph", () => {
    const start = graph({ a: { id: "a", type: "time", inputs: {} } });
    let h = createHistory(start);
    h = push(h, graph({
      a: { id: "a", type: "time", inputs: {} },
      b: { id: "b", type: "sine", inputs: {} },
    }));
    expect(isDirty(h)).toBe(true);
    h = undo(h);
    expect(isDirty(h)).toBe(false);
  });
});
