// Typed IPC codegen (MISSION-009).
//
// Reads scripts/ipc-contract.json (single source of truth) and emits
// src/api/ipc.generated.ts: typed command wrappers + event helpers.
// Also validates the contract against the Rust `#[command]` handlers so a
// command added or renamed in Rust without a contract update fails here.
//
//   node scripts/codegen-ipc.mjs            # write the generated file
//   node scripts/codegen-ipc.mjs --check    # CI: fail on drift, write nothing

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import prettier from "prettier";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CONTRACT_PATH = path.join(ROOT, "scripts", "ipc-contract.json");
const OUT_PATH = path.join(ROOT, "src", "api", "ipc.generated.ts");
const COMMANDS_DIR = path.join(ROOT, "src-tauri", "src", "commands");

/** Resolve a contract type expression to a TS type. */
function tsType(expr) {
  const arrayMatch = /^(.*)\[\]$/.exec(expr);
  if (arrayMatch) return `${tsType(arrayMatch[1])}[]`;
  const recordMatch = /^Record<\s*string\s*,\s*(.*)\s*>$/.exec(expr);
  if (recordMatch) return `Record<string, ${tsType(recordMatch[1])}>`;
  return expr;
}

/** Collect `#[command] pub fn name` from all Rust files under a directory. */
function findRustCommandNames(dir) {
  const names = new Set();
  const commandRe =
    /\[command\](?:\([^)]*\))?[\s\n]*pub(?:\([^)]*\))?[\s\n]*(?:async[\s\n]+)?fn[\s\n]+(\w+)/g;
  const scan = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        scan(entryPath);
      } else if (entry.name.endsWith(".rs")) {
        const source = readFileSync(entryPath, "utf8");
        for (const match of source.matchAll(commandRe)) names.add(match[1]);
      }
    }
  };
  scan(dir);
  return names;
}

/** Build a JSDoc comment from a contract doc string (or null when absent). */
function jsDoc(doc) {
  if (!doc) return "";
  const lines = doc.split("\n");
  return `/** ${lines.join(" * ")} */`;
}

/** Generate the TS source for a single command wrapper. */
function commandWrapper(command) {
  const lines = [];
  const doc = jsDoc(command.doc);
  if (doc) lines.push(doc);
  const argNames = Object.entries(command.args ?? {});
  const params =
    argNames.length > 0
      ? `args: { ${argNames.map(([n, t]) => `${n}: ${tsType(t)}`).join(", ")} }`
      : "";
  const invokeArgs = argNames.length > 0 ? ", args" : "";
  const ret = tsType(command.returns ?? "void");
  lines.push(`export function ${command.name}(${params}): Promise<${ret}> {`);
  lines.push(`  return invoke<${ret}>("${command.name}"${invokeArgs});`);
  lines.push("}");
  return lines.join("\n");
}

/** Generate the TS source for a single event's listen/emit helpers. */
function eventHelpers(event) {
  const payload = tsType(event.payload ?? "unknown");
  const suffix = event.name
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
  return [
    `export function listen${suffix}(handler: (payload: ${payload}) => void): Promise<UnlistenFn> {`,
    `  return listen<${payload}>("${event.name}", (event) => handler(event.payload));`,
    "}",
    "",
    `export function emit${suffix}(payload: ${payload}): Promise<void> {`,
    `  return emit("${event.name}", payload);`,
    "}",
  ].join("\n");
}

/** Validate the contract against the Rust command surface; return error list. */
function validate(contract, rustNames) {
  const errors = [];
  const seen = new Set();
  for (const command of contract.commands) {
    if (seen.has(command.name)) errors.push(`Duplicate command in contract: ${command.name}`);
    seen.add(command.name);
    if (!rustNames.has(command.name)) {
      errors.push(`Command "${command.name}" is in the contract but has no #[command] fn in Rust.`);
    }
  }
  for (const rustName of rustNames) {
    if (!seen.has(rustName)) {
      errors.push(`Command "${rustName}" exists in Rust but is missing from the contract.`);
    }
  }
  return errors;
}

/** Render the full generated TS module. */
function render(contract) {
  const sections = [
    "// AUTO-GENERATED — do not edit. Regenerate with `npm run codegen`.",
    "// Source of truth: scripts/ipc-contract.json",
    "",
    'import { invoke } from "@tauri-apps/api/core";',
  ];
  if ((contract.events ?? []).length > 0) {
    sections.push('import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";');
  }
  sections.push("");
  for (const command of contract.commands) sections.push(commandWrapper(command), "");
  for (const event of contract.events ?? []) sections.push(eventHelpers(event), "");
  return `${sections.join("\n").trimEnd()}\n`;
}

async function main() {
  const checkMode = process.argv.includes("--check");
  const contract = JSON.parse(readFileSync(CONTRACT_PATH, "utf8"));
  const rustNames = findRustCommandNames(COMMANDS_DIR);
  const errors = validate(contract, rustNames);
  if (errors.length > 0) {
    console.error("IPC codegen failed validation:");
    for (const error of errors) console.error(`  - ${error}`);
    process.exit(1);
  }

  const config = await prettier.resolveConfig(OUT_PATH);
  let formatted = render(contract);
  const options = { parser: "typescript", ...config };
  while (true) {
    const next = await prettier.format(formatted, options);
    if (next === formatted) break;
    formatted = next;
  }
  if (checkMode) {
    const existing = readFileSync(OUT_PATH, "utf8");
    if (existing === formatted) {
      console.log("IPC codegen: src/api/ipc.generated.ts is up to date.");
      process.exit(0);
    }
    console.error(
      "IPC codegen: src/api/ipc.generated.ts is out of date. Run `npm run codegen` and commit the change.",
    );
    process.exit(1);
  }

  writeFileSync(OUT_PATH, formatted);
  console.log(`IPC codegen: wrote ${path.relative(ROOT, OUT_PATH)}`);
}

await main();
