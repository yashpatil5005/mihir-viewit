import { describe, expect, it, vi } from "vitest";
import { searchFilesFuzzy, searchFilesContent } from "../src/browse";

describe("browse search platform APIs", () => {
  it("returns empty array in non-Tauri environment without errors", async () => {
    const fuzzy = await searchFilesFuzzy("test");
    const content = await searchFilesContent("test");
    expect(fuzzy).toEqual([]);
    expect(content).toEqual([]);
  });

  it("handles isRegex parameter on searchFilesContent", async () => {
    const content = await searchFilesContent("test.*pattern", undefined, 50, true);
    expect(content).toEqual([]);
  });
});
