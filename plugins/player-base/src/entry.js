// player-base (base=play): a custom media player that replaces the built-in
// <video>/<audio> UI. Hosts hand bytes/URLs; this example is a styled stub that
// proves the base=play routing works end-to-end.
export function createPlayer(host, { source, kind = "video", name = "" } = {}) {
  const el = document.createElement(kind === "audio" ? "audio" : "video");
  el.controls = true;
  el.style.cssText = "width:100%;height:100%;object-fit:contain;background:#000;";
  if (source) el.src = source;
  const label = document.createElement("div");
  label.textContent = `Player Base · ${name || "media"}`;
  label.style.cssText = "font-size:12px;color:#888;padding:4px;";
  host.replaceChildren(label, el);
  return {
    setSource(src) { el.src = src; },
    play() { return el.play(); },
    pause() { el.pause(); },
    destroy() { host.replaceChildren(); el.removeAttribute("src"); el.load?.(); }
  };
}
