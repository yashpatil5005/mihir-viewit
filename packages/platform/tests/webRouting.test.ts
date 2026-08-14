import { describe, expect, it } from "vitest";
import { webRouteKind } from "../src/webRouting";

describe("web route classification (parity with Rust sniff_ext)", () => {
  it("routes office, archive, and image extensions", () => {
    expect(webRouteKind("docx")).toBe("office");
    expect(webRouteKind("xlsx")).toBe("office");
    expect(webRouteKind("odt")).toBe("office");
    expect(webRouteKind("zip")).toBe("archive");
    expect(webRouteKind("7z")).toBe("archive");
    expect(webRouteKind("png")).toBe("image");
    expect(webRouteKind("heic")).toBe("image");
    expect(webRouteKind("svg")).toBe("image");
  });

  it("routes text, code, and structured-text extensions", () => {
    for (const ext of ["md", "json", "csv", "yaml", "xml", "toml", "ini"]) {
      expect(webRouteKind(ext)).toBe("text");
    }
    // NOTE: `ts` is deliberately excluded — like Rust's sniff_ext, it routes to
    // MPEG-TS video, not TypeScript. TypeScript uses `tsx`/`.mts` elsewhere.
    for (const ext of ["rs", "tsx", "jsx", "js", "py", "go", "cpp", "java", "kt", "swift"]) {
      expect(webRouteKind(ext)).toBe("text");
    }
    expect(webRouteKind("ics")).toBe("text");
    expect(webRouteKind("vcf")).toBe("text");
    expect(webRouteKind("desktop")).toBe("text");
    expect(webRouteKind("plist")).toBe("text");
  });

  it("routes fonts, pdf, and media", () => {
    expect(webRouteKind("ttf")).toBe("font");
    expect(webRouteKind("otf")).toBe("font");
    expect(webRouteKind("woff2")).toBe("font");
    expect(webRouteKind("pdf")).toBe("pdf");
    expect(webRouteKind("mp4")).toBe("video");
    expect(webRouteKind("webm")).toBe("video");
    expect(webRouteKind("mp3")).toBe("audio");
    expect(webRouteKind("flac")).toBe("audio");
  });

  it("normalizes dots and case", () => {
    expect(webRouteKind(".DOCX")).toBe("office");
    expect(webRouteKind("Md")).toBe("text");
    expect(webRouteKind("  Pdf ")).toBe("pdf");
    expect(webRouteKind(".Js")).toBe("text");
  });

  it("routes ebook and iWork to their WASM-backed kinds", () => {
    expect(webRouteKind("epub")).toBe("ebook");
    expect(webRouteKind("mobi")).toBe("ebook");
    expect(webRouteKind("azw3")).toBe("ebook");
    expect(webRouteKind("pages")).toBe("iwork");
    expect(webRouteKind("numbers")).toBe("iwork");
    expect(webRouteKind("key")).toBe("iwork");
  });

  it("leaves unknown and empty extensions as placeholder", () => {
    expect(webRouteKind("xyzq")).toBe("placeholder");
    expect(webRouteKind("")).toBe("placeholder");
    expect(webRouteKind(undefined as unknown as string)).toBe("placeholder");
  });
});
