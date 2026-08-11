// editor-base (base=edit): minimal inline text/markdown editor. Editing + save are
// the plugin's responsibility. Hosts hand the document text; the editor returns it.
export function createEditor(host, { text = "", name = "", readOnly = false } = {}) {
  const root = document.createElement("div");
  root.className = "editorbase-root";
  root.style.cssText = "display:flex;flex-direction:column;height:100%;min-height:40vh;";
  const bar = document.createElement("div");
  bar.style.cssText = "display:flex;gap:8px;align-items:center;padding:6px 8px;font:12px system-ui;color:#888;";
  bar.textContent = "Editor Base · " + (name || "document");
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.spellcheck = false;
  ta.readOnly = readOnly;
  ta.style.cssText =
    "flex:1;width:100%;resize:none;border:1px solid var(--border,#333);border-radius:6px;" +
    "background:var(--bg-primary,#171717);color:var(--text-primary,#eee);" +
    "font:13px/1.5 ui-monospace,monospace;padding:8px;";
  ta.addEventListener("input", () => bar.textContent = "Editor Base · " + (name || "document") + " (edited)");
  root.append(bar, ta);
  host.replaceChildren(root);
  return {
    getText: () => ta.value,
    setText(t) { ta.value = t; bar.textContent = "Editor Base · " + (name || "document"); },
    focus: () => ta.focus(),
    destroy() { root.remove(); }
  };
}
