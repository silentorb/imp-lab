/** Settings dialog: one page per settings category, plus the fallback page. */

import { useEffect, useMemo, useState } from "react";
import {
  Chip,
  Group,
  Modal,
  NavLink,
  ScrollArea,
  Select,
  Stack,
  Switch,
  Text,
} from "@mantine/core";
import { useTopic } from "../extensionTopics";
import {
  optionsFromTopic,
  type SettingDecl,
  type SettingsLayout,
  type SettingValues,
} from "./settingsStore";

export type SettingsModalProps = {
  opened: boolean;
  layout: SettingsLayout;
  values: SettingValues;
  /** Category page to show on open (`null` = first page). */
  initialCategory: string | null;
  onChange: (settingId: string, value: unknown) => void;
  onClose: () => void;
};

function pageKey(categoryId: string | null): string {
  return categoryId ?? "\u0000other";
}

function ChoiceField({
  decl,
  value,
  onChange,
}: {
  decl: SettingDecl;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const live = useTopic(decl.optionsTopic);
  const options = decl.options ?? optionsFromTopic(live);
  const selected = typeof value === "string" ? value : "";
  const data = options.map((o) => ({ value: o.value, label: o.label }));
  if (selected !== "" && !options.some((o) => o.value === selected)) {
    data.push({ value: selected, label: `${selected} (unavailable)` });
  }
  return (
    <Select
      label={decl.label}
      description={decl.description ?? undefined}
      data={data}
      value={data.some((d) => d.value === selected) ? selected : null}
      allowDeselect={false}
      onChange={(next) => {
        if (next != null) {
          onChange(next);
        }
      }}
    />
  );
}

function MultiChoiceField({
  decl,
  value,
  onChange,
}: {
  decl: SettingDecl;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const live = useTopic(decl.optionsTopic);
  const options = decl.options ?? optionsFromTopic(live);
  const selected = Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];
  const min = decl.minSelected ?? 0;
  return (
    <Stack gap={4}>
      <Text size="sm" fw={500}>
        {decl.label}
      </Text>
      {decl.description ? (
        <Text size="xs" c="dimmed">
          {decl.description}
        </Text>
      ) : null}
      <Chip.Group
        multiple
        value={selected}
        onChange={(next) => {
          if (next.length >= min) {
            // Keep declaration order, not click order.
            const order = options.map((o) => o.value);
            onChange([...next].sort((a, b) => order.indexOf(a) - order.indexOf(b)));
          }
        }}
      >
        <Group gap={4}>
          {options.map((o) => (
            <Chip key={o.value} value={o.value} size="xs">
              {o.label}
            </Chip>
          ))}
        </Group>
      </Chip.Group>
    </Stack>
  );
}

function SettingField({
  decl,
  value,
  onChange,
}: {
  decl: SettingDecl;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  switch (decl.valueType) {
    case "choice":
      return <ChoiceField decl={decl} value={value} onChange={onChange} />;
    case "multi-choice":
      return <MultiChoiceField decl={decl} value={value} onChange={onChange} />;
    case "boolean":
      return (
        <Switch
          label={decl.label}
          description={decl.description ?? undefined}
          checked={value === true}
          onChange={(event) => onChange(event.currentTarget.checked)}
        />
      );
  }
}

export function SettingsModal({
  opened,
  layout,
  values,
  initialCategory,
  onChange,
  onClose,
}: SettingsModalProps) {
  const pages = layout.pages;
  const [active, setActive] = useState<string>(pageKey(null));

  useEffect(() => {
    if (!opened) {
      return;
    }
    const wanted = pages.find((p) => p.categoryId === initialCategory) ?? pages[0];
    if (wanted) {
      setActive(pageKey(wanted.categoryId));
    }
  }, [opened, initialCategory, pages]);

  const page = useMemo(
    () => pages.find((p) => pageKey(p.categoryId) === active) ?? pages[0],
    [active, pages],
  );

  return (
    <Modal opened={opened} onClose={onClose} title="Settings" size="xl">
      {pages.length === 0 ? (
        <Text size="sm" c="dimmed">
          No settings. Extension packs can add settings here.
        </Text>
      ) : (
        <Group align="flex-start" wrap="nowrap" gap="md">
          <Stack gap={2} w={180} style={{ flexShrink: 0 }}>
            {pages.map((p) => (
              <NavLink
                key={pageKey(p.categoryId)}
                label={p.title}
                active={page != null && pageKey(p.categoryId) === pageKey(page.categoryId)}
                onClick={() => setActive(pageKey(p.categoryId))}
              />
            ))}
          </Stack>
          <ScrollArea.Autosize mah={480} style={{ flex: 1 }}>
            <Stack gap="md" pr="sm">
              {page?.settings.length ? (
                page.settings.map((decl) => (
                  <SettingField
                    key={decl.id}
                    decl={decl}
                    value={values[decl.id]}
                    onChange={(value) => onChange(decl.id, value)}
                  />
                ))
              ) : (
                <Text size="sm" c="dimmed">
                  Nothing on this page yet.
                </Text>
              )}
            </Stack>
          </ScrollArea.Autosize>
        </Group>
      )}
    </Modal>
  );
}
