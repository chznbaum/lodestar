import DOMPurify from "dompurify";
import { marked } from "marked";

// SECURITY: This module is safe for untrusted / LLM-generated content.
// marked passes raw HTML through unchanged, so the output is run through
// DOMPurify before being used with {@html}. Plain `dompurify` (not the
// isomorphic variant) is used because the app is a client-only SPA
// (src/routes/+layout.ts: ssr = false) — rendering only ever happens in
// the Tauri webview, which always has a real DOM. No CSP is included here;
// that is a separate hardening layer tracked separately.

/** Render markdown to sanitized HTML. Safe for untrusted / LLM-generated input. */
export function renderMarkdown(md: string): string {
  const raw = marked.parse(md, { async: false }) as string;
  return DOMPurify.sanitize(raw);
}

/** Render a company note body, stripping a single leading "## Notes" heading
 *  (the panel already labels the section). Sanitizes output — safe for
 *  LLM-generated notes. */
export function renderNotes(md: string): string {
  const stripped = md.replace(/^\s*##\s+Notes\s*\r?\n+/, "");
  return renderMarkdown(stripped);
}
