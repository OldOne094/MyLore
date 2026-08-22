// One-off: balance parens in import_service Behavior::Ok(Box::new(media(..), nodes())) lines.
import fs from "node:fs";

const file = "src-tauri/src/application/import_service.rs";
const lines = fs.readFileSync(file, "utf8").split("\n");
let fixed = 0;

const out = lines.map((line) => {
  if (!line.includes("Behavior::Ok(Box::new(media(")) return line;
  if (line.includes("Box::new(media(") && !/Box::new\(media\([^)]*\)\)/.test(line)) {
    // Move one ")" from the tail into place: close media() before ", nodes()".
    const fixedLine = line.replace(/"\), nodes\(\)\)\),/, "\")), nodes())),");
    if (fixedLine !== line) {
      fixed++;
      return fixedLine;
    }
    // Fallback generic: insert ")" right before ", nodes()" when media is boxed.
    const alt = line.replace(/\), nodes\(\)\)\),/, "), nodes())),");
    if (alt !== line) {
      fixed++;
      return alt;
    }
  }
  return line;
});

fs.writeFileSync(file, out.join("\n"), "utf8");
console.log("fixed lines:", fixed);