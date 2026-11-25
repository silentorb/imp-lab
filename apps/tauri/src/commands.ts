/** Command id strings aligned with the Rust menu. */

export const CMD_PROJECT_NEW = "project.new";
export const CMD_PROJECT_OPEN = "project.open";
export const CMD_PROJECT_MANAGE_STORES = "project.manage-stores";
export const CMD_RECORDS_FOCUS_FILTER = "records.focus-filter";
/** Quick-open over the project's records (scales past browsing the explorer). */
export const CMD_RECORDS_SEARCH = "records.search";
/** Open the active graph's node-type picker. */
export const CMD_GRAPH_ADD_NODE = "graph.add-node";
export const CMD_DOCUMENT_NEW_GRAPH = "document.new.graph";
export const CMD_DOCUMENT_SAVE = "document.save";
export const CMD_DOCUMENT_DUPLICATE = "document.duplicate-to-store";
export const CMD_DOCUMENT_CLOSE_ACTIVE = "document.close_active";
export const CMD_EDIT_UNDO = "edit.undo";
export const CMD_EDIT_REDO = "edit.redo";
export const CMD_VIEW_TOGGLE_COLOR_SCHEME = "view.toggle-color-scheme";

/** Extension-contributed play commands (present when a playback pack is loaded). */
export const CMD_PLAY_START = "play.start";
export const CMD_PLAY_STOP = "play.stop";
export const CMD_PLAY_TOGGLE = "play.toggle";

/** Channel for native menu → React command dispatch. */

export const EDITOR_COMMAND_EVENT = "editor://command";

export type EditorCommandPayload = {
  id: string;
};

/** Lab settings dialog (extension settings, one page per category). */
export const CMD_SETTINGS_OPEN = "settings.open";
