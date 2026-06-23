import { describe, it, expect } from "vitest";
import { formatCache } from "./check";
import type { Step } from "./check";

/** Minimal Step fixture — only cache fields matter for formatCache. */
const step = (
  cache_read_tokens: number | null,
  cache_write_tokens: number | null,
): Pick<Step, "cache_read_tokens" | "cache_write_tokens"> => ({
  cache_read_tokens,
  cache_write_tokens,
});

describe("formatCache", () => {
  it("returns empty string when both fields are null (common case: scrape/script steps)", () => {
    expect(formatCache(step(null, null))).toBe("");
  });

  it("shows read count when cache_read_tokens is present and write is null", () => {
    expect(formatCache(step(6656, null))).toBe("· cache 6,656 read");
  });

  it("shows both read and write counts when both are present", () => {
    expect(formatCache(step(6656, 7000))).toBe("· cache 6,656 read · 7,000 write");
  });

  it("shows only read count with no dangling separator when write is null", () => {
    const result = formatCache(step(6656, null));
    expect(result).not.toContain("write");
    expect(result).not.toMatch(/·\s*$/); // no trailing separator
    expect(result).toBe("· cache 6,656 read");
  });

  it("labels a write-only step as cache activity (cold-cache first call: prefix written, nothing read)", () => {
    expect(formatCache(step(null, 7000))).toBe("· cache 7,000 write");
  });
});
