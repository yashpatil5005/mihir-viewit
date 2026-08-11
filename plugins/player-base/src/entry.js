// player-base (base=play): a functional custom media player that replaces the
// built-in <video>/<audio> UI. Owns source resolution, playback controls and
// resume-position persistence, so the base is genuinely "the plugin's job".
export function createPlayer(host, { source = "", stream = "", kind = "video", name = "", persistKey = "", tracks = [] } = {}) {
  const root = document.createElement("div");
  root.className = "playerbase-root";
  root.style.cssText =
    "display:flex;flex-direction:column;height:100%;background:#000;color:#fff;font:12px/1.4 system-ui;";

  const media = document.createElement(kind === "audio" ? "audio" : "video");
  media.controls = false;
  media.style.cssText =
    "flex:1;width:100%;max-height:100%;object-fit:contain;background:#000;";
  if (stream) media.src = stream;
  else if (source) media.src = source;
  // Subtitles/captions are entirely plugin-owned: pass [{src,label,lang,default}].
  for (const t of tracks) {
    const track = document.createElement("track");
    track.kind = t.kind || "subtitles";
    track.src = t.src;
    track.label = t.label || t.lang || "Subtitles";
    track.srclang = t.lang || "en";
    track.default = !!t.default;
    media.appendChild(track);
  }

  const bar = document.createElement("div");
  bar.style.cssText =
    "display:flex;align-items:center;gap:8px;padding:6px 8px;border-top:1px solid #222;";

  const label = document.createElement("span");
  label.textContent = "Player Base · " + (name || "media");
  label.style.cssText = "flex:1;color:#9aa;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";

  const btn = document.createElement("button");
  btn.textContent = "▶";
  btn.style.cssText = "width:30px;height:30px;border:0;border-radius:50%;background:#2a9;color:#fff;cursor:pointer;";

  const cur = document.createElement("span");
  cur.style.cssText = "min-width:44px;text-align:center;color:#888;font-variant-numeric:tabular-nums;";
  const dur = document.createElement("span");
  dur.style.cssText = "color:#888;font-variant-numeric:tabular-nums;";

  const seek = document.createElement("input");
  seek.type = "range";
  seek.min = 0; seek.max = 1000; seek.value = 0;
  seek.style.cssText = "flex:1;accent-color:#2a9;";

  const rate = document.createElement("select");
  rate.style.cssText = "background:#111;color:#eee;border:1px solid #333;border-radius:4px;";
  ["0.5", "0.75", "1", "1.25", "1.5", "2"].forEach((r) => {
    const o = document.createElement("option");
    o.value = r; o.textContent = r + "×";
    if (r === "1") o.selected = true;
    rate.appendChild(o);
  });

  const pip = document.createElement("button");
  pip.textContent = "PiP";
  pip.title = "Picture in picture";
  pip.style.cssText = "background:#111;color:#eee;border:1px solid #333;border-radius:4px;padding:4px;cursor:pointer;";
  const full = document.createElement("button");
  full.textContent = "⛶";
  full.title = "Fullscreen";
  full.style.cssText = pip.style.cssText;
  pip.hidden = !("requestPictureInPicture" in media) || kind === "audio";
  pip.addEventListener("click", () => media.requestPictureInPicture?.().catch(() => {}));
  full.addEventListener("click", () => root.requestFullscreen?.().catch(() => {}));

  bar.append(btn, cur, seek, dur, rate, pip, full, label);
  root.append(media, bar);
  host.replaceChildren(root);

  const fmt = (s) => {
    if (!isFinite(s)) return "0:00";
    s = Math.floor(s);
    const m = Math.floor(s / 60), sec = s % 60;
    return m + ":" + String(sec).padStart(2, "0");
  };
  const resumeKey = persistKey || (name ? "vbp:" + name : "");

  function loaded() {
    dur.textContent = fmt(media.duration);
    if (resumeKey) {
      try {
        const t = parseFloat(localStorage.getItem(resumeKey) || "0");
        if (t > 1 && t < media.duration - 5) media.currentTime = t;
      } catch {}
    }
    if (!media.paused) btn.textContent = "⏸"; else btn.textContent = "▶";
  }

  media.addEventListener("loadedmetadata", loaded);
  media.addEventListener("timeupdate", () => {
    cur.textContent = fmt(media.currentTime);
    seek.value = media.duration ? (media.currentTime / media.duration) * 1000 : 0;
    if (resumeKey) { try { localStorage.setItem(resumeKey, String(media.currentTime)); } catch {} }
  });
  media.addEventListener("play", () => (btn.textContent = "⏸"));
  media.addEventListener("pause", () => (btn.textContent = "▶"));
  media.addEventListener("ended", () => (btn.textContent = "▶"));

  const toggle = () => { if (media.paused) media.play().catch(() => {}); else media.pause(); };
  btn.addEventListener("click", toggle);
  seek.addEventListener("input", () => { if (media.duration) media.currentTime = (seek.value / 1000) * media.duration; });
  rate.addEventListener("change", () => (media.playbackRate = parseFloat(rate.value)));

  if (media.play) media.play().catch(() => {});

  return {
    setSource(src) { media.src = src; media.play().catch(() => {}); },
    play: () => media.play(),
    pause: () => media.pause(),
    stop() { media.pause(); media.currentTime = 0; },
    seekTo(s) { media.currentTime = s; },
    getCurrentTime: () => media.currentTime,
    getDuration: () => media.duration,
    setRate(r) { media.playbackRate = r; rate.value = String(r); },
    setVolume(v) { media.volume = Math.max(0, Math.min(1, v)); },
    getMedia: () => media,
    enterFullscreen: () => root.requestFullscreen?.(),
    exitFullscreen: () => document.exitFullscreen?.(),
    enterPictureInPicture: () => media.requestPictureInPicture?.(),
    addTrack(t) { const el=document.createElement("track"); Object.assign(el,{kind:t.kind||"subtitles",src:t.src,label:t.label||"Subtitles",srclang:t.lang||"en",default:!!t.default}); media.appendChild(el); },
    onError(cb) { media.addEventListener("error", () => cb(media.error && media.error.message)); },
    destroy() {
      try { media.pause(); media.removeAttribute("src"); media.load?.(); } catch {}
      root.remove();
    }
  };
}
