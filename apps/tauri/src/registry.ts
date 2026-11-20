import { coreNodeLibrary } from "imp-core-types";
import { createRegistry, loadNodeLibrary, type Registry } from "imp-registry";

/** Lab Tauri host registry — Imp core types only (no Aura catalogs). */
export function getLabRegistry(): Registry {
  return loadNodeLibrary(createRegistry(), coreNodeLibrary);
}

export const labRegistry: Registry = getLabRegistry();
