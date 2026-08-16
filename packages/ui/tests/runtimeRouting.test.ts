import { describe, expect, it } from "vitest";
import type { PluginInfo } from "../src/pluginBridge";
import {
  NON_ARCHIVE_BUNDLE_EXTS,
  hasPptxJsRenderer,
  resolveIworkFormat,
  resolveInstalledProvider,
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

describe("provider resolver shadow decisions", () => {
  const office = plugin({ id: "office-universal", runtime: "native", formats: ["docx", "pptx"] });
  const pptx = plugin({ id: "pptx-vanilla", runtime: "js", formats: ["pptx"] });
  const player = plugin({ id: "player-base", base: "play", runtime: "js", formats: ["mp3"] });
  const editor = plugin({ id: "editor-base", base: "edit", runtime: "js", formats: ["md"] });

  it("matches automatic Office package selection", () => {
    expect(
      resolveInstalledProvider([office], "viewit.document.parse", "docx", null).selected?.packageId,
    ).toBe(selectNativeOfficePlugin([office], "docx", null)?.id);
  });

  it("selects PPTX renderer only when its parser service is available", () => {
    expect(
      resolveInstalledProvider([pptx], "viewit.document.render", "pptx", null).selected,
    ).toBeNull();
    expect(
      resolveInstalledProvider([pptx], "viewit.document.render", "pptx", null, [
        { service: "viewit.document.parse", contractVersion: 1 },
      ]).selected?.packageId,
    ).toBe(selectPptxJsPlugin([pptx], null)?.id);
  });

  it("matches current player and editor package selection", () => {
    expect(
      resolveInstalledProvider([player], "viewit.media.play", "mp3", null).selected?.packageId,
    ).toBe(selectBasePlugin([player], "play", "mp3", false)?.id);
    expect(
      resolveInstalledProvider([editor], "viewit.document.edit", "md", null).selected?.packageId,
    ).toBe(selectBasePlugin([editor], "edit", "md", true)?.id);
  });

  it("maps legacy package preferences to canonical provider IDs", () => {
    expect(
      resolveInstalledProvider([office], "viewit.document.parse", "docx", "office-universal")
        .selected?.id,
    ).toBe("office-universal.parse");
  });

  it("excludes providers quarantined by persisted Android health", () => {
    const unhealthy = {
      ...office,
      providerHealth: {
        "office-universal.parse": {
          state: "quarantined",
          consecutiveFailures: 3,
          totalFailures: 3,
          updatedAt: 1,
        },
      },
    };
    expect(
      resolveInstalledProvider([unhealthy], "viewit.document.parse", "docx", null).selected,
    ).toBeNull();
  });
});

describe("format plugin routing", () => {
  it("keeps iWork bundles out of generic archive sniff fallback", () => {
    expect(NON_ARCHIVE_BUNDLE_EXTS.has("pages")).toBe(true);
    expect(NON_ARCHIVE_BUNDLE_EXTS.has("numbers-template")).toBe(true);
  });

  it("recognizes iWork provenance from either the source document or extension", () => {
    expect(resolveIworkFormat("pages")).toBe("pages");
    expect(resolveIworkFormat(".NUMBERS")).toBe("numbers");
    expect(resolveIworkFormat("blob", "iwork-key")).toBe("key");
    expect(resolveIworkFormat("docx")).toBeNull();
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
