/**
 * Extension UI libraries: load each library the enabled UI components need, and resolve every
 * `node-type-editor` component to the implementation its library exports.
 *
 * A library is one ES module (`uiLibraryPath`) exporting `implementations` by id; components in
 * `extensions.json` choose and configure them. Lab shares its React and Mantine with libraries
 * through `globalThis[SHARED_GLOBAL]` so a library never bundles its own copy.
 */

import * as React from "react";
import * as jsxRuntime from "react/jsx-runtime";
import * as Mantine from "@mantine/core";
import {
  SHARED_GLOBAL,
  type NodeTypeEditorImplementation,
  type SharedModuleId,
  type UiLibrary,
} from "imp-lab-ui";

/** `node-type-editor` component as the host reports it. */
export type NodeTypeEditorDecl = {
  id: string;
  extensionId: string;
  implementationId: string;
  label: string;
  nodeTypeId: string;
  typeTitle: string;
  column: string;
  params: Record<string, unknown>;
};

export type UiExtensionsDto = {
  libraries: Array<{ extensionId: string }>;
  editors: NodeTypeEditorDecl[];
  warnings?: string[];
};

/** An editor component with its implementation. */
export type LoadedEditor = NodeTypeEditorDecl & {
  implementation: NodeTypeEditorImplementation<unknown>;
};

export type LoadedUi = { editors: LoadedEditor[]; warnings: string[] };

export const EMPTY_UI: LoadedUi = { editors: [], warnings: [] };

/** Publish the modules Lab shares with UI libraries. */
export function installSharedModules(): void {
  const shared: Record<SharedModuleId, unknown> = {
    react: React,
    "react/jsx-runtime": jsxRuntime,
    "@mantine/core": Mantine,
  };
  (globalThis as Record<string, unknown>)[SHARED_GLOBAL] = shared;
}

function isLibrary(value: unknown): value is UiLibrary {
  return (
    !!value &&
    typeof value === "object" &&
    !!(value as UiLibrary).implementations &&
    typeof (value as UiLibrary).implementations === "object"
  );
}

/** Match editor components to the implementations of the libraries that loaded. */
export function resolveEditors(
  dto: UiExtensionsDto,
  libraries: ReadonlyMap<string, unknown>,
): LoadedUi {
  const warnings = [...(dto.warnings ?? [])];
  const editors: LoadedEditor[] = [];
  for (const decl of dto.editors) {
    const library = libraries.get(decl.extensionId);
    if (!isLibrary(library)) {
      warnings.push(`node-type-editor \`${decl.id}\`: UI library of \`${decl.extensionId}\` did not load`);
      continue;
    }
    const implementation = library.implementations[decl.implementationId];
    if (!implementation || implementation.kind !== "node-type-editor") {
      warnings.push(
        `node-type-editor \`${decl.id}\`: \`${decl.extensionId}\` exports no node-type-editor \`${decl.implementationId}\``,
      );
      continue;
    }
    editors.push({ ...decl, implementation });
  }
  return { editors, warnings };
}

export type UiLoaderIo = {
  getUiExtensions(): Promise<UiExtensionsDto>;
  readLibrary(extensionId: string): Promise<string>;
  importModule(source: string): Promise<unknown>;
};

/** Import an ES module from source text. */
export async function importModuleSource(source: string): Promise<unknown> {
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  try {
    return await import(/* @vite-ignore */ url);
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Load every UI library the enabled components need and resolve their editors. */
export async function loadUi(io: UiLoaderIo): Promise<LoadedUi> {
  const dto = await io.getUiExtensions();
  const libraries = new Map<string, unknown>();
  const warnings: string[] = [];
  for (const { extensionId } of dto.libraries) {
    try {
      libraries.set(extensionId, await io.importModule(await io.readLibrary(extensionId)));
    } catch (err) {
      warnings.push(`UI library of \`${extensionId}\` failed to load: ${String(err)}`);
    }
  }
  const resolved = resolveEditors(dto, libraries);
  return { editors: resolved.editors, warnings: [...warnings, ...resolved.warnings] };
}

/** Command id opening a new record in an editor component (matches the host menu). */
export function newRecordCommand(editorId: string): string {
  return `document.new:${editorId}`;
}

/** User-state entry of a component's own prefs. */
export function componentEntry(componentId: string): string {
  return `component:${componentId}`;
}
