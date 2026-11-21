import { coreNodeLibrary, type GraphLibrary, type NodeLibrary, type TypeConstraintLibrary } from "imp-core-types";
import {
  createRegistry,
  loadGraphLibrary,
  loadNodeLibrary,
  loadTypeConstraintLibrary,
  type Registry,
} from "imp-registry";

export type ExtensionCatalogs = {
  nodeLibraries?: unknown[];
  typeConstraintLibraries?: unknown[];
  graphLibraries?: unknown[];
};

/** Lab Tauri host registry — Imp core, plus catalogs from optional extensions. */
export function getLabRegistry(catalogs?: ExtensionCatalogs | null): Registry {
  let registry = loadNodeLibrary(createRegistry(), coreNodeLibrary);
  if (!catalogs) {
    return registry;
  }
  for (const library of catalogs.nodeLibraries ?? []) {
    registry = loadNodeLibrary(registry, library as NodeLibrary);
  }
  for (const library of catalogs.typeConstraintLibraries ?? []) {
    registry = loadTypeConstraintLibrary(
      registry,
      library as TypeConstraintLibrary,
    );
  }
  for (const library of catalogs.graphLibraries ?? []) {
    registry = loadGraphLibrary(registry, library as GraphLibrary);
  }
  return registry;
}

/** Core-only registry for tests / before extensions load. */
export const labRegistry: Registry = getLabRegistry();
