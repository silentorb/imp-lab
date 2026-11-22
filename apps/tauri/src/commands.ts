/** Command id strings aligned with the Rust menu. */

export const CMD_PROJECT_NEW = "project.new";
export const CMD_PROJECT_OPEN = "project.open";
export const CMD_PROJECT_MANAGE_STORES = "project.manage-stores";
export const CMD_RECORDS_FOCUS_FILTER = "records.focus-filter";
export const CMD_DOCUMENT_NEW_GRAPH = "document.new.graph";
export const CMD_DOCUMENT_NEW_NOTATION = "document.new.notation";
export const CMD_DOCUMENT_SAVE = "document.save";
export const CMD_DOCUMENT_DUPLICATE = "document.duplicate-to-store";
export const CMD_DOCUMENT_CLOSE_ACTIVE = "document.close_active";
export const CMD_EDIT_UNDO = "edit.undo";
export const CMD_EDIT_REDO = "edit.redo";
export const CMD_VIEW_TOGGLE_COLOR_SCHEME = "view.toggle-color-scheme";

export const CMD_NOTATION_TOOL_DRAW = "notation.tool.draw";
export const CMD_NOTATION_TOOL_SELECT = "notation.tool.select";
export const CMD_NOTATION_TOOL_ERASE = "notation.tool.erase";
export const CMD_NOTATION_SNAP_TOGGLE = "notation.snap.toggle";

/** Extension-contributed play commands (present when Aura playback pack is loaded). */
export const CMD_PLAY_START = "play.start";
export const CMD_PLAY_STOP = "play.stop";
export const CMD_PLAY_TOGGLE = "play.toggle";

/** Channel for native menu → React command dispatch. */

export const EDITOR_COMMAND_EVENT = "editor://command";

export type EditorCommandPayload = {
  id: string;
};
