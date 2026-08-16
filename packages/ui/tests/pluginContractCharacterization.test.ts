import { describe, expect, it, vi } from "vitest";
import {
  installManifest,
  isRestartToApplyError,
  RestartRequiredError,
  type PluginInfo,
} from "../src/pluginBridge";

describe("plugin install contract", () => {
  it("drains native envelopes queued before the lazy plugin bridge loads", async () => {
    const queuedWindow: {
      __viewitBridgeQueue?: unknown[];
      __viewitBridgeDispatch?: (payload: unknown) => boolean;
    } = {};
    queuedWindow.__viewitBridgeQueue = [{ id: "early", event: "error", error: "ignored" }];
    vi.stubGlobal("window", queuedWindow);
    vi.resetModules();
    await import("../src/pluginBridge");
    expect(queuedWindow.__viewitBridgeQueue).toBeUndefined();
    expect(typeof queuedWindow.__viewitBridgeDispatch).toBe("function");
    vi.unstubAllGlobals();
  });

  it("uses a typed restart-required outcome instead of message matching", () => {
    expect(isRestartToApplyError(new RestartRequiredError("apply after restart"))).toBe(true);
    expect(isRestartToApplyError(new Error("restart the app to apply"))).toBe(false);
  });

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
