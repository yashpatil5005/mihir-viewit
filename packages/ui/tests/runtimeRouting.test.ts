import { describe, expect, it } from "vitest";
import type { PluginInfo } from "../src/pluginBridge";
import {
  NON_ARCHIVE_BUNDLE_EXTS,
  hasMeaningfulPresentationContent,
  resolveIworkFormat,
  resolveInstalledProvider,
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
    ).toBe("office-universal");
  });

  it("selects PPTX renderer only when its parser service is available", () => {
    expect(
      resolveInstalledProvider([pptx], "viewit.document.render", "pptx", null).selected,
    ).toBeNull();
    expect(
      resolveInstalledProvider([pptx], "viewit.document.render", "pptx", null, [
        { service: "viewit.document.parse", contractVersion: 1 },
      ]).selected?.packageId,
    ).toBe("pptx-vanilla");
  });

  it("matches current player and editor package selection", () => {
    expect(
      resolveInstalledProvider([player], "viewit.media.play", "mp3", null).selected?.packageId,
    ).toBe("player-base");
    expect(
      resolveInstalledProvider([editor], "viewit.document.edit", "md", null).selected?.packageId,
    ).toBe("editor-base");
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

  it("rejects placeholder-only presentation output", () => {
    expect(
      hasMeaningfulPresentationContent({
        slides: [{ title: "", body: "", elements: [{ kind: "text", text: "<number>" }] }],
      }),
    ).toBe(false);
    expect(
      hasMeaningfulPresentationContent({
        slides: [
          {
            title: "",
            body: "",
            elements: [
              { paragraphs: [{ runs: [{ text: "Runtime rendering verifies content" }] }] },
            ],
          },
        ],
      }),
    ).toBe(true);
  });
});
