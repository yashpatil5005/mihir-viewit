<script lang="ts">
  // Phase 4.6 — ICS calendar card viewer.
  // Naive Fwd-Line parser; surfaces events as cards.
  let { content = '' }: { content?: string } = $props();

  type Event = { summary?: string; start?: string; end?: string; location?: string; description?: string };

  let events: Event[] = $derived.by(() => parseIcs(content));

  function parseIcs(text: string): Event[] {
    const out: Event[] = [];
    let cur: Event | null = null;
    for (const rawLine of text.split(/\r?\n/)) {
      // Handle folded lines (RFC 5545 §3.1): continuation starts with space/tab
      if (/^[ \t]/.test(rawLine) && out.length > 0) continue;
      const line = rawLine.trim();
      if (line === 'BEGIN:VEVENT') { cur = {}; }
      else if (line === 'END:VEVENT') { if (cur) out.push(cur); cur = null; }
      else if (cur) {
        const m = line.match(/^([A-Z-]+):(.*)$/);
        if (!m) continue;
        const [, key, val] = m;
        if (key === 'SUMMARY') cur.summary = val;
        else if (key === 'DTSTART') cur.start = val;
        else if (key === 'DTEND') cur.end = val;
        else if (key === 'LOCATION') cur.location = val;
        else if (key === 'DESCRIPTION') cur.description = val;
      }
    }
    return out;
  }
</script>

<article class="ics-viewer">
  <aside class="meta">{events.length} event{(events.length ?? 0) === 1 ? '' : 's'}</aside>
  {#if events.length === 0}
    <p class="empty">No calendar events found.</p>
  {/if}
  {#each events as e}
    <div class="card">
      <h3>{e.summary ?? '(no title)'}</h3>
      {#if e.start}<p><strong>From:</strong> {e.start}</p>{/if}
      {#if e.end}<p><strong>To:</strong> {e.end}</p>{/if}
      {#if e.location}<p><strong>Where:</strong> {e.location}</p>{/if}
      {#if e.description}<p class="desc">{e.description}</p>{/if}
    </div>
  {/each}
</article>

<style>
  .ics-viewer { padding: 0.5rem 1rem; color: var(--text-primary); max-width: 700px; margin: 0 auto; }
  .meta { color: var(--text-secondary); margin-bottom: 1rem; font-size: 0.75rem; }
  .card { border: 1px solid var(--border); border-radius: 0.5rem; padding: 1rem; margin-bottom: 0.8rem; background: var(--bg-secondary); }
  .card h3 { margin: 0 0 0.5rem; color: var(--text-primary); }
  .card p { margin: 0.3rem 0; font-size: 0.85rem; }
  .desc { color: var(--text-secondary); white-space: pre-wrap; }
  .empty { color: var(--text-secondary); font-style: italic; }
</style>