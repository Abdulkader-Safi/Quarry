// Usage: node validate.mjs <file.glb>...  Exits 1 if any file has errors.
import { readFileSync } from "node:fs";
import validator from "gltf-validator";

let failed = false;
for (const path of process.argv.slice(2)) {
  const report = await validator.validateBytes(new Uint8Array(readFileSync(path)));
  const { numErrors, numWarnings, numInfos, messages } = report.issues;
  console.log(`${path}: ${numErrors} errors, ${numWarnings} warnings, ${numInfos} infos`);
  for (const m of messages) console.log(`  [${m.severity}] ${m.code} ${m.pointer ?? ""}: ${m.message}`);
  if (numErrors > 0) failed = true;
}
process.exit(failed ? 1 : 0);
