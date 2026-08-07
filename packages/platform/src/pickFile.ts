import { debugLog } from "./debugLog";

const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window ?? {});

/** System file picker on Tauri (SAF on Android); falls back to `<input type="file">`. */
export async function pickSingleFile(): Promise<File | null> {
  if (IS_TAURI) {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        directory: false,
      });
      if (selected === null) return null;
      if (typeof selected === "string") {
        debugLog(`picked ${selected.slice(0, 60)}…`);
        return uriPickerHandle(selected);
      }
      if (Array.isArray(selected) && selected[0]) {
        return uriPickerHandle(selected[0]);
      }
      return null;
    } catch (e) {
      debugLog(`dialog open err: ${e instanceof Error ? e.message : String(e)}`);
    }
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.click();
  });
}

/** SAF `content://` URIs must open via Rust `open_uri`, not a full read into JS. */
async function uriPickerHandle(uri: string): Promise<File> {
  const name = uri.split("/").pop()?.split("?")[0] || "picked-file";
  const f = new File([], name, { type: "application/octet-stream" });
  Object.defineProperty(f, "viewitUri", { value: uri, enumerable: true });
  return f;
}

async function pathToFile(path: string): Promise<File> {
  const { readFile } = await import("@tauri-apps/plugin-fs");
  const bytes = await readFile(path);
  const name = path.split(/[/\\]/).pop() ?? "file";
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes as ArrayBuffer);
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  const mime = guessMime(ext);
  return new File([u8], name, { type: mime });
}

function guessMime(ext: string): string {
  const m: Record<string, string> = {
    pdf: "application/pdf",
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    txt: "text/plain",
    md: "text/markdown",
    json: "application/json",
    csv: "text/csv",
    mp4: "video/mp4",
  };
  return m[ext] ?? "application/octet-stream";
}
