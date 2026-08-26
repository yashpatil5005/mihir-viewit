import { describe, expect, it } from "vitest";
import { findAllMatches, highlightHtml, escapeHtml } from "../src/search";

describe("search utility functions", () => {
  it("finds case-insensitive matches", () => {
    const matches = findAllMatches("Hello World hello", "hello", false);
    expect(matches.length).toBe(2);
    expect(matches[0]).toEqual({ index: 0, length: 5 });
    expect(matches[1]).toEqual({ index: 12, length: 5 });
  });

  it("finds case-sensitive matches", () => {
    const matches = findAllMatches("Hello World hello", "hello", true);
    expect(matches.length).toBe(1);
    expect(matches[0]).toEqual({ index: 12, length: 5 });
  });

  it("escapes regex special chars in query", () => {
    const matches = findAllMatches("test (a+b) query [1]", "(a+b)", false);
    expect(matches.length).toBe(1);
    expect(matches[0]).toEqual({ index: 5, length: 5 });
  });

  it("returns empty matches for empty query", () => {
    expect(findAllMatches("some text", "", false)).toEqual([]);
  });

  it("escapes HTML properly", () => {
    expect(escapeHtml("<script>alert('x')&\"</script>")).toBe(
      "&lt;script&gt;alert('x')&amp;\"&lt;/script&gt;",
    );
  });

  it("wraps matches in <mark> tags", () => {
    const text = "apple banana apple";
    const matches = findAllMatches(text, "apple", false);
    const highlighted = highlightHtml(text, matches);
    expect(highlighted).toBe("<mark>apple</mark> banana <mark>apple</mark>");
  });
});
