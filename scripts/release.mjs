#!/usr/bin/env node
/* MISSION-099 — Release pipeline helper. Single-sources the app version
   across package.json, src-tauri/Cargo.toml and src-tauri/tauri.conf.json,
   scaffolds the CHANGELOG entry for the release from git history, and checks
   that everything is in sync (used as a CI gate).

   Usage:
     node scripts/release.mjs check              — verify all versions match
     node scripts/release.mjs <semver>           — bump to <version> (e.g. 0.2.0)
     node scripts/release.mjs changelog <semver> — scaffold the CHANGELOG entry */

import fs from "node:fs";
import { execSync } from "node:child_process";

const PACKAGE = "package.json";
const CARGO = "src-tauri/Cargo.toml";
const TAURI_CONF = "src-tauri/tauri.conf.json";
const CHANGELOG = "CHANGELOG.md";

const SEMVER = /^\d+\.\d+\.\d+(-[a-z0-9.]+)?$/;

function readJson(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}

function packageVersion() {
  return readJson(PACKAGE).version;
}

function tauriVersion() {
  const match = fs.readFileSync(TAURI_CONF, "utf8").match(/"version":\s*"([^"]+)"/);
  if (!match) throw new Error("tauri.conf.json has no version");
  return match[1];
}

function cargoVersion() {
  const match = fs
    .readFileSync(CARGO, "utf8")
    .match(/^name = "mylore"[\s\S]*?^version = "([^"]+)"/m);
  if (!match) throw new Error("Cargo.toml [lib] block has no version");
  return match[1];
}

/** Rewrite `version = "…"` inside the Cargo.toml [package] block only
    (CRLF-safe: matches across \r\n endings without touching them). */
function setCargoVersion(version) {
  const text = fs.readFileSync(CARGO, "utf8");
  const pattern = /(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m;
  if (!pattern.test(text)) throw new Error("Cargo.toml [package] version not found");
  fs.writeFileSync(CARGO, text.replace(pattern, `$1${version}$2`));
}

function setTauriVersion(version) {
  const text = fs.readFileSync(TAURI_CONF, "utf8");
  fs.writeFileSync(TAURI_CONF, text.replace(/"version":\s*"[^"]+"/, `"version": "${version}"`));
}

function setPackageVersion(version) {
  const pkg = readJson(PACKAGE);
  pkg.version = version;
  fs.writeFileSync(PACKAGE, `${JSON.stringify(pkg, null, 2)}\n`);
}

/** Scaffold a Keep-a-Changelog entry from commits since the last v* tag. */
function scaffoldChangelog(version) {
  let range = "";
  try {
    const tag = execSync("git describe --tags --abbrev=0 --match=v*", { encoding: "utf8" }).trim();
    range = `${tag}..HEAD`;
  } catch {
    /* no tags yet — use the whole history */
  }
  const log = execSync(`git log ${range} --pretty=format:"- %s"`, {
    encoding: "utf8",
  })
    .split("\n")
    .filter((line) => line.trim() && !line.includes("- Merge"))
    .join("\n");

  const today = new Date().toISOString().slice(0, 10);
  const entry = `## [${version}] - ${today}\n\n### Added / Changed / Fixed\n\n${log}\n`;

  let text = fs.existsSync(CHANGELOG) ? fs.readFileSync(CHANGELOG, "utf8") : "";
  if (!text.includes("Keep a Changelog")) {
    text = `# Changelog\n\nAll notable changes to MyLore are documented here.\nFormat: [Keep a Changelog](https://keepachangelog.com), versions follow [SemVer](https://semver.org).\n\n`;
  }
  const insertAt = text.indexOf("## [") === -1 ? text.length : text.indexOf("## [");
  fs.writeFileSync(CHANGELOG, text.slice(0, insertAt) + entry + "\n" + text.slice(insertAt));
  console.log(`CHANGELOG.md: scaffolded [${version}] (${log.split("\n").length} commits)`);
}

const [, , command, arg] = process.argv;

if (command === "check") {
  const versions = new Set([packageVersion(), tauriVersion(), cargoVersion()]);
  if (versions.size !== 1) {
    console.error(
      `version mismatch: package.json=${packageVersion()} tauri.conf.json=${tauriVersion()} Cargo.toml=${cargoVersion()}`,
    );
    process.exit(1);
  }
  console.log(`versions in sync: ${packageVersion()}`);
} else if (command === "changelog" && arg && SEMVER.test(arg)) {
  scaffoldChangelog(arg);
} else if (command && SEMVER.test(command)) {
  setPackageVersion(command);
  setTauriVersion(command);
  setCargoVersion(command);
  console.log(`version bumped to ${command} in package.json, tauri.conf.json, Cargo.toml`);
  console.log("next: cargo check to refresh Cargo.lock, then commit + tag v" + command);
} else {
  console.error("usage: release.mjs check | release.mjs <semver> | release.mjs changelog <semver>");
  process.exit(1);
}
