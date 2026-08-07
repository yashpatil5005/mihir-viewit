// Phase 5.7 — Biometric gate stub.
// Returns true (fail-open) until tauri-plugin-biometric is wired natively.
// Real gate applies only to "protected" files (encrypted Office per Phase 3.7).
// → skipped: native plugin wiring; add when Phase 5 native surface gen runs.

export async function checkBiometric(): Promise<boolean> {
  if (typeof window === "undefined" || !(window as any).__TAURI_INTERNALS__) {
    return true; // non-Tauri (web) — fail-open
  }
  // Tauri plugin not yet wired — fail-open with console marker.
  console.debug("[viewit-biometric] stub gate, fail-open pending native wiring");
  return true;
}

export async function gateFile(_name: string): Promise<boolean> {
  return checkBiometric();
}
