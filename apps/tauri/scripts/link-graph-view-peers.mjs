#!/usr/bin/env node
/**
 * TypeScript resolves workspace `imp-graph-view` source from that package's
 * directory, so peer deps must be visible under its `node_modules`. Bun hoists
 * peers into this app's `node_modules`, which is not an ancestor of
 * `imp-graph-view`. Symlink the shared peers (and @types) so `tsc` sees one
 * React / React Flow type root.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const hostNm = path.join(appRoot, "node_modules");
const graphViewRoot = path.resolve(appRoot, "../../../imp-graph-view");
const graphNm = path.join(graphViewRoot, "node_modules");

const links = [
  ["@xyflow", path.join(hostNm, "@xyflow")],
  ["react", path.join(hostNm, "react")],
  ["react-dom", path.join(hostNm, "react-dom")],
  [path.join("@types", "react"), path.join(hostNm, "@types", "react")],
  [path.join("@types", "react-dom"), path.join(hostNm, "@types", "react-dom")],
];

if (!fs.existsSync(graphViewRoot)) {
  console.warn(
    "link-graph-view-peers: imp-graph-view not found; skipping peer links",
  );
  process.exit(0);
}

fs.mkdirSync(path.join(graphNm, "@types"), { recursive: true });

for (const [rel, target] of links) {
  if (!fs.existsSync(target)) {
    console.warn(`link-graph-view-peers: missing ${target}; skip ${rel}`);
    continue;
  }
  const dest = path.join(graphNm, rel);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.rmSync(dest, { recursive: true, force: true });
  fs.symlinkSync(target, dest, "junction");
}

console.log("link-graph-view-peers: linked react / @xyflow peers into imp-graph-view");
