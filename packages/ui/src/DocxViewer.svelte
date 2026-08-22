<script lang="ts">
  // Phase 3.1 — DOCX viewer. Renders blocks: paragraphs (heading-aware),
  // list items, tables, image placeholders.
  import { findAllMatches } from "./search";

  type InlineImage = { name?: string; src?: string };
  type Inline = {
    text?: string;
    bold?: boolean;
    italic?: boolean;
    underline?: boolean;
    color?: string;
    font_size?: number;
    href?: string;
    image?: InlineImage;
  };

  import { fullscreenState } from "./fullscreen.svelte";

  let {
    document: docProp = {},
    searchQuery = "",
    searchCaseSensitive = false,
    onSearchResult = (_count: number, _current: number) => {},
  }: {
    document?: any;
    searchQuery?: string;
    searchCaseSensitive?: boolean;
    onSearchResult?: (count: number, current: number) => void;
  } = $props();

  let blocks = $derived((docProp.blocks ?? []) as Array<any>);
  let pluginHtml = $derived((docProp.html ?? "") as string);
  let byte_len = $derived((docProp.byte_len ?? 0) as number);
  let query = $state("");
  let caseSensitive = $state(false);

  $effect(() => {
    const q = searchQuery;
    const cs = searchCaseSensitive;
    if (q === query && cs === caseSensitive) return;
    query = q;
    caseSensitive = cs;
    const text = blocks.map((b) => b.text ?? inlineText(b.inlines ?? [])).join(" ");
    const m = findAllMatches(text, q, cs);
    onSearchResult(m.length, m.length > 0 ? 1 : 0);
  });

  let outline = $derived(
    blocks
      .map((block, index) => ({ ...block, index }))
      .map((block) => ({
        ...block,
        text: block.text ?? inlineText(block.inlines ?? []),
      }))
      .filter((block) => block.kind === "paragraph" && block.heading && block.text),
  );
  let stats = $derived.by(() => {
    const text = blocks
      .map((block) => {
        if (block.kind === "table") return (block.rows ?? []).flat().join(" ");
        if (block.kind === "table" && block.rich_rows)
          return (block.rich_rows ?? [])
            .flat()
            .map((cell: { inlines?: Inline[] }) => inlineText(cell.inlines ?? []))
            .join(" ");
        return block.text ?? inlineText(block.inlines ?? []);
      })
      .join(" ");
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    const tables = blocks.filter((block) => block.kind === "table").length;
    return { words, tables, blocks: blocks.length };
  });

  function highlightedParts(s: string): Array<{ text: string; match: boolean }> {
    if (!query) return [{ text: s, match: false }];
    const matches = findAllMatches(s, query, caseSensitive);
    if (matches.length === 0) return [{ text: s, match: false }];
    const parts: Array<{ text: string; match: boolean }> = [];
    let cursor = 0;
    for (const m of matches) {
      if (m.index > cursor) parts.push({ text: s.slice(cursor, m.index), match: false });
      parts.push({ text: s.slice(m.index, m.index + m.length), match: true });
      cursor = m.index + m.length;
    }
    if (cursor < s.length) parts.push({ text: s.slice(cursor), match: false });
    return parts;
  }

  function inlineText(inlines: Inline[]): string {
    return inlines.map((inline) => inline.text ?? "").join("");
  }

  function safeHref(href: string | undefined): string | undefined {
    if (!href) return undefined;
    const trimmed = href.trim();
    if (trimmed.startsWith("#")) return trimmed;
    try {
      const url = new URL(trimmed);
      return ["http:", "https:", "mailto:", "tel:"].includes(url.protocol) ? trimmed : undefined;
    } catch {
      return undefined;
    }
  }

  function safeImageSrc(src: string | undefined): string | undefined {
    if (!src) return undefined;
    return /^(data:image\/(png|jpeg|gif|webp|bmp);base64,|blob:)/i.test(src) ? src : undefined;
  }

  function inlineStyle(inline: Inline): string {
    const styles: string[] = [];
    if (inline.color && /^#[0-9a-f]{6}$/i.test(inline.color)) styles.push(`color:${inline.color}`);
    if (Number.isFinite(inline.font_size) && (inline.font_size ?? 0) > 0)
      styles.push(`font-size:${Math.min(inline.font_size ?? 0, 200)}pt`);
    return styles.join(";");
  }

  function headingTag(level: number): "h1" | "h2" | "h3" | "h4" | "h5" | "h6" {
    const safeLevel = Math.max(1, Math.min(6, Number(level) || 1));
    return `h${safeLevel}` as "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
  }
</script>

{#snippet highlightedText(text: string)}
  {#each highlightedParts(text) as part}
    {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
  {/each}
{/snippet}

{#snippet richInlines(inlines: Inline[])}
  {#each inlines as inline}
    {@const href = safeHref(inline.href)}
    {@const content = inline.text ?? ""}
    {#if inline.image}
      {@const imageSrc = safeImageSrc(inline.image.src)}
      <span class="inline-image">
        {#if imageSrc}
          <img src={imageSrc} alt={inline.image.name ?? "Embedded image"} />
        {:else}
          <span class="img-placeholder">Embedded image</span>
        {/if}
      </span>
    {/if}
    {#if content}
      {#if href}
        <a
          {href}
          target={href.startsWith("#") ? undefined : "_blank"}
          rel={href.startsWith("#") ? undefined : "noopener noreferrer"}
          class:bold={inline.bold}
          class:italic={inline.italic}
          class:underline={inline.underline}
          style={inlineStyle(inline)}>{@render highlightedText(content)}</a
        >
      {:else}
        <span
          class:bold={inline.bold}
          class:italic={inline.italic}
          class:underline={inline.underline}
          style={inlineStyle(inline)}>{@render highlightedText(content)}</span
        >
      {/if}
    {/if}
  {/each}
{/snippet}

<article class="docx-viewer" class:fs={fullscreenState.active}>
  {#if pluginHtml}
    <div class="plugin-html-surface">{@html pluginHtml}</div>
  {:else}
    <div class="toolbar">
      <aside class="meta">
        <span>{stats.words.toLocaleString()} words</span>
        <span>{stats.blocks.toLocaleString()} blocks</span>
        {#if stats.tables > 0}<span>{stats.tables} tables</span>{/if}
        {#if byte_len > 0}<span>{byte_len.toLocaleString()} bytes</span>{/if}
      </aside>
    </div>

    <div class="layout">
      {#if outline.length > 0}
        <nav class="outline" aria-label="Document outline">
          <strong>Outline</strong>
          {#each outline as item}
            <a
              href={`#block-${item.index}`}
              style={`padding-left:${Math.max(0, (item.heading ?? 1) - 1) * 0.75}rem`}
              >{item.text}</a
            >
          {/each}
        </nav>
      {/if}

      <div class="page">
        <div class="body">
          {#each blocks as block, index}
            {#if block.kind === "paragraph"}
              {#if block.heading}
                <svelte:element this={headingTag(block.heading)} id={`block-${index}`}
                  >{#if block.inlines}{@render richInlines(
                      block.inlines,
                    )}{:else}{@render highlightedText(block.text)}{/if}</svelte:element
                >
              {:else}
                <p id={`block-${index}`}>
                  {#if block.inlines}{@render richInlines(
                      block.inlines,
                    )}{:else}{@render highlightedText(block.text)}{/if}
                </p>
              {/if}
            {:else if block.kind === "list-item"}
              <ul id={`block-${index}`} style={`--level:${block.level ?? 0}`}>
                <li>
                  {#if block.inlines}{@render richInlines(
                      block.inlines,
                    )}{:else}{@render highlightedText(block.text)}{/if}
                </li>
              </ul>
            {:else if block.kind === "table"}
              <div class="table-wrap" id={`block-${index}`}>
                <table>
                  <tbody>
                    {#each block.rich_rows ?? block.rows as row}
                      <tr
                        >{#each row as cell}<td
                            >{#if block.rich_rows}{@render richInlines(
                                cell.inlines ?? [],
                              )}{:else}{@render highlightedText(cell)}{/if}</td
                          >{/each}</tr
                      >
                    {/each}
                  </tbody>
                </table>
              </div>
            {:else if block.kind === "image"}
              <figure class="img-block" id={`block-${index}`}>
                {#if block.src}
                  <img
                    src={block.src}
                    alt={block.name ?? "Embedded image"}
                    title={block.name ?? "Embedded image"}
                    style="max-width:100%;height:auto;border-radius:0.4rem;"
                  />
                {:else}
                  <span class="img-placeholder">Embedded image</span>
                {/if}
              </figure>
            {:else if block.kind === "hyperlink"}
              {@const href = safeHref(block.href)}
              <p id={`block-${index}`}>
                {#if href}<a
                    {href}
                    target={href.startsWith("#") ? undefined : "_blank"}
                    rel={href.startsWith("#") ? undefined : "noopener noreferrer"}
                    >{@render highlightedText(block.text)}</a
                  >{:else}{@render highlightedText(block.text)}{/if}
              </p>
            {/if}
          {/each}
        </div>
      </div>
    </div>
  {/if}
</article>

<style>
  .docx-viewer {
    color: var(--text-primary);
  }
  .plugin-html-surface {
    overflow: auto;
    border-radius: 0.75rem;
  }
  .toolbar {
    position: sticky;
    top: 0.25rem;
    z-index: 3;
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border);
    padding: 0.75rem 0 0.6rem;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    color: var(--text-secondary);
    margin-top: 0.5rem;
    font-size: 0.75rem;
  }
  .meta span {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.12rem 0.45rem;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(10rem, 16rem) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
    margin-top: 1rem;
  }
  .outline {
    position: sticky;
    top: 5rem;
    max-height: calc(100vh - 6rem);
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 0.75rem;
    padding: 0.75rem;
    background: var(--bg-secondary);
  }
  .outline strong {
    display: block;
    font-size: 0.78rem;
    margin-bottom: 0.5rem;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .outline a {
    display: block;
    color: var(--text-primary);
    text-decoration: none;
    font-size: 0.8rem;
    line-height: 1.25;
    padding: 0.22rem 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .outline a:hover {
    color: var(--link);
  }
  .page {
    max-width: 76ch;
    margin: 0 auto 2rem;
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 0.9rem;
    box-shadow: 0 0.8rem 2rem rgba(0, 0, 0, 0.08);
  }
  .body {
    line-height: 1.68;
    padding: clamp(1rem, 4vw, 2.5rem);
  }
  /* Full screen: a wider reading measure and no side rail clutter. */
  .docx-viewer.fs .page {
    max-width: min(100%, 88ch);
  }
  .docx-viewer.fs .outline {
    display: none;
  }
  .docx-viewer.fs .layout {
    grid-template-columns: minmax(0, 1fr);
  }
  /* Offscreen blocks skip layout and paint, so huge documents mount and
     scroll without locking the compositor; intrinsic size keeps the
     scrollbar stable while blocks are skipped. */
  .body > * {
    content-visibility: auto;
    contain-intrinsic-size: auto 3rem;
  }
  .body :global(h1) {
    font-size: 1.8rem;
    margin: 1.4rem 0 0.7rem;
    letter-spacing: -0.02em;
  }
  .body :global(h2) {
    font-size: 1.45rem;
    margin: 1.2rem 0 0.6rem;
    letter-spacing: -0.01em;
  }
  .body :global(h3) {
    font-size: 1.2rem;
    margin: 1rem 0 0.45rem;
  }
  .body :global(h4),
  .body :global(h5),
  .body :global(h6) {
    font-size: 1rem;
    margin: 0.75rem 0 0.35rem;
  }
  p {
    margin: 0.58rem 0;
  }
  ul {
    padding-left: calc(1.35rem + var(--level, 0) * 1rem);
    margin: 0.28rem 0;
  }
  li {
    margin: 0.22rem 0;
  }
  .table-wrap {
    overflow: auto;
    margin: 1rem 0;
    border: 1px solid var(--border);
    border-radius: 0.55rem;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  td {
    border: 1px solid var(--border);
    padding: 0.42rem 0.6rem;
    font-size: 0.86rem;
    vertical-align: top;
  }
  tr:nth-child(even) td {
    background: var(--bg-secondary);
  }
  .img-placeholder {
    padding: 0.75rem;
    margin: 0.75rem 0;
    color: var(--text-secondary);
    font-style: italic;
    border: 1px dashed var(--border);
    border-radius: 0.5rem;
    text-align: center;
    font-size: 0.85rem;
    background: var(--bg-secondary);
  }
  .img-block {
    margin: 0.75rem 0;
    text-align: center;
  }
  .img-block img {
    max-width: 100%;
    height: auto;
    border-radius: 0.4rem;
    box-shadow: 0 0.3rem 1rem rgba(0, 0, 0, 0.08);
  }
  .body a {
    color: var(--link);
    text-decoration: underline;
    word-break: break-all;
  }
  .bold {
    font-weight: 700;
  }
  .italic {
    font-style: italic;
  }
  .underline {
    text-decoration: underline;
  }
  .body :global(p),
  .body :global(li),
  td {
    white-space: pre-wrap;
  }
  .inline-image img {
    display: inline-block;
    max-width: 100%;
    height: auto;
    vertical-align: middle;
  }
  :global(mark) {
    background: rgba(255, 213, 79, 0.6);
    border-radius: 0.15rem;
  }
  @media (max-width: 800px) {
    .layout {
      display: block;
    }
    .outline {
      display: none;
    }
    .toolbar {
      top: 3rem;
    }
    .page {
      border-radius: 0.65rem;
    }
  }
</style>
