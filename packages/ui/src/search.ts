// Phase 5.2 — In-file search.
// Pure-TS highlight + scroll-to match utility. Viewer components consume this.

export type Match = { index: number; length: number };

export function findAllMatches(haystack: string, query: string, caseSensitive = false): Match[] {
  if (!query) return [];
  const flags = caseSensitive ? "g" : "gi";
  const safe = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const re = new RegExp(safe, flags);
  const out: Match[] = [];
  let m: RegExpExecArray | null;
  while ((m = re.exec(haystack)) !== null) {
    out.push({ index: m.index, length: m[0].length });
    if (m.index === re.lastIndex) re.lastIndex++; // zero-len match guard
  }
  return out;
}

export function highlightHtml(text: string, matches: Match[]): string {
  // Returns HTML with <mark> wrappers. Caller must ensure text has no active HTML
  // (used for plain-text viewers only).
  if (matches.length === 0 || !text) return escapeHtml(text);
  let out = "";
  let cursor = 0;
  for (const m of matches) {
    out += escapeHtml(text.slice(cursor, m.index));
    out += "<mark>" + escapeHtml(text.slice(m.index, m.index + m.length)) + "</mark>";
    cursor = m.index + m.length;
  }
  out += escapeHtml(text.slice(cursor));
  return out;
}

export function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
