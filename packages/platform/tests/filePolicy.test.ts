import { describe, it, expect } from "vitest";
import { fileExtension, checkFileBeforeRead, OPEN_BYTES_CAP } from "../src/filePolicy";
import { displayNameFromUri } from "../src/displayNameFromUri";

describe("fileExtension", () => {
  it("extracts the lowercased extension from a bare name", () => {
    expect(fileExtension("report.PDF")).toBe("pdf");
  });
  it("handles nested paths and strips query strings", () => {
    expect(fileExtension("/a/b/report.docx")).toBe("docx");
  });
  it("returns empty for dotfiles / no extension / null", () => {
    expect(fileExtension("archive.tar.gz")).toBe("gz");
    expect(fileExtension("noext")).toBe("");
    expect(fileExtension(null)).toBe("");
    expect(fileExtension(undefined)).toBe("");
  });
  it("handles windows separators", () => {
    expect(fileExtension("C:\\x\\y\\song.mp3")).toBe("mp3");
  });
});

describe("checkFileBeforeRead", () => {
  const fileLike = (name: string, size: number) => ({ name, size }) as File;

  it("always allows video/audio/pdf (streamed, never read wholesale)", () => {
    expect(checkFileBeforeRead(fileLike("clip.mp4", 1024 ** 3)).reject).toBe(false);
    expect(checkFileBeforeRead(fileLike("tune.aiff", 1024 ** 3)).reject).toBe(false);
    expect(checkFileBeforeRead(fileLike("book.pdf", 1024 ** 3)).reject).toBe(false);
  });

  it("opens non-media/pdf files under the cap", () => {
    expect(checkFileBeforeRead(fileLike("notes.txt", 1024)).reject).toBe(false);
    expect(checkFileBeforeRead(fileLike("book.epub", OPEN_BYTES_CAP - 1)).reject).toBe(false);
  });

  it("rejects oversized non-media/pdf files with open-with-external", () => {
    const r = checkFileBeforeRead(fileLike("big.xlsx", OPEN_BYTES_CAP + 1));
    expect(r.reject).toBe(true);
    if (r.reject) {
      expect(r.openWithExternal).toBe(true);
      expect(r.reason).toContain("MB");
    }
  });

  it("short-circuits on an existing viewitUri regardless of size", () => {
    const f = { name: "x.docx", size: 1024 ** 3, viewitUri: "viewit://..." } as File & {
      viewitUri?: string;
    };
    expect(checkFileBeforeRead(f).reject).toBe(false);
  });
});

describe("displayNameFromUri", () => {
  it("returns the decoded basename for file uris", () => {
    expect(displayNameFromUri("file:///sdcard/report%20final.pdf")).toBe("report final.pdf");
  });
  it("maps content:// video: and audio: to conventional names", () => {
    expect(displayNameFromUri("content://media/video%3A123")).toBe("video.mp4");
    expect(displayNameFromUri("content://media/audio:456")).toBe("audio.mp3");
  });
  it("returns null for non content/file schemes", () => {
    expect(displayNameFromUri("http://x/y.pdf")).toBeNull();
  });
  it("returns null for numeric tails and empty/overlong tails", () => {
    expect(displayNameFromUri("content://media/123")).toBeNull();
    expect(displayNameFromUri("file:///x/")).toBeNull();
  });
});
