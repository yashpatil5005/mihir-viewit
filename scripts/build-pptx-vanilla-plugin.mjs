// Build the pptx-vanilla JS plugin bundle for the plugin catalog:
//   - esbuild-bundles `pptx-vanilla-viewer` (entry: createPptxViewer) into a single
//     self-contained IIFE that assigns `window.ViewItPlugin__pptx_vanilla` (all deps
//     folded in — no bare specifiers leftover). IIFE + eval injection is required:
//     this WebView blocks dynamic-import of blob: URLs and never executes
//     dynamically created inline <script> elements.
//   - copies styles.css + LICENSE from the npm package
//   - writes plugin.json (sizeBytes + checksum) into apps/mobile/plugins/pptx-vanilla
//   - zips web/ + plugin.json + LICENSE into dist/plugins/pptx-vanilla-<v>.zip
//
// Usage:  node scripts/build-pptx-vanilla-plugin.mjs
// The plugin is delivered post-install (PluginManager runtime=js), so it never
// touches the APK size budget.
import { createWriteStream, mkdirSync, readFileSync, copyFileSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execSync } from 'node:child_process';
import path from 'node:path';

const ROOT = path.resolve(new URL('..', import.meta.url).pathname);
const PKG_VERSION = '1.14.0';
const PLUGIN_VERSION = '1.0.1';
const PLUGIN_ID = 'pptx-vanilla';
const SCRATCH = path.join(ROOT, '.cache', 'pptx-vanilla-build');
const PLUGIN_DIR = path.join(ROOT, 'apps', 'mobile', 'plugins', 'pptx-vanilla');
const WEB_DIR = path.join(PLUGIN_DIR, 'web');
const DIST_DIR = path.join(ROOT, 'dist', 'plugins');
const ENTRY = path.join(SCRATCH, 'entry.js');

async function ensureDeps() {
  mkdirSync(SCRATCH, { recursive: true });
  const pkgJson = path.join(SCRATCH, 'package.json');
  if (!existsSync(pkgJson)) {
    writeFileSync(pkgJson, JSON.stringify({ private: true, type: 'module' }, null, 2));
  }
  const has = (m) => existsSync(path.join(SCRATCH, 'node_modules', m));
  if (!has('pptx-vanilla-viewer') || !has('esbuild') || !has('archiver')) {
    console.log('[pptx-vanilla] installing build deps (scratch cache)…');
    execSync('npm install --no-audit --no-fund pptx-vanilla-viewer@1.14.0 esbuild three archiver', {
      cwd: SCRATCH, stdio: 'inherit',
    });
  }
}

async function build() {
  await ensureDeps();
  const esbuild = await import(path.join(SCRATCH, 'node_modules', 'esbuild', 'lib', 'main.js'));
  mkdirSync(WEB_DIR, { recursive: true });
  writeFileSync(ENTRY, 'export { createPptxViewer } from "pptx-vanilla-viewer";\n');

  console.log('[pptx-vanilla] esbuild bundling…');
  await esbuild.build({
    entryPoints: [ENTRY],
    outfile: path.join(WEB_DIR, 'index.js'),
    bundle: true,
    format: 'iife',
    globalName: `ViewItPlugin__${PLUGIN_ID.replace(/[^a-zA-Z0-9_]/g, '_')}`,
    minify: true,
    target: ['chrome110', 'safari15'],
    logLevel: 'warning',
  });

  const pkgDist = path.join(SCRATCH, 'node_modules', 'pptx-vanilla-viewer', 'dist');
  copyFileSync(path.join(pkgDist, 'styles.css'), path.join(WEB_DIR, 'styles.css'));
  const licenseSrc = path.join(SCRATCH, 'node_modules', 'pptx-vanilla-viewer', 'LICENSE');
  if (existsSync(licenseSrc)) copyFileSync(licenseSrc, path.join(PLUGIN_DIR, 'LICENSE'));

  const indexSize = statSync(path.join(WEB_DIR, 'index.js')).size;
  console.log(`[pptx-vanilla] web/index.js = ${(indexSize / 1024).toFixed(0)} KB`);
  console.log(`[pptx-vanilla] web/styles.css = ${statSync(path.join(WEB_DIR, 'styles.css')).size} B`);
  return true;
}

function sha256Of(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}

async function zipDir(srcDir, manifestFile, licenseFile, outZip) {
  const archiver = await import(path.join(SCRATCH, 'node_modules', 'archiver', 'lib', 'index.js')).then((m) => m.default).catch(() => null);
  if (archiver) {
    const out = createWriteStream(outZip);
    const archive = archiver('zip', { zlib: { level: 9 } });
    archive.pipe(out);
    archive.directory(srcDir, false);
    archive.file(manifestFile, { name: 'plugin.json' });
    if (existsSync(licenseFile)) archive.file(licenseFile, { name: 'LICENSE' });
    archive.finalize();
    await new Promise((r) => out.on('close', r));
    return;
  }
  // Fallback: system zip CLI.
  execSync(`cd "${srcDir}" && zip -q -r "${outZip}" . && cd - >/dev/null`, { stdio: 'inherit' });
  execSync(`cd "${path.dirname(manifestFile)}" && zip -q -j "${outZip}" plugin.json`, { stdio: 'inherit' });
  if (existsSync(licenseFile)) {
    execSync(`cd "${path.dirname(licenseFile)}" && zip -q -j "${outZip}" LICENSE`, { stdio: 'inherit' });
  }
}

async function packagePlugin() {
  mkdirSync(DIST_DIR, { recursive: true });
  const manifestPath = path.join(PLUGIN_DIR, 'plugin.json');
  const licensePath = path.join(PLUGIN_DIR, 'LICENSE');
  const outZip = path.join(DIST_DIR, `${PLUGIN_ID}-${PLUGIN_VERSION}.zip`);

  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  manifest.jsEntry = 'index.js';
  manifest.cssEntry = 'styles.css';
  manifest.runtime = 'js';
  manifest.version = PLUGIN_VERSION;
  manifest.downloadUrl = `https://viewit-plugin-catalog-temp.pages.dev/plugins/${PLUGIN_ID}-${PLUGIN_VERSION}.zip`;

  // sizeBytes/checksum are filled after the zip exists (zip is written from web/ +
  // current manifest; checksum covers the zip, which includes plugin.json — so set
  // accurate sizeBytes but leave checksum for a second pass).
  manifest.sizeBytes = 0;
  manifest.checksum = '';
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

  await zipDir(WEB_DIR, manifestPath, licensePath, outZip);
  const stat = statSync(outZip);
  const size = stat.size;
  const checksum = sha256Of(outZip);

  manifest.sizeBytes = size;
  manifest.installedSizeBytes = size * 3;
  manifest.checksum = checksum;
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

  console.log(`[pptx-vanilla] zip: ${outZip}`);
  console.log(`[pptx-vanilla] sizeBytes=${size} sha256=${checksum}`);
  console.log(`[pptx-vanilla] plugin.json updated in ${path.relative(ROOT, manifestPath)}`);
}

await build();
await packagePlugin();
