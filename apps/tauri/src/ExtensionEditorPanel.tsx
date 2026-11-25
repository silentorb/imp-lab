/** Renders an extension's node-type editor for one open document, with its {@link EditorHost}. */

import { useEffect, useMemo, useRef, type ComponentType, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EditorHost, EditorPanelProps, PlaybackStatus } from "imp-lab-ui";
import type { LoadedEditor } from "./extensionUi";
import { componentEntry } from "./extensionUi";
import { useTopic } from "./extensionTopics";
import { GraphErrorBoundary } from "./GraphErrorBoundary";
import { getUserStateEntry, setUserStateEntry } from "./userState";

export type CommandHandler = (commandId: string) => boolean;

type Props = {
  editor: LoadedEditor;
  docId: string;
  value: unknown;
  active: boolean;
  /** Project config path, so extension commands can follow references between records. */
  neighborPath: string | null;
  extensionCommandIds: ReadonlySet<string>;
  statusItems: ReactNode;
  setStatus: (text: string) => void;
  onChange: (next: unknown, coalesceKey?: string) => void;
  /** Register this document's command handler (`null` to remove). */
  setCommandHandler: (docId: string, handler: CommandHandler | null) => void;
};

export function ExtensionEditorPanel(props: Props) {
  const { editor, docId, active, neighborPath, extensionCommandIds, statusItems, setStatus } =
    props;
  const setCommandHandler = useRef(props.setCommandHandler);
  setCommandHandler.current = props.setCommandHandler;
  const handlers = useRef(new Set<CommandHandler>());

  // One handler per document that offers the command to every handler the panel registered.
  useEffect(() => {
    const dispatch: CommandHandler = (id) => [...handlers.current].some((h) => h(id));
    setCommandHandler.current(docId, dispatch);
    return () => setCommandHandler.current(docId, null);
  }, [docId]);

  const host = useMemo<EditorHost>(
    () => ({
      params: editor.params,
      componentId: editor.id,
      active,
      setStatus,
      statusItems,
      hasCommand: (id) => extensionCommandIds.has(id),
      runCommand: (id, documentJson) =>
        invoke("run_extension_command", { id, graphJson: documentJson, neighborPath }),
      playbackStatus: () => invoke<PlaybackStatus>("get_playback_status"),
      sendServiceEvent: async (serviceId, event) => {
        await invoke("send_service_event", { serviceId, eventJson: JSON.stringify(event) });
      },
      requestService: async (serviceId, request) => {
        const reply = await invoke<unknown>("service_request", {
          serviceId,
          requestJson: JSON.stringify(request),
        });
        return typeof reply === "string" ? (JSON.parse(reply) as unknown) : reply;
      },
      useTopic: (topic) => useTopic(topic),
      loadPrefs: <T,>() => getUserStateEntry<T>(componentEntry(editor.id)),
      savePrefs: (value) => setUserStateEntry(componentEntry(editor.id), value),
      onCommand: (handler) => {
        handlers.current.add(handler);
        return () => {
          handlers.current.delete(handler);
        };
      },
    }),
    [active, editor, extensionCommandIds, neighborPath, setStatus, statusItems],
  );

  const Panel = editor.implementation.Panel as ComponentType<EditorPanelProps<unknown>>;
  return (
    <GraphErrorBoundary resetKey={docId} what={`${editor.label} editor`}>
      <Panel value={props.value} onChange={props.onChange} host={host} />
    </GraphErrorBoundary>
  );
}
