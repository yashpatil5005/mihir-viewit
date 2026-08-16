import { describe, expect, it } from "vitest";
import { installManifest, type PluginInfo } from "../src/pluginBridge";

describe("plugin install contract", () => {
  it("preserves verified policy fields when the WebView projects an install manifest", () => {
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
      abiVersion: 7,
      minAppVersion: 42,
      runtime: "native",
      base: "tool",
      capabilities: ["example.execute"],
    } satisfies PluginInfo;

    const manifest = installManifest(plugin) as Record<string, unknown>;

    expect(manifest.minAppVersion).toBe(42);
    expect(manifest.abiVersion).toBe(7);
    expect(manifest.base).toBe("tool");
    expect(manifest.capabilities).toEqual(["example.execute"]);
  });
});
