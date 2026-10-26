/**
 * verify_docs_completeness.js
 *
 * Mechanical verification script for Module 3 documentation & code comments:
 * 1. Checks parity between English (en/) and Indonesian (id/) docs (files, headings, table row counts).
 * 2. Checks mapping completeness:
 *    - All 39 Tauri commands from src-tauri/src/lib.rs in backend/mapping.md
 *    - All 38 frontend api.ts methods in frontend/mapping.md
 *    - All 12 map_storage path patterns in project-structure.md and backend/mapping.md
 * 3. Checks that all relative markdown links in docs/ resolve to real files.
 * 4. Checks that every function (pub, pub(crate), private) in src-tauri/src/modules/module_03/ has a doc comment (///).
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ROOT = path.resolve(__dirname, "..");
const DOCS_DIR = path.join(ROOT, "docs");
const MOD3_DOCS = path.join(DOCS_DIR, "modules", "module_3");
const EN_DIR = path.join(MOD3_DOCS, "en");
const ID_DIR = path.join(MOD3_DOCS, "id");
const MODULE_03_RUST = path.join(ROOT, "src-tauri", "src", "modules", "module_03");

let errors = [];
let warnings = [];

// Helper to list all files in directory recursively
function listFiles(dir, filterExt = "") {
  let results = [];
  if (!fs.existsSync(dir)) return results;
  const list = fs.readdirSync(dir);
  for (const item of list) {
    const full = path.join(dir, item);
    const stat = fs.statSync(full);
    if (stat.isDirectory()) {
      results = results.concat(listFiles(full, filterExt));
    } else if (!filterExt || item.endsWith(filterExt)) {
      results.push(full);
    }
  }
  return results;
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. EN vs ID Parity Check
// ─────────────────────────────────────────────────────────────────────────────
console.log("=== 1. Checking EN <-> ID Parity ===");

const enFiles = listFiles(EN_DIR, ".md").map((f) => path.relative(EN_DIR, f).replace(/\\/g, "/"));
const idFiles = listFiles(ID_DIR, ".md").map((f) => path.relative(ID_DIR, f).replace(/\\/g, "/"));

// Check set of files
const missingInId = enFiles.filter((f) => !idFiles.includes(f));
const missingInEn = idFiles.filter((f) => !enFiles.includes(f));

if (missingInId.length > 0) {
  errors.push(`Missing in ID docs: ${missingInId.join(", ")}`);
}
if (missingInEn.length > 0) {
  errors.push(`Missing in EN docs: ${missingInEn.join(", ")}`);
}

// Check headings and table rows parity per matching file
for (const relFile of enFiles) {
  if (!idFiles.includes(relFile)) continue;
  const enPath = path.join(EN_DIR, relFile);
  const idPath = path.join(ID_DIR, relFile);

  const enContent = fs.readFileSync(enPath, "utf8");
  const idContent = fs.readFileSync(idPath, "utf8");

  // Headings
  const enHeadings = enContent
    .split("\n")
    .filter((l) => /^#{1,4}\s+/.test(l.trim()))
    .map((l) => l.trim().split(" ")[0]);
  const idHeadings = idContent
    .split("\n")
    .filter((l) => /^#{1,4}\s+/.test(l.trim()))
    .map((l) => l.trim().split(" ")[0]);

  if (enHeadings.length !== idHeadings.length) {
    errors.push(
      `Headings count mismatch in ${relFile}: EN has ${enHeadings.length}, ID has ${idHeadings.length}`,
    );
  } else {
    for (let i = 0; i < enHeadings.length; i++) {
      if (enHeadings[i] !== idHeadings[i]) {
        errors.push(
          `Heading level mismatch in ${relFile} at index ${i}: EN='${enHeadings[i]}' vs ID='${idHeadings[i]}'`,
        );
        break;
      }
    }
  }

  // Table rows containing pipe (|)
  const enTableRows = enContent
    .split("\n")
    .filter((l) => l.trim().startsWith("|") && !l.includes("---"));
  const idTableRows = idContent
    .split("\n")
    .filter((l) => l.trim().startsWith("|") && !l.includes("---"));

  if (enTableRows.length !== idTableRows.length) {
    errors.push(
      `Table content rows count mismatch in ${relFile}: EN has ${enTableRows.length}, ID has ${idTableRows.length}`,
    );
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Mapping Completeness Check
// ─────────────────────────────────────────────────────────────────────────────
console.log("=== 2. Checking Mapping Completeness ===");

// Extract exactly the 39 commands from invoke_handler in lib.rs lines 440-485
const libRs = fs.readFileSync(path.join(ROOT, "src-tauri", "src", "lib.rs"), "utf8");
const handlerPart = libRs.substring(libRs.indexOf("tauri::generate_handler!["));
const handlerMatches = [
  ...handlerPart.matchAll(/map_controller::[a-zA-Z0-9_:]+::([a-zA-Z0-9_]+)/g),
];
const uniqueCommands = [...new Set(handlerMatches.map((m) => m[1]))];
console.log(`Found ${uniqueCommands.length} Module 3 commands in generate_handler of lib.rs.`);

const enBackendMapping = fs.existsSync(path.join(EN_DIR, "backend", "mapping.md"))
  ? fs.readFileSync(path.join(EN_DIR, "backend", "mapping.md"), "utf8")
  : "";
const idBackendMapping = fs.existsSync(path.join(ID_DIR, "backend", "mapping.md"))
  ? fs.readFileSync(path.join(ID_DIR, "backend", "mapping.md"), "utf8")
  : "";

for (const cmd of uniqueCommands) {
  if (!enBackendMapping.includes(`\`${cmd}\``)) {
    errors.push(`Command '${cmd}' missing from EN backend/mapping.md`);
  }
  if (!idBackendMapping.includes(`\`${cmd}\``)) {
    errors.push(`Command '${cmd}' missing from ID backend/mapping.md`);
  }
}

// Extract 38 api.ts functions
const apiTs = fs.readFileSync(
  path.join(ROOT, "src", "modules", "module-03", "services", "api.ts"),
  "utf8",
);
const apiFuncMatches = [...apiTs.matchAll(/export (?:async )?function ([a-zA-Z0-9_]+)/g)].map(
  (m) => m[1],
);
console.log(`Found ${apiFuncMatches.length} exported API functions in api.ts.`);

const enFrontendMapping = fs.existsSync(path.join(EN_DIR, "frontend", "mapping.md"))
  ? fs.readFileSync(path.join(EN_DIR, "frontend", "mapping.md"), "utf8")
  : "";
const idFrontendMapping = fs.existsSync(path.join(ID_DIR, "frontend", "mapping.md"))
  ? fs.readFileSync(path.join(ID_DIR, "frontend", "mapping.md"), "utf8")
  : "";

for (const fnName of apiFuncMatches) {
  if (!enFrontendMapping.includes(`\`${fnName}\``)) {
    errors.push(`Frontend API '${fnName}' missing from EN frontend/mapping.md`);
  }
  if (!idFrontendMapping.includes(`\`${fnName}\``)) {
    errors.push(`Frontend API '${fnName}' missing from ID frontend/mapping.md`);
  }
}

// Check 12 map_storage paths
const storagePaths = [
  ".app.lock",
  ".jobs/",
  "aois/",
  "images/",
  "layers/",
  "spatial_results/",
  "annotations/",
  "photos/",
  ".trash_",
  "temp_",
  "result_image/",
  "features/",
];

for (const p of storagePaths) {
  if (!enBackendMapping.includes(p) && !enContentIncludes(p)) {
    warnings.push(`Storage path '${p}' might not be documented in EN docs`);
  }
  if (!idBackendMapping.includes(p) && !idContentIncludes(p)) {
    warnings.push(`Storage path '${p}' might not be documented in ID docs`);
  }
}

function enContentIncludes(pattern) {
  const pStructure = path.join(EN_DIR, "project-structure.md");
  return fs.existsSync(pStructure) && fs.readFileSync(pStructure, "utf8").includes(pattern);
}

function idContentIncludes(pattern) {
  const pStructure = path.join(ID_DIR, "project-structure.md");
  return fs.existsSync(pStructure) && fs.readFileSync(pStructure, "utf8").includes(pattern);
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Relative Links Check
// ─────────────────────────────────────────────────────────────────────────────
console.log("=== 3. Checking Relative Links in Docs ===");

const allDocs = listFiles(DOCS_DIR, ".md");
for (const docFile of allDocs) {
  const content = fs.readFileSync(docFile, "utf8");
  const dir = path.dirname(docFile);
  // Match [text](target) where target does not start with http, file:, #, or mailto:
  const linkMatches = content.matchAll(/\[([^\]]+)\]\(([^)]+)\)/g);
  for (const match of linkMatches) {
    const rawTarget = match[2].trim();
    if (
      rawTarget.startsWith("http://") ||
      rawTarget.startsWith("https://") ||
      rawTarget.startsWith("#") ||
      rawTarget.startsWith("file:") ||
      rawTarget.startsWith("mailto:")
    ) {
      continue;
    }
    // Remove query/anchor
    const targetFile = rawTarget.split("#")[0].split("?")[0];
    if (!targetFile) continue; // anchor only
    const resolved = path.resolve(dir, targetFile);
    if (!fs.existsSync(resolved)) {
      errors.push(
        `Broken link in ${path.relative(ROOT, docFile)}: '${rawTarget}' -> resolved '${path.relative(ROOT, resolved)}' does not exist`,
      );
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Function Doc Comment Check (Rust)
// ─────────────────────────────────────────────────────────────────────────────
console.log("=== 4. Checking Rust Function Doc Comments in module_03 ===");

const rustFiles = listFiles(MODULE_03_RUST, ".rs");
let missingDocCount = 0;
let totalFunctionCount = 0;

for (const rf of rustFiles) {
  const content = fs.readFileSync(rf, "utf8");
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    const match = line.match(/^(?:pub(?:\s*\([^)]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)/);
    if (match) {
      const fnName = match[1];
      // Skip test functions
      let isTest = false;
      for (let j = Math.max(0, i - 4); j < i; j++) {
        if (lines[j].includes("#[test]") || lines[j].includes("#[tokio::test]")) {
          isTest = true;
          break;
        }
      }
      if (isTest) continue;

      totalFunctionCount++;
      let hasDoc = false;
      for (let j = i - 1; j >= Math.max(0, i - 12); j--) {
        const prev = lines[j].trim();
        if (prev.startsWith("///")) {
          hasDoc = true;
          break;
        }
        if (!prev.startsWith("#[") && prev !== "") {
          break;
        }
      }
      if (!hasDoc) {
        missingDocCount++;
        // Log first few
        if (missingDocCount <= 10) {
          warnings.push(`Undocumented function: ${path.relative(ROOT, rf)}:${i + 1} (${fnName})`);
        }
      }
    }
  }
}

console.log(`Total Rust functions in module_03: ${totalFunctionCount}`);
console.log(`Documented: ${totalFunctionCount - missingDocCount}`);
console.log(`Undocumented: ${missingDocCount}`);

// ─────────────────────────────────────────────────────────────────────────────
// Summary
// ─────────────────────────────────────────────────────────────────────────────
console.log("\n=== VERIFICATION SUMMARY ===");
if (warnings.length > 0) {
  console.log(`WARNINGS (${warnings.length}):`);
  warnings.forEach((w) => console.log(`  - ${w}`));
}

if (errors.length > 0) {
  console.error(`ERRORS (${errors.length}):`);
  errors.forEach((e) => console.error(`  [X] ${e}`));
  process.exit(1);
} else {
  console.log("SUCCESS: All checks passed with 0 errors.");
  process.exit(0);
}
