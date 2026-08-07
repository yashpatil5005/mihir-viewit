// Phase 5.8 — Desktop notifications for long-running operations (archive
// extraction, decryption). Mobile-equivalent plugin is wired Rust-side.

export async function notify(title: string, body: string): Promise<void> {
  if (typeof window === "undefined" || !(window as any).__TAURI_INTERNALS__) {
    // Non-Tauri (web) — fall back to browser Notification API
    if (typeof Notification !== "undefined" && Notification.permission === "granted") {
      new Notification(title, { body });
    }
    return;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("plugin:notification|send", {
      args: { title, body, channelId: "viewit-ops" },
    });
  } catch (e) {
    console.debug("[viewit-notify] failed:", e);
  }
}
