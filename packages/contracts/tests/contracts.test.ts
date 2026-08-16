import { describe, expect, it } from "vitest";
import {
  ContractValidationError,
  parsePluginPackageManifestV1,
  parseProviderDescriptorV1,
  parseServiceDescriptorV1,
} from "../src/index";

describe("canonical ViewIt contracts", () => {
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
});
