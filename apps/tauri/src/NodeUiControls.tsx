/** Properties controls for extension-declared node UI (see `nodeUi.ts`). */

import { useState } from "react";
import { Button, Select, Stack, Text } from "@mantine/core";
import { useTopic } from "./extensionTopics";
import { optionsFromTopic } from "./settings/settingsStore";
import { runNodeAction, type ActionInvoke, type ActionNode, type NodeActionDecl } from "./nodeUi";

/** A literal chosen from a topic's `[{value, label}]`; an unlisted current value stays shown. */
export function LiteralOptionsSelect(props: {
  label: string;
  topic: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const { label, topic, value, onChange } = props;
  const options = optionsFromTopic(useTopic(topic));
  const data =
    value && !options.some((o) => o.value === value)
      ? [{ value, label: `${value} (not found)` }, ...options]
      : options;
  return (
    <Select
      size="xs"
      label={label}
      data={data}
      value={value || null}
      searchable
      clearable
      nothingFoundMessage={options.length === 0 ? "None available" : "No match"}
      placeholder={options.length === 0 ? "None available" : "Choose…"}
      onChange={(next) => onChange(next ?? "")}
    />
  );
}

/** Buttons for a node's declared actions; a failed action shows its message below. */
export function NodeActions(props: {
  actions: NodeActionDecl[];
  node: ActionNode;
  invoke: ActionInvoke;
  /** Called before an action is sent (the host notes which document the node is in). */
  onRun?: (action: NodeActionDecl) => void;
}) {
  const { actions, node, invoke, onRun } = props;
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<{ node: string; message: string } | null>(null);
  return (
    <Stack gap={4} mt={4}>
      {actions.map((action) => (
        <Button
          key={action.id}
          size="xs"
          variant="light"
          loading={busy === action.id}
          onClick={() => {
            onRun?.(action);
            setBusy(action.id);
            setError(null);
            void runNodeAction(invoke, action, node).then((message) => {
              setBusy(null);
              if (message) {
                setError({ node: node.id, message });
              }
            });
          }}
        >
          {action.label}
        </Button>
      ))}
      {error && error.node === node.id ? (
        <Text size="xs" c="red">
          {error.message}
        </Text>
      ) : null}
    </Stack>
  );
}
