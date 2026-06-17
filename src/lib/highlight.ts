/** Pure, Svelte-free text-highlighting helper shared by the Combobox and the
 *  company search. Splits `text` into matched / unmatched runs so the caller
 *  can wrap matched runs in <mark>. Marks the FIRST case-insensitive occurrence
 *  of `query`; an empty/whitespace query (or no match) yields a single unmarked run. */
export interface Segment {
  text: string;
  mark: boolean;
}

export function segments(text: string, query: string): Segment[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [{ text, mark: false }];
  const idx = text.toLowerCase().indexOf(q);
  if (idx < 0) return [{ text, mark: false }];
  const out: Segment[] = [];
  if (idx > 0) out.push({ text: text.slice(0, idx), mark: false });
  out.push({ text: text.slice(idx, idx + q.length), mark: true });
  if (idx + q.length < text.length) out.push({ text: text.slice(idx + q.length), mark: false });
  return out;
}
