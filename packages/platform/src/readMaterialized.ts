const IS_TAURI =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in (window ?? {});

const IS_ANDROID =
  typeof navigator !== 'undefined' &&
  /android/i.test(navigator.userAgent || '');

/** Offline: read a materialized cache file via Rust (no fetch / no mobile data).
 *
 *  Why two paths?
 *  - Android WebView does NOT support Tauri's `InvokeBody::Raw` (binary IPC), so
 *    returning `Vec<u8>` serializes as a JSON `number[]` — ~4x size blowup + GC
 *    pain for a 400 MB video. We use a base64 string instead (~33% overhead, fast).
 *  - Desktop / iOS WebView support raw bytes via `tauri::ipc::Response`, which
 *    arrives as an `ArrayBuffer` — zero-copy, optimal.
 */
export async function readMaterializedBytes(assetPath: string): Promise<Uint8Array> {
  if (!assetPath) {
    throw new Error('missing asset_path');
  }
  if (!IS_TAURI) {
    throw new Error('readMaterializedBytes requires Tauri');
  }
  const { invoke } = await import('@tauri-apps/api/core');

  const log = (s: string) => {
    try {
      (window as any).__viewit_log__?.(s);
    } catch {}
    console.log(s);
  };

  if (IS_ANDROID) {
    log(`[readMaterialized] android: invoking read_materialized_bytes_b64`);
    const b64 = await invoke<string>('read_materialized_bytes_b64', { assetPath });
    log(`[readMaterialized] got b64 string len=${b64.length}`);
    // Decode base64 → Uint8Array (in chunks to avoid call-stack limits on huge strings).
    const bin = atob(b64);
    const len = bin.length;
    const out = new Uint8Array(len);
    const CHUNK = 0x8000;
    for (let i = 0; i < len; i += CHUNK) {
      const end = Math.min(i + CHUNK, len);
      for (let j = i; j < end; j++) out[j] = bin.charCodeAt(j);
    }
    log(`[readMaterialized] decoded ${out.length} bytes`);
    return out;
  }

  log(`[readMaterialized] desktop: invoking read_materialized_bytes (raw)`);
  const buf = await invoke<ArrayBuffer>('read_materialized_bytes', { assetPath });
  log(`[readMaterialized] got ArrayBuffer byteLength=${buf.byteLength}`);
  return new Uint8Array(buf);
}
