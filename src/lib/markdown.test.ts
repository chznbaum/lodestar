import { describe, it, expect } from "vitest";
import { renderMarkdown, renderNotes } from "./markdown";

describe("renderMarkdown", () => {
  it("renders basic markdown", () => {
    const html = renderMarkdown("# Title\n\nHello **world**");
    expect(html).toContain("<h1");
    expect(html).toContain("<strong>world</strong>");
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
