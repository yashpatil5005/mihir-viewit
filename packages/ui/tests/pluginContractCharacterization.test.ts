import { describe, expect, it } from "vitest";
import { legacyInstallManifest, type PluginInfo } from "../src/pluginBridge";

describe("legacy plugin contract characterization", () => {
  it("documents policy fields currently lost when the WebView reconstructs an install manifest", () => {
    const plugin = {
      id: "characterization-plugin",
      name: "Characterization Plugin",
      version: "2.0.0",
      description: "baseline",
      formats: ["txt"],
      downloadUrl: "https://example.test/plugin.zip",
      checksum: "abc123",
      sizeBytes: 10,
      installedSizeBytes: 20,
      abi: "arm64-v8a",
      runtime: "native",
      base: "tool",
      capabilities: ["example.execute"],
    } satisfies PluginInfo;

    const manifest = legacyInstallManifest(plugin) as Record<string, unknown>;

    expect(manifest.minAppVersion).toBe(1);
    expect(manifest).not.toHaveProperty("abiVersion");
    expect(manifest).not.toHaveProperty("base");
    expect(manifest).not.toHaveProperty("capabilities");
  });
});
