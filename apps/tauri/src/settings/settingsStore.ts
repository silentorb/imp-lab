/**
 * Extension settings: declarations come from the host (`get_settings_layout`), values are opaque
 * JSON that Lab stores (`extension-settings` user-state entry) and delivers to each owning
 * service as one `settings` event.
 */

export const ENTRY_EXTENSION_SETTINGS = "extension-settings";

export type SettingValueType = "choice" | "multi-choice" | "boolean";

export type SettingOption = { value: string; label: string };

export type SettingDecl = {
  id: string;
  extensionId: string;
  label: string;
  description: string | null;
  valueType: SettingValueType;
  default: unknown;
  options: SettingOption[] | null;
  optionsTopic: string | null;
  minSelected: number | null;
  serviceId: string | null;
};

export type SettingsPage = {
  /** `null` for the fallback page (settings no category claims). */
  categoryId: string | null;
  title: string;
  settings: SettingDecl[];
};

export type SettingsLayout = { pages: SettingsPage[]; warnings: string[] };

export type SettingValues = Record<string, unknown>;

export const EMPTY_LAYOUT: SettingsLayout = { pages: [], warnings: [] };

/** Every setting once (a setting may appear on several pages). */
export function allSettings(layout: SettingsLayout): SettingDecl[] {
  const seen = new Map<string, SettingDecl>();
  for (const page of layout.pages) {
    for (const setting of page.settings) {
      if (!seen.has(setting.id)) {
        seen.set(setting.id, setting);
      }
    }
  }
  return [...seen.values()];
}

/** Options from a topic value (`[{value, label}]`); anything else is no options. */
export function optionsFromTopic(value: unknown): SettingOption[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.flatMap((item) => {
    if (!item || typeof item !== "object") {
      return [];
    }
    const { value: v, label } = item as Record<string, unknown>;
    return typeof v === "string"
      ? [{ value: v, label: typeof label === "string" ? label : v }]
      : [];
  });
}

/**
 * `value` if it fits the declaration, else the default. A choice keeps a value missing from the
 * current options (a device that is unplugged stays selected).
 */
export function validateValue(decl: SettingDecl, value: unknown): unknown {
  switch (decl.valueType) {
    case "boolean":
      return typeof value === "boolean" ? value : decl.default;
    case "choice":
      return typeof value === "string" ? value : decl.default;
    case "multi-choice": {
      if (!Array.isArray(value) || !value.every((v) => typeof v === "string")) {
        return decl.default;
      }
      const known = decl.options ? new Set(decl.options.map((o) => o.value)) : null;
      const kept = (value as string[]).filter((v) => !known || known.has(v));
      if (decl.minSelected != null && kept.length < decl.minSelected) {
        return decl.default;
      }
      return kept;
    }
  }
}

/** Stored values merged over defaults, validated. */
export function effectiveValues(
  layout: SettingsLayout,
  stored: SettingValues | null | undefined,
): SettingValues {
  const values: SettingValues = {};
  for (const decl of allSettings(layout)) {
    values[decl.id] =
      stored && decl.id in stored
        ? validateValue(decl, stored[decl.id])
        : decl.default;
  }
  return values;
}

/** Values grouped by the service that receives them. */
export function valuesByService(
  layout: SettingsLayout,
  values: SettingValues,
): Record<string, SettingValues> {
  const grouped: Record<string, SettingValues> = {};
  for (const decl of allSettings(layout)) {
    if (!decl.serviceId) {
      continue;
    }
    grouped[decl.serviceId] ??= {};
    grouped[decl.serviceId]![decl.id] = values[decl.id];
  }
  return grouped;
}

export type SettingsInvoke = (
  command: "send_service_event",
  args: { serviceId: string; eventJson: string },
) => Promise<unknown>;

/**
 * Sends each owning service its settings' values (`{"type":"settings","values"}`). Limited to
 * `serviceIds` when given. A missing service is skipped quietly.
 */
export async function deliverSettings(
  invoke: SettingsInvoke,
  layout: SettingsLayout,
  values: SettingValues,
  serviceIds?: string[],
): Promise<void> {
  const grouped = valuesByService(layout, values);
  await Promise.all(
    Object.entries(grouped)
      .filter(([serviceId]) => !serviceIds || serviceIds.includes(serviceId))
      .map(([serviceId, serviceValues]) =>
        invoke("send_service_event", {
          serviceId,
          eventJson: JSON.stringify({ type: "settings", values: serviceValues }),
        }).catch((err: unknown) => {
          console.warn(`settings not delivered to ${serviceId}:`, err);
        }),
      ),
  );
}

export type StatusLevel = "ok" | "warn" | "error" | "off";

export type StatusItemDecl = {
  id: string;
  label: string | null;
  topic: string;
  openCategory: string | null;
  order: number;
};

export type StatusValue = { text: string; level: StatusLevel };

/** A status topic value, or `null` when it isn't one. */
export function statusFromTopic(value: unknown): StatusValue | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const { text, level } = value as Record<string, unknown>;
  if (typeof text !== "string") {
    return null;
  }
  return {
    text,
    level:
      level === "ok" || level === "warn" || level === "error" ? level : "off",
  };
}
