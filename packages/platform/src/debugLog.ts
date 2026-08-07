const MAX = 40;
const lines: string[] = [];

export function debugLog(msg: string): void {
  const t = new Date().toISOString().slice(11, 19);
  lines.unshift(`[${t}] ${msg}`);
  if (lines.length > MAX) lines.length = MAX;
  // Also emit to console so it surfaces in Android logcat via Tauri's
  // RustWebChromeClient.onConsoleMessage (tag "Tauri/Console").
  try {
    console.log(`[debugLog] ${msg}`);
  } catch {}
}

export function debugLogLines(): string[] {
  return [...lines];
}

export function debugLogClear(): void {
  lines.length = 0;
}
