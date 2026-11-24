import { describe, expect, test } from "bun:test";
import {
  deliverSettings,
  effectiveValues,
  optionsFromTopic,
  statusFromTopic,
  validateValue,
  valuesByService,
  type SettingDecl,
  type SettingsLayout,
} from "./settingsStore";

const device: SettingDecl = {
  id: "aura.midi.input",
  extensionId: "aura-playback",
  label: "Input device",
  description: null,
  valueType: "choice",
  default: "",
  options: null,
  optionsTopic: "aura.midi.ports",
  minSelected: null,
  serviceId: "playback",
};

const channels: SettingDecl = {
  ...device,
  id: "aura.midi.channels",
  label: "Channels",
  valueType: "multi-choice",
  default: ["1", "2"],
  options: [
    { value: "1", label: "1" },
    { value: "2", label: "2" },
  ],
  optionsTopic: null,
  minSelected: 1,
};

const flag: SettingDecl = {
  ...device,
  id: "other.flag",
  label: "Flag",
  valueType: "boolean",
  default: false,
  optionsTopic: null,
  serviceId: "other",
};

const layout: SettingsLayout = {
  pages: [
    { categoryId: "aura.midi", title: "MIDI", settings: [device, channels] },
    { categoryId: "general", title: "General", settings: [device] },
    { categoryId: null, title: "Other", settings: [flag] },
  ],
  warnings: [],
};

describe("settings values", () => {
  test("validation keeps fitting values and falls back to defaults", () => {
    expect(validateValue(device, "Keys")).toBe("Keys");
    expect(validateValue(device, 3)).toBe("");
    expect(validateValue(channels, ["2", "9"])).toEqual(["2"]);
    expect(validateValue(channels, [])).toEqual(["1", "2"]);
    expect(validateValue(flag, "yes")).toBe(false);
  });

  test("effective values merge stored over defaults, once per setting", () => {
    const values = effectiveValues(layout, { "aura.midi.input": "Keys", stale: 1 });
    expect(values).toEqual({
      "aura.midi.input": "Keys",
      "aura.midi.channels": ["1", "2"],
      "other.flag": false,
    });
  });

  test("values group by owning service and deliver as one event each", async () => {
    const values = effectiveValues(layout, null);
    expect(Object.keys(valuesByService(layout, values)).sort()).toEqual(["other", "playback"]);
    const sent: Array<{ serviceId: string; event: unknown }> = [];
    await deliverSettings(
      async (command, args) => {
        expect(command).toBe("send_service_event");
        sent.push({ serviceId: args.serviceId, event: JSON.parse(args.eventJson) });
        if (args.serviceId === "other") {
          throw new Error("not registered");
        }
      },
      layout,
      values,
    );
    expect(sent.find((s) => s.serviceId === "playback")?.event).toEqual({
      type: "settings",
      values: { "aura.midi.input": "", "aura.midi.channels": ["1", "2"] },
    });
    sent.length = 0;
    await deliverSettings(async (_c, args) => void sent.push({ serviceId: args.serviceId, event: null }), layout, values, ["other"]);
    expect(sent.map((s) => s.serviceId)).toEqual(["other"]);
  });

  test("topic shapes", () => {
    expect(optionsFromTopic([{ value: "", label: "None" }, { value: "K" }, 3])).toEqual([
      { value: "", label: "None" },
      { value: "K", label: "K" },
    ]);
    expect(optionsFromTopic(null)).toEqual([]);
    expect(statusFromTopic({ text: "MIDI: off", level: "off" })).toEqual({
      text: "MIDI: off",
      level: "off",
    });
    expect(statusFromTopic({ text: "x", level: "weird" })?.level).toBe("off");
    expect(statusFromTopic({ level: "ok" })).toBeNull();
  });
});
