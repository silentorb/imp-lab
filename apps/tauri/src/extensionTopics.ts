/**
 * Topics: the latest value extensions published under each topic id (opaque JSON).
 *
 * Extensions push changes; the host forwards each one as an `extension://topic` event. Lab does
 * not interpret topic values; the components that display a topic (a status item, a setting's
 * options, the piano-roll take) know its shape.
 */

import { useCallback, useSyncExternalStore } from "react";

export const TOPIC_EVENT = "extension://topic";

export type TopicPayload = { topic: string; value: unknown };

type Listener = () => void;

export type TopicStore = {
  get(topic: string): unknown;
  set(topic: string, value: unknown): void;
  /** Seeds topics that have no value yet (a snapshot may be older than live events). */
  seed(snapshot: Record<string, unknown>): void;
  subscribe(topic: string, listener: Listener): () => void;
};

export function createTopicStore(): TopicStore {
  const values = new Map<string, unknown>();
  const listeners = new Map<string, Set<Listener>>();
  const notify = (topic: string) => {
    for (const listener of listeners.get(topic) ?? []) {
      listener();
    }
  };
  return {
    get: (topic) => values.get(topic),
    set(topic, value) {
      values.set(topic, value);
      notify(topic);
    },
    seed(snapshot) {
      for (const [topic, value] of Object.entries(snapshot)) {
        if (!values.has(topic)) {
          values.set(topic, value);
          notify(topic);
        }
      }
    },
    subscribe(topic, listener) {
      let set = listeners.get(topic);
      if (!set) {
        set = new Set();
        listeners.set(topic, set);
      }
      set.add(listener);
      return () => {
        set.delete(listener);
      };
    },
  };
}

/** The app's topic store. */
export const topicStore = createTopicStore();

export type TopicInvoke = (command: "get_topics") => Promise<unknown>;
export type TopicListen = (
  event: string,
  handler: (event: { payload: TopicPayload }) => void,
) => Promise<() => void>;

/**
 * Feeds `store` from the host: live events first, then the snapshot of everything published
 * before the webview was listening. Returns the unlisten function.
 */
export async function connectTopics(
  invoke: TopicInvoke,
  listen: TopicListen,
  store: TopicStore = topicStore,
): Promise<() => void> {
  const unlisten = await listen(TOPIC_EVENT, (event) => {
    const { topic, value } = event.payload;
    if (typeof topic === "string") {
      store.set(topic, value);
    }
  });
  try {
    const snapshot = await invoke("get_topics");
    if (snapshot && typeof snapshot === "object") {
      store.seed(snapshot as Record<string, unknown>);
    }
  } catch (err) {
    console.warn("topic snapshot unavailable:", err);
  }
  return unlisten;
}

/** Current value of `topic` (re-renders on change); `undefined` until something is published. */
export function useTopic<T = unknown>(
  topic: string | null | undefined,
  store: TopicStore = topicStore,
): T | undefined {
  const subscribe = useCallback(
    (listener: Listener) => (topic ? store.subscribe(topic, listener) : () => {}),
    [store, topic],
  );
  const snapshot = useCallback(
    () => (topic ? (store.get(topic) as T | undefined) : undefined),
    [store, topic],
  );
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}
