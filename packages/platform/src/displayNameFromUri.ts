export function displayNameFromUri(uri: string): string | null {
  if (!uri.startsWith("content://") && !uri.startsWith("file://")) return null;
  const lower = uri.toLowerCase();
  if (lower.includes("video%3a") || lower.includes("video:")) return "video.mp4";
  if (lower.includes("audio%3a") || lower.includes("audio:")) return "audio.mp3";
  const tail = uri.split("/").pop()?.split("?")[0];
  if (!tail || tail.length > 200) return null;
  if (/^\d+$/.test(tail)) return null;
  try {
    return decodeURIComponent(tail);
  } catch {
    return tail;
  }
}
