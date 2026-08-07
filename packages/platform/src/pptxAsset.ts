const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window ?? {});

/** Resolve offline cache path for pptx-viewer (materializes content:// once). */
export async function resolvePptxAssetPath(
  assetPath: string | undefined,
  sourceUri: string | undefined,
  displayName?: string | null,
): Promise<string> {
  if (assetPath) return assetPath;
  if (!IS_TAURI || !sourceUri) {
    throw new Error("Open the .pptx with Open with or the in-app picker");
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string>("ensure_pptx_asset", {
    uri: sourceUri,
    name: displayName ?? null,
  });
}
