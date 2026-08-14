import { describe, expect, it } from "vitest";
import {
  formatFromFile,
  isBuiltInFormat,
  resolveFormatCapability,
  type FormatProvider,
} from "../src/formatCapabilities";

const plugin = (id: string, formats: string[]): FormatProvider => ({ id, name: id, formats });

describe("format capability resolution", () => {
  it("derives formats from names first and MIME types second", () => {
    expect(formatFromFile("content://provider/report.DOCX", "application/octet-stream")).toBe(
      "docx",
    );
    expect(formatFromFile("content://provider/42", "application/vnd.ms-excel")).toBe("xls");
  });

  it("resolves installed, built-in, catalog, and unsupported states", () => {
    const installed = plugin("installed", ["future"]);
    const available = plugin("available", ["future", "brand-new"]);
    expect(resolveFormatCapability("future", [installed], [available]).kind).toBe(
      "installed-plugin",
    );
    expect(resolveFormatCapability("pdf", [], [plugin("better-pdf", ["pdf"])])).toMatchObject({
      kind: "built-in",
      available: [{ id: "better-pdf" }],
    });
    expect(resolveFormatCapability("brand-new", [], [available])).toMatchObject({
      kind: "available-plugin",
      available: [{ id: "available" }],
    });
    expect(resolveFormatCapability("unknown", [], [])).toEqual({
      kind: "unsupported",
      ext: "unknown",
      available: [],
    });
  });

  it("normalizes formats and deduplicates catalog variants by plugin id", () => {
    const result = resolveFormatCapability(
      ".DOCX",
      [],
      [plugin("office", ["docx"]), plugin("office", ["DOCX"])],
    );
    expect(result.available).toHaveLength(1);
  });

  it("does not advertise edit or JS plugins as generic document handlers", () => {
    expect(
      resolveFormatCapability(
        "future",
        [],
        [{ ...plugin("editor", ["future"]), base: "edit", runtime: "js" }],
      ).kind,
    ).toBe("unsupported");
  });

  it("offers an update when the installed version lacks the catalog format", () => {
    expect(
      resolveFormatCapability(
        "future",
        [plugin("provider", ["old"])],
        [plugin("provider", ["future"])],
      ).kind,
    ).toBe("available-plugin");
  });

  it("treats code/image/media/extended formats as built-in (parity with renderers)", () => {
    for (const ext of [
      // code / config
      "rs",
      "ts",
      "tsx",
      "js",
      "py",
      "go",
      "cpp",
      "java",
      "kt",
      "swift",
      "sql",
      "sh",
      "yaml",
      "toml",
      "ini",
      "html",
      "css",
      // extended/raw images
      "djvu",
      "dds",
      "exr",
      "tga",
      "dng",
      "cr2",
      // structured text
      "ics",
      "vcf",
      "desktop",
      "plist",
      // fonts
      "ttf",
      "otf",
      // media
      "mp4",
      "webm",
      "mp3",
      "flac",
      // archives
      "7z",
      "tar",
      "zst",
    ]) {
      expect(resolveFormatCapability(ext, [], []).kind, ext).toBe("built-in");
      expect(isBuiltInFormat(ext), ext).toBe(true);
    }
  });

  it("maps MIME types across documents, images, audio, and video", () => {
    expect(formatFromFile(null, "application/pdf")).toBe("pdf");
    expect(formatFromFile(null, "application/vnd.ms-excel")).toBe("xls");
    expect(formatFromFile(null, "image/png")).toBe("png");
    expect(formatFromFile(null, "audio/mpeg")).toBe("mp3");
    expect(formatFromFile(null, "video/webm")).toBe("webm");
    expect(formatFromFile(null, "font/woff2")).toBe("woff2");
    expect(formatFromFile(null, "application/epub+zip")).toBe("epub");
    expect(formatFromFile(null, "text/vcard")).toBe("vcf");
    expect(formatFromFile(null, "unknown/x-thing")).toBe("");
  });
});
