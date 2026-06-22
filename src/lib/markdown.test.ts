// @vitest-environment jsdom
import { describe, it, expect } from "vitest";
import { renderMarkdown, renderNotes } from "./markdown";

describe("renderMarkdown", () => {
  it("renders basic markdown", () => {
    const html = renderMarkdown("# Title\n\nHello **world**");
    expect(html).toContain("<h1");
    expect(html).toContain("<strong>world</strong>");
  });

  it("strips <script> tags from untrusted input", () => {
    const html = renderMarkdown("<script>alert(1)</script>\n\n**safe**");
    expect(html).not.toContain("<script");
    expect(html).toContain("<strong>safe</strong>");
  });

  it("strips inline event handlers (onerror)", () => {
    const html = renderMarkdown("<img src=x onerror=alert(1)>");
    expect(html).not.toContain("onerror");
  });

  it("strips javascript: URL scheme from links but keeps the anchor", () => {
    const html = renderMarkdown("[x](javascript:alert(1))");
    expect(html).not.toContain("javascript:");
    // The anchor element survives (link text intact) — only the unsafe scheme is removed.
    expect(html).toContain("<a");
    expect(html).toContain("x");
  });
});

describe("renderNotes", () => {
  it("strips a leading '## Notes' heading then renders", () => {
    const html = renderNotes("## Notes\n\nDoes not require **degrees**.");
    expect(html).not.toContain(">Notes<");
    expect(html).toContain("<strong>degrees</strong>");
  });
  it("leaves other content intact when no Notes heading", () => {
    const html = renderNotes("Just a line.");
    expect(html).toContain("Just a line.");
  });
});
