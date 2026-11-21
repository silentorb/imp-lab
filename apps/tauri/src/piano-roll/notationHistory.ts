/** Per-document notation undo/redo snapshot stack. */

import type { Notation } from "./notation";

export const DEFAULT_HISTORY_CAP = 100;

export type NotationHistory = {
  past: Notation[];
  present: Notation;
  future: Notation[];
  saved: Notation;
  coalesceKey?: string;
};

function cloneNotation(notation: Notation): Notation {
  return structuredClone(notation);
}

export function notationFingerprint(notation: Notation): string {
  return JSON.stringify({
    id: notation.id ?? null,
    title: notation.title ?? null,
    tempoBpm: notation.tempoBpm,
    timeSignature: notation.timeSignature,
    events: notation.events,
  });
}

export function notationsEqual(a: Notation, b: Notation): boolean {
  return notationFingerprint(a) === notationFingerprint(b);
}

export function isNotationDirty(history: NotationHistory): boolean {
  return !notationsEqual(history.present, history.saved);
}

export function createNotationHistory(notation: Notation): NotationHistory {
  const present = cloneNotation(notation);
  return {
    past: [],
    present,
    future: [],
    saved: cloneNotation(present),
  };
}

export function pushNotation(
  history: NotationHistory,
  next: Notation,
  coalesceKey?: string,
  cap: number = DEFAULT_HISTORY_CAP,
): NotationHistory {
  if (notationsEqual(history.present, next)) {
    return history;
  }

  const nextPresent = cloneNotation(next);

  if (
    coalesceKey != null &&
    coalesceKey === history.coalesceKey &&
    history.past.length > 0
  ) {
    return {
      ...history,
      present: nextPresent,
      future: [],
      coalesceKey,
    };
  }

  const past = [...history.past, cloneNotation(history.present)];
  while (past.length > cap) {
    past.shift();
  }

  return {
    past,
    present: nextPresent,
    future: [],
    saved: history.saved,
    coalesceKey,
  };
}

export function undoNotation(history: NotationHistory): NotationHistory {
  if (history.past.length === 0) {
    return history;
  }
  const past = [...history.past];
  const previous = past.pop()!;
  return {
    past,
    present: previous,
    future: [cloneNotation(history.present), ...history.future],
    saved: history.saved,
    coalesceKey: undefined,
  };
}

export function redoNotation(history: NotationHistory): NotationHistory {
  if (history.future.length === 0) {
    return history;
  }
  const [next, ...future] = history.future;
  return {
    past: [...history.past, cloneNotation(history.present)],
    present: next!,
    future,
    saved: history.saved,
    coalesceKey: undefined,
  };
}

export function markNotationSaved(history: NotationHistory): NotationHistory {
  return {
    ...history,
    saved: cloneNotation(history.present),
  };
}

export function canUndoNotation(history: NotationHistory): boolean {
  return history.past.length > 0;
}

export function canRedoNotation(history: NotationHistory): boolean {
  return history.future.length > 0;
}
