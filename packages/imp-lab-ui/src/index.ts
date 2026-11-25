/**
 * Contract between Imp Lab and an extension's webview UI library.
 *
 * An extension's UI library (`uiLibraryPath` in `extensions.json`) is one ES module that exports
 * `implementations`: UI implementations keyed by implementation id. It contains no wiring.
 * Components in `extensions.json` choose an implementation by `implementationId` and configure
 * it with `params` (for a `node-type-editor`: the Tome node type it edits, the record column,
 * the services it talks to).
 *
 * Libraries must not bundle their own React or Mantine: Lab shares its instances (see
 * {@link SHARED_MODULE_IDS}) on `globalThis[SHARED_GLOBAL]`, and a library's build maps those
 * imports onto that object.
 *
 * Types here are React-free so the package carries no React type root; Lab renders `Panel` as a
 * React component.
 */

/** Global object holding the modules Lab shares with UI libraries, keyed by module id. */
export const SHARED_GLOBAL = "__impLabShared";

/** Module ids Lab shares with UI libraries (import them; don't bundle them). */
export const SHARED_MODULE_IDS = [
  "react",
  "react/jsx-runtime",
  "@mantine/core",
] as const;

export type SharedModuleId = (typeof SHARED_MODULE_IDS)[number];

/** Playback clock of a service that plays documents. */
export type PlaybackStatus = { playing: boolean; positionSecs: number };

/**
 * What Lab gives an editor panel. Calls are scoped to the panel's component (params, user
 * state) and document (commands).
 */
export type EditorHost = {
  /** The component's merged params. A `serviceId` param is the service's implementation id. */
  params: Readonly<Record<string, unknown>>;
  /** Component id (stable across launches). */
  componentId: string;
  /** Whether this panel's document is the active tab. */
  active: boolean;
  /** Footer status line. */
  setStatus(text: string): void;
  /** Extension status items (a React node) for an editor toolbar. */
  statusItems: unknown;
  /** Whether an extension command (`command` component) is available. */
  hasCommand(commandId: string): boolean;
  /** Run an extension command with a document payload (JSON). */
  runCommand(commandId: string, documentJson: string | null): Promise<unknown>;
  /** Clock of the playback service, when one is loaded. */
  playbackStatus(): Promise<PlaybackStatus>;
  /** Fire-and-forget service event (opaque JSON). */
  sendServiceEvent(serviceId: string, event: unknown): Promise<void>;
  /** Service request; resolves to the service's parsed JSON reply. */
  requestService(serviceId: string, request: unknown): Promise<unknown>;
  /** React hook: latest value of an extension topic (`undefined` until published). */
  useTopic(topic: string | null): unknown;
  /** The component's persisted UI prefs (`null` when none are stored). */
  loadPrefs<T>(): Promise<T | null>;
  savePrefs(value: unknown): Promise<void>;
  /**
   * Handle commands (menus, key bindings) while this panel's document is active. Return `true`
   * when handled; Lab then stops. Returns an unsubscribe function.
   */
  onCommand(handler: (commandId: string) => boolean): () => void;
};

export type EditorPanelProps<V> = {
  value: V;
  /** Replace the value. Lab keeps undo history; edits sharing `coalesceKey` are one step. */
  onChange(next: V, coalesceKey?: string): void;
  host: EditorHost;
};

/** Label / value row shown in Lab's Properties pane. */
export type EditorProperty = { label: string; value: string };

/** Implementation of a `node-type-editor` component: edits the value of one record column. */
export type NodeTypeEditorImplementation<V = unknown> = {
  kind: "node-type-editor";
  /** Value of a new record. */
  empty(): V;
  /** Value from the stored column (`null` when absent). Throw when it is invalid. */
  parse(stored: unknown): V;
  /** Column value to store (default: the value itself). */
  serialize?(value: V): unknown;
  /** One-line summary under the tab title. */
  summarize(value: V): string;
  /** Extra Properties rows. */
  properties?(value: V): EditorProperty[];
  /** Value equality for dirty tracking and undo (default: JSON equality). */
  equals?(a: V, b: V): boolean;
  /** React component `(props: EditorPanelProps<V>) => ReactNode`. */
  Panel: (props: EditorPanelProps<V>) => unknown;
};

export type UiImplementation = NodeTypeEditorImplementation<any>;

/** What a UI library module exports. */
export type UiLibrary = {
  implementations: Record<string, UiImplementation>;
};
