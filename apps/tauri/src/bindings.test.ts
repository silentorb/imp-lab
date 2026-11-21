import { describe, expect, test } from "bun:test";
import { resolveKeyEvent, type BindingDto } from "./bindings";

function keyEvent(
  key: string,
  mods: { ctrl?: boolean; meta?: boolean; alt?: boolean; shift?: boolean } = {},
): KeyboardEvent {
  return {
    key,
    ctrlKey: !!mods.ctrl,
    metaKey: !!mods.meta,
    altKey: !!mods.alt,
    shiftKey: !!mods.shift,
  } as KeyboardEvent;
}

const sample: BindingDto[] = [
  {
    command: "document.save",
    key: "s",
    modifiers: ["ctrl"],
  },
  {
    command: "notation.tool.select",
    key: "s",
    modifiers: [],
  },
  {
    command: "play.toggle",
    key: "space",
    modifiers: [],
  },
  {
    command: "edit.redo",
    key: "z",
    modifiers: ["ctrl", "shift"],
  },
];

describe("resolveKeyEvent", () => {
  test("matches ctrl binding with Ctrl", () => {
    expect(resolveKeyEvent(keyEvent("s", { ctrl: true }), sample)).toBe(
      "document.save",
    );
  });

  test("matches ctrl binding with Meta (Cmd)", () => {
    expect(resolveKeyEvent(keyEvent("s", { meta: true }), sample)).toBe(
      "document.save",
    );
  });

  test("matches bare letter without modifiers", () => {
    expect(resolveKeyEvent(keyEvent("s"), sample)).toBe(
      "notation.tool.select",
    );
  });

  test("matches space", () => {
    expect(resolveKeyEvent(keyEvent(" "), sample)).toBe("play.toggle");
  });

  test("requires exact modifiers", () => {
    expect(
      resolveKeyEvent(keyEvent("s", { ctrl: true, shift: true }), sample),
    ).toBeNull();
  });

  test("matches redo chord", () => {
    expect(
      resolveKeyEvent(keyEvent("z", { ctrl: true, shift: true }), sample),
    ).toBe("edit.redo");
  });
});
