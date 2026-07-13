<script lang="ts">
  // Phase 4.6 — VCF contact card viewer.
  // Naive Fwd-Line parser; surfaces contacts as cards.
  let { content = '' }: { content?: string } = $props();

  type Contact = { fn?: string; email?: string; tel?: string; org?: string; title?: string };

  let contacts: Contact[] = $derived.by(() => parseVcf(content));

  function parseVcf(text: string): Contact[] {
    const out: Contact[] = [];
    let cur: Contact | null = null;
    for (const rawLine of text.split(/\r?\n/)) {
      if (/^[ \t]/.test(rawLine) && out.length > 0) continue;
      const line = rawLine.trim();
      if (line === 'BEGIN:VCARD') { cur = {}; }
      else if (line === 'END:VCARD') { if (cur) out.push(cur); cur = null; }
      else if (cur) {
        // VCard lines look like "KEY;PARAM:VALUE" — naive strip before colon
        const ci = line.indexOf(':');
        if (ci === -1) continue;
        const key = line.slice(0, ci).split(';')[0];
        const val = line.slice(ci + 1);
        if (key === 'FN') cur.fn = val;
        else if (key === 'EMAIL') cur.email = (cur.email ?? '') + val + ' ';
        else if (key === 'TEL') cur.tel = (cur.tel ?? '') + val + ' ';
        else if (key === 'ORG') cur.org = val;
        else if (key === 'TITLE') cur.title = val;
      }
    }
    return out;
  }
</script>

<article class="vcf-viewer">
  <aside class="meta">{contacts.length} contact{(contacts.length ?? 0) === 1 ? '' : 's'}</aside>
  {#if contacts.length === 0}
    <p class="empty">No contacts found.</p>
  {/if}
  {#each contacts as c}
    <div class="card">
      <h3>{c.fn ?? '(no name)'}</h3>
      {#if c.title}<p class="title"><strong>{c.title}</strong>{#if c.org} · {c.org}{/if}</p>{/if}
      {#if c.email}<p><strong>Email:</strong> {c.email}</p>{/if}
      {#if c.tel}<p><strong>Tel:</strong> {c.tel}</p>{/if}
    </div>
  {/each}
</article>

<script context="module" lang="ts">
  // 동일 scope에서 충돌 방지용 alias
</script>

<style>
  .vcf-viewer { padding: 0.5rem 1rem; color: var(--text-primary); max-width: 500px; margin: 0 auto; }
  .meta { color: var(--text-secondary); margin-bottom: 1rem; font-size: 0.75rem; }
  .card { border: 1px solid var(--border); border-radius: 0.5rem; padding: 1rem; margin-bottom: 0.8rem; background: var(--bg-secondary); }
  .card h3 { margin: 0 0 0.3rem; color: var(--text-primary); }
  .card p { margin: 0.3rem 0; font-size: 0.85rem; }
  .card .title { color: var(--link); font-size: 0.8rem; }
  .empty { color: var(--text-secondary); font-style: italic; }
</style>