import { describe, expect, test } from "bun:test";
import { applyNotationEdit } from "./edits";
import { emptyNotation } from "./notation";
import {
  canRedoNotation,
  canUndoNotation,
  createNotationHistory,
  isNotationDirty,
  markNotationSaved,
  pushNotation,
  redoNotation,
  undoNotation,
} from "./notationHistory";

describe("notationHistory", () => {
  test("push undo redo round-trip", () => {
    const base = emptyNotation("t");
    let history = createNotationHistory(base);
    expect(isNotationDirty(history)).toBe(false);

    const drawn = applyNotationEdit(history.present, {
      kind: "draw",
      semitone: 60,
      startBeats: 0,
      durationBeats: 0.25,
    });
    history = pushNotation(history, drawn);
    expect(isNotationDirty(history)).toBe(true);
    expect(canUndoNotation(history)).toBe(true);

    history = undoNotation(history);
    expect(history.present.events).toHaveLength(0);
    expect(canRedoNotation(history)).toBe(true);

    history = redoNotation(history);
    expect(history.present.events).toHaveLength(1);

    history = markNotationSaved(history);
    expect(isNotationDirty(history)).toBe(false);
  });
});
