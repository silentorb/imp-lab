import { coreNodeLibrary, type ImpFile, type NodeLibrary } from "imp-core-types";
import { parseImpFile } from "imp-file";
import { createRegistry, loadImpFile, loadNodeLibrary, type Registry } from "imp-registry";

export type ExtensionCatalogs = {
  nodeLibraries?: unknown[];
  /** Imp files (types and node definitions) registered by extensions. */
  impFiles?: unknown[];
};

/** Imp Lab host registry — Imp core, plus catalogs from optional extensions. */
export function getLabRegistry(catalogs?: ExtensionCatalogs | null): Registry {
  let registry = loadNodeLibrary(createRegistry(), coreNodeLibrary);
  if (!catalogs) {
    return registry;
  }
  for (const library of catalogs.nodeLibraries ?? []) {
    registry = loadNodeLibrary(registry, library as NodeLibrary);
  }
  for (const file of catalogs.impFiles ?? []) {
    registry = loadImpFile(registry, parseImpFile(file) as ImpFile);
  }
  return registry;
}

/** Core-only registry for tests / before extensions load. */
export const labRegistry: Registry = getLabRegistry();
