import { marked } from "marked";

// SECURITY: renders raw HTML (marked passes <script> etc. through unchanged) and is
// used with {@html}. Phase 0 only renders the user's OWN company notes (trusted).
// Do NOT use this for vaults you did not author, or for any scraped/untrusted content
// (job descriptions, web research) — those must be sanitized (DOMPurify + a CSP) before
// rendering; see the design spec (§4.7) where that is mandated for the pipeline phases.
/** Render trusted markdown to HTML. (Synchronous; verify the installed marked API.) */
export function renderMarkdown(md: string): string {
  return marked.parse(md, { async: false }) as string;
}

/** Render a company note body, stripping a single leading "## Notes" heading
 *  (the panel already labels the section). */
export function renderNotes(md: string): string {
  const stripped = md.replace(/^\s*##\s+Notes\s*\r?\n+/, "");
  return renderMarkdown(stripped);
}
