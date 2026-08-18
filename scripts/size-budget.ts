#!/usr/bin/env node
/**
 * Phase 1.11 — CI size-budget gate.
 * Per plan §5 ("CI size-budget gate"): measures the built AAB / IPA /
 * installer / WASM bundle on every PR and FAILS the build if it exceeds
 * budget. Track growth crate-by-crate from commit 1.
 *
 * Usage:
 *   node scripts/size-budget.ts [--artifacts <dir>] [--json scripts/size-budget.json]
 *
 * The script is intentionally framework-free — only Node stdlib — to keep
 * CI lightweight. No external deps.
 *
 * Phase 1 status: scaffolding the gate. Reflects "no measurement yet" until
 * the first real Phase-1 build runs and populates `last_measured`.
 */

import { createRequire } from 'node:module';
import { statSync, readdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { resolve, extname, join } from 'node:path';
import { gzipSync } from 'node:zlib';

const require = createRequire(import.meta.url);

interface Budget {
  _comment?: string;
  _unit?: string;
  _date?: string;
  _phase?: string;
  budgets: Record<string, number>;
  last_measured: Record<string, number | null> & { _status?: string };
}

const ARTIFACT_DIRS_DEFAULT = {
  'android-aab-arm64-v8a': 'apps/mobile/src-tauri/gen/android/app/build/outputs/bundle/arm64Release',
  'android-apk-arm64-v8a': 'apps/mobile/src-tauri/gen/android/app/build/outputs/apk/arm64/release',
  'ios-ipa': 'apps/mobile/src-tauri/gen/apple/build/Build/Products/Release-iphoneos',
  'desktop-linux': 'apps/desktop/src-tauri/target/release/bundle',
  'windows-msi': 'apps/desktop/src-tauri/target/release/bundle/msi',
  'macos-dmg': 'apps/desktop/src-tauri/target/release/bundle/dmg',
  'web-wasm-bundle': 'apps/web/build',
};

const EXTENSIONS: Record<string, string[]> = {
  'android-aab-arm64-v8a': ['.aab'],
  'android-apk-arm64-v8a': ['.apk'],
  'ios-ipa': ['.ipa'],
  'desktop-linux': ['.deb', '.rpm', '.AppImage', '.tar.gz'],
  'windows-msi': ['.msi'],
  'macos-dmg': ['.dmg'],
  'web-wasm-bundle': ['.wasm', '.js', '.html', '.css'],
};

function parseArgs(): { artifactsDir: string; budgetPath: string } {
  const args = process.argv.slice(2);
  let artifactsDir = process.cwd();
  let budgetPath = 'scripts/size-budget.json';
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--artifacts' && args[i + 1]) artifactsDir = args[++i];
    else if (args[i] === '--json' && args[i + 1]) budgetPath = args[++i];
  }
  return { artifactsDir, budgetPath };
}

function findArtifact(target: string, rootDir: string): string | null {
  if (target === 'android-aab-arm64-v8a') {
    const signedAab = resolve(rootDir, 'dist/viewit-android-arm64-release.aab');
    if (existsSync(signedAab)) return signedAab;
  }
  if (target === 'android-apk-arm64-v8a') {
    const signedApk = resolve(rootDir, 'dist/viewit-android-arm64-release.apk');
    if (existsSync(signedApk)) return signedApk;
  }

  const candidates = ARTIFACT_DIRS_DEFAULT[target as keyof typeof ARTIFACT_DIRS_DEFAULT];
  if (!candidates) return null;
  const dir = resolve(rootDir, candidates);
  if (!existsSync(dir)) return null;
  const exts = EXTENSIONS[target] ?? [];
  // Returns either:
  // - for single-artifact targets (aab, apk, ipa, deb, msi, dmg): the path to
  //   that artifact, OR null
  // - for `web-wasm-bundle`: a synthetic "directory aggregate" marker — we
  //   sum all file sizes in the directory recursively and return a sentinel
  //   path. measure() knows how to handle this.
  if (target === 'web-wasm-bundle') {
    // Walk recursive and accumulate; synthesize a path-style sentinel that
    // measure() understands. The string still needs to round-trip through
    // `resolve()` cleanly so callers can be agnostic.
    let total = 0;
    let largest = dir;
    function walk(p: string) {
      for (const entry of readdirSync(p, { withFileTypes: true })) {
        const full = join(p, entry.name);
        if (entry.isDirectory()) walk(full);
        else {
          total += statSync(full).size;
          if (statSync(largest).size < statSync(full).size) largest = full;
        }
      }
    }
    walk(dir);
    // Special encoded form for measure() to detect.
    return `web-aggregate:${dir}:${total}:${largest}`;
  }
  for (const f of readdirSync(dir, { recursive: true }) as string[]) {
    const full = join(dir, f);
    if (exts.includes(extname(full).toLowerCase())) return full;
  }
  return null;
}

