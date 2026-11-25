import { describe, expect, test } from "bun:test";
import { loadUi, resolveEditors, type UiExtensionsDto } from "./extensionUi";

const sketch = {
  kind: "node-type-editor" as const,
  empty: () => ({ strokes: [] }),
  parse: (v: unknown) => v,
  summarize: () => "sketch",
  Panel: () => null,
};

const dto: UiExtensionsDto = {
  libraries: [{ extensionId: "ui" }, { extensionId: "broken" }],
  editors: [
    {
      id: "sketch-editor",
      extensionId: "ui",
      implementationId: "sketch",
      label: "Sketch",
      nodeTypeId: "01SKETCH",
      typeTitle: "Sketches",
      column: "sketch",
      params: {},
    },
    {
      id: "missing-impl",
      extensionId: "ui",
      implementationId: "nope",
      label: "Nope",
      nodeTypeId: "01NOPE",
      typeTitle: "Nopes",
      column: "x",
      params: {},
    },
    {
      id: "from-broken",
      extensionId: "broken",
      implementationId: "sketch",
      label: "B",
      nodeTypeId: "01B",
      typeTitle: "Bs",
      column: "x",
      params: {},
    },
  ],
  warnings: ["host warning"],
};

describe("extension UI", () => {
  test("resolves editors to exported implementations", () => {
    const ui = resolveEditors(dto, new Map([["ui", { implementations: { sketch } }]]));
    expect(ui.editors.map((e) => e.id)).toEqual(["sketch-editor"]);
    expect(ui.editors[0]!.implementation).toBe(sketch);
    expect(ui.warnings).toHaveLength(3);
  });

  test("a library that fails to load only drops its own editors", async () => {
    const ui = await loadUi({
      getUiExtensions: async () => dto,
      readLibrary: async (id) => id,
      importModule: async (source) => {
        if (source === "broken") throw new Error("syntax");
        return { implementations: { sketch } };
      },
    });
    expect(ui.editors.map((e) => e.id)).toEqual(["sketch-editor"]);
    expect(ui.warnings.some((w) => w.includes("failed to load"))).toBe(true);
  });
});
