import { describe, expect, it } from "vitest";
import {
  ContractValidationError,
  parsePluginPackageManifestV1,
  parseProviderDescriptorV1,
  parseServiceDescriptorV1,
  parseExternalDocumentV1,
} from "../src/index";
import providers from "../catalog/providers.v1.json" with { type: "json" };
import services from "../catalog/services.v1.json" with { type: "json" };
import officeManifest from "../manifests/office-universal.v1.json" with { type: "json" };
import { validateProviderGraph } from "../src/dependencyGraph";

describe("canonical ViewIt contracts", () => {
  it("validates every registered service and provider", () => {
    expect(services.map(parseServiceDescriptorV1)).toHaveLength(11);
    expect(providers.map(parseProviderDescriptorV1)).toHaveLength(10);
    expect(validateProviderGraph(providers.map(parseProviderDescriptorV1)).issues).toEqual([]);
  });

  it("validates generated canonical package manifests", () => {
    expect(parsePluginPackageManifestV1(officeManifest).providers).toHaveLength(1);
  });
  it("accepts versioned service and provider descriptors", () => {
    expect(
      parseServiceDescriptorV1({
        id: "viewit.document.parse",
        contractVersion: 1,
        requestSchema: "viewit://schemas/document-parse-request.v1",
        responseSchema: "viewit://schemas/document.v1",
        cardinality: "single",
        scope: "operation",
      }).id,
    ).toBe("viewit.document.parse");

    expect(
      parseProviderDescriptorV1({
        id: "office-universal.parse",
        packageId: "office-universal",
        service: "viewit.document.parse",
        contractVersion: 1,
        runtime: "android-dex-jni",
        trustClass: "trusted-in-process",
        formats: ["docx", "xlsx", "pptx"],
      }).runtime,
    ).toBe("android-dex-jni");
  });

  it("requires JavaScript and native entry points according to runtime", () => {
    expect(() =>
      parsePluginPackageManifestV1({
        schemaVersion: 1,
        id: "player-base",
        name: "Player Base",
        version: "1.0.0",
        minAppVersion: 1,
        runtime: "webview-js",
        providers: [],
      }),
    ).toThrow(ContractValidationError);
  });

  it("rejects service names outside the ViewIt namespace", () => {
    expect(() =>
      parseServiceDescriptorV1({
        id: "document.parse",
        contractVersion: 1,
        requestSchema: "request",
        responseSchema: "response",
        cardinality: "single",
        scope: "operation",
      }),
    ).toThrow(ContractValidationError);
  });

  it("validates plugin document variants before rendering", () => {
    expect(
      parseExternalDocumentV1({
        kind: "docx",
        blocks: [{ kind: "paragraph", text: "Hello" }],
        byte_len: 5,
      }).kind,
    ).toBe("docx");
    expect(() => parseExternalDocumentV1({ kind: "docx", byte_len: 5 })).toThrow(
      ContractValidationError,
    );
    expect(() => parseExternalDocumentV1({ kind: "unknown" })).toThrow(ContractValidationError);
  });

  it("rejects excessively deep plugin document values", () => {
    let value: Record<string, unknown> = { kind: "docx", blocks: [], byte_len: 0 };
    for (let index = 0; index < 34; index++) value = { nested: value };
    expect(() => parseExternalDocumentV1(value)).toThrow("nesting depth");
  });
});
