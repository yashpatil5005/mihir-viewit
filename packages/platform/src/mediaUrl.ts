export const IS_TAURI =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in (window ?? {});

function pathForConvert(uriOrPath: string): string {
  if (uriOrPath.startsWith('file://')) {
    try {
      return decodeURIComponent(uriOrPath.replace(/^file:\/\//, ''));
    } catch {
      return uriOrPath.replace(/^file:\/\//, '');
    }
  }
  return uriOrPath;
}

/** URL the WebView can load. */
export async function assetUrlForPath(uri: string, assetPath?: string): Promise<string> {
  // Already a full URL — pass through (http://127.0.0.1:PORT/ID from localhost server,
  // or viewit-stream://, or blob/http/https).
  if (assetPath && /^(https?:\/\/|viewit-stream:|blob:)/.test(assetPath)) {
    return assetPath;
  }
  if (!IS_TAURI) {
    if (assetPath) return assetPath;
    return uri;
  }
  const { convertFileSrc } = await import('@tauri-apps/api/core');
  if (assetPath) {
    return convertFileSrc(pathForConvert(assetPath));
  }
  if (!uri) return uri;
  if (uri.startsWith('blob:') || uri.startsWith('http://') || uri.startsWith('https://')) {
    return uri;
  }
  if (uri.startsWith('content://') || uri.startsWith('file://')) {
    return convertFileSrc(uri);
  }
  return convertFileSrc(uri);
}
