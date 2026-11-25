/** Per-document undo/redo snapshot stack for extension editor values. */

export const DEFAULT_HISTORY_CAP = 100;

export type Equals<V> = (a: V, b: V) => boolean;

export type ValueHistory<V> = {
  past: V[];
  present: V;
  future: V[];
  saved: V;
  coalesceKey?: string;
};

/** Stable JSON (sorted object keys), the default equality. */
export function stableJson(value: unknown): string {
  return JSON.stringify(value, (_key, v: unknown) =>
    v && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(
          Object.keys(v as Record<string, unknown>)
            .sort()
            .map((k) => [k, (v as Record<string, unknown>)[k]]),
        )
      : v,
  );
}

export function jsonEquals<V>(a: V, b: V): boolean {
  return stableJson(a) === stableJson(b);
}

export function createValueHistory<V>(value: V): ValueHistory<V> {
  const present = structuredClone(value);
  return { past: [], present, future: [], saved: structuredClone(present) };
}

export function isValueDirty<V>(history: ValueHistory<V>, equals: Equals<V> = jsonEquals): boolean {
  return !equals(history.present, history.saved);
}

export function pushValue<V>(
  history: ValueHistory<V>,
  next: V,
  coalesceKey?: string,
  equals: Equals<V> = jsonEquals,
  cap: number = DEFAULT_HISTORY_CAP,
): ValueHistory<V> {
  if (equals(history.present, next)) {
    return history;
  }
  const present = structuredClone(next);
  if (coalesceKey != null && coalesceKey === history.coalesceKey && history.past.length > 0) {
    return { ...history, present, future: [], coalesceKey };
  }
  const past = [...history.past, structuredClone(history.present)];
  while (past.length > cap) {
    past.shift();
  }
  return { past, present, future: [], saved: history.saved, coalesceKey };
}

export function undoValue<V>(history: ValueHistory<V>): ValueHistory<V> {
  if (history.past.length === 0) {
    return history;
  }
  const past = [...history.past];
  const previous = past.pop()!;
  return {
    past,
    present: previous,
    future: [structuredClone(history.present), ...history.future],
    saved: history.saved,
  };
}

export function redoValue<V>(history: ValueHistory<V>): ValueHistory<V> {
  if (history.future.length === 0) {
    return history;
  }
  const [next, ...future] = history.future;
  return {
    past: [...history.past, structuredClone(history.present)],
    present: next!,
    future,
    saved: history.saved,
  };
}

export function markValueSaved<V>(history: ValueHistory<V>): ValueHistory<V> {
  return { ...history, saved: structuredClone(history.present) };
}

export const canUndoValue = <V>(history: ValueHistory<V>) => history.past.length > 0;
export const canRedoValue = <V>(history: ValueHistory<V>) => history.future.length > 0;