function measure(path: string): number {
  // web-wasm-bundle aggregate marker: extract precomputed size.
  if (path.startsWith('web-aggregate:')) {
    return Number(path.split(':')[2]);
  }
  return statSync(path).size;
}

function gzipSize(path: string): number {
  if (path.startsWith('web-aggregate:')) {
    // Reconstruct and gzip all files concatenated. Cheap approximation: gzip
    // the bytes of the concat of all files (close enough for size-budget).
    const dir = path.split(':')[1];
    let totalBytes = Buffer.alloc(0);
    function walk(p: string) {
      for (const entry of readdirSync(p, { withFileTypes: true })) {
        const full = join(p, entry.name);
        if (entry.isDirectory()) walk(full);
        else totalBytes = Buffer.concat([totalBytes, readFileSync(full)]);
      }
    }
    walk(dir);
    return gzipSync(totalBytes).length;
  }
  return gzipSync(readFileSync(path)).length;
}

function main(): void {
  const { artifactsDir, budgetPath } = parseArgs();
  const budgetPathFull = resolve(artifactsDir, budgetPath);
  if (!existsSync(budgetPathFull)) {
    console.error(`[size-budget] budget file not found: ${budgetPathFull}`);
    process.exit(2);
  }
  const budget: Budget = JSON.parse(readFileSync(budgetPathFull, 'utf-8'));

  let anyMeasured = false;
  let failed = false;
  const updated: Budget = { ...budget, last_measured: { ...budget.last_measured, _status: `measured ${new Date().toISOString()}` } };

  for (const target of Object.keys(budget.budgets)) {
    const artifact = findArtifact(target, artifactsDir);
    if (!artifact) {
      console.log(`[size-budget] SKIP  ${target.padEnd(28)} (no artifact, expected if this target wasn't built)`);
      continue;
    }
    const size = measure(artifact);
    const budgetVal = budget.budgets[target];
    let sizeGz: number | null = null;
    if (target === 'web-wasm-bundle') {
      sizeGz = gzipSize(artifact);
      const budgetGz = budget.budgets['web-wasm-bundle-gz'];
      if (typeof budgetGz === 'number' && sizeGz > budgetGz) {
        console.error(`[size-budget] FAIL  web-wasm-bundle-gz  measured=${sizeGz} budget=${budgetGz} delta=+${sizeGz - budgetGz}`);
        failed = true;
      }
    }
    const verdict = size <= budgetVal ? 'OK   ' : 'FAIL ';
    const delta = size - budgetVal;
    if (verdict === 'FAIL ') failed = true;
    const gzStr = sizeGz !== null ? `  gz=${sizeGz}` : '';
    console.log(`[size-budget] ${verdict} ${target.padEnd(28)} ${size.toString().padStart(10)}${gzStr}  budget=${budgetVal}${delta > 0 ? `  +${delta}` : ''}  <${artifact.replace(artifactsDir + '/', '')}>`);
    updated.last_measured[target] = size;
    if (sizeGz !== null) updated.last_measured[`${target}-gz`] = sizeGz;
    anyMeasured = true;
  }

  if (!anyMeasured) {
    console.warn('[size-budget] WARNING  no artifacts were found. The gate will not fail on no measurement — only on overruns.');
    process.exit(0);
  }

  // Persist back so subsequent runs show the latest measurement — but only when
  // a value actually changed, so a quality-gate run leaves the worktree clean
  // (keeps step [1/7] "Git worktree clean" meaningful on re-runs).
  const changed = Object.keys(updated.last_measured).some(
    (k) => k !== '_status' && updated.last_measured[k] !== budget.last_measured[k],
  );
  if (changed) {
    writeFileSync(budgetPathFull, JSON.stringify(updated, null, 2) + '\n');
    console.log('\n[size-budget] measurement persisted (values changed)');
  } else {
    console.log('\n[size-budget] unchanged (no rewrite)');
  }
  if (failed) {
    console.error('\n[size-budget] FAILED — at least one artifact exceeded budget. Adjust budget or shrink bundle.');
    process.exit(1);
  }
  console.log('\n[size-budget] PASSED');
  process.exit(0);
}

main();
