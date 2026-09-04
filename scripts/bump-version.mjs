#!/usr/bin/env node
// Single source of truth for the SecretSticky version.
//
//   node scripts/bump-version.mjs 0.1.7
//   npm run bump:version -- 0.1.7
//
// Keeps every version surface in step so nothing else in the repo needs to
// hardcode the version number:
//   - package.json        (npm)
//   - package-lock.json   (npm lockfile root entry)
//   - src-tauri/Cargo.toml ([package] version)
//   - src-tauri/tauri.conf.json (product/installer/updater version)
//
// Cargo.lock is refreshed by the next `cargo` invocation (check/build/test).
// Docs (CHANGELOG.md etc.) are updated by hand — this script never guesses them.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const next = process.argv[2];

if (!/^\d+\.\d+\.\d+$/.test(next ?? "")) {
  console.error(`usage: node scripts/bump-version.mjs <X.Y.Z>   (got ${JSON.stringify(next)})`);
  process.exit(1);
}

function readJson(rel) {
  return JSON.parse(readFileSync(join(root, rel), "utf8"));
}

function writeJson(rel, obj) {
  writeFileSync(join(root, rel), JSON.stringify(obj, null, 2) + "\n", "utf8");
}

// npm surfaces (package-lock mirrors the root package version)
for (const rel of ["package.json", "package-lock.json"]) {
  const json = readJson(rel);
  if (typeof json.version !== "string") {
    console.error(`No "version" at root of ${rel}`);
    process.exit(1);
  }
  json.version = next;
  if (rel === "package-lock.json" && json.packages?.[""]?.version) {
    json.packages[""].version = next;
  }
  writeJson(rel, json);
}

// Tauri product version (installer + updater channel)
const confRel = "src-tauri/tauri.conf.json";
const conf = readJson(confRel);
if (typeof conf.version !== "string") {
  console.error(`No "version" in ${confRel}`);
  process.exit(1);
}
conf.version = next;
writeJson(confRel, conf);

// Cargo [package] version — first bare `version = "x.y.z"` line is the package
const cargoRel = "src-tauri/Cargo.toml";
const cargoPath = join(root, cargoRel);
const cargo = readFileSync(cargoPath, "utf8");
const bumped = cargo.replace(/^version = "\d+\.\d+\.\d+"$/m, `version = "${next}"`);
if (bumped === cargo || !bumped.includes(`version = "${next}"`)) {
  console.error(`Could not bump [package] version in ${cargoRel}`);
  process.exit(1);
}
writeFileSync(cargoPath, bumped, "utf8");

console.log(`bumped SecretSticky to ${next} across package.json, package-lock.json, ${confRel}, ${cargoRel}`);
console.log("Next: update CHANGELOG.md, run the CI gates, commit, tag v" + next + ".");
