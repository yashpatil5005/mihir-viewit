<script lang="ts">
  import Icon from "./Icon.svelte";
  import { onDestroy } from "svelte";
  import { readMaterializedBytes, resolvePptxAssetPath, debugLog } from "@viewit/platform";

  let {
    document: docProp = {},
    source_uri = "",
    source_kind = "office",
  }: {
    document?: { asset_path?: string; slides?: PptxSlide[] };
    source_uri?: string;
    source_kind?: "office" | "iwork";
  } = $props();

  type PptxParagraph = {
    runs: {
      text: string;
      bold?: boolean;
      italic?: boolean;
      underline?: boolean;
      font_size?: number;
      color?: string;
      font_family?: string;
    }[];
    bullet?: boolean;
    alignment?: string;
  };

  type PptxElement = {
    kind: "text" | "image" | "shape";
    x: number;
    y: number;
    w: number;
    h: number;
    text?: string;
    src?: string;
    font_size?: number;
    paragraphs?: PptxParagraph[];
    fill_color?: string;
    border_color?: string;
    border_width?: number;
  };

  type PptxSlide = {
    title: string;
    body: string;
    width?: number;
    height?: number;
    elements?: PptxElement[];
    background?: { type: string; color?: string; gradient?: string };
  };

  let canvasEl: HTMLCanvasElement | null = $state(null);
  let rootEl: HTMLDivElement | null = $state(null);
  let status = $state<"loading" | "ready" | "error">("loading");
  let errorMsg = $state("");
  let idx = $state(0);
  let total = $state(0);
  let stageEl: HTMLDivElement | null = $state(null);
  let stageWidthPx = $state(0);
  let preParsedSlides: PptxSlide[] = $state([]);
  let pluginHtml = $derived(((docProp as any).html ?? "") as string);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let viewer: any = null;

  $effect(() => {
    if (!stageEl) return;
    const observer = new ResizeObserver(([entry]) => {
      stageWidthPx = entry.contentRect.width;
    });
    observer.observe(stageEl);
    return () => observer.disconnect();
  });

  async function loadDeck() {
    status = "loading";
    errorMsg = "";
    preParsedSlides = [];
    try {
      // If the Rust backend already parsed slides (ODP, ODP fallback, etc.),
      // use them directly instead of re-parsing the raw file with pptx-parser.
      if (docProp.slides && docProp.slides.length > 0) {
        preParsedSlides = docProp.slides;
        total = docProp.slides.length;
        idx = 0;
        status = "ready";
        debugLog(`pptx ok slides=${total} (pre-parsed)`);
        return;
      }

      const path = await resolvePptxAssetPath(docProp.asset_path, source_uri || undefined, null);
      const bytes = await readMaterializedBytes(path);
      debugLog(`pptx bytes ${bytes.length}`);

      const { PptxViewer: OoxmlPptxViewer } = await import("@silurus/ooxml/pptx");

      if (!canvasEl) {
        status = "error";
        errorMsg = "Canvas not ready";
        return;
      }

      viewer = new OoxmlPptxViewer(canvasEl);
      await viewer.load(bytes);

      total = viewer.slideCount ?? 0;
      idx = viewer.currentSlide ?? 0;
      status = "ready";
      debugLog(`pptx ok slides=${total}`);
    } catch (e) {
      status = "error";
      errorMsg = e instanceof Error ? e.message : String(e);
      debugLog(`pptx err: ${errorMsg}`);
      viewer = null;
    }
  }

  $effect(() => {
    const uri = source_uri;
    const ap = docProp.asset_path;
    const slides = docProp.slides ?? [];
    if (slides.length === 0 && !uri && !ap) {
      status = "error";
      errorMsg = "Open a .pptx with Open with or Open file…";
      return;
    }
    if (slides.length === 0 && !canvasEl) return;
    void loadDeck();
  });

  onDestroy(() => {
    viewer?.destroy?.();
    viewer = null;
  });

  function nav(d: number) {
    const n = Math.max(0, Math.min(total - 1, idx + d));
    if (n !== idx) {
      idx = n;
      viewer?.goToSlide?.(n);
    }
  }

  function hasLayout(slide: PptxSlide | undefined): boolean {
    if (!slide?.elements?.length) return false;
    const width = slide.width || 9144000;
    const height = slide.height || 5143500;
    return slide.elements.some((element) => {
      const hasUsableSize = element.w / width >= 0.005 && element.h / height >= 0.005;
      if (!hasUsableSize) return false;
      if (element.kind === "image") return Boolean(safeImageSrc(element.src));
      const text = [
        element.text,
        ...(element.paragraphs ?? []).flatMap((paragraph) => paragraph.runs.map((run) => run.text)),
      ]
        .filter(Boolean)
        .join(" ")
        .replace(/<[^>]+>/g, "")
        .trim();
      return text.length > 2 && !/^(number|date|time|footer|header)$/i.test(text);
    });
  }

  function slideStyle(slide: PptxSlide | undefined): string {
    if (!slide?.background) return "";
    const bg = slide.background;
    const color = safeColor(bg.color);
    if (bg.type === "solid" && color) {
      return `background-color:${color};`;
    }
    if (bg.type === "gradient" && bg.gradient && safeGradient(bg.gradient)) {
      return `background:${bg.gradient};`;
    }
    return "";
  }

  function safeColor(color: string | undefined): string {
    return color && /^(#[0-9a-f]{3,8}|rgba?\([\d\s.,%]+\))$/i.test(color) ? color : "";
  }

  function safeGradient(gradient: string): boolean {
    return /^linear-gradient\(\d+(?:\.\d+)?deg(?:,\s*#[0-9a-f]{6}\s+\d+(?:\.\d+)?%){2,}\)$/i.test(
      gradient,
    );
  }

  function safeImageSrc(src: string | undefined): string | undefined {
    return src && /^(data:image\/(png|jpeg|gif|webp|bmp);base64,|blob:)/i.test(src)
      ? src
      : undefined;
  }

  function safeFontFamily(font: string | undefined): string {
    return font && /^[\w\s.,'"-]{1,100}$/.test(font) ? font : "";
  }

  function cssAlignment(alignment: string | undefined): string | undefined {
    return ({ l: "left", ctr: "center", r: "right", just: "justify" } as Record<string, string>)[
      alignment ?? ""
    ];
  }

  function elementStyle(element: PptxElement, slide: PptxSlide): string {
    const width = slide.width || 9144000;
    const height = slide.height || 5143500;
    const left = (element.x / width) * 100;
    const top = (element.y / height) * 100;
    const w = (element.w / width) * 100;
    const h = (element.h / height) * 100;
    const fontSize = element.font_size
      ? `font-size:${(element.font_size / 1200) * (stageWidthPx || window.innerWidth)}px;`
      : "";
    const fillColor = safeColor(element.fill_color);
    const borderColor = safeColor(element.border_color);
    const fill = fillColor ? `background-color:${fillColor};` : "";
    const border = borderColor
      ? `border:${Math.max(1, Math.min(20, element.border_width ?? 1))}px solid ${borderColor};`
      : "";
    return `left:${left}%;top:${top}%;width:${w}%;height:${h}%;${fontSize}${fill}${border}`;
  }
</script>

<div class="pptx-root" bind:this={rootEl}>
  {#if pluginHtml}
    <div class="plugin-html-surface">{@html pluginHtml}</div>
  {:else}
    {#if status === "loading"}<p class="muted">Loading presentation…</p>{/if}
    {#if status === "error"}<p class="err">{errorMsg}</p>{/if}
    {#if status === "ready"}
      <div class="bar">
        <button
          type="button"
          onclick={() => nav(-1)}
          disabled={idx <= 0}
          aria-label="Previous slide"><Icon name="arrow-left" /></button
        >
        <span>{idx + 1} / {total}</span>
        <button
          type="button"
          onclick={() => nav(1)}
          disabled={idx >= total - 1}
          aria-label="Next slide"><Icon name="arrow-right" /></button
        >
      </div>
    {/if}
    <div class="stage" bind:this={stageEl}>
      {#if preParsedSlides.length > 0}
        <div
          class:slide-layout={hasLayout(preParsedSlides[idx])}
          class:slide-text={!hasLayout(preParsedSlides[idx])}
          style={slideStyle(preParsedSlides[idx])}
        >
          {#if preParsedSlides[idx] && hasLayout(preParsedSlides[idx])}
            {#each preParsedSlides[idx].elements ?? [] as element}
              {#if element.kind === "text"}
                <div
                  class="slide-element text-box"
                  style={elementStyle(element, preParsedSlides[idx])}
                >
                  {#if element.paragraphs && element.paragraphs.length > 0}
                    {#each element.paragraphs as para}
                      <p class="pptx-para" style:text-align={cssAlignment(para.alignment)}>
                        {#if para.bullet}<span class="pptx-bullet">•</span>{/if}
                        {#each para.runs as run}
                          <span
                            class:pptx-bold={run.bold}
                            class:pptx-italic={run.italic}
                            class:pptx-underline={run.underline}
                            style={[
                              run.font_size
                                ? `font-size:${(run.font_size / 1200) * (stageWidthPx || window.innerWidth)}px`
                                : "",
                              safeColor(run.color) ? `color:${safeColor(run.color)}` : "",
                              safeFontFamily(run.font_family)
                                ? `font-family:${safeFontFamily(run.font_family)}`
                                : "",
                            ]
                              .filter(Boolean)
                              .join("; ")}>{run.text}</span
                          >
                        {/each}
                      </p>
                    {/each}
                  {:else}
                    {element.text}
                  {/if}
                </div>
              {:else if element.kind === "image" && element.src}
                {@const imageSrc = safeImageSrc(element.src)}
                {#if imageSrc}
                  <img
                    class="slide-element image-box"
                    style={elementStyle(element, preParsedSlides[idx])}
                    src={imageSrc}
                    alt=""
                  />
                {/if}
              {:else if element.kind === "shape"}
                <div
                  class="slide-element shape-box"
                  style={elementStyle(element, preParsedSlides[idx])}
                  aria-hidden="true"
                ></div>
              {/if}
            {/each}
          {:else if preParsedSlides[idx]}
            {#if preParsedSlides[idx].title}<h3>{preParsedSlides[idx].title}</h3>{/if}
            <pre>{preParsedSlides[idx].body}</pre>
          {/if}
        </div>
      {:else}
        <canvas bind:this={canvasEl}></canvas>
      {/if}
    </div>
    {#if status === "ready"}
      <p class="muted">
        Offline · {hasLayout(preParsedSlides[idx])
          ? source_kind === "iwork"
            ? "rendered via iWork package preview extraction"
            : "rendered via enhanced Office layout extraction"
          : preParsedSlides.length > 0
            ? source_kind === "iwork"
              ? "rendered via iWork package text extraction"
              : "rendered via enhanced Office text extraction"
            : "rendered via Canvas"} (no PowerPoint animations).
      </p>
    {/if}
  {/if}
</div>

<style>
  .pptx-root {
    padding: 0.5rem;
    color: var(--text-primary);
  }
  .plugin-html-surface {
    overflow: auto;
    border-radius: 0.75rem;
  }
  .bar {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-bottom: 0.5rem;
    flex-wrap: wrap;
  }
  .bar button {
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 0.3rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    min-width: 44px;
    min-height: 44px;
  }
  .bar button:disabled {
    opacity: 0.35;
  }
  .stage {
    width: 100%;
    aspect-ratio: 16/9;
    max-height: 70vh;
    max-width: 124.444vh;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: #111;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto;
  }
  canvas {
    max-width: 100%;
    max-height: 100%;
    display: block;
  }
  .slide-text {
    width: 100%;
    height: 100%;
    padding: 1.5rem;
    overflow: auto;
    color: var(--text-primary);
    background: var(--bg-primary);
    font-size: 0.9rem;
    line-height: 1.6;
  }
  .slide-layout {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: white;
    color: #111;
  }
  .slide-element {
    position: absolute;
    overflow: hidden;
  }
  .text-box {
    white-space: pre-wrap;
    line-height: 1.2;
    padding: 0.2rem;
    color: #111;
  }
  .text-box .pptx-para {
    margin: 0 0 0.15rem;
    line-height: 1.3;
    font-size: inherit;
  }
  .text-box .pptx-para .pptx-bold {
    font-weight: 700;
  }
  .text-box .pptx-para .pptx-italic {
    font-style: italic;
  }
  .text-box .pptx-para .pptx-underline {
    text-decoration: underline;
  }
  .text-box .pptx-bullet {
    margin-right: 0.3em;
  }
  .image-box {
    object-fit: contain;
  }
  .slide-text h3 {
    margin: 0 0 0.5rem;
    font-size: 1.1rem;
  }
  .slide-text pre {
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    font-family: inherit;
  }
  .muted {
    font-size: 0.75rem;
    color: var(--text-secondary);
  }
  .err {
    color: var(--error);
    white-space: pre-wrap;
  }
</style>
