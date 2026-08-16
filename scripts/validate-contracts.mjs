#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  ContractValidationError,
  parseLegacyCatalog,
  parseLegacyPluginEntry,
  parsePluginPackageManifestV1,
} from "../packages/contracts/src/index.ts";

const [kind, input] = process.argv.slice(2);
if (!kind || !input) {
  console.error("usage: node --experimental-strip-types scripts/validate-contracts.mjs <legacy-catalog|legacy-entry|package-v1> <json-file>");
  process.exit(2);
}

const path = resolve(input);
try {
  const value = JSON.parse(readFileSync(path, "utf8"));
  if (kind === "legacy-catalog") parseLegacyCatalog(value);
  else if (kind === "legacy-entry") parseLegacyPluginEntry(value);
  else if (kind === "package-v1") parsePluginPackageManifestV1(value);
  else throw new Error(`unknown contract kind: ${kind}`);
  console.log(`[contracts] OK ${kind}: ${input}`);
} catch (error) {
  if (error instanceof ContractValidationError) {
    console.error(`[contracts] INVALID ${kind}: ${input}`);
    for (const issue of error.errors) {
      console.error(`  ${issue.instancePath || "/"} ${issue.message ?? "is invalid"}`);
    }
  } else {
    console.error(`[contracts] ERROR ${kind}: ${input}: ${error instanceof Error ? error.message : error}`);
  }
  process.exit(1);
}
