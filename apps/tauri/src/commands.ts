/** Command id strings aligned with the Rust menu. */

export const CMD_DOCUMENT_OPEN_FILE = "document.open-file";
export const CMD_DOCUMENT_NEW_GRAPH = "document.new.graph";
export const CMD_DOCUMENT_SAVE = "document.save";
export const CMD_DOCUMENT_CLOSE_ACTIVE = "document.close_active";
export const CMD_VIEW_TOGGLE_COLOR_SCHEME = "view.toggle-color-scheme";

/** Channel for native menu → React command dispatch. */

export const EDITOR_COMMAND_EVENT = "editor://command";

export type EditorCommandPayload = {
  id: string;
};
