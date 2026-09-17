/* MISSION-118 — Release-binary size budget.

   The `p2p` feature is optional (yrs + chacha20poly1305 + nostr-sdk), and an
   optional feature is only honest if its cost is known. This builds the app both
   ways in release mode, reports the sizes and the delta, and fails when either
   exceeds its budget — so a dependency creeping into the default build, or the
   p2p feature doubling the binary, is caught before it ships.

   Run with `npm run size:budget`. It compiles twice from scratch, so it is a
   release gate, not part of the everyday test loop (TESTING.md §size). */

import { execFileSync } from "node:child_process";
import { statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const crate = path.join(root, "src-tauri");
const isWindows = process.platform === "win32";
const binary = path.join(crate, "target", "release", isWindows ? "mylore.exe" : "mylore");

/** Budgets in bytes. The default build carries no sync dependencies; the p2p
 *  feature may add a quarter more and no more. */
export const BUDGETS = {
  default: 40 * 1024 * 1024,
  p2p: 52 * 1024 * 1024,
  /** How much larger the p2p build may be than the default one. */
  delta: 14 * 1024 * 1024,
};

function build(features) {
  const args = ["build", "--release"];
  if (features) args.push("--features", features);
  execFileSync("cargo", args, { cwd: crate, stdio: "inherit" });
  return statSync(binary).size;
}

function mb(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

const sizes = {
  default: build(null),
  p2p: build("p2p"),
};
const delta = sizes.p2p - sizes.default;

console.log(`default  ${mb(sizes.default)}  (budget ${mb(BUDGETS.default)})`);
console.log(`p2p      ${mb(sizes.p2p)}  (budget ${mb(BUDGETS.p2p)})`);
console.log(`delta    ${mb(delta)}  (budget ${mb(BUDGETS.delta)})`);

const failures = [];
if (sizes.default > BUDGETS.default) failures.push("default");
if (sizes.p2p > BUDGETS.p2p) failures.push("p2p");
if (delta > BUDGETS.delta) failures.push("delta");

if (failures.length > 0) {
  console.error(`size budget exceeded: ${failures.join(", ")}`);
  process.exit(1);
}
console.log("size budget ok");
