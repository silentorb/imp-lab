import { describe, expect, test } from "bun:test";
import {
  canRedoValue,
  canUndoValue,
  createValueHistory,
  isValueDirty,
  jsonEquals,
  markValueSaved,
  pushValue,
  redoValue,
  undoValue,
} from "./valueHistory";

type Sketch = { strokes: number[] };

describe("valueHistory", () => {
  test("push undo redo round-trip", () => {
    let history = createValueHistory<Sketch>({ strokes: [] });
    expect(isValueDirty(history)).toBe(false);

    history = pushValue(history, { strokes: [1] });
    expect(isValueDirty(history)).toBe(true);
    expect(canUndoValue(history)).toBe(true);

    history = undoValue(history);
    expect(history.present.strokes).toHaveLength(0);
    expect(canRedoValue(history)).toBe(true);

    history = redoValue(history);
    expect(history.present.strokes).toEqual([1]);

    history = markValueSaved(history);
    expect(isValueDirty(history)).toBe(false);
  });

  test("equal values and coalesced edits", () => {
    let history = createValueHistory<Sketch>({ strokes: [] });
    expect(pushValue(history, { strokes: [] })).toBe(history);
    history = pushValue(history, { strokes: [1] }, "drag");
    history = pushValue(history, { strokes: [2] }, "drag");
    expect(history.past).toHaveLength(1);
    expect(history.present.strokes).toEqual([2]);
  });

  test("custom equality and key order", () => {
    expect(jsonEquals({ a: 1, b: 2 }, { b: 2, a: 1 })).toBe(true);
    const sameLength = (a: Sketch, b: Sketch) => a.strokes.length === b.strokes.length;
    const history = createValueHistory<Sketch>({ strokes: [1] });
    expect(pushValue(history, { strokes: [9] }, undefined, sameLength)).toBe(history);
  });
});
