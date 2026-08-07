<script lang="ts">
  import { onDestroy } from "svelte";
  import { readMaterializedBytes, resolvePptxAssetPath, debugLog } from "@viewit/platform";

  let {
    document: docProp = {},
    source_uri = "",
  }: {
    document?: { asset_path?: string; slides?: PptxSlide[] };
    source_uri?: string;
  } = $props();

  type PptxParagraph = {
    runs: {
      text: string;
      bold?: boolean;
      italic?: boolean;
      underline?: boolean;
      font_size?: number;
      color?: string;
    }[];
    bullet?: boolean;
  };

  type PptxElement = {
    kind: "text" | "image";
    x: number;
    y: number;
    w: number;
    h: number;
    text?: string;
    src?: string;
    font_size?: number;
    paragraphs?: PptxParagraph[];
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
  let preParsedSlides: PptxSlide[] = $state([]);
  let pluginHtml = $derived(((docProp as any).html ?? "") as string);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let viewer: any = null;

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
    return (slide?.elements?.length ?? 0) > 0;
  }

  function slideStyle(slide: PptxSlide | undefined): string {
    if (!slide?.background) return "";
    const bg = slide.background;
    if (bg.type === "solid" && bg.color) {
      return `background-color:${bg.color};`;
    }
    if (bg.type === "gradient" && bg.gradient) {
      return `background:${bg.gradient};`;
    }
    return "";
  }

  function elementStyle(element: PptxElement, slide: PptxSlide): string {
    const width = slide.width || 9144000;
    const height = slide.height || 5143500;
    const left = (element.x / width) * 100;
    const top = (element.y / height) * 100;
    const w = (element.w / width) * 100;
    const h = (element.h / height) * 100;
    const fontSize = element.font_size ? `font-size:${element.font_size / 12}vw;` : "";
    return `left:${left}%;top:${top}%;width:${w}%;height:${h}%;${fontSize}`;
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
        <button type="button" onclick={() => nav(-1)} disabled={idx <= 0}>←</button>
        <span>{idx + 1} / {total}</span>
        <button type="button" onclick={() => nav(1)} disabled={idx >= total - 1}>→</button>
      </div>
    {/if}
    <div class="stage">
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
                      <p class="pptx-para">
                        {#if para.bullet}<span class="pptx-bullet">•</span>{/if}
                        {#each para.runs as run}
                          <span
                            class:pptx-bold={run.bold}
                            class:pptx-italic={run.italic}
                            class:pptx-underline={run.underline}
                            style={run.font_size
                              ? `font-size:${run.font_size / 12}vw`
                              : run.color
                                ? `color:${run.color}`
                                : undefined}>{run.text}</span
                          >
                        {/each}
                      </p>
                    {/each}
                  {:else}
                    {element.text}
                  {/if}
                </div>
              {:else if element.kind === "image" && element.src}
                <img
                  class="slide-element image-box"
                  style={elementStyle(element, preParsedSlides[idx])}
                  src={element.src}
                  alt=""
                />
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
          ? "rendered via enhanced Office layout extraction"
          : preParsedSlides.length > 0
            ? "rendered via enhanced Office text extraction"
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
  }
  .bar button:disabled {
    opacity: 0.35;
  }
  .stage {
    width: 100%;
    aspect-ratio: 16/9;
    max-height: 70vh;
    min-height: 200px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: #111;
    display: flex;
    align-items: center;
    justify-content: center;
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
