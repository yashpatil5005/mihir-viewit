import { describe, expect, it } from "vitest";
import type { PluginInfo } from "../src/pluginBridge";
import {
  NON_ARCHIVE_BUNDLE_EXTS,
  hasPptxJsRenderer,
  selectBasePlugin,
  selectDetectedOfficePlugin,
  selectNativeOfficePlugin,
  selectPptxJsPlugin,
} from "../src/runtimeRouting";

const plugin = (overrides: Partial<PluginInfo>): PluginInfo => ({
  id: "plugin",
  name: "Plugin",
  version: "1",
  description: "",
  formats: [],
  ...overrides,
});

describe("format plugin routing", () => {
  it("keeps iWork bundles out of generic archive sniff fallback", () => {
    expect(NON_ARCHIVE_BUNDLE_EXTS.has("pages")).toBe(true);
    expect(NON_ARCHIVE_BUNDLE_EXTS.has("numbers-template")).toBe(true);
  });
});

describe("PPTX JS selection", () => {
  const nativeHybrid = plugin({
    id: "office-universal",
    runtime: "native",
    formats: ["pptx"],
    jsEntry: "index.js",
  });
  const jsRenderer = plugin({ id: "pptx-js", runtime: "js", formats: ["pptx"] });

  it("never treats a native hybrid plugin as the JS renderer", () => {
    expect(hasPptxJsRenderer(nativeHybrid)).toBe(false);
    expect(selectPptxJsPlugin([nativeHybrid], null)).toBeNull();
  });

  it("honors builtin, preferred JS, missing preferred, and automatic choices", () => {
    expect(selectPptxJsPlugin([jsRenderer], "__builtin__")).toBeNull();
    expect(selectPptxJsPlugin([jsRenderer], "pptx-js")).toBe(jsRenderer);
    expect(selectPptxJsPlugin([jsRenderer], "missing")).toBeNull();
    expect(selectPptxJsPlugin([nativeHybrid, jsRenderer], null)).toBe(jsRenderer);
  });
});

describe("native office selection", () => {
  const office = plugin({ id: "office-universal", runtime: "native", formats: ["docx"] });
  const alternate = plugin({ id: "alternate", runtime: "dex", formats: ["docx"] });
  const js = plugin({ id: "js", runtime: "js", formats: ["docx"] });

  it("honors builtin and preferred native runtime", () => {
    expect(selectNativeOfficePlugin([office], "docx", "__builtin__")).toBeNull();
    expect(selectNativeOfficePlugin([office, alternate], "docx", "alternate")).toBe(alternate);
    expect(selectNativeOfficePlugin([office], "docx", "missing")).toBeNull();
  });

  it("prefers office-universal automatically, skips JS and failed plugins", () => {
    expect(selectNativeOfficePlugin([alternate, js, office], "docx", null)).toBe(office);
    expect(selectNativeOfficePlugin([office, alternate], "docx", null, "office-universal")).toBe(
      alternate,
    );
  });

  it("uses a native parse provider for preferred JS and missing preferences after detection", () => {
    expect(selectDetectedOfficePlugin([office, alternate], "docx", "pptx-js")).toBe(office);
    expect(selectDetectedOfficePlugin([office, alternate], "docx", "missing")).toBe(office);
    expect(selectDetectedOfficePlugin([office], "docx", "__builtin__")).toBeNull();
    expect(selectDetectedOfficePlugin([office, alternate], "docx", "pptx-js", office.id)).toBe(
      alternate,
    );
  });
});

describe("play/edit base selection", () => {
  const editor = plugin({ id: "editor", base: "edit", runtime: "js", formats: ["md"] });
  const genericEditor = plugin({ id: "generic", base: "edit", runtime: "js", formats: [] });
  const player = plugin({ id: "player", base: "play", runtime: "js", formats: ["mp3"] });

  it("requires exact play support and permits edit fallback", () => {
    expect(selectBasePlugin([player], "play", "mp3", false)).toBe(player);
    expect(selectBasePlugin([player], "play", "wav", false)).toBeNull();
    expect(selectBasePlugin([genericEditor, editor], "edit", "md", true)).toBe(editor);
    expect(selectBasePlugin([genericEditor], "edit", "txt", true)).toBe(genericEditor);
  });
});
