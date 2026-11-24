/** Extension status items: badges that display a topic's `{text, level}`. */

import { Badge, Group, Tooltip } from "@mantine/core";
import { useTopic } from "../extensionTopics";
import { statusFromTopic, type StatusItemDecl, type StatusLevel } from "./settingsStore";

const LEVEL_COLOR: Record<StatusLevel, string> = {
  ok: "green",
  warn: "yellow",
  error: "red",
  off: "gray",
};

function StatusItem({
  item,
  onOpenCategory,
}: {
  item: StatusItemDecl;
  onOpenCategory: (categoryId: string) => void;
}) {
  const status = statusFromTopic(useTopic(item.topic));
  if (!status) {
    return null;
  }
  const clickable = item.openCategory != null;
  const badge = (
    <Badge
      size="sm"
      variant="light"
      color={LEVEL_COLOR[status.level]}
      style={{ cursor: clickable ? "pointer" : "default", textTransform: "none" }}
      onClick={
        clickable ? () => onOpenCategory(item.openCategory as string) : undefined
      }
    >
      {status.text}
    </Badge>
  );
  return item.label ? (
    <Tooltip label={item.label} openDelay={400}>
      {badge}
    </Tooltip>
  ) : (
    badge
  );
}

export function StatusItems({
  items,
  onOpenCategory,
}: {
  items: StatusItemDecl[];
  onOpenCategory: (categoryId: string) => void;
}) {
  if (items.length === 0) {
    return null;
  }
  return (
    <Group gap={6} wrap="nowrap">
      {items.map((item) => (
        <StatusItem key={item.id} item={item} onOpenCategory={onOpenCategory} />
      ))}
    </Group>
  );
}
