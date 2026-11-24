import { describe, expect, test } from "bun:test";
import { connectTopics, createTopicStore, TOPIC_EVENT, type TopicPayload } from "./extensionTopics";

describe("topic store", () => {
  test("set notifies subscribers of that topic only", () => {
    const store = createTopicStore();
    const seen: string[] = [];
    const off = store.subscribe("a", () => seen.push("a"));
    store.subscribe("b", () => seen.push("b"));
    store.set("a", 1);
    expect(store.get("a")).toBe(1);
    expect(seen).toEqual(["a"]);
    off();
    store.set("a", 2);
    expect(seen).toEqual(["a"]);
  });

  test("seed never overwrites a live value", () => {
    const store = createTopicStore();
    store.set("a", "live");
    store.seed({ a: "old", b: "snap" });
    expect(store.get("a")).toBe("live");
    expect(store.get("b")).toBe("snap");
  });

  test("connect listens first, then seeds from the snapshot", async () => {
    const store = createTopicStore();
    let handler: ((event: { payload: TopicPayload }) => void) | null = null;
    const unlisten = await connectTopics(
      async () => {
        // An event arrives while the snapshot is in flight.
        handler?.({ payload: { topic: "x", value: "event" } });
        return { x: "snapshot", y: 1 };
      },
      async (event, h) => {
        expect(event).toBe(TOPIC_EVENT);
        handler = h;
        return () => {};
      },
      store,
    );
    expect(store.get("x")).toBe("event");
    expect(store.get("y")).toBe(1);
    unlisten();
  });
});
