// One-off: add `None` as the third argument to ALL search(&pool, ...) calls.
import fs from "node:fs";

const files = [
  "src-tauri/src/infrastructure/repositories/media.rs",
  "src-tauri/tests/integration_tests.rs",
  "src-tauri/src/application/media_service.rs",
];

for (const file of files) {
  if (!fs.existsSync(file)) continue;
  let source = fs.readFileSync(file, "utf8");
  const before = source;
  // Match search(&pool, "query") — 2 args only (no third arg)
  source = source.replace(
    /search\(&pool,\s*("[^"]*")\s*\)/g,
    `search(&pool, $1, None)`,
  );
  if (source !== before) {
    fs.writeFileSync(file, source, "utf8");
    console.log("fixed", file);
  }
}